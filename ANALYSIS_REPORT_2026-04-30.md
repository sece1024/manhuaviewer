# MangaViewer v2 — 代码架构与清理分析报告

> **报告时间**: 2026-04-30  
> **分析范围**: 代码架构 · 使用方式 · 无用文件/代码清理  
> **技术栈**: React 19 + Express + SQLite (better-sqlite3) + CRA  
> **对比基准**: ANALYSIS_REPORT_2026-04-23.md

---

## 一、上次报告（4/23）修复进度

| 编号 | 问题 | 状态 |
|------|------|------|
| #3 | N+1 SQL 查询 | ✅ 已修复 — `archiveRoutes.js` 已改为批量查询 |
| #4 | `scanTimer.js` 循环依赖 | ✅ 已修复 — 已提取为独立模块 |
| #22 | 零测试覆盖 | ✅ 部分修复 — 新增 `searchParser.test.js`（13 个用例） |

---

## 二、代码架构改进点

### 2.1 Reader.js 仍然过重（561 行）🔴 高

`frontend/src/pages/Reader.js` 混合了键盘处理、触摸手势、鼠标拖拽、图片渲染、缩略图面板、跳页对话框、帮助面板等多个关注点。建议拆分为：

```
frontend/src/
├── pages/Reader.js                    # 顶层协调（<200行）
└── components/reader/
    ├── ReaderToolbar.js               # 顶部工具栏
    ├── ReaderCanvas.js                # 图片渲染 + 事件处理
    ├── ThumbnailPanel.js              # 缩略图浮层
    ├── HelpPanel.js                   # 快捷键帮助
    └── hooks/
        ├── useReaderGestures.js       # 触摸/鼠标手势
        └── useReaderKeyboard.js       # 键盘快捷键
```

### 2.2 Settings.js 职责过多（312 行）🟠 中

标签管理、分类管理、系统设置、备份恢复、统计信息全部在 Settings.js 中。建议拆分为：

```
frontend/src/pages/Settings.js         # 设置路由页（<150行）
frontend/src/components/TagManager.js  # 标签 CRUD
frontend/src/components/CategoryManager.js  # 分类 CRUD
```

### 2.3 前端无测试覆盖 🔴 高

前端 `src/` 目录中没有任何测试文件（0 个 `*.test.js`）。建议使用 React Testing Library 添加核心组件测试：

| 组件 | 优先测试项 |
|------|-----------|
| Library | 搜索/筛选渲染、视图切换 |
| Reader | 翻页逻辑、键盘快捷键 |
| History | 搜索过滤、删除操作 |

### 2.4 localStorage 与后端 settings 不同步 🟠 中

主题存在 `localStorage('theme')`，阅读偏好存在 `localStorage('readerFit')` / `localStorage('pageDirection')`，视图模式存在 `localStorage('viewMode')`，与后端 `settings` 表中同名字段可能不同步。

**建议**：统一用 `/api/settings` 作为唯一来源，或封装 `useSettings()` hook 自动双向同步。

### 2.5 `GET /api/archives` 无分页 🟠 中

路由没有 `LIMIT` 子句，返回全部结果。当档案数量 >1000 时会明显变慢（前端渲染 + 封面图并发请求爆炸）。

**建议**：后端支持 `limit/offset` 分页，前端支持无限滚动。

### 2.6 CORS 完全开放 🟡 低

```js
app.use(cors()); // 允许任意来源
```

作为本地工具可接受，但若暴露到局域网，应限制来源。

### 2.7 `PUT /api/settings` 无输入校验 🟡 低

`value` 没有类型/范围校验，建议按 key 做白名单校验。

### 2.8 路径遍历未显式防护 🟡 低

`GET /api/archives/:id/pages/:index` 中从 `pages` 表取出 `filepath` 后直接用于读取文件，未检查路径是否仍在 `root_dir` 范围内。

### 2.9 `root_dir` 有两个独立 API 端点 🟡 低

`/api/config` 和 `/api/settings` 都可以读写 `root_dir`，行为不完全一致。建议统一使用 `/api/settings`。

### 2.10 无 CI/CD 流水线 🟡 低

`.github/` 目录下没有 `workflows/` 配置，无自动化测试、lint、构建验证。

---

## 三、使用方式改进点

### 3.1 双页模式 RTL 顺序 🟠 中

RTL 漫画（日漫）双页显示时，正确布局应为右页 = 当前页，左页 = 下一页。需检查当前实现是否已修正。

### 3.2 移动端菜单按钮硬编码隐藏 🟠 中

```jsx
<button style={{ display: 'none' }} id="menu-toggle">⋯</button>
```

CSS 中无对应 `@media` 规则将其显示。建议用 CSS media query 统一控制工具栏响应式。

### 3.3 `window.confirm` 删除确认体验割裂 🟡 低

系统原生对话框样式与应用不匹配，建议封装 `ConfirmDialog` 组件。

### 3.4 阅读器翻页重置缩放 🟠 中

翻页时 `setScale(1)` 会重置缩放状态，用户放大看文字后翻页会丢失放大状态。

### 3.5 长图模式全量渲染 🟠 中

200+ 页漫画在长图模式下 DOM 中同时存在 200 个 `<img>`，内存占用极高。建议使用虚拟滚动或 IntersectionObserver 卸载不可见图片。

---

## 四、无用文件与代码清理

### 4.1 无用文件

| # | 文件 | 问题 | 建议 |
|---|------|------|------|
| 1 | `.venv/` (140 MB) | Python 虚拟环境，本项目是纯 Node.js | 删除 |
| 2 | `ANALYSIS_REPORT_2026-04-23.md` | 旧分析报告，未被 git 跟踪 | 移至 `docs/` 或删除 |
| 3 | `ANALYSIS_REPORT_2026-04-30.md` | 本报告 | 移至 `docs/` |
| 4 | `.github/copilot-instructions.md` | 内容已过时（仍写"零测试"） | 更新或删除 |

### 4.2 无用代码

| # | 文件 | 问题 | 建议 |
|---|------|------|------|
| 1 | `frontend/src/utils/format.js` — `formatDate()` | 未被任何组件导入使用 | 删除或标记为备用 |
| 2 | `frontend/src/pages/Settings.js` 中的 `formatSize()` | 与 `utils/format.js` 中同名函数重复 | 改为 import |
| 3 | `frontend/src/utils/api.js` 中大量未调用方法 | 见下方详细列表 | 保留供未来使用或清理 |
| 4 | `frontend/src/pages/History.js` 中的 `CoverImage` 组件 | 与 `LazyImage` 功能重叠 | 改用 `LazyImage` |

### 4.3 api.js 未使用的 API 方法

以下方法在 `frontend/src/utils/api.js` 中定义但从未被任何 UI 组件调用：

| 方法 | 用途 | 建议 |
|------|------|------|
| `getArchive(id)` | 获取单个档案详情 | 保留 |
| `pageUrl()` | 构造页面 URL | 保留（可能被 OPDS 使用） |
| `pageThumbUrl()` | 构造缩略图 URL | 保留 |
| `coverUrl()` | 构造封面 URL | 保留 |
| `deleteArchive(id)` | 删除档案 | 保留（Settings 未集成但功能需要） |
| `getNamespaces()` | 获取命名空间列表 | 保留 |
| `updateTag()` | 更新标签 | 保留 |
| `assignTag()` | 分配标签 | 保留 |
| `removeTag()` | 移除标签 | 保留 |
| `updateCategory()` | 更新分类 | 保留 |
| `assignCategory()` | 分配分类 | 保留 |
| `removeCategory()` | 移除分类 | 保留 |

> **结论**：这些方法虽未被调用，但属于 CRUD API 的完整封装，建议保留。

### 4.4 根 .gitignore 包含无关条目

根目录 `.gitignore` 包含大量 Python 相关条目（`__pycache__`、`*.egg`、PyInstaller、`uv.lock`、`.python-version`），但本项目是纯 Node.js 项目。

**建议**：清理为仅包含 Node.js 相关忽略规则。

---

## 五、优化优先级汇总

| 优先级 | 编号 | 内容 | 预计影响 |
|--------|------|------|----------|
| 🔴 高 | 2.1 | Reader.js 拆分 | 可维护性 |
| 🔴 高 | 2.3 | 前端测试覆盖 | 质量 |
| 🟠 中 | 2.2 | Settings.js 拆分 | 可维护性 |
| 🟠 中 | 2.4 | localStorage/Settings 同步 | 一致性 |
| 🟠 中 | 2.5 | Library 无分页 | 性能+UX |
| 🟠 中 | 3.1 | 双页 RTL 顺序 | 正确性 |
| 🟠 中 | 3.2 | 移动端菜单失效 | UX |
| 🟠 中 | 3.4 | 翻页重置缩放 | UX |
| 🟠 中 | 3.5 | 长图无虚拟化 | 性能 |
| 🟡 低 | 2.6 | CORS 限制 | 安全 |
| 🟡 低 | 2.7 | settings 输入校验 | 安全 |
| 🟡 低 | 2.8 | 路径遍历防护 | 安全 |
| 🟡 低 | 4.1 | 删除 .venv (140MB) | 磁盘 |
| 🟡 低 | 4.4 | 清理 .gitignore | 代码整洁 |

---

## 六、最速可见效改进（1-2 小时）

1. **删除 `.venv/` 目录** — 释放 140MB，本项目无 Python 依赖
2. **Settings.js 中 `formatSize` 改为 import** — 消除代码重复
3. **清理根 `.gitignore` Python 条目** — 代码整洁
4. **更新 `.github/copilot-instructions.md`** — 同步"已有测试"的事实
5. **History.js 改用 LazyImage** — 统一组件复用
