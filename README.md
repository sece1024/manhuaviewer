# MangaViewer v3

一个基于 Tauri 2.0 + React 的跨平台漫画阅读管理系统，支持文件夹和压缩包（ZIP/CBZ/RAR/CBR），提供原生桌面应用体验。

## ✨ 功能特性

- ↩️ **误删可撤销** — 「从库中移除」删的只是库里的记录（磁盘源文件不动），删除后提示条上可一键还原标签/分类/书签/阅读进度；撤销窗口 10 分钟
- 📚 **漫画库** — 封面卡片网格 / 列表视图切换，按标签、分类、名称、阅读状态（未读/在读/已读完）筛选；顶部「继续阅读」横条一键续读
- 📖 **阅读器** — 单页/双页/长图模式，RTL/LTR 翻页方向，适应高度/宽度/原始大小
- 📦 **压缩包支持** — ZIP/CBZ/RAR/CBR/7Z 直接浏览，无需解压
- 📦 **CBZ 归档** — 将漫画文件夹打包为 CBZ 格式
- 🗜️ **批量转 CBZ** — 一键将 7z/RAR/CBR/ZIP 批量转为 CBZ（可只转选中项，成功后清理原文件，支持进度/取消）
- 🏷️ **命名空间标签** — 支持 `artist:name`、`series:name` 格式；书库「整理」模式可键盘逐本打标签（输入即筛选/新建，Enter 打标并下一本，Esc 跳过）
- 📂 **分类系统** — 动态/静态分类，支持置顶
- 🔍 **搜索过滤** — 按名称模糊搜索，标签过滤侧栏
- 📊 **阅读历史** — 自动保存进度，断点续读；长图模式滚动即记进度；末页读完弹面板（合并组内自动续下一话，同目录同系列可直接进入下一卷）
- 🔖 **书签与封面** — 任意页书签（缩略图面板 ⭐ 快捷跳转）；封面支持首页/指定页/远程 URL/Bangumi 搜索
- 🎨 **主题系统** — 浅色/深色/护眼三套主题（首帧预置，无闪烁）
- 📱 **移动端适配** — 响应式布局，触摸手势（缩放/双击/滑动翻页）；侧边栏可一键收起成图标窄栏（iPad 上把宽度还给内容，阅读器整屏沉浸）
- ⌨️ **快捷键** — 完整键盘操作；支持游戏手柄/USB 翻页器与鼠标侧键翻页
- 🏠 **局域网模式** — 设置内开启后手机/平板浏览器与 OPDS 阅读器均可访问
- 🔄 **跨机同步** — 从局域网内另一台 MangaViewer 整库拉取档案（含标签/分类/进度），支持断点续传与任务取消；同步收尾时把本机标签**镜像回远端**（启动同步的机器是标签权威方，本机删掉的标签远端也删）
- 🔄 **增量扫描** — 可记住多个扫描目录（内置盘 + 移动硬盘），清理只作用于该目录范围；书库与设置页都能直接发起，自动跳过未变化档案
- ⏳ **长任务可见** — 扫描 / 跨机同步 / 批量转 CBZ 的进度与取消跟着你走：设置页就地显示在各自设置项旁，其它页面常驻右下角，切页不丢状态
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
| ↑ / ↓ | 翻页（长图模式下不生效） |
| Space | 下一页 |
| D | 切换双页模式 |
| L | 切换长图模式 |
| R / Shift+R | 旋转（顺时针/逆时针） |
| T | 缩略图总览 |
| G | 跳转到指定页 |
| W | 循环切换适应模式 |
| Home / End | 第一页 / 最后一页 |
| F1 | 帮助面板（快捷键列表） |
| F11 | 全屏模式 |
| Esc | 关闭弹出面板 |

## 👆 触摸手势（iPad / 手机浏览器）

局域网模式下用 iPad 等设备浏览器打开 `http://<电脑IP>:5002/` 即可阅读，手势如下（原生非被动监听，浏览器不会抢走手势）：

| 手势 | 功能 |
|------|------|
| 单指左右滑动 | 翻页；方向跟随「翻页方向」设置（日漫/右翻：右滑 = 下一页） |
| 单击左/右区域 | 翻页（与滑动方向一致） |
| 双击 | 缩放 / 还原（2.5×） |
| 双指捏合 | 连续缩放 |
| 放大后单指拖动 | 平移查看 |
| 长图模式 | 纵向滚动交给浏览器，不触发滑动翻页 |

书库里的卡片操作在触屏上同样可用：封面右下角的「⋯」**常显**（无需悬停），点它或**长按卡片**都会弹出操作面板（标签 / 分类 / 重命名 / 移除）。桌面端这四个动作仍是悬停显形的小图标——触屏没有 hover，那套按钮在那里既看不见也点不到。

> 前端产物由后端内嵌：debug 构建（`pnpm tauri dev`）运行时从 `frontend/build` 读盘，改完前端只需重新 `pnpm --filter manhuaviewer-frontend build` 再刷新 iPad；release 安装包需要重新 `pnpm tauri build` 才会带上新前端。

## 🗂️ 项目结构

```
src-tauri/                          # Tauri + Rust 后端
├── Cargo.toml
├── tauri.conf.json
├── capabilities/default.json       # Tauri 权限配置
└── src/
    ├── main.rs                     # 入口（Axum 服务 + 单实例锁 + 启动失败提示）
    ├── logging.rs                  # 按天滚动文件日志 + panic hook
    ├── db/                         # rusqlite 封装（r2d2 连接池）
    │   ├── mod.rs                  # Database 结构体 + 连接池 + 公共工具
    │   ├── archives.rs             # 按域拆分的 SQL（另有 tags/categories/history/settings/bookmarks/backup）
    │   ├── schema.rs               # 幂等建表 SQL
    │   └── migrations.rs           # 旧版数据表迁移 + 列补充
    ├── routes/                     # Axum 路由（archives/tags/categories/history/settings/scan/convert/sync/metadata/update/opds + auth 局域网鉴权）
    └── services/                   # 业务逻辑（archive/scanner/thumbnail/cbz/backup/cleanup/cache_budget/page_cache/metadata/fs_ext）

frontend/
├── src/
│   ├── App.js                      # 路由 + 主题 + ErrorBoundary
│   ├── index.js                    # 入口
│   ├── index.css                   # 全局样式（三套主题/响应式）
│   ├── components/                 # Toast/Modal/LazyImage/ErrorBoundary/TagPicker/CategoryPicker/ConfirmDialog/ThumbnailPanel/ReaderVirtualList/CbzConvertPanel/JobIndicator
│   ├── hooks/                      # useSettings/useTags/useReaderKeyboard/useGamepad/useScan/useSync/useCbzConvert/useLibrarySession/useJobs 等 12 个
│   ├── pages/                      # Library/Reader/History/Settings
│   ├── utils/                      # api.js（唯一 API 客户端）/format.js/listReconcile.js/spreadFit.js
│   └── __tests__/                  # 测试文件
└── package.json
```

## 📡 API 文档

所有接口均挂在 `/api` 前缀下（OPDS 接口挂在 `/opds`）：

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/archives` | GET | 档案列表（支持 search, tag, category_id, group_id, sort/sort_by, order/sort_order, page, limit, read=unread/in_progress/finished 阅读状态, tag_state=untagged/tagged 标签状态, added_from/added_to 按添加日期过滤，YYYY-MM-DD，from 含 to 不含） |
| `/api/archives/added-tree` | GET | 按添加日期的年/月聚合（侧栏"日期"树；本机时区分桶，年降序、月升序） |
| `/api/archives/:id` | GET | 档案详情 |
| `/api/archives/:id` | DELETE | 删除档案；响应带一次性 `undo_token`，配合下一条可在 10 分钟内撤销 |
| `/api/archives/:id/title` | PUT | 重命名档案 |
| `/api/archives/:id/cover` | GET | 封面缩略图 |
| `/api/archives/:id/pages` | GET | 页面列表 |
| `/api/archives/:id/pages/:page` | GET | 单页图片（支持 ETag/Last-Modified 缓存） |
| `/api/archives/:id/pages/:page/thumb` | GET | 单页缩略图 |
| `/api/archives/:id/siblings` | GET | 同目录的其他档案（含自己），供阅读器末页判定「下一卷」 |
| `/api/archives/:id/cover` | PUT | 手动封面（page_index=null 恢复默认） |
| `/api/archives/:id/cover-url` | PUT | 远程封面 URL（null 清除） |
| `/api/archives/:id/bookmarks` | GET/POST | 书签列表 / 添加书签 |
| `/api/archives/:id/bookmarks/:page_index` | DELETE | 移除书签 |
| `/api/metadata/search` | GET | Bangumi 元数据搜索（?q=） |
| `/api/update/check` | GET | 检查 GitHub Releases 更新 |
| `/api/archives/batch-delete` | POST | 批量删除档案（整批一个 `undo_token`） |
| `/api/archives/undo-delete` | POST | 撤销刚才的移除（`{token}`），把档案行、标签/分类关联、书签与阅读进度原样还原；跳过在撤销窗口内已被重新扫描入库的路径 |
| `/api/archives/regenerate-titles` | POST | 按文件名重新生成标题 |
| `/api/archives/:id/file` | POST | 下载档案原文件（跨机同步用） |
| `/api/archives/pack-cbz` | POST | 将文件夹打包为 CBZ |
| `/api/archives/convert-cbz/start` | POST | 批量将 7z/RAR/CBR/ZIP 转为 CBZ（成功后删除原文件；可传 `ids` 仅转换选中项） |
| `/api/archives/convert-cbz/status` | GET | 转换为 CBZ 的进度 |
| `/api/archives/convert-cbz/cancel` | POST | 取消转换为 CBZ |
| `/api/open` | POST | 直接打开文件/文件夹路径 |
| `/api/scan` | POST | 扫描目录（增量；返回汇总） |
| `/api/scan/status` | GET | 扫描进度（轮询） |
| `/api/scan/cancel` | POST | 取消扫描 |
| `/api/merge` | POST | 合并档案为章节组 |
| `/api/cbz/list` | GET | 列出可打包的 CBZ 文件 |
| `/api/tags` | GET/POST | 标签列表 / 创建标签 |
| `/api/tags/:id` | PUT/DELETE | 更新 / 删除标签 |
| `/api/tags/assign` | POST | 给档案分配标签 |
| `/api/tags/batch-assign` | POST | 批量分配标签 |
| `/api/tags/batch-remove` | POST | 批量移除标签 |
| `/api/tags/namespaces` | GET | 标签命名空间列表 |
| `/api/tags/counts` | GET | 每个标签在给定选中档案（`?ids=1,2,3`）里的出现次数，供批量打标签区分「全部/部分/未包含」 |
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
| `/api/lan-ip` | GET | 本机局域网可达 IPv4 + 端口（设置页展示访问地址） |
| `/api/backup` | GET | 导出备份 |
| `/api/restore` | POST | 导入备份 |
| `/api/sync/manifest` | GET | 跨机同步：远端清单（标题/类型/大小 + 标签/分类/进度） |
| `/api/sync/plan` | POST | 跨机同步：对比预览（url + token + dir），返回新增/变化/已最新清单，不下载 |
| `/api/sync/start` | POST | 跨机同步：启动同步任务（url + token + dir） |
| `/api/sync/status` | GET | 跨机同步：任务进度 |
| `/api/sync/cancel` | POST | 跨机同步：取消任务 |
| `/api/sync/push` | POST | 跨机同步：接收对端标签镜像（title → 标签数组，整组替换；空数组清空，未知标题忽略） |
| `/opds/` | GET | OPDS 根目录 |
| `/opds/catalog` | GET | OPDS 全部档案 |
| `/opds/archive/:id` | GET | OPDS 档案详情 |
| `/opds/recent` | GET | OPDS 最近阅读 |
| `/opds/tags` | GET | OPDS 标签目录 |
| `/opds/tag/:tag_id` | GET | OPDS 标签下的档案 |
| `/opds/categories` | GET | OPDS 分类目录 |
| `/opds/category/:id` | GET | OPDS 分类下的档案 |

### 局域网访问鉴权

设置页开启"局域网口令"（`server_token`）后，**同一局域网内**（非回环地址）的所有 `/api` 与 `/opds` 请求都必须携带口令，否则返回 401；本机回环请求始终放行，桌面端不受影响：

- HTTP 头：`Authorization: Bearer <token>`
- 或查询参数：`?token=<token>`（OPDS 阅读器与网页端图片都用这个——服务端返回的 OPDS XML 内站内链接会自动带上 token；网页端的封面/逐页图片由前端拼接，因为 `<img>` 无法携带请求头）

未设置口令时局域网请求完全开放（仅建议在可信网络下使用）。

> 网页端（手机/平板）只能阅读**已入库**的内容：扫描目录、打开本机文件夹/压缩包依赖桌面端文件对话框，请在桌面应用中先加入书库。

> 网页端首次打开会弹出口令输入框（口令存在浏览器本地），之后书库、封面与逐页图片都自动带上口令；若口令在桌面端被重新生成，请在 iPad 上重新输入一次。

> **访问地址限制（防 DNS 重绑定）**：所有请求都会校验 `Host`/`Origin`，只放行字面 IP、`localhost`/`*.localhost`、`*.local` 与无点局域网短名（如 `nas`），其余带点域名一律返回 403。请用 `http://192.168.x.x:5002` 或局域网短名访问；DDNS/自定义域名会被拒绝。
>
> 另外 `server_token`（口令）与 `server_bind`（绑定地址）只能在本机（回环）修改，局域网设备修改这两个设置会返回 403。

## 📄 License

[MIT](LICENSE)
