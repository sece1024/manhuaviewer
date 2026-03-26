# MangaViewer v2 — 待完成事项 (TODO)

> 基于 LANraragi 项目参考的优化重构，分支 `web-rewrite-v2`。
> 以下按优先级排列，标注了当前状态和依赖关系。

---

## ✅ 已完成

- [x] **数据库 schema 重构** — 统一 archives 表支持文件夹和压缩包
- [x] **压缩包支持** — ZIP/CBZ/RAR/CBR 解压、图片提取、封面生成（archiveService.js）
- [x] **扫描服务** — scanService.js 同时处理文件夹和压缩包发现
- [x] **命名空间标签** — 支持 `artist:name`、`series:name` 格式
- [x] **分类系统** — Categories CRUD，支持动态/静态分类
- [x] **Library 页面** — 封面卡片网格 + 列表视图切换
- [x] **标签过滤侧栏** — 按命名空间分组，点击筛选
- [x] **排序** — 按最近阅读/名称/添加时间/页数/大小
- [x] **封面图** — 自动从第一页生成缩略图
- [x] **Reader 改进** — RTL/LTR 翻页、适应高度/宽度/原始三种模式、阅读器 overlay
- [x] **Settings 页面** — 目录、阅读器、外观设置 + 标签/分类管理 + 统计面板
- [x] **主题系统** — 浅色/深色/护眼三套主题
- [x] **移动端适配** — 底部导航、触摸手势（缩放/双击/滑动翻页）

---

## 🔧 待完成 — 高优先级

### 1. 更新 README.md
- **状态**: 未开始
- **说明**: 当前 README 仍描述旧版 PyQt5 桌面版，需要重写为 Web 版介绍
- **内容**:
  - 新的项目介绍和截图
  - Web 版安装和启动说明（`cd backend && npm install && npm start`）
  - 支持的压缩包格式列表
  - 新功能概览（命名空间标签、分类、封面等）
  - API 文档简要说明

### 2. 后端 npm install 验证
- **状态**: 未验证
- **说明**: 新增了 `adm-zip` 和 `node-unrar-js` 依赖，需要 `cd backend && npm install` 测试安装
- **注意**: `node-unrar-js` 在某些平台可能需要额外配置，需要确认兼容性

### 3. 数据库迁移完善
- **状态**: 基础迁移已写，但未充分测试
- **说明**: database.js 中有旧表 `folders` → 新表 `archives` 的迁移逻辑
- **待办**:
  - 迁移 `images` 表数据到 `pages` 表（仅压缩包类型需要）
  - 迁移旧 `folder_tags` 到新 `archive_tags`
  - 迁移旧 `history`（folder_id → archive_id 映射）
  - 添加迁移失败的回滚机制

### 4. OPDS Catalog 支持（参考 LANraragi）
- **状态**: 未开始
- **说明**: LANraragi 支持 OPDS 协议，允许第三方阅读器（如 Perfect Viewer、ComicScreen）通过标准协议访问漫画库
- **实现**: 新增 `/opds` 路由，生成 XML catalog feed
- **价值**: 允许用户用手机/平板上的专用漫画阅读器浏览和阅读

---

## 🔧 待完成 — 中优先级

### 5. 7Z 格式支持
- **状态**: 未开始
- **说明**: 当前只支持 ZIP/CBZ/RAR/CBR，7Z 格式需要额外依赖
- **方案**: 使用 `7zip-min`（需要系统安装 7z）或纯 JS 方案

### 6. 批量操作
- **状态**: 未开始
- **说明**: 批量打标签、批量删除、批量移动分类
- **实现**: 前端多选模式 + 后端批量 API

### 7. 搜索增强
- **状态**: 基础搜索已实现（文件名模糊匹配）
- **待办**:
  - 支持标签搜索语法（如 `artist:xxx tag:yyy`）
  - 支持排除语法（如 `-已读`）
  - 搜索历史
  - LANraragi 风格的高级搜索过滤器

### 8. 重复检测
- **状态**: 未开始
- **说明**: LANraragi 支持扫描重复档案
- **方案**: 按文件 hash（MD5/SHA1）或文件名相似度检测

### 9. 备份与恢复
- **状态**: 未开始
- **说明**: LANraragi 支持导出/导入 JSON 备份
- **实现**:
  - 导出：`GET /api/backup` → 返回 JSON（包含所有档案元数据、标签、分类、阅读历史）
  - 导入：`POST /api/restore` → 从 JSON 恢复

### 10. 自动扫描定时器
- **状态**: 未开始
- **说明**: 设置 `auto_scan_interval` 字段已预留，但前端和后端均未实现
- **方案**: 后端使用 `node-cron` 或 `setInterval` 定时触发扫描

---

## 🔧 待完成 — 低优先级

### 11. 快捷键帮助页面
- **状态**: 未开始
- **说明**: Reader 中 F1 弹出快捷键列表
- **实现**: 新增 KeyboardShortcuts 组件

### 12. Docker 支持
- **状态**: 未开始
- **说明**: 参考 LANraragi 的 Docker 部署方式
- **文件**: Dockerfile + docker-compose.yml
- **说明**: 需要同时构建 frontend 和 backend

### 13. 拖拽上传/添加漫画
- **状态**: 未开始
- **说明**: 前端拖拽压缩包到浏览器直接上传
- **注意**: 大文件上传需要分片

### 14. 图片懒加载优化
- **状态**: 长图模式已使用 `loading="lazy"`
- **待办**: 网格封面图使用 IntersectionObserver 优化

### 15. 国际化 (i18n)
- **状态**: 未开始
- **说明**: LANraragi 使用 Weblate 做多语言
- **方案**: react-i18next 或简单 JSON 字典

### 16. 前端测试
- **状态**: 未开始
- **说明**: 旧版有 pytest 测试（test_data_store.py, test_preload.py），新版无前端测试
- **方案**: React Testing Library + Jest

---

## 🐛 已知问题

1. **Archive_tags 表 SQL 语法错误** — `CREATE TABLE IF IF NOT EXISTS`（双 IF），已修复
2. **旧 Python 测试失效** — tests/ 目录下的 pytest 测试是针对旧版 PyQt5 的，新版不适用
3. **imageRoutes.js 已删除但旧 API 可能被引用** — 前端 `api.js` 中 `imageUrl` 和 `thumbUrl` 已迁移到 `pageUrl`/`pageThumbUrl`
4. **node-unrar-js 跨平台兼容性** — 需要在不同 OS 上测试
5. **封面生成失败无 fallback** — 如果 sharp 不可用或图片损坏，封面显示为空

---

## 📁 新文件结构（web-rewrite-v2）

```
backend/src/
├── config/logger.js              # 日志
├── db/database.js                # 数据库 schema + 初始化
├── middleware/errorHandler.js     # 错误处理
├── services/
│   ├── archiveService.js         # ★ 压缩包解压服务（ZIP/RAR）
│   └── scanService.js            # ★ 目录扫描服务
├── routes/
│   ├── api.js                    # 路由聚合
│   ├── archiveRoutes.js          # ★ 档案 CRUD + 图片服务
│   ├── categoryRoutes.js         # ★ 分类 CRUD
│   ├── historyRoutes.js          # 阅读历史
│   ├── settingsRoutes.js         # 设置 + 统计
│   └── tagRoutes.js              # ★ 命名空间标签 CRUD
└── index.js                      # 入口

frontend/src/
├── components/Toast.js           # 通知组件
├── pages/
│   ├── Library.js                # ★ 全新漫画库（封面卡片/列表/过滤）
│   ├── Reader.js                 # ★ 改进阅读器（RTL/适应模式/overlay）
│   ├── History.js                # 阅读历史
│   └── Settings.js               # ★ 设置页面（标签/分类管理+统计）
├── utils/api.js                  # API 客户端
├── App.js                        # 路由 + 主题
├── index.js                      # 入口
└── index.css                     # ★ 全新 CSS（三套主题/响应式）
```

`★` = 新增或大幅重写的文件
