"""
漫画浏览器 - 主窗口
支持单页/双页/长图模式、预加载、主题切换、快捷键操作
"""
import logging
import os
import sys

from PyQt5.QtWidgets import (
    QApplication, QMainWindow, QGraphicsView, QGraphicsScene, QFileDialog,
    QVBoxLayout, QWidget, QHBoxLayout, QPushButton, QLabel, QCheckBox,
    QFrame, QSpacerItem, QSizePolicy, QMenu, QAction, QDialog,
    QMessageBox, QProgressBar,
)
from PyQt5.QtGui import QPixmap, QIcon, QPainter, QKeySequence, QTransform, QDragEnterEvent, QDropEvent
from PyQt5.QtCore import Qt, QDir, QTimer, QSettings, QThread, pyqtSignal

from manhuaviewer.constants import SUPPORTED_FORMATS, MAX_RECENT_FILES
from manhuaviewer.styles import STYLE_SHEET, THEMES
from manhuaviewer.data_store import ReadingHistory, TagManager
from manhuaviewer.preload import PreloadThread, LRUCache
from manhuaviewer.dialogs import (
    SettingsDialog, HistoryDialog, TagDialog, ThumbnailDialog, JumpPageDialog,
)

logger = logging.getLogger(__name__)

APP_VERSION = "0.4.0"


class ComicViewer(QMainWindow):
    """漫画浏览器主窗口"""

    def __init__(self):
        super().__init__()
        self.setWindowTitle("漫画浏览器")
        self.setAcceptDrops(True)

        # 状态
        self.settings = QSettings("ManhuaViewer", "ComicViewer")
        self.history = ReadingHistory()
        self.tag_manager = TagManager()
        self.image_files: list[str] = []
        self.current_index = 0
        self.current_folder: str = ""
        self.scale_factor = 1.0
        self.preloaded_images = LRUCache()
        self.double_page_mode = False
        self.long_image_mode = False
        self._is_fullscreen = False
        self._rotation = 0  # 0, 90, 180, 270
        self._current_theme = "浅色"
        self._bg_color = "#ffffff"
        self._preload_thread: PreloadThread | None = None

        self._init_ui()
        self._connect_signals()
        self._restore_state()

    # ── UI 初始化 ──────────────────────────────────────────────

    def _init_ui(self):
        self.setStyleSheet(STYLE_SHEET)
        self.setGeometry(100, 100, 1200, 800)
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
        view_layout.addWidget(self.view_right)
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

        file_menu.addSeparator()

        history_act = QAction("阅读历史", self)
        history_act.setShortcut(QKeySequence("Ctrl+H"))
        history_act.triggered.connect(self._show_history)
        file_menu.addAction(history_act)

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

        view_menu.addSeparator()

        fullscreen_act = QAction("全屏模式", self, checkable=True)
        fullscreen_act.setShortcut(QKeySequence("F11"))
        fullscreen_act.triggered.connect(self._toggle_fullscreen)
        view_menu.addAction(fullscreen_act)

        view_menu.addSeparator()

        thumb_act = QAction("缩略图总览", self)
        thumb_act.setShortcut(QKeySequence("T"))
        thumb_act.triggered.connect(self._show_thumbnails)
        view_menu.addAction(thumb_act)

        rotate_cw = QAction("顺时针旋转 90°", self)
        rotate_cw.setShortcut(QKeySequence("R"))
        rotate_cw.triggered.connect(lambda: self._rotate(90))
        view_menu.addAction(rotate_cw)

        rotate_ccw = QAction("逆时针旋转 90°", self)
        rotate_ccw.setShortcut(QKeySequence("Shift+R"))
        rotate_ccw.triggered.connect(lambda: self._rotate(-90))
        view_menu.addAction(rotate_ccw)

        # 漫画
        comic_menu = menubar.addMenu("漫画")

        tag_act = QAction("管理标签", self)
        tag_act.setShortcut(QKeySequence("M"))
        tag_act.triggered.connect(self._show_tag_dialog)
        comic_menu.addAction(tag_act)

        jump_act = QAction("跳转到页", self)
        jump_act.setShortcut(QKeySequence("G"))
        jump_act.triggered.connect(self._jump_to_page)
        comic_menu.addAction(jump_act)

        # 设置
        settings_menu = menubar.addMenu("设置")
        pref_act = QAction("偏好设置", self)
        pref_act.setShortcut(QKeySequence("Ctrl+,"))
        pref_act.triggered.connect(self._show_settings)
        settings_menu.addAction(pref_act)

        # 帮助
        help_menu = menubar.addMenu("帮助")
        shortcuts_act = QAction("快捷键列表", self)
        shortcuts_act.setShortcut(QKeySequence("F1"))
        shortcuts_act.triggered.connect(self._show_shortcuts)
        help_menu.addAction(shortcuts_act)

        about_act = QAction("关于", self)
        about_act.triggered.connect(self._show_about)
        help_menu.addAction(about_act)

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
        # 恢复设置
        self.scale_factor = float(self.settings.value("scale_factor", 1.0))
        self._current_theme = self.settings.value("theme", "浅色")
        self._bg_color = self.settings.value("bg_color", "#ffffff")
        self._apply_theme(self._current_theme)
        self._apply_bg_color(self._bg_color)
        # 加载上次文件夹
        last = self.settings.value("last_folder", "")
        if last and os.path.isdir(last):
            self.load_folder(last)

    def _save_state(self):
        self.settings.setValue("geometry", self.saveGeometry())
        self.settings.setValue("scale_factor", self.scale_factor)
        self.settings.setValue("theme", self._current_theme)
        self.settings.setValue("bg_color", self._bg_color)

    def closeEvent(self, event):
        self._save_state()
        super().closeEvent(event)

    # ── 拖拽支持 ───────────────────────────────────────────────

    def dragEnterEvent(self, event: QDragEnterEvent):
        if event.mimeData().hasUrls():
            for url in event.mimeData().urls():
                if url.isLocalFile() and os.path.isdir(url.toLocalFile()):
                    event.acceptProposedAction()
                    return
        event.ignore()

    def dropEvent(self, event: QDropEvent):
        for url in event.mimeData().urls():
            if url.isLocalFile():
                folder = url.toLocalFile()
                if os.path.isdir(folder):
                    self.settings.setValue("last_folder", folder)
                    self._add_recent(folder)
                    self.load_folder(folder)
                    break

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
        self.settings.setValue("recent_files", recent[:MAX_RECENT_FILES])
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
            QMessageBox.information(
                self, "提示",
                f"在 {folder} 中未找到支持的图片文件。\n\n"
                f"支持格式: {', '.join(f.replace('*.', '') for f in SUPPORTED_FORMATS)}"
            )
            return

        self.image_files = [os.path.join(folder, f) for f in self.image_files]
        self.current_folder = folder
        self.current_index = 0

        # 恢复阅读进度
        progress = self.history.get_progress(folder)
        if progress:
            saved_page = progress.get("page_index", 0)
            if saved_page < len(self.image_files):
                self.current_index = saved_page
                logger.info(f"恢复阅读进度: {folder} -> 第 {saved_page + 1} 页")

        self.btn_prev.setEnabled(True)
        self.btn_next.setEnabled(True)
        self.progress_bar.show()
        self._load_current()
        self._start_preload()
        self.progress_bar.hide()

        logger.info(f"加载文件夹: {folder} ({len(self.image_files)} 页)")

    # ── 图片显示 ───────────────────────────────────────────────

    def _load_current(self):
        """加载并显示当前页"""
        if not self.image_files:
            return

        total = len(self.image_files)
        tags = self.tag_manager.get_tags_for_folder(self.current_folder) if self.current_folder else []
        tag_str = " ".join(f"[{t}]" for t in tags) if tags else ""
        rotation_str = f" | 旋转 {self._rotation}°" if self._rotation else ""
        self.label_status.setText(
            f"状态: {self.current_index + 1}/{total} | "
            f"{os.path.basename(self.image_files[self.current_index])}"
            f"{rotation_str} {tag_str}"
        )
        self.progress_bar.setValue(int((self.current_index + 1) / total * 100))

        # 保存阅读进度
        if self.current_folder:
            self.history.save_progress(self.current_folder, self.current_index, total)

        if not self.double_page_mode:
            self._show_single()
        else:
            self._show_double()

    def _apply_rotation(self, pixmap: QPixmap) -> QPixmap:
        """应用旋转"""
        if self._rotation == 0 or pixmap.isNull():
            return pixmap
        transform = QTransform().rotate(self._rotation)
        return pixmap.transformed(transform, Qt.SmoothTransformation)

    def _apply_scale(self, view: QGraphicsView, scene: QGraphicsScene):
        """根据 scale_factor 缩放视图"""
        view.resetTransform()
        view.fitInView(scene.itemsBoundingRect(), Qt.KeepAspectRatio)
        if self.scale_factor != 1.0:
            view.scale(self.scale_factor, self.scale_factor)

    def _show_single(self):
        self.scene_left.clear()
        pixmap = self._get_pixmap(self.current_index)
        if pixmap:
            pixmap = self._apply_rotation(pixmap)
            self.scene_left.addPixmap(pixmap)
            if not self.long_image_mode:
                self._apply_scale(self.view_left, self.scene_left)
            else:
                self.view_left.resetTransform()
                self.view_left.fitInView(self.scene_left.itemsBoundingRect(), Qt.KeepAspectRatio)
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
                pixmap = self._apply_rotation(pixmap)
                scene.addPixmap(pixmap)
                if not self.long_image_mode:
                    self._apply_scale(view, scene)
                else:
                    view.resetTransform()
                    view.fitInView(scene.itemsBoundingRect(), Qt.KeepAspectRatio)
                    view.verticalScrollBar().setValue(0)

    def _get_pixmap(self, index):
        """获取指定索引的图片，带异常处理"""
        if index >= len(self.image_files):
            return None
        cached = self.preloaded_images.get(index)
        if cached:
            return cached
        try:
            pixmap = QPixmap(self.image_files[index])
            if not pixmap.isNull():
                self.preloaded_images.put(index, pixmap)
                return pixmap
        except Exception as e:
            logger.warning(f"加载图片失败 [{index}]: {self.image_files[index]}: {e}")
        return None

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
            self.preloaded_images.put(index, QPixmap.fromImage(qimage))

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

    # ── 设置 ───────────────────────────────────────────────────

    def _show_settings(self):
        dlg = SettingsDialog(
            self,
            current_scale=self.scale_factor,
            current_theme=self._current_theme,
            bg_color=self._bg_color,
        )
        if dlg.exec_() == QDialog.Accepted:
            self.scale_factor = dlg.scale_slider.value() / 100.0
            self._current_theme = dlg.theme_combo.currentText()
            self._bg_color = dlg.get_bg_color()
            self._apply_theme(self._current_theme)
            self._apply_bg_color(self._bg_color)
            self._load_current()
            logger.info(f"设置更新: 缩放={self.scale_factor}, 主题={self._current_theme}, 背景={self._bg_color}")

    def _apply_theme(self, theme_name: str):
        if theme_name in THEMES:
            self.setStyleSheet(STYLE_SHEET + THEMES[theme_name])

    def _apply_bg_color(self, color: str):
        """应用背景颜色到图片显示区"""
        for view in (self.view_left, self.view_right):
            view.setStyleSheet(f"QGraphicsView {{ background-color: {color}; }}")

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
            # obj 是 viewport，parent() 才是 QGraphicsView
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

    # ── 全屏 ───────────────────────────────────────────────────

    def _toggle_fullscreen(self):
        if self._is_fullscreen:
            self.showNormal()
            self._is_fullscreen = False
        else:
            self.showFullScreen()
            self._is_fullscreen = True

    # ── 缩略图 ─────────────────────────────────────────────────

    def _show_thumbnails(self):
        if not self.image_files:
            return
        dlg = ThumbnailDialog(self.image_files, self.current_index, self)
        dlg.page_selected.connect(self._on_thumb_select)
        dlg.exec_()

    def _on_thumb_select(self, index):
        self.current_index = index
        self._load_current()
        self._start_preload()

    # ── 旋转 ───────────────────────────────────────────────────

    def _rotate(self, degrees: int):
        self._rotation = (self._rotation + degrees) % 360
        self._load_current()

    # ── 跳转到页 ───────────────────────────────────────────────

    def _jump_to_page(self):
        if not self.image_files:
            return
        dlg = JumpPageDialog(self.current_index, len(self.image_files), self)
        if dlg.exec_() == QDialog.Accepted:
            self.current_index = dlg.get_page()
            self._load_current()
            self._start_preload()

    # ── 阅读历史 ───────────────────────────────────────────────

    def _show_history(self):
        dlg = HistoryDialog(self.history, self.tag_manager, self)
        dlg.folder_selected.connect(self._on_history_select)
        dlg.exec_()

    def _on_history_select(self, folder):
        if os.path.isdir(folder):
            self.settings.setValue("last_folder", folder)
            self._add_recent(folder)
            self.load_folder(folder)

    # ── 标签管理 ───────────────────────────────────────────────

    def _show_tag_dialog(self):
        if not self.current_folder:
            QMessageBox.information(self, "提示", "请先打开一个漫画文件夹")
            return
        dlg = TagDialog(self.current_folder, self.tag_manager, self)
        dlg.exec_()

    # ── 帮助 ───────────────────────────────────────────────────

    def _show_shortcuts(self):
        shortcuts_text = """
        <h3>快捷键列表</h3>
        <table cellpadding="6" cellspacing="0" border="1" style="border-collapse:collapse;">
        <tr><th>按键</th><th>功能</th></tr>
        <tr><td>← / A</td><td>上一页</td></tr>
        <tr><td>→ / Space</td><td>下一页</td></tr>
        <tr><td>D</td><td>切换双页模式</td></tr>
        <tr><td>L</td><td>切换长图模式</td></tr>
        <tr><td>R</td><td>顺时针旋转 90°</td></tr>
        <tr><td>Shift+R</td><td>逆时针旋转 90°</td></tr>
        <tr><td>T</td><td>缩略图总览</td></tr>
        <tr><td>G</td><td>跳转到指定页</td></tr>
        <tr><td>M</td><td>管理标签</td></tr>
        <tr><td>F11</td><td>全屏模式</td></tr>
        <tr><td>Home</td><td>跳到第一页</td></tr>
        <tr><td>End</td><td>跳到最后一页</td></tr>
        <tr><td>Ctrl+O</td><td>打开文件夹</td></tr>
        <tr><td>Ctrl+H</td><td>阅读历史</td></tr>
        <tr><td>Ctrl+,</td><td>偏好设置</td></tr>
        <tr><td>Ctrl+Q</td><td>退出</td></tr>
        <tr><td>滚轮</td><td>缩放 / 长图滚动</td></tr>
        <tr><td>双击</td><td>重置缩放</td></tr>
        <tr><td>中键拖拽</td><td>平移图片</td></tr>
        <tr><td>点击左1/3</td><td>上一页</td></tr>
        <tr><td>点击右1/3</td><td>下一页</td></tr>
        </table>
        """
        QMessageBox.about(self, "快捷键", shortcuts_text)

    def _show_about(self):
        QMessageBox.about(
            self, "关于",
            f"<h2>漫画浏览器</h2>"
            f"<p>版本 {APP_VERSION}</p>"
            f"<p>基于 PyQt5 的桌面漫画/图片浏览器</p>"
            f"<p>支持单页、双页、长图模式</p>"
            f"<p><a href='https://github.com/sece1024/manhuaviewer'>GitHub</a></p>"
        )

    # ── 响应式布局 ─────────────────────────────────────────────

    def resizeEvent(self, event):
        super().resizeEvent(event)
        if self.image_files:
            self._load_current()


def main_cli():
    """命令行入口（供 pyproject.toml [project.scripts] 使用）"""
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    )
    app = QApplication(sys.argv)
    viewer = ComicViewer()
    viewer.show()
    sys.exit(app.exec_())


if __name__ == "__main__":
    main_cli()
