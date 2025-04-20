import os
import sys
from PyQt5.QtWidgets import (
    QApplication, QMainWindow, QGraphicsView, QGraphicsScene, QFileDialog,
    QVBoxLayout, QWidget, QHBoxLayout, QPushButton, QLabel, QCheckBox,
    QFrame, QSpacerItem, QSizePolicy
)
from PyQt5.QtGui import QPixmap, QImage, QIcon, QColor, QPainter
from PyQt5.QtCore import Qt, QDir, QTimer, QSize
from threading import Thread
import time

class ComicViewer(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Python 漫画浏览器 (增强版)")
        self.setGeometry(100, 100, 1200, 800)
        
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
        """)

        # 变量初始化
        self.image_files = []
        self.current_index = 0
        self.scale_factor = 1.0
        self.preload_thread = None
        self.preloaded_images = {}
        self.double_page_mode = False

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
            }
        """)
        toolbar_container.setLayout(self.toolbar)

        self.btn_open = QPushButton("打开文件夹")
        self.btn_prev = QPushButton("上一页 (←)")
        self.btn_next = QPushButton("下一页 (→)")
        self.check_double = QCheckBox("双页模式")
        self.label_status = QLabel("状态: 未加载图片")
        
        # 添加弹性空间
        spacer = QSpacerItem(40, 20, QSizePolicy.Expanding, QSizePolicy.Minimum)
        
        self.toolbar.addWidget(self.btn_open)
        self.toolbar.addWidget(self.btn_prev)
        self.toolbar.addWidget(self.btn_next)
        self.toolbar.addWidget(self.check_double)
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

    def open_folder(self):
        """选择文件夹并加载图片"""
        folder = QFileDialog.getExistingDirectory(self, "选择漫画文件夹")
        if not folder:
            return

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
                self.view_left.fitInView(self.scene_left.itemsBoundingRect(), Qt.KeepAspectRatio)
        # 双页模式
        else:
            self.scene_left.clear()
            self.scene_right.clear()
            pixmap_left = self.get_pixmap(self.current_index)
            pixmap_right = self.get_pixmap(self.current_index + 1) if self.current_index + 1 < len(self.image_files) else None

            if pixmap_left:
                self.scene_left.addPixmap(pixmap_left)
                self.view_left.fitInView(self.scene_left.itemsBoundingRect(), Qt.KeepAspectRatio)
            if pixmap_right:
                self.scene_right.addPixmap(pixmap_right)
                self.view_right.fitInView(self.scene_right.itemsBoundingRect(), Qt.KeepAspectRatio)

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

if __name__ == "__main__":
    app = QApplication(sys.argv)
    viewer = ComicViewer()
    viewer.show()
    sys.exit(app.exec_())