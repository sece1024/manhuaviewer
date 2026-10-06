import { useCallback, useEffect, useRef } from 'react';
import { TAP_MAX_DISTANCE_PX, isDoubleTap, isTap, resolveSwipe } from '../utils/swipeGesture';

// 手势被消费后，抑制浏览器补发的合成事件（click/dblclick）的时间窗。
// iPad Safari 在 touchend 之后仍可能补一个 click：滑动会被当成点击 → 一次滑动翻两页；
// 双击时又会补 dblclick → 与自带双击缩放互相抵消（等于双击缩放失效）。
const SYNTHETIC_EVENT_GUARD_MS = 500;

/**
 * useReaderGestures —— 阅读区手势（触摸 + 滚轮），全部走「原生非被动监听」。
 *
 * 为什么不用 React 的 onTouchMove/onWheel：React 17+ 把 touchstart/touchmove/wheel
 * 以 passive 方式挂在根容器上，handler 里的 preventDefault() 是空操作。iPad Safari 上
 * 表现为双指捏合会把整个网页一起放大、滑动时页面回弹/滚动，手势因此不准。
 * 这里在阅读区元素上直接 addEventListener(..., { passive: false })。
 *
 * 手势映射（长图模式 enabled=false，纵向滚动交给浏览器原生）：
 *   · 单指水平滑动 → onNext/onPrev（方向由 pageDirection 决定，见 utils/swipeGesture.js）
 *   · 放大（scale>1.05）后单指拖动 → onPan（与鼠标拖动一致）
 *   · 双指捏合 → onPinch(factor)
 *   · 单击 → 不拦截，放行给阅读区 onClick（左右点击区翻页）
 *   · 双击 → onDoubleTapZoom；并抑制浏览器的 dblclick
 *   · 滚轮 → onWheelZoom(delta)
 *
 * 返回值 gestureHandledRecently()：调用方在 onClick/onDoubleClick 里用它忽略「手势之后
 * 浏览器补发的合成事件」。
 */
export default function useReaderGestures({
  container,
  enabled = true,
  scale = 1,
  translate = { x: 0, y: 0 },
  pageDirection = 'rtl',
  onPrev,
  onNext,
  onPan,
  onPanStart,
  onPanEnd,
  onPinch,
  onWheelZoom,
  onDoubleTapZoom,
}) {
  // 回调与状态的最新值镜像：手势在原生事件里执行，读 ref 取最新值，
  // 这样监听只在「容器变化/模式切换」时重挂，不随每次 state 更新重挂。
  const latest = useRef({ scale, translate, pageDirection, onPrev, onNext, onPan, onPanStart, onPanEnd, onPinch, onWheelZoom, onDoubleTapZoom });
  useEffect(() => {
    latest.current = { scale, translate, pageDirection, onPrev, onNext, onPan, onPanStart, onPanEnd, onPinch, onWheelZoom, onDoubleTapZoom };
  });

  // 手势过程中的临时状态（不参与渲染）
  const gesture = useRef({
    armed: false,
    startX: 0, startY: 0, startAt: 0,
    panning: false, origX: 0, origY: 0,
    pinchDist: 0, lastTapAt: 0,
  });
  const handledAt = useRef(0);

  useEffect(() => {
    const el = container;
    if (!el || !enabled) return undefined;

    const g = gesture.current;
    const dist = (touches) => Math.hypot(
      touches[0].clientX - touches[1].clientX,
      touches[0].clientY - touches[1].clientY
    );

    const onTouchStart = (e) => {
      if (e.touches.length >= 2) {
        // 双指（捏合/后续加第二指）：本次手势不参与翻页判定 ——
        // 否则捏合结束时 changedTouches 里那根手指会被误判成一次滑动
        g.armed = false;
        g.panning = false;
        g.pinchDist = dist(e.touches);
        return;
      }
      const t = e.touches[0];
      if (!t) return;
      const s = latest.current;
      g.armed = true;
      g.startX = t.clientX;
      g.startY = t.clientY;
      g.startAt = Date.now();
      g.pinchDist = 0;
      // 放大后单指是「平移查看」，不再翻页（与鼠标拖动一致）
      g.panning = (s.scale || 1) > 1.05;
      g.origX = s.translate ? s.translate.x : 0;
      g.origY = s.translate ? s.translate.y : 0;
      // 拖动中的变换不加过渡（否则每帧都会追一段动画，跟手变糊）
      if (g.panning && s.onPanStart) s.onPanStart();
    };

    const onTouchMove = (e) => {
      const s = latest.current;
      if (e.touches.length >= 2) {
        // 双指捏合：只有非被动监听里的 preventDefault 才能真正阻止
        // Safari 连整个网页一起缩放
        e.preventDefault();
        g.armed = false;
        const d = dist(e.touches);
        if (g.pinchDist > 0 && d > 0 && s.onPinch) s.onPinch(d / g.pinchDist);
        g.pinchDist = d;
        return;
      }
      const t = e.touches[0];
      if (!t) return;
      if (g.panning) {
        e.preventDefault();
        if (s.onPan) {
          s.onPan(
            g.origX + (t.clientX - g.startX),
            g.origY + (t.clientY - g.startY)
          );
        }
        return;
      }
      // 未放大：识别到「以水平为主」的滑动后立刻阻止浏览器接管
      // （iOS 回弹/滚动、长按选择），避免滑动到一半手势被抢走
      const dx = t.clientX - g.startX;
      const dy = t.clientY - g.startY;
      if (Math.abs(dx) > TAP_MAX_DISTANCE_PX && Math.abs(dx) > Math.abs(dy)) e.preventDefault();
    };

    const onTouchEnd = (e) => {
      // armed：本次手势确实「从单指开始」才做翻页/点击判定（捏合结束不算）
      const armed = g.armed;
      const wasPanning = g.panning;
      g.armed = false;
      g.panning = false;
      g.pinchDist = 0;
      if (wasPanning) { if (armed && latest.current.onPanEnd) latest.current.onPanEnd(); return; }
      if (!armed || e.changedTouches.length === 0) return;

      const t = e.changedTouches[0];
      const dx = t.clientX - g.startX;
      const dy = t.clientY - g.startY;
      const dt = Date.now() - g.startAt;
      const s = latest.current;

      const dir = resolveSwipe({ dx, dy, dt, width: el.clientWidth || 0 }, s.pageDirection);
      if (dir) {
        handledAt.current = Date.now(); // 抑制随后合成的 click：一次滑动只翻一页
        if (dir === 'next') { if (s.onNext) s.onNext(); } else if (s.onPrev) s.onPrev();
        return;
      }

      // 单击不在这里翻页（走点击区 onClick），这里只负责双击缩放
      if (!isTap({ dx, dy, dt })) return;
      const now = Date.now();
      if (isDoubleTap(g.lastTapAt, now)) {
        g.lastTapAt = 0;
        handledAt.current = now; // 抑制合成 click/dblclick：双击缩放不再被自己抵消
        if (s.onDoubleTapZoom) s.onDoubleTapZoom();
        return;
      }
      g.lastTapAt = now;
    };

    const onTouchCancel = () => {
      const wasPanning = g.panning;
      g.armed = false;
      g.panning = false;
      g.pinchDist = 0;
      if (wasPanning && latest.current.onPanEnd) latest.current.onPanEnd();
    };

    const onWheel = (e) => {
      e.preventDefault();
      const s = latest.current;
      if (s.onWheelZoom) s.onWheelZoom(e.deltaY > 0 ? -0.15 : 0.15);
    };

    // passive: false —— preventDefault 必须真正生效（React 合成事件在这里是空操作）
    el.addEventListener('touchstart', onTouchStart, { passive: false });
    el.addEventListener('touchmove', onTouchMove, { passive: false });
    el.addEventListener('touchend', onTouchEnd, { passive: false });
    el.addEventListener('touchcancel', onTouchCancel, { passive: false });
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => {
      el.removeEventListener('touchstart', onTouchStart);
      el.removeEventListener('touchmove', onTouchMove);
      el.removeEventListener('touchend', onTouchEnd);
      el.removeEventListener('touchcancel', onTouchCancel);
      el.removeEventListener('wheel', onWheel);
    };
  }, [container, enabled]);

  return {
    /** 刚刚消费过手势（滑动/双击）→ 忽略浏览器补发的合成 click/dblclick */
    gestureHandledRecently: useCallback(
      () => Date.now() - handledAt.current < SYNTHETIC_EVENT_GUARD_MS,
      []
    ),
  };
}
