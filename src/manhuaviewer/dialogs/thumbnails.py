"""缩略图总览对话框 - 异步加载"""
import logging
import os

from PyQt5.QtWidgets import (
    QDialog, QVBoxLayout, QPushButton, QLabel, QScrollArea, QWidget, QGridLayout,
)
from PyQt5.QtGui import QPixmap
from PyQt5.QtCore import Qt, QThreadPool, QRunnable, pyqtSignal, QObject

from manhuaviewer.constants import THUMBNAIL_SIZE

logger = logging.getLogger(__name__)


class ThumbnailSignals(QObject):
    """缩略图加载信号"""
    loaded = pyqtSignal(int, QPixmap)  # index, pixmap


class ThumbnailLoader(QRunnable):
    """异步缩略图加载任务"""

    def __init__(self, index: int, filepath: str):
        super().__init__()
        self.index = index
        self.filepath = filepath
        self.signals = ThumbnailSignals()
        self.setAutoDelete(True)

    def run(self):
        try:
            pixmap = QPixmap(self.filepath)
            if not pixmap.isNull():
                scaled = pixmap.scaled(
                    THUMBNAIL_SIZE, THUMBNAIL_SIZE,
                    Qt.KeepAspectRatio, Qt.SmoothTransformation
                )
                self.signals.loaded.emit(self.index, scaled)
        except Exception as e:
            logger.warning(f"缩略图加载失败 [{self.index}]: {self.filepath}: {e}")


class ThumbnailDialog(QDialog):
    """缩略图总览对话框"""

    page_selected = pyqtSignal(int)

    def __init__(self, image_files: list[str], current_index: int, parent=None):
        super().__init__(parent)
        self.image_files = image_files
        self.current_index = current_index
        self._thumb_labels: dict[int, QLabel] = {}
        self._closed = False

        self.setWindowTitle(f"缩略图总览 ({len(image_files)} 页)")
        self.setMinimumSize(700, 500)

        layout = QVBoxLayout(self)

        # 滚动区域
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)

        self._container = QWidget()
        self._grid = QGridLayout(self._container)
        self._grid.setSpacing(8)

        # 先创建占位标签
        cols = 4
        for i, filepath in enumerate(image_files):
            thumb_widget = QWidget()
            thumb_layout = QVBoxLayout(thumb_widget)
            thumb_layout.setContentsMargins(4, 4, 4, 4)
            thumb_layout.setSpacing(4)

            # 占位标签
            label = QLabel("加载中...")
            label.setAlignment(Qt.AlignCenter)
            label.setFixedSize(THUMBNAIL_SIZE, THUMBNAIL_SIZE)
            label.setStyleSheet(
                "border: 1px solid #dddddd; border-radius: 4px; color: #999; font-size: 12px;"
            )
            if i == current_index:
                label.setStyleSheet(
                    "border: 3px solid #4a86e8; border-radius: 4px; color: #999; font-size: 12px;"
                )

            self._thumb_labels[i] = label
            label.setCursor(Qt.PointingHandCursor)

            # 使用闭包正确捕获索引
            idx = i
            label.mousePressEvent = lambda e, x=idx: self._on_click(x)
            thumb_layout.addWidget(label)

            # 页码和文件名
            name_label = QLabel(f"{i + 1}. {os.path.basename(filepath)}")
            name_label.setAlignment(Qt.AlignCenter)
            name_label.setStyleSheet("font-size: 11px; color: #666;")
            name_label.setWordWrap(True)
            thumb_layout.addWidget(name_label)

            self._grid.addWidget(thumb_widget, i // cols, i % cols)

        scroll.setWidget(self._container)
        layout.addWidget(scroll)

        # 关闭按钮
        btn_close = QPushButton("关闭")
        btn_close.clicked.connect(self.accept)
        layout.addWidget(btn_close)

        # 使用独立线程池，关闭时可清理
        self._thread_pool = QThreadPool(self)
        self._thread_pool.setMaxThreadCount(4)
        self._load_thumbnails()

    def _load_thumbnails(self):
        """异步加载所有缩略图"""
        for i, filepath in enumerate(self.image_files):
            loader = ThumbnailLoader(i, filepath)
            loader.signals.loaded.connect(self._on_thumbnail_loaded)
            self._thread_pool.start(loader)

    def _on_thumbnail_loaded(self, index: int, pixmap: QPixmap):
        """缩略图加载完成回调，忽略关闭后的残留信号"""
        if self._closed:
            return
        if index in self._thumb_labels:
            label = self._thumb_labels[index]
            label.setPixmap(pixmap)
            if index == self.current_index:
                label.setStyleSheet("border: 3px solid #4a86e8; border-radius: 4px;")
            else:
                label.setStyleSheet("border: 1px solid #dddddd; border-radius: 4px;")

    def reject(self):
        self._closed = True
        self._thread_pool.clear()
        super().reject()

    def accept(self):
        self._closed = True
        self._thread_pool.clear()
        super().accept()

    def _on_click(self, index):
        self.page_selected.emit(index)
        self.accept()
