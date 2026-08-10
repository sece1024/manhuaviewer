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
    ├── main.rs                     # 入口（Axum 服务 + 单实例锁 + 启动失败提示）
    ├── logging.rs                  # 按天滚动文件日志 + panic hook
    ├── db/                         # rusqlite 封装
    │   ├── mod.rs                  # Database 结构体 + 全部 SQL 查询
    │   ├── schema.rs               # 幂等建表 SQL
    │   └── migrations.rs           # 旧版数据表迁移 + 列补充
    ├── routes/                     # Axum 路由（archives/tags/categories/history/settings/opds）
    └── services/                   # 业务逻辑（archive/scanner/thumbnail/cbz）

frontend/
├── src/
│   ├── App.js                      # 路由 + 主题 + ErrorBoundary
│   ├── index.js                    # 入口
│   ├── index.css                   # 全局样式（三套主题/响应式）
│   ├── components/                 # Toast/LazyImage/ErrorBoundary/TagPicker/CategoryPicker/ConfirmDialog
│   ├── hooks/                      # useSettings/useTags/useReaderKeyboard
│   ├── pages/                      # Library/Reader/History/Settings
│   ├── utils/                      # api.js（唯一 API 客户端）/format.js
│   └── __tests__/                  # 测试文件
└── package.json
```

## 📡 API 文档

所有接口均挂在 `/api` 前缀下（OPDS 接口挂在 `/opds`）：

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/archives` | GET | 档案列表（支持 search, tag, category_id, group_id, sort/sort_by, order/sort_order, page, limit） |
| `/api/archives/:id` | GET | 档案详情 |
| `/api/archives/:id` | DELETE | 删除档案 |
| `/api/archives/:id/title` | PUT | 重命名档案 |
| `/api/archives/:id/cover` | GET | 封面缩略图 |
| `/api/archives/:id/pages` | GET | 页面列表 |
| `/api/archives/:id/pages/:page` | GET | 单页图片（支持 ETag/Last-Modified 缓存） |
| `/api/archives/:id/pages/:page/thumb` | GET | 单页缩略图 |
| `/api/archives/batch-delete` | POST | 批量删除档案 |
| `/api/archives/pack-cbz` | POST | 将文件夹打包为 CBZ |
| `/api/open` | POST | 直接打开文件/文件夹路径 |
| `/api/scan` | POST | 扫描目录 |
| `/api/merge` | POST | 合并档案为章节组 |
| `/api/cbz/list` | GET | 列出可打包的 CBZ 文件 |
| `/api/tags` | GET/POST | 标签列表 / 创建标签 |
| `/api/tags/:id` | PUT/DELETE | 更新 / 删除标签 |
| `/api/tags/assign` | POST | 给档案分配标签 |
| `/api/tags/batch-assign` | POST | 批量分配标签 |
| `/api/tags/batch-remove` | POST | 批量移除标签 |
| `/api/tags/namespaces` | GET | 标签命名空间列表 |
| `/api/archives/:id/tags` | GET | 档案的标签列表 |
| `/api/tags/:archive_id/:tag_id` | DELETE | 移除档案上的标签 |
| `/api/categories` | GET/POST | 分类列表 / 创建分类 |
| `/api/categories/:id` | PUT/DELETE | 更新 / 删除分类 |
| `/api/categories/assign` | POST | 给档案分配分类 |
| `/api/categories/batch-assign` | POST | 批量分配分类 |
| `/api/categories/batch-remove` | POST | 批量移除分类 |
| `/api/archives/:id/categories` | GET | 档案的分类列表 |
| `/api/categories/:archive_id/:category_id` | DELETE | 移除档案上的分类 |
| `/api/history` | GET | 阅读历史（分页） |
| `/api/history` | POST/DELETE | 保存进度 / 清空历史 |
| `/api/history/:archive_id` | DELETE | 删除单条历史 |
| `/api/settings` | GET/PUT | 获取 / 更新设置 |
| `/api/config` | GET/PUT | 根目录配置 |
| `/api/stats` | GET | 数据库统计 |
| `/api/backup` | GET | 导出备份 |
| `/api/restore` | POST | 导入备份 |
| `/opds/` | GET | OPDS 根目录 |
| `/opds/catalog` | GET | OPDS 全部档案 |
| `/opds/archive/:id` | GET | OPDS 档案详情 |
| `/opds/recent` | GET | OPDS 最近阅读 |
| `/opds/tags` | GET | OPDS 标签目录 |
| `/opds/tag/:tag_id` | GET | OPDS 标签下的档案 |
| `/opds/categories` | GET | OPDS 分类目录 |

## 📄 License

[MIT](LICENSE)
