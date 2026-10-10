import {
  movedBeyondSlop,
  reachedLongPress,
  isLongPress,
  LONG_PRESS_MS,
  LONG_PRESS_SLOP_PX,
} from '../utils/longPress';

/**
 * 长按判定的回归点只有一个但很致命：滑动列表时不能弹面板。
 * 所以这里的用例围着「按够时间」与「手指没跑远」两个条件分别取证。
 */

describe('movedBeyondSlop', () => {
  test('没动或小幅抖动都算没跑远', () => {
    expect(movedBeyondSlop({ x: 0, y: 0 }, { x: 0, y: 0 })).toBe(false);
    expect(movedBeyondSlop({ x: 0, y: 0 }, { x: 3, y: 4 })).toBe(false); // 距离 5 < 10
  });

  test('超过容差即算跑远（斜向同样成立）', () => {
    expect(movedBeyondSlop({ x: 0, y: 0 }, { x: 11, y: 0 })).toBe(true);
    expect(movedBeyondSlop({ x: 0, y: 0 }, { x: 8, y: 8 })).toBe(true); // 距离 ≈11.3
  });

  test('恰好等于容差不算跑远（边界留在长按一侧）', () => {
    expect(movedBeyondSlop({ x: 0, y: 0 }, { x: LONG_PRESS_SLOP_PX, y: 0 })).toBe(false);
  });

  test('缺失坐标按原点处理，不抛错', () => {
    expect(movedBeyondSlop(null, { x: 1, y: 1 })).toBe(false);
    expect(movedBeyondSlop({ x: 0, y: 0 }, undefined)).toBe(false);
    expect(movedBeyondSlop({}, { x: 20, y: 0 })).toBe(true);
  });
});

describe('reachedLongPress', () => {
  test('达到阈值才算', () => {
    expect(reachedLongPress(LONG_PRESS_MS - 1)).toBe(false);
    expect(reachedLongPress(LONG_PRESS_MS)).toBe(true);
    expect(reachedLongPress(LONG_PRESS_MS + 1)).toBe(true);
  });

  test('非法时长不触发', () => {
    expect(reachedLongPress(NaN)).toBe(false);
    expect(reachedLongPress(undefined)).toBe(false);
  });
});

describe('isLongPress', () => {
  const gesture = (elapsedMs, to = { x: 0, y: 0 }) => ({
    elapsedMs, start: { x: 0, y: 0 }, current: to,
  });

  test('按够时间且没移动 = 长按', () => {
    expect(isLongPress(gesture(600))).toBe(true);
  });

  test('时间够但手指滑走了 = 不是长按（滚动列表不该弹面板）', () => {
    expect(isLongPress(gesture(600, { x: 0, y: 60 }))).toBe(false);
  });

  test('时间不够 = 不是长按（点击不该弹面板）', () => {
    expect(isLongPress(gesture(200))).toBe(false);
  });

  test('可覆写阈值（便于调用方调手感）', () => {
    expect(isLongPress(gesture(200), { delay: 150 })).toBe(true);
    // 同样是位移 6px：默认容差 10 算没跑远，收紧到 5 就算跑远
    expect(isLongPress(gesture(600, { x: 6, y: 0 }))).toBe(true);
    expect(isLongPress(gesture(600, { x: 6, y: 0 }), { slop: 5 })).toBe(false);
  });

  test('空手势不触发', () => {
    expect(isLongPress(null)).toBe(false);
    expect(isLongPress(undefined)).toBe(false);
  });
});
