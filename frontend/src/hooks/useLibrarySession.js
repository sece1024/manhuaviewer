import { useCallback, useEffect, useRef } from 'react';
import api, { membershipGeneration } from '../utils/api';
import { membershipChanged, idsWithin } from '../utils/listReconcile';

// 会话缓存：{ [mode]: { archives, page, hasMore, search, sortBy, sortOrder,
// selectedTag, selectedCategory, addedRange, expandedGroup, groupMembers, scrollTop } }
// 模块级 —— Library 卸载（进入阅读器等路由）后保留，返回时可秒开旧列表。
const librarySessions = {};

// 会话里最多保留的档案条数：与 reconcileLibrary 的比对窗口（500）一致。
// 超出部分不常驻内存——卸载时若把翻了几十页的整份列表都存进会话，这份引用会让
// 整个数组（可能上万条）在下次进入书库前一直不被 GC；截断后超出的条目返回时由
// 触底加载哨兵按 page 续拉，秒开体验对前 500 条不受影响。
const MAX_SESSION_ARCHIVES = 500;

// 写操作（扫描/删除/导入/改名…）会改变档案成员集合，此时会话里的旧列表必须作废：
// 否则从设置页扫描完回到书库，会先秒开出已被清理的档案名（比对失败或中途切页时
// 还会被再次写回会话），表现为“手动删掉的漫画一直在”。
// 用 api 的成员代际号判定：会话记录写入时的代际，代际变了即视为失效。
// （用比较而不是注册回调，避免依赖模块加载顺序与 mock 行为。）
function sessionIsStale(session) {
  return !session || session.generation !== membershipGeneration();
}

/// 测试辅助：清空模块级会话缓存（跨用例隔离；模块级缓存不会随卸载消失）。
export function clearLibrarySessions() {
  for (const key of Object.keys(librarySessions)) delete librarySessions[key];
}

/**
 * 书库浏览会话：保存（每帧镜像 + 卸载写入、带滚动位置）与后台一致性比对。
 *
 * 会话“恢复”留在组件里（它需要写一堆筛选 state 与 ref），本 hook 负责会话的
 * 产生与维护：
 * - `snapshot`：组件渲染时传入的最新状态对象；内部以 effect 每帧镜像。
 * - `listScrollEl`：列表容器元素（回调 ref 提供）；滚动时把位置镜像进 ref —— 卸载清理里
 *   React 已把 ref 置 null，读 DOM 只会拿到 0（见下方 scrollPosRef 注释）。
 * - 卸载时把镜像 + 滚动位置写入 `librarySessions[mode]`。
 * - `reconcileLibrary(s)`：与会话“已加载窗口”做成员集合对比——只有成员集合变化
 *   （档案增删/替换）才整体刷新；顺序变化（典型：读完一本后它在“最近阅读”排序
 *   里前移）与字段变化一律走合并分支，保留已加载分页与滚动位置，否则每次从
 *   阅读器返回都会被踢回第一页。
 */
export default function useLibrarySession({
  mode,
  sessionEnabled,
  listScrollRef,
  listScrollEl,
  snapshot,
  filterRefs,
  pageRef,
  pageSize,
  setArchives,
  setHasMore,
  setExpandedGroup,
  setGroupMembers,
}) {
  // 每帧镜像最新状态（卸载时用于写浏览会话）
  const latestStateRef = useRef(null);
  useEffect(() => {
    latestStateRef.current = snapshot;
  });

  // 滚动位置镜像：**必须边滚边抓**。组件卸载时 React 会在 commit 阶段先把 ref 置 null，
  // passive cleanup（useEffect 清理）随后才执行，那时 listScrollRef.current 已经是 null
  // —— 这就是此前 scrollTop 恒为 0、"退出阅读器回到书库跳回顶部"的原因。
  const scrollPosRef = useRef({ list: 0, main: 0 });

  useEffect(() => {
    const el = listScrollEl;
    if (!el) return undefined;
    // 宽屏（>768px，含 iPad 横竖屏）：.library-main 自己滚；
    // 窄屏（手机 / iPad 分屏）：.library-layout 高度自适应，滚动发生在 .main-content。
    // 两个容器都记，恢复时都写回（写不动的那个会被浏览器钳成 0，无副作用）。
    const main = el.closest('.main-content');
    const onScroll = () => {
      scrollPosRef.current = { list: el.scrollTop, main: main ? main.scrollTop : 0 };
    };
    // 注意不要在这里读一次初值：恢复会话时镜像已被 restoreScroll 设成目标位置，
    // 而此刻 DOM 还没滚过去（rAF 未执行），读初值会把镜像又抹回 0
    el.addEventListener('scroll', onScroll, { passive: true });
    if (main) main.addEventListener('scroll', onScroll, { passive: true });
    return () => {
      el.removeEventListener('scroll', onScroll);
      if (main) main.removeEventListener('scroll', onScroll);
    };
  }, [listScrollEl]);

  /// 恢复会话里的滚动位置（组件在恢复会话时调用）。
  /// DOM 与镜像一起写：部分环境（jsdom、个别 WebView）程序化设 scrollTop 不派发
  /// scroll 事件，只写 DOM 的话，"恢复后没再滚动就退出"会把位置重新存成 0。
  const restoreScroll = useCallback((pos) => {
    if (!pos) return;
    scrollPosRef.current = { list: pos, main: pos };
    requestAnimationFrame(() => {
      const el = listScrollRef.current;
      if (el) el.scrollTop = pos;
      // 窄屏（≤768px）滚动发生在 .main-content；宽屏下它没有溢出，写入会被钳成 0
      const main = document.querySelector('.main-content');
      if (main) main.scrollTop = pos;
    });
  }, [listScrollRef]);

  // 会话恢复后是否已通过后台比对确认过：未确认前不允许把当前列表写回会话。
  // 否则“恢复旧列表 → 比对尚未返回就切走页面”会把可能已陈旧的列表重新固化进会话，
  // 让已被删除的档案在每次往返中复活。无会话可恢复（首次加载）时直接视为已确认。
  const sessionVerifiedRef = useRef(false);

  // 卸载（进入阅读器等路由）时保存浏览会话，返回时可恢复
  useEffect(() => {
    return () => {
      if (!sessionEnabled) return;
      if (!sessionVerifiedRef.current) return; // 未经比对确认的列表不写回，避免固化陈旧数据
      const st = latestStateRef.current;
      if (st && st.archives && st.archives.length > 0) {
        const pos = scrollPosRef.current;
        // 截断到比对窗口：避免整份（可能上万条）列表常驻内存；page/hasMore 同步收窄，
        // 这样恢复后触底哨兵能从正确页码继续把后面的条目拉回来，不会出现断页。
        const truncated = st.archives.length > MAX_SESSION_ARCHIVES;
        const archives = truncated ? st.archives.slice(0, MAX_SESSION_ARCHIVES) : st.archives;
        librarySessions[mode] = {
          ...st,
          archives,
          page: truncated ? Math.ceil(MAX_SESSION_ARCHIVES / pageSize) : st.page,
          hasMore: truncated ? true : st.hasMore,
          // 真正滚动的那一个（另一个恒为 0）；两者都为 0 时就是 0
          scrollTop: pos.list || pos.main,
          generation: membershipGeneration(),
        };
      }
    };
  }, [mode, sessionEnabled, listScrollRef, pageSize]);

  // 后台一致性比对（与会话快照 s 对比；切条件后丢弃过期结果）
  const reconcileLibrary = useCallback(async (s) => {
    if (!s || !s.archives) return;
    try {
      // 比对窗口 = 会话已加载条数（上限 500，避免大库全量拉取）
      const windowSize = Math.min(Math.max(s.archives.length, 1), 500);
      const data = await api.getArchives({
        sort_by: s.sortBy,
        sort_order: s.sortOrder,
        limit: windowSize,
        page: 1,
        search: s.search,
        ...(s.readFilter && s.readFilter !== 'all' ? { read: s.readFilter } : {}),
        ...(s.selectedTag ? { tag: s.selectedTag } : {}),
        ...(s.selectedCategory ? { category_id: s.selectedCategory } : {}),
        // 日期过滤也要带上，否则比对会用未过滤列表顶掉会话里的过滤视图
        ...(s.addedRange || {}),
      });
      if (!Array.isArray(data)) return;
      // 用户在比对期间已切换条件：丢弃过期结果
      if (filterRefs.sortByRef.current !== s.sortBy ||
          filterRefs.sortOrderRef.current !== s.sortOrder ||
          filterRefs.searchRef.current !== s.search ||
          filterRefs.selectedTagRef.current !== s.selectedTag ||
          filterRefs.readFilterRef.current !== (s.readFilter || 'all') ||
          filterRefs.selectedCategoryRef.current !== s.selectedCategory ||
          (filterRefs.addedRangeRef ? filterRefs.addedRangeRef.current : null) !== (s.addedRange || null)) {
        return;
      }
      if (membershipChanged(idsWithin(s.archives, windowSize), idsWithin(data, windowSize))) {
        // 成员变化 → 归档确实增删/替换，整体刷新（此时回到顶部是正确行为）
        setArchives(data);
        pageRef.current = 1;
        setHasMore(data.length >= pageSize);
        setExpandedGroup(null);
        setGroupMembers(null);
        sessionVerifiedRef.current = true;
        return;
      }
      // 顺序一致 → 合并第一页的字段变化，保留滚动与后续分页
      const patch = new Map(data.map(a => [a.id, a]));
      setArchives(prev => {
        let changed = false;
        const next = prev.map(it => {
          const p = patch.get(it.id);
          if (!p) return it;
          if (p.read_page === it.read_page && p.updated_at === it.updated_at &&
              p.title === it.title && p.page_count === it.page_count && p.file_size === it.file_size) {
            return it;
          }
          changed = true;
          return { ...it, ...p };
        });
        return changed ? next : prev;
      });
      sessionVerifiedRef.current = true;
    } catch (e) {
      // 已恢复到旧数据；比对失败时静默保留现状，但不允许把未确认的列表写回会话
    }
  }, [filterRefs, pageRef, pageSize, setArchives, setHasMore, setExpandedGroup, setGroupMembers]);

  /// 首次加载（无会话可恢复）时列表直接来自服务端，视为已确认。
  const markSessionVerified = useCallback(() => {
    sessionVerifiedRef.current = true;
  }, []);

  return { librarySessions, reconcileLibrary, markSessionVerified, sessionIsStale, restoreScroll };
}