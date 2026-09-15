import { useEffect, useRef } from 'react';

/**
 * useModalKeyboard — 弹层通用键盘/焦点行为：Esc 关闭、Tab 焦点环、背景滚动锁定。
 *
 * Modal 与自绘 overlay（TagPicker/CategoryPicker 等）共用，统一弹层的可访问性：
 * - Esc：关闭（capture 阶段拦截，优先于 Reader 的全局翻页快捷键，避免穿透）；
 * - Tab：焦点在弹层内循环，防止键盘焦点逃逸到背景页面；
 * - 打开时锁定 body 滚动，关闭时还原；
 * - 打开时把焦点收敛到弹层内首个可聚焦元素（尊重 autoFocus 的输入框）。
 *
 * @param {() => void} onClose  关闭回调（每次渲染取最新值，无需 useCallback）
 * @returns {import('react').RefObject} 挂到弹层面板（外层 .modal）的 ref
 */
export default function useModalKeyboard(onClose) {
  const panelRef = useRef(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    const panel = panelRef.current;
    if (!panel) return;

    // 焦点收敛：优先弹层内首个可聚焦元素（含 autoFocus 输入框），否则聚焦面板本身
    const firstFocusable = panel.querySelector(
      'input, select, textarea, button:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])'
    );
    (firstFocusable || panel).focus({ preventScroll: true });

    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';

    const onKeyDown = (e) => {
      if (e.key === 'Escape' || e.key === 'Esc') {
        e.preventDefault();
        e.stopPropagation();
        onCloseRef.current();
        return;
      }
      if (e.key !== 'Tab') return;
      const focusables = panel.querySelectorAll(
        'a[href], button:not([disabled]), input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );
      if (focusables.length === 0) return;
      const first = focusables[0];
      const last = focusables[focusables.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && (active === first || !panel.contains(active))) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    };

    // capture：在 Reader 等全局快捷键（window bubble）之前拦截，stopPropagation 阻断穿透
    document.addEventListener('keydown', onKeyDown, true);
    return () => {
      document.removeEventListener('keydown', onKeyDown, true);
      document.body.style.overflow = prevOverflow;
    };
  }, []);

  return panelRef;
}