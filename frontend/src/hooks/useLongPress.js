import { useCallback, useEffect, useRef } from 'react';
import { isLongPress, movedBeyondSlop, LONG_PRESS_MS, LONG_PRESS_SLOP_PX } from '../utils/longPress';

/**
 * useLongPress — 元素上的长按检测。
 *
 * 用法：
 *   const { longPressProps, swallowedByLongPress } = useLongPress(() => openSheet());
 *   <div {...longPressProps} onClick={() => { if (swallowedByLongPress()) return; ... }}>
 *
 * 三件事必须一起做对，否则长按会变成 bug：
 * 1. **计时**：touchstart 起表，到 LONG_PRESS_MS 触发；
 * 2. **滑动取消**：touchmove 超出容差立刻取消——否则在列表上滑一下就弹面板；
 * 3. **吞掉随后的 click**：长按结束时浏览器仍会补一个 click，不吞掉就会"弹了面板
 *    又进了阅读器"。由调用方在 onClick 开头调用 `swallowedByLongPress()` 消费。
 *
 * 监听用 React 的 onTouch* 而不是原生非被动监听：这里只需要"读"手势，不需要
 * preventDefault（那才是必须原生 `{ passive: false }` 的场景，见 useReaderGestures）。
 * 阻止 iOS 长按弹出系统菜单/选中文本放在 CSS 里（`-webkit-touch-callout: none`）。
 */
export default function useLongPress(onLongPress, options = {}) {
  const { delay = LONG_PRESS_MS, slop = LONG_PRESS_SLOP_PX } = options;
  const timerRef = useRef(null);
  // 按下点与按下时刻；移动超容差或抬手会被清空
  const startRef = useRef(null);
  // 本次触摸是否已经触发过长按：用于吞掉随后的 click
  const firedRef = useRef(false);
  const onLongPressRef = useRef(onLongPress);
  onLongPressRef.current = onLongPress;

  const clearTimer = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const reset = useCallback(() => {
    clearTimer();
    startRef.current = null;
  }, [clearTimer]);

  useEffect(() => reset, [reset]);

  const onTouchStart = useCallback((e) => {
    const touch = e.touches && e.touches[0];
    // 多指手势（捏合等）不参与长按
    if (!touch || e.touches.length > 1) {
      reset();
      return;
    }
    firedRef.current = false;
    startRef.current = { x: touch.clientX, y: touch.clientY, at: Date.now() };
    clearTimer();
    timerRef.current = setTimeout(() => {
      timerRef.current = null;
      const start = startRef.current;
      if (!start) return;
      // 到点时再整体确认一次：够久 + 没跑远（移动本来就会取消，这里是兜底）
      if (!isLongPress({ elapsedMs: Date.now() - start.at, start, current: start }, { delay, slop })) {
        return;
      }
      firedRef.current = true;
      startRef.current = null;
      if (onLongPressRef.current) onLongPressRef.current();
    }, delay);
  }, [clearTimer, delay, reset, slop]);

  const onTouchMove = useCallback((e) => {
    const start = startRef.current;
    const touch = e.touches && e.touches[0];
    if (!start || !touch) return;
    // 只判断"跑远了就取消"：时间条件交给定时器
    if (movedBeyondSlop(start, { x: touch.clientX, y: touch.clientY }, slop)) {
      reset();
    }
  }, [reset, slop]);

  // 消费"刚刚发生过长按"的标记：返回 true 表示这次 click 应该被忽略
  const swallowedByLongPress = useCallback(() => {
    if (!firedRef.current) return false;
    firedRef.current = false;
    return true;
  }, []);

  return {
    longPressProps: {
      onTouchStart,
      onTouchMove,
      onTouchEnd: reset,
      onTouchCancel: reset,
    },
    swallowedByLongPress,
  };
}
