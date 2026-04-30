import React, { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import api from '../utils/api';
import { useToast } from '../components/Toast';

export default function Reader() {
  const { archiveId } = useParams();
  const navigate = useNavigate();
  const toast = useToast();

  const [archive, setArchive] = useState(null);
  const [pages, setPages] = useState([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [doublePage, setDoublePage] = useState(false);
  const [longImage, setLongImage] = useState(false);
  const [scale, setScale] = useState(1);
  const [rotation, setRotation] = useState(0);
  const [fitMode, setFitMode] = useState(() => localStorage.getItem('readerFit') || 'height');
  const [showThumbnails, setShowThumbnails] = useState(false);
  const [jumpPage, setJumpPage] = useState('');
  const [showJump, setShowJump] = useState(false);
  const [showHelp, setShowHelp] = useState(false);
  const [showMenu, setShowMenu] = useState(false);
  const [pageDirection, setPageDirection] = useState(() => localStorage.getItem('pageDirection') || 'rtl');
  const [overlayText, setOverlayText] = useState('');
  const overlayTimer = useRef(null);
  const [imageLoaded, setImageLoaded] = useState(false);

  const [translate, setTranslate] = useState({ x: 0, y: 0 });
  const dragRef = useRef({ active: false, startX: 0, startY: 0, origX: 0, origY: 0 });
  const touchRef = useRef({ startX: 0, startY: 0, startTime: 0, lastTapTime: 0, pinchDist: 0 });
  const containerRef = useRef(null);
  const saveTimerRef = useRef(null);
  // 预加载缓存：持有 Image 对象引用防止被 GC
  const preloadCacheRef = useRef({});

  // 显示 overlay 信息
  const showOverlay = useCallback((text) => {
    setOverlayText(text);
    clearTimeout(overlayTimer.current);
    overlayTimer.current = setTimeout(() => setOverlayText(''), 2000);
  }, []);

  // 加载数据
  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const data = await api.getPages(archiveId);
        if (cancelled) return;
        setArchive(data.archive);
        setPages(data.pages);
        if (data.read_page > 0 && data.read_page < data.pages.length) {
          setCurrentIndex(data.read_page);
        }
      } catch (e) {
        toast(e.message, 'error');
      }
    }
    load();
    return () => { cancelled = true; };
  }, [archiveId, toast]);

  // 组件卸载时清理所有定时器
  useEffect(() => {
    return () => {
      clearTimeout(overlayTimer.current);
      clearTimeout(saveTimerRef.current);
    };
  }, []);

  // 保存进度（防抖）
  useEffect(() => {
    if (!archive || pages.length === 0) return;
    clearTimeout(saveTimerRef.current);
    saveTimerRef.current = setTimeout(() => {
      api.saveHistory(parseInt(archiveId), currentIndex, pages.length).catch(() => {});
    }, 1000);
    return () => clearTimeout(saveTimerRef.current);
  }, [currentIndex, archive, pages.length, archiveId]);

  // 预加载图片（持久化引用防止 GC）
  useEffect(() => {
    if (pages.length === 0) return;
    const cache = preloadCacheRef.current;
    const start = Math.max(0, currentIndex - 2);
    const end = Math.min(pages.length, currentIndex + 5);
    for (let i = start; i < end; i++) {
      if (i === currentIndex) continue;
      const url = pages[i].url;
      if (!cache[url]) {
        const img = new Image();
        img.src = url;
        cache[url] = img;
      }
    }
    // 清理远离当前页的缓存（保留窗口外 10 页范围）
    for (const url of Object.keys(cache)) {
      const idx = pages.findIndex(p => p.url === url);
      if (idx !== -1 && (idx < currentIndex - 10 || idx > currentIndex + 10)) {
        delete cache[url];
      }
    }
  }, [currentIndex, pages]);

  // 翻页
  const goPage = useCallback((newIndex) => {
    if (newIndex < 0 || newIndex >= pages.length) return;
    setCurrentIndex(newIndex);
    setScale(1);
    setTranslate({ x: 0, y: 0 });
    setImageLoaded(false);
    showOverlay(`${newIndex + 1} / ${pages.length}`);
  }, [pages.length, showOverlay]);

  const goPrev = useCallback(() => {
    const step = doublePage ? 2 : 1;
    const dir = pageDirection === 'rtl' ? 1 : -1;
    goPage(currentIndex - step * dir);
  }, [currentIndex, doublePage, pageDirection, goPage]);

  const goNext = useCallback(() => {
    const step = doublePage ? 2 : 1;
    const dir = pageDirection === 'rtl' ? 1 : -1;
    goPage(currentIndex + step * dir);
  }, [currentIndex, doublePage, pageDirection, goPage]);

  // 快捷键
  useEffect(() => {
    const handler = (e) => {
      if (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT') return;
      const dir = pageDirection === 'rtl' ? 1 : -1;
      switch (e.key) {
        case 'ArrowLeft': goPrev(); break;
        case 'ArrowRight': goNext(); break;
        case 'ArrowUp': if (!longImage) goPrev(); break;
        case 'ArrowDown': if (!longImage) goNext(); break;
        case ' ': e.preventDefault(); goNext(); break;
        case 'd': case 'D': if (!e.ctrlKey) setDoublePage(v => !v); break;
        case 'Home': goPage(0); break;
        case 'End': goPage(pages.length - 1); break;
        case 'l': case 'L': setLongImage(v => !v); break;
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
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [goPrev, goNext, goPage, longImage, pages.length, showThumbnails, showJump, showMenu, pageDirection, showOverlay]);

  // 触摸手势
  const getTouchDist = (touches) => {
    const dx = touches[0].clientX - touches[1].clientX;
    const dy = touches[0].clientY - touches[1].clientY;
    return Math.sqrt(dx * dx + dy * dy);
  };

  const handleTouchStart = useCallback((e) => {
    if (longImage) return;
    if (e.touches.length === 2) {
      touchRef.current.pinchDist = getTouchDist(e.touches);
      return;
    }
    const t = e.touches[0];
    touchRef.current.startX = t.clientX;
    touchRef.current.startY = t.clientY;
    touchRef.current.startTime = Date.now();
    if (scale > 1.05) {
      dragRef.current = { active: true, startX: t.clientX, startY: t.clientY, origX: translate.x, origY: translate.y };
    }
  }, [longImage, scale, translate]);

  const handleTouchMove = useCallback((e) => {
    if (longImage) return;
    if (e.touches.length === 2) {
      e.preventDefault();
      const dist = getTouchDist(e.touches);
      if (touchRef.current.pinchDist > 0) {
        const ratio = dist / touchRef.current.pinchDist;
        setScale(s => Math.min(10, Math.max(0.5, s * ratio)));
        touchRef.current.pinchDist = dist;
      }
      return;
    }
    if (dragRef.current.active) {
      e.preventDefault();
      const t = e.touches[0];
      setTranslate({
        x: dragRef.current.origX + (t.clientX - dragRef.current.startX),
        y: dragRef.current.origY + (t.clientY - dragRef.current.startY),
      });
    }
  }, [longImage]);

  const handleTouchEnd = useCallback((e) => {
    if (longImage) return;
    dragRef.current.active = false;
    touchRef.current.pinchDist = 0;
    if (e.changedTouches.length === 0) return;
    const t = e.changedTouches[0];
    const dx = t.clientX - touchRef.current.startX;
    const dy = t.clientY - touchRef.current.startY;
    const dt = Date.now() - touchRef.current.startTime;

    // 双击
    if (Math.abs(dx) < 10 && Math.abs(dy) < 10 && dt < 300) {
      const now = Date.now();
      if (now - touchRef.current.lastTapTime < 300) {
        if (scale > 1.05) { setScale(1); setTranslate({ x: 0, y: 0 }); }
        else { setScale(2.5); setTranslate({ x: 0, y: 0 }); }
        touchRef.current.lastTapTime = 0;
        return;
      }
      touchRef.current.lastTapTime = now;
    }

    if (scale > 1.05) return;
    const absDx = Math.abs(dx);
    const absDy = Math.abs(dy);
    if (absDx > 50 && absDx > absDy && dt < 500) {
      if (dx > 0) goPrev(); else goNext();
    }
  }, [longImage, scale, goPrev, goNext]);

  // 鼠标
  const handleWheel = useCallback((e) => {
    if (longImage) return;
    e.preventDefault();
    const delta = e.deltaY > 0 ? -0.15 : 0.15;
    setScale(s => Math.min(10, Math.max(0.1, s + delta)));
  }, [longImage]);

  const handleMouseDown = (e) => {
    if (e.button === 1 || (e.button === 0 && scale > 1.05)) {
      e.preventDefault();
      dragRef.current = { active: true, startX: e.clientX, startY: e.clientY, origX: translate.x, origY: translate.y };
    }
  };

  const handleMouseMove = (e) => {
    if (!dragRef.current.active) return;
    setTranslate({
      x: dragRef.current.origX + (e.clientX - dragRef.current.startX),
      y: dragRef.current.origY + (e.clientY - dragRef.current.startY),
    });
  };

  const handleMouseUp = () => { dragRef.current.active = false; };

  const handleClick = (e) => {
    if (longImage || scale > 1.05) return;
    const w = e.currentTarget.clientWidth;
    const isLeft = pageDirection === 'rtl' ? e.clientX > w * 2 / 3 : e.clientX < w / 3;
    if (isLeft) goPrev();
    else if (pageDirection === 'rtl' ? e.clientX < w / 3 : e.clientX > w * 2 / 3) goNext();
  };

  const handleDblClick = () => {
    if (scale > 1.05) { setScale(1); setTranslate({ x: 0, y: 0 }); }
    else { setScale(2.5); setTranslate({ x: 0, y: 0 }); }
  };

  const handleJump = () => {
    const page = parseInt(jumpPage);
    if (page >= 1 && page <= pages.length) {
      goPage(page - 1);
      setShowJump(false);
      setJumpPage('');
    }
  };

  // 图片样式（memoized 避免每次渲染重建）
  const imgStyle = useMemo(() => {
    const base = {
      transform: `scale(${scale}) rotate(${rotation}deg) translate(${translate.x / scale}px, ${translate.y / scale}px)`,
      transition: dragRef.current.active ? 'none' : 'transform 0.15s',
      objectFit: 'contain',
      userSelect: 'none',
      pointerEvents: 'none',
    };

    if (longImage) return { ...base, maxHeight: 'none', maxWidth: '100%', width: '100%' };
    if (fitMode === 'height') return { ...base, maxHeight: '100%', maxWidth: '100%' };
    if (fitMode === 'width') return { ...base, maxWidth: '100%', height: 'auto' };
    return { ...base, maxHeight: 'none', maxWidth: 'none' };
  }, [scale, rotation, translate, longImage, fitMode]);

  if (!archive || pages.length === 0) {
    return (
      <div className="empty-state">
        <div className="empty-state-icon">⏳</div>
        <div className="empty-state-text">加载中...</div>
      </div>
    );
  }

  return (
    <div ref={containerRef} style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      {/* 工具栏 */}
      <div className="reader-toolbar">
        <button className="btn btn-secondary btn-icon" onClick={() => navigate('/')}>←</button>
        <button className="btn btn-secondary btn-icon" onClick={goPrev}>‹</button>
        <button className="btn btn-secondary btn-icon" onClick={goNext}>›</button>

        <label style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 13 }}>
          <input type="checkbox" checked={doublePage} onChange={(e) => setDoublePage(e.target.checked)} /> 双页
        </label>
        <label style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 13 }}>
          <input type="checkbox" checked={longImage} onChange={(e) => setLongImage(e.target.checked)} /> 长图
        </label>

        <div className="toolbar-group-secondary">
          <select value={fitMode} onChange={(e) => { setFitMode(e.target.value); localStorage.setItem('readerFit', e.target.value); }} style={{ minWidth: 70, fontSize: 13 }}>
            <option value="height">适应高度</option>
            <option value="width">适应宽度</option>
            <option value="original">原始大小</option>
          </select>
          <button className="btn btn-secondary btn-icon" onClick={() => setRotation(r => (r + 90) % 360)} title="旋转">↻</button>
          <button className="btn btn-secondary btn-icon" onClick={() => setShowThumbnails(true)} title="缩略图">📋</button>
          <button className="btn btn-secondary btn-icon" onClick={() => setShowJump(true)} title="跳转">🔢</button>
          <button className="btn btn-secondary btn-icon" onClick={() => { setScale(1); setTranslate({ x: 0, y: 0 }); }} title="重置缩放">1:1</button>
          <button className="btn btn-secondary btn-icon" onClick={() => setPageDirection(d => d === 'rtl' ? 'ltr' : 'rtl')} title="翻页方向">
            {pageDirection === 'rtl' ? '→←' : '←→'}
          </button>
        </div>

        {/* 移动端：折叠次要工具按钮 */}
        <button className="btn btn-secondary btn-icon toolbar-menu-btn" onClick={() => setShowMenu(v => !v)} title="更多">⋯</button>

        <span className="status-text">
          {currentIndex + 1}/{pages.length} · {Math.round(scale * 100)}% · {fitMode === 'height' ? '适应高' : fitMode === 'width' ? '适应宽' : '原始'}
        </span>
      </div>

      {/* 移动端展开菜单 */}
      {showMenu && (
        <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', padding: '8px 0', borderBottom: '1px solid var(--border)', marginBottom: 8 }}>
          <button className="btn btn-secondary btn-sm" onClick={() => { setRotation(r => (r + 90) % 360); setShowMenu(false); }}>↻ 旋转</button>
          <button className="btn btn-secondary btn-sm" onClick={() => { setShowThumbnails(true); setShowMenu(false); }}>📋 缩略图</button>
          <button className="btn btn-secondary btn-sm" onClick={() => { setShowJump(true); setShowMenu(false); }}>🔢 跳转</button>
          <button className="btn btn-secondary btn-sm" onClick={() => { setScale(1); setTranslate({ x: 0, y: 0 }); setShowMenu(false); }}>重置缩放</button>
          <select value={fitMode} onChange={(e) => { setFitMode(e.target.value); localStorage.setItem('readerFit', e.target.value); setShowMenu(false); }} style={{ minWidth: 80 }}>
            <option value="height">适应高度</option>
            <option value="width">适应宽度</option>
            <option value="original">原始大小</option>
          </select>
        </div>
      )}

      {/* 进度条 */}
      <div className="progress-bar-outer">
        <div className="progress-bar-inner" style={{ width: `${((currentIndex + 1) / pages.length) * 100}%` }} />
      </div>

      {/* 阅读区 */}
      <div
        className={`reader-container ${dragRef.current.active ? 'dragging' : ''}`}
        style={{
          background: 'var(--bg-tertiary)',
          border: '1px solid var(--border)',
          cursor: longImage ? 'default' : (scale > 1.05 ? 'grab' : 'pointer'),
          touchAction: longImage ? 'pan-y' : 'none',
        }}
        onClick={handleClick}
        onDoubleClick={handleDblClick}
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onTouchStart={handleTouchStart}
        onTouchMove={handleTouchMove}
        onTouchEnd={handleTouchEnd}
      >
        {longImage ? (
          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', touchAction: 'pan-y', width: '100%' }}>
            {pages.map((p, i) => (
              <img
                key={p.id}
                src={p.url}
                alt={p.filename}
                loading="lazy"
                style={imgStyle}
                onError={(e) => { e.target.style.display = 'none'; }}
              />
            ))}
          </div>
        ) : doublePage && currentIndex + 1 < pages.length ? (
          // 双页模式：RTL（日漫）右页=当前页，左页=下一页；LTR 反之
          <div style={{ display: 'flex', gap: 4, height: '100%', alignItems: 'center' }}>
            {pageDirection === 'rtl' ? (
              <>
                <img
                  src={pages[currentIndex + 1]?.url}
                  alt={pages[currentIndex + 1]?.filename}
                  className="reader-image"
                  style={imgStyle}
                  draggable={false}
                />
                <img
                  src={pages[currentIndex]?.url}
                  alt={pages[currentIndex]?.filename}
                  className="reader-image"
                  style={imgStyle}
                  draggable={false}
                />
              </>
            ) : (
              <>
                <img
                  src={pages[currentIndex]?.url}
                  alt={pages[currentIndex]?.filename}
                  className="reader-image"
                  style={imgStyle}
                  draggable={false}
                />
                <img
                  src={pages[currentIndex + 1]?.url}
                  alt={pages[currentIndex + 1]?.filename}
                  className="reader-image"
                  style={imgStyle}
                  draggable={false}
                />
              </>
            )}
          </div>
        ) : (
          <img
            src={pages[currentIndex]?.url}
            alt={pages[currentIndex]?.filename}
            className="reader-image"
            style={imgStyle}
            draggable={false}
            onLoad={() => setImageLoaded(true)}
          />
        )}
      </div>

      {/* Overlay */}
      <div className={`reader-overlay ${overlayText ? 'visible' : ''}`}>
        {overlayText}
      </div>

      {/* 移动端底部操作栏 */}
      <div className="mobile-bottom-bar">
        <button className="btn btn-secondary btn-sm" onClick={() => setRotation(r => (r + 90) % 360)}>↻</button>
        <button className="btn btn-secondary btn-sm" onClick={() => setShowThumbnails(true)}>📋</button>
        <button className="btn btn-secondary btn-sm" onClick={() => setShowJump(true)}>🔢</button>
        <select value={fitMode} onChange={(e) => { setFitMode(e.target.value); localStorage.setItem('readerFit', e.target.value); }} style={{ minWidth: 70, height: 36 }}>
          <option value="height">适应高度</option>
          <option value="width">适应宽度</option>
          <option value="original">原始</option>
        </select>
      </div>

      {/* 缩略图面板 */}
      {showThumbnails && (
        <div className="thumbnail-panel" onClick={() => setShowThumbnails(false)}>
          <div className="thumbnail-panel-inner" onClick={e => e.stopPropagation()}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
              <h3>缩略图 ({pages.length} 页)</h3>
              <button className="btn btn-secondary btn-sm" onClick={() => setShowThumbnails(false)}>关闭</button>
            </div>
            <div className="thumbnail-grid">
              {pages.map((p, i) => (
                <div
                  key={p.id}
                  className={`thumbnail-item ${i === currentIndex ? 'active' : ''}`}
                  onClick={() => { goPage(i); setShowThumbnails(false); }}
                >
                  <img src={p.thumb_url || p.url} alt={p.filename} loading="lazy" />
                  <div className="page-num">{i + 1}</div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* 跳转对话框 */}
      {showJump && (
        <div style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.5)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 100 }}
          onClick={() => setShowJump(false)}>
          <div style={{ background: 'var(--bg-secondary)', borderRadius: 'var(--radius)', padding: 24, minWidth: 280 }}
            onClick={e => e.stopPropagation()}>
            <h3 style={{ marginBottom: 12 }}>跳转到页 (1-{pages.length})</h3>
            <div style={{ display: 'flex', gap: 8 }}>
              <input type="number" min={1} max={pages.length} value={jumpPage}
                onChange={e => setJumpPage(e.target.value)}
                onKeyDown={e => e.key === 'Enter' && handleJump()}
                placeholder="页码" autoFocus style={{ flex: 1 }} />
              <button className="btn" onClick={handleJump}>跳转</button>
            </div>
          </div>
        </div>
      )}

      {/* 快捷键帮助面板 */}
      {showHelp && (
        <div style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.5)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 100 }}
          onClick={() => setShowHelp(false)}>
          <div style={{ background: 'var(--bg-secondary)', borderRadius: 'var(--radius)', padding: 24, maxWidth: 480, maxHeight: '80vh', overflow: 'auto' }}
            onClick={e => e.stopPropagation()}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
              <h3>⌨️ 快捷键</h3>
              <button className="btn btn-secondary btn-sm" onClick={() => setShowHelp(false)}>关闭</button>
            </div>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 14 }}>
              <tbody>
                {[
                  ['← / →', '翻页（方向取决于 RTL/LTR 设置）'],
                  ['Space', '下一页'],
                  ['↑ / ↓', '翻页（非长图模式）'],
                  ['Home / End', '第一页 / 最后一页'],
                  ['', ''],
                  ['D', '切换双页模式'],
                  ['L', '切换长图模式'],
                  ['W', '循环切换适应模式（高度/宽度/原始）'],
                  ['R / Shift+R', '旋转（顺时针/逆时针）'],
                  ['', ''],
                  ['T', '缩略图总览'],
                  ['G', '跳转到指定页'],
                  ['F1', '快捷键帮助（当前面板）'],
                  ['F11', '全屏模式'],
                  ['Esc', '关闭弹出面板'],
                ].map(([key, desc], i) => (
                  key === '' ? (
                    <tr key={i}><td colSpan={2} style={{ height: 8 }} /></tr>
                  ) : (
                    <tr key={i}>
                      <td style={{ padding: '6px 12px 6px 0', fontFamily: 'monospace', fontWeight: 600, whiteSpace: 'nowrap', color: 'var(--accent)' }}>{key}</td>
                      <td style={{ padding: '6px 0', color: 'var(--text-secondary)' }}>{desc}</td>
                    </tr>
                  )
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
