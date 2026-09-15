// 双页跨页“过宽”→ 自动单页降级判定。
// 双页模式（适应高度）下，整跨页按 contain 等比适配进视口；若适配后
// 当前页的显示宽度仍低于容器宽度的阈值比例，说明两页并排会让页面小到
// 难以阅读（典型：横版漫画跨页、宽屏显示器上的竖版漫画），阅读器应
// 自动改为单页显示当前页。
export const WIDE_SPREAD_MIN_PAGE_RATIO = 0.4; // 适配后单页宽度 < 容器宽度 40% 即视为过宽

/**
 * @param {object|null} dims       当前页原始尺寸 { w, h }（naturalWidth/naturalHeight）；尚未加载返回 false
 * @param {object|null} otherDims  跨页另一张的原始尺寸；未知时按与当前页同尺寸近似
 * @param {object|null} container  阅读区可视尺寸 { w, h }；未知（首帧未测量）返回 false，避免误判
 * @param {object}      opts       { gap = 4, minPageRatio = WIDE_SPREAD_MIN_PAGE_RATIO }
 * @returns {boolean}              true = 过宽，应降级为单页显示
 */
export function spreadTooWide(dims, otherDims, container, { gap = 4, minPageRatio = WIDE_SPREAD_MIN_PAGE_RATIO } = {}) {
  if (!dims || dims.w <= 0 || dims.h <= 0) return false;
  if (!container || container.w <= 0 || container.h <= 0) return false;

  const otherW = otherDims ? otherDims.w : dims.w;
  const otherH = otherDims ? otherDims.h : dims.h;
  const spreadW = dims.w + otherW + gap;
  const spreadH = Math.max(dims.h, otherH);

  // contain：整体等比适配，取宽度/高度两个比例中较小者
  const scale = Math.min(container.w / spreadW, container.h / spreadH);
  const fittedW = dims.w * scale;
  return fittedW < container.w * minPageRatio;
}