import os
import sys
from PyQt5.QtWidgets import (
    QApplication, QMainWindow, QGraphicsView, QGraphicsScene, QFileDialog,
    QVBoxLayout, QWidget, QHBoxLayout, QPushButton, QLabel
)
from PyQt5.QtGui import QPixmap, QImage, QWheelEvent
from PyQt5.QtCore import Qt, QDir


class ComicViewer(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Python 漫画浏览器")
        self.setGeometry(100, 100, 800, 600)

        # 变量初始化
        self.image_files = []
        self.current_index = 0
        self.scale_factor = 1.0

        # 主界面布局
        self.central_widget = QWidget()
        self.setCentralWidget(self.central_widget)
        self.layout = QVBoxLayout(self.central_widget)

        # 工具栏
        self.toolbar = QHBoxLayout()
        self.btn_open = QPushButton("打开文件夹")
        self.btn_prev = QPushButton("上一页")
        self.btn_next = QPushButton("下一页")
        self.label_status = QLabel("未加载图片")

        self.toolbar.addWidget(self.btn_open)
        self.toolbar.addWidget(self.btn_prev)
        self.toolbar.addWidget(self.btn_next)
        self.toolbar.addWidget(self.label_status)
        self.layout.addLayout(self.toolbar)

        # 图片显示区域
        self.view = QGraphicsView()
        self.scene = QGraphicsScene()
        self.view.setScene(self.scene)
        self.layout.addWidget(self.view)

        # 连接信号槽
        self.btn_open.clicked.connect(self.open_folder)
        self.btn_prev.clicked.connect(self.prev_page)
        self.btn_next.clicked.connect(self.next_page)

    def open_folder(self):
        """选择文件夹并加载图片"""
        folder = QFileDialog.getExistingDirectory(self, "选择漫画文件夹")
        if not folder:
            return

        # 获取文件夹内所有图片文件
        self.image_files = []
        for ext in ["*.jpg", "*.jpeg", "*.png", "*.bmp", "*.webp"]:
            self.image_files.extend(QDir(folder).entryList([ext], QDir.Files))
        
        if not self.image_files:
            self.label_status.setText("未找到图片文件！")
            return

        self.image_files = [os.path.join(folder, f) for f in self.image_files]
        self.current_index = 0
        self.load_image()

    def load_image(self):
        """加载当前图片"""
        if not self.image_files:
            return

        self.scene.clear()
        pixmap = QPixmap(self.image_files[self.current_index])
        if pixmap.isNull():
            self.label_status.setText("加载失败: " + self.image_files[self.current_index])
            return

        # 显示图片
        self.scene.addPixmap(pixmap)
        self.view.fitInView(self.scene.itemsBoundingRect(), Qt.KeepAspectRatio)
        self.label_status.setText(
            f"{self.current_index + 1}/{len(self.image_files)}: "
            f"{os.path.basename(self.image_files[self.current_index])}"
        )

    def prev_page(self):
        """上一页"""
        if self.current_index > 0:
            self.current_index -= 1
            self.load_image()

    def next_page(self):
        """下一页"""
        if self.current_index < len(self.image_files) - 1:
            self.current_index += 1
            self.load_image()

    def wheelEvent(self, event: QWheelEvent):
        """鼠标滚轮缩放"""
        if event.angleDelta().y() > 0:
            self.view.scale(1.1, 1.1)  # 放大
        else:
            self.view.scale(0.9, 0.9)  # 缩小


if __name__ == "__main__":
    app = QApplication(sys.argv)
    viewer = ComicViewer()
    viewer.show()
    sys.exit(app.exec_())