# 优化报告 #3 - 2026-03-21 15:54

## 深度审查发现

### 🔴 代码质量 / Bug 风险
1. **全屏模式 Escape 和 F11 互斥逻辑不完整** — 按 F11 进入全屏后，菜单栏快捷键（如 Ctrl+O）仍能触发但看不见菜单
2. **preloaded_images 在 load_folder 时 clear() 后没有保护** — 如果 PreloadThread 仍在运行，可能向已清空的 cache 写入旧数据
3. **_apply_bg_color 覆盖了全局样式** — `view.setStyleSheet()` 会覆盖主题样式表中的 QGraphicsView 部分，主题切换时背景色可能残留
4. **双页模式翻页 step=2 但首尾边界不对称** — 在第 0 页按「上一页」不动（max=0），但在最后一页按「下一页」会跳到 total-1 而非停住
5. **LRUCache.lock 在 put/get 期间持有，如果调用方在锁内做 IO 会阻塞**

### 🟡 用户体验
6. **打开空文件夹后 prev/next 按钮 disabled，但快捷键仍可触发翻页**
7. **缩略图对话框不支持键盘导航** — 应支持 ↑↓←→ 选择、Enter 确认
8. **标签管理对话框中「快速添加」按钮的 lambda 捕获有问题** — `lambda checked, t=tag` 在 PyQt5 中 checked 参数可能导致 t 不被正确传入
9. **最近文件列表不显示文件夹图片数量** — 用户无法区分空文件夹和有内容的文件夹
10. **设置中的 scale 滑块对长图模式无意义** — 长图模式下缩放逻辑不同于普通模式

### 🟢 性能
11. **_load_current 每次都 os.path.getsize** — 翻页频繁调用时产生大量 syscalls，应缓存
12. **QDir.entryList 对大目录排序效率未知** — 应考虑使用 os.scandir + sorted 替代
