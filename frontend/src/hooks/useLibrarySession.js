import { useCallback, useEffect, useRef } from 'react';
import api, { membershipGeneration } from '../utils/api';
import { membershipChanged, idsWithin } from '../utils/listReconcile';

// 会话缓存：{ [mode]: { archives, page, hasMore, search, sortBy, sortOrder,
// selectedTag, selectedCategory, expandedGroup, groupMembers, scrollTop } }
// 模块级 —— Library 卸载（进入阅读器等路由）后保留，返回时可秒开旧列表。
const librarySessions = {};

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

  // 会话恢复后是否已通过后台比对确认过：未确认前不允许把当前列表写回会话。
  // 否则“恢复旧列表 → 比对尚未返回就切走页面”会把可能已陈旧的列表重新固化进会话，
  // 让已被删除的档案在每次往返中复活。无会话可恢复（首次加载）时直接视为已确认。
  const sessionVerifiedRef = useRef(false);

  // 卸载（进入阅读器等路由）时保存浏览会话，返回时可恢复
  useEffect(() => {
    return () => {
      if (!sessionEnabled) return;
      if (!sessionVerifiedRef.current) return; // 未经比对确认的列表不写回，避免固化陈旧数据
      const el = listScrollRef.current;
      const st = latestStateRef.current;
      if (st && st.archives && st.archives.length > 0) {
        librarySessions[mode] = {
          ...st,
          scrollTop: el ? el.scrollTop : 0,
          generation: membershipGeneration(),
        };
      }
    };
  }, [mode, sessionEnabled, listScrollRef]);

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
      });
      if (!Array.isArray(data)) return;
      // 用户在比对期间已切换条件：丢弃过期结果
      if (filterRefs.sortByRef.current !== s.sortBy ||
          filterRefs.sortOrderRef.current !== s.sortOrder ||
          filterRefs.searchRef.current !== s.search ||
          filterRefs.selectedTagRef.current !== s.selectedTag ||
          filterRefs.readFilterRef.current !== (s.readFilter || 'all') ||
          filterRefs.selectedCategoryRef.current !== s.selectedCategory) {
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

  return { librarySessions, reconcileLibrary, markSessionVerified, sessionIsStale };
}