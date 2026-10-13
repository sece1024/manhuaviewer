import { screenDeltaToLocal, swapsAxes } from '../utils/rotatePan';

/**
 * 这组用例守着"旋转之后拖动方向要对"：图片用
 * `transform: scale(s) rotate(r) translate(tx/s, ty/s)` 摆放，CSS 从右往左作用，
 * 所以本地位移在屏幕上表现为 R(r)·t。写错方向的后果是"向右拖，图片上下走"，
 * 而这一点在没旋转时看不出来（r=0 时两种写法等价）。
 */

describe('screenDeltaToLocal', () => {
  test('未旋转时原样返回', () => {
    expect(screenDeltaToLocal(60, 30, 0)).toEqual({ x: 60, y: 30 });
    expect(screenDeltaToLocal(-10, 5, 360)).toEqual({ x: -10, y: 5 });
  });

  test('顺时针 90°：屏幕向右 = 本地向下', () => {
    // 屏幕位移 (60, 0) → 本地 (0, -60)；代回 R(90°)·(0,-60) = (60, 0) ✓ 图片跟手
    expect(screenDeltaToLocal(60, 0, 90)).toEqual({ x: 0, y: -60 });
    expect(screenDeltaToLocal(0, 60, 90)).toEqual({ x: 60, y: 0 });
  });

  test('180°：两个方向都取反', () => {
    expect(screenDeltaToLocal(60, 30, 180)).toEqual({ x: -60, y: -30 });
  });

  test('270°：与 90° 相反', () => {
    expect(screenDeltaToLocal(60, 0, 270)).toEqual({ x: 0, y: 60 });
  });

  test('任意角度保持位移模长（通用分支走的是旋转矩阵，不该缩放或扭斜）', () => {
    const { x, y } = screenDeltaToLocal(3, 4, 45);
    expect(Math.hypot(x, y)).toBeCloseTo(5, 6);
  });

  test('结果里不出现 -0（会写成 CSS 的 -0px，也让相等比较变得莫名）', () => {
    expect(Object.is(screenDeltaToLocal(0, 0, 90).y, -0)).toBe(false);
    expect(Object.is(screenDeltaToLocal(-0, 0, 180).x, -0)).toBe(false);
  });

  test('非法角度按 0 处理，不产生 NaN', () => {
    expect(screenDeltaToLocal(5, 6, undefined)).toEqual({ x: 5, y: 6 });
    expect(screenDeltaToLocal(5, 6, NaN)).toEqual({ x: 5, y: 6 });
  });

  test('负角度按等价的正角度处理', () => {
    expect(screenDeltaToLocal(60, 0, -270)).toEqual(screenDeltaToLocal(60, 0, 90));
  });
});

describe('swapsAxes', () => {
  test('只有 90 / 270 交换横纵轴', () => {
    expect(swapsAxes(0)).toBe(false);
    expect(swapsAxes(180)).toBe(false);
    expect(swapsAxes(360)).toBe(false);
    expect(swapsAxes(90)).toBe(true);
    expect(swapsAxes(270)).toBe(true);
    expect(swapsAxes(-90)).toBe(true);
    expect(swapsAxes(450)).toBe(true);
  });
});
