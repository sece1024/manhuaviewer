# 优化报告 #4 - 2026-03-21 15:58

## 最后一轮深度审查

### 🔴 稳定性
1. **PreloadThread.terminate() 不安全** — 应使用标志位优雅退出，terminate 可能导致资源泄漏
2. **eventFilter 中 view.parent() 可能返回 None** — 如果 viewport 的 parent 不是预期的 QGraphicsView
3. **缩略图对话框 500+ 页时一次性创建 500 个 QRunnable** — 应批量提交或限制并发

### 🟡 用户体验
4. **双页模式下右键菜单的「上一页/下一页」step 应为 2** — 目前始终 step=1
5. **缩略图对话框无页码搜索** — 大文件夹时应支持输入页码跳转
6. **最近文件夹列表不排序/不显示时间** — 按时间排序 + 显示打开时间更直观
7. **窗口标题不显示当前漫画名** — 标题栏始终是「漫画浏览器」

### 🟢 代码健壮性
8. **_save_json 的 tmp 文件在 Windows 上 rename 可能失败** — Windows 不允许覆盖已打开的文件
9. **无 __main__.py** — 无法通过 `python -m manhuaviewer` 启动
