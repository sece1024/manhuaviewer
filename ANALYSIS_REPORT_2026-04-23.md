# MangaViewer v2 — 全面优化分析报告

> **报告时间**: 2026-04-23 15:49:21  
> **分析范围**: 代码结构 · 文件命名 · UI/UX · 性能 · 测试完备性  
> **技术栈**: React 19 + Express + SQLite (better-sqlite3) + CRA

---

## 一、代码结构 🔴 高优先级

### 1. Reader.js 单文件过重（~550 行）

`Reader.js` 混合了数据加载、键盘处理、触摸手势、鼠标拖拽、图片渲染、缩略图面板、跳页对话框等多个关注点。

**建议拆分**：

```
frontend/src/
├── pages/Reader.js              # 顶层协调（<200行）
└── components/reader/
    ├── ReaderToolbar.js         # 顶部工具栏
    ├── ReaderCanvas.js          # 图片渲染 + 事件处理
    ├── ThumbnailPanel.js        # 缩略图浮层
    ├── HelpPanel.js             # 快捷键帮助
    └── hooks/
        ├── useReaderGestures.js # 触摸/鼠标手势
        └── useReaderKeyboard.js # 键盘快捷键
```

### 2. Settings.js 职责过多（标签管理 + 分类管理 + 系统设置混在一起）

**建议**：将标签/分类管理提取为独立子组件 `TagManager.js`、`CategoryManager.js`，Settings 只做设置路由。

### 3. N+1 查询问题（`GET /archives`）

当前实现：先查所有档案，再对每个档案单独执行一条标签查询。

```js
// ❌ 当前：N 个档案 = N+1 条 SQL
const result = archives.map(a => ({
  ...a,
  tags: tagStmt.all(a.id),  // 每次循环都执行一条 SQL
}));
```

**建议**：一次批量查询所有标签，然后在 JS 中 group by：

```js
// ✅ 建议：2 条 SQL 搞定
const allTags = db.prepare(`
  SELECT at2.archive_id, t.namespace, t.name, t.color
  FROM archive_tags at2 JOIN tags t ON t.id = at2.tag_id
  WHERE at2.archive_id IN (${archives.map(() => '?').join(',')})
`).all(...archives.map(a => a.id));

const tagMap = {};
for (const tag of allTags) {
  if (!tagMap[tag.archive_id]) tagMap[tag.archive_id] = [];
  tagMap[tag.archive_id].push(tag);
}
const result = archives.map(a => ({ ...a, tags: tagMap[a.id] || [] }));
```

### 4. 循环依赖：`settingsRoutes.js` require `../index.js`

```js
// settingsRoutes.js — 反模式
const { startAutoScanTimer } = require('../index');
```

`index.js` 是整个应用的入口，route 文件不应反向依赖它。

**建议**：将 `startAutoScanTimer` 提取到独立模块 `services/scanTimer.js`，由 `index.js` 和 `settingsRoutes.js` 共同 require。

### 5. `localStorage` 状态分散，无法同步

主题存在 `localStorage('theme')`，阅读偏好存在 `localStorage('readerFit')`/`localStorage('pageDirection')`，视图模式存在 `localStorage('viewMode')`。这些与后端 `settings` 表中的同名字段可能不同步。

**建议**：统一用 `/api/settings` 作为唯一来源（或封装一个 `useSettings()` hook 自动双向同步）。

### 6. 工具函数孤立，无法复用

`formatSize()` 定义在 `Library.js`，`CoverImage` fallback 组件定义在 `Library.js` 底部（与 `LazyImage.js` 功能重叠）。

**建议**：
- 新建 `frontend/src/utils/format.js`，放 `formatSize`、`formatDate` 等通用格式化函数
- 统一使用 `LazyImage` 替代 `CoverImage`，删除后者

---

## 二、文件命名与目录组织 🟠 中优先级

### 7. 组件层级不清晰

```
frontend/src/
├── components/
│   ├── LazyImage.js      # 通用组件 ✅
│   └── Toast.js          # 通用组件 ✅
└── pages/
    ├── Library.js        # 混入了 CoverImage、formatSize ❌
    └── Reader.js         # 混入了所有子面板 ❌
```

**建议结构**：

```
frontend/src/
├── components/           # 跨页面复用的原子组件
├── features/             # 按功能模块组织（reader/, library/）
├── hooks/                # 全局自定义 hook
├── utils/                # 纯函数工具（format.js, api.js）
└── pages/                # 轻量路由页（仅组装，不含业务逻辑）
```

### 8. SQL 别名过于简写

```sql
-- ❌ 难以阅读
INNER JOIN archive_tags at2 ON ...
INNER JOIN archive_tags at_inc ON ...
INNER JOIN archive_tags at_ex ON ...
```

**建议**：使用更具描述性的别名，如 `sidebar_tags`、`search_tags`、`exclude_tags`。

### 9. 魔法字符串 `'_other'`

在 `Library.js` 中用 `'_other'` 表示无命名空间的标签，这是一个魔法字符串，应提取为常量。

---

## 三、UI/UX 🟠 中优先级

### 10. 搜索无防抖，每键必触发请求

```js
// Library.js ❌
const handleSearch = (val) => {
  setSearch(val);
  loadArchives({ search: val, ... }); // 每次按键都发请求
};
```

**建议**：加 300ms debounce，或改为按下 Enter / 失焦时触发。

### 11. Library 无分页 / 虚拟列表

所有档案一次性加载返回，当档案数量 > 1000 时会明显变慢（前端渲染 + 封面图并发请求爆炸）。

**建议**：后端支持 `limit/offset` 分页，或前端使用虚拟列表（如 `react-window`）。

### 12. 阅读器翻页时缩放强制归 1

```js
const goPage = useCallback((newIndex) => {
  setScale(1);              // ❌ 翻页即重置缩放
  setTranslate({ x: 0, y: 0 });
  ...
}, [...]);
```

许多用户希望保持放大状态连续翻页（如放大看文字）。

**建议**：仅在用户主动切换 fitMode 或按重置按钮时归 1，翻页时保留。

### 13. 双页模式 RTL 顺序错误

RTL 漫画（日漫）双页显示时，正确布局应为右页 = 当前页（currentIndex），左页 = currentIndex+1。当前实现左右顺序相反：

```jsx
// ❌ 当前：左 = currentIndex，右 = currentIndex+1（LTR 顺序）
<img src={pages[currentIndex]?.url} ... />
<img src={pages[currentIndex + 1]?.url} ... />
```

**建议**：根据 `pageDirection` 条件调换两张图的渲染顺序。

### 14. 移动端菜单按钮 `display: 'none'` 无法触发

```jsx
// Reader.js — 永远不可见
<button style={{ display: 'none' }} id="menu-toggle">⋯</button>
```

CSS 中没有对应的 `@media` 规则将其显示。移动端底部工具栏（`.mobile-bottom-bar`）存在，但工具栏中的次要按钮在小屏无法隐藏/折叠，导致溢出或挤压。

**建议**：用 CSS media query 统一控制工具栏响应式，移除 `display: 'none'` 的硬编码。

### 15. `window.confirm` 删除确认体验割裂

```js
// Settings.js
if (!window.confirm('确定删除此标签？')) return;
```

系统原生对话框样式与应用完全不匹配（尤其深色主题下）。

**建议**：封装一个轻量的 `ConfirmDialog` 组件（或用 Toast 的"撤销"模式）。

### 16. 长图模式全量渲染无虚拟化

```jsx
// Reader.js — 渲染所有页面
{pages.map((p, i) => (
  <img key={p.id} src={p.url} loading="lazy" ... />
))}
```

对于 200+ 页的漫画，DOM 中同时存在 200 个 `<img>`，内存占用极高。`loading="lazy"` 仅延迟加载，不会卸载已加载图片。

**建议**：长图模式下使用 IntersectionObserver 实现图片卸载，或引入 `react-window`。

### 17. Reader 加载状态不友好

```jsx
// ❌ 纯文字 loading
if (!archive || pages.length === 0) {
  return <div className="empty-state">⏳ 加载中...</div>;
}
```

整个页面在加载期间显示空状态，没有进度指示。

**建议**：添加带骨架屏的 loading 状态，或至少用动画 spinner。

### 18. 标签颜色可及性不足

标签在卡片网格中以 8×8px 的彩色圆点显示，颜色语义完全依赖记忆，无文字辅助。同时护眼主题（eye-care）下部分标签颜色（硬编码 hex）对比度不够。

**建议**：标签至少在 hover/展开状态显示名称；护眼主题下对标签背景色做自动亮度调整。

---

## 四、性能 🟡 中优先级

### 19. Reader 预加载图片未持久化引用

```js
// Reader.js ❌ — 预加载图片立即被 GC
for (let i = start; i < end; i++) {
  const img = new Image();
  img.src = pages[i].url;  // 变量不存储，浏览器可能不缓存
}
```

**建议**：将预加载的 `Image` 对象存入 `useRef`，防止被垃圾回收：

```js
const preloadCacheRef = useRef({});
// 预加载时存入 cache，翻页时清理远端缓存
```

### 20. LazyImage 为每个图片创建独立 `IntersectionObserver`

每个 `LazyImage` 实例都 `new IntersectionObserver()`，库中有 100 张封面就有 100 个 observer。

**建议**：共享一个全局 `IntersectionObserver`（通过 context 或单例模式），大幅降低内存和 CPU 消耗。

### 21. `GET /archives` 缺少数量限制

路由没有 `LIMIT` 子句，返回全部结果。1 万条记录全部序列化、传输、渲染，是明显的性能瓶颈。

---

## 五、测试完备性 🔴 高优先级

### 22. 零测试覆盖

整个 Web 版（后端 + 前端）无任何测试。

**建议最低可行测试集**：

| 层次 | 工具 | 优先覆盖 |
|------|------|----------|
| 后端单元测试 | Jest + Supertest | `parseSearchSyntax`、`scanRoot`、archive CRUD API |
| 后端集成测试 | Supertest + 内存 DB | `/api/archives` 过滤/排序、`/api/settings` 更新 |
| 前端组件测试 | React Testing Library | `Library`（搜索/筛选渲染）、`Reader`（翻页逻辑） |
| E2E 测试 | Playwright | 首次配置根目录 → 扫描 → 打开阅读器 → 翻页完整流程 |

**推荐配置**（在 `backend/` 中）：

```bash
npm install -D jest @jest/globals supertest better-sqlite3
```

```json
// backend/package.json
"scripts": {
  "test": "jest",
  "test:watch": "jest --watch"
}
```

---

## 六、架构与安全 🟡 低优先级

### 23. CORS 完全开放

```js
app.use(cors()); // ❌ 允许任意来源
```

作为本地工具可接受，但若将来暴露到局域网，应限制来源：

```js
app.use(cors({ origin: ['http://localhost:3000', 'http://localhost:5002'] }));
```

### 24. `PUT /api/settings` 无输入校验

虽然有 `if (existing)` 保护，但 `value` 没有类型/范围校验：

```js
// 可以传 { auto_scan_interval: "rm -rf /" } 到数据库
stmt.run(String(value), key);
```

**建议**：按 key 做白名单校验（如 `auto_scan_interval` 必须是数字，`theme` 只允许 `light/dark/eye-care`）。

### 25. 路径遍历未显式防护

`GET /api/archives/:id/pages/:index` 中从 `pages` 表取出 `filepath` 后直接用于读取文件（见 `archiveRoutes.js`），未检查路径是否仍在 `root_dir` 范围内。

**建议**：用 `path.resolve` + 前缀检查防止路径穿越：

```js
const resolved = path.resolve(imagePath);
if (!resolved.startsWith(path.resolve(rootDir))) {
  return res.status(403).json({ error: 'Forbidden' });
}
```

### 26. `root_dir` 有两个独立 API 端点

`/api/config` 和 `/api/settings` 都可以读写 `root_dir`，行为不完全一致（`PUT /config` 用 `UPSERT`，`PUT /settings` 用 `UPDATE`）。前端有两处调用分别使用其中一个，容易混淆。

**建议**：统一使用 `/api/settings` 接口，废弃 `/api/config`（或将其改为 alias，内部调用 settings 逻辑）。

---

## 七、优化优先级汇总

| 优先级 | 编号 | 内容 | 预计影响 |
|--------|------|------|----------|
| 🔴 高 | #3 | N+1 SQL 查询 | 性能 |
| 🔴 高 | #10 | 搜索无防抖 | 性能 + UX |
| 🔴 高 | #22 | 零测试覆盖 | 质量 |
| 🔴 高 | #4 | 循环依赖 index.js | 架构 |
| 🟠 中 | #1 | Reader.js 拆分 | 可维护性 |
| 🟠 中 | #11 | Library 无分页 | 性能 + UX |
| 🟠 中 | #13 | 双页 RTL 顺序错误 | 正确性 |
| 🟠 中 | #14 | 移动端菜单失效 | UX |
| 🟠 中 | #16 | 长图无虚拟化 | 性能 |
| 🟠 中 | #5 | localStorage 与 API 不同步 | 一致性 |
| 🟡 低 | #6 | CoverImage/LazyImage 重复 | 代码整洁 |
| 🟡 低 | #15 | window.confirm 体验 | UX |
| 🟡 低 | #19 | 预加载图片被 GC | 性能 |
| 🟡 低 | #25 | 路径遍历防护 | 安全 |
| 🟡 低 | #26 | 双 config API | 一致性 |

---

## 八、最速可见效优化（1-2小时内可完成）

1. **`parseSearchSyntax` 加单元测试** — 逻辑明确，10 个测试用例完全覆盖
2. **修复双页 RTL 顺序** — 5 行代码，修复明显阅读体验 bug
3. **搜索加 debounce** — 引入 `useRef` + `setTimeout`，减少 90% 无效请求
4. **N+1 查询改批量** — 20 行 SQL 改动，库大时性能提升数倍
5. **`startAutoScanTimer` 解除循环依赖** — 新建 `services/scanTimer.js`，10 行重构
