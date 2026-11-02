import { spreadTooWide, WIDE_SPREAD_MIN_PAGE_RATIO } from '../utils/spreadFit';

const PORTRAIT = { w: 800, h: 1200 }; // 竖版页（2:3）
const LANDSCAPE = { w: 1600, h: 900 }; // 横版页（16:9），典型的跨页大图
const WIDE_SPREAD = { w: 3200, h: 900 }; // 极宽跨页

// 判据：双页时每页只有「半个阅读区宽」，只有这页比单页显示明显变小才降级。
describe('spreadTooWide 双页会把当前页挤小吗', () => {
  test('宽屏(1920×1080) + 竖版页：双页与单页都是 1080 高、720 宽 → 保持双页', () => {
    expect(spreadTooWide(PORTRAIT, { w: 1920, h: 1080 })).toBe(false);
  });

  test('1200×800 窗口（阅读区约 1150×536）+ 竖版页：两侧留白但画面没变小 → 保持双页（回归）', () => {
    // 旧实现按「页宽 < 容器宽 40%」判定：720/1150 = 31% → 被误降级成单页
    expect(spreadTooWide(PORTRAIT, { w: 1150, h: 536 })).toBe(false);
  });

  test('超宽屏(2560×1080) + 竖版页：两侧留白更多，但页高仍是 1080 → 保持双页', () => {
    expect(spreadTooWide(PORTRAIT, { w: 2560, h: 1080 })).toBe(false);
  });

  test('4:3 屏(1280×960) + 竖版页：按高度适配，页宽几乎不变 → 保持双页', () => {
    expect(spreadTooWide(PORTRAIT, { w: 1280, h: 960 })).toBe(false);
  });

  test('横版页(16:9) + 16:9 窗口：双页只剩单页宽度的 50% → 降级单页', () => {
    // 单页 scale=1.2；双页半宽 958 → 0.599 → 0.599/1.2 ≈ 0.5
    expect(spreadTooWide(LANDSCAPE, { w: 1920, h: 1080 })).toBe(true);
  });

  test('横版页 + 4:3 窗口：单页刚好铺满宽度，双页要缩到约 40% → 降级单页', () => {
    expect(spreadTooWide(LANDSCAPE, { w: 1280, h: 960 })).toBe(true);
  });

  test('极宽跨页：单页按宽度铺满，双页只剩一半宽 → 降级单页', () => {
    expect(spreadTooWide(WIDE_SPREAD, { w: 1920, h: 1080 })).toBe(true);
  });

  test('方形页：双页约为单页的 89% → 保持双页', () => {
    expect(spreadTooWide({ w: 1000, h: 1000 }, { w: 1920, h: 1080 })).toBe(false);
  });

  test('竖版页 + 竖屏窄窗口：双页只剩约 50% 宽 → 降级单页', () => {
    expect(spreadTooWide(PORTRAIT, { w: 600, h: 900 })).toBe(true);
  });

  test('尺寸或容器未就绪 → 不降级（首帧安全）', () => {
    expect(spreadTooWide(null, { w: 1920, h: 1080 })).toBe(false);
    expect(spreadTooWide(PORTRAIT, { w: 0, h: 1080 })).toBe(false);
    expect(spreadTooWide(PORTRAIT, null)).toBe(false);
  });

  test('minPageRatio 可调：横版页 50% 在 0.4 下保留、0.6 下降级', () => {
    expect(spreadTooWide(LANDSCAPE, { w: 1920, h: 1080 }, { minPageRatio: 0.4 })).toBe(false);
    expect(spreadTooWide(LANDSCAPE, { w: 1920, h: 1080 }, { minPageRatio: 0.6 })).toBe(true);
  });

  test('默认阈值常量为 0.7', () => {
    expect(WIDE_SPREAD_MIN_PAGE_RATIO).toBe(0.7);
  });
});
