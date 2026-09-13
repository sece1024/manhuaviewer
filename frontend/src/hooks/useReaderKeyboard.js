import { useEffect, useCallback } from 'react';

/**
 * useReaderKeyboard — 阅读器键盘快捷键 hook
 * 将键盘逻辑从 Reader.js 中分离，减少主组件依赖数量
 */
export default function useReaderKeyboard({
  goPrev,
  goNext,
  goPage,
  pagesLength,
  longImage,
  showThumbnails,
  setShowThumbnails,
  showJump,
  setShowJump,
  showHelp,
  setShowHelp,
  showMenu,
  setShowMenu,
  showTagPicker,
  setShowTagPicker,
  setDoublePage,
  setLongImage,
  setRotation,
  setFitMode,
  onFitModeChange,
  showOverlay,
  containerRef,
  doublePageDisabled,
  doublePage,
}) {
  const handler = useCallback((e) => {
    // 长按重复触发只对翻页类键有意义，且会造成连跳
    if (e.repeat && [' ', 'ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End'].includes(e.key)) return;

    const overlayOpen = showHelp || showTagPicker || showThumbnails || showJump || showMenu;
    // 任一浮层打开时：只放行 Escape 关闭，方向键/空格不再翻到底层页面
    if (overlayOpen) {
      if (e.key === 'Escape') {
        if (showHelp) setShowHelp(false);
        else if (showTagPicker) setShowTagPicker(false);
        else if (showThumbnails) setShowThumbnails(false);
        else if (showJump) setShowJump(false);
        else if (showMenu) setShowMenu(false);
      }
      return;
    }

    if (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT' || e.target.tagName === 'TEXTAREA') return;
    // 焦点在按钮上：Space/Enter 由按钮本身处理，避免“hook 翻一页 + 按钮 click 再翻一页”
    if (e.target.tagName === 'BUTTON' && (e.key === ' ' || e.key === 'Enter')) return;

    switch (e.key) {
      case 'ArrowLeft': goPrev(); break;
      case 'ArrowRight': goNext(); break;
      case 'ArrowUp': if (!longImage) goPrev(); break;
      case 'ArrowDown': if (!longImage) goNext(); break;
      case ' ': if (!longImage) { e.preventDefault(); goNext(); } break;
      case 'd': case 'D':
        // 关闭双页任何时候都允许；仅“开启”受窗口宽度/长图模式限制——
        // 否则在窄窗口（containerTooNarrow）或长图模式下开启后，D 会被同一条件锁死，退不出来。
        if (!e.ctrlKey && (doublePage || !doublePageDisabled)) {
          setDoublePage(v => {
            if (!v) setLongImage(false); // 开启双页时关闭长图
            return !v;
          });
        }
        break;
      case 'Home': goPage(0); break;
      case 'End': goPage(pagesLength - 1); break;
      case 'l': case 'L':
        if (!doublePage) {
          setLongImage(v => !v);
        }
        break;
      case 'r': case 'R':
        setRotation(r => (e.shiftKey ? (r - 90 + 360) % 360 : (r + 90) % 360));
        break;
      case 't': case 'T': setShowThumbnails(v => !v); break;
      case 'g': case 'G': setShowJump(true); break;
      case 'w': case 'W':
        setFitMode(m => {
          const next = m === 'height' ? 'width' : m === 'width' ? 'original' : 'height';
          if (onFitModeChange) onFitModeChange(next);
          showOverlay(`适应: ${next === 'height' ? '高度' : next === 'width' ? '宽度' : '原始'}`);
          return next;
        });
        break;
      case 'F1':
        e.preventDefault();
        setShowHelp(v => !v);
        break;
      case 'F11':
        e.preventDefault();
        if (document.fullscreenElement) document.exitFullscreen();
        else containerRef.current?.requestFullscreen();
        break;
      case 'Escape':
        if (showHelp) setShowHelp(false);
        else if (showTagPicker) setShowTagPicker(false);
        else if (showThumbnails) setShowThumbnails(false);
        else if (showJump) setShowJump(false);
        else if (showMenu) setShowMenu(false);
        break;
      default: break;
    }
  }, [
    goPrev, goNext, goPage, pagesLength, longImage, doublePage, doublePageDisabled,
    showThumbnails, showJump, showMenu, showHelp, showTagPicker,
    setDoublePage, setLongImage, setRotation, setFitMode, onFitModeChange, showOverlay, containerRef,
  ]);

  useEffect(() => {
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [handler]);
}
