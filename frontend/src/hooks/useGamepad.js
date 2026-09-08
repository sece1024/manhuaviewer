import { useEffect, useRef } from 'react';

/**
 * useGamepad — 简易游戏手柄/翻页器支持（Edge 检测，长按只触发一次）。
 * 右方向/右肩键/右扳机/主键(A) = 下一页，左方向/左肩键/左扳机 = 上一页。
 * 桌面阅读时用手柄或 USB 翻页器翻漫画很常见，这是成本最低的实现。
 */
export default function useGamepad({ goPrev, goNext, enabled = true }) {
  const edgeRef = useRef({ next: false, prev: false });

  useEffect(() => {
    if (!enabled) return;
    if (typeof navigator === 'undefined' || !('getGamepads' in navigator)) return;

    let stopped = false;
    const tick = () => {
      if (stopped) return;
      let next = false;
      let prev = false;
      let pads = [];
      try {
        pads = navigator.getGamepads();
      } catch (e) {
        /* 某些 WebView 未实现时忽略 */
      }
      for (const pad of pads) {
        if (!pad || !pad.buttons) continue;
        const b = pad.buttons;
        if ((b[15] && b[15].pressed) || (b[6] && b[6].pressed) || (b[0] && b[0].pressed)) next = true;
        if ((b[14] && b[14].pressed) || (b[4] && b[4].pressed)) prev = true;
      }
      const was = edgeRef.current;
      if (next && !was.next) goNext();
      if (prev && !was.prev) goPrev();
      edgeRef.current = { next, prev };
    };

    // 手柄按键只在轮询采样时可见，150ms 足够跟手又不会太频繁
    const timer = setInterval(tick, 150);
    return () => {
      stopped = true;
      clearInterval(timer);
      edgeRef.current = { next: false, prev: false };
    };
  }, [goPrev, goNext, enabled]);
}
