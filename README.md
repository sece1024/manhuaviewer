# MangaViewer v3

一个基于 Tauri 2.0 + React 的跨平台漫画阅读管理系统，支持文件夹和压缩包（ZIP/CBZ/RAR/CBR），提供原生桌面应用体验。

## ✨ 功能特性

- 📚 **漫画库** — 封面卡片网格 / 列表视图切换，按标签、分类、名称筛选
- 📖 **阅读器** — 单页/双页/长图模式，RTL/LTR 翻页方向，适应高度/宽度/原始大小
- 📦 **压缩包支持** — ZIP/CBZ/RAR/CBR 直接浏览，无需解压
- 📦 **CBZ 归档** — 将漫画文件夹打包为 CBZ 格式
- 🏷️ **命名空间标签** — 支持 `artist:name`、`series:name` 格式
- 📂 **分类系统** — 动态/静态分类，支持置顶
- 🔍 **搜索过滤** — 按名称模糊搜索，标签过滤侧栏
- 📊 **阅读历史** — 自动保存进度，断点续读
- 🎨 **主题系统** — 浅色/深色/护眼三套主题
- 📱 **移动端适配** — 响应式布局，触摸手势（缩放/双击/滑动翻页）
- ⌨️ **快捷键** — 完整的键盘操作支持
- 🖥️ **跨平台应用** — macOS, Windows, Linux（基于 Tauri 2.0）

## 🚀 快速开始

### 环境要求

- Node.js >= 18
- pnpm
- Rust (用于 Tauri 构建)

### 安装与启动

```bash
git clone https://github.com/sece1024/manhuaviewer.git
cd manhuaviewer
pnpm install
pnpm tauri dev                 # 开发模式（热重载）
pnpm tauri build               # 生产构建
```

## 📁 支持格式

| 类型 | 格式 |
|------|------|
| 图片 | JPG, PNG, BMP, WebP, GIF, TIFF, AVIF |
| 压缩包 | ZIP, CBZ, RAR, CBR, 7Z |
| 文件夹 | 直接包含图片的文件夹 |

## ⌨️ 快捷键

| 按键 | 功能 |
|------|------|
| ← / → | 翻页（方向取决于 RTL/LTR 设置） |
| Space | 下一页 |
| D | 切换双页模式 |
| L | 切换长图模式 |
| R / Shift+R | 旋转（顺时针/逆时针） |
| T | 缩略图总览 |
| G | 跳转到指定页 |
| W | 循环切换适应模式 |
| Home / End | 第一页 / 最后一页 |
| F11 | 全屏模式 |
| Esc | 关闭弹出面板 |

## 🗂️ 项目结构

```
src-tauri/                          # Tauri + Rust 后端
├── Cargo.toml
├── tauri.conf.json
├── capabilities/default.json       # Tauri 权限配置
└── src/
    ├── main.rs                     # 入口（Axum 服务）
    ├── db/                         # rusqlite 封装
    ├── routes/                     # Axum 路由
    └── services/                   # 业务逻辑（解压、缩略图等）

frontend/
├── src/
│   ├── App.js                      # 路由 + 主题 + ErrorBoundary
│   ├── index.js                    # 入口
│   ├── index.css                   # 全局样式（三套主题/响应式）
│   ├── components/
│   │   ├── ErrorBoundary.js        # 错误边界组件
│   │   ├── LazyImage.js            # 懒加载图片组件
│   │   └── Toast.js                # 通知组件
│   ├── hooks/
│   │   ├── useReaderKeyboard.js    # 阅读器快捷键 hook
│   │   └── useSettings.js          # 全局设置 hook（Context）
│   ├── pages/
│   │   ├── Library.js              # 漫画库（封面卡片/列表/过滤）
│   │   ├── Reader.js               # 阅读器（RTL/适应模式/overlay）
│   │   ├── History.js              # 阅读历史
│   │   └── Settings.js             # 设置页面（标签/分类管理+统计）
│   ├── utils/
│   │   ├── api.js                  # API 客户端
│   │   └── format.js               # 格式化工具函数
│   └── __tests__/                  # 测试文件
└── package.json
```

## 📡 API 文档

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/archives` | GET | 获取档案列表（支持 search, tag, category, sort_by, sort_order） |
| `/api/archives/:id` | GET | 获取档案详情 |
| `/api/archives/:id/pages` | GET | 获取档案页面列表 |
| `/api/archives/:id/pages/:index` | GET | 读取单页图片 |
| `/api/archives/:id/cover` | GET | 获取封面缩略图 |
| `/api/archives/:id` | DELETE | 删除档案 |
| `/api/open` | POST | 直接打开文件/文件夹路径 |
| `/api/scan` | POST | 扫描根目录 |
| `/api/tags` | GET/POST | 标签列表 / 创建标签 |
| `/api/tags/:id` | PUT/DELETE | 更新 / 删除标签 |
| `/api/tags/assign` | POST | 给档案分配标签 |
| `/api/categories` | GET/POST | 分类列表 / 创建分类 |
| `/api/categories/:id` | PUT/DELETE | 更新 / 删除分类 |
| `/api/history` | GET/POST/DELETE | 历史列表 / 保存进度 / 清空 |
| `/api/settings` | GET/PUT | 获取 / 更新设置 |
| `/api/stats` | GET | 数据库统计 |
| `/api/config` | GET/PUT | 根目录配置 |

## 📄 License

[MIT](LICENSE)
