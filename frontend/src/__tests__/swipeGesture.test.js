import {
  DOUBLE_TAP_MAX_INTERVAL_MS,
  SWIPE_MIN_DISTANCE_PX,
  SWIPE_MIN_DISTANCE_RATIO,
  isDoubleTap,
  isTap,
  resolveSwipe,
  swipeMinDistance,
} from '../utils/swipeGesture';

// 阅读区宽度：窄屏（手机）用固定阈值；宽屏（iPad 横竖屏）按比例提高
const NARROW = 375;
const WIDE = 1000; // 8% = 80px

describe('swipeMinDistance', () => {
  test('窄屏使用固定下限', () => {
    expect(swipeMinDistance(NARROW)).toBe(SWIPE_MIN_DISTANCE_PX);
  });

  test('宽屏按宽度比例提高阈值', () => {
    expect(swipeMinDistance(WIDE)).toBe(WIDE * SWIPE_MIN_DISTANCE_RATIO);
  });

  test('宽度未知（0/负数）时退回固定下限', () => {
    expect(swipeMinDistance(0)).toBe(SWIPE_MIN_DISTANCE_PX);
    expect(swipeMinDistance(-100)).toBe(SWIPE_MIN_DISTANCE_PX);
  });
});

describe('resolveSwipe 翻页方向', () => {
  test('rtl（日漫/右翻）：右滑 = 下一页，左滑 = 上一页', () => {
    expect(resolveSwipe({ dx: 200, dy: 0, dt: 200, width: NARROW }, 'rtl')).toBe('next');
    expect(resolveSwipe({ dx: -200, dy: 0, dt: 200, width: NARROW }, 'rtl')).toBe('prev');
  });

  test('ltr（左翻）：左滑 = 下一页，右滑 = 上一页', () => {
    expect(resolveSwipe({ dx: -200, dy: 0, dt: 200, width: NARROW }, 'ltr')).toBe('next');
    expect(resolveSwipe({ dx: 200, dy: 0, dt: 200, width: NARROW }, 'ltr')).toBe('prev');
  });

  test('默认按 rtl（应用默认翻页方向）处理', () => {
    expect(resolveSwipe({ dx: 200, dy: 0, dt: 200, width: NARROW })).toBe('next');
  });
});

describe('resolveSwipe 边界', () => {
  test('位移过小：交给点击区（返回 null）', () => {
    expect(resolveSwipe({ dx: 8, dy: 2, dt: 80, width: NARROW }, 'rtl')).toBeNull();
  });

  test('竖向为主：不翻页', () => {
    expect(resolveSwipe({ dx: 60, dy: 200, dt: 200, width: NARROW }, 'rtl')).toBeNull();
  });

  test('慢速滑动（>500ms）仍然翻页（回归：原 500ms 硬上限让慢滑彻底无效）', () => {
    expect(resolveSwipe({ dx: 150, dy: 10, dt: 750, width: NARROW }, 'rtl')).toBe('next');
  });

  test('超过 1s 视为长按/拖拽，不翻页', () => {
    expect(resolveSwipe({ dx: 150, dy: 10, dt: 1200, width: NARROW }, 'rtl')).toBeNull();
  });

  test('快速轻扫：位移不足阈值但速度够快也翻页', () => {
    // 25px / 30ms ≈ 0.83 px/ms，高于 0.5 的轻扫阈值
    expect(resolveSwipe({ dx: 25, dy: 4, dt: 30, width: NARROW }, 'rtl')).toBe('next');
  });

  test('大屏上按比例提高阈值：慢速 60px 不翻页，但快速 60px 仍算轻扫', () => {
    expect(resolveSwipe({ dx: 60, dy: 0, dt: 400, width: WIDE }, 'rtl')).toBeNull();
    expect(resolveSwipe({ dx: 60, dy: 0, dt: 60, width: WIDE }, 'rtl')).toBe('next');
  });

  test('时长非法（负数）不翻页', () => {
    expect(resolveSwipe({ dx: 150, dy: 0, dt: -1, width: NARROW }, 'rtl')).toBeNull();
  });
});

describe('isTap / isDoubleTap', () => {
  test('位移与时长都小才算点击', () => {
    expect(isTap({ dx: 4, dy: -3, dt: 120 })).toBe(true);
    expect(isTap({ dx: 30, dy: 0, dt: 120 })).toBe(false);
    expect(isTap({ dx: 0, dy: 30, dt: 120 })).toBe(false);
    expect(isTap({ dx: 2, dy: 2, dt: 600 })).toBe(false);
  });

  test('双击需要在时间窗内，且必须有一次前置点击', () => {
    expect(isDoubleTap(0, 1000)).toBe(false);
    expect(isDoubleTap(1000, 1000 + DOUBLE_TAP_MAX_INTERVAL_MS)).toBe(true);
    expect(isDoubleTap(1000, 1000 + DOUBLE_TAP_MAX_INTERVAL_MS + 1)).toBe(false);
  });
});
