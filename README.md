# 漫画浏览器 (ManhuaViewer)

一个基于 PyQt5 的桌面漫画/图片浏览器，支持单页、双页、长图模式。

## 功能特性

- 📖 **单页/双页模式** — 适配普通漫画和对开页
- 🖼️ **长图模式** — 适合条漫、长条图浏览
- ⚡ **预加载** — 后台预加载后续页面，翻页无延迟
- 🎨 **主题切换** — 浅色 / 深色 / 护眼三种主题
- 📂 **最近文件** — 自动记住最近打开的文件夹
- 🔍 **缩放拖拽** — 滚轮缩放，中键拖拽，双击重置
- 📊 **阅读历史** — 自动记录每个文件夹的阅读进度，再次打开自动恢复
- 🏷️ **标签系统** — 给漫画文件夹打标签，方便分类整理
- 🖼️ **缩略图总览** — 一页看所有页面，点击直达
- 🔄 **图片旋转** — 支持顺时针/逆时针 90° 旋转
- ⛶ **全屏模式** — 沉浸式阅读体验
- 🎯 **跳转到页** — 快速跳转到指定页码
- ⌨️ **快捷键** — 完整的键盘操作支持

## 支持格式

JPG, JPEG, PNG, BMP, WebP, GIF, TIFF

## 快捷键

| 按键 | 功能 |
|------|------|
| ← / A | 上一页 |
| → / Space | 下一页 |
| D | 切换双页模式 |
| L | 切换长图模式 |
| R | 顺时针旋转 90° |
| Shift+R | 逆时针旋转 90° |
| T | 缩略图总览 |
| G | 跳转到指定页 |
| M | 管理标签 |
| F11 | 全屏模式 |
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
# 安装依赖
uv pip install --lock

# 运行
uv run comic_viewer_enhance.py

# 或者通过入口文件
uv run main.py
```

## 打包为 exe

```bash
uv run build.py
```

生成的 `漫画浏览器.exe` 在 `dist/` 目录下。

## 运行测试

```bash
pip install pytest
pytest tests/ -v
```

## 项目结构

```
├── main.py                    # 入口点
├── comic_viewer_enhance.py    # 主程序（增强版浏览器）
├── data_store.py              # 数据持久化层（阅读历史 + 标签）
├── build.py                   # PyInstaller 打包脚本
├── tests/                     # 单元测试
│   └── test_data_store.py
├── pyproject.toml             # 项目配置
├── requirements.txt           # 依赖列表
├── uv.lock                    # uv 锁文件
└── LICENSE                    # MIT 许可证
```

## 数据存储

- 阅读历史和标签数据保存在:
  - **Windows**: `%APPDATA%/ManhuaViewer/`
  - **Linux/macOS**: `~/.local/share/ManhuaViewer/`

## License

[MIT](LICENSE)
