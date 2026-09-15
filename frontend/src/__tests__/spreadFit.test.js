import { spreadTooWide, WIDE_SPREAD_MIN_PAGE_RATIO } from '../utils/spreadFit';

const PORTRAIT = { w: 800, h: 1200 }; // 竖版漫画页（2:3）
const LANDSCAPE = { w: 1600, h: 900 }; // 横版漫画页（16:9）

describe('spreadTooWide 跨页过宽判定', () => {
  test('宽屏(16:9) + 竖版漫画：contain 后每页 720px = 37.5% < 40% → 降级单页', () => {
    // 跨页 1604×1200 适配进 1920×1080：scale=min(1.197, 0.9)=0.9 → 页宽 720
    expect(spreadTooWide(PORTRAIT, PORTRAIT, { w: 1920, h: 1080 })).toBe(true);
  });

  test('超宽屏(21:9) + 竖版漫画 → 降级单页', () => {
    expect(spreadTooWide(PORTRAIT, PORTRAIT, { w: 2560, h: 1080 })).toBe(true);
  });

  test('4:3 屏 + 竖版漫画：按宽度适配每页约 50% → 保持双页', () => {
    expect(spreadTooWide(PORTRAIT, PORTRAIT, { w: 1280, h: 960 })).toBe(false);
  });

  test('竖屏窗口 + 竖版漫画：宽度适配每页约 50% → 保持双页', () => {
    expect(spreadTooWide(PORTRAIT, PORTRAIT, { w: 700, h: 900 })).toBe(false);
  });

  test('横版漫画跨页（宽 32:9）宽度适配后每页 50% → 不降级', () => {
    expect(spreadTooWide(LANDSCAPE, LANDSCAPE, { w: 1920, h: 1080 })).toBe(false);
  });

  test('跨页另一张未知时按同尺寸近似，结果与已知一致', () => {
    expect(spreadTooWide(PORTRAIT, null, { w: 1920, h: 1080 }))
      .toBe(spreadTooWide(PORTRAIT, PORTRAIT, { w: 1920, h: 1080 }));
  });

  test('当前页比另一张窄：下一页更宽使整跨页更高 → 当前页更小 → 降级', () => {
    // 跨页 1568×1200，高受限 scale=0.9 → 窄页适配 450px < 768
    expect(spreadTooWide({ w: 500, h: 1200 }, { w: 1064, h: 1200 }, { w: 1920, h: 1080 })).toBe(true);
  });

  test('尺寸或容器未就绪 → 不降级（首帧安全）', () => {
    expect(spreadTooWide(null, PORTRAIT, { w: 1920, h: 1080 })).toBe(false);
    expect(spreadTooWide(PORTRAIT, PORTRAIT, { w: 0, h: 1080 })).toBe(false);
    expect(spreadTooWide(PORTRAIT, PORTRAIT, null)).toBe(false);
  });

  test('minPageRatio 可调', () => {
    expect(spreadTooWide(PORTRAIT, PORTRAIT, { w: 1920, h: 1080 }, { minPageRatio: 0.3 })).toBe(false);
    expect(spreadTooWide(PORTRAIT, PORTRAIT, { w: 1920, h: 1080 }, { minPageRatio: 0.5 })).toBe(true);
  });

  test('默认阈值常量为 0.4', () => {
    expect(WIDE_SPREAD_MIN_PAGE_RATIO).toBe(0.4);
  });
});