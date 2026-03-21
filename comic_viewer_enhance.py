"""
漫画浏览器 (增强版) - Comic Viewer Enhanced
支持单页/双页/长图模式、预加载、主题切换、快捷键操作
"""
import os
import sys
from PyQt5.QtWidgets import (
    QApplication, QMainWindow, QGraphicsView, QGraphicsScene, QFileDialog,
    QVBoxLayout, QWidget, QHBoxLayout, QPushButton, QLabel, QCheckBox,
    QFrame, QSpacerItem, QSizePolicy, QMenu, QAction, QDialog,
    QSlider, QColorDialog, QComboBox, QFormLayout, QDialogButtonBox, QMessageBox,
    QProgressBar
)
from PyQt5.QtGui import QPixmap, QImage, QIcon, QColor, QPainter, QKeySequence
from PyQt5.QtCore import Qt, QDir, QTimer, QSize, QSettings, QThread, pyqtSignal
from threading import Thread


# 支持的图片格式
SUPPORTED_FORMATS = ["*.jpg", "*.jpeg", "*.png", "*.bmp", "*.webp", "*.gif", "*.tiff"]

# 样式表
STYLE_SHEET = """
    QMainWindow { background-color: #f0f0f0; }
    QPushButton {
        background-color: #4a86e8; color: white; border: none;
        padding: 8px 16px; border-radius: 4px; font-size: 14px;
    }
    QPushButton:hover { background-color: #3a76d8; }
    QPushButton:pressed { background-color: #2a66c8; }
    QPushButton:disabled { background-color: #a0a0a0; }
    QLabel { color: #333333; font-size: 14px; }
    QCheckBox { color: #333333; font-size: 14px; spacing: 8px; }
    QCheckBox::indicator { width: 18px; height: 18px; }
    QGraphicsView {
        background-color: #ffffff; border: 1px solid #dddddd; border-radius: 4px;
    }
    QMenuBar { background-color: #ffffff; border-bottom: 1px solid #dddddd; }
    QMenuBar::item { padding: 4px 8px; background-color: transparent; }
    QMenuBar::item:selected { background-color: #e0e0e0; }
    QMenu { background-color: #ffffff; border: 1px solid #dddddd; }
    QMenu::item { padding: 6px 20px; }
    QMenu::item:selected { background-color: #e0e0e0; }
    QProgressBar {
        border: 1px solid #dddddd; border-radius: 4px; text-align: center;
        height: 6px; background-color: #f0f0f0;
    }
    QProgressBar::chunk { background-color: #4a86e8; border-radius: 3px; }
"""

THEMES = {
    "浅色": "QMainWindow { background-color: #f0f0f0; } QGraphicsView { background-color: #ffffff; }",
    "深色": """
        QMainWindow { background-color: #2d2d2d; }
        QGraphicsView { background-color: #1a1a1a; }
        QLabel { color: #ffffff; }
        QCheckBox { color: #ffffff; }
    """,
    "护眼": "QMainWindow { background-color: #f0f7eb; } QGraphicsView { background-color: #ffffff; }",
}


class PreloadThread(QThread):
    """后台预加载线程，使用 QImage 线程安全"""
    loaded = pyqtSignal(int, QImage)

    def __init__(self, image_files, center_index, cache):
        super().__init__()
        self.image_files = image_files
        self.center_index = center_index
        self.cache = cache

    def run(self):
        start = max(0, self.center_index - 3)
        end = min(len(self.image_files), self.center_index + 6)
        for i in range(start, end):
            if i not in self.cache:
                img = QImage(self.image_files[i])
                if not img.isNull():
                    self.loaded.emit(i, img)


class SettingsDialog(QDialog):
    """设置对话框"""

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setWindowTitle("设置")
        self.setMinimumWidth(300)

        layout = QFormLayout(self)

        self.scale_slider = QSlider(Qt.Horizontal)
        self.scale_slider.setMinimum(50)
        self.scale_slider.setMaximum(200)
        self.scale_slider.setValue(100)
        self.scale_slider.setTickPosition(QSlider.TicksBelow)
        self.scale_slider.setTickInterval(10)
        layout.addRow("图片缩放:", self.scale_slider)

        self.bg_color_btn = QPushButton("选择颜色")
        self.bg_color_btn.clicked.connect(self._choose_bg_color)
        layout.addRow("背景颜色:", self.bg_color_btn)

        self.theme_combo = QComboBox()
        self.theme_combo.addItems(list(THEMES.keys()))
        layout.addRow("主题:", self.theme_combo)

        button_box = QDialogButtonBox(QDialogButtonBox.Ok | QDialogButtonBox.Cancel)
        button_box.accepted.connect(self.accept)
        button_box.rejected.connect(self.reject)
        layout.addRow(button_box)

    def _choose_bg_color(self):
        color = QColorDialog.getColor()
        if color.isValid():
            self.bg_color_btn.setStyleSheet(
                f"background-color: {color.name()}; border: 1px solid #cccccc; border-radius: 4px;"
            )


class ComicViewer(QMainWindow):
    """漫画浏览器主窗口"""

    PRELOAD_COUNT = 6

    def __init__(self):
        super().__init__()
        self.setWindowTitle("漫画浏览器")
        self.setGeometry(100, 100, 1200, 800)

        # 状态
        self.settings = QSettings("ManhuaViewer", "ComicViewer")
        self.image_files: list[str] = []
        self.current_index = 0
        self.scale_factor = 1.0
        self.preloaded_images: dict[int, QPixmap] = {}
        self.double_page_mode = False
        self.long_image_mode = False
        self._preload_thread: PreloadThread | None = None

        self._init_ui()
        self._connect_signals()
        self._restore_state()

    # ── UI 初始化 ──────────────────────────────────────────────

    def _init_ui(self):
        self.setStyleSheet(STYLE_SHEET)
        self._create_menu_bar()

        central = QWidget()
        self.setCentralWidget(central)
        root_layout = QVBoxLayout(central)
        root_layout.setContentsMargins(10, 10, 10, 10)
        root_layout.setSpacing(10)

        # 工具栏
        toolbar_frame = QFrame()
        toolbar_frame.setStyleSheet(
            "QFrame { background-color: #ffffff; border-radius: 8px; padding: 10px; }"
        )
        toolbar = QHBoxLayout(toolbar_frame)
        toolbar.setSpacing(10)

        self.btn_open = QPushButton("打开文件夹")
        self.btn_prev = QPushButton("上一页 (←)")
        self.btn_next = QPushButton("下一页 (→)")
        self.check_double = QCheckBox("双页模式")
        self.check_long = QCheckBox("长图模式")
        self.label_status = QLabel("状态: 未加载图片")

        toolbar.addWidget(self.btn_open)
        toolbar.addWidget(self.btn_prev)
        toolbar.addWidget(self.btn_next)
        toolbar.addWidget(self.check_double)
        toolbar.addWidget(self.check_long)
        toolbar.addItem(QSpacerItem(40, 20, QSizePolicy.Expanding, QSizePolicy.Minimum))
        toolbar.addWidget(self.label_status)
        root_layout.addWidget(toolbar_frame)

        # 进度条
        self.progress_bar = QProgressBar()
        self.progress_bar.setMaximumHeight(6)
        self.progress_bar.setTextVisible(False)
        self.progress_bar.hide()
        root_layout.addWidget(self.progress_bar)

        # 图片显示区
        view_frame = QFrame()
        view_frame.setStyleSheet(
            "QFrame { background-color: #ffffff; border-radius: 8px; margin-top: 10px; }"
        )
        view_layout = QHBoxLayout(view_frame)
        view_layout.setSpacing(10)
        view_layout.setContentsMargins(10, 10, 10, 10)

        self.view_left = QGraphicsView()
        self.view_right = QGraphicsView()
        self.scene_left = QGraphicsScene()
        self.scene_right = QGraphicsScene()

        for view in (self.view_left, self.view_right):
            view.setRenderHint(QPainter.Antialiasing)
            view.setRenderHint(QPainter.SmoothPixmapTransform)
            view.setViewportUpdateMode(QGraphicsView.FullViewportUpdate)
            view.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
            view.setVerticalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
            view.setMouseTracking(True)
            view.viewport().setMouseTracking(True)
            view.viewport().installEventFilter(self)
            view.setTransformationAnchor(QGraphicsView.AnchorUnderMouse)
            view.setResizeAnchor(QGraphicsView.AnchorUnderMouse)

        self.view_left.setScene(self.scene_left)
        self.view_right.setScene(self.scene_right)
        view_layout.addWidget(self.view_left)
        self.view_right.hide()

        root_layout.addWidget(view_frame)
        root_layout.setStretch(1, 1)

    def _create_menu_bar(self):
        menubar = self.menuBar()

        # 文件
        file_menu = menubar.addMenu("文件")

        open_act = QAction("打开文件夹", self)
        open_act.setShortcut(QKeySequence("Ctrl+O"))
        open_act.triggered.connect(self.open_folder)
        file_menu.addAction(open_act)

        self.recent_menu = QMenu("最近打开", self)
        file_menu.addMenu(self.recent_menu)
        self._update_recent_menu()

        file_menu.addSeparator()

        clear_act = QAction("清除最近文件", self)
        clear_act.triggered.connect(self._clear_recent)
        file_menu.addAction(clear_act)

        file_menu.addSeparator()

        exit_act = QAction("退出", self)
        exit_act.setShortcut(QKeySequence("Ctrl+Q"))
        exit_act.triggered.connect(self.close)
        file_menu.addAction(exit_act)

        # 视图
        view_menu = menubar.addMenu("视图")

        double_act = QAction("双页模式", self, checkable=True)
        double_act.setShortcut(QKeySequence("D"))
        double_act.triggered.connect(lambda: self.check_double.setChecked(not self.check_double.isChecked()))
        view_menu.addAction(double_act)

        long_act = QAction("长图模式", self, checkable=True)
        long_act.setShortcut(QKeySequence("L"))
        long_act.triggered.connect(lambda: self.check_long.setChecked(not self.check_long.isChecked()))
        view_menu.addAction(long_act)

        # 设置
        settings_menu = menubar.addMenu("设置")
        pref_act = QAction("偏好设置", self)
        pref_act.setShortcut(QKeySequence("Ctrl+,"))
        pref_act.triggered.connect(self._show_settings)
        settings_menu.addAction(pref_act)

    def _connect_signals(self):
        self.btn_open.clicked.connect(self.open_folder)
        self.btn_prev.clicked.connect(self.prev_page)
        self.btn_next.clicked.connect(self.next_page)
        self.check_double.stateChanged.connect(self._toggle_double)
        self.check_long.stateChanged.connect(self._toggle_long)

    # ── 状态持久化 ─────────────────────────────────────────────

    def _restore_state(self):
        geo = self.settings.value("geometry")
        if geo:
            self.restoreGeometry(geo)
        last = self.settings.value("last_folder", "")
        if last and os.path.isdir(last):
            self.load_folder(last)

    def _save_state(self):
        self.settings.setValue("geometry", self.saveGeometry())

    def closeEvent(self, event):
        self._save_state()
        super().closeEvent(event)

    # ── 最近文件 ───────────────────────────────────────────────

    def _update_recent_menu(self):
        self.recent_menu.clear()
        recent = self.settings.value("recent_files", [])
        if not recent:
            self.recent_menu.setEnabled(False)
            return
        self.recent_menu.setEnabled(True)
        for path in recent:
            act = QAction(path, self)
            act.triggered.connect(lambda checked, p=path: self._open_recent(p))
            self.recent_menu.addAction(act)

    def _open_recent(self, path):
        if os.path.exists(path):
            self.load_folder(path)
        else:
            recent = self.settings.value("recent_files", [])
            if path in recent:
                recent.remove(path)
                self.settings.setValue("recent_files", recent)
                self._update_recent_menu()
            QMessageBox.warning(self, "警告", "文件夹不存在或已被删除！")

    def _clear_recent(self):
        self.settings.setValue("recent_files", [])
        self._update_recent_menu()

    def _add_recent(self, path):
        recent = self.settings.value("recent_files", [])
        if path in recent:
            recent.remove(path)
        recent.insert(0, path)
        self.settings.setValue("recent_files", recent[:10])
        self._update_recent_menu()

    # ── 文件夹加载 ─────────────────────────────────────────────

    def open_folder(self):
        last = self.settings.value("last_folder", "")
        folder = QFileDialog.getExistingDirectory(self, "选择漫画文件夹", last)
        if not folder:
            return
        self.settings.setValue("last_folder", folder)
        self._add_recent(folder)
        self.load_folder(folder)

    def load_folder(self, folder):
        """加载文件夹中的图片"""
        self.preloaded_images.clear()

        self.image_files = []
        for ext in SUPPORTED_FORMATS:
            self.image_files.extend(QDir(folder).entryList([ext], QDir.Files, QDir.Name))

        if not self.image_files:
            self.label_status.setText("状态: 未找到图片文件！")
            self.btn_prev.setEnabled(False)
            self.btn_next.setEnabled(False)
            QMessageBox.information(self, "提示", f"在 {folder} 中未找到支持的图片文件。\n\n支持格式: {', '.join(f.replace('*.', '') for f in SUPPORTED_FORMATS)}")
            return

        self.image_files = [os.path.join(folder, f) for f in self.image_files]
        self.current_index = 0
        self.btn_prev.setEnabled(True)
        self.btn_next.setEnabled(True)
        self._show_progress(True)
        self._load_current()
        self._start_preload()
        self._show_progress(False)

    # ── 图片显示 ───────────────────────────────────────────────

    def _load_current(self):
        """加载并显示当前页"""
        if not self.image_files:
            return

        total = len(self.image_files)
        self.label_status.setText(
            f"状态: {self.current_index + 1}/{total} | {os.path.basename(self.image_files[self.current_index])}"
        )
        self.progress_bar.setValue(int((self.current_index + 1) / total * 100))

        if not self.double_page_mode:
            self._show_single()
        else:
            self._show_double()

    def _show_single(self):
        self.scene_left.clear()
        pixmap = self._get_pixmap(self.current_index)
        if pixmap:
            self.scene_left.addPixmap(pixmap)
            self.view_left.resetTransform()
            if not self.long_image_mode:
                self.view_left.fitInView(self.scene_left.itemsBoundingRect(), Qt.KeepAspectRatio)
            else:
                self.view_left.verticalScrollBar().setValue(0)

    def _show_double(self):
        self.scene_left.clear()
        self.scene_right.clear()

        px_left = self._get_pixmap(self.current_index)
        px_right = self._get_pixmap(self.current_index + 1) if self.current_index + 1 < len(self.image_files) else None

        for view, scene, pixmap in [
            (self.view_left, self.scene_left, px_left),
            (self.view_right, self.scene_right, px_right),
        ]:
            if pixmap:
                scene.addPixmap(pixmap)
                view.resetTransform()
                if not self.long_image_mode:
                    view.fitInView(scene.itemsBoundingRect(), Qt.KeepAspectRatio)
                else:
                    view.verticalScrollBar().setValue(0)

    def _get_pixmap(self, index):
        if index >= len(self.image_files):
            return None
        if index in self.preloaded_images:
            return self.preloaded_images[index]
        pixmap = QPixmap(self.image_files[index])
        if not pixmap.isNull():
            self.preloaded_images[index] = pixmap
            return pixmap
        return None

    def _show_progress(self, show):
        self.progress_bar.setVisible(show)

    # ── 预加载 ─────────────────────────────────────────────────

    def _start_preload(self):
        if self._preload_thread and self._preload_thread.isRunning():
            return

        self._preload_thread = PreloadThread(
            self.image_files, self.current_index, self.preloaded_images
        )
        self._preload_thread.loaded.connect(self._on_preload_loaded)
        self._preload_thread.start()

    def _on_preload_loaded(self, index, qimage):
        """预加载回调，将 QImage 转为 QPixmap（主线程安全）"""
        if not qimage.isNull():
            self.preloaded_images[index] = QPixmap.fromImage(qimage)

    # ── 模式切换 ───────────────────────────────────────────────

    def _toggle_double(self, state):
        self.double_page_mode = state == Qt.Checked
        if self.double_page_mode:
            self.view_right.show()
        else:
            self.view_right.hide()
        self._load_current()

    def _toggle_long(self, state):
        self.long_image_mode = state == Qt.Checked
        for view in (self.view_left, self.view_right):
            if self.long_image_mode:
                view.setVerticalScrollBarPolicy(Qt.ScrollBarAsNeeded)
            else:
                view.setVerticalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
        self._load_current()

    def _show_settings(self):
        dlg = SettingsDialog(self)
        if dlg.exec_() == QDialog.Accepted:
            self.scale_factor = dlg.scale_slider.value() / 100.0
            theme = dlg.theme_combo.currentText()
            if theme in THEMES:
                self.setStyleSheet(STYLE_SHEET + THEMES[theme])
            self._load_current()

    # ── 翻页 ───────────────────────────────────────────────────

    def prev_page(self):
        step = 2 if self.double_page_mode else 1
        self.current_index = max(0, self.current_index - step)
        self._load_current()
        self._start_preload()

    def next_page(self):
        step = 2 if self.double_page_mode else 1
        self.current_index = min(len(self.image_files) - 1, self.current_index + step)
        self._load_current()
        self._start_preload()

    # ── 键盘 & 鼠标 ───────────────────────────────────────────

    def keyPressEvent(self, event):
        key = event.key()
        if key == Qt.Key_Left or key == Qt.Key_A:
            self.prev_page()
        elif key == Qt.Key_Right:
            self.next_page()
        elif key == Qt.Key_Home:
            self.current_index = 0
            self._load_current()
        elif key == Qt.Key_End:
            self.current_index = len(self.image_files) - 1
            self._load_current()
        elif key == Qt.Key_Space:
            self.next_page()
        else:
            super().keyPressEvent(event)

    def eventFilter(self, obj, event):
        from PyQt5.QtCore import QEvent

        if event.type() == QEvent.MouseButtonPress:
            pos = event.pos()
            view = obj.parent()

            if event.button() == Qt.MiddleButton:
                view._drag_start = event.pos()
                view._orig_transform = view.transform()
                return True

            w = view.width()
            if pos.x() < w / 3:
                self.prev_page()
                return True
            elif pos.x() > w * 2 / 3:
                self.next_page()
                return True

        elif event.type() == QEvent.MouseMove:
            view = obj.parent()
            if hasattr(view, '_drag_start') and view._drag_start is not None:
                delta = event.pos() - view._drag_start
                view.setTransform(view._orig_transform)
                view.translate(delta.x(), delta.y())
                return True

        elif event.type() == QEvent.MouseButtonRelease:
            view = obj.parent()
            if event.button() == Qt.MiddleButton and hasattr(view, '_drag_start'):
                view.setTransform(view._orig_transform)
                view._drag_start = None
                return True

        elif event.type() == QEvent.MouseButtonDblClick:
            view = obj.parent()
            view.resetTransform()
            scene = self.scene_left if view == self.view_left else self.scene_right
            view.fitInView(scene.itemsBoundingRect(), Qt.KeepAspectRatio)
            return True

        elif event.type() == QEvent.Wheel:
            delta = event.angleDelta().y()
            view = obj.parent()

            if self.long_image_mode:
                step = delta / 120 * 30
                view.verticalScrollBar().setValue(
                    int(view.verticalScrollBar().value() - step)
                )
            else:
                factor = 1.15 if delta > 0 else 1 / 1.15
                current = view.transform().m11()
                new_scale = current * factor
                if 0.1 <= new_scale <= 10:
                    view.scale(factor, factor)
            return True

        return super().eventFilter(obj, event)

    def resizeEvent(self, event):
        super().resizeEvent(event)
        if self.image_files:
            self._load_current()


if __name__ == "__main__":
    app = QApplication(sys.argv)
    viewer = ComicViewer()
    viewer.show()
    sys.exit(app.exec_())
