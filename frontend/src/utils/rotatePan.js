/**
 * rotatePan.js — 旋转状态下把「屏幕拖动位移」换算成图片本地坐标的位移（纯函数，可单测）。
 *
 * 背景：图片用 `transform: scale(s) rotate(r) translate(tx/s, ty/s)` 摆放。CSS 的
 * transform 从右往左作用，所以本地位移 t 在屏幕上表现为 `R(r) · t`——也就是说
 * **一旦旋转 90°/270°，直接拿屏幕位移去改 translate，图片会朝错误方向移动**
 * （向右拖，图片上/下走）。
 *
 * 要让图片跟着手指/鼠标走，就得把屏幕位移反向旋转回去：`local = R(-r) · screen`。
 */

/**
 * @param {number} dx 屏幕坐标下的水平位移
 * @param {number} dy 屏幕坐标下的垂直位移
 * @param {number} rotation 当前旋转角度（度，顺时针为正，与 CSS rotate 一致）
 * @returns {{x:number, y:number}} 应写进 translate 的位移
 */
export function screenDeltaToLocal(dx, dy, rotation = 0) {
  const deg = Number.isFinite(rotation) ? rotation : 0;
  // 规范化到 0..359，便于判断"是否处于交换了横纵轴的角度"
  const norm = ((deg % 360) + 360) % 360;
  // -0 会在 CSS 里写成 `-0px`，也会让 `toEqual({x: 0})` 之类的比较莫名其妙地失败
  const nz = (n) => (n === 0 ? 0 : n);
  if (norm === 0) return { x: nz(dx), y: nz(dy) };
  if (norm === 90) return { x: nz(dy), y: nz(-dx) };
  if (norm === 180) return { x: nz(-dx), y: nz(-dy) };
  if (norm === 270) return { x: nz(-dy), y: nz(dx) };
  // 任意角度（目前 UI 只有 90° 的整数倍，留给后续自由旋转）
  const rad = (-norm * Math.PI) / 180;
  const cos = Math.cos(rad);
  const sin = Math.sin(rad);
  return { x: nz(dx * cos - dy * sin), y: nz(dx * sin + dy * cos) };
}

/**
 * 旋转后横纵轴是否被交换（90°/270°）。
 * 用来决定「是否允许拖动平移」：转到这两个角度时，图片的可视包围盒与布局盒不一致，
 * 超出容器的部分会被 `overflow: hidden` 裁掉，必须能手拖出来看。
 */
export function swapsAxes(rotation = 0) {
  const norm = ((Number.isFinite(rotation) ? rotation : 0) % 360 + 360) % 360;
  return norm === 90 || norm === 270;
}
