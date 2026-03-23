import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import api from '../utils/api';
import { useToast } from '../components/Toast';

export default function Reader() {
  const { folderId } = useParams();
  const navigate = useNavigate();
  const toast = useToast();

  const [folder, setFolder] = useState(null);
  const [images, setImages] = useState([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [doublePage, setDoublePage] = useState(false);
  const [longImage, setLongImage] = useState(false);
  const [scale, setScale] = useState(1);
  const [rotation, setRotation] = useState(0);
  const [preloaded, setPreloaded] = useState({});
  const [showThumbnails, setShowThumbnails] = useState(false);
  const [jumpPage, setJumpPage] = useState('');
  const [showJump, setShowJump] = useState(false);
  const [showMenu, setShowMenu] = useState(false);

  // 拖拽/触摸状态
  const [translate, setTranslate] = useState({ x: 0, y: 0 });
  const dragRef = useRef({ active: false, startX: 0, startY: 0, origX: 0, origY: 0 });

  // 触摸状态
  const touchRef = useRef({
    startX: 0, startY: 0, startTime: 0,
    lastTapTime: 0,
    pinchDist: 0,
  });

  const containerRef = useRef(null);
  const saveTimerRef = useRef(null);

  // 加载文件夹数据
  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const data = await api.getImages(folderId);
        if (cancelled) return;
        setFolder(data.folder);
        setImages(data.images);
        if (data.folder) {
          const hist = await api.getHistory();
          const entry = hist.find((h) => h.folder_id === parseInt(folderId));
          if (entry && entry.page_index < data.images.length) {
            setCurrentIndex(entry.page_index);
          }
        }
      } catch (e) {
        toast(e.message, 'error');
      }
    }
    load();
    return () => { cancelled = true; };
  }, [folderId, toast]);

  // 保存进度（防抖）
  useEffect(() => {
    if (!folder || images.length === 0) return;
    clearTimeout(saveTimerRef.current);
    saveTimerRef.current = setTimeout(() => {
      api.saveHistory(parseInt(folderId), currentIndex, images.length).catch(() => {});
    }, 1000);
  }, [currentIndex, folder, images.length, folderId]);

  // 预加载
  useEffect(() => {
    if (images.length === 0) return;
    const start = Math.max(0, currentIndex - 3);
    const end = Math.min(images.length, currentIndex + 7);
    for (let i = start; i < end; i++) {
      if (preloaded[i]) continue;
      const img = new Image();
      img.src = api.imageUrl(images[i].id);
      img.onload = () => setPreloaded((prev) => ({ ...prev, [i]: true }));
    }
  }, [currentIndex, images, preloaded]);

  // 翻页（带动画保护）
  const goPrev = useCallback(() => {
    const step = doublePage ? 2 : 1;
    setCurrentIndex((i) => Math.max(0, i - step));
  }, [doublePage]);

  const goNext = useCallback(() => {
    const step = doublePage ? 2 : 1;
    setCurrentIndex((i) => Math.min(images.length - 1, i + step));
  }, [doublePage, images.length]);

  // 快捷键
  useEffect(() => {
    const handler = (e) => {
      if (e.target.tagName === 'INPUT') return;
      switch (e.key) {
        case 'ArrowLeft': case 'a': case 'A': goPrev(); break;
        case 'ArrowRight': case ' ': goNext(); break;
        case 'd': case 'D':
          if (!e.ctrlKey) setDoublePage((v) => !v);
          break;
        case 'ArrowUp':
          if (!longImage) goPrev();
          break;
        case 'ArrowDown':
          if (!longImage) goNext();
          break;
        case 'Home': setCurrentIndex(0); break;
        case 'End': setCurrentIndex(images.length - 1); break;
        case 'l': case 'L': setLongImage((v) => !v); break;
        case 'r': case 'R':
          setRotation((r) => (e.shiftKey ? (r - 90 + 360) % 360 : (r + 90) % 360));
          break;
        case 't': case 'T': setShowThumbnails((v) => !v); break;
        case 'g': case 'G': setShowJump(true); break;
        case 'F11':
          e.preventDefault();
          if (document.fullscreenElement) document.exitFullscreen();
          else containerRef.current?.requestFullscreen();
          break;
        case 'Escape':
          if (showThumbnails) setShowThumbnails(false);
          else if (showJump) setShowJump(false);
          else if (showMenu) setShowMenu(false);
          break;
        default: break;
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [goPrev, goNext, longImage, images.length, showThumbnails, showJump, showMenu]);

  // ── 触摸手势 ──

  const getTouchDist = (touches) => {
    const dx = touches[0].clientX - touches[1].clientX;
    const dy = touches[0].clientY - touches[1].clientY;
    return Math.sqrt(dx * dx + dy * dy);
  };

  const handleTouchStart = useCallback((e) => {
    if (longImage) return; // 长图模式交给浏览器原生滚动

    if (e.touches.length === 2) {
      // 双指 — 缩放
      touchRef.current.pinchDist = getTouchDist(e.touches);
      return;
    }

    const t = e.touches[0];
    touchRef.current.startX = t.clientX;
    touchRef.current.startY = t.clientY;
    touchRef.current.startTime = Date.now();

    // 拖拽
    if (scale > 1.05) {
      dragRef.current = {
        active: true,
        startX: t.clientX,
        startY: t.clientY,
        origX: translate.x,
        origY: translate.y,
      };
    }
  }, [longImage, scale, translate]);

  const handleTouchMove = useCallback((e) => {
    if (longImage) return;

    if (e.touches.length === 2) {
      e.preventDefault();
      const dist = getTouchDist(e.touches);
      if (touchRef.current.pinchDist > 0) {
        const ratio = dist / touchRef.current.pinchDist;
        setScale((s) => Math.min(10, Math.max(0.5, s * ratio)));
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

    // 双击检测
    if (Math.abs(dx) < 10 && Math.abs(dy) < 10 && dt < 300) {
      const now = Date.now();
      if (now - touchRef.current.lastTapTime < 300) {
        // 双击 → 重置缩放
        if (scale > 1.05) {
          setScale(1);
          setTranslate({ x: 0, y: 0 });
        } else {
          setScale(2);
          setTranslate({ x: 0, y: 0 });
        }
        touchRef.current.lastTapTime = 0;
        return;
      }
      touchRef.current.lastTapTime = now;
    }

    // 已在拖拽时不触发翻页
    if (scale > 1.05) return;

    // 滑动翻页
    const absDx = Math.abs(dx);
    const absDy = Math.abs(dy);
    if (absDx > 50 && absDx > absDy && dt < 500) {
      if (dx > 0) goPrev(); else goNext();
    }
  }, [longImage, scale, goPrev, goNext]);

  // 鼠标事件（桌面端）
  const handleWheel = useCallback((e) => {
    if (longImage) return;
    e.preventDefault();
    const delta = e.deltaY > 0 ? -0.1 : 0.1;
    setScale((s) => Math.min(10, Math.max(0.1, s + delta)));
  }, [longImage]);

  const handleMouseDown = (e) => {
    if (e.button === 1 || (e.button === 0 && scale > 1.05)) {
      e.preventDefault();
      dragRef.current = {
        active: true,
        startX: e.clientX,
        startY: e.clientY,
        origX: translate.x,
        origY: translate.y,
      };
    }
  };

  const handleMouseMove = (e) => {
    if (!dragRef.current.active) return;
    setTranslate({
      x: dragRef.current.origX + (e.clientX - dragRef.current.startX),
      y: dragRef.current.origY + (e.clientY - dragRef.current.startY),
    });
  };

  const handleMouseUp = () => {
    dragRef.current.active = false;
  };

  // 点击区域翻页（桌面端单击）
  const handleClick = (e) => {
    if (longImage || scale > 1.05) return;
    // 检查是否刚滑动过
    const w = e.currentTarget.clientWidth;
    const step = doublePage ? 2 : 1;
    if (e.clientX < w / 3) {
      setCurrentIndex((i) => Math.max(0, i - step));
    } else if (e.clientX > w * 2 / 3) {
      setCurrentIndex((i) => Math.min(images.length - 1, i + step));
    }
  };

  const handleDblClick = () => {
    if (scale > 1.05) {
      setScale(1);
      setTranslate({ x: 0, y: 0 });
    } else {
      setScale(2);
      setTranslate({ x: 0, y: 0 });
    }
  };

  const handleJump = () => {
    const page = parseInt(jumpPage);
    if (page >= 1 && page <= images.length) {
      setCurrentIndex(page - 1);
      setShowJump(false);
      setJumpPage('');
    }
  };

  if (!folder || images.length === 0) {
    return <div style={{ textAlign: 'center', padding: 60 }}>加载中...</div>;
  }

  const imgStyle = (extra = {}) => ({
    transform: `scale(${scale}) rotate(${rotation}deg) translate(${translate.x / scale}px, ${translate.y / scale}px)`,
    transition: dragRef.current.active ? 'none' : 'transform 0.1s',
    maxHeight: longImage ? 'none' : '100%',
    maxWidth: longImage ? '100%' : '100%',
    objectFit: 'contain',
    userSelect: 'none',
    ...extra,
  });

  return (
    <div
      ref={containerRef}
      style={{ height: '100%', display: 'flex', flexDirection: 'column' }}
    >
      {/* 顶部工具栏 */}
      <div className="reader-toolbar">
        <button className="btn btn-secondary" onClick={() => navigate('/')}>←</button>
        <button className="btn btn-secondary" onClick={goPrev}>‹</button>
        <button className="btn btn-secondary" onClick={goNext}>›</button>

        <label style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 14, whiteSpace: 'nowrap' }}>
          <input type="checkbox" checked={doublePage} onChange={(e) => setDoublePage(e.target.checked)} /> 双页
        </label>
        <label style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 14, whiteSpace: 'nowrap' }}>
          <input type="checkbox" checked={longImage} onChange={(e) => setLongImage(e.target.checked)} /> 长图
        </label>

        {/* 桌面端显示的额外按钮 */}
        <div className="toolbar-group-secondary" style={{ display: 'contents' }}>
          <button className="btn btn-secondary" onClick={() => setRotation((r) => (r + 90) % 360)}>↻</button>
          <button className="btn btn-secondary" onClick={() => setShowThumbnails(true)}>📋</button>
          <button className="btn btn-secondary" onClick={() => setShowJump(true)}>🔢</button>
          <button className="btn btn-secondary" onClick={() => { setScale(1); setTranslate({ x: 0, y: 0 }); }}>1:1</button>
        </div>

        {/* 移动端更多菜单按钮 */}
        <button
          className="btn btn-secondary btn-icon"
          style={{ display: 'none' }}
          onClick={() => setShowMenu((v) => !v)}
          id="menu-toggle"
        >
          ⋯
        </button>

        <span className="status-text">
          {currentIndex + 1}/{images.length} · {Math.round(scale * 100)}%
        </span>
      </div>

      {/* 移动端展开菜单 */}
      {showMenu && (
        <div style={{
          display: 'flex', gap: 8, flexWrap: 'wrap', padding: '8px 0',
          borderBottom: '1px solid var(--border)', marginBottom: 8,
        }}>
          <button className="btn btn-secondary" onClick={() => { setRotation((r) => (r + 90) % 360); setShowMenu(false); }}>↻ 旋转</button>
          <button className="btn btn-secondary" onClick={() => { setShowThumbnails(true); setShowMenu(false); }}>📋 缩略图</button>
          <button className="btn btn-secondary" onClick={() => { setShowJump(true); setShowMenu(false); }}>🔢 跳转</button>
          <button className="btn btn-secondary" onClick={() => { setScale(1); setTranslate({ x: 0, y: 0 }); setShowMenu(false); }}>重置缩放</button>
          <select value={scale} onChange={(e) => { setScale(parseFloat(e.target.value)); setShowMenu(false); }} style={{ minWidth: 80 }}>
            <option value={0.5}>50%</option>
            <option value={0.75}>75%</option>
            <option value={1}>100%</option>
            <option value={1.5}>150%</option>
            <option value={2}>200%</option>
          </select>
        </div>
      )}

      {/* 进度条 */}
      <div className="progress-bar-outer">
        <div className="progress-bar-inner" style={{ width: `${((currentIndex + 1) / images.length) * 100}%` }} />
      </div>

      {/* 图片显示区 */}
      <div
        style={{
          flex: 1,
          display: 'flex',
          justifyContent: 'center',
          alignItems: longImage ? 'flex-start' : 'center',
          overflow: longImage ? 'auto' : 'hidden',
          background: 'var(--bg-secondary)',
          border: '1px solid var(--border)',
          borderRadius: 'var(--radius)',
          marginTop: 8,
          cursor: longImage ? 'default' : (scale > 1.05 ? 'grab' : 'pointer'),
          touchAction: longImage ? 'pan-y' : 'none',
          position: 'relative',
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
          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', touchAction: 'pan-y' }}>
            {images.map((img) => (
              <img
                key={img.id}
                src={api.imageUrl(img.id)}
                alt={img.filename}
                loading="lazy"
                style={imgStyle({ width: '100%' })}
                onError={(e) => { e.target.style.display = 'none'; }}
              />
            ))}
          </div>
        ) : doublePage ? (
          <div style={{ display: 'flex', gap: 8, height: '100%', alignItems: 'center' }}>
            <img
              src={api.imageUrl(images[currentIndex]?.id)}
              alt={images[currentIndex]?.filename}
              style={imgStyle({ height: '100%' })}
              draggable={false}
            />
            {currentIndex + 1 < images.length && (
              <img
                src={api.imageUrl(images[currentIndex + 1]?.id)}
                alt={images[currentIndex + 1]?.filename}
                style={imgStyle({ height: '100%' })}
                draggable={false}
              />
            )}
          </div>
        ) : (
          <img
            src={api.imageUrl(images[currentIndex]?.id)}
            alt={images[currentIndex]?.filename}
            style={imgStyle({ maxHeight: '100%' })}
            draggable={false}
          />
        )}
      </div>

      {/* 移动端底部操作栏 */}
      <div className="mobile-bottom-bar">
        <button className="btn btn-secondary" onClick={() => setRotation((r) => (r + 90) % 360)}>↻ 旋转</button>
        <button className="btn btn-secondary" onClick={() => setShowThumbnails(true)}>📋</button>
        <button className="btn btn-secondary" onClick={() => setShowJump(true)}>🔢</button>
        <select
          value={scale}
          onChange={(e) => setScale(parseFloat(e.target.value))}
          style={{ minWidth: 70, height: 40 }}
        >
          <option value={0.5}>50%</option>
          <option value={0.75}>75%</option>
          <option value={1}>100%</option>
          <option value={1.5}>150%</option>
          <option value={2}>200%</option>
        </select>
      </div>

      {/* 缩略图面板 */}
      {showThumbnails && (
        <div
          style={{
            position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.7)',
            display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 100,
          }}
          onClick={() => setShowThumbnails(false)}
        >
          <div
            style={{
              background: 'var(--bg-secondary)', borderRadius: 'var(--radius)',
              padding: 20, width: '90%', maxHeight: '80vh', overflow: 'auto',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 16 }}>
              <h3>缩略图 ({images.length} 页)</h3>
              <button className="btn btn-secondary" onClick={() => setShowThumbnails(false)}>关闭</button>
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(100px, 1fr))', gap: 8 }}>
              {images.map((img, i) => (
                <div
                  key={img.id}
                  style={{
                    cursor: 'pointer',
                    border: i === currentIndex ? '3px solid var(--accent)' : '1px solid var(--border)',
                    borderRadius: 4,
                    overflow: 'hidden',
                    textAlign: 'center',
                  }}
                  onClick={() => { setCurrentIndex(i); setShowThumbnails(false); }}
                >
                  <img
                    src={api.thumbUrl(img.id)}
                    alt={img.filename}
                    style={{ width: '100%', height: 90, objectFit: 'cover' }}
                    loading="lazy"
                  />
                  <div style={{ fontSize: 11, padding: 4, color: 'var(--text-secondary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {i + 1}
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* 跳转对话框 */}
      {showJump && (
        <div
          style={{
            position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.5)',
            display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 100,
          }}
          onClick={() => setShowJump(false)}
        >
          <div
            style={{ background: 'var(--bg-secondary)', borderRadius: 'var(--radius)', padding: 24, minWidth: 280 }}
            onClick={(e) => e.stopPropagation()}
          >
            <h3 style={{ marginBottom: 12 }}>跳转到页 (1-{images.length})</h3>
            <div style={{ display: 'flex', gap: 8 }}>
              <input
                type="number"
                min={1}
                max={images.length}
                value={jumpPage}
                onChange={(e) => setJumpPage(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleJump()}
                placeholder="页码"
                autoFocus
                style={{ flex: 1 }}
              />
              <button className="btn" onClick={handleJump}>跳转</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
