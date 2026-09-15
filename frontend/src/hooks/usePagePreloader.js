import { useCallback, useEffect, useMemo, useRef } from 'react';

/**
 * 阅读器图片预加载（LRU Image 缓存，最多 12 张）。
 *
 * 持有 Image 对象引用防止被 GC；翻页时预载当前页前后小窗口，远离当前页的缓存按
 * ±6 页范围清理，换了书清空不属于新书的残留。长图模式页面由 <img> 随滚动加载，
 * 额外 new Image() 只会上双份内存，故长图模式不预载。
 *
 * 返回 `pageReady(i)`：页面“已就绪”（本会话显示过 或 预加载 Image 已完成解码），
 * 供翻页时保持不透明度直接呈现，消除闪烁帧；`loadedPageIdsRef` 由调用方共享
 * （图片 onLoad 记录、换档时重置）。
 */
export default function usePagePreloader({ pages, currentIndex, longImage }) {
  // 预加载缓存（LRU）：持有 Image 对象引用防止被 GC，最多 12 张
  const preloadCacheRef = useRef({ order: [], map: {} });
  // 本次会话已完成过加载的页 id 集合：翻页到已就绪的跨页时保持不透明度直接呈现
  const loadedPageIdsRef = useRef(new Set());

  // url -> index 映射，用于预加载清理，避免 O(pages × 30) 的 findIndex 扫描
  const pageIndexByUrl = useMemo(() => {
    const map = {};
    for (let i = 0; i < pages.length; i++) map[pages[i].url] = i;
    return map;
  }, [pages]);

  // 页面是否“已就绪”：本会话显示过（onLoad 已记录 id）或预加载 Image 已完成解码。
  // 已就绪的页面在翻页时直接呈现（保持不透明度），不再把透明度拉回 0，消除闪烁帧。
  const pageReady = useCallback((i) => {
    if (i == null || i < 0 || i >= pages.length) return false;
    if (loadedPageIdsRef.current.has(pages[i].id)) return true;
    const img = preloadCacheRef.current.map[pages[i].url];
    return !!(img && img.complete && img.naturalWidth > 0);
  }, [pages]);

  useEffect(() => {
    if (pages.length === 0 || longImage) return;
    const MAX_PRELOAD = 12;
    const cache = preloadCacheRef.current;

    // 换了书：清空不属于当前页的残留缓存
    for (const url of [...cache.order]) {
      if (pageIndexByUrl[url] === undefined) {
        delete cache.map[url];
        cache.order = cache.order.filter(u => u !== url);
      }
    }

    const touch = (url) => {
      if (cache.map[url]) {
        // 已存在，移到最新
        cache.order = cache.order.filter(u => u !== url);
        cache.order.push(url);
        return;
      }
      // 新增
      const img = new Image();
      img.decoding = 'async';
      img.src = url;
      cache.map[url] = img;
      cache.order.push(url);
      // LRU 淘汰
      while (cache.order.length > MAX_PRELOAD) {
        const oldest = cache.order.shift();
        delete cache.map[oldest];
      }
    };

    const remove = (url) => {
      if (cache.map[url]) {
        delete cache.map[url];
        cache.order = cache.order.filter(u => u !== url);
      }
    };

    // 预载当前页前后小窗口（-2 … +3 含，双页模式下下一跨页需要 {ci+2, ci+3} 都就绪）
    const start = Math.max(0, currentIndex - 2);
    const end = Math.min(pages.length, currentIndex + 4);
    for (let i = start; i < end; i++) {
      if (i !== currentIndex) touch(pages[i].url);
    }

    // 清理远离当前页的缓存（±6 页范围外）
    for (const url of [...cache.order]) {
      const idx = pageIndexByUrl[url];
      if (idx !== undefined && (idx < currentIndex - 6 || idx > currentIndex + 6)) {
        remove(url);
      }
    }
  }, [currentIndex, pages, pageIndexByUrl, longImage]);

  return { pageReady, loadedPageIdsRef };
}