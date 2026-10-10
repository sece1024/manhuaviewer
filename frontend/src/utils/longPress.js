/**
 * longPress.js — 长按判定（纯函数，可单测）。
 *
 * 用途：触屏上给书库卡片一个「操作面板」入口。卡片上的 4 个小图标按钮靠 hover 显形，
 * 在 iPad 上根本看不见（CSS 里连 `@media (hover: none)` 都没有），所以除了给一个
 * 常显的「⋯」按钮，还要支持长按——这是触屏用户的默认直觉。
 *
 * 长按最难的不是计时，而是**别把滚动/拖动误判成长按**：手指在列表上滑动时会产生
 * 一次 touchstart，如果只按时间判定，滑到一半就会弹出面板。所以判定要同时满足
 * 「按够时间」与「手指没跑远」。
 */

// 长按阈值：500ms 是 iOS/Android 的通行值，再短会与"点击"抢手势
export const LONG_PRESS_MS = 500;
// 手指相对按下点的位移容忍（px）：超过就当成滚动/拖动，取消长按
export const LONG_PRESS_SLOP_PX = 10;

/**
 * 位移是否已超出容忍范围（含边界外）。
 * 用欧氏距离而不是单轴，因为斜向滑动同样应该取消长按。
 */
export function movedBeyondSlop(start, current, slop = LONG_PRESS_SLOP_PX) {
  if (!start || !current) return false;
  const dx = (current.x ?? 0) - (start.x ?? 0);
  const dy = (current.y ?? 0) - (start.y ?? 0);
  return Math.hypot(dx, dy) > slop;
}

// 按压时长是否达到长按阈值
export function reachedLongPress(elapsedMs, delay = LONG_PRESS_MS) {
  return Number.isFinite(elapsedMs) && elapsedMs >= delay;
}

/**
 * 综合判定：够久且没跑远才算长按。
 * `gesture` = { elapsedMs, start, current }（current 省略时视为没移动）。
 */
export function isLongPress(gesture, { delay = LONG_PRESS_MS, slop = LONG_PRESS_SLOP_PX } = {}) {
  if (!gesture) return false;
  if (movedBeyondSlop(gesture.start, gesture.current, slop)) return false;
  return reachedLongPress(gesture.elapsedMs, delay);
}
