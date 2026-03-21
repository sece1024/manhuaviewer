# 漫画浏览器 (ManhuaViewer)

一个基于 PyQt5 的桌面漫画/图片浏览器，支持单页、双页、长图模式。

## 功能特性

- 📖 **单页/双页模式** — 适配普通漫画和对开页
- 🖼️ **长图模式** — 适合条漫、长条图浏览（支持 ↑/↓ 滚动）
- ⚡ **预加载** — LRU 缓存 + 后台预加载，翻页无延迟
- 🎨 **主题切换** — 浅色 / 深色 / 护眼三种主题 + 自定义背景色
- 📂 **最近文件** — 自动记住最近打开的文件夹
- 🔍 **缩放拖拽** — 滚轮缩放，中键拖拽，双击重置
- 📊 **阅读历史** — 自动记录每个文件夹的阅读进度，再次打开自动恢复
- 🏷️ **标签系统** — 给漫画文件夹打标签，方便分类整理
- 🖼️ **缩略图总览** — 异步加载，一页看所有页面，自动定位当前页
- 🔄 **图片旋转** — 支持顺时针/逆时针 90° 旋转
- ⛶ **全屏模式** — 沉浸式阅读（隐藏工具栏和菜单栏）
- 🎯 **跳转到页** — 快速跳转到指定页码
- 🖱️ **拖拽打开** — 直接拖拽文件夹到窗口打开
- 📋 **右键菜单** — 图片区右键快速操作
- ⌨️ **快捷键** — 完整的键盘操作支持（F1 查看列表）
- 📝 **状态栏增强** — 显示图片分辨率和文件大小

## 支持格式

JPG, JPEG, PNG, BMP, WebP, GIF, TIFF

## 快捷键

| 按键 | 功能 |
|------|------|
| ← / A | 上一页 |
| → / Space | 下一页 |
| ↑ / ↓ | 长图模式滚动 / 翻页 |
| D | 切换双页模式 |
| L | 切换长图模式 |
| R | 顺时针旋转 90° |
| Shift+R | 逆时针旋转 90° |
| T | 缩略图总览 |
| G | 跳转到指定页 |
| M | 管理标签 |
| F1 | 快捷键列表 |
| F11 | 全屏模式（Esc 退出） |
| Home | 跳到第一页 |
| End | 跳到最后一页 |
| Ctrl+O | 打开文件夹 |
| Ctrl+H | 阅读历史 |
| Ctrl+, | 偏好设置 |
| Ctrl+Q | 退出 |
| 滚轮 | 缩放 / 长图滚动 |
| 双击 | 重置缩放 |
| 中键拖拽 | 平移图片 |
| 点击左1/3 | 上一页 |
| 点击右1/3 | 下一页 |

## 本地运行

```bash
# 克隆项目
git clone https://github.com/sece1024/manhuaviewer.git
cd manhuaviewer

# 一键启动（自动创建 venv、安装依赖、运行）
# Windows:
run.bat
# macOS / Linux:
bash run.sh
```

或手动操作：

```bash
uv venv
uv pip install -e .

# Windows
.venv\Scripts\python main.py
# macOS / Linux
.venv/bin/python main.py

# 也可以
python -m manhuaviewer
```

> ⚠️ Windows 上不要用 `uv run`，它会重新解析依赖并拉到没有 Windows wheel 的 `pyqt5-qt5` 版本。用 `uv pip install` + 直接调用 python 即可。

## 打包为可执行文件

```bash
# 安装打包依赖
uv sync --extra build

# 打包 (自动识别平台)
uv run scripts/build.py
```

- Windows → `dist/漫画浏览器.exe`
- macOS → `dist/漫画浏览器.app`
- Linux → `dist/漫画浏览器`

## 运行测试

```bash
pip install pytest
pytest tests/ -v
```

## 项目结构

```
├── main.py                         # 入口点（兼容直接运行）
├── pyproject.toml                  # 项目配置 + 依赖 + ruff 规范
├── LICENSE                         # MIT 许可证
├── README.md
├── .gitignore
├── src/
│   └── manhuaviewer/
│       ├── __init__.py             # 版本号
│       ├── __main__.py             # python -m 入口
│       ├── viewer.py               # 主窗口
│       ├── constants.py            # 常量（格式、交互参数）
│       ├── styles.py               # 样式表 & 主题
│       ├── preload.py              # LRU 缓存 + 预加载线程
│       ├── data_store.py           # 数据持久化（原子写入）
│       └── dialogs/                # 对话框模块
│           ├── __init__.py
│           ├── settings.py         # 设置（恢复已保存值）
│           ├── history.py          # 阅读历史
│           ├── tags.py             # 标签管理
│           ├── thumbnails.py       # 缩略图（异步加载）
│           └── jump.py             # 跳转到页
├── scripts/
│   └── build.py                    # PyInstaller 打包脚本
└── tests/
    ├── test_data_store.py          # 数据层测试（19 个）
    └── test_preload.py             # LRU 缓存测试（7 个）
```

## 数据存储

- 阅读历史和标签数据保存在:
  - **Windows**: `%APPDATA%/ManhuaViewer/`
  - **Linux/macOS**: `~/.local/share/ManhuaViewer/`
- 使用原子写入（tempfile + os.replace），防止断电丢数据

## License

[MIT](LICENSE)
