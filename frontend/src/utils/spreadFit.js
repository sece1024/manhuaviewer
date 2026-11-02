// 双页跨页「过宽」→ 自动单页降级判定。
//
// 判据不是「单页宽度占阅读区的比例」：宽屏上竖版页本来就只占阅读区宽度三分之一左右，
// 两侧留白是「适应高度」的正常结果——按容器比例判会把这种完全放得下的双页错误降级
// （旧实现 1920×1080 下 800×1200 的竖版页只有 37.5% 宽，于是永远被强制单页）。
// 真正值得降级的只有一种情况：两页并排后**这一页比单页显示时明显变小**。
// 双页布局里每页可用的是「半个阅读区宽 × 阅读区高」的盒子（.reader-spread-side），
// 单页则是整个阅读区；比较这两个 contain 比例即可。典型会命中的是横版跨页：
// 一页铺满宽度的画面被塞进半个宽度里，缩得太多，不如单页看。
export const WIDE_SPREAD_MIN_PAGE_RATIO = 0.7; // 双页时页宽 < 单页时的 70% 才降级

/**
 * @param {object|null} dims      当前页原始尺寸 { w, h }（naturalWidth/naturalHeight）；尚未加载返回 null
 * @param {object|null} container 阅读区可视尺寸 { w, h }；未知（首帧未测量）返回 false，避免误判
 * @param {object}      opts      { gap = 4, minPageRatio = WIDE_SPREAD_MIN_PAGE_RATIO }
 * @returns {boolean}             true = 两页并排会明显缩小当前页，应降级为单页显示
 */
export function spreadTooWide(dims, container, { gap = 4, minPageRatio = WIDE_SPREAD_MIN_PAGE_RATIO } = {}) {
  if (!dims || dims.w <= 0 || dims.h <= 0) return false;
  if (!container || container.w <= 0 || container.h <= 0) return false;

  // 单页：整个阅读区做 contain 盒子
  const singleScale = Math.min(container.w / dims.w, container.h / dims.h);
  // 双页：左右各一半（中间留 gap）做 contain 盒子
  const sideW = Math.max(0, (container.w - gap) / 2);
  const doubleScale = Math.min(sideW / dims.w, container.h / dims.h);

  return doubleScale < singleScale * minPageRatio;
}
