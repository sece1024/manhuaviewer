import os
import sys
from PyQt5.QtWidgets import (
    QApplication, QMainWindow, QGraphicsView, QGraphicsScene, QFileDialog,
    QVBoxLayout, QWidget, QHBoxLayout, QPushButton, QLabel, QCheckBox,
    QFrame, QSpacerItem, QSizePolicy, QMenu, QAction, QDialog,
    QSlider, QColorDialog, QComboBox, QFormLayout, QDialogButtonBox, QMessageBox
)
from PyQt5.QtGui import QPixmap, QImage, QIcon, QColor, QPainter
from PyQt5.QtCore import Qt, QDir, QTimer, QSize, QSettings
from threading import Thread
import time

class SettingsDialog(QDialog):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setWindowTitle("设置")
        self.setMinimumWidth(300)
        
        layout = QFormLayout(self)
        
        # 缩放设置
        self.scale_slider = QSlider(Qt.Horizontal)
        self.scale_slider.setMinimum(50)
        self.scale_slider.setMaximum(200)
        self.scale_slider.setValue(100)
        self.scale_slider.setTickPosition(QSlider.TicksBelow)
        self.scale_slider.setTickInterval(10)
        layout.addRow("图片缩放:", self.scale_slider)
        
        # 背景颜色选择
        self.bg_color_btn = QPushButton("选择颜色")
        self.bg_color_btn.clicked.connect(self.choose_bg_color)
        layout.addRow("背景颜色:", self.bg_color_btn)
        
        # 主题选择
        self.theme_combo = QComboBox()
        self.theme_combo.addItems(["浅色", "深色", "护眼"])
        layout.addRow("主题:", self.theme_combo)
        
        # 按钮
        self.button_box = QDialogButtonBox(
            QDialogButtonBox.Ok | QDialogButtonBox.Cancel
        )
        self.button_box.accepted.connect(self.accept)
        self.button_box.rejected.connect(self.reject)
        layout.addRow(self.button_box)
    
    def choose_bg_color(self):
        color = QColorDialog.getColor()
        if color.isValid():
            self.bg_color_btn.setStyleSheet(
                f"background-color: {color.name()};"
                f"border: 1px solid #cccccc;"
                f"border-radius: 4px;"
            )

class ComicViewer(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Python 漫画浏览器 (增强版)")
        self.setGeometry(100, 100, 1200, 800)
        
        # 初始化设置
        self.settings = QSettings("ManhuaViewer", "ComicViewer")
        
        # 添加设置菜单
        self.create_menu_bar()
        
        # 设置窗口样式
        self.setStyleSheet("""
            QMainWindow {
                background-color: #f0f0f0;
            }
            QPushButton {
                background-color: #4a86e8;
                color: white;
                border: none;
                padding: 8px 16px;
                border-radius: 4px;
                font-size: 14px;
            }
            QPushButton:hover {
                background-color: #3a76d8;
            }
            QPushButton:pressed {
                background-color: #2a66c8;
            }
            QLabel {
                color: #333333;
                font-size: 14px;
            }
            QCheckBox {
                color: #333333;
                font-size: 14px;
                spacing: 8px;
            }
            QCheckBox::indicator {
                width: 18px;
                height: 18px;
            }
            QGraphicsView {
                background-color: #ffffff;
                border: 1px solid #dddddd;
                border-radius: 4px;
            }
            QMenuBar {
                background-color: #ffffff;
                border-bottom: 1px solid #dddddd;
            }
            QMenuBar::item {
                padding: 4px 8px;
                background-color: transparent;
            }
            QMenuBar::item:selected {
                background-color: #e0e0e0;
            }
            QMenu {
                background-color: #ffffff;
                border: 1px solid #dddddd;
            }
            QMenu::item {
                padding: 6px 20px;
            }
            QMenu::item:selected {
                background-color: #e0e0e0;
            }
        """)

        # 变量初始化
        self.image_files = []
        self.current_index = 0
        self.scale_factor = 1.0
        self.preload_thread = None
        self.preloaded_images = {}
        self.double_page_mode = False
        self.long_image_mode = False  # 长图模式开关

        # 主界面布局
        self.central_widget = QWidget()
        self.setCentralWidget(self.central_widget)
        self.layout = QVBoxLayout(self.central_widget)
        self.layout.setContentsMargins(10, 10, 10, 10)
        self.layout.setSpacing(10)

        # 工具栏
        self.toolbar = QHBoxLayout()
        self.toolbar.setSpacing(10)
        
        # 创建工具栏容器
        toolbar_container = QFrame()
        toolbar_container.setStyleSheet("""
            QFrame {
                background-color: #ffffff;
                border-radius: 8px;
                padding: 10px;
                margin-bottom: 10px;
            }
        """)
        toolbar_container.setLayout(self.toolbar)

        self.btn_open = QPushButton("打开文件夹")
        self.btn_prev = QPushButton("上一页 (←)")
        self.btn_next = QPushButton("下一页 (→)")
        self.check_double = QCheckBox("双页模式")
        self.check_long = QCheckBox("长图模式")  # 添加长图模式复选框
        self.label_status = QLabel("状态: 未加载图片")
        
        # 添加弹性空间
        spacer = QSpacerItem(40, 20, QSizePolicy.Expanding, QSizePolicy.Minimum)
        
        self.toolbar.addWidget(self.btn_open)
        self.toolbar.addWidget(self.btn_prev)
        self.toolbar.addWidget(self.btn_next)
        self.toolbar.addWidget(self.check_double)
        self.toolbar.addWidget(self.check_long)  # 添加长图模式复选框到工具栏
        self.toolbar.addItem(spacer)
        self.toolbar.addWidget(self.label_status)
        
        self.layout.addWidget(toolbar_container)

        # 图片显示区域
        self.view_container = QWidget()
        self.view_layout = QHBoxLayout(self.view_container)
        self.view_layout.setSpacing(10)
        self.view_layout.setContentsMargins(10, 10, 10, 10)
        
        self.view_left = QGraphicsView()
        self.view_right = QGraphicsView()
        self.scene_left = QGraphicsScene()
        self.scene_right = QGraphicsScene()
        
        # 设置视图属性
        for view in [self.view_left, self.view_right]:
            view.setRenderHint(QPainter.Antialiasing)
            view.setRenderHint(QPainter.SmoothPixmapTransform)
            view.setViewportUpdateMode(QGraphicsView.FullViewportUpdate)
            view.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
            view.setVerticalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
            # 启用鼠标事件
            view.setMouseTracking(True)
            view.viewport().setMouseTracking(True)
            # 安装事件过滤器
            view.viewport().installEventFilter(self)
            # 设置缩放属性
            view.setTransformationAnchor(QGraphicsView.AnchorUnderMouse)
            view.setResizeAnchor(QGraphicsView.AnchorUnderMouse)
            # 初始化拖动相关变量
            view.drag_start_pos = None
            view.original_transform = None
        
        self.view_left.setScene(self.scene_left)
        self.view_right.setScene(self.scene_right)
        
        # 默认单页模式
        self.view_layout.addWidget(self.view_left)
        self.view_right.hide()
        
        # 创建视图容器
        view_container = QFrame()
        view_container.setStyleSheet("""
            QFrame {
                background-color: #ffffff;
                border-radius: 8px;
                margin-top: 10px;
            }
        """)
        view_container.setLayout(self.view_layout)
        
        self.layout.addWidget(view_container)
        self.layout.setStretch(1, 1)  # 让视图区域占据更多空间

        # 连接信号槽
        self.btn_open.clicked.connect(self.open_folder)
        self.btn_prev.clicked.connect(self.prev_page)
        self.btn_next.clicked.connect(self.next_page)
        self.check_double.stateChanged.connect(self.toggle_double_page)
        self.check_long.stateChanged.connect(self.toggle_long_image_mode)  # 连接长图模式信号

    def create_menu_bar(self):
        """创建菜单栏"""
        menubar = self.menuBar()
        
        # 文件菜单
        file_menu = menubar.addMenu("文件")
        
        # 打开文件夹动作
        open_action = QAction("打开文件夹", self)
        open_action.triggered.connect(self.open_folder)
        file_menu.addAction(open_action)
        
        # 最近打开的文件
        self.recent_menu = QMenu("最近打开", self)
        file_menu.addMenu(self.recent_menu)
        self.update_recent_files_menu()
        
        # 添加分隔线
        file_menu.addSeparator()
        
        # 清除最近文件
        clear_recent_action = QAction("清除最近文件", self)
        clear_recent_action.triggered.connect(self.clear_recent_files)
        file_menu.addAction(clear_recent_action)
        
        # 视图菜单
        view_menu = menubar.addMenu("视图")
        double_page_action = QAction("双页模式", self, checkable=True)
        double_page_action.triggered.connect(
            lambda: self.check_double.setChecked(not self.check_double.isChecked())
        )
        view_menu.addAction(double_page_action)
        
        # 设置菜单
        settings_menu = menubar.addMenu("设置")
        preferences_action = QAction("偏好设置", self)
        preferences_action.triggered.connect(self.show_settings)
        settings_menu.addAction(preferences_action)
    
    def show_settings(self):
        """显示设置对话框"""
        dialog = SettingsDialog(self)
        if dialog.exec_() == QDialog.Accepted:
            # 应用设置
            scale_factor = dialog.scale_slider.value() / 100.0
            self.apply_scale(scale_factor)
            
            # 应用主题
            theme = dialog.theme_combo.currentText()
            self.apply_theme(theme)
    
    def apply_scale(self, factor):
        """应用缩放设置"""
        self.scale_factor = factor
        self.load_image()  # 重新加载当前图片以应用缩放
    
    def apply_theme(self, theme):
        """应用主题设置"""
        if theme == "浅色":
            self.setStyleSheet("""
                QMainWindow { background-color: #f0f0f0; }
                QGraphicsView { background-color: #ffffff; }
            """)
        elif theme == "深色":
            self.setStyleSheet("""
                QMainWindow { background-color: #2d2d2d; }
                QGraphicsView { background-color: #1a1a1a; }
                QLabel { color: #ffffff; }
                QCheckBox { color: #ffffff; }
            """)
        elif theme == "护眼":
            self.setStyleSheet("""
                QMainWindow { background-color: #f0f7eb; }
                QGraphicsView { background-color: #ffffff; }
            """)

    def update_recent_files_menu(self):
        """更新最近打开文件菜单"""
        self.recent_menu.clear()
        recent_files = self.settings.value("recent_files", [])
        
        if not recent_files:
            self.recent_menu.setEnabled(False)
            return
            
        self.recent_menu.setEnabled(True)
        for file_path in recent_files:
            # 显示完整路径
            action = QAction(file_path, self)
            action.setData(file_path)
            action.triggered.connect(lambda checked, path=file_path: self.open_recent_file(path))
            self.recent_menu.addAction(action)

    def open_recent_file(self, file_path):
        """打开最近的文件"""
        if os.path.exists(file_path):
            self.load_folder(file_path)
        else:
            # 如果文件不存在，从最近文件列表中移除
            recent_files = self.settings.value("recent_files", [])
            if file_path in recent_files:
                recent_files.remove(file_path)
                self.settings.setValue("recent_files", recent_files)
                self.update_recent_files_menu()
            QMessageBox.warning(self, "警告", "文件不存在或已被删除！")

    def clear_recent_files(self):
        """清除最近打开文件列表"""
        self.settings.setValue("recent_files", [])
        self.update_recent_files_menu()

    def add_to_recent_files(self, file_path):
        """添加文件到最近打开列表"""
        recent_files = self.settings.value("recent_files", [])
        
        # 如果文件已经在列表中，先移除
        if file_path in recent_files:
            recent_files.remove(file_path)
        
        # 将文件添加到列表开头
        recent_files.insert(0, file_path)
        
        # 限制最近文件数量为10个
        recent_files = recent_files[:10]
        
        # 保存设置
        self.settings.setValue("recent_files", recent_files)
        self.update_recent_files_menu()

    def open_folder(self):
        """选择文件夹并加载图片"""
        # 获取上次打开的文件夹路径
        last_path = self.settings.value("last_folder", "")
        
        folder = QFileDialog.getExistingDirectory(
            self, 
            "选择漫画文件夹",
            last_path
        )
        
        if not folder:
            return
            
        # 保存当前文件夹路径
        self.settings.setValue("last_folder", folder)
        self.add_to_recent_files(folder)
        
        self.load_folder(folder)

    def load_folder(self, folder):
        """加载指定文件夹的图片"""
        # 清空缓存
        self.preloaded_images.clear()

        # 获取文件夹内所有图片文件（按文件名排序）
        self.image_files = []
        for ext in ["*.jpg", "*.jpeg", "*.png", "*.bmp", "*.webp"]:
            self.image_files.extend(QDir(folder).entryList([ext], QDir.Files, QDir.Name))
        
        if not self.image_files:
            self.label_status.setText("状态: 未找到图片文件！")
            return

        self.image_files = [os.path.join(folder, f) for f in self.image_files]
        self.current_index = 0
        self.load_image()

        # 启动预加载线程
        self.start_preload_thread()

    def load_image(self):
        """加载当前图片（单页/双页）"""
        if not self.image_files:
            return

        # 更新状态栏
        self.label_status.setText(
            f"状态: {self.current_index + 1}/{len(self.image_files)} | "
            f"文件: {os.path.basename(self.image_files[self.current_index])}"
        )

        # 单页模式
        if not self.double_page_mode:
            self.scene_left.clear()
            pixmap = self.get_pixmap(self.current_index)
            if pixmap:
                self.scene_left.addPixmap(pixmap)
                # 重置缩放并适应视图
                self.view_left.resetTransform()
                if not self.long_image_mode:
                    self.view_left.fitInView(self.scene_left.itemsBoundingRect(), Qt.KeepAspectRatio)
                else:
                    # 长图模式下滚动到顶部
                    self.view_left.verticalScrollBar().setValue(0)
        # 双页模式
        else:
            self.scene_left.clear()
            self.scene_right.clear()
            pixmap_left = self.get_pixmap(self.current_index)
            pixmap_right = self.get_pixmap(self.current_index + 1) if self.current_index + 1 < len(self.image_files) else None

            if pixmap_left:
                self.scene_left.addPixmap(pixmap_left)
                # 重置缩放并适应视图
                self.view_left.resetTransform()
                if not self.long_image_mode:
                    self.view_left.fitInView(self.scene_left.itemsBoundingRect(), Qt.KeepAspectRatio)
                else:
                    # 长图模式下滚动到顶部
                    self.view_left.verticalScrollBar().setValue(0)
            if pixmap_right:
                self.scene_right.addPixmap(pixmap_right)
                # 重置缩放并适应视图
                self.view_right.resetTransform()
                if not self.long_image_mode:
                    self.view_right.fitInView(self.scene_right.itemsBoundingRect(), Qt.KeepAspectRatio)
                else:
                    # 长图模式下滚动到顶部
                    self.view_right.verticalScrollBar().setValue(0)

    def get_pixmap(self, index):
        """从缓存或文件获取图片，避免重复加载"""
        if index >= len(self.image_files):
            return None

        # 如果已预加载，直接返回缓存
        if index in self.preloaded_images:
            return self.preloaded_images[index]
        
        # 否则加载图片并加入缓存
        pixmap = QPixmap(self.image_files[index])
        if not pixmap.isNull():
            self.preloaded_images[index] = pixmap
            return pixmap
        return None

    def start_preload_thread(self):
        """启动预加载线程（后台加载后续图片）"""
        if self.preload_thread and self.preload_thread.is_alive():
            return

        def preload_task():
            # 预加载当前页前后各3张图片
            preload_range = range(
                max(0, self.current_index - 3),
                min(len(self.image_files), self.current_index + 6)
            )
            for i in preload_range:
                if i not in self.preloaded_images:  # 避免重复加载
                    self.preloaded_images[i] = QPixmap(self.image_files[i])

        self.preload_thread = Thread(target=preload_task, daemon=True)
        self.preload_thread.start()

    def toggle_double_page(self, state):
        """切换单页/双页模式"""
        self.double_page_mode = state == Qt.Checked
        if self.double_page_mode:
            self.view_right.show()
            self.view_layout.addWidget(self.view_right)
        else:
            self.view_right.hide()
        self.load_image()  # 重新加载当前页

    def toggle_long_image_mode(self, state):
        """切换长图模式"""
        self.long_image_mode = state == Qt.Checked
        
        # 根据模式设置滚动条策略
        for view in [self.view_left, self.view_right]:
            if self.long_image_mode:
                view.setVerticalScrollBarPolicy(Qt.ScrollBarAsNeeded)
                view.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
            else:
                view.setVerticalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
                view.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
        
        self.load_image()  # 重新加载当前图片

    def prev_page(self):
        """上一页（双页模式一次翻两页）"""
        step = 2 if self.double_page_mode else 1
        self.current_index = max(0, self.current_index - step)
        self.load_image()
        self.start_preload_thread()  # 翻页后重新预加载

    def next_page(self):
        """下一页（双页模式一次翻两页）"""
        step = 2 if self.double_page_mode else 1
        self.current_index = min(len(self.image_files) - 1, self.current_index + step)
        self.load_image()
        self.start_preload_thread()  # 翻页后重新预加载

    def keyPressEvent(self, event):
        """键盘快捷键支持"""
        if event.key() == Qt.Key_Left:
            self.prev_page()
        elif event.key() == Qt.Key_Right:
            self.next_page()
        elif event.key() == Qt.Key_D:
            self.check_double.setChecked(not self.check_double.isChecked())
        elif event.key() == Qt.Key_L:  # 添加长图模式快捷键
            self.check_long.setChecked(not self.check_long.isChecked())

    def eventFilter(self, obj, event):
        """处理鼠标事件"""
        if event.type() == event.MouseButtonPress:
            # 获取点击位置
            pos = event.pos()
            view = obj.parent()
            
            # 如果是中键按下，开始拖动
            if event.button() == Qt.MiddleButton:
                view.drag_start_pos = event.pos()
                view.original_transform = view.transform()
                return True
            
            # 获取视图的宽度
            width = view.width()
            
            # 判断点击位置
            if pos.x() < width / 3:  # 左侧1/3区域
                self.prev_page()
                return True
            elif pos.x() > width * 2 / 3:  # 右侧1/3区域
                self.next_page()
                return True
        elif event.type() == event.MouseMove:
            # 处理拖动
            view = obj.parent()
            if view.drag_start_pos is not None:
                # 计算移动距离
                delta = event.pos() - view.drag_start_pos
                # 应用移动
                view.setTransform(view.original_transform)
                view.translate(delta.x(), delta.y())
                return True
        elif event.type() == event.MouseButtonRelease:
            # 释放鼠标时恢复原位
            view = obj.parent()
            if event.button() == Qt.MiddleButton:
                # 恢复原始位置
                view.setTransform(view.original_transform)
                view.drag_start_pos = None
                view.original_transform = None
                return True
        elif event.type() == event.MouseButtonDblClick:
            # 双击重置缩放
            view = obj.parent()
            view.resetTransform()
            # 重新适应视图
            if view == self.view_left:
                self.view_left.fitInView(self.scene_left.itemsBoundingRect(), Qt.KeepAspectRatio)
            elif view == self.view_right:
                self.view_right.fitInView(self.scene_right.itemsBoundingRect(), Qt.KeepAspectRatio)
            return True
        elif event.type() == event.Wheel:
            # 处理滚轮事件
            delta = event.angleDelta().y()
            view = obj.parent()
            
            if self.long_image_mode:
                # 长图模式下，滚轮控制垂直滚动
                # 向下滚动时图片向上移动
                scroll_amount = delta / 120 * 20  # 调整滚动速度
                view.verticalScrollBar().setValue(
                    int(view.verticalScrollBar().value() - scroll_amount)
                )
            else:
                # 普通模式下，滚轮控制缩放
                factor = 1.1 if delta > 0 else 0.9
                view.scale(factor, factor)
                
                # 限制缩放范围
                current_scale = view.transform().m11()
                if current_scale < 0.1:  # 最小缩放
                    view.resetTransform()
                    view.scale(0.1, 0.1)
                elif current_scale > 10:  # 最大缩放
                    view.resetTransform()
                    view.scale(10, 10)
            
            return True
                
        return super().eventFilter(obj, event)

    def resizeEvent(self, event):
        """窗口大小改变时重新调整图片显示"""
        super().resizeEvent(event)
        # 重新加载当前图片以适应新的大小
        self.load_image()

if __name__ == "__main__":
    app = QApplication(sys.argv)
    viewer = ComicViewer()
    viewer.show()
    sys.exit(app.exec_())