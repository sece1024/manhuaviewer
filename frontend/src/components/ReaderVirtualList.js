import React from 'react';

// —— 长图模式虚拟滚动 ——
// 只渲染可视窗口 ± OVERSCAN 页的 DOM 节点；窗口外用上下 spacer 撑出总高度维持滚动条。
// 未测量页面按 EST_PAGE_HEIGHT 估算，已在 pageHeights 中的页面按真实显示高度累加，
// 因此滚动↔页码定位与“全量挂载占位”时精度一致，但 DOM 节点从上千降到几十个。
const OVERSCAN = 10; // 窗口上下各多渲染的页数，保证快速滚动时 sentinel 已就位
export const EST_PAGE_HEIGHT = 250; // 未测量页面的估算显示高度（与旧版占位一致）

/// 单个窗口页（memoized）：只有本页的高度/可见性等 props 变化才重渲染，
/// 其余页面在别的页图片加载后跳过 diff（前缀和变化不再拖累整个窗口）。
const WindowedPage = React.memo(function WindowedPage({ p, index, inRange, sentinelRef, imgStyle, pageHeight, onImageLoad }) {
  return (
    <div
      ref={sentinelRef}
      data-idx={index}
      style={{ width: '100%', minHeight: inRange ? undefined : pageHeight }}
    >
      {inRange ? (
        <img
          src={p.url}
          alt={p.filename}
          loading="lazy"
          decoding="async"
          style={imgStyle}
          onError={(e) => { e.target.style.display = 'none'; }}
          onLoad={(e) => {
            // 记录真实渲染高度（宽 100%，高度=容器宽×原始高宽比），供占位与跳页定位使用；
            // 顺带把原始宽高带回，供“跨页过宽→自动单页”判定缓存页尺寸
            const img = e.currentTarget;
            const container = img.parentElement;
            const cw = container ? container.clientWidth : 0;
            const nh = img.naturalHeight || 0;
            const nw = img.naturalWidth || 1;
            if (cw > 0 && nh > 0) onImageLoad(index, Math.round((cw * nh) / nw), nw, nh);
          }}
        />
      ) : null}
    </div>
  );
});

/// 长图虚拟列表：高度来自前缀和（O(1) 取上下 spacer），页节点 memoized。
export default function LongImageList({ pages, visibleRange, sentinelRef, imgStyle, prefix, onImageLoad }) {
  const n = pages.length;
  const start = Math.max(0, visibleRange.start - OVERSCAN);
  const end = Math.min(n, visibleRange.end + OVERSCAN);

  const topSpacer = prefix[start];
  const bottomSpacer = prefix[n] - prefix[end];

  const items = [];
  for (let i = start; i < end; i++) {
    const p = pages[i];
    items.push(
      <WindowedPage
        key={p.id}
        p={p}
        index={i}
        inRange={i >= visibleRange.start && i < visibleRange.end}
        sentinelRef={sentinelRef}
        imgStyle={imgStyle}
        pageHeight={prefix[i + 1] - prefix[i]}
        onImageLoad={onImageLoad}
      />
    );
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', touchAction: 'pan-y', width: '100%' }}>
      {topSpacer > 0 && <div aria-hidden="true" style={{ height: topSpacer, flexShrink: 0 }} />}
      {items}
      {bottomSpacer > 0 && <div aria-hidden="true" style={{ height: bottomSpacer, flexShrink: 0 }} />}
    </div>
  );
}