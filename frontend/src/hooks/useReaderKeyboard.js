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
  setDoublePage,
  setLongImage,
  setRotation,
  setFitMode,
  showOverlay,
  containerRef,
  doublePageDisabled,
  doublePage,
}) {
  const handler = useCallback((e) => {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT') return;
    switch (e.key) {
      case 'ArrowLeft': goPrev(); break;
      case 'ArrowRight': goNext(); break;
      case 'ArrowUp': if (!longImage) goPrev(); break;
      case 'ArrowDown': if (!longImage) goNext(); break;
      case ' ': if (!longImage) { e.preventDefault(); goNext(); } break;
      case 'd': case 'D':
        if (!e.ctrlKey && !doublePageDisabled) {
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
          localStorage.setItem('readerFit', next);
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
        else if (showThumbnails) setShowThumbnails(false);
        else if (showJump) setShowJump(false);
        else if (showMenu) setShowMenu(false);
        break;
      default: break;
    }
  }, [
    goPrev, goNext, goPage, pagesLength, longImage, doublePage, doublePageDisabled,
    showThumbnails, showJump, showMenu, showHelp,
    setDoublePage, setLongImage, setRotation, setFitMode, showOverlay, containerRef,
  ]);

  useEffect(() => {
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [handler]);
}
