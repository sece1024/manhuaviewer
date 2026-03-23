# MangaViewer Web 版重写可行性分析报告

> 分析日期：2026-03-23
> 分析目标：评估将 PyQt5 桌面端漫画阅读器重写为前后端分离 Web 应用的可行性

---

## 一、现有桌面端功能盘点

### 1.1 核心功能

| 功能模块 | 具体能力 | 复杂度 |
|----------|----------|--------|
| **图片浏览** | 单页模式、双页模式、长图（滚动）模式 | 中 |
| **图片渲染** | 缩放（滚轮/百分比）、旋转（±90°）、鼠标拖拽平移 | 中 |
| **文件加载** | 打开本地文件夹、拖拽文件夹、支持 7 种图片格式 | 低 |
| **预加载** | 后台线程预加载前后 N 页，LRU 缓存（40 张） | 中 |
| **缩略图总览** | 异步加载缩略图网格，点击跳转 | 低 |
| **阅读进度** | 自动记住每文件夹的阅读页数，下次打开自动恢复 | 低 |
| **阅读历史** | 文件夹列表 + 搜索 + 标签筛选 + 按时间排序 | 低 |
| **标签管理** | 给文件夹打标签、标签颜色、全局标签管理、重命名/删除 | 低 |
| **最近文件** | 最近打开的 10 个文件夹快速访问 | 低 |
| **快捷键** | ← → 翻页、D 双页、L 长图、F11 全屏、G 跳转等 | 低 |
| **主题切换** | 浅色/深色/护眼 + 自定义背景色 | 低 |
| **全屏模式** | 隐藏菜单栏和工具栏的沉浸式全屏 | 低 |
| **跳转到页** | 输入页码直接跳转 | 低 |
| **右键菜单** | 翻页、模式切换、缩略图、旋转 | 低 |
| **响应式** | 窗口 resize 防抖后自动重新适配 | 低 |

### 1.2 模块结构

```
manhuaviewer/
├── viewer.py          ← 主窗口，~650 行，集成了所有 UI 和交互逻辑
├── data_store.py      ← 阅读历史 + 标签管理（JSON 持久化）
├── preload.py         ← LRU 缓存 + QThread 预加载
├── constants.py       ← 常量定义
├── styles.py          ← CSS 样式 + 主题
├── dialogs/
│   ├── thumbnails.py  ← 缩略图总览
│   ├── history.py     ← 阅读历史对话框
│   ├── settings.py    ← 设置对话框
│   ├── tags.py        ← 标签管理
│   └── jump.py        ← 跳转页码
```

---

## 二、参考项目架构（trans-app）

trans-app 采用了经典的**前后端分离**架构：

```
┌──────────────────────┐
│   Frontend (React)   │  ← Create React App，构建后由后端托管
└──────────┬───────────┘
           │ fetch API
┌──────────▼───────────┐
│  Backend (Express)   │  ← Node.js，提供 REST API
├──────────────────────┤
│  SQLite (Sequelize)  │  ← 数据持久化
│  本地文件系统         │  ← 文件存储
└──────────────────────┘
```

**关键设计决策**：
- 前端构建产物由后端托管（单端口部署）
- `pkg` 打包为单个可执行文件
- SQLite 存储元数据，磁盘存文件
- 无 Docker，适合树莓派等轻量部署

---

## 三、Web 版方案设计

### 3.1 整体架构

```
┌────────────────────────────────────────────┐
│           Frontend (React)                  │
│  ┌──────────┐ ┌──────────┐ ┌───────────┐   │
│  │ Reader   │ │ History  │ │ Settings  │   │
│  │ (Canvas) │ │ & Tags   │ │ & Theme   │   │
│  └──────────┘ └──────────┘ └───────────┘   │
│       │             │             │         │
│       └─────────────┼─────────────┘         │
│                     │ REST + WebSocket      │
└─────────────────────┼───────────────────────┘
                      │
┌─────────────────────▼───────────────────────┐
│          Backend (Express / Python)          │
│  ┌──────────┐ ┌──────────┐ ┌───────────┐    │
│  │ 文件扫描 │ │ 预加载   │ │ 进度/标签 │    │
│  │ API      │ │ 缓存     │ │ 持久化    │    │
│  └──────────┘ └──────────┘ └───────────┘    │
│       │              │             │         │
│       ▼              ▼             ▼         │
│   本地文件系统    内存缓存      SQLite/JSON   │
└─────────────────────────────────────────────┘
```

### 3.2 后端技术选型

**推荐方案：Express (Node.js)** — 与 trans-app 保持一致，复用经验。

| 组件 | 技术 | 说明 |
|------|------|------|
| 服务框架 | Express.js | 与 trans-app 一致，熟悉 |
| 图片服务 | 内置 `fs` + `sharp`（可选） | 提供图片流、缩略图生成 |
| 数据库 | SQLite (better-sqlite3) | 阅读历史、标签、用户设置 |
| 缓存 | 内存 Map + LRU | 预加载缩略图 |
| 部署 | pkg 打包 | 与 trans-app 一致 |

**备选方案：Python FastAPI** — 如果想保留 Python 生态。

| 组件 | 技术 | 说明 |
|------|------|------|
| 服务框架 | FastAPI + uvicorn | 异步高性能 |
| 图片服务 | Pillow / 内置 | 缩略图生成 |
| 数据库 | SQLite (sqlite3) | 同上 |
| 部署 | PyInstaller | 打包为单文件 |

### 3.3 后端 API 设计

```
GET  /api/folders                          # 获取可访问的漫画文件夹列表
GET  /api/folders/:id/images               # 获取文件夹图片列表（排序后）
GET  /api/images/:id                       # 获取原图（支持 Range 请求）
GET  /api/images/:id/thumbnail             # 获取缩略图
GET  /api/history                          # 获取阅读历史
POST /api/history                          # 保存阅读进度
DELETE /api/history/:folderId              # 删除历史记录
GET  /api/tags                             # 获取所有标签
POST /api/tags                             # 创建标签
POST /api/tags/assign                      # 给文件夹分配标签
DELETE /api/tags/:id                       # 删除标签
GET  /api/settings                         # 获取用户设置
PUT  /api/settings                         # 更新设置
```

### 3.4 前端组件设计

```
src/
├── App.js                    # 路由 + 全局主题
├── pages/
│   ├── Reader.js             # 主阅读器页面
│   ├── History.js            # 阅读历史
│   └── FolderList.js         # 文件夹列表
├── components/
│   ├── ImageViewer.js        # 图片渲染核心（Canvas）
│   ├── ThumbnailGrid.js      # 缩略图总览
│   ├── ProgressBar.js        # 阅读进度条
│   ├── Toolbar.js            # 工具栏
│   ├── TagManager.js         # 标签管理
│   ├── SettingsPanel.js      # 设置面板
│   └── ThemeProvider.js      # 主题上下文
├── hooks/
│   ├── usePreload.js         # 预加载逻辑
│   ├── useKeyboard.js        # 快捷键绑定
│   └── useProgress.js        # 进度同步
└── utils/
    └── api.js                # API 封装层
```

### 3.5 关键技术挑战

#### 1. 图片渲染模式

| 桌面端（Qt） | Web 实现方案 | 难度 |
|--------------|-------------|------|
| QGraphicsView + QPixmap | `<img>` + CSS transform 或 Canvas API | 低 |
| 缩放/旋转 | CSS `transform: scale() rotate()` | 低 |
| 长图滚动 | CSS `overflow-y: auto` + 固定宽度 | 低 |
| 双页模式 | Flex 布局两个 `<img>` | 低 |
| 拖拽平移 | `mousedown/move/up` 事件 + transform | 中 |

#### 2. 预加载

| 桌面端（Qt） | Web 实现方案 | 说明 |
|--------------|-------------|------|
| QThread + LRU Cache | `<link rel="preload">` + Service Worker Cache API + Image() 预加载 | 浏览器原生支持 |
| 后台线程加载 | `requestIdleCallback` + `IntersectionObserver` | 空闲时预加载 |

#### 3. 文件访问（核心差异）

这是桌面端 vs Web 端的**最大差异**：

| 方面 | 桌面端 | Web 端 |
|------|--------|--------|
| 文件读取 | 直接 `os.listdir()` | **必须通过后端 API** |
| 拖拽支持 | PyQt5 原生 | HTML5 Drag & Drop API（有限） |
| 文件系统访问 | 完全访问 | 后端代理访问服务器本地文件 |

**Web 方案**：用户不再直接"打开文件夹"，而是：
1. 管理员在后端配置可访问的根目录（漫画存储路径）
2. 前端从 API 获取可用文件夹列表
3. 前端通过 API 请求图片数据

#### 4. 桌面端特有功能的 Web 替代

| 桌面端功能 | Web 替代方案 |
|-----------|-------------|
| 全屏模式 | Fullscreen API (`element.requestFullscreen()`) |
| 系统托盘/快捷键 | 键盘事件监听（桌面端已有，Web 同理） |
| QSettings 本地存储 | `localStorage` / 后端数据库 |
| 窗口状态记忆 | `localStorage` 保存布局偏好 |
| 文件拖拽到窗口 | HTML5 Drop API（但只能获取文件，需上传） |

---

## 四、功能映射与迁移复杂度

| 功能 | 桌面端实现 | Web 迁移复杂度 | 备注 |
|------|-----------|---------------|------|
| 单页/双页浏览 | QGraphicsView | ⭐ 低 | HTML `<img>` 即可 |
| 缩放 | QTransform | ⭐ 低 | CSS transform |
| 旋转 | QTransform.rotate | ⭐ 低 | CSS rotate |
| 长图模式 | 垂直滚动条 | ⭐ 低 | CSS overflow |
| 预加载 | QThread + LRU | ⭐⭐ 中 | Image() + Cache API |
| 文件夹加载 | 本地文件系统 | ⭐⭐ 中 | 后端 API 扫描 |
| 阅读进度 | JSON 文件 | ⭐ 低 | SQLite + API |
| 阅读历史 | JSON 文件 | ⭐ 低 | SQLite + API |
| 标签管理 | JSON 文件 | ⭐ 低 | SQLite + API |
| 缩略图总览 | QThreadPool | ⭐⭐ 中 | 分页加载 + 后端缩略图 |
| 主题切换 | QSS | ⭐ 低 | CSS 变量 |
| 全屏 | Qt fullscreen | ⭐ 低 | Fullscreen API |
| 快捷键 | keyPressEvent | ⭐ 低 | addEventListener |
| 拖拽文件夹 | QDragEnterEvent | ⭐⭐⭐ 较高 | Web 受限，改为配置式 |
| 右键菜单 | QMenu | ⭐ 低 | 自定义 div |
| 跳转到页 | QDialog | ⭐ 低 | Modal 组件 |

**总体评估：⭐⭐ 中等复杂度**，没有硬性技术障碍。

---

## 五、与 trans-app 的技术对齐

| 维度 | trans-app | 建议 MangaViewer Web 版 |
|------|-----------|----------------------|
| 前端 | React + CRA | ✅ 保持一致 |
| 后端 | Express.js | ✅ 保持一致 |
| 数据库 | SQLite (Sequelize) | ✅ 保持一致 |
| 打包 | pkg | ✅ 保持一致 |
| 路由 | React Router | ✅ 保持一致 |
| 主题 | data-theme + CSS | ✅ 保持一致 |
| Toast | Context API | ✅ 保持一致 |
| 部署 | 单端口，前端由后端托管 | ✅ 保持一致 |

---

## 六、风险评估

### 6.1 低风险

- 技术栈成熟，React + Express 是主流方案
- 图片渲染在浏览器中天然支持
- 参考项目 trans-app 已验证了类似架构的可行性

### 6.2 中风险

- **大图片性能**：超大漫画页（>10MB）可能造成浏览器卡顿
  - 缓解：后端生成缩略图用于浏览，原图按需加载
- **多浏览器兼容性**：不同浏览器对图片渲染和 CSS transform 行为略有差异
  - 缓解：核心功能不依赖浏览器特性
- **文件系统权限**：后端需要读取本地文件系统
  - 缓解：类似 trans-app，运行在用户自己的机器上

### 6.3 高风险（无）

本项目没有无法克服的技术难题。

---

## 七、推荐实施路径

### Phase 1：后端 API（2-3 天）
- Express 项目脚手架（复用 trans-app 模板）
- SQLite 数据库设计（history, tags, settings 表）
- 文件夹扫描 + 图片列表 API
- 图片流式传输 API
- 缩略图生成（sharp 或 Pillow）

### Phase 2：前端阅读器（3-4 天）
- React 项目初始化（复用 trans-app 模板）
- 主阅读器组件（单页/双页/长图）
- 缩放、旋转、拖拽平移
- 预加载机制
- 快键键绑定
- 主题系统

### Phase 3：辅助功能（2-3 天）
- 阅读历史页面
- 标签管理
- 缩略图总览
- 设置面板
- 全屏模式

### Phase 4：集成与打磨（1-2 天）
- 前后端联调
- 响应式适配
- 打包部署（pkg）

**预估总工期：8-12 天**

---

## 八、结论

**可以重写。** 所有桌面端功能都有成熟的 Web 实现方案，没有硬性技术障碍。

主要工作量在于：
1. 将本地文件访问改为 API 代理（后端）
2. 将 PyQt5 渲染管线改为浏览器原生渲染（前端）
3. 将 JSON 文件持久化改为 SQLite（后端）

参考 trans-app 的架构，可以高效复用项目模板和设计模式，降低开发成本。
