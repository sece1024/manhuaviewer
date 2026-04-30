# MangaViewer v2 — 深度安全·性能·质量审计报告

> **报告时间**: 2026-04-30  
> **分析方法**: 全量代码逐行审计  
> **范围**: 后端 19 个源文件 + 前端 10 个源文件  
> **前序报告**: ANALYSIS_REPORT_2026-04-23.md（架构优化）, ANALYSIS_REPORT_2026-04-30.md（代码清理）

---

## 一、安全漏洞

### 🔴 CRITICAL — 7z 解压路径穿越

`services/archiveService.js:184-188` — `extractFile7z` 将 DB 中的 `filepath` 直接拼接到临时目录：

```js
const targetPath = path.join(tmpDir, entryPath);  // entryPath 来自 DB
return fs.readFileSync(targetPath);                // 可能读取 tmpDir 之外的文件
```

恶意压缩包可包含 `../../etc/passwd` 等条目，`path.join` 会解析为上级目录路径。

**修复**: 验证 `path.resolve(targetPath)` 是否在 `tmpDir` 内：

```js
const resolved = path.resolve(tmpDir, entryPath);
if (!resolved.startsWith(path.resolve(tmpDir))) {
  throw new Error('非法路径');
}
```

### 🔴 CRITICAL — 备份恢复无输入校验

`routes/backupRoutes.js:48-136` — `POST /api/restore` 接受任意 JSON 并直接写入数据库。攻击者可：

- 注入 `root_dir` 指向敏感目录
- 插入伪造的档案记录
- 覆盖所有设置

**修复**: 恢复前校验 `root_dir` 路径合法性，限制可恢复的字段。

### 🟠 HIGH — 删除档案未清理磁盘文件

`routes/archiveRoutes.js:353-358` — `DELETE /archives/:id` 仅删除数据库记录，但不会删除：

- `DATA_DIR/thumbnails/{id}_cover.jpg`（封面缩略图）
- `DATA_DIR/thumbnails/pages/{id}_*.jpg`（页面缩略图）

导致磁盘空间泄漏。

**修复**: 删除前清理缩略图文件。

### 🟠 HIGH — 错误消息泄漏内部路径

`middleware/errorHandler.js:7` — `err.message` 直接返回给客户端，可能泄露数据库路径、文件系统结构等敏感信息。

```js
res.status(status).json({ error: err.message });  // 泄漏内部路径
```

**修复**: 生产环境返回通用错误信息，仅日志记录详细错误。

### 🟠 HIGH — `parseInt` 无 NaN 检查

多处 `parseInt(req.params.id)` 未检查 NaN（`archiveRoutes.js:141,183,254,297,355`、`historyRoutes.js:54`）。当参数为非数字时，SQLite 将 NaN 视为 NULL，可能导致意外查询结果。

### 🟡 MEDIUM — 无安全头

`index.js` 未使用 `helmet` 中间件，缺少 `Content-Security-Policy`、`X-Frame-Options`、`X-Content-Type-Options` 等安全头。

### 🟡 MEDIUM — 无速率限制

`/api/scan`（重 I/O）、`/api/backup`（导出全库）、`/api/restore`（覆盖全库）均无速率限制，可被恶意调用导致 DoS。

---

## 二、内存泄漏

### 🔴 CRITICAL — Reader.js 未清理 setTimeout

`pages/Reader.js:26,40-41` — `overlayTimer` 在 `useEffect` 中设置但无 cleanup：

```js
useEffect(() => {
  // 没有 return () => clearTimeout(overlayTimer.current);
  overlayTimer.current = setTimeout(() => setOverlayText(''), 2000);
}, []);
```

用户翻页离开后，定时器仍会触发 `setOverlayText('')`（组件已卸载）。

### 🔴 CRITICAL — Reader.js 未清理 saveTimer

`pages/Reader.js:33,67-68` — `saveTimerRef` 同样无 cleanup。组件卸载后仍可能触发 `api.saveHistory()`。

### 🔴 CRITICAL — Library.js 未清理 searchDebounce

`pages/Library.js:20,68-71` — `searchDebounceRef` 超时未清理。组件卸载后调用 `loadArchives` 触发 `setArchives`（已卸载组件）。

---

## 三、错误处理缺陷

### 🟠 HIGH — 吞掉错误的空 catch 块

| 位置 | 影响 |
|------|------|
| `tagRoutes.js:98` | 标签分配失败静默丢弃 |
| `categoryRoutes.js:64` | 分类分配失败静默丢弃 |
| `scanService.js:105` | 页面写入失败静默丢弃 |
| `scanService.js:56,108` | 封面生成失败无日志 |

**修复**: 至少记录 `logger.warn`，不要用空 `catch {}`。

### 🟠 HIGH — GET /archives 无 try/catch

`routes/archiveRoutes.js:14-136` — 最复杂的路由没有 try/catch。若 DB schema 不匹配（如升级后），将抛出未捕获异常。

### 🟠 HIGH — Reader 无错误状态区分

`pages/Reader.js:306-313` — 初始状态和 API 失败均显示"加载中..."，用户永远看不到错误信息。

### 🟠 HIGH — 前端无 Error Boundary

`App.js` 未包裹 `ErrorBoundary`。任何组件崩溃（如 `pages[currentIndex]?.url` 为 undefined）将导致整个 UI 白屏。

### 🟡 MEDIUM — api.js 吞掉 JSON 解析错误

`utils/api.js:9` — `.catch(() => ({}))` 丢失了原始错误信息（如 500 返回的 HTML body）。

---

## 四、性能瓶颈

### 🟠 HIGH — History 列表 N+1 查询

`routes/historyRoutes.js:19-28` — 每条历史记录单独查询标签。500 条记录 = 501 条 SQL。

对比 `archiveRoutes.js:114-127` 已正确使用批量查询。

**修复**: 复用 archiveRoutes 的批量查询模式。

### 🟠 HIGH — 扫描时重复解析压缩包

`services/scanService.js:69,95` — 每个压缩包调用两次 `getImageList()`：第一次取页数，第二次插入 pages 表。ZIP 文件被完整解析两次。

**修复**: 复用第一次结果。

### 🟡 MEDIUM — 同步文件 I/O 阻塞事件循环

| 位置 | 操作 |
|------|------|
| `archiveRoutes.js:215-217` | `fs.readdirSync()` 每次请求 |
| `archiveRoutes.js:319` | `fs.readFileSync()` 读取整张图片 |
| `opdsRoutes.js:126` | `fs.readdirSync()` |

大目录（数千张图片）会阻塞 Node 事件循环。

### 🟡 MEDIUM — 缩略图缓存竞态条件

`archiveRoutes.js:304-305,332` — `existsSync` 检查 + 生成缩略图不是原子操作。并发请求可能重复生成（浪费 sharp 的 CPU 开销）。

### 🟡 MEDIUM — Reader 每次渲染重建 getImgStyle

`pages/Reader.js:291-304` — `getImgStyle()` 每次渲染返回新对象，导致每个 `<img>` 的 style diff 失效。应包裹 `useMemo`。

### 🟡 MEDIUM — Reader 键盘事件监听器频繁重建

`pages/Reader.js:120-167` — `useEffect` 有 11 个依赖，每次状态变化都移除并重新注册事件监听器。

---

## 五、可访问性（Accessibility）

### 🟠 HIGH — 所有对话框缺少 ARIA 属性

| 组件 | 缺失 |
|------|------|
| `Reader.js:475-496` 缩略图面板 | `role="dialog"`, `aria-modal`, 焦点陷阱 |
| `Reader.js:499-514` 跳页对话框 | 同上 |
| `Reader.js:517-558` 帮助对话框 | 同上 |

键盘用户可以 Tab 到对话框后面的元素。

### 🟠 HIGH — 搜索和按钮缺少 ARIA 标签

| 位置 | 问题 |
|------|------|
| `App.js:45-49` 主题选择器 | 无 `aria-label` |
| `Library.js:178-184` 搜索输入框 | 仅 placeholder，无 label |
| `Reader.js:319-321` 工具栏按钮 | 纯符号"←""‹""›"，无 aria-label |
| `History.js:100-105` 删除按钮 | 无 aria-label 区分不同条目 |

### 🟡 MEDIUM — 无跳过导航链接

`App.js:53` — `<main>` 无 `id`，无 skip-to-content 机制。键盘用户必须 Tab 遍历整个侧边栏。

### 🟡 MEDIUM — 无 prefers-reduced-motion 支持

`index.css:226-232,375` — 动画（toast、图片缩放、过渡）在用户请求减少动画时仍然执行。

### 🟡 MEDIUM — 无 prefers-color-scheme 自动检测

`App.js:10` — 主题默认从 localStorage 读取，首次访问者无法自动匹配系统主题。

---

## 六、React 代码质量

### 🟠 HIGH — 闭包陈旧导致数据不一致

`pages/Library.js:30-37,40-41,66-72` — `loadArchives` 和 `handleSearch` 存在闭包陈旧问题：

- `handleSearch` 捕获旧的 `sortBy`，搜索时使用过期的排序方式
- `useEffect` 依赖数组被 `eslint-disable-line` 强制抑制

### 🟡 MEDIUM — useState/useEffect 依赖问题

| 位置 | 问题 |
|------|------|
| `Library.js:41` | `eslint-disable-line` 抑制缺失依赖警告 |
| `Library.js:72` | 同上 |
| `Reader.js` 多处 | 键盘/触摸处理的 11 个依赖导致频繁重建 |

### 🟡 MEDIUM — History filtered 无 useMemo

`pages/History.js:45-49` — `filtered` 在每次渲染时重新计算。大历史列表下每次按键都触发全量过滤。

---

## 七、代码重复

| 类型 | 位置 |
|------|------|
| MIME 类型映射 | `archiveRoutes.js:282` 与 `opdsRoutes.js:147` 重复定义 |
| `SUPPORTED_IMG` 常量 | `scanService.js:22` 与 `archiveService.js` 的 `IMAGE_EXTS` 重复 |
| `DATA_DIR` 引用 | `archiveRoutes.js:187,299` 在 handler 内 `require`，应顶层导入 |
| `db.prepare` 重复 | `settingsRoutes.js:24` 在循环内重复 prepare 同一 SQL |

---

## 八、杂项

| # | 位置 | 问题 |
|---|------|------|
| 1 | `index.js:31-33` | `frontend/build` 不存在时 `res.sendFile` 抛异常 |
| 2 | `scanService.js:22` | 本地 `SUPPORTED_IMG` 与 `archiveService.IMAGE_EXTS` 可能不同步 |
| 3 | `Library.js:287` | `NS_OTHER` 常量定义在使用之后（依赖提升） |
| 4 | `Settings.js:187-202` | `view_mode`/`sort_by` 设置存入 DB 但 Library 从 localStorage 读取 |
| 5 | `api.js:1` | `/api` base path 硬编码，无法适配不同部署环境 |
| 6 | `Settings.js:297-304` | 大量内联样式，应提取为 CSS 类 |
| 7 | `opdsRoutes.js:64` | `baseUrl` 依赖 `req.get('host')`，可被 Host header 注入 |

---

## 九、优先级汇总

| 优先级 | 编号 | 内容 | 类别 |
|--------|------|------|------|
| 🔴 CRITICAL | 1 | 7z 解压路径穿越 | 安全 |
| 🔴 CRITICAL | 2 | 备份恢复无校验 | 安全 |
| 🔴 CRITICAL | 3-5 | Reader/Library 未清理 setTimeout | 内存泄漏 |
| 🟠 HIGH | 6 | 删除档案未清理缩略图 | 资源泄漏 |
| 🟠 HIGH | 7 | 错误消息泄漏内部路径 | 安全 |
| 🟠 HIGH | 8 | 空 catch 块吞掉错误 | 质量 |
| 🟠 HIGH | 9 | GET /archives 无 try/catch | 健壮性 |
| 🟠 HIGH | 10 | 前端无 Error Boundary | 健壮性 |
| 🟠 HIGH | 11 | History N+1 查询 | 性能 |
| 🟠 HIGH | 12 | 扫描重复解析压缩包 | 性能 |
| 🟠 HIGH | 13 | 对话框缺少 ARIA + 焦点陷阱 | 无障碍 |
| 🟠 HIGH | 14 | 按钮/输入框缺少 ARIA 标签 | 无障碍 |
| 🟠 HIGH | 15 | 闭包陈旧导致搜索排序错误 | Bug |
| 🟡 MEDIUM | 16 | 无安全头 (helmet) | 安全 |
| 🟡 MEDIUM | 17 | 无速率限制 | 安全 |
| 🟡 MEDIUM | 18 | 同步文件 I/O 阻塞 | 性能 |
| 🟡 MEDIUM | 19 | 缩略图竞态条件 | 性能 |
| 🟡 MEDIUM | 20 | Reader getImgStyle 未 memo | 性能 |
| 🟡 MEDIUM | 21 | History filtered 未 memo | 性能 |
| 🟡 MEDIUM | 22 | 无 prefers-reduced-motion | 无障碍 |
| 🟡 MEDIUM | 23 | 无 Error Boundary | 健壮性 |

---

## 十、建议修复路线

### Phase 1 — 紧急修复（1-2 天）

1. **修复 7z 路径穿越** — 添加路径前缀校验（5 行代码）
2. **修复内存泄漏** — 在 useEffect return 中 clearTimeout（10 行代码）
3. **修复空 catch 块** — 添加 logger.warn（10 处，每处 1 行）
4. **修复 GET /archives 无 try/catch** — 包裹 try/catch（1 处）
5. **修复 History N+1** — 复用批量查询模式（20 行 SQL）

### Phase 2 — 安全加固（2-3 天）

6. **备份恢复输入校验** — 校验 root_dir 路径合法性
7. **错误消息脱敏** — 生产环境返回通用错误
8. **删除档案清理缩略图** — 3 行文件删除代码
9. **添加 helmet 中间件** — `npm i helmet` + 3 行配置
10. **添加 parseInt NaN 检查** — 5 处，每处加 `isNaN` 判断

### Phase 3 — 质量提升（1 周）

11. **前端 Error Boundary** — 新建 `components/ErrorBoundary.js`
12. **对话框 ARIA 改造** — 添加 `role="dialog"`, `aria-modal`, 焦点陷阱
13. **Reader 键盘监听优化** — 用 `useCallback` 减少依赖
14. **Reader getImgStyle useMemo** — 包裹 useMemo
15. **同步 I/O 改异步** — `readdirSync` → `readdir`
