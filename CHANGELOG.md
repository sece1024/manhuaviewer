# Changelog

所有值得注意的变更均按 [Conventional Commits](https://www.conventionalcommits.org/) 记录。

本文件由 [git-cliff](https://git-cliff.org) 自动生成（`pnpm changelog`），请勿手工编辑。

<!-- git-cliff: end of header -->

## 未发布

### 🐛 修复

- *(reader)* 消除 Windows 上双页模式的持续闪烁/阅读区跳动([7d9c580](https://github.com/sece1024/manhuaviewer/commit/7d9c5805fdd3b25c1951216e2b6b526e3f126a86))

## [3.6.0](https://github.com/sece1024/manhuaviewer/releases/tag/v3.6.0) - 2026-09-22

### 🚀 新特性

- *(sync)* 同步收尾回传标签镜像（POST /api/sync/push，替换语义）([34cd474](https://github.com/sece1024/manhuaviewer/commit/34cd4749fb971ea3f887a2580da97475bb4b49a6))
- *(library)* 卡片与列表显示漫画添加时间，并修正 DB 时间的 UTC 解析([0d1ab43](https://github.com/sece1024/manhuaviewer/commit/0d1ab43913d2c016643769116e2b70b047a62bbd))
- *(library)* 侧栏新增日期分组（年 → 月）按添加时间过滤浏览([58d989b](https://github.com/sece1024/manhuaviewer/commit/58d989bc622b53ce79d873dbed42c4456d0ee66c))


### 🐛 修复

- *(security)* 增加 Host/Origin 主机形态校验，防 DNS 重绑定([e0dbf4d](https://github.com/sece1024/manhuaviewer/commit/e0dbf4d22832af42fda348d983d885c0913da762))
- *(security)* 敏感设置仅回环可写，备份导入校验档案类型与扩展名([6cdce9f](https://github.com/sece1024/manhuaviewer/commit/6cdce9f9a0221c25aba442187a71d2c06c5fa165))
- *(security)* 文件回传魔数校验、禁止跟随重定向并为同步下载加字节上限([040ba9d](https://github.com/sece1024/manhuaviewer/commit/040ba9df6fa0fb49fb302e7b7098612f35e20344))


### 📚 文档

- 校正四份文档中与代码脱节的事实([e37bccb](https://github.com/sece1024/manhuaviewer/commit/e37bccb061cf71d8c8d643573c6159bde2e74e4e))
- 收敛 agent 指南至 AGENTS.md 唯一正文([b22c277](https://github.com/sece1024/manhuaviewer/commit/b22c277dcf6431e36c1992d7bf3117e07962ea60))
- 补充 DNS 重绑定防护与局域网设置限制说明([818fbc6](https://github.com/sece1024/manhuaviewer/commit/818fbc611472e5f4f835d536a159f9b820f31b3a))


### ⚙️ 杂项

- *(release)* 引入 git-cliff 自动生成 CHANGELOG 与 Release 正文([76fd782](https://github.com/sece1024/manhuaviewer/commit/76fd7821390d93a630f6f40429179eaa62d5a549))
- *(scripts)* 一键发版脚本 release.sh（bump → changelog → commit → tag → push）([6d7eaf3](https://github.com/sece1024/manhuaviewer/commit/6d7eaf3583fdeb4894d2b73e9f48591a36ad9da5))
- *(release)* 发版加固——tag 版本断言、release.sh --check 门禁、Dependabot([c301c1d](https://github.com/sece1024/manhuaviewer/commit/c301c1d4f828da769ab01afb8faf2caeba80838a))

## [3.5.3](https://github.com/sece1024/manhuaviewer/releases/tag/v3.5.3) - 2026-09-21

### 🚀 新特性

- *(scan)* 扫描进度条、取消与并行数页([f99f379](https://github.com/sece1024/manhuaviewer/commit/f99f379e0e88201ddfdcc7cb7d2d570fec57d8dd))
- *(thumbnails)* 封面按磁盘预算 LRU，页面缩略图独立成页([7345ef5](https://github.com/sece1024/manhuaviewer/commit/7345ef5a95df19b4d9c5c6e476236f65ad2bb677))
- *(cache)* 解压产物按 4GB 预算 LRU，模块更名 cache_budget([ee72528](https://github.com/sece1024/manhuaviewer/commit/ee72528cf3f479b8cb616cb97bfe63c6e49eb00b))
- *(convert)* 批量将 7z/RAR/CBR/ZIP 转为 CBZ 并清理原文件([2a4a5a3](https://github.com/sece1024/manhuaviewer/commit/2a4a5a3ca3dbbaaf863a97a5fb7fd3db34d66076))
- *(convert)* 支持仅转换选中项、小并发，并修正进度轮询([76bc98b](https://github.com/sece1024/manhuaviewer/commit/76bc98b30d7718909fe4e8c65918effcf4ce13d7))


### ⚡ 性能

- *(archives)* 书库列表分组与分页下推到 SQL([69e1cdd](https://github.com/sece1024/manhuaviewer/commit/69e1cddc2419cadeaea8195f756d99fb8a93f2b0))

## [3.5.2](https://github.com/sece1024/manhuaviewer/releases/tag/v3.5.2) - 2026-09-16

### 🐛 修复

- Settings statistics cards were blank due to mismatched response keys([cc2491b](https://github.com/sece1024/manhuaviewer/commit/cc2491b027193a9a8901965a8905779f10e860d0))
- Group-main chapter list dead in reader; align settings tags with backend([a43289e](https://github.com/sece1024/manhuaviewer/commit/a43289e17ead7e4e7791b559b78c21074104c741))

## [3.5.1](https://github.com/sece1024/manhuaviewer/releases/tag/v3.5.1) - 2026-09-15

### 🐛 修复

- Return 404 for archives deleted from disk instead of opaque 500([4080289](https://github.com/sece1024/manhuaviewer/commit/408028928cbdea7762f4d92121ab9dd489a644be))
- Invalidate library browse session when archives change([e163b94](https://github.com/sece1024/manhuaviewer/commit/e163b942c599660bdbee0714930954881f808304))


### 🚜 重构

- Dedupe archive row mapping, mtime helper, handler boilerplate([55793f1](https://github.com/sece1024/manhuaviewer/commit/55793f17deb4adca89cfd90aafe6329a715e5b88))
- Split monolithic Database impl into per-domain query modules([2d0ed72](https://github.com/sece1024/manhuaviewer/commit/2d0ed72e3a9180cb19d07eda87e622149f0706e3))
- Move page-list cache into services, dedupe OPDS feed templates([6dbc217](https://github.com/sece1024/manhuaviewer/commit/6dbc21769c7222f87fc0202fa37a99af0fedf754))
- Extract reader virtual list, thumbnail panel and reader hooks([4f7fcdd](https://github.com/sece1024/manhuaviewer/commit/4f7fcdd9a15d7f56d12aeeb1432281b66c516aa5))
- Extract library browse-session state into useLibrarySession([1bf1d3b](https://github.com/sece1024/manhuaviewer/commit/1bf1d3b3521f715cbedbfc5ee6bfe1f86271a20e))
- Extract cross-machine sync state into useSync([3daca98](https://github.com/sece1024/manhuaviewer/commit/3daca98fd550d356085e1c26350bca768812492d))


### ⚙️ 杂项

- Remove dead api mock file([327da53](https://github.com/sece1024/manhuaviewer/commit/327da53007cf8462d282cac192c5405f7fd970a1))

## [3.5.0](https://github.com/sece1024/manhuaviewer/releases/tag/v3.5.0) - 2026-09-15

### 🚀 新特性

- Wrap around to first/last page when flipping past book end([d91e294](https://github.com/sece1024/manhuaviewer/commit/d91e294f118359018dfaaadc150b21a86ee2a177))
- Unified modal keyboard handling, safe localStorage, fix ConfirmDialog Enter([4c99a62](https://github.com/sece1024/manhuaviewer/commit/4c99a625e82f2c46d80545354726701c148f7cb7))
- Whitelist server bind address and surface LAN exposure warnings([0ed18cd](https://github.com/sece1024/manhuaviewer/commit/0ed18cd90b65d2d07cfd030ddf08ae2b0361c37f))
- Token now guards all LAN access; add LAN token prompt and OPDS link passthrough([b87a28b](https://github.com/sece1024/manhuaviewer/commit/b87a28b984dc9a1834ef0dc26b187e7637772dc1))
- Merge library and folder tabs into one unified comics library([791194e](https://github.com/sece1024/manhuaviewer/commit/791194ed189b6333f1c3a3e93f9fab00a1677493))


### 🐛 修复

- Reader progress corruption when switching archives on same route([420c02a](https://github.com/sece1024/manhuaviewer/commit/420c02a6f0d5ccf74b41415ddc87dcc93653bb2e))
- SSRF whitelist bypass via IPv4-mapped IPv6 and mixed DNS records([9dd21f7](https://github.com/sece1024/manhuaviewer/commit/9dd21f706a6de40d4b3940d6eb0b1866b0fb5d32))
- Harden archive extraction against zip-slip, bazooka DoS and hung processes([83f28ba](https://github.com/sece1024/manhuaviewer/commit/83f28baf2e17aa00347cb970c954fcc6b0629688))
- Returning from the reader no longer resets the library paging position([ca7b49c](https://github.com/sece1024/manhuaviewer/commit/ca7b49c466dc730fbcf731811763e8b777936f8a))


### ⚡ 性能

- Expression index for recent-read sort, cap thumbnail generation concurrency([84fed41](https://github.com/sece1024/manhuaviewer/commit/84fed4159723add6fd0fb1396c9ba9a372945985))
- Render thumbnail panel as a virtual window instead of mounting every <img>([ed33cd2](https://github.com/sece1024/manhuaviewer/commit/ed33cd26f6f47137374dcf0c60cb81fd6e77f65d))
- Stream compressed page bytes chunked instead of one Vec per page([68b9eaa](https://github.com/sece1024/manhuaviewer/commit/68b9eaa74d3d309217f18d00470d5b9b3493aaa1))


### ⚙️ 杂项

- Drop unused thumbnail item ref registry left by windowing rewrite([0d3db54](https://github.com/sece1024/manhuaviewer/commit/0d3db5431fe5b9b03ad9d7007f5f2e2b01c8b4f1))

## [3.4.7](https://github.com/sece1024/manhuaviewer/releases/tag/v3.4.7) - 2026-09-15

### 🚀 新特性

- Auto-fallback to single page when double-page spread is too wide([f1155f5](https://github.com/sece1024/manhuaviewer/commit/f1155f571d7a0c82fdd1c7572878d8f991896f11))


### 🐛 修复

- Eliminate double-page flicker on page turns([938241d](https://github.com/sece1024/manhuaviewer/commit/938241d7842520bf0c4981059e157bab27783e1b))

## [3.4.6](https://github.com/sece1024/manhuaviewer/releases/tag/v3.4.6) - 2026-09-14

### 🚀 新特性

- Sync compare/preview (plan endpoint + 'only sync diffs')([5497b9c](https://github.com/sece1024/manhuaviewer/commit/5497b9c853731555063029c612473cae9603bc00))
- Preserve source file mtime through sync([8e1084c](https://github.com/sece1024/manhuaviewer/commit/8e1084cb87f238fecf693efd67833296c6ee4285))
- Cancel sync immediately mid-download and purge stale .part files([5125ca6](https://github.com/sece1024/manhuaviewer/commit/5125ca654112ca12b5492dcf27d5d6ecb09caa57))

## [3.4.5](https://github.com/sece1024/manhuaviewer/releases/tag/v3.4.5) - 2026-09-14

### 🚀 新特性

- Show LAN access addresses in settings and sidebar([9840b0b](https://github.com/sece1024/manhuaviewer/commit/9840b0b9b49b3feaa58132abd186ebcff8718cdb))


### 🐛 修复

- Double-page mode could not be exited; fade double images on load([a2174db](https://github.com/sece1024/manhuaviewer/commit/a2174dbf7b0588e3a7534eec317f99aef54b2afe))
- Reader image error fallback instead of infinite spinner([856672c](https://github.com/sece1024/manhuaviewer/commit/856672c6783926bb0eaa00a2abbce97ea56bca1a))
- Mobile layout adaptation — bottom nav, safe-area dvh, allow zoom([e3ac292](https://github.com/sece1024/manhuaviewer/commit/e3ac292cf456651d752426cf1eef17af83fb237a))


### 📚 文档

- Sync CI/test/API docs with current workflows and routes([318e74b](https://github.com/sece1024/manhuaviewer/commit/318e74b2273eb0fe5f19bb6846eba748b9f5362a))
- Correct frontend test mocking note; document /api/lan-ip([61626fe](https://github.com/sece1024/manhuaviewer/commit/61626feb9dc3fd11b1d57cf6d6257354250defd8))


### ⚡ 性能

- Offload auth token read off async runtime; reuse page cache in OPDS([51f0db3](https://github.com/sece1024/manhuaviewer/commit/51f0db3f6a40098f73de92aec3bc657475c20b47))
- Stabilize tags context value and memoize category sort([769425f](https://github.com/sece1024/manhuaviewer/commit/769425fdac0f579468687ac1aec1b9cd3013b187))


### 💼 其他

- Validate remote cover URLs against SSRF allowlist (shared with sync)([bc48907](https://github.com/sece1024/manhuaviewer/commit/bc4890759401e61dc52ff891a2170807c360cb53))
- Only expose host LAN addresses to loopback clients([6b548c1](https://github.com/sece1024/manhuaviewer/commit/6b548c187401e97c3fc6229d10cbd7e5ef9ac78d))

## [3.4.4](https://github.com/sece1024/manhuaviewer/releases/tag/v3.4.4) - 2026-09-13

### 🚀 新特性

- Cover-first compact grid density with tag dots([7596e82](https://github.com/sece1024/manhuaviewer/commit/7596e825cf57d8d6c9be290f802632e18668aa12))
- Sync entire library from a LAN server to this machine([c96da1e](https://github.com/sece1024/manhuaviewer/commit/c96da1e08d3282aa60ab087f5e569a5713f13bf4))


### 🐛 修复

- Make scan orphan cleanup conservative and root matching component-based([2d5e8d5](https://github.com/sece1024/manhuaviewer/commit/2d5e8d5bf8cf76a58408c121734f6d8f92db6306))
- Attach tags to library list items so card tags render([5f00662](https://github.com/sece1024/manhuaviewer/commit/5f00662acdf66df57747bcb62cca8dd92aa10c27))
- Clippy -D warnings (unneeded cast, type complexity, needless ?)([e60ec1a](https://github.com/sece1024/manhuaviewer/commit/e60ec1ab36ca46caed49154f420cce5f432d0872))
- Never create _2 duplicates for folder archives on re-sync([2be1afd](https://github.com/sece1024/manhuaviewer/commit/2be1afd7f9028f962818ad33ffa4fea247b9fccd))


### ⚡ 性能

- Stabilize reader virtual scroll, memoize pages, precise cache invalidation([be6f553](https://github.com/sece1024/manhuaviewer/commit/be6f553f474c9266c0a046dd49fe939c73aca482))
- Cache page tables in memory, keep thumbnail IO off async threads([a727a85](https://github.com/sece1024/manhuaviewer/commit/a727a85934d74adf31bb4def83ad225a6573745b))
- Index recent-read sort, batch small writes, stable random pagination([5a29ecf](https://github.com/sece1024/manhuaviewer/commit/5a29ecf17dc3c70f5fa388d608e673a278302b6f))


### ⚙️ 杂项

- Solid thumbnail panel, memoize settings context, dep upgrades, doc sync([c9824bf](https://github.com/sece1024/manhuaviewer/commit/c9824bf4563d8aef78c3c4555176b96fd340b180))


### 💼 其他

- Harden LAN auth, response redaction, backup filtering, extract paths([c52a7b0](https://github.com/sece1024/manhuaviewer/commit/c52a7b0bae9acc6435a724a4b1e5de7ca38594de))
- Fix LAN redaction bypass on top-level arrays, harden sync surface([28895f8](https://github.com/sece1024/manhuaviewer/commit/28895f867d7749582fee1526ba24b78d1825e1de))

## [3.4.3](https://github.com/sece1024/manhuaviewer/releases/tag/v3.4.3) - 2026-09-11

### 🐛 修复

- Serve LAN web UI from embedded frontend assets([319d577](https://github.com/sece1024/manhuaviewer/commit/319d57739fc1f2fdb78b4686b6b0ea8cafa0b21e))


### 📚 文档

- Expand copilot instructions with auth, caching, and testing conventions([19d59b7](https://github.com/sece1024/manhuaviewer/commit/19d59b7ce99436ea104388ed53d034faef48454a))

## [3.4.2](https://github.com/sece1024/manhuaviewer/releases/tag/v3.4.2) - 2026-09-09

### 🚀 新特性

- Optional LAN auth (configurable server_token)([3bf2496](https://github.com/sece1024/manhuaviewer/commit/3bf2496dd6dca62c03680a89249bdc57d1a5bff0))
- Attach LAN token header and manage token in Settings([3a50669](https://github.com/sece1024/manhuaviewer/commit/3a50669713554934a2de8068392ddb02d0b6e8da))


### 💼 其他

- Drop unsafe-eval CSP and harden open_file path input([1ab2ca4](https://github.com/sece1024/manhuaviewer/commit/1ab2ca43e72b9c147bdbc7495762e1509596aaac))

## [3.4.1](https://github.com/sece1024/manhuaviewer/releases/tag/v3.4.1) - 2026-09-08

### 🚀 新特性

- Prune orphaned cache dirs and evict thumbnails on startup([3e13f0a](https://github.com/sece1024/manhuaviewer/commit/3e13f0a9d70a26ca03a07f117b2e29a80850bb7a))


### 🐛 修复

- Remove duplicate #[test] attribute([9f5d7f4](https://github.com/sece1024/manhuaviewer/commit/9f5d7f4adb19f2f16da941388776e7d074316e5c))
- Use ON CONFLICT DO NOTHING in insert_archive to avoid race([645c976](https://github.com/sece1024/manhuaviewer/commit/645c9766c7b5839e5e2433a07c7de1300a11055b))
- Reset reader view state when archive changes([3bd5109](https://github.com/sece1024/manhuaviewer/commit/3bd5109df3397e15f2d2084b6e79146d3d392f4e))
- Keep list_cbz_files directory I/O off the async runtime([bdcf654](https://github.com/sece1024/manhuaviewer/commit/bdcf6542801f4913928dd1219692cc0345c0c5dd))
- Invalidate bookmark cache on add/remove([2a2b3b8](https://github.com/sece1024/manhuaviewer/commit/2a2b3b81d106c12cf1d43555923e4f710596da1d))


### 🚜 重构

- Fetch remote cover in the same query as archive([f3943e9](https://github.com/sece1024/manhuaviewer/commit/f3943e91bd01c488536e8c0a28708ad004268d83))


### 📚 文档

- Fix single-test command (pnpm mangles --testPathPattern=X)([b9a0c35](https://github.com/sece1024/manhuaviewer/commit/b9a0c3574d60a57da7ec76a6b9b5ecfd38f20dc9))


### ⚡ 性能

- Stabilize toggleGroup callback to avoid list-wide re-renders([9ee2d79](https://github.com/sece1024/manhuaviewer/commit/9ee2d79da7ff373b3e9ec734bba1873bd497926d))
- Debounce reader_bg color input to avoid PUT storm([3ec7c7d](https://github.com/sece1024/manhuaviewer/commit/3ec7c7d4033c7428bbcb434c0dfca5a43254ee57))
- Batch scan upserts into a single transaction([f14ce0d](https://github.com/sece1024/manhuaviewer/commit/f14ce0d513fcefca508c9a2579cbe9c5ddfa2337))
- Window long-image list to avoid mounting all pages([7ecfef2](https://github.com/sece1024/manhuaviewer/commit/7ecfef23baa6991f675aa59b0411b65497b20946))


### 🎨 样式

- Apply cargo fmt([b2ee073](https://github.com/sece1024/manhuaviewer/commit/b2ee0738be0c55b6b03dffaae2eec05536cde53b))

## [3.4.0](https://github.com/sece1024/manhuaviewer/releases/tag/v3.4.0) - 2026-09-08

### 🚀 新特性

- Support tag:, exclusion and tag-name search in library search([34f74f2](https://github.com/sece1024/manhuaviewer/commit/34f74f276f0806ee1a0b7218c789a4100599340f))
- Incremental rescan with orphan cleanup (backend)([0dabaa9](https://github.com/sece1024/manhuaviewer/commit/0dabaa9245e49749d5424322496104d3b64d11c0))
- Scan library root directory from settings UI([d09ffe1](https://github.com/sece1024/manhuaviewer/commit/d09ffe1ea9d4c11937e84646a3489488d67a297c))
- Restore library scroll and loaded pages when returning from reader([a7376e3](https://github.com/sece1024/manhuaviewer/commit/a7376e369ef59979ad8ead680960c86f1b27d184))
- Auto-continue to next chapter and show per-chapter progress([67f1509](https://github.com/sece1024/manhuaviewer/commit/67f15093485baf07b44c9816a8fd6f33c534a6fa))
- Persist reader layout preferences across sessions([6b66502](https://github.com/sece1024/manhuaviewer/commit/6b66502deb53dbdc674933966830e9fadb47784f))
- Optional LAN mode with tightened CORS and HTTP frontend([e4e3d9b](https://github.com/sece1024/manhuaviewer/commit/e4e3d9b0731a99623274e50e32bac1670d949985))
- Filter library by read/unread and shuffle sort([50dd5dc](https://github.com/sece1024/manhuaviewer/commit/50dd5dcd14f845958e82aa95e937703597ac6114))
- Page turns via gamepad/flipper and mouse side buttons([09b974f](https://github.com/sece1024/manhuaviewer/commit/09b974f684c5908fcc7add5bacef22b5578f65a8))
- Track DB schema with PRAGMA user_version([ba6c06e](https://github.com/sece1024/manhuaviewer/commit/ba6c06eaa294035917f82c35e2a03c821cbd7b96))
- Per-page bookmarks (backend)([b375af0](https://github.com/sece1024/manhuaviewer/commit/b375af02a23c3c6ffa7ffbc882a57013030470a0))
- Per-page bookmarks UI in the reader([55e48ec](https://github.com/sece1024/manhuaviewer/commit/55e48ecd40aab1d314d0ad13ab6e35a05660daf4))
- Scheduled automatic backups with retention([4360b6d](https://github.com/sece1024/manhuaviewer/commit/4360b6d3293a9bd23052e1aaf5969f2bfcfc19b7))
- Manual cover page management([0e4d073](https://github.com/sece1024/manhuaviewer/commit/0e4d073dad0c0b549d5aec4e35d6ccd6cb22ad8f))
- Remote cover URL (schema v3)([533bc0b](https://github.com/sece1024/manhuaviewer/commit/533bc0b815c3f0642ee70e6b334c34d287217782))
- Remote cover URL UI in reader([b11e95a](https://github.com/sece1024/manhuaviewer/commit/b11e95a987fa297a1ed7f5deb2f24a78038cebe9))
- Bangumi metadata search endpoint([8e04abf](https://github.com/sece1024/manhuaviewer/commit/8e04abf5aea0dd78f56a827f53d44e02c8eca8ef))
- Apply Bangumi search results as covers from the reader([e7e319b](https://github.com/sece1024/manhuaviewer/commit/e7e319b69c126aa12ad5bf131f7fe8c7c651abc6))
- Release update check (no auto-install)([71f5719](https://github.com/sece1024/manhuaviewer/commit/71f5719078cf4d275727127d207c6261289220a3))


### 🐛 修复

- Wire real reading progress into library list and recent-read sort([80a806d](https://github.com/sece1024/manhuaviewer/commit/80a806d70b95a76d2412e65b4964b1b57357e047))
- Track and restore progress in long-image (webtoon) mode([768d81c](https://github.com/sece1024/manhuaviewer/commit/768d81c239a00ddcb62fd731840a3b0ee9ea6cbb))
- Complete backup/restore with history and tag/category bindings([a99b803](https://github.com/sece1024/manhuaviewer/commit/a99b803739c77b8f36e88fdd3de50dee429f3e84))
- Wait on SQLite locks and retry history writes under contention([41fa410](https://github.com/sece1024/manhuaviewer/commit/41fa41079c937132a57692303267eabcd2f767b4))
- Degrade to original image instead of 500 when thumbnail decode fails([9af1533](https://github.com/sece1024/manhuaviewer/commit/9af15336a43b398b84b4870584d40b5f319df112))
- Make OPDS feeds valid and usable by external readers([938491e](https://github.com/sece1024/manhuaviewer/commit/938491e89fbaf4f0322e148a0dc907627a476a01))
- LRU thumbnails by real usage, atomic writes, validators on thumbs([1332d22](https://github.com/sece1024/manhuaviewer/commit/1332d22db86f7f491cede0b8aa1a82e842efbe24))
- Gate reader keys on overlays/buttons, pre-paint theme, fresh tag counts([6bc7773](https://github.com/sece1024/manhuaviewer/commit/6bc77737f222f8f775f4d2cbabced104f85855bb))
- Migrate pnpm build whitelist to allowBuilds map([ee4284f](https://github.com/sece1024/manhuaviewer/commit/ee4284fd72cb06e333be2633d9b4b05ccf6df561))


### 📚 文档

- Refresh README features and API endpoints([6b767f9](https://github.com/sece1024/manhuaviewer/commit/6b767f951e11203f2eced62643f45ec8473376dd))


### ⚡ 性能

- Memoize external tool resolution for rar/7z archives([aba0cd2](https://github.com/sece1024/manhuaviewer/commit/aba0cd25ca9fe905527b308f891c56462f845352))
- Persistent per-archive extraction cache for rar/7z pages([e8b4d55](https://github.com/sece1024/manhuaviewer/commit/e8b4d554cbd21082035169b1651fc48158670ad1))
- Infinite-scroll loading and cheap offscreen culling in library([f644b59](https://github.com/sece1024/manhuaviewer/commit/f644b5936a93e9d3fb7470161c301713837820a0))
- Accurate long-image geometry, tighter preload and async decode([35692e1](https://github.com/sece1024/manhuaviewer/commit/35692e1724629365d345eca37c301af876403b82))


### 🎨 样式

- *(frontend)* Refine design system with premium UI polish([0726a96](https://github.com/sece1024/manhuaviewer/commit/0726a96704281eb4fb40aa4121751650f3079667))


### 🧪 测试

- *(frontend)* Fix 2 pre-existing failing test suites([53a3241](https://github.com/sece1024/manhuaviewer/commit/53a32414f06d81dc8c215efabec6463ba005aaad))


### ⚙️ 杂项

- Run frontend test suite in CI([50d2436](https://github.com/sece1024/manhuaviewer/commit/50d24366d0f00544d8e3f90ab41bdf412693e409))
- Migrate frontend from CRA to Vite([aab95a3](https://github.com/sece1024/manhuaviewer/commit/aab95a3f8ee6072d327435fb68cb3a44c45aa435))

## [3.3.9](https://github.com/sece1024/manhuaviewer/releases/tag/v3.3.9) - 2026-08-17

### 🚀 新特性

- Auto-merge same-title manga with inline chapter expansion([a812163](https://github.com/sece1024/manhuaviewer/commit/a81216338810dcdc87622487aa09e450c193dceb))


### 🐛 修复

- Correct auto-merge grouping and add per-chapter management([81fccab](https://github.com/sece1024/manhuaviewer/commit/81fccab1f6298c0350257637152832225e407743))


### 🎨 样式

- Apply rustfmt to archives.rs([f5ef60b](https://github.com/sece1024/manhuaviewer/commit/f5ef60b7980855b730e90fd58d945d908ae6948f))
- Apply rustfmt across backend([c9443db](https://github.com/sece1024/manhuaviewer/commit/c9443dbefae825c5563d6a9f4a30d99dff91cebd))


### ⚙️ 杂项

- Whitelist core-js build scripts for pnpm 11([6137f01](https://github.com/sece1024/manhuaviewer/commit/6137f01568e6c9a79846607467b7eba2115ca5c1))

## [3.3.8](https://github.com/sece1024/manhuaviewer/releases/tag/v3.3.8) - 2026-08-14

### 🐛 修复

- Preserve cover thumbnail aspect ratio and cache covers([3b5423d](https://github.com/sece1024/manhuaviewer/commit/3b5423d9edd7567c6f67eab8cb2fd8b2bb4297d7))
- *(ui)* Improve secondary button contrast and hover feedback([3dabf80](https://github.com/sece1024/manhuaviewer/commit/3dabf80734a14089b8d7e138650747feb73df766))

## [3.3.7](https://github.com/sece1024/manhuaviewer/releases/tag/v3.3.7) - 2026-08-12

### 🚀 新特性

- Configurable initial title depth when opening comics([1e765aa](https://github.com/sece1024/manhuaviewer/commit/1e765aa25cca73ac1b7849b778380abcb8d2525b))
- Live title preview and bulk regenerate existing titles([cac1f3a](https://github.com/sece1024/manhuaviewer/commit/cac1f3aadb32337736db6e5170faded7bd3a14f7))

## [3.3.6](https://github.com/sece1024/manhuaviewer/releases/tag/v3.3.6) - 2026-08-10

### 🐛 修复

- *(frontend)* Correct api.js cache invalidation and TTL keys([168feac](https://github.com/sece1024/manhuaviewer/commit/168feac19a0c432ae710af81d07b65ceb34e15bc))
- *(frontend)* Fix infinite duplicate page-2 pagination in Library([25d26e8](https://github.com/sece1024/manhuaviewer/commit/25d26e84546bf856f5d3dd38e46e28bbf1259788))
- *(frontend)* Fix LazyImage observer leak and stale image state([de72a83](https://github.com/sece1024/manhuaviewer/commit/de72a838e8dca5a44faaea0d0d70dc117ee5f3c6))
- Windows compatibility for rename suggestions and RAR/7Z support([e7c6c0b](https://github.com/sece1024/manhuaviewer/commit/e7c6c0be467817931a164b3e3791a72c50ccc2fb))


### 🚜 重构

- *(backend)* Replace global DB mutex with r2d2 pool + spawn_blocking([a5d2e02](https://github.com/sece1024/manhuaviewer/commit/a5d2e0225f4deb93d25bd122e22676b6256c1004))
- *(frontend)* Remove dead useSettings API and extract shared Modal([16447c3](https://github.com/sece1024/manhuaviewer/commit/16447c3cf5c8bb23c223786c5d51015f9305e0df))


### ⚡ 性能

- *(backend)* Cache compressed-archive page lists in the pages table([cda25c5](https://github.com/sece1024/manhuaviewer/commit/cda25c55a1c1601702006badebfc6412b08ef7cf))
- *(backend)* Throttle thumbnail eviction and drop correlated subquery([c1d3c9d](https://github.com/sece1024/manhuaviewer/commit/c1d3c9d481022205a1e135cf2a09da869725876e))
- *(backend)* Drop wasted archive COUNT and fix category listing([9e4ddfa](https://github.com/sece1024/manhuaviewer/commit/9e4ddfa2a4a9d8c956bfb96708630fe52a5f6503))
- *(backend)* Transactional scan inserts and stream folder pages([6d0679b](https://github.com/sece1024/manhuaviewer/commit/6d0679b5a96031686f91f637ff7069f2583fbfa2))
- *(frontend)* Virtualize Reader long-image and thumbnail panel([9cfa7dd](https://github.com/sece1024/manhuaviewer/commit/9cfa7ddd9601000d1260d5e6ae9f1dba4f688901))
- *(frontend)* Memoize archive cards/LazyImage and split routes([d89dee3](https://github.com/sece1024/manhuaviewer/commit/d89dee39cacd94b8b9f7efc76c8ca0b58f2ab19d))


### ⚙️ 杂项

- Clean up stale docs, dead code, and unused artifacts([b4efaa1](https://github.com/sece1024/manhuaviewer/commit/b4efaa12c52acd64d852133fc5d7e070a309bb84))

## [3.3.5](https://github.com/sece1024/manhuaviewer/releases/tag/v3.3.5) - 2026-08-03

### 🐛 修复

- Add file logging, panic hook, and single-instance lock to prevent silent startup failures([e4ec36e](https://github.com/sece1024/manhuaviewer/commit/e4ec36ed3ff179babbcd5c95c83a4276a7d14d74))

## [3.3.4](https://github.com/sece1024/manhuaviewer/releases/tag/v3.3.4) - 2026-07-31

### 🚀 新特性

- Paginate history endpoint([3f90fbd](https://github.com/sece1024/manhuaviewer/commit/3f90fbd16720b69865cd3bae335f7217d21d1fa1))
- Surface category navigation and add batch tag/category/delete operations([23c9404](https://github.com/sece1024/manhuaviewer/commit/23c9404703568a87f1132e726c105281e21e4846))
- Remove root-directory auto-scan feature from frontend([291934a](https://github.com/sece1024/manhuaviewer/commit/291934aec2605524058ec3ee7f3b7851fe006fb4))
- Make rename-suggestion path depth configurable([3829133](https://github.com/sece1024/manhuaviewer/commit/3829133c186e86fecf06d150810725191c1ae9c5))


### 🐛 修复

- Correct eslint-disable directive in History.js([fb58812](https://github.com/sece1024/manhuaviewer/commit/fb588124e9d80f5c12fdc40fc539dd58a666e677))


### ⚡ 性能

- Add db indexes for archive sort/search and history columns([989c6e7](https://github.com/sece1024/manhuaviewer/commit/989c6e7f04a5c3ab93d36fa0b61d273cd0fda6c8))

## [3.3.3](https://github.com/sece1024/manhuaviewer/releases/tag/v3.3.3) - 2026-07-13

### 🚀 新特性

- Add manga rename and merge features([679c2de](https://github.com/sece1024/manhuaviewer/commit/679c2dec3b644f1e202ddb9fbfdff877b731996f))


### 📚 文档

- Add implementation workflow with commit step to AGENTS.md([97c39a4](https://github.com/sece1024/manhuaviewer/commit/97c39a49bf1cc09b34ce22874da5e9d9b81219a4))

## [3.3.2](https://github.com/sece1024/manhuaviewer/releases/tag/v3.3.2) - 2026-07-07

### 🚀 新特性

- Add in-memory cache layer with TTL and in-flight dedup for GET API requests([b744875](https://github.com/sece1024/manhuaviewer/commit/b744875a10bde60e638eddd69483e0ca4c01f166))
- Add module-level cache for useTags to avoid redundant fetches on remount([c5ff4a0](https://github.com/sece1024/manhuaviewer/commit/c5ff4a0747f24739583713f8bd6e59c0634a2b18))


### 🐛 修复

- Add confirmation dialog before removing archive from library([2a26fcb](https://github.com/sece1024/manhuaviewer/commit/2a26fcb1e7fb7e4e8f15d86ab6fa62982ebf337a))
- Replace IIFE in Reader JSX with pre-computed double-page values([9673a44](https://github.com/sece1024/manhuaviewer/commit/9673a449f586b5adb14b710cebb1ec69e3caebb3))


### 🚜 重构

- Deduplicate Library action buttons across desktop/mobile/welcome([dfbc4f0](https://github.com/sece1024/manhuaviewer/commit/dfbc4f040f3904155eab662e4cc936e6821f6826))
- Deduplicate Reader double-page mode JSX([c9b70a5](https://github.com/sece1024/manhuaviewer/commit/c9b70a556a402359caa49d709112ab36ade32c1f))
- Simplify Reader toolbar — move rotation/1:1/direction to more menu([ba36724](https://github.com/sece1024/manhuaviewer/commit/ba3672473c1bd619a597b168cb40ceeb76a04984))


### ⚡ 性能

- Use shared IntersectionObserver for all LazyImage instances([ff0017f](https://github.com/sece1024/manhuaviewer/commit/ff0017ff88c7cc3a94a7cd279723e27421593de2))
- Add LRU eviction to Reader image preload cache (max 30)([b8bd6f4](https://github.com/sece1024/manhuaviewer/commit/b8bd6f49a1f1e8e3838d3ec7f9e1e5b456350429))

## [3.3.1](https://github.com/sece1024/manhuaviewer/releases/tag/v3.3.1) - 2026-06-25

### 🚀 新特性

- Restore separate folder/archive buttons in toolbar, each opens dialog directly([0cc98ad](https://github.com/sece1024/manhuaviewer/commit/0cc98ad235b579b1dbbecc6fbab1ef35517072b9))
- Show exported CBZ files in Settings with click-to-open([5a22e2d](https://github.com/sece1024/manhuaviewer/commit/5a22e2da906dcab630e7a29580ece7b912cd498e))


### 🎨 样式

- Increase touch targets to 44px minimum for mobile accessibility([7f66ecb](https://github.com/sece1024/manhuaviewer/commit/7f66ecbe35b2e77554bf646d6d9ed2f9a8509d36))
- Archive grid uses adaptive minmax columns instead of fixed 160px([fc5b596](https://github.com/sece1024/manhuaviewer/commit/fc5b596a3fc8d132ae292468891a213a70d41a03))
- Settings breakpoint 720px → 768px for consistency, improve mobile nav tap targets([c1b89f0](https://github.com/sece1024/manhuaviewer/commit/c1b89f097d1d0bbd960e0bd36cfca8d7f69ac0f4))
- Move mobile toast to bottom (above nav) to avoid overlapping reader toolbar([0d9d4df](https://github.com/sece1024/manhuaviewer/commit/0d9d4dfece44516fac86f9df6b409bcc4eaf47d4))
- History delete button hidden by default, shown on hover/focus([e56dbae](https://github.com/sece1024/manhuaviewer/commit/e56dbaed6f04a53a7ba0fb10a60f5318a54abad8))
- Extract welcome screen inline styles to CSS classes for consistency([d02621b](https://github.com/sece1024/manhuaviewer/commit/d02621bb2929175ecef90a3504e667e4b93bb766))

## [3.3.0](https://github.com/sece1024/manhuaviewer/releases/tag/v3.3.0) - 2026-06-17

### 🚀 新特性

- Add toast dismiss button, pause on hover, and longer error duration([39441b3](https://github.com/sece1024/manhuaviewer/commit/39441b3f9dc80211f46b348d24b9a554a7fab2c0))
- Scroll to top on route navigation([3aa89fb](https://github.com/sece1024/manhuaviewer/commit/3aa89fb1aaf9d4e218cf69f971e37a1ca9701d9e))
- Escape key now closes TagPicker in Reader([ee4b077](https://github.com/sece1024/manhuaviewer/commit/ee4b077f597b99b4bc37733e12eed7b55c93eb2e))
- ErrorBoundary now offers back-to-library option instead of only reload([a56e3bf](https://github.com/sece1024/manhuaviewer/commit/a56e3bfc624243ce7ff7c3be7bfc472ffdf09fc5))
- Show loading spinner and fade-in transition when turning pages in Reader([b30813a](https://github.com/sece1024/manhuaviewer/commit/b30813a2db47d142ee0e307d2438b4ff83fccf29))
- Thumbnail panel auto-scrolls to current page on open([4496b1b](https://github.com/sece1024/manhuaviewer/commit/4496b1b183bef015078adb4a559b80fd87f94d6d))
- Show skeleton loading cards in Library while data loads([c71eba0](https://github.com/sece1024/manhuaviewer/commit/c71eba0ac365b04524fe28a79a2a7671c8ac2a3d))
- Replace window.confirm with custom ConfirmDialog modal in History and Settings([bee08e5](https://github.com/sece1024/manhuaviewer/commit/bee08e56499d147259587550fd5da45680d3d1b4))
- Simplify open file flow - toolbar button opens file dialog directly, remove folder/archive choice from modal([e5a50d0](https://github.com/sece1024/manhuaviewer/commit/e5a50d05aafc8b06bdae58da6b3b81ad56bf5438))


### 🐛 修复

- Replace undefined setSettings with local state for CBZ export dir input([1c98585](https://github.com/sece1024/manhuaviewer/commit/1c9858509474819e024231981b55cbb2857e2b9c))
- Handle null tags in History and add loading state([6dced3b](https://github.com/sece1024/manhuaviewer/commit/6dced3b3573ec4dbbbca12a395a6531862dc68f9))
- Handle null total_pages in Settings stats rendering([fae2b89](https://github.com/sece1024/manhuaviewer/commit/fae2b89812953ee99d6b5c029e31f8be2e18f9a6))
- Fix search debounce bypassed by useEffect, reducing double API calls per keystroke([5da943a](https://github.com/sece1024/manhuaviewer/commit/5da943a626571dcca619abf186649fc6d42443b4))
- Use env!("CARGO_PKG_VERSION") instead of hardcoded version strings([a67ceac](https://github.com/sece1024/manhuaviewer/commit/a67ceac2b521de6417ab935d1ef980c5ffb74cd9))
- Validate IDs in assign_category instead of defaulting to 0([1aae38a](https://github.com/sece1024/manhuaviewer/commit/1aae38a027d3a3acba56cd111c5d2e4555a9df85))
- Return empty root_dir instead of 500 error when config not set([6738040](https://github.com/sece1024/manhuaviewer/commit/6738040464fd02dce72feaba2479d72495a8879e))
- Add missing API mocks, apply reader_bg setting, and pass sort_order to API([e85177b](https://github.com/sece1024/manhuaviewer/commit/e85177b8fc938b4539a64dc2a4c07985d08fe55c))
- Log scan insert errors instead of silently counting them([452c49f](https://github.com/sece1024/manhuaviewer/commit/452c49fd884c135569814e5d772cbd1bf82bf81f))


### 🚜 重构

- Remove dead code from thumbnail.rs (unused methods, enum, utility functions)([fd8ca85](https://github.com/sece1024/manhuaviewer/commit/fd8ca85df5dff638ea907c9eb2be999096df7203))


### 🎨 样式

- Apply rustfmt to test code([7b362b1](https://github.com/sece1024/manhuaviewer/commit/7b362b14195cfbc2349866d844375cc15545c985))


### ⚙️ 杂项

- Remove 6 unused Rust dependencies (tower, sevenz-rust, dotenvy, quick-xml, thiserror, base64)([d7cbd2e](https://github.com/sece1024/manhuaviewer/commit/d7cbd2ece5496076d61a3d1c2f708632f318b58c))
- Add target/ and runtime data patterns to .gitignore([f7397e5](https://github.com/sece1024/manhuaviewer/commit/f7397e504f544d298caa4e9b4a5ae1429928e4da))
- Remove invalid allowBuilds entries from pnpm-workspace.yaml([a73c160](https://github.com/sece1024/manhuaviewer/commit/a73c1604e19406899b860948c0de688704eaa0bc))


### 💼 其他

- Add mimo agent([5c5836b](https://github.com/sece1024/manhuaviewer/commit/5c5836b2c445408fa633386bb9d6757b8ee6d9bc))

## [3.2.3](https://github.com/sece1024/manhuaviewer/releases/tag/v3.2.3) - 2026-06-02

### 🚀 新特性

- Allow opening manga directly from welcome screen without root dir([f173a18](https://github.com/sece1024/manhuaviewer/commit/f173a18496809c5e3c63296fe60c8897e6e50f1f))
- Separate archives by type and add remove button([44d0a69](https://github.com/sece1024/manhuaviewer/commit/44d0a69bbb140248f9319d28138d38eba568a1bf))
- Implement tag filtering for archive listing([ca0b063](https://github.com/sece1024/manhuaviewer/commit/ca0b063c6efa95bb7fabf9cafb993fbb75482cf5))
- Add GET /api/archives/:id/tags endpoint([599b055](https://github.com/sece1024/manhuaviewer/commit/599b055e28041fa4c05c6915de4f1b82bfe6e835))
- Add TagPicker component for assigning tags to archives([974830c](https://github.com/sece1024/manhuaviewer/commit/974830ced54ef70442e64fd4addce09d7cae9df1))
- Move compressed archives to sidebar '收藏' menu([251d24d](https://github.com/sece1024/manhuaviewer/commit/251d24d680df85da0f46110f9fd89035cdb163b2))
- *(api)* Add ETag / Last-Modified caching to image endpoints([765f662](https://github.com/sece1024/manhuaviewer/commit/765f6621510543b343f1c44df79eb3002f983a86))
- *(ui)* Mobile library toolbar fold, tag sidebar search, settings nav([0602cc9](https://github.com/sece1024/manhuaviewer/commit/0602cc9184c2ad3e7101ca5f71af49d819d2f73b))
- *(backup,reader)* Tauri save dialog, drop redundant mobile bottom-bar([94f6a3f](https://github.com/sece1024/manhuaviewer/commit/94f6a3f4d485ac57386c777ef9b0f8699b54e907))


### 🐛 修复

- Uniform archive card sizes in grid view([7baf91c](https://github.com/sece1024/manhuaviewer/commit/7baf91c25a7393f0fdd7f63f241c768555b977ce))
- *(ui)* Rename sidebar nav to '文件夹', tune long-image scroll, faster search([5b442c2](https://github.com/sece1024/manhuaviewer/commit/5b442c2e359ab0738e55fa3a36ac6d14d4970286))
- *(ui)* A11y labels, edge click hints, dedup reader progress save([ebb83a3](https://github.com/sece1024/manhuaviewer/commit/ebb83a3c327585cde333d579a20124dff22a9361))
- *(api)* Conditional content-type, archive pagination, friendlier error msgs([34cacbc](https://github.com/sece1024/manhuaviewer/commit/34cacbcb6c67b5f5c3cb8ce4f0bbe3ff864fb8ce))


### 🚜 重构

- Share tag state via context, slim list_pages archive, apiBase helper([3124f1f](https://github.com/sece1024/manhuaviewer/commit/3124f1f713be9135dd8e1d442e5744a29acb92d9))


### 📚 文档

- Update copilot-instructions([79f547c](https://github.com/sece1024/manhuaviewer/commit/79f547c5f1f0903bb0958e641949b48a8eb3650f))
- Update opencode AGENTS([6e4f74d](https://github.com/sece1024/manhuaviewer/commit/6e4f74dac89082bd777bb823da5a2558e209e9d6))


### 🧪 测试

- Fix 3 pre-existing failing test suites (format, Library, Settings)([2994cfe](https://github.com/sece1024/manhuaviewer/commit/2994cfecc897103b63420eac7d844b6d6dfccfbc))


### ⚙️ 杂项

- Add ad-hoc code signing for macOS release builds([08ece55](https://github.com/sece1024/manhuaviewer/commit/08ece55067f3d25282dfdd2009e15eb0aff8c7a9))

## [3.2.2](https://github.com/sece1024/manhuaviewer/releases/tag/v3.2.2) - 2026-05-27

### 🚀 新特性

- Per-archive thumbnail cache with LRU eviction (max 20 manga)([23672ee](https://github.com/sece1024/manhuaviewer/commit/23672ee0eb521d676a9c4e967aecfa31d72c658b))


### 🐛 修复

- Check thumbnail cache before opening archive([f3c8e9a](https://github.com/sece1024/manhuaviewer/commit/f3c8e9aae736212815f0e348b311963378af7d09))


### ⚙️ 杂项

- Remove legacy Node.js backend and dead code([6bb6153](https://github.com/sece1024/manhuaviewer/commit/6bb6153a0398252658de391f20ba50e9c0fc4798))

## [3.2.1](https://github.com/sece1024/manhuaviewer/releases/tag/v3.2.1) - 2026-05-27

### ⚙️ 杂项

- Add lint and format:check pnpm commands, fix rustfmt issues([8748a79](https://github.com/sece1024/manhuaviewer/commit/8748a793ef9615b4f226943f1e02c1fc88997c5d))

## [3.2.0](https://github.com/sece1024/manhuaviewer/releases/tag/v3.2.0) - 2026-05-27

### 🐛 修复

- Replace unwrap() with proper error handling in OPDS tag_archives handler([052aefc](https://github.com/sece1024/manhuaviewer/commit/052aefcf4fdf2da1a292af9a80a8f3b4417f2efd))
- Remove unsafe std::env::set_var and improve error handling in main([dfe1cf9](https://github.com/sece1024/manhuaviewer/commit/dfe1cf9360a52a71681ddabe0c5e48262d1c353c))
- Move blocking I/O to spawn_blocking in archive handlers([9c0e1b3](https://github.com/sece1024/manhuaviewer/commit/9c0e1b3b73400d82a7d1c409222ce9e3696dd621))
- Use tempfile::tempdir for RAR/7z extraction to prevent race condition([cd4e58a](https://github.com/sece1024/manhuaviewer/commit/cd4e58a54cdd416d44c108c20325ec6957e14511))
- Only retry GET requests and reduce retry delay([ab25bcd](https://github.com/sece1024/manhuaviewer/commit/ab25bcdfbe90feceea00d6e7f0c509b61b8cc02d))
- Add virtual scroll to long-image mode to prevent DOM bloat([a8686dd](https://github.com/sece1024/manhuaviewer/commit/a8686dd3dec557477d74bc330e83aab964f2eabe))
- Eliminate N+1 queries in history and OPDS tag_archives([7beb8d9](https://github.com/sece1024/manhuaviewer/commit/7beb8d99675fd20e8db780168188be8e42a7aa72))
- Enable thumbnail caching in get_page_thumb handler([9f8989b](https://github.com/sece1024/manhuaviewer/commit/9f8989babc9ceaa424bd6e5f202e01fdc0a322d9))
- Use UPDATE instead of DELETE+RECREATE for tags and categories([5be097d](https://github.com/sece1024/manhuaviewer/commit/5be097dfa90bcfcb19392b13f8f9d1f77fbba7d8))
- Add page-level ErrorBoundary and fix Toast ID collision([435393a](https://github.com/sece1024/manhuaviewer/commit/435393a0cdaaa678c455f1f95b5a90ca376f397d))
- Log row-level errors instead of silently swallowing them([886a1ed](https://github.com/sece1024/manhuaviewer/commit/886a1ed4cc9cabf2aad958d3bd262102f1c3434a))
- Add keyboard accessibility to archive cards and history items([9477ffb](https://github.com/sece1024/manhuaviewer/commit/9477ffb94891deee91ca87504e607bb3ca4b1413))
- Add aria-labels to delete buttons in Settings page([2a26b02](https://github.com/sece1024/manhuaviewer/commit/2a26b02dbe80e624efb0aafa5513b6532de154bf))
- Address clippy warning - redundant closure in cbz.rs([80d3ad4](https://github.com/sece1024/manhuaviewer/commit/80d3ad44a51019ae707cdc571840ec3b27c60225))
- Restore archive and read_page fields in list_pages response([8ec6369](https://github.com/sece1024/manhuaviewer/commit/8ec636987a6408b17ed4ddcadfb330cd05332389))
- Use ref for currentIndex in goPrev/goNext to prevent stale closures([0b006d3](https://github.com/sece1024/manhuaviewer/commit/0b006d3c74c3f8773dcf237fe55a1908cd5b0c03))
- Eliminate race conditions in Library loading([725acd3](https://github.com/sece1024/manhuaviewer/commit/725acd3ea5e0e388dfaba65e820d473017e758d4))
- Prevent saveHistory race and save on unload([ddb102a](https://github.com/sece1024/manhuaviewer/commit/ddb102aaaca2451cf7d94b50d242e2f8bf635089))
- Unify settings to server-only, remove localStorage for settings([8a5cb1a](https://github.com/sece1024/manhuaviewer/commit/8a5cb1a59415a049999fb8a874cf96877cda77f4))


### 🚜 重构

- Deduplicate error_response, is_image_file, and image extensions([d7c7ff9](https://github.com/sece1024/manhuaviewer/commit/d7c7ff92886493686f70abeca5d0e49a707d438c))


### 📚 文档

- Update documentation to reflect Tauri-first architecture([8245182](https://github.com/sece1024/manhuaviewer/commit/82451824993318530f71f1fea2aca1d2d09ea441))


### ⚙️ 杂项

- Release 新增 macOS Apple Silicon 构建([6132fde](https://github.com/sece1024/manhuaviewer/commit/6132fde4549d7dace1816db4806801ad5213a189))

## [3.1.0](https://github.com/sece1024/manhuaviewer/releases/tag/v3.1.0) - 2026-05-26

### 🐛 修复

- *(ci)* Tauri-action 版本从 v1 改为 v0([1969c03](https://github.com/sece1024/manhuaviewer/commit/1969c03d395dd584a04e958fe45eaeef6935971e))


### ⚙️ 杂项

- Release 工作流仅保留 Windows 构建([f535933](https://github.com/sece1024/manhuaviewer/commit/f53593374c5a8c5bc7a1680315252eab3b350183))
- 添加版本号统一修改脚本([0f0b54b](https://github.com/sece1024/manhuaviewer/commit/0f0b54b1d64eec1e7c797c13cbf16e26f4c4db0f))

## [1.0.1](https://github.com/sece1024/manhuaviewer/releases/tag/v1.0.1) - 2026-05-26

### 🚀 新特性

- 添加数据持久化层 (阅读历史 + 标签管理)([105cbd2](https://github.com/sece1024/manhuaviewer/commit/105cbd215484df276cf96268eec45f3e1feba7a7))
- 集成阅读历史、标签管理、全屏、跳转到页([ae7d14d](https://github.com/sece1024/manhuaviewer/commit/ae7d14d376ea7c5895057d00e9ff3d7c03a779b4))
- 缩略图总览、图片旋转、状态栏标签显示([c5a1b0c](https://github.com/sece1024/manhuaviewer/commit/c5a1b0c628e0e62bf4b63bf540186c52b29cb2bc))
- Add direct file open mode([844032a](https://github.com/sece1024/manhuaviewer/commit/844032aa62dfe2f0ce81c49489932a6b5e1b26d5))
- Add Electron packaging for standalone macOS app([da4fe75](https://github.com/sece1024/manhuaviewer/commit/da4fe751ba763631943ea6f294b887e3fe0e7c87))
- Initialize Tauri 2.0 migration project([20028bb](https://github.com/sece1024/manhuaviewer/commit/20028bbb4f2c6dd46d317d7cefe15e53a87843fa))
- Implement stage 2 - Rust backend core([c0303b8](https://github.com/sece1024/manhuaviewer/commit/c0303b857a6dd90249dd7f29e20dff6eb0f993a6))
- Implement stage 3 - Archive format support([2f6d84c](https://github.com/sece1024/manhuaviewer/commit/2f6d84c8c08c4351b4c0d94896f880614211e0dd))
- Implement stage 4 - Image processing([f2b43cd](https://github.com/sece1024/manhuaviewer/commit/f2b43cddee378e2037ea0d1dfe3251b27efd272f))
- Implement stage 5 - OPDS server([0e3304b](https://github.com/sece1024/manhuaviewer/commit/0e3304b69d905f7ffd775c3286d76fae7e9efe23))
- Fix API compatibility issues([8b07812](https://github.com/sece1024/manhuaviewer/commit/8b07812773fa6bc470bfb8b2dee8ec7de7c3ba3b))
- Create professional app icons([d53219f](https://github.com/sece1024/manhuaviewer/commit/d53219f0aed3aeb998fedc7e3754e363f9cfaab0))
- 漫画文件夹打包归档为CBZ功能([83953c6](https://github.com/sece1024/manhuaviewer/commit/83953c634f8e346fe36d4bd4672e6a77f5d629bd))
- 优化双页模式 - 窗口过窄时禁用，与长图模式互斥([f05a543](https://github.com/sece1024/manhuaviewer/commit/f05a5433517273886830ea133912ab124605afd9))


### 🐛 修复

- 跨平台打包兼容 (macOS/Windows/Linux)([cb6d93f](https://github.com/sece1024/manhuaviewer/commit/cb6d93f1a505bd7f851915de4db6ecb637ea81a1))
- 删除 uv.lock 解决跨平台依赖问题([8542130](https://github.com/sece1024/manhuaviewer/commit/8542130f54fb65e7312e5b4d15cba33e14d8ac51))
- Windows 上 pyqt5-qt5 依赖问题([275eab9](https://github.com/sece1024/manhuaviewer/commit/275eab9f4b9ca87721ef41ba9a7be836b9376eb4))
- 添加启动脚本，绕过 uv run 的依赖重解析问题([c2b0cf8](https://github.com/sece1024/manhuaviewer/commit/c2b0cf8fdfca8a8ca289b58871212587aa5cd84d))
- 双页模式右视图未加入布局导致弹出为独立窗口([dc9bc50](https://github.com/sece1024/manhuaviewer/commit/dc9bc503e70fca12ac184a3862f49d2bdb07128a))
- 优化报告#1 - 防抖/全屏/线程安全/长图快捷键([9349e7c](https://github.com/sece1024/manhuaviewer/commit/9349e7c5810a253c2e9fbb14c17a7111bc43241c))
- 优化报告#2 - 原子写入/右键菜单/状态栏增强/常量提取([328810b](https://github.com/sece1024/manhuaviewer/commit/328810b57121248bb14d73022c3fed9f0f4b9da6))
- 优化报告#3 - 预加载线程安全/空文件夹防护/文件大小缓存([8f96939](https://github.com/sece1024/manhuaviewer/commit/8f96939b144e29435da9bc15193e8effa03b6e14))
- 优化报告#4 - 优雅退出/标题栏/原子写入/eventFilter健壮性([7a630c6](https://github.com/sece1024/manhuaviewer/commit/7a630c6729e8de69b6d33aa3c1a4cc784ce1f222))
- Long image mode improvements + update lock files([cdc2fe7](https://github.com/sece1024/manhuaviewer/commit/cdc2fe7d210363d97346aeb865813858bfe352bf))
- Add pnpm workspace support([529e6dc](https://github.com/sece1024/manhuaviewer/commit/529e6dc52f15f045f67881f92274253236ad86b3))
- Fix compilation errors and add placeholder icons([704bddc](https://github.com/sece1024/manhuaviewer/commit/704bddc2615d026c4949b4bc84e0a4d4c075aff1))
- Fix API response format to match frontend expectations([38afe72](https://github.com/sece1024/manhuaviewer/commit/38afe72cdbf7c0b77097636781c21dfe29f393e2))
- Use fixed port 5002 for API server([0b92958](https://github.com/sece1024/manhuaviewer/commit/0b9295802bd669ab7e29e857d97ae8a502688ff9))
- Add retry logic to API calls([0ea72c7](https://github.com/sece1024/manhuaviewer/commit/0ea72c7d77b31198d1728e451a651dabe40a9886))
- Fix pages response format([0a6f345](https://github.com/sece1024/manhuaviewer/commit/0a6f345359261ab22a7a1db1de6f99d85fcb9ff2))
- Remove Overlay title bar style to enable window dragging([99b8154](https://github.com/sece1024/manhuaviewer/commit/99b8154eb592af204dc5369b72fb01fe1165fe80))
- Prevent browser from opening in dev mode([c66c2bc](https://github.com/sece1024/manhuaviewer/commit/c66c2bccaab7122a1c83effd25315b4f407cc520))
- Disable browser opening in dev mode([a703d4b](https://github.com/sece1024/manhuaviewer/commit/a703d4bb9c5ccb4c0ac96bf68e8831654e3f9aae))
- Add archive_type to history response([0a8c812](https://github.com/sece1024/manhuaviewer/commit/0a8c812c0b612834302d25bc54271792b142bfe3))
- Add tags and cover_url to history response([cf3be5f](https://github.com/sece1024/manhuaviewer/commit/cf3be5f394ba33243586e14d66d407b72bd663ce))
- Tauri dialog plugin配置及open_file多个bug修复([7fee613](https://github.com/sece1024/manhuaviewer/commit/7fee613e05bf25e1d314debf313a27f5d5d083c7))
- 添加dialog confirm/ask/message权限修复删除确认报错([7ba2aae](https://github.com/sece1024/manhuaviewer/commit/7ba2aaeb59241c681a11071198f305a06508f651))
- 修复Tauri生产构建下API请求失败([106b83d](https://github.com/sece1024/manhuaviewer/commit/106b83d632e2734e617715eda60479dfa91c5704))
- 修复生产模式下图片和封面URL无法加载([b375017](https://github.com/sece1024/manhuaviewer/commit/b375017c06e4f1e4246154cb86ed5f1123185d46))
- 修复 eslint react-hooks/exhaustive-deps 规则未找到的编译错误([b7cccaf](https://github.com/sece1024/manhuaviewer/commit/b7cccaf539027c7f75e949a9da4cd6d6ac3ee056))


### 🚜 重构

- 项目结构优化与功能增强([5732657](https://github.com/sece1024/manhuaviewer/commit/57326570b7c457a703912e8bcac5a2625ccaa19f))
- 采用 src/ 标准包布局，优化项目结构([6f60617](https://github.com/sece1024/manhuaviewer/commit/6f6061711cf70ed9a4e550b9134980d37881fcc4))
- 重大重构 - 模块化架构 + Bug修复 + 性能优化([363e1ba](https://github.com/sece1024/manhuaviewer/commit/363e1ba3fc25f2486c69a7c6235fcca37b15d626))
- Prepare backend for Electron packaging([943801c](https://github.com/sece1024/manhuaviewer/commit/943801c95fc905d1dd8a2ed1321c9521d97da176))
- Structural audit cleanup([9a9eb79](https://github.com/sece1024/manhuaviewer/commit/9a9eb79ab0d168d666c254c0929c828effb3e149))


### 📚 文档

- 优化报告#5 - README更新/版本号统一([6f9e474](https://github.com/sece1024/manhuaviewer/commit/6f9e474d8de550d6d01ede79128d60997672f058))
- Add Electron packaging and open file API docs([0f0578d](https://github.com/sece1024/manhuaviewer/commit/0f0578d58acaa33ac701e66282004e2a3f121539))
- Update AGENTS.md for pnpm workspace + Electron([5114fc6](https://github.com/sece1024/manhuaviewer/commit/5114fc6381aa4bc7e44bced26ebb3559d694365e))
- Update README for pnpm workspace setup([ed49275](https://github.com/sece1024/manhuaviewer/commit/ed49275f497e17a5bef0fe8bdd74e6048d2d264e))
- Update migration plan with progress([4020599](https://github.com/sece1024/manhuaviewer/commit/4020599916daddcd6474951b9b6c75121bb3d4c8))
- Mark migration as completed([e1a233d](https://github.com/sece1024/manhuaviewer/commit/e1a233ddc4340aeb2a4f8d788bc1febbc24d534b))
- 更新所有文档移除 Electron 引用([0b30fca](https://github.com/sece1024/manhuaviewer/commit/0b30fca2d5b2112361885b6eac9a4cd6ead7d355))
- 添加 CONTRIBUTING.md 贡献指南([b8030f5](https://github.com/sece1024/manhuaviewer/commit/b8030f5dbc6c3838dec523258a115365c6c5c5f3))


### 🧪 测试

- Add comprehensive Rust unit tests for database layer([df99e91](https://github.com/sece1024/manhuaviewer/commit/df99e91aaab4fd683ebaac293f4a8296c63b4dea))


### ⚙️ 杂项

- Cleanup outdated docs and add AGENTS.md([24592c5](https://github.com/sece1024/manhuaviewer/commit/24592c521c4705cecd3dbc32386547f08d9531b8))
- Add Electron scripts and dependencies([d78ae25](https://github.com/sece1024/manhuaviewer/commit/d78ae2575fb98230fab1a5e87e901ef9a9472bd2))
- 移除 Electron 相关代码，全面迁移至 Tauri([a258be2](https://github.com/sece1024/manhuaviewer/commit/a258be208b5b21fe5e1085b1000a37f6f7a21367))
- 更新 GitHub Actions，新增 Rust 格式化和 lint 检查([e8daee8](https://github.com/sece1024/manhuaviewer/commit/e8daee83715d3383ab334b2e7542f6302d0c0eea))
- 添加 Tauri 多平台自动构建发布工作流([f2866ab](https://github.com/sece1024/manhuaviewer/commit/f2866ab1a3800a94898b8f2cc25a6ceebebd1881))
- 按 tauri-action 官方推荐重构 release 工作流([977c23c](https://github.com/sece1024/manhuaviewer/commit/977c23c6b3135ecdeab7900abe7b5db164b08846))

## [betav0.0.2](https://github.com/sece1024/manhuaviewer/releases/tag/vbetav0.0.2) - 2025-04-20

### 🚀 新特性

- Next page & drag image([51683ad](https://github.com/sece1024/manhuaviewer/commit/51683ada2a5cdb0c72bdf0f42af143130a11112e))
- Build exe file([3b26c3e](https://github.com/sece1024/manhuaviewer/commit/3b26c3e63392030281a90e446564dd452fa8de4e))
- Add long image mode([38bc27d](https://github.com/sece1024/manhuaviewer/commit/38bc27de1dbf64e0b21762d42fcaac49be57d38a))


### 📚 文档

- Update readme([d630ad8](https://github.com/sece1024/manhuaviewer/commit/d630ad841452ecbe41f3e7a68d3ec30cb420b2c0))


### 💼 其他

- Config dependencies([fed23d4](https://github.com/sece1024/manhuaviewer/commit/fed23d4ee5502443c2ca1cff4d323bfb37979cab))

