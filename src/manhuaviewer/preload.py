"""预加载模块 - LRU 缓存 + 线程安全预加载"""
import logging
import threading
from collections import OrderedDict

from PyQt5.QtGui import QImage, QPixmap
from PyQt5.QtCore import QThread, pyqtSignal

from manhuaviewer.constants import PRELOAD_BEFORE, PRELOAD_AFTER, CACHE_MAX_SIZE

logger = logging.getLogger(__name__)


class LRUCache:
    """线程安全的 LRU 缓存，用于存储预加载的 QPixmap"""

    def __init__(self, max_size: int = CACHE_MAX_SIZE):
        self._cache: OrderedDict[int, QPixmap] = OrderedDict()
        self._max_size = max_size
        self._lock = threading.Lock()

    def get(self, key: int):
        with self._lock:
            if key in self._cache:
                self._cache.move_to_end(key)
                return self._cache[key]
            return None

    def put(self, key: int, value: QPixmap):
        with self._lock:
            if key in self._cache:
                self._cache.move_to_end(key)
                self._cache[key] = value
            else:
                self._cache[key] = value
                while len(self._cache) > self._max_size:
                    self._cache.popitem(last=False)

    def clear(self):
        with self._lock:
            self._cache.clear()

    def __contains__(self, key: int):
        with self._lock:
            return key in self._cache

    def __len__(self):
        with self._lock:
            return len(self._cache)


class PreloadThread(QThread):
    """后台预加载线程，使用 QImage（线程安全）"""
    loaded = pyqtSignal(int, QImage)

    def __init__(self, image_files: list[str], center_index: int, cache: LRUCache):
        super().__init__()
        self.image_files = image_files
        self.center_index = center_index
        self.cache = cache

    def run(self):
        start = max(0, self.center_index - PRELOAD_BEFORE)
        end = min(len(self.image_files), self.center_index + PRELOAD_AFTER)
        for i in range(start, end):
            if i not in self.cache:
                try:
                    img = QImage(self.image_files[i])
                    if not img.isNull():
                        self.loaded.emit(i, img)
                except Exception as e:
                    logger.warning(f"预加载图片失败 [{i}]: {self.image_files[i]}: {e}")
