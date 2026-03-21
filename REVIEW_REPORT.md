# 📋 代码审查报告：架构、性能、质量改进建议

> **审查人**: AI Code Review  
> **审查范围**: 全量代码 (~1541 行)  
> **审查日期**: 2026-03-21

---

## 一、架构问题 🔴 高优先级

### 1. viewer.py 单体文件过重 (1034 行)

**现状**: 所有 UI 类、对话框、业务逻辑、样式表全塞在一个文件里。

**影响**:
- 难以定位和维护代码
- 新功能添加会导致文件继续膨胀
- 代码复用率低

**建议拆分**:
```
src/manhuaviewer/
├── viewer.py          # ComicViewer 主窗口 (~300行)
├── dialogs/           # 对话框模块
│   ├── __init__.py
│   ├── settings.py    # SettingsDialog
│   ├── history.py     # HistoryDialog
│   ├── tags.py        # TagDialog
│   ├── thumbnails.py  # ThumbnailDialog
│   └── jump.py        # JumpPageDialog
├── styles.py          # 样式表 & 主题定义
├── constants.py       # 支持格式、预加载数量等常量
└── preload.py         # PreloadThread
```

### 2. 设置面板未恢复已保存值 🐛

**现状**: 每次打开「偏好设置」，缩放滑块始终显示 100，主题始终显示「浅色」，不读取当前状态。

**影响**: 用户无法确认当前设置，体验割裂。

**建议**:
```python
def __init__(self, parent=None, current_scale=1.0, current_theme="浅色"):
    super().__init__(parent)
    self.scale_slider.setValue(int(current_scale * 100))
    self.theme_combo.setCurrentText(current_theme)
```

### 3. 缩放设置 `scale_factor` 未生效 🐛

**现状**: `SettingsDialog` 返回的 `scale_factor` 被存储到 `self.scale_factor`，但 `_show_single()` / `_show_double()` 中从未引用它。缩放设置形同虚设。

**建议**: 在图片显示逻辑中应用 `scale_factor`，或移除此设置以避免误导。

---

## 二、性能问题 🟠 中优先级

### 4. 预加载缓存无上限，内存泄漏风险

**现状**: `self.preloaded_images` 是普通 dict，只增不减。对于 500+ 页的漫画，会把所有已浏览页面的 QPixmap 都留在内存中。

**建议**: 使用 LRU 缓存，只保留当前页前后各 N 页：

```python
from collections import OrderedDict

class LRUCache:
    def __init__(self, max_size=30):
        self._cache = OrderedDict()
        self._max_size = max_size

    def get(self, key):
        if key in self._cache:
            self._cache.move_to_end(key)
            return self._cache[key]
        return None

    def put(self, key, value):
        self._cache[key] = value
        self._cache.move_to_end(key)
        while len(self._cache) > self._max_size:
            self._cache.popitem(last=False)
```

### 5. 缩略图总览同步加载，大文件夹卡死

**现状**: `ThumbnailDialog.__init__` 遍历所有图片同步生成 QPixmap 缩略图。500 页漫画打开缩略图对话框可能卡住数秒。

**建议**:
- 使用 `QThreadPool` + `QRunnable` 异步加载缩略图
- 或使用懒加载：只加载可视区域的缩略图（结合 `QScrollArea` 的滚动事件）

### 6. 预加载线程与主线程共享缓存无锁保护

**现状**: `PreloadThread` 和主线程同时读写 `self.preloaded_images`。虽然 Python GIL 提供了一定保护，但设计上不安全。

**建议**: 使用 `threading.Lock` 保护缓存访问，或改为纯信号通信（预加载线程只通过 `pyqtSignal` 传递结果）。

---

## 三、稳定性问题 🟠 中优先级

### 7. 图片加载无异常处理

**现状**: `_get_pixmap()` 和 `PreloadThread.run()` 中直接用 `QPixmap(filepath)` / `QImage(filepath)` 加载图片，没有 try/except。

**风险**: 文件损坏、磁盘读取错误、权限问题都会导致未处理异常崩溃。

**建议**:
```python
def _get_pixmap(self, index):
    try:
        pixmap = QPixmap(self.image_files[index])
        if not pixmap.isNull():
            return pixmap
    except Exception as e:
        logging.warning(f"加载图片失败: {self.image_files[index]}: {e}")
    return None
```

### 8. 无日志系统

**现状**: 整个项目没有任何 logging 调用。用户遇到问题无法提供有效信息排查。

**建议**: 至少在关键路径添加基础日志：
```python
import logging
logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
```

---

## 四、功能缺失 🟡 低优先级

### 9. 不支持拖拽打开文件夹

**建议**: 实现 `dragEnterEvent` / `dropEvent` 支持拖拽文件夹到窗口。

### 10. 没有「帮助 / 关于」菜单

**建议**: 添加帮助菜单，包含快捷键列表和版本信息。

### 11. 设置中的「背景颜色」按钮未实现功能

**现状**: `_choose_bg_color()` 弹出颜色选择器但只改了按钮自身样式，没有实际改变背景色。

---

## 五、工程规范 🟡 低优先级

### 12. 缺少 CI/CD 流水线

**建议**: 添加 `.github/workflows/ci.yml`：
```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: astral-sh/setup-uv@v4
      - run: uv venv && uv pip install -e ".[dev]"
      - run: uv run pytest tests/ -v
```

### 13. 缺少代码规范工具

**建议**: 在 `pyproject.toml` 中添加 ruff 配置：
```toml
[tool.ruff]
line-length = 120
target-version = "py311"

[tool.ruff.lint]
select = ["E", "F", "W", "I", "N", "UP"]
```

### 14. 测试覆盖率不足

**现状**: 19 个测试仅覆盖 `data_store.py`，主逻辑 `viewer.py` 无任何测试。

**建议**:
- 至少为 `PreloadThread`、`_get_pixmap`、翻页逻辑等添加单元测试
- 对话框逻辑可用 mock 测试

### 15. 缺少应用图标

**现状**: `build.py` 中 `--icon=NONE`，打包后无图标。

**建议**: 添加 `assets/icon.ico` / `icon.icns` 并在打包脚本中引用。

### 16. main.py 与 viewer.py 中的 `main_cli()` 功能重复

**现状**: `main.py` 和 `viewer.py` 底部都定义了启动逻辑，入口不统一。

**建议**: `main.py` 只做一件事：`from manhuaviewer.viewer import main_cli; main_cli()`

---

## 六、安全与兼容性

### 17. 文件路径未做跨平台处理

**现状**: 虽然用了 `os.path.abspath`，但未处理含特殊字符（中文、emoji）的路径，Windows 上可能出问题。

### 18. .gitignore 中缺少 IDE 配置常见项

**建议**: 添加 `.cursor/`, `.claude/`, `.aider*` 等现代 AI 编辑器配置。

---

## 总结

| 优先级 | 数量 | 关键项 |
|--------|------|--------|
| 🔴 高 | 3 | 文件拆分、设置恢复、缩放失效 |
| 🟠 中 | 4 | 内存泄漏、缩略图卡顿、线程安全、异常处理 |
| 🟡 低 | 6 | 拖拽、帮助菜单、CI、测试、图标、入口统一 |
| ⚪ 建议 | 5 | 日志、代码规范、路径处理等 |

**建议优先处理**: 第 1、2、3、4、7 项，这些直接影响用户体验和稳定性。
