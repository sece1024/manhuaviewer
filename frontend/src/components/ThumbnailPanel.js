import React, { useEffect, useRef, useState } from 'react';

// 缩略图面板：虚拟窗口渲染——格子的占位始终存在（撑出滚动条与滚动位置），
// 只有进入可视范围 ±THUMB_OVERSCAN 的格子才真正挂载 <img>。
// 此前是"每次追加 100 个直到整本挂满"，2000 页最终会有 2000 个 <img> + 2000 个请求。
const THUMB_OVERSCAN = 12;

/**
 * 缩略图总览面板（虚拟窗口）。
 * props:
 * - pages: 页表（含 url / thumb_url / filename / id）
 * - currentIndex: 当前阅读页，用于高亮与打开时定位
 * - bookmarks: 书签页码集合（Set），用于格子角标
 * - onSelect(index): 点击某页（调用方负责翻页并关闭面板）
 * - onClose: 关闭面板（点背景 / 关闭按钮）
 */
export default function ThumbnailPanel({ pages, currentIndex, bookmarks, onSelect, onClose }) {
  const [thumbRange, setThumbRange] = useState({ start: 0, end: 30 });
  const thumbPanelRef = useRef(null);
  const thumbGridRef = useRef(null);
  const activeThumbRef = useRef(null);

  // 面板打开时把窗口重置到当前页附近（否则从第 1 页开始，翻到 800 页会看到空白）
  useEffect(() => {
    const start = Math.max(0, currentIndex - 15);
    setThumbRange({ start, end: Math.min(pages.length, start + 30) });
  }, [currentIndex, pages.length]);

  // 滚动/尺寸变化时重算窗口：以已挂载格子的实际位置为准（格子高度一致，误差小）
  useEffect(() => {
    const root = thumbPanelRef.current;
    const grid = thumbGridRef.current;
    if (!root || !grid || pages.length === 0) return;

    let rafPending = false;
    const recompute = () => {
      rafPending = false;
      const gridRect = grid.getBoundingClientRect();
      const rootRect = root.getBoundingClientRect();
      // 网格内可见区域的上下边界，外扩 OVERSCAN 个格子的高度
      const cellH = 150; // 缩略图 120px + 页码 + gap 的近似行高
      const rowsVisible = Math.ceil((rootRect.height || 600) / cellH);
      const firstRow = Math.max(0, Math.floor((rootRect.top - gridRect.top) / cellH));
      const cols = Math.max(1, Math.floor(gridRect.width / 110));
      const start = Math.max(0, (firstRow - THUMB_OVERSCAN) * cols);
      const end = Math.min(pages.length, (firstRow + rowsVisible + THUMB_OVERSCAN) * cols);
      setThumbRange(prev => (prev.start === start && prev.end === end ? prev : { start, end }));
    };
    const onScroll = () => {
      if (!rafPending) {
        rafPending = true;
        requestAnimationFrame(recompute);
      }
    };
    recompute();
    root.addEventListener('scroll', onScroll, { passive: true });
    return () => root.removeEventListener('scroll', onScroll);
  }, [pages.length]);

  // 打开后把当前页滚动进视野
  useEffect(() => {
    if (activeThumbRef.current) {
      setTimeout(() => {
        activeThumbRef.current?.scrollIntoView({ block: 'center', behavior: 'smooth' });
      }, 100);
    }
  }, []);

  return (
    <div className="thumbnail-panel" onClick={onClose} role="dialog" aria-modal="true" aria-label="缩略图总览">
      <div className="thumbnail-panel-inner" ref={thumbPanelRef} onClick={e => e.stopPropagation()}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
          <h3 id="thumbnail-title">缩略图 ({pages.length} 页)</h3>
          <button className="btn btn-secondary btn-sm" onClick={onClose} aria-label="关闭缩略图面板">关闭</button>
        </div>
        <div className="thumbnail-grid" ref={thumbGridRef}>
          {/* 全部格子都渲染（占位撑出滚动高度），窗口外的格子不挂 <img> */}
          {pages.map((p, i) => {
            const inWindow = i >= thumbRange.start && i < thumbRange.end;
            return (
              <div
                key={p.id}
                ref={(el) => {
                  if (i === currentIndex) activeThumbRef.current = el;
                }}
                className={`thumbnail-item ${i === currentIndex ? 'active' : ''}`}
                onClick={() => onSelect(i)}
              >
                {inWindow ? (
                  <img src={p.thumb_url || p.url} alt={p.filename} loading="lazy" decoding="async" />
                ) : (
                  // 未进入窗口：保留同尺寸空位，避免网格塌陷导致滚动位置跳动
                  <div className="thumbnail-placeholder" aria-hidden="true" />
                )}
                <div className="page-num">{i + 1}{bookmarks.has(i) ? ' ⭐' : ''}</div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}