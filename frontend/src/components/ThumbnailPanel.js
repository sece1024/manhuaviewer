import { useCallback, useEffect, useRef, useState } from 'react';

// 缩略图面板：spacer 虚拟化 —— 只渲染可视窗口 ± THUMB_OVERSCAN 的格子，
// 上下用满宽 spacer 撑出总高度维持滚动条与滚动位置。
// 此前是"每页都渲染一个占位 div"（2000 页 = 2000 个节点，每次滚动全量 diff），
// 与 ReaderVirtualList 的 spacer 思路一致，DOM 从 O(n) 降到 O(几十)。
const THUMB_OVERSCAN = 12;
// 网格轨道：minmax(100px, 1fr) + 10px gap（与 .thumbnail-grid 的 CSS 保持一致）
const CELL_MIN = 100;
const CELL_GAP = 10;
// 行高（图片 120px + 页码 ~23px + 边框 4px）的估算值，实测后会被更正
const ROW_H_FALLBACK = 150;

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
  const [gridMetrics, setGridMetrics] = useState({ cols: 4, rowH: ROW_H_FALLBACK });
  const thumbPanelRef = useRef(null);
  const thumbGridRef = useRef(null);
  const activeThumbRef = useRef(null);

  // 面板打开时把窗口重置到当前页附近（否则从第 1 页开始，翻到 800 页会看到空白）
  useEffect(() => {
    const start = Math.max(0, currentIndex - 15);
    setThumbRange({ start, end: Math.min(pages.length, start + 30) });
  }, [currentIndex, pages.length]);

  // 滚动/尺寸变化时重算窗口与网格度量（列数、行高）。
  // 列数来自 auto-fill 公式；行高优先取已挂载格子的真实高度，缺失时才用估算。
  useEffect(() => {
    const root = thumbPanelRef.current;
    const grid = thumbGridRef.current;
    if (!root || !grid || pages.length === 0) return;

    const colCount = () => {
      const w = grid.clientWidth || 800;
      return Math.max(1, Math.floor((w + CELL_GAP) / (CELL_MIN + CELL_GAP)));
    };
    const rowHeight = () => {
      // jsdom/隐藏元素没有布局，offsetHeight 为 0；实测为 0 时退回估算，
      // 否则 600/0 = Infinity、0/0 = NaN 会让窗口算出 {start:null,end:null}
      const item = grid.querySelector('.thumbnail-item');
      const h = item ? item.offsetHeight : 0;
      return h > 0 ? h : ROW_H_FALLBACK;
    };

    let rafPending = false;
    const recompute = () => {
      rafPending = false;
      const cols = colCount();
      const rowH = rowHeight();
      setGridMetrics(prev => (prev.cols === cols && prev.rowH === rowH ? prev : { cols, rowH }));

      const rootRect = root.getBoundingClientRect();
      const gridRect = grid.getBoundingClientRect();
      // 首行可见行号（滚动容器顶缘相对网格顶缘的距离 ÷ 行高），外扩 OVERSCAN 行
      const firstRow = Math.max(0, Math.floor((rootRect.top - gridRect.top) / rowH));
      const rowsVisible = Math.ceil((rootRect.height || 600) / rowH);
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
    window.addEventListener('resize', onScroll);
    return () => {
      root.removeEventListener('scroll', onScroll);
      window.removeEventListener('resize', onScroll);
    };
  }, [pages.length]);

  // 打开后把当前页滚动进视野
  useEffect(() => {
    if (activeThumbRef.current) {
      setTimeout(() => {
        activeThumbRef.current?.scrollIntoView({ block: 'center', behavior: 'smooth' });
      }, 100);
    }
  }, []);

  // 稳化的 item ref 回调：避免每次渲染产生新函数导致 React 反复 detach/attach
  const bindActiveThumb = useCallback((el) => {
    if (el) activeThumbRef.current = el;
  }, []);

  const { cols, rowH } = gridMetrics;
  const startRow = Math.floor(thumbRange.start / cols);
  const endRow = Math.ceil(thumbRange.end / cols);
  const totalRows = Math.ceil(pages.length / cols);
  const topPad = startRow * rowH;
  const bottomPad = Math.max(0, totalRows - endRow) * rowH;

  return (
    <div className="thumbnail-panel" onClick={onClose} role="dialog" aria-modal="true" aria-label="缩略图总览">
      <div className="thumbnail-panel-inner" ref={thumbPanelRef} onClick={e => e.stopPropagation()}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
          <h3 id="thumbnail-title">缩略图 ({pages.length} 页)</h3>
          <button className="btn btn-secondary btn-sm" onClick={onClose} aria-label="关闭缩略图面板">关闭</button>
        </div>
        <div className="thumbnail-grid" ref={thumbGridRef}>
          {/* 上下满宽 spacer 撑出总高度，维持滚动条；只有窗口内的格子真正挂载 DOM */}
          {topPad > 0 && (
            <div className="thumbnail-spacer" aria-hidden="true" style={{ height: topPad }} />
          )}
          {pages.slice(thumbRange.start, thumbRange.end).map((p, i) => {
            const pageIndex = thumbRange.start + i;
            return (
              <div
                key={p.id}
                ref={pageIndex === currentIndex ? bindActiveThumb : undefined}
                className={`thumbnail-item ${pageIndex === currentIndex ? 'active' : ''}`}
                onClick={() => onSelect(pageIndex)}
              >
                <img src={p.thumb_url || p.url} alt={p.filename} loading="lazy" decoding="async" />
                <div className="page-num">{pageIndex + 1}{bookmarks.has(pageIndex) ? ' ⭐' : ''}</div>
              </div>
            );
          })}
          {bottomPad > 0 && (
            <div className="thumbnail-spacer" aria-hidden="true" style={{ height: bottomPad }} />
          )}
        </div>
      </div>
    </div>
  );
}
