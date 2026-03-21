"""常量定义"""

# 支持的图片格式
SUPPORTED_FORMATS = ["*.jpg", "*.jpeg", "*.png", "*.bmp", "*.webp", "*.gif", "*.tiff"]

# 预加载前后页数
PRELOAD_BEFORE = 3
PRELOAD_AFTER = 6

# 预加载缓存最大条目数
CACHE_MAX_SIZE = 40

# 缩略图尺寸
THUMBNAIL_SIZE = 150

# 最近文件列表上限
MAX_RECENT_FILES = 10

# 数据目录名称
APP_DATA_DIR = "ManhuaViewer"

# ── 交互参数 ──

# 滚轮缩放系数
ZOOM_FACTOR = 1.15

# 长图模式滚轮/键盘滚动步长 (px)
LONG_SCROLL_STEP = 30
LONG_KEY_SCROLL_STEP = 80

# 缩放范围
ZOOM_MIN = 0.1
ZOOM_MAX = 10.0

# resize 防抖间隔 (ms)
RESIZE_DEBOUNCE_MS = 150
