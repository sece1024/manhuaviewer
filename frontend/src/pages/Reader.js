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
  const [dragging, setDragging] = useState(false);
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
  const [translate, setTranslate] = useState({ x: 0, y: 0 });
  const [showThumbnails, setShowThumbnails] = useState(false);
  const [jumpPage, setJumpPage] = useState('');
  const [showJump, setShowJump] = useState(false);

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
        // 恢复进度
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
    const PRELOAD_BEFORE = 3;
    const PRELOAD_AFTER = 6;
    const start = Math.max(0, currentIndex - PRELOAD_BEFORE);
    const end = Math.min(images.length, currentIndex + PRELOAD_AFTER + 1);

    for (let i = start; i < end; i++) {
      if (preloaded[i]) continue;
      const img = new Image();
      img.src = api.imageUrl(images[i].id);
      img.onload = () => {
        setPreloaded((prev) => ({ ...prev, [i]: true }));
      };
    }
  }, [currentIndex, images, preloaded]);

  // 快捷键
  useEffect(() => {
    const handler = (e) => {
      if (e.target.tagName === 'INPUT') return;
      const step = doublePage ? 2 : 1;
      switch (e.key) {
        case 'ArrowLeft': case 'a': case 'A':
          setCurrentIndex((i) => Math.max(0, i - step)); break;
        case 'ArrowRight': case ' ': case 'd': case 'D':
          if (e.key === 'd' || e.key === 'D') {
            if (!e.ctrlKey) { setDoublePage((v) => !v); break; }
          }
          setCurrentIndex((i) => Math.min(images.length - 1, i + step)); break;
        case 'ArrowUp':
          if (longImage) break;
          setCurrentIndex((i) => Math.max(0, i - step)); break;
        case 'ArrowDown':
          if (longImage) break;
          setCurrentIndex((i) => Math.min(images.length - 1, i + step)); break;
        case 'Home': setCurrentIndex(0); break;
        case 'End': setCurrentIndex(images.length - 1); break;
        case 'l': case 'L': setLongImage((v) => !v); break;
        case 'r': case 'R':
          setRotation((r) => (e.shiftKey ? (r - 90 + 360) % 360 : (r + 90) % 360)); break;
        case 't': case 'T': setShowThumbnails((v) => !v); break;
        case 'g': case 'G': setShowJump(true); break;
        case 'F11':
          e.preventDefault();
          if (document.fullscreenElement) document.exitFullscreen();
          else containerRef.current?.requestFullscreen(); break;
        case 'Escape':
          if (showThumbnails) setShowThumbnails(false);
          if (showJump) setShowJump(false);
          break;
        default: break;
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [doublePage, longImage, images.length, showThumbnails, showJump]);

  // 鼠标拖拽
  const handleMouseDown = (e) => {
    if (e.button === 1) { // 中键
      e.preventDefault();
      setDragging(true);
      setDragStart({ x: e.clientX - translate.x, y: e.clientY - translate.y });
    }
  };
  const handleMouseMove = (e) => {
    if (!dragging) return;
    setTranslate({ x: e.clientX - dragStart.x, y: e.clientY - dragStart.y });
  };
  const handleMouseUp = (e) => {
    if (e.button === 1) setDragging(false);
  };

  // 滚轮缩放
  const handleWheel = useCallback((e) => {
    if (longImage) return; // 长图模式下浏览器原生滚动
    e.preventDefault();
    const delta = e.deltaY > 0 ? -0.1 : 0.1;
    setScale((s) => Math.min(10, Math.max(0.1, s + delta)));
  }, [longImage]);

  // 点击区域翻页
  const handleClick = (e) => {
    if (longImage) return;
    const w = e.currentTarget.clientWidth;
    const step = doublePage ? 2 : 1;
    if (e.clientX < w / 3) {
      setCurrentIndex((i) => Math.max(0, i - step));
    } else if (e.clientX > w * 2 / 3) {
      setCurrentIndex((i) => Math.min(images.length - 1, i + step));
    }
  };

  // 双击重置缩放
  const handleDblClick = () => {
    setScale(1);
    setTranslate({ x: 0, y: 0 });
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
    transition: dragging ? 'none' : 'transform 0.1s',
    maxHeight: longImage ? 'none' : '100%',
    maxWidth: longImage ? '100%' : '100%',
    objectFit: 'contain',
    cursor: longImage ? 'default' : (scale > 1 ? 'grab' : 'default'),
    ...extra,
  });

  return (
    <div
      ref={containerRef}
      style={{ height: '100%', display: 'flex', flexDirection: 'column' }}
    >
      {/* 工具栏 */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 12, flexWrap: 'wrap' }}>
        <button className="btn btn-secondary" onClick={() => navigate('/')}>← 返回</button>
        <button className="btn btn-secondary" onClick={() => setCurrentIndex((i) => Math.max(0, i - (doublePage ? 2 : 1)))}>上一页</button>
        <button className="btn btn-secondary" onClick={() => setCurrentIndex((i) => Math.min(images.length - 1, i + (doublePage ? 2 : 1)))}>下一页</button>
        <label style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 14 }}>
          <input type="checkbox" checked={doublePage} onChange={(e) => setDoublePage(e.target.checked)} /> 双页
        </label>
        <label style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 14 }}>
          <input type="checkbox" checked={longImage} onChange={(e) => setLongImage(e.target.checked)} /> 长图
        </label>
        <button className="btn btn-secondary" onClick={() => setRotation((r) => (r + 90) % 360)}>↻ 旋转</button>
        <button className="btn btn-secondary" onClick={() => setShowThumbnails(true)}>📋 缩略图</button>
        <button className="btn btn-secondary" onClick={() => setShowJump(true)}>🔢 跳转</button>
        <button className="btn btn-secondary" onClick={() => { setScale(1); setTranslate({ x: 0, y: 0 }); }}>重置缩放</button>
        <span style={{ marginLeft: 'auto', fontSize: 13, color: 'var(--text-secondary)' }}>
          {currentIndex + 1}/{images.length} · {images[currentIndex]?.filename} · {Math.round(scale * 100)}%
        </span>
      </div>

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
          cursor: longImage ? 'default' : 'pointer',
        }}
        onClick={handleClick}
        onDoubleClick={handleDblClick}
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
      >
        {longImage ? (
          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center' }}>
            {images.map((img, i) => (
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
            />
            {currentIndex + 1 < images.length && (
              <img
                src={api.imageUrl(images[currentIndex + 1]?.id)}
                alt={images[currentIndex + 1]?.filename}
                style={imgStyle({ height: '100%' })}
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
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(120px, 1fr))', gap: 8 }}>
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
                    style={{ width: '100%', height: 100, objectFit: 'cover' }}
                    loading="lazy"
                  />
                  <div style={{ fontSize: 11, padding: 4, color: 'var(--text-secondary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {i + 1}. {img.filename}
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
            style={{ background: 'var(--bg-secondary)', borderRadius: 'var(--radius)', padding: 24 }}
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
                style={{ width: 120 }}
              />
              <button className="btn" onClick={handleJump}>跳转</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
