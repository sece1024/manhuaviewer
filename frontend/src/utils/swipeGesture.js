// 阅读器触摸滑动的「判定逻辑」（纯函数，与 DOM 无关，便于单测）。
//
// 为什么单独拆出来：Reader 原先用 React 合成事件（onTouchStart/onTouchMove）做手势，
// 而 React 17+ 把 touchstart/touchmove/wheel 挂在根容器上时用的是 passive 监听，
// handler 里的 preventDefault() 实际是空操作（浏览器只在控制台警告）。iPad Safari 上的
// 后果是：双指捏合除缩放图片外还会把整个网页一起放大、滑动过程中页面被浏览器接管
// （回弹/滚动），手势抖动、翻页方向也不对。
//
// 原生非被动监听在 hooks/useReaderGestures.js 里；这里只保留可单测的判定规则。

// —— 滑动翻页 ——
// 位移阈值取「固定像素」与「阅读区宽度比例」的较大者：iPad 上 40px 对宽屏只是
// 轻微抖动，容易误翻页；窄屏（手机）则用固定下限，避免阈值过小。
export const SWIPE_MIN_DISTANCE_PX = 40;
export const SWIPE_MIN_DISTANCE_RATIO = 0.08;
// 轻扫（flick）：速度够快时允许更短位移 —— iPad 上「甩一下」常常只有 20~40px
export const FLICK_MIN_DISTANCE_PX = 20;
export const FLICK_MIN_VELOCITY = 0.5; // px/ms
// 时长上限：原实现写死 500ms，iPad 上稍慢一点的滑动（很常见）会被整段丢弃，
// 表现为「滑了没反应」。放宽到 1s，超过才算长按/拖拽。
export const SWIPE_MAX_DURATION_MS = 1000;
// —— 点击 / 双击 ——
export const TAP_MAX_DISTANCE_PX = 12;
export const TAP_MAX_DURATION_MS = 300;
export const DOUBLE_TAP_MAX_INTERVAL_MS = 320;

/**
 * 滑动翻页所需的水平位移阈值（px）：大屏按阅读区宽度比例提高。
 * @param {number} containerWidth 阅读区宽度（px），未知传 0
 */
export function swipeMinDistance(containerWidth = 0) {
  const byRatio = (containerWidth > 0 ? containerWidth : 0) * SWIPE_MIN_DISTANCE_RATIO;
  return Math.max(SWIPE_MIN_DISTANCE_PX, byRatio);
}

/**
 * 一次手指滑动 → 翻页方向。
 *
 * 方向跟随「翻页方向」设置，与阅读区点击分区一致（rtl 点左侧 = 下一页）：
 *   · rtl（日漫/右翻）：下一页在左边，往前翻是往右翻 → 右滑 = next
 *   · ltr（左翻）      ：下一页在右边，往前翻是往左翻 → 左滑 = next
 * 双页模式的左右页布局（rtl 下 currentIndex+1 在左）也与之一致。
 *
 * @param {object} g { dx, dy, dt, width }
 *        dx/dy 手指位移（右/下为正，px）；dt 手势时长（ms）；width 阅读区宽度（px）
 * @param {string} pageDirection 'rtl' | 'ltr'
 * @returns {'next'|'prev'|null} null = 不构成翻页手势（交给点击/滚动）
 */
export function resolveSwipe({ dx, dy, dt, width = 0 }, pageDirection = 'rtl') {
  const absDx = Math.abs(dx);
  const absDy = Math.abs(dy);
  if (absDx < FLICK_MIN_DISTANCE_PX) return null; // 位移太小：交给点击区
  if (absDy > absDx) return null; // 以竖向为主：不是翻页意图
  if (!(dt >= 0) || dt > SWIPE_MAX_DURATION_MS) return null;

  const velocity = absDx / Math.max(1, dt);
  const far = absDx >= swipeMinDistance(width);
  const flick = velocity >= FLICK_MIN_VELOCITY;
  if (!far && !flick) return null;

  const forward = pageDirection === 'rtl' ? dx > 0 : dx < 0;
  return forward ? 'next' : 'prev';
}

/**
 * 是否算「点击」（位移与时长都在阈值内）：用于双击判定与区分滑动。
 */
export function isTap({ dx, dy, dt }) {
  return Math.abs(dx) < TAP_MAX_DISTANCE_PX
    && Math.abs(dy) < TAP_MAX_DISTANCE_PX
    && dt >= 0 && dt < TAP_MAX_DURATION_MS;
}

/**
 * 是否与上一次点击构成双击。prevAt 为 0 表示没有上一次点击。
 */
export function isDoubleTap(prevAt, nowAt, interval = DOUBLE_TAP_MAX_INTERVAL_MS) {
  return prevAt > 0 && nowAt - prevAt <= interval;
}
