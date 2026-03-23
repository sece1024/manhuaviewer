# MangaViewer Web 版实施计划

> 版本：v1.0 | 日期：2026-03-23
> 技术栈：React + Express + SQLite（对齐 trans-app）

---

## Phase 1：后端基础（API 骨架 + 数据库）

### 1.1 项目脚手架
- [x] 初始化 backend/ 目录（Express + package.json）
- [x] 配置 .gitignore、.env、nodemon
- [x] 基础中间件（cors、express.json、错误处理、日志）
- [x] 健康检查 API `GET /api`

### 1.2 数据库设计
- [x] SQLite 数据库初始化（better-sqlite3 或 sequelize）
- [x] `folders` 表 — 扫描到的漫画文件夹
- [x] `images` 表 — 文件夹内的图片（路径、排序、尺寸）
- [x] `history` 表 — 阅读进度（folder_id, page_index, total_pages, timestamp）
- [x] `tags` 表 — 标签定义（name, color）
- [x] `folder_tags` 表 — 文件夹-标签关联

### 1.3 文件扫描 API
- [x] `GET /api/config` — 获取根目录配置
- [x] `PUT /api/config` — 更新根目录
- [x] `POST /api/scan` — 触发扫描根目录下的漫画文件夹
- [x] `GET /api/folders` — 获取文件夹列表（支持搜索、标签筛选）

### 1.4 图片服务 API
- [x] `GET /api/folders/:id/images` — 获取文件夹图片列表
- [x] `GET /api/images/:id` — 获取原图（流式传输）
- [x] `GET /api/images/:id/thumbnail` — 缩略图（sharp 生成）

---

## Phase 2：后端业务逻辑（进度、标签、设置）

### 2.1 阅读进度 API
- [x] `GET /api/history` — 获取所有阅读历史
- [x] `POST /api/history` — 保存/更新阅读进度
- [x] `DELETE /api/history/:folderId` — 删除某条历史

### 2.2 标签管理 API
- [x] `GET /api/tags` — 获取所有标签
- [x] `POST /api/tags` — 创建标签
- [x] `PUT /api/tags/:id` — 更新标签（改名、改色）
- [x] `DELETE /api/tags/:id` — 删除标签
- [x] `POST /api/tags/assign` — 给文件夹分配标签
- [x] `DELETE /api/tags/:folderId/:tagId` — 移除文件夹标签

### 2.3 设置 API
- [x] `GET /api/settings` — 获取用户设置
- [x] `PUT /api/settings` — 更新设置

---

## Phase 3：前端基础（脚手架 + 阅读器核心）

### 3.1 项目脚手架
- [x] 初始化 frontend/ 目录（React）
- [x] 路由结构（React Router）
- [x] 全局布局（侧边栏 + 主内容区，复用 trans-app 风格）
- [x] API 封装层（utils/api.js）
- [x] Toast 通知组件

### 3.2 文件夹列表页
- [x] 根目录配置界面
- [x] 文件夹列表展示（卡片式）
- [x] 搜索 + 标签筛选
- [x] 点击进入阅读器

### 3.3 主阅读器页面
- [x] 单页模式（<img> 居中显示）
- [x] 双页模式（Flex 布局两图）
- [x] 长图模式（垂直滚动）
- [x] 缩放（滚轮 + 百分比控制）
- [x] 旋转（±90°）
- [x] 鼠标拖拽平移
- [x] 点击左/右 1/3 翻页
- [x] 进度条显示

---

## Phase 4：前端高级功能

### 4.1 预加载
- [x] Image 对象预加载前后 N 张
- [x] 预加载状态指示

### 4.2 快捷键
- [x] ← → / A D 翻页
- [x] Space 下一页
- [x] Home / End 首尾页
- [x] D 切换双页 / L 长图
- [x] R 旋转 / F11 全屏 / G 跳转

### 4.3 辅助功能
- [x] 缩略图总览（网格 + 点击跳转）
- [x] 跳转到页
- [x] 阅读历史页面
- [x] 标签管理
- [x] 设置面板（缩放/主题/背景色）
- [x] 主题切换（浅色/深色/护眼）
- [x] 全屏模式

### 4.4 状态持久化
- [x] 阅读进度自动保存到后端
- [x] 设置保存到后端
- [x] 后端 root_dir 持久化

---

## Phase 5：集成与部署

### 5.1 前后端集成
- [x] 前端构建产物由后端托管
- [x] 单端口启动

### 5.2 打包部署
- [x] pkg 打包配置
- [x] 启动脚本

---

## 技术约定

- **提交频率**：每完成一个小功能点即 commit + push
- **分支**：`web-rewrite`
- **后端端口**：5002（避免与 trans-app 的 5001 冲突）
- **数据库**：`backend/data/manhuaviewer.db`
- **缩略图缓存**：`backend/data/thumbnails/`
- **前端构建输出**：由后端 express.static 托管
