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
    QProgressBar, QListWidget, QListWidgetItem, QLineEdit, QInputDialog,
    QSplitter, QGroupBox, QGridLayout, QScrollArea, QSpinBox
)
from PyQt5.QtGui import QPixmap, QImage, QIcon, QColor, QPainter, QKeySequence, QTransform
from PyQt5.QtCore import Qt, QDir, QTimer, QSize, QSettings, QThread, pyqtSignal
from manhuaviewer.data_store import ReadingHistory, TagManager


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


class HistoryDialog(QDialog):
    """阅读历史对话框"""

    folder_selected = pyqtSignal(str)

    def __init__(self, history: ReadingHistory, tag_manager: TagManager, parent=None):
        super().__init__(parent)
        self.history = history
        self.tag_manager = tag_manager
        self.setWindowTitle("阅读历史")
        self.setMinimumSize(600, 400)

        layout = QVBoxLayout(self)

        # 搜索框
        search_layout = QHBoxLayout()
        self.search_input = QLineEdit()
        self.search_input.setPlaceholderText("🔍 搜索文件夹名称或标签...")
        self.search_input.textChanged.connect(self._filter_list)
        search_layout.addWidget(self.search_input)
        layout.addLayout(search_layout)

        # 标签过滤
        tag_layout = QHBoxLayout()
        tag_layout.addWidget(QLabel("标签筛选:"))
        self.tag_combo = QComboBox()
        self.tag_combo.addItem("全部")
        self.tag_combo.addItems(tag_manager.get_all_tags())
        self.tag_combo.currentTextChanged.connect(self._filter_list)
        tag_layout.addWidget(self.tag_combo)
        tag_layout.addStretch()
        layout.addLayout(tag_layout)

        # 列表
        self.list_widget = QListWidget()
        self.list_widget.itemDoubleClicked.connect(self._on_double_click)
        layout.addWidget(self.list_widget)

        # 按钮
        btn_layout = QHBoxLayout()
        btn_open = QPushButton("打开")
        btn_open.clicked.connect(self._open_selected)
        btn_delete = QPushButton("删除记录")
        btn_delete.clicked.connect(self._delete_selected)
        btn_clear = QPushButton("清空历史")
        btn_clear.clicked.connect(self._clear_all)
        btn_layout.addWidget(btn_open)
        btn_layout.addWidget(btn_delete)
        btn_layout.addStretch()
        btn_layout.addWidget(btn_clear)
        layout.addLayout(btn_layout)

        self._load_list()

    def _load_list(self):
        self.list_widget.clear()
        entries = self.history.get_all_history()
        for entry in entries:
            folder = entry["folder"]
            page = entry["page_index"] + 1
            total = entry["total_pages"]
            tags = self.tag_manager.get_tags_for_folder(folder)
            tag_str = " ".join(f"[{t}]" for t in tags) if tags else ""
            name = os.path.basename(folder) or folder
            display = f"{name}  —  第 {page}/{total} 页  {tag_str}"
            item = QListWidgetItem(display)
            item.setData(Qt.UserRole, folder)
            self.list_widget.addItem(item)

    def _filter_list(self):
        keyword = self.search_input.text().lower()
        tag_filter = self.tag_combo.currentText()

        for i in range(self.list_widget.count()):
            item = self.list_widget.item(i)
            folder = item.data(Qt.UserRole)
            text = item.text().lower()

            match_keyword = not keyword or keyword in text
            match_tag = tag_filter == "全部" or tag_filter in self.tag_manager.get_tags_for_folder(folder)

            item.setHidden(not (match_keyword and match_tag))

    def _get_selected_folder(self):
        item = self.list_widget.currentItem()
        if item:
            return item.data(Qt.UserRole)
        return None

    def _on_double_click(self, item):
        folder = item.data(Qt.UserRole)
        if folder:
            self.folder_selected.emit(folder)
            self.accept()

    def _open_selected(self):
        folder = self._get_selected_folder()
        if folder:
            self.folder_selected.emit(folder)
            self.accept()

    def _delete_selected(self):
        folder = self._get_selected_folder()
        if folder:
            self.history.remove_entry(folder)
            self._load_list()

    def _clear_all(self):
        reply = QMessageBox.question(
            self, "确认", "确定要清空所有阅读历史吗？",
            QMessageBox.Yes | QMessageBox.No
        )
        if reply == QMessageBox.Yes:
            self.history.clear()
            self._load_list()


class TagDialog(QDialog):
    """标签管理对话框"""

    def __init__(self, folder: str, tag_manager: TagManager, parent=None):
        super().__init__(parent)
        self.folder = folder
        self.tag_manager = tag_manager
        self.setWindowTitle(f"标签管理 — {os.path.basename(folder)}")
        self.setMinimumWidth(350)

        layout = QVBoxLayout(self)

        # 当前标签
        layout.addWidget(QLabel("当前标签:"))
        self.tag_list = QListWidget()
        layout.addWidget(self.tag_list)

        # 添加标签
        add_layout = QHBoxLayout()
        self.new_tag_input = QLineEdit()
        self.new_tag_input.setPlaceholderText("输入新标签...")
        self.new_tag_input.returnPressed.connect(self._add_tag)
        btn_add = QPushButton("添加")
        btn_add.clicked.connect(self._add_tag)
        add_layout.addWidget(self.new_tag_input)
        add_layout.addWidget(btn_add)
        layout.addLayout(add_layout)

        # 全局标签快捷添加
        all_tags = tag_manager.get_all_tags()
        current_tags = set(tag_manager.get_tags_for_folder(folder))
        quick_tags = [t for t in all_tags if t not in current_tags]
        if quick_tags:
            layout.addWidget(QLabel("快速添加:"))
            quick_layout = QGridLayout()
            for i, tag in enumerate(quick_tags[:12]):
                btn = QPushButton(tag)
                btn.setStyleSheet(
                    f"background-color: {tag_manager.tag_colors.get(tag, '#888888')}; "
                    "color: white; border-radius: 3px; padding: 4px 10px;"
                )
                btn.clicked.connect(lambda checked, t=tag: self._quick_add(t))
                quick_layout.addWidget(btn, i // 4, i % 4)
            layout.addLayout(quick_layout)

        # 删除按钮
        btn_remove = QPushButton("移除选中标签")
        btn_remove.clicked.connect(self._remove_selected)
        layout.addWidget(btn_remove)

        # 关闭
        btn_close = QPushButton("关闭")
        btn_close.clicked.connect(self.accept)
        layout.addWidget(btn_close)

        self._refresh_list()

    def _refresh_list(self):
        self.tag_list.clear()
        tags = self.tag_manager.get_tags_for_folder(self.folder)
        for tag in tags:
            item = QListWidgetItem(tag)
            color = self.tag_manager.tag_colors.get(tag, "#888888")
            item.setForeground(QColor(color))
            self.tag_list.addItem(item)

    def _add_tag(self):
        tag = self.new_tag_input.text().strip()
        if tag:
            self.tag_manager.add_tag(self.folder, tag)
            self.new_tag_input.clear()
            self._refresh_list()

    def _quick_add(self, tag):
        self.tag_manager.add_tag(self.folder, tag)
        self._refresh_list()

    def _remove_selected(self):
        item = self.tag_list.currentItem()
        if item:
            self.tag_manager.remove_tag(self.folder, item.text())
            self._refresh_list()


class JumpPageDialog(QDialog):
    """跳转到指定页"""

    def __init__(self, current: int, total: int, parent=None):
        super().__init__(parent)
        self.setWindowTitle("跳转到")
        layout = QFormLayout(self)

        self.page_spin = QSlider(Qt.Horizontal)
        self.page_spin.setMinimum(1)
        self.page_spin.setMaximum(total)
        self.page_spin.setValue(current + 1)
        self.page_spin.setTickPosition(QSlider.TicksBelow)

        self.page_label = QLabel(f"第 {current + 1} / {total} 页")
        self.page_spin.valueChanged.connect(
            lambda v: self.page_label.setText(f"第 {v} / {total} 页")
        )

        layout.addRow(self.page_label)
        layout.addRow(self.page_spin)

        btn_box = QDialogButtonBox(QDialogButtonBox.Ok | QDialogButtonBox.Cancel)
        btn_box.accepted.connect(self.accept)
        btn_box.rejected.connect(self.reject)
        layout.addRow(btn_box)

    def get_page(self) -> int:
        return self.page_spin.value() - 1


class ThumbnailDialog(QDialog):
    """缩略图总览对话框"""

    page_selected = pyqtSignal(int)

    THUMB_SIZE = 150

    def __init__(self, image_files: list[str], current_index: int, parent=None):
        super().__init__(parent)
        self.image_files = image_files
        self.setWindowTitle(f"缩略图总览 ({len(image_files)} 页)")
        self.setMinimumSize(700, 500)

        layout = QVBoxLayout(self)

        # 滚动区域
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)

        container = QWidget()
        grid = QGridLayout(container)
        grid.setSpacing(8)

        cols = 4
        for i, filepath in enumerate(image_files):
            thumb_widget = QWidget()
            thumb_layout = QVBoxLayout(thumb_widget)
            thumb_layout.setContentsMargins(4, 4, 4, 4)
            thumb_layout.setSpacing(4)

            # 缩略图
            pixmap = QPixmap(filepath)
            if not pixmap.isNull():
                scaled = pixmap.scaled(
                    self.THUMB_SIZE, self.THUMB_SIZE,
                    Qt.KeepAspectRatio, Qt.SmoothTransformation
                )
                label = QLabel()
                label.setPixmap(scaled)
                label.setAlignment(Qt.AlignCenter)
                if i == current_index:
                    label.setStyleSheet("border: 3px solid #4a86e8; border-radius: 4px;")
                else:
                    label.setStyleSheet("border: 1px solid #dddddd; border-radius: 4px;")
                thumb_layout.addWidget(label)

            # 页码和文件名
            name_label = QLabel(f"{i + 1}. {os.path.basename(filepath)}")
            name_label.setAlignment(Qt.AlignCenter)
            name_label.setStyleSheet("font-size: 11px; color: #666;")
            name_label.setWordWrap(True)
            thumb_layout.addWidget(name_label)

            # 点击事件
            idx = i
            label.mousePressEvent = lambda e, x=idx: self._on_click(x)
            label.setCursor(Qt.PointingHandCursor)

            grid.addWidget(thumb_widget, i // cols, i % cols)

        scroll.setWidget(container)
        layout.addWidget(scroll)

        # 跳转按钮
        btn_close = QPushButton("关闭")
        btn_close.clicked.connect(self.accept)
        layout.addWidget(btn_close)

    def _on_click(self, index):
        self.page_selected.emit(index)
        self.accept()


class ComicViewer(QMainWindow):
    """漫画浏览器主窗口"""

    PRELOAD_COUNT = 6

    def __init__(self):
        super().__init__()
        self.setWindowTitle("漫画浏览器")
        self.setGeometry(100, 100, 1200, 800)

        # 状态
        self.settings = QSettings("ManhuaViewer", "ComicViewer")
        self.history = ReadingHistory()
        self.tag_manager = TagManager()
        self.image_files: list[str] = []
        self.current_index = 0
        self.current_folder: str = ""
        self.scale_factor = 1.0
        self.preloaded_images: dict[int, QPixmap] = {}
        self.double_page_mode = False
        self.long_image_mode = False
        self._is_fullscreen = False
        self._rotation = 0  # 0, 90, 180, 270
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

    def _show_single(self):
        self.scene_left.clear()
        pixmap = self._get_pixmap(self.current_index)
        if pixmap:
            pixmap = self._apply_rotation(pixmap)
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
                pixmap = self._apply_rotation(pixmap)
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

    # ── 响应式布局 ─────────────────────────────────────────────

    def resizeEvent(self, event):
        super().resizeEvent(event)
        if self.image_files:
            self._load_current()


def main_cli():
    """命令行入口（供 pyproject.toml [project.scripts] 使用）"""
    app = QApplication(sys.argv)
    viewer = ComicViewer()
    viewer.show()
    sys.exit(app.exec_())


if __name__ == "__main__":
    main_cli()
