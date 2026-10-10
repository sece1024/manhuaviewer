import React, { useState, useEffect, useMemo, useRef, useCallback, Fragment } from 'react';
import { useNavigate } from 'react-router-dom';
import api from '../utils/api';
import { formatSize, formatDateShort, splitPathParts, lastPathPart, readStateOf, formatReadProgress } from '../utils/format';
import { useToast } from '../components/Toast';
import useSettings from '../hooks/useSettings';
import useTags from '../hooks/useTags';
import useLibrarySession from '../hooks/useLibrarySession';
import LazyImage from '../components/LazyImage';
import TagPicker from '../components/TagPicker';
import CategoryPicker from '../components/CategoryPicker';
import ConfirmDialog from '../components/ConfirmDialog';
import TagTriage from '../components/TagTriage';
import CardActionSheet from '../components/CardActionSheet';
import useLongPress from '../hooks/useLongPress';
import useCommands from '../hooks/useCommands';
import CbzConvertPanel from '../components/CbzConvertPanel';
import Modal from '../components/Modal';
import useCbzConvert from '../hooks/useCbzConvert';
import useScan from '../hooks/useScan';
import useJobs from '../hooks/useJobs';
import { parseScanRoots, addScanRoot, serializeScanRoots, clampDepth } from '../utils/scanRoots';

// 检测是否在 Tauri 环境中
const isTauri = window.__TAURI__ !== undefined;

// 网格卡片：memoized，避免多选切换时整屏重渲染。
// 所有回调通过 props 传入（父组件 useCallback 稳定引用）。
const ArchiveCard = React.memo(function ArchiveCard({ a, compact, isSelected, isKbFocused, selectMode, isExpanded, onOpen, onToggleGroup, onToggleSelect, onTag, onCategory, onRename, onRemove, onOpenSheet }) {
  // 阅读状态与进度文案由 utils/format 统一推导，判定口径与后端 read 筛选一致
  const readState = readStateOf(a);
  const progressText = formatReadProgress(a);
  const progressPercent = a.page_count > 0
    ? Math.min(100, (((a.read_page || 0) + 1) / a.page_count) * 100)
    : 0;
  // 触屏：长按卡片唤起操作面板（那四个 hover 按钮在 iPad 上看不见）
  const { longPressProps, swallowedByLongPress } = useLongPress(() => onOpenSheet(a));
  return (
    <div
      className={`archive-card ${selectMode && isSelected ? 'archive-card-selected' : ''} ${isKbFocused ? 'kb-focus' : ''}`}
      data-archive-id={a.id}
      {...longPressProps}
      onClick={(e) => {
        if (swallowedByLongPress()) return; // 长按已经开过面板，别再进阅读器
        if (selectMode) { onToggleSelect(e, a.id); return; }
        if (a._isGroup) { onToggleGroup(a); return; }
        onOpen(a.id);
      }}
      tabIndex={0}
      role="button"
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          if (selectMode) { onToggleSelect(e, a.id); return; }
          if (a._isGroup) { onToggleGroup(a); return; }
          onOpen(a.id);
        }
      }}
    >
      <div className="archive-card-cover">
        <LazyImage src={a.cover_url} alt={a.title} />
        {selectMode && (
          <div className={`archive-select-check ${isSelected ? 'checked' : ''}`}>
            {isSelected ? '✓' : ''}
          </div>
        )}
        {!selectMode && (
          <>
            <button className="archive-tag-btn" onClick={(e) => onTag(e, a.id)} title="标签">🏷️</button>
            <button className="archive-tag-btn" onClick={(e) => onCategory(e, a.id)} title="分类">📂</button>
            <button className="archive-rename-btn" onClick={(e) => onRename(e, a)} title="重命名">✏️</button>
            <button className="archive-remove-btn" onClick={(e) => onRemove(e, a.id)} title="移除">✕</button>
          </>
        )}
        {/* 粗指针设备上的唯一入口：尺寸够大、常显可见（细指针下与其它按钮一样 hover 才出现） */}
        {!selectMode && (
          <button
            className="archive-more-btn"
            onClick={(e) => { e.stopPropagation(); onOpenSheet(a); }}
            title="更多操作"
            aria-label={`更多操作：${a.title}`}
          >⋯</button>
        )}
        {/* 进度条：一旦读过就显示（第 1 页也要有条，否则「在读」看不出来）；
            宽度按 0 基 read_page +1 计算；已读完单独配色，与「读到一半」区分 */}
        {readState !== 'unread' && (
          <div className={`archive-card-progress ${readState === 'finished' ? 'is-finished' : ''}`}>
            <div className="archive-card-progress-bar" style={{ width: `${progressPercent}%` }} />
          </div>
        )}
      </div>
      <div className="archive-card-info">
        <div className="archive-card-title" title={a.title}>
          {a.title}
          {a._isGroup && <span className="archive-group-chevron" aria-hidden="true">{isExpanded ? '▾' : '▸'}</span>}
        </div>
        <div className="archive-card-meta">
          {a._isGroup ? (
            <span>{a.chapter_count} 话</span>
          ) : (
            <span>{a.page_count} 页</span>
          )}
          {/* 组的进度来自代表成员，混在「N 话」后面会指代不明，故组只留进度条 */}
          {!a._isGroup && progressText && <span>· {progressText}</span>}
          {a.file_size > 0 && <span>· {formatSize(a.file_size)}</span>}
          {/* 添加时间：created_at 首次入库即固定，重扫/更新不重置 */}
          {a.created_at && <span>· {formatDateShort(a.created_at)}</span>}
        </div>
        {a.tags && a.tags.length > 0 && (compact ? (
          // 紧凑模式：标签只留色点条带，颜色对应侧栏；悬停/标题提示看全名
          <div className="archive-card-tagdots">
            {a.tags.slice(0, 8).map(t => (
              <span
                key={t.name}
                className="tag-dot"
                style={{ background: t.color }}
                title={t.namespace ? `${t.namespace}:${t.name}` : t.name}
              />
            ))}
            {a.tags.length > 8 && (
              <span
                className="tag-dot tag-dot-more"
                title={`+${a.tags.length - 8} 个标签`}
              />
            )}
          </div>
        ) : (
          <div className="archive-card-tags">
            {a.tags.slice(0, 3).map(t => (
              <span key={t.name} className="tag" style={{ background: t.color }}>
                {t.namespace && <span className="tag-namespace">{t.namespace}:</span>}
                {t.name}
              </span>
            ))}
            {a.tags.length > 3 && <span className="tag" style={{ background: 'var(--text-tertiary)' }}>+{a.tags.length - 3}</span>}
          </div>
        ))}
      </div>
    </div>
  );
});

// 列表行：memoized，同 ArchiveCard
const ArchiveListItem = React.memo(function ArchiveListItem({ a, isSelected, isKbFocused, selectMode, isExpanded, onOpen, onToggleGroup, onToggleSelect, onTag, onCategory, onRename, onRemove, onOpenSheet }) {
  // 此前这里直接显示 0 基的 read_page（少一页），且 page_index=0 时整段被隐藏
  const progressText = formatReadProgress(a);
  const { longPressProps, swallowedByLongPress } = useLongPress(() => onOpenSheet(a));
  return (
    <div
      className={`archive-list-item ${selectMode && isSelected ? 'archive-list-item-selected' : ''} ${isKbFocused ? 'kb-focus' : ''}`}
      data-archive-id={a.id}
      {...longPressProps}
      onClick={(e) => {
        if (swallowedByLongPress()) return;
        if (selectMode) { onToggleSelect(e, a.id); return; }
        if (a._isGroup) { onToggleGroup(a); return; }
        onOpen(a.id);
      }}
    >
      {selectMode && (
        <div className={`archive-select-check-list ${isSelected ? 'checked' : ''}`}>
          {isSelected ? '✓' : ''}
        </div>
      )}
      <div className="archive-list-thumb">
        <LazyImage src={a.cover_url} alt={a.title} />
      </div>
      <div className="archive-list-info">
        <div className="archive-list-title">
          {a.title}
          {a._isGroup && <span className="archive-group-chevron" aria-hidden="true">{isExpanded ? '▾' : '▸'}</span>}
        </div>
        <div className="archive-list-meta">
          {a._isGroup ? `${a.chapter_count} 话` : `${a.page_count} 页`}
          {' · '}{a.archive_type === 'folder' ? '文件夹' : '压缩包'}
          {a.file_size > 0 && ` · ${formatSize(a.file_size)}`}
          {progressText && ` · ${progressText}`}
          {a.created_at && ` · ${formatDateShort(a.created_at)}`}
        </div>
        {a.tags && a.tags.length > 0 && (
          <div className="archive-list-tags">
            {a.tags.map(t => (
              <span key={t.name} className="tag" style={{ background: t.color }}>
                {t.namespace && <span className="tag-namespace">{t.namespace}:</span>}
                {t.name}
              </span>
            ))}
          </div>
        )}
      </div>
      {!selectMode && (
        <>
          <button className="archive-tag-btn-list" onClick={(e) => onTag(e, a.id)} title="标签">🏷️</button>
          <button className="archive-tag-btn-list" onClick={(e) => onCategory(e, a.id)} title="分类">📂</button>
          <button className="archive-rename-btn-list" onClick={(e) => onRename(e, a)} title="重命名">✏️</button>
          <button className="archive-remove-btn-list" onClick={(e) => onRemove(e, a.id)} title="移除">✕</button>
        </>
      )}
      {/* 粗指针设备上的常显入口（列表项右侧空间足够，直接给一个够大的按钮） */}
      {!selectMode && (
        <button
          className="archive-more-btn-list"
          onClick={(e) => { e.stopPropagation(); onOpenSheet(a); }}
          title="更多操作"
          aria-label={`更多操作：${a.title}`}
        >⋯</button>
      )}
    </div>
  );
});

// 展开后的章节面板（同标题自动组 / 永久合并组共用）
const GroupChapterPanel = React.memo(function GroupChapterPanel({ loading, members, onOpenChapter, onTag, onCategory, onRename, onRemove }) {
  return (
    <div className="archive-group-expanded">
      {loading ? (
        <div className="archive-group-loading">加载章节中...</div>
      ) : (
        <div className="archive-group-chapters">
          {members.map(ch => (
            <div
              key={ch.id}
              className="archive-group-chapter"
              onClick={() => onOpenChapter(ch.id)}
              tabIndex={0}
              role="button"
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  onOpenChapter(ch.id);
                }
              }}
            >
              <div className="archive-group-chapter-cover">
                <LazyImage src={ch.cover_url} alt={lastPathPart(ch.path, ch.archive_type !== 'folder') || ch.title} />
              </div>
              <span className="archive-group-chapter-name">{lastPathPart(ch.path, ch.archive_type !== 'folder') || ch.title}</span>
              <span className="archive-group-chapter-meta">
                {ch.read_page > 0
                  ? `已读 ${Math.min(ch.read_page, ch.page_count || 0)}/${ch.page_count || '?'}`
                  : `${ch.page_count} 页`}
              </span>
              <div className="archive-group-chapter-actions">
                <button className="archive-group-chapter-action" onClick={(e) => onTag(e, ch.id)} title="标签">🏷️</button>
                <button className="archive-group-chapter-action" onClick={(e) => onCategory(e, ch.id)} title="分类">📂</button>
                <button className="archive-group-chapter-action" onClick={(e) => onRename(e, ch)} title="重命名">✏️</button>
                <button className="archive-group-chapter-action danger" onClick={(e) => onRemove(e, ch.id)} title="移除">✕</button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
});

// 跨路由浏览会话由 useLibrarySession 管理：进入阅读器/设置等页面时 Library 会被卸载，
// 按 mode 暂存列表/筛选/分页/展开状态与滚动位置；返回时先恢复、再后台与服务器比对。
// jest 环境下每个用例都是独立的“首次访问”，跨用例恢复会造成泄漏，故默认禁用
const IS_TEST = typeof process !== 'undefined' && process.env.NODE_ENV === 'test';

export default function Library({ mode = 'library', enableSession }) {
  // 会话恢复默认在测试环境禁用（跨用例泄漏）；回归测试可显式 enableSession 打开
  const sessionEnabled = enableSession !== undefined ? enableSession : !IS_TEST;
  const { settings, updateSetting } = useSettings();
  const { tags, reload: reloadTags } = useTags();
  const [archives, setArchives] = useState([]);
  const [search, setSearch] = useState('');
  const [loading, setLoading] = useState(false);
  const [viewMode, setViewMode] = useState(() => settings.view_mode || 'grid');
  // 网格卡片密度：large | normal | compact（封面优先的视觉密度，仅影响网格布局）
  const [cardDensity, setCardDensity] = useState(() => settings.card_density || 'normal');
  const [sortBy, setSortBy] = useState(() => settings.sort_by || 'updated');
  const [sortOrder, setSortOrder] = useState(() => settings.sort_order || 'desc');
  const [selectedTag, setSelectedTag] = useState('');
  const [readFilter, setReadFilter] = useState('all'); // all | unread | in_progress | finished（与后端 read 参数取值一致）
  // 标签状态：all | untagged | tagged（与后端 tag_state 取值一致）。“未打标签”既是
  // 一个筛选，也是整理模式的输入集合
  const [tagState, setTagState] = useState('all');
  // 档案类型筛选：all | folder | archive（压缩包）。统一书库默认展示全部类型，
  // 该筛选仅收窄显示（原“漫画库/收藏”双 tab 合并而来，类型不再是顶层导航位）。
  const [typeFilter, setTypeFilter] = useState(() => {
    const v = settings.type_filter;
    return v === 'folder' || v === 'archive' ? v : 'all';
  });
  const [categories, setCategories] = useState([]);
  const [selectedCategory, setSelectedCategory] = useState(null);
  // 日期过滤（按添加时间）：{ added_from, added_to } 本地日期边界（from 含、to 不含），
  // 对象身份用于会话恢复比对；null = 不过滤
  const [addedRange, setAddedRange] = useState(null);
  const [expandedYears, setExpandedYears] = useState(() => new Set());
  const [dateTree, setDateTree] = useState(null);
  const [showSidebar, setShowSidebar] = useState(true);
  const [showOpenModal, setShowOpenModal] = useState(false);
  const [openPath, setOpenPath] = useState('');
  const [opening, setOpening] = useState(false);
  const [packingCbz, setPackingCbz] = useState(false);
  const [showMobileMenu, setShowMobileMenu] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [confirmTarget, setConfirmTarget] = useState(null);
  const [convertConfirmOpen, setConvertConfirmOpen] = useState(false);
  // 重命名弹窗
  const [renamingId, setRenamingId] = useState(null);
  const [renameValue, setRenameValue] = useState('');
  // 多选模式
  const [selectMode, setSelectMode] = useState(false);
  // 整理模式（键盘逐本打标签）；退出时重拉一次，因为整理期间这些书已经离开「未打标签」
  const [showTriage, setShowTriage] = useState(false);
  // 卡片操作面板的目标档案（触屏上长按卡片或点「⋯」打开）
  const [sheetTarget, setSheetTarget] = useState(null);
  const [selectedIds, setSelectedIds] = useState(new Set());
  // 组展开状态（点击合并后的漫画卡片，就地展开子目录）
  const [expandedGroup, setExpandedGroup] = useState(null);
  const [groupMembers, setGroupMembers] = useState(null);
  const [groupLoading, setGroupLoading] = useState(false);
  const activeGroupRef = useRef(null); // 防止过期请求覆盖新展开组的数据
  const expandedGroupRef = useRef(null); // 镜像 expandedGroup，供稳定的 toggleGroup 引用读取
  // 窄屏：把次要操作收进 ⋯ 菜单
  const [isNarrow, setIsNarrow] = useState(() => typeof window !== 'undefined' && window.innerWidth < 768);
  // 分页状态（page 用 ref，避免 loadMore 的 memoized 闭包读到过期值）
  const PAGE_SIZE = 50;
  const CONTINUE_LIMIT = 6; // 继续阅读横条条数：一行放得下，超出请走「在读」筛选
  const [continueItems, setContinueItems] = useState([]);
  const [continueTick, setContinueTick] = useState(0);
  const pageRef = useRef(1);
  const [hasMore, setHasMore] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const searchDebounceRef = useRef(null);
  const sortByRef = useRef(sortBy);
  const sortOrderRef = useRef(sortOrder);
  // 随机排序的会话种子：同一 seed 下服务端顺序确定，滚动加载更多不会跨页重复/遗漏
  const randomSeedRef = useRef(null);
  const selectedTagRef = useRef(selectedTag);
  const readFilterRef = useRef(readFilter);
  const tagStateRef = useRef(tagState);
  const typeFilterRef = useRef(typeFilter);
  const selectedCategoryRef = useRef(selectedCategory);
  const addedRangeRef = useRef(addedRange);
  const searchRef = useRef(search);
  const requestIdRef = useRef(0);
  const appendLockRef = useRef(false); // 防触底自动加载与按钮点击重复追加同一页
  const loadMoreSentinelRef = useRef(null); // 触底自动加载观察哨兵
  const listScrollRef = useRef(null); // 列表滚动容器
  // 列表容器同时镜像到 state：书库为空时渲染的是欢迎页（没有 .library-main），
  // 恢复会话后容器才挂载，"边滚边记位置"的监听必须等它出现才能挂上 ——
  // 只靠 ref + effect 会在 ref 为 null 时早退且永不重跑。
  const [listScrollEl, setListScrollEl] = useState(null);
  const bindListScroll = useCallback((el) => {
    listScrollRef.current = el; // 既有逻辑（分页/加载更多）继续读这个 ref
    setListScrollEl(el);
  }, []);
  // 会话恢复写入的筛选值快照：用于在恢复后跳过“筛选变化重拉”，避免覆盖恢复的分页
  const restoredFiltersRef = useRef(null);
  const navigate = useNavigate();
  const toast = useToast();

  // ── 批量转换为 CBZ（对选中项发起；后台任务进度见 useCbzConvert）──
  const { converting: convertingCbz, info: convertInfo, startConvert, cancelConvert } =
    useCbzConvert({ toast });

  // ── 扫描目录（书库内直接批量入库；状态/进度见任务层）──
  // 复用 useScan 只为「先持久化 root_dir/scan_depth 再启动」与完成提示这一份逻辑，
  // 进度展示交给全局任务指示器（本页不再自绘进度面板）
  const { scanning: scanningDir, handleScan: startDirScan } = useScan({ updateSetting, toast });
  const { jobs } = useJobs();
  // 扫描完成计数器：用于重拉列表（放 state 而不是直接调 loadArchives，
  // 这样能挂到既有的「筛选变化重拉」effect 上，不新增一处 loadArchives 依赖告警）
  const [reloadTick, setReloadTick] = useState(0);
  const seenScanFinishedAt = useRef(jobs.scan.finishedAt);
  useEffect(() => {
    if (jobs.scan.finishedAt === seenScanFinishedAt.current) return;
    seenScanFinishedAt.current = jobs.scan.finishedAt;
    if (jobs.scan.error) return; // 失败由 useScan 提示，列表无需重拉
    // 扫描会新增/清理档案：重拉列表让新入库的条目直接可见。默认排序是「最近阅读」
    // = COALESCE(last_read_at, updated_at)，新条目没有 last_read_at，天然排在前面。
    restoredFiltersRef.current = null; // 别让会话恢复的"跳过重拉"把这次刷新吃掉
    setReloadTick(t => t + 1);
  }, [jobs.scan.finishedAt, jobs.scan.error]);

  // 浏览会话：卸载时保存（含滚动位置）、进入时后台与服务器比对
  const { librarySessions, reconcileLibrary, markSessionVerified, sessionIsStale, restoreScroll } = useLibrarySession({
    mode,
    sessionEnabled,
    listScrollRef,
    listScrollEl,
    snapshot: {
      archives, page: pageRef.current, hasMore,
      search, sortBy, sortOrder, selectedTag, readFilter, tagState, typeFilter, selectedCategory,
      addedRange,
      expandedGroup, groupMembers,
    },
    filterRefs: { sortByRef, searchRef, selectedTagRef, readFilterRef, tagStateRef, selectedCategoryRef, addedRangeRef },
    pageRef,
    pageSize: PAGE_SIZE,
    setArchives, setHasMore, setExpandedGroup, setGroupMembers,
  });

  // 保持 refs 同步
  useEffect(() => { sortByRef.current = sortBy; }, [sortBy]);
  useEffect(() => { sortOrderRef.current = sortOrder; }, [sortOrder]);
  useEffect(() => { selectedTagRef.current = selectedTag; }, [selectedTag]);
  useEffect(() => { readFilterRef.current = readFilter; }, [readFilter]);
  useEffect(() => { tagStateRef.current = tagState; }, [tagState]);
  useEffect(() => { typeFilterRef.current = typeFilter; }, [typeFilter]);
  useEffect(() => { selectedCategoryRef.current = selectedCategory; }, [selectedCategory]);
  useEffect(() => { addedRangeRef.current = addedRange; }, [addedRange]);
  useEffect(() => { searchRef.current = search; }, [search]);
  useEffect(() => { expandedGroupRef.current = expandedGroup; }, [expandedGroup]);

  const reloadCategories = useCallback(() => {
    return api.getCategories().then(data => { setCategories(data); return data; }).catch(() => []);
  }, []);

  // ── 继续阅读：「在读」且最近读过的几条，横向铺在书库顶部 ──
  // 必须单独拉一次：主列表受当前排序/筛选分页控制，最近在读的条目未必落在第 1 页。
  // tick 用于在删除档案后主动重拉（其余场景由挂载 + saveHistory 的缓存失效覆盖）。
  const refreshContinue = useCallback(() => setContinueTick(t => t + 1), []);
  useEffect(() => {
    let cancelled = false;
    // Promise.resolve + Array.isArray 双重兜底：测试 automock 下该方法返回 undefined，
    // 此时横条保持隐藏，不会与主列表里的同名卡片抢 getByText
    Promise.resolve(api.getContinueReading(CONTINUE_LIMIT))
      .then(items => { if (!cancelled) setContinueItems(Array.isArray(items) ? items : []); })
      .catch(() => { if (!cancelled) setContinueItems([]); });
    return () => { cancelled = true; };
  }, [continueTick]);

  // 日期树（年 → 月计数）。Promise.resolve 包一层：测试的 automock 下方法返回
  // undefined 而非 Promise，直接 .then 会崩；首次拿到树时默认展开最新一年。
  const reloadDateTree = useCallback(() => {
    return Promise.resolve(api.getAddedTree())
      .then(t => {
        const ok = t && Array.isArray(t.years) ? t : null;
        setDateTree(ok);
        setExpandedYears(prev => (prev.size === 0 && ok && ok.years.length > 0)
          ? new Set([ok.years[0].year])
          : prev);
        return ok;
      })
      .catch(() => null);
  }, []);

  useEffect(() => {
    const s = sessionEnabled ? librarySessions[mode] : null;
    // 会话记录写入后若发生过影响成员集合的写操作（扫描/删除/导入…），代际会变化，
    // 该会话视为陈旧：直接重新拉取，不再用旧列表秒开（否则已删档案名会先出现）
    if (s && !sessionIsStale(s) && s.archives && s.archives.length > 0) {
      // 恢复浏览会话：秒开旧列表，保留已加载分页、展开状态与滚动位置
      // 旧会话可能带着已废弃的 read 取值（read=有阅读记录），它不是下拉框里的选项，
      // 直接用会让筛选框显示为空、且用户看不出还挂着一个筛选，故归一化为「全部」
      const readFilterRestored = ['unread', 'in_progress', 'finished'].includes(s.readFilter)
        ? s.readFilter
        : 'all';
      const tagStateRestored = ['untagged', 'tagged'].includes(s.tagState) ? s.tagState : 'all';
      setSearch(s.search); searchRef.current = s.search;
      setSortBy(s.sortBy); sortByRef.current = s.sortBy;
      setSortOrder(s.sortOrder); sortOrderRef.current = s.sortOrder;
      setSelectedTag(s.selectedTag); selectedTagRef.current = s.selectedTag;
      setReadFilter(readFilterRestored); readFilterRef.current = readFilterRestored;
      setTagState(tagStateRestored); tagStateRef.current = tagStateRestored;
      setTypeFilter(s.typeFilter || 'all'); typeFilterRef.current = s.typeFilter || 'all';
      setSelectedCategory(s.selectedCategory); selectedCategoryRef.current = s.selectedCategory;
      setAddedRange(s.addedRange || null); addedRangeRef.current = s.addedRange || null;
      // 记录本轮恢复写入的筛选值：上述 setState 提交后，“筛选变化重拉”effect 会看到
      // 与这里相同的值并跳过，避免把恢复好的分页/滚动位置覆盖成第 1 页
      restoredFiltersRef.current = {
        sortBy: s.sortBy, sortOrder: s.sortOrder, selectedTag: s.selectedTag,
        readFilter: readFilterRestored, tagState: tagStateRestored, selectedCategory: s.selectedCategory,
        typeFilter: s.typeFilter || 'all', addedRange: s.addedRange || null,
      };
      setArchives(s.archives);
      pageRef.current = s.page;
      setHasMore(s.hasMore);
      if (s.expandedGroup) {
        setExpandedGroup(s.expandedGroup);
        if (s.groupMembers) setGroupMembers(s.groupMembers);
      }
      if (s.scrollTop) {
        // 等列表挂载（空书库时先渲染欢迎页）+ 布局落定后，DOM 与位置镜像一起恢复
        restoreScroll(s.scrollTop);
      }
      // 后台与服务器比对（仅内容重排时整体刷新，否则合并字段）
      reconcileLibrary(s);
    } else {
      loadArchives();
    }
    reloadCategories();
    reloadDateTree();
    // 每次进入书库刷新标签列表与计数（阅读器/设置页里的改动可能已过期）
    reloadTags();
    return () => clearTimeout(searchDebounceRef.current);
  // eslint-disable-next-line
  }, [mode, sessionEnabled]);

  useEffect(() => {
    const check = () => setIsNarrow(window.innerWidth < 768);
    window.addEventListener('resize', check);
    return () => window.removeEventListener('resize', check);
  }, []);

  const loadArchives = async (params = {}, append = false) => {
    if (append && appendLockRef.current) return; // 追加页正在加载，忽略重复触发
    const id = ++requestIdRef.current;
    if (append) {
      appendLockRef.current = true;
      setLoadingMore(true);
    } else {
      appendLockRef.current = false; // 新一轮查询使在途追加失效
      setLoading(true);
      // 列表整体刷新（排序/过滤/增删改）后，分组结构可能变化，收起已展开的组
      setExpandedGroup(null);
      setGroupMembers(null);
    }
    try {
      const nextPage = append ? pageRef.current + 1 : 1;
      const categoryId = params.category_id !== undefined ? params.category_id : selectedCategoryRef.current;
      const baseParams = {
        sort_by: sortByRef.current,
        sort_order: sortOrderRef.current,
        limit: PAGE_SIZE,
        page: nextPage,
        ...params,
      };
      if (sortByRef.current === 'random') {
        if (!randomSeedRef.current) randomSeedRef.current = Math.floor(Math.random() * 1e9) + 1;
        baseParams.seed = randomSeedRef.current;
      }
      if (categoryId) baseParams.category_id = categoryId;
      else delete baseParams.category_id;
      if (readFilterRef.current && readFilterRef.current !== 'all') {
        baseParams.read = readFilterRef.current;
      }
      if (tagStateRef.current && tagStateRef.current !== 'all') {
        baseParams.tag_state = tagStateRef.current;
      }
      const ar = addedRangeRef.current;
      if (ar) {
        baseParams.added_from = ar.added_from;
        baseParams.added_to = ar.added_to;
      }
      const data = await api.getArchives(baseParams);
      if (id !== requestIdRef.current) return;
      setArchives(prev => append ? [...prev, ...data] : data);
      pageRef.current = nextPage;
      setHasMore(data.length >= PAGE_SIZE);
      // 列表直接来自服务端（首次加载 / 筛选变化 / 写操作后重拉）：可作为会话基准写回
      markSessionVerified();
      // 日期树随成员集合变化（新增/移除改计数）：首页拉取后顺带刷新，
      // GET 缓存 30s + /archives 写失效，未变化时基本无开销
      if (!append) reloadDateTree();
    } catch (e) {
      if (id === requestIdRef.current) toast(e.message, 'error');
    } finally {
      if (id === requestIdRef.current) {
        if (append) {
          appendLockRef.current = false;
          setLoadingMore(false);
        } else {
          setLoading(false);
        }
      }
    }
  };

  // 会话恢复后抑制“筛选变化重拉”：恢复时写入的筛选值与当前 state 一致时不应触发整表
  // 重拉（否则会把恢复的分页覆盖成第 1 页）。用记录“该轮恢复写入的筛选值”代替单帧
  // ref：React 批处理下 setState 可能跨帧提交，单帧窗口不足以覆盖。
  useEffect(() => {
    if (restoredFiltersRef.current) {
      const r = restoredFiltersRef.current;
      const same = r.sortBy === sortBy && r.sortOrder === sortOrder &&
        r.selectedTag === selectedTag && r.readFilter === readFilter && r.tagState === tagState &&
        r.selectedCategory === selectedCategory && r.typeFilter === typeFilter &&
        r.addedRange === addedRange;
      if (same) {
        // 仍是恢复写入的那组值：跳过重拉；用户一旦改动筛选，下次运行不再匹配即正常重拉
        return;
      }
      restoredFiltersRef.current = null;
    }
    loadArchives({ search: searchRef.current, tag: selectedTag, category_id: selectedCategory });
    // reloadTick 只在扫描结束后 +1：复用同一条重拉路径（含分页重置、展开组收起）
  }, [sortBy, sortOrder, selectedTag, readFilter, tagState, selectedCategory, typeFilter, addedRange, reloadTick]);

  const handleSearch = useCallback((val) => {
    setSearch(val);
    // 立刻置 loading：列表为空时用骨架屏占住"正在找"的空窗。
    // 已有结果时不显示骨架（loading 分支还要求 displayArchives 为空），所以不打断阅读。
    setLoading(true);
    clearTimeout(searchDebounceRef.current);
    searchDebounceRef.current = setTimeout(() => {
      loadArchives({ search: val, tag: selectedTagRef.current });
    }, 150);
  }, []);

  const handleLoadMore = useCallback(() => {
    loadArchives({ search: searchRef.current, tag: selectedTagRef.current }, true);
  }, []);

  const handleViewMode = (mode) => {
    setViewMode(mode);
    updateSetting('view_mode', mode);
  };

  const handleDensityChange = (density) => {
    setCardDensity(density);
    updateSetting('card_density', density);
  };

  // 清除全部筛选。search 不走「筛选变化重拉」effect（它自己有防抖路径），
  // 所以这里顺带 +1 reloadTick 保证一定会重拉一次，而不是"改了却没刷新"。
  const clearFilters = useCallback(() => {
    clearTimeout(searchDebounceRef.current);
    setSearch(''); searchRef.current = '';
    setSelectedTag(''); selectedTagRef.current = '';
    setSelectedCategory(null); selectedCategoryRef.current = null;
    setAddedRange(null); addedRangeRef.current = null;
    setReadFilter('all'); readFilterRef.current = 'all';
    setTagState('all'); tagStateRef.current = 'all';
    setTypeFilter('all'); typeFilterRef.current = 'all';
    updateSetting('type_filter', 'all');
    setReloadTick(t => t + 1);
  }, [updateSetting]);

  const handleTagFilter = (tagName) => {
    clearTimeout(searchDebounceRef.current);
    const next = selectedTag === tagName ? '' : tagName;
    setSelectedTag(next);
  };

  const handleCategoryFilter = (categoryId) => {
    clearTimeout(searchDebounceRef.current);
    const next = selectedCategory === categoryId ? null : categoryId;
    setSelectedCategory(next);
  };

  // 日期过滤（按添加时间）：点年 = 整年，点月 = 单月；再点同一节点取消过滤。
  // 边界在点击时一次性算成本地日期字符串，会话比对直接用对象身份。
  const handleDateFilter = (year, month = null) => {
    clearTimeout(searchDebounceRef.current);
    const pad = n => String(n).padStart(2, '0');
    const from = `${year}-${month ? pad(month) : '01'}-01`;
    const to = month && month < 12 ? `${year}-${pad(month + 1)}-01` : `${year + 1}-01-01`;
    setAddedRange(addedRange && addedRange.added_from === from
      ? null
      : { added_from: from, added_to: to });
  };

  // 展开/收起某年的月份列表（箭头点击，与整年过滤互不干扰）
  const toggleYear = (year) => {
    setExpandedYears(prev => {
      const next = new Set(prev);
      if (next.has(year)) next.delete(year); else next.add(year);
      return next;
    });
  };

  const handleOpenFile = async () => {
    if (!openPath.trim()) return;
    setOpening(true);
    try {
      const result = await api.openFile(openPath.trim());
      setShowOpenModal(false);
      setOpenPath('');
      toast(result.message || '已打开', 'success');
      navigate(`/reader/${result.id}`);
    } catch (e) {
      toast(e.message, 'error');
    }
    setOpening(false);
  };

  const handleQuickOpen = async (type) => {
    if (!isTauri) {
      setShowOpenModal(true);
      return;
    }
    try {
      const options = type === 'folder'
        ? { directory: true, multiple: false, title: '选择漫画文件夹' }
        : { multiple: false, title: '选择漫画文件', filters: [{ name: '漫画文件', extensions: ['zip', 'cbz', 'rar', 'cbr', '7z'] }] };
      const selected = await window.__TAURI__.dialog.open(options);
      if (selected) {
        setOpening(true);
        try {
          const result = await api.openFile(selected);
          toast(result.message || '已打开', 'success');
          navigate(`/reader/${result.id}`);
        } catch (e) {
          toast(e.message, 'error');
        }
        setOpening(false);
      }
    } catch (e) {
      toast('选择文件失败: ' + e.message, 'error');
    }
  };

  // 扫描一个目录（批量入库）：书库是一等入口，不该只把这件事放在设置页。
  // 选中的目录会同时记进「扫描目录」列表，下次可直接重扫。
  const handleScanDirectory = async () => {
    if (!isTauri) {
      toast('扫描目录仅在桌面应用中可用', 'warning');
      return;
    }
    if (scanningDir) {
      toast('已有扫描任务在进行中，进度见右下角', 'info');
      return;
    }
    let selected;
    try {
      selected = await window.__TAURI__.dialog.open({
        directory: true,
        multiple: false,
        title: '选择要扫描的漫画目录',
      });
    } catch (e) {
      toast('选择目录失败: ' + e.message, 'error');
      return;
    }
    if (!selected) return;

    const depth = clampDepth(settings.scan_depth);
    const roots = parseScanRoots(settings.scan_roots, settings.root_dir, settings.scan_depth);
    updateSetting('scan_roots', serializeScanRoots(addScanRoot(roots, selected, depth)));
    // useScan 会持久化 root_dir/scan_depth 再启动任务，并负责完成提示
    startDirScan(selected, depth);
  };

  // 选择文件夹并直接打包为 CBZ
  const handleConvertFolderToCbz = async () => {
    if (!isTauri) {
      toast('此功能仅在桌面应用中可用', 'warning');
      return;
    }
    try {
      const selected = await window.__TAURI__.dialog.open({
        directory: true,
        multiple: false,
        title: '选择要转换为 CBZ 的漫画文件夹',
      });
      if (!selected) return;

      setPackingCbz(true);
      try {
        const result = await api.packCbz(selected);
        toast(result.message || '归档成功', 'success');
      } catch (e) {
        toast(e.message, 'error');
      }
      setPackingCbz(false);
    } catch (e) {
      toast('选择文件夹失败: ' + e.message, 'error');
    }
  };

  // 「从库中移除」：核心逻辑与事件分离，操作面板（无需 stopPropagation）可直接调用
  const confirmRemoveFor = useCallback((id) => {
    setConfirmTarget(id);
    setConfirmOpen(true);
  }, []);
  const handleRemoveArchive = useCallback((e, id) => {
    e.stopPropagation();
    confirmRemoveFor(id);
  }, [confirmRemoveFor]);

  // 撤销刚才的移除：把提示条上的动作接到后端的一次性令牌。
  // 刻意不用 useCallback：它只在事件回调里使用（不是传给 memo 组件的 prop），
  // 而 memo 化会引入 loadArchives 的依赖告警——别的 useCallback 已经踩过三次。
  const handleUndoDelete = async (token) => {
    try {
      const r = await api.undoDeleteArchive(token);
      const restored = r?.restored ?? 0;
      const skipped = r?.skipped ?? 0;
      if (skipped > 0) {
        // 跳过的是撤销窗口内已被重新扫描入库的路径：如实说明，别让用户以为全没了
        toast(`已恢复 ${restored} 个；另有 ${skipped} 个已在库中（期间被扫描重新入库）`, 'warning');
      } else {
        toast(`已恢复 ${restored} 个档案（含标签 / 书签 / 进度）`, 'success');
      }
      loadArchives({ search: searchRef.current, tag: selectedTagRef.current });
      refreshContinue();
    } catch (e) {
      toast(e.message || '撤销失败', 'error');
    }
  };

  // 带「撤销」动作的提示；后端没给令牌（理论上不会）时退化成普通提示
  const toastWithUndo = (message, token) => {
    if (!token) {
      toast(message, 'success');
      return;
    }
    toast(message, 'success', undefined, {
      label: '撤销',
      onClick: () => handleUndoDelete(token),
    });
  };

  const handleConfirmRemove = async () => {
    setConfirmOpen(false);
    const id = confirmTarget;
    if (!id) return;
    const title = archives.find(a => a.id === id)?.title;
    try {
      const r = await api.deleteArchive(id);
      loadArchives({ search, tag: selectedTag });
      refreshContinue();
      toastWithUndo(title ? `已移除《${title}》` : '已移除', r?.undo_token);
    } catch (err) {
      toast(err.message, 'error');
    }
  };

  // TagPicker 状态
  const [tagPickerArchiveId, setTagPickerArchiveId] = useState(null);
  const openTagPickerFor = useCallback((id) => setTagPickerArchiveId(id), []);
  const handleOpenTagPicker = useCallback((e, id) => {
    e.stopPropagation();
    openTagPickerFor(id);
  }, [openTagPickerFor]);
  // 统一的 picker 关闭逻辑：关闭弹层，若有变更则刷新列表 + 对应数据
  const reloadAfterPick = (changed, refetch) => {
    if (changed) {
      loadArchives({ search, tag: selectedTag });
      refetch();
    }
  };
  const handleCloseTagPicker = (changed) => {
    setTagPickerArchiveId(null);
    reloadAfterPick(changed, reloadTags);
  };

  // CategoryPicker 状态
  const [categoryPickerArchiveId, setCategoryPickerArchiveId] = useState(null);
  const openCategoryPickerFor = useCallback((id) => setCategoryPickerArchiveId(id), []);
  const handleOpenCategoryPicker = useCallback((e, id) => {
    e.stopPropagation();
    openCategoryPickerFor(id);
  }, [openCategoryPickerFor]);
  const handleCloseCategoryPicker = (changed) => {
    setCategoryPickerArchiveId(null);
    reloadAfterPick(changed, reloadCategories);
  };

  // 重命名
  const openRenameFor = useCallback((a) => {
    setRenamingId(a.id);
    setRenameValue(a.title);
  }, []);
  const handleOpenRename = useCallback((e, a) => {
    e.stopPropagation();
    openRenameFor(a);
  }, [openRenameFor]);
  const handleConfirmRename = async () => {
    if (!renameValue.trim() || !renamingId) return;
    try {
      await api.updateTitle(renamingId, renameValue.trim());
      toast('已重命名', 'success');
      setRenamingId(null);
      loadArchives({ search, tag: selectedTag });
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  // 多选
  const toggleSelectFor = useCallback((id) => {
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);
  const handleToggleSelect = useCallback((e, id) => {
    e.stopPropagation();
    toggleSelectFor(id);
  }, [toggleSelectFor]);

  // 打开阅读器（稳定引用，供 memoized 卡片使用）
  const openArchive = useCallback((id) => navigate(`/reader/${id}`), [navigate]);

  // 组展开/收起：永久合并组走 getGroupChapters，同标题自动组走 getArchivesByTitle
  const toggleGroup = useCallback(async (a) => {
    const key = a._autoGroup ? a._autoKey : `g:${a.id}`;
    if (expandedGroupRef.current === key) {
      expandedGroupRef.current = null;
      setExpandedGroup(null);
      setGroupMembers(null);
      return;
    }
    expandedGroupRef.current = key;
    setExpandedGroup(key);
    setGroupMembers(null);
    setGroupLoading(true);
    activeGroupRef.current = key;
    try {
      let members;
      if (a._autoGroup) {
        members = (await api.getArchivesByTitle(a.title, a._parentDir)).filter(m => !m.group_id);
      } else {
        members = await api.getGroupChapters(a.id);
      }
      if (activeGroupRef.current === key) setGroupMembers(members);
    } catch (e) {
      if (activeGroupRef.current === key) {
        toast(e.message, 'error');
        expandedGroupRef.current = null;
        setExpandedGroup(null);
      }
    } finally {
      if (activeGroupRef.current === key) setGroupLoading(false);
    }
  }, [toast]);

  const handleExitSelectMode = () => {
    setSelectMode(false);
    setSelectedIds(new Set());
  };
  const handleMerge = async () => {
    const ids = Array.from(selectedIds);
    if (ids.length < 2) return;
    try {
      await api.mergeArchives(ids);
      toast(`已合并 ${ids.length} 个档案`, 'success');
      handleExitSelectMode();
      loadArchives({ search, tag: selectedTag });
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  // 批量打标签 / 批量分类
  const [batchTagPickerOpen, setBatchTagPickerOpen] = useState(false);
  const [batchCategoryPickerOpen, setBatchCategoryPickerOpen] = useState(false);
  const handleCloseBatchTagPicker = (changed) => {
    setBatchTagPickerOpen(false);
    reloadAfterPick(changed, reloadTags);
  };
  const handleCloseBatchCategoryPicker = (changed) => {
    setBatchCategoryPickerOpen(false);
    reloadAfterPick(changed, reloadCategories);
  };

  // 批量删除
  const [batchDeleteConfirmOpen, setBatchDeleteConfirmOpen] = useState(false);
  const handleBatchDelete = async () => {
    const ids = Array.from(selectedIds);
    if (ids.length === 0) return;
    setBatchDeleteConfirmOpen(false);
    try {
      const r = await api.batchDeleteArchives(ids);
      handleExitSelectMode();
      loadArchives({ search, tag: selectedTag });
      refreshContinue();
      toastWithUndo(`已删除 ${ids.length} 个档案`, r?.undo_token);
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  // 卡片操作面板的动作：与卡片上那四个 hover 按钮一一对应
  const sheetItems = useMemo(() => {
    if (!sheetTarget) return [];
    return [
      { key: 'tag', icon: '🏷️', label: '标签', onSelect: () => openTagPickerFor(sheetTarget.id) },
      { key: 'category', icon: '📂', label: '分类', onSelect: () => openCategoryPickerFor(sheetTarget.id) },
      { key: 'rename', icon: '✏️', label: '重命名', onSelect: () => openRenameFor(sheetTarget) },
      {
        key: 'remove', icon: '✕', label: '从库中移除', danger: true,
        onSelect: () => confirmRemoveFor(sheetTarget.id),
      },
    ];
  }, [confirmRemoveFor, openCategoryPickerFor, openRenameFor, openTagPickerFor, sheetTarget]);

  // 「转换为 CBZ」确认框里列出的原文件（只在打开时计算）
  const convertTargetTitles = useMemo(() => {
    if (!convertConfirmOpen) return [];
    return archives.filter(a => selectedIds.has(a.id)).map(a => a.title);
  }, [archives, convertConfirmOpen, selectedIds]);

  // 命令面板：这一屏的操作由页面自己登记（面板在外壳上，拿不到这里的 setState）。
  // 回调经 ref 取最新值，命令列表引用保持稳定——否则每次渲染重登记都会让面板重渲染。
  const paletteActionsRef = useRef({});
  paletteActionsRef.current = {
    scan: handleScanDirectory,
    view: () => handleViewMode(viewMode === 'grid' ? 'list' : 'grid'),
    triage: () => { setTagState('untagged'); setShowTriage(true); },
    select: () => { setSelectMode(true); setSelectedIds(new Set()); },
    clearFilters,
  };
  const paletteCommands = useMemo(() => {
    const act = (key) => () => paletteActionsRef.current[key]();
    return [
      { id: 'lib-scan', group: '书库', icon: '🗂️', label: '扫描目录…', keywords: 'scan add 添加 入库 批量', run: act('scan') },
      { id: 'lib-triage', group: '书库', icon: '🏷️', label: '整理标签（逐本给未打标签的书打标）', keywords: 'triage tag 整理 打标签', run: act('triage') },
      { id: 'lib-view', group: '书库', icon: '▦', label: viewMode === 'grid' ? '切换为列表视图' : '切换为网格视图', keywords: 'view 视图 网格 列表', run: act('view') },
      { id: 'lib-select', group: '书库', icon: '☑️', label: '进入多选模式', keywords: 'select 多选 批量 选择', run: act('select') },
      { id: 'filter-inprogress', group: '筛选', icon: '📖', label: '筛出「在读」', keywords: 'filter 在读 未读完 reading', run: () => setReadFilter('in_progress') },
      { id: 'filter-unread', group: '筛选', icon: '🆕', label: '筛出「未读」', keywords: 'filter 未读 unread', run: () => setReadFilter('unread') },
      { id: 'filter-untagged', group: '筛选', icon: '🏷️', label: '筛出「未打标签」', keywords: 'filter 未整理 untagged', run: () => setTagState('untagged') },
      { id: 'filter-clear', group: '筛选', icon: '🧹', label: '清除全部筛选', keywords: 'clear reset 重置 清理', run: act('clearFilters') },
    ];
  }, [viewMode]);
  useCommands(paletteCommands);

  // 分组已在服务端完成：`archives` 里的每一项要么是普通档案，要么是带 _isGroup 的组卡片。
  const groupedArchives = archives;

  // 类型筛选（filter，不再是顶层导航位）：默认全部，可按容器类型收窄
  const handleTypeFilter = (v) => {
    setTypeFilter(v);
    updateSetting('type_filter', v);
  };
  const displayArchives = useMemo(() => {
    if (typeFilter === 'all') return groupedArchives;
    return groupedArchives.filter(a =>
      typeFilter === 'folder' ? a.archive_type === 'folder' : a.archive_type !== 'folder'
    );
  }, [groupedArchives, typeFilter]);

  // 顶部续读区只在「没加任何筛选」的首页状态出现：用户一旦主动筛选，横条与下面的
  // 列表就表达了两套不同条件，会让人怀疑列表漏了东西；此时把 `read=in_progress`
  // 交给筛选下拉即可（横条上的「查看全部在读」正是这个入口）。
  // ── 书库键盘层 ──
  // 阅读器是完整键盘驱动的，书库此前一个全局快捷键都没有；而书库恰恰是"批量操作"
  // 发生的地方，键盘杠杆最高。这里给最小可用的一套：/ 搜索、j/k 移动、Enter 打开、
  // x 多选、Esc 退出、? 看帮助。
  const searchInputRef = useRef(null);
  const [showSearchHelp, setShowSearchHelp] = useState(false);
  const [kbIndex, setKbIndex] = useState(-1);
  const kbIndexRef = useRef(-1);
  const displayArchivesRef = useRef([]);
  const selectModeRef = useRef(false);
  useEffect(() => { kbIndexRef.current = kbIndex; }, [kbIndex]);
  useEffect(() => { displayArchivesRef.current = displayArchives; }, [displayArchives]);
  useEffect(() => { selectModeRef.current = selectMode; }, [selectMode]);
  // 高亮项跟随移动，并滚进可视区（`nearest` 不会把整页顶走）
  useEffect(() => {
    if (kbIndex < 0) return;
    const el = document.querySelector(`[data-archive-id="${displayArchives[kbIndex]?.id}"]`);
    if (el && el.scrollIntoView) el.scrollIntoView({ block: 'nearest' });
  }, [displayArchives, kbIndex]);

  useEffect(() => {
    const onKeyDown = (e) => {
      // 正在输入 / 有弹层打开时不抢键：用 DOM 查询而不是枚举十几个弹层状态，
      // 免得以后新增一个弹层就漏一个（所有弹层都是 .modal-overlay）
      const tag = e.target && e.target.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (document.querySelector('.modal-overlay')) return;

      if (e.key === '/') {
        e.preventDefault();
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
        return;
      }
      if (e.key === '?') {
        e.preventDefault();
        setShowSearchHelp(true);
        return;
      }
      if (e.key === 'j' || e.key === 'k') {
        const len = displayArchivesRef.current.length;
        if (len === 0) return;
        e.preventDefault();
        setKbIndex(prev => {
          if (prev < 0) return e.key === 'j' ? 0 : len - 1;
          return e.key === 'j' ? Math.min(len - 1, prev + 1) : Math.max(0, prev - 1);
        });
        return;
      }
      if (e.key === 'x') {
        const item = displayArchivesRef.current[kbIndexRef.current];
        if (!item || item._isGroup) return;
        e.preventDefault();
        setSelectMode(true);
        toggleSelectFor(item.id);
        return;
      }
      if (e.key === 'Enter') {
        const item = displayArchivesRef.current[kbIndexRef.current];
        if (!item) return;
        e.preventDefault();
        if (selectModeRef.current) {
          if (!item._isGroup) toggleSelectFor(item.id);
        } else if (item._isGroup) {
          toggleGroup(item);
        } else {
          navigate(`/reader/${item.id}`);
        }
        return;
      }
      if (e.key === 'Escape') {
        if (selectModeRef.current) {
          e.preventDefault();
          setSelectMode(false);
          setSelectedIds(new Set());
        } else if (kbIndexRef.current >= 0) {
          e.preventDefault();
          setKbIndex(-1);
        }
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [navigate, toggleGroup, toggleSelectFor]);

  const hasActiveFilter = Boolean(
    search || selectedTag || selectedCategory || addedRange ||
    (typeFilter && typeFilter !== 'all') || readFilter !== 'all' || tagState !== 'all'
  );
  const showContinue = !hasActiveFilter && continueItems.length > 0;

  // 空结果时告诉用户"是哪几个筛选条件把结果筛没了"——只写"没有匹配的漫画"无从下手
  const activeFilterSummary = useMemo(() => {
    const parts = [];
    if (search) parts.push(`搜索「${search}」`);
    if (selectedTag) parts.push(`标签「${selectedTag}」`);
    if (readFilter !== 'all') {
      parts.push(`阅读状态「${{ unread: '未读', in_progress: '在读', finished: '已读完' }[readFilter] || readFilter}」`);
    }
    if (tagState !== 'all') {
      parts.push(`标签状态「${tagState === 'untagged' ? '未打标签' : '已打标签'}」`);
    }
    if (typeFilter && typeFilter !== 'all') {
      parts.push(`类型「${typeFilter === 'folder' ? '文件夹' : '压缩包'}」`);
    }
    if (selectedCategory) {
      const name = categories.find(c => c.id === selectedCategory)?.name;
      parts.push(`分类「${name || selectedCategory}」`);
    }
    if (addedRange) parts.push('添加日期');
    return parts.join(' · ');
  }, [addedRange, categories, readFilter, search, selectedCategory, selectedTag, tagState, typeFilter]);

  // 触底自动加载更多：滚动接近底部自动拉下一页（底部按钮保留作手动兜底）。
  // 用 appendLockRef 防止 IO 回调与点击在短时间内重复请求同一页。
  useEffect(() => {
    const el = loadMoreSentinelRef.current;
    if (!el || !hasMore || displayArchives.length === 0 || loading || loadingMore) return;
    const obs = new IntersectionObserver((entries) => {
      if (entries[0].isIntersecting) handleLoadMore();
    }, { rootMargin: '600px 0px' });
    obs.observe(el);
    return () => obs.disconnect();
  }, [hasMore, displayArchives.length, loading, loadingMore, handleLoadMore]);

  // 按命名空间分组标签
  const tagsByNamespace = useMemo(() => {
    const map = {};
    for (const t of tags) {
      const ns = t.namespace || NS_OTHER;
      if (!map[ns]) map[ns] = [];
      map[ns].push(t);
    }
    return map;
  }, [tags]);

  // 标签侧栏过滤
  const [tagSearch, setTagSearch] = useState('');
  const filteredTagsByNamespace = useMemo(() => {
    if (!tagSearch.trim()) return tagsByNamespace;
    const q = tagSearch.toLowerCase();
    const out = {};
    for (const [ns, nsTags] of Object.entries(tagsByNamespace)) {
      const filtered = nsTags.filter(t => {
        const fullName = t.namespace ? `${t.namespace}:${t.name}` : t.name;
        return fullName.toLowerCase().includes(q);
      });
      if (filtered.length > 0) out[ns] = filtered;
    }
    return out;
  }, [tagsByNamespace, tagSearch]);
  // 标签多时才显示搜索框（>10 才有意义）
  const showTagSearch = tags.length > 10;

  // 分类排序（置顶优先）：排序在渲染体里裸跑会随每次搜索/选择/密度切换重排数组
  const sortedCategories = useMemo(
    () => [...categories].sort((a, b) => (b.pinned ? 1 : 0) - (a.pinned ? 1 : 0)),
    [categories]
  );

  // 欢迎页只在「没有任何筛选」且**不在加载中**时出现。
  // 此前只判 `archives.length === 0`，而 archives 是"当前筛选的结果集"——于是搜不到时
  // 整个页面（连搜索框一起）被欢迎页顶掉，用户只能切到别的页面再切回来才能重搜。
  // 也不在 loading 时判定：清空搜索到结果回来之间有空窗，否则会闪一下欢迎页。
  const showWelcome = archives.length === 0 && !hasActiveFilter && !loading;
  if (showWelcome) {
    return (
      <div className="welcome-screen">
        <div className="welcome-screen-icon">📚</div>
        <h2>欢迎使用 MangaViewer</h2>
        <p className="welcome-screen-desc">
          先把漫画加入书库，再开始阅读<br />
          <span className="welcome-screen-sub">
            支持文件夹、ZIP/CBZ、RAR/CBR、7Z 压缩包
          </span>
        </p>

        {/* 三选一：批量入库（扫描目录）放第一位——它才是"我有几百个文件"的答案，
            此前空状态只教用户一次打开一个，等于把最费时的路径当成了唯一路径 */}
        {isTauri ? (
          <div className="welcome-screen-actions">
            <button className="btn" onClick={handleScanDirectory} disabled={scanningDir}>
              {scanningDir ? '⏳ 扫描中...' : '🗂️ 扫描漫画目录'}
            </button>
            <button className="btn btn-secondary" onClick={() => handleQuickOpen('folder')} disabled={opening}>
              📁 只打开一个文件夹
            </button>
            <button className="btn btn-secondary" onClick={() => handleQuickOpen('archive')} disabled={opening}>
              📄 只打开一个压缩包
            </button>
          </div>
        ) : (
          <p className="welcome-screen-desc">
            <span className="welcome-screen-sub">
              加入漫画需要桌面应用（扫描/打开本机目录）；手机/平板端只能阅读已入库的内容。
            </span>
          </p>
        )}
      </div>
    );
  }

  return (
    <div className="library-layout">
      {/* 侧边栏过滤器 */}
      {showSidebar && (
        <div className="library-sidebar">
          {/* 分类过滤 */}
          {categories.length > 0 && (
            <div className="filter-section">
              <div className="filter-section-title">分类</div>
              {sortedCategories.map(c => (
                <div
                  key={c.id}
                  className={`filter-tag ${selectedCategory === c.id ? 'active' : ''}`}
                  onClick={() => handleCategoryFilter(c.id)}
                  title={c.search ? `动态分类：${c.search}` : undefined}
                >
                  <span style={{ width: 8, height: 8, borderRadius: '50%', background: c.color, flexShrink: 0 }} />
                  <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {c.pinned ? '📌 ' : ''}{c.name}
                  </span>
                  <span className="count">{c.archive_count}</span>
                </div>
              ))}
            </div>
          )}

          {/* 日期过滤（按添加时间：年 → 月，本机时区分桶） */}
          {dateTree && dateTree.years.length > 0 && (
            <div className="filter-section">
              <div className="filter-section-title">日期</div>
              {dateTree.years.map(y => {
                const yearFrom = `${y.year}-01-01`;
                const yearActive = addedRange && addedRange.added_from === yearFrom;
                const expanded = expandedYears.has(y.year);
                return (
                  <div key={y.year}>
                    <div
                      className={`filter-tag ${yearActive ? 'active' : ''}`}
                      onClick={() => handleDateFilter(y.year)}
                    >
                      <span
                        role="button"
                        aria-label={expanded ? `收起 ${y.year} 年的月份` : `展开 ${y.year} 年的月份`}
                        onClick={(e) => { e.stopPropagation(); toggleYear(y.year); }}
                        style={{ flexShrink: 0, width: 12, cursor: 'pointer', opacity: 0.7 }}
                      >
                        {expanded ? '▾' : '▸'}
                      </span>
                      <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {y.year}年
                      </span>
                      <span className="count">{y.count}</span>
                    </div>
                    {expanded && y.months.map(m => {
                      const monthFrom = `${y.year}-${String(m.month).padStart(2, '0')}-01`;
                      const monthActive = addedRange && addedRange.added_from === monthFrom;
                      return (
                        <div
                          key={m.month}
                          className={`filter-tag ${monthActive ? 'active' : ''}`}
                          style={{ paddingLeft: 24 }}
                          onClick={() => handleDateFilter(y.year, m.month)}
                        >
                          <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                            {m.month}月
                          </span>
                          <span className="count">{m.count}</span>
                        </div>
                      );
                    })}
                  </div>
                );
              })}
            </div>
          )}

          {/* 标签过滤 */}
          <div className="filter-section">
            <div className="filter-section-title">标签</div>
            {showTagSearch && (
              <input
                type="text"
                value={tagSearch}
                onChange={(e) => setTagSearch(e.target.value)}
                placeholder="过滤标签..."
                style={{ width: '100%', marginBottom: 8, fontSize: 12 }}
                aria-label="按名称过滤标签"
              />
            )}
            {Object.keys(filteredTagsByNamespace).length === 0 ? (
              <div style={{ color: 'var(--text-tertiary)', fontSize: 12, padding: 4 }}>无匹配标签</div>
            ) : (
              Object.entries(filteredTagsByNamespace).map(([ns, nsTags]) => (
                <div key={ns} style={{ marginBottom: 8 }}>
                  {ns !== NS_OTHER && (
                    <div style={{ fontSize: 11, color: 'var(--text-tertiary)', padding: '2px 0' }}>{ns}</div>
                  )}
                  {nsTags.map(t => {
                    const fullName = t.namespace ? `${t.namespace}:${t.name}` : t.name;
                    return (
                      <div
                        key={t.id}
                        className={`filter-tag ${selectedTag === fullName ? 'active' : ''}`}
                        onClick={() => handleTagFilter(fullName)}
                      >
                        <span style={{ width: 8, height: 8, borderRadius: '50%', background: t.color, flexShrink: 0 }} />
                        <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{t.name}</span>
                        <span className="count">{t.archive_count}</span>
                      </div>
                    );
                  })}
                </div>
              ))
            )}
          </div>
        </div>
      )}

      {/* 主内容区 */}
      <div className="library-main" ref={bindListScroll}>
        {/* 顶栏 */}
        <div className="library-header">
          <input
            ref={searchInputRef}
            className="search-input"
            placeholder="搜索漫画…（按 / 聚焦）"
            value={search}
            onChange={(e) => handleSearch(e.target.value)}
            aria-label="搜索漫画"
            style={{ maxWidth: 280 }}
          />
          {/* 语法以前只写在 placeholder 里，等于没文档；给一个看得见的入口 */}
          <button
            className="btn btn-secondary btn-icon"
            onClick={() => setShowSearchHelp(true)}
            title="搜索语法与快捷键"
            aria-label="搜索语法与快捷键"
          >?</button>

          <div className="spacer" />

          <select value={typeFilter} onChange={(e) => handleTypeFilter(e.target.value)} style={{ minWidth: 92 }} aria-label="档案类型">
            <option value="all">全部类型</option>
            <option value="folder">文件夹</option>
            <option value="archive">压缩包</option>
          </select>

          <select value={readFilter} onChange={(e) => setReadFilter(e.target.value)} style={{ minWidth: 88 }} aria-label="阅读状态">
            <option value="all">全部</option>
            <option value="unread">未读</option>
            <option value="in_progress">在读</option>
            <option value="finished">已读完</option>
          </select>

          {/* 标签状态：与阅读状态并列，因为"哪些还没整理过"是和"哪些还没读完"同级的
              日常问题；选到「未打标签」时旁边会出现进入整理模式的入口 */}
          <select value={tagState} onChange={(e) => setTagState(e.target.value)} style={{ minWidth: 96 }} aria-label="标签状态">
            <option value="all">全部标签</option>
            <option value="untagged">未打标签</option>
            <option value="tagged">已打标签</option>
          </select>

          <select value={sortBy} onChange={(e) => { randomSeedRef.current = null; setSortBy(e.target.value); }} style={{ minWidth: 100 }} aria-label="排序方式">
            <option value="updated">最近阅读</option>
            <option value="name">名称</option>
            <option value="created">添加时间</option>
            <option value="pages">页数</option>
            <option value="size">大小</option>
            <option value="random">随机</option>
          </select>

          <div className="toggle-group" role="group" aria-label="视图模式">
            <button className={viewMode === 'grid' ? 'active' : ''} onClick={() => handleViewMode('grid')} title="网格" aria-label="网格视图">▦</button>
            <button className={viewMode === 'list' ? 'active' : ''} onClick={() => handleViewMode('list')} title="列表" aria-label="列表视图">☰</button>
          </div>

          {viewMode === 'grid' && (
            <div className="toggle-group" role="group" aria-label="卡片密度">
              <button className={cardDensity === 'large' ? 'active' : ''} onClick={() => handleDensityChange('large')} title="大封面" aria-label="大封面">大</button>
              <button className={cardDensity === 'normal' ? 'active' : ''} onClick={() => handleDensityChange('normal')} title="标准尺寸" aria-label="标准封面">中</button>
              <button className={cardDensity === 'compact' ? 'active' : ''} onClick={() => handleDensityChange('compact')} title="紧凑（封面优先）" aria-label="紧凑封面">小</button>
            </div>
          )}

          {hasActiveFilter && (
            <button
              className="btn btn-secondary"
              onClick={clearFilters}
              title="清除搜索、标签、分类、日期与状态筛选"
            >
              ✕ 清除筛选
            </button>
          )}

          <button className="btn btn-secondary" onClick={() => setShowSidebar(v => !v)} title="过滤器" aria-label={showSidebar ? '隐藏过滤器' : '显示过滤器'}>
            {showSidebar ? '◁' : '▷'}
          </button>

          {/* 整理模式入口：与「选择」并列，因为它是"批量操作"的另一种形态——
              选择是"先选哪些"，整理是"一本一本来"，后者对给上百本打标签更省手 */}
          <button
            className="btn btn-secondary"
            onClick={() => {
              // 让背后的列表与整理模式说同一件事：进去整理未打标签的，列表也看未打标签
              setTagState('untagged');
              setShowTriage(true);
            }}
            title="逐个给未打标签的漫画打标签（键盘操作，Enter 打标并下一本）"
          >
            🏷️ 整理
          </button>

          {selectMode ? (
            <button className="btn btn-secondary" onClick={handleExitSelectMode}>取消选择</button>
          ) : (
            <button className="btn btn-secondary" onClick={() => { setSelectMode(true); setSelectedIds(new Set()); }}>选择</button>
          )}

          {isNarrow ? (
            <button
              className="btn btn-secondary btn-icon"
              onClick={() => setShowMobileMenu(v => !v)}
              title="更多操作"
              aria-label="打开更多操作菜单"
              aria-expanded={showMobileMenu}
            >⋯</button>
          ) : (
            <ArchiveActionButtons
              isTauri={isTauri}
              opening={opening}
              loading={loading}
              packingCbz={packingCbz}
              scanning={scanningDir}
              onScanDir={handleScanDirectory}
              onOpenFolder={() => handleQuickOpen('folder')}
              onOpenArchive={() => handleQuickOpen('archive')}
              onConvertCbz={handleConvertFolderToCbz}
            />
          )}
        </div>

        {/* 窄屏：折叠次要操作 */}
        {isNarrow && showMobileMenu && (
          <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', padding: '8px 0', borderBottom: '1px solid var(--border)', marginBottom: 8 }}>
            <button
              className="btn btn-secondary btn-sm"
              onClick={() => { setTagState('untagged'); setShowTriage(true); setShowMobileMenu(false); }}
            >
              🏷️ 整理
            </button>
            <ArchiveActionButtons
              isTauri={isTauri}
              opening={opening}
              loading={loading}
              packingCbz={packingCbz}
              scanning={scanningDir}
              variant="mobile"
              onScanDir={() => { handleScanDirectory(); setShowMobileMenu(false); }}
              onOpenFolder={() => { handleQuickOpen('folder'); setShowMobileMenu(false); }}
              onOpenArchive={() => { handleQuickOpen('archive'); setShowMobileMenu(false); }}
              onConvertCbz={() => { handleConvertFolderToCbz(); setShowMobileMenu(false); }}
            />
          </div>
        )}

        {/* 继续阅读：打开应用后最常见的动作是「接着上次看」，把它放在列表之前，
            一次点击即可续读；这里展示的是服务端《在读 + 最近阅读》的前 N 条 */}
        {showContinue && (
          <section className="continue-strip" aria-label="继续阅读">
            <div className="continue-strip-head">
              <span className="continue-strip-title">继续阅读</span>
              <button
                className="btn btn-sm btn-secondary"
                onClick={() => setReadFilter('in_progress')}
                title="在书库中筛出所有读到一半的漫画"
              >
                查看全部在读
              </button>
            </div>
            <div className="continue-strip-row">
              {continueItems.map(a => (
                <button
                  key={a.id}
                  type="button"
                  className="continue-item"
                  onClick={() => openArchive(a.id)}
                  title={`继续阅读《${a.title}》`}
                >
                  <span className="continue-item-cover">
                    <LazyImage src={a.cover_url} alt="" />
                    <span className="archive-card-progress">
                      <span
                        className="archive-card-progress-bar"
                        style={{
                          width: `${a.page_count > 0
                            ? Math.min(100, (((a.read_page || 0) + 1) / a.page_count) * 100)
                            : 0}%`,
                        }}
                      />
                    </span>
                  </span>
                  <span className="continue-item-title">{a.title}</span>
                  <span className="continue-item-meta">{formatReadProgress(a)}</span>
                </button>
              ))}
            </div>
          </section>
        )}

        {/* 档案列表 */}
        {loading && displayArchives.length === 0 ? (
          <div className="archive-grid">
            {Array.from({ length: 8 }).map((_, i) => (
              <div key={`skeleton-${i}`} className="archive-card skeleton-card">
                <div className="archive-card-cover skeleton-pulse" style={{ background: 'var(--border)' }} />
                <div className="archive-card-info">
                  <div className="skeleton-pulse" style={{ height: 16, width: '70%', background: 'var(--border)', borderRadius: 4 }} />
                  <div className="skeleton-pulse" style={{ height: 12, width: '40%', background: 'var(--border)', borderRadius: 4, marginTop: 6 }} />
                </div>
              </div>
            ))}
          </div>
        ) : displayArchives.length === 0 ? (
          <div className="empty-state">
            <div className="empty-state-icon">{hasActiveFilter ? '🔍' : '📚'}</div>
            <div className="empty-state-text">
              {hasActiveFilter ? '没有匹配的漫画' : '书库是空的'}
            </div>
            {hasActiveFilter ? (
              <>
                {/* 说清楚"是哪几个条件把结果筛没了"——只写"没有匹配"用户无从下手 */}
                <div className="empty-state-sub">当前筛选：{activeFilterSummary}</div>
                <button className="btn btn-secondary" onClick={clearFilters}>✕ 清除筛选</button>
              </>
            ) : (
              // 走到这里 typeFilter 必然是「全部」（否则 hasActiveFilter 为真），无需再按类型分支
              <div className="empty-state-sub">点上面的「扫描漫画目录」把漫画加入书库</div>
            )}
          </div>
        ) : viewMode === 'grid' ? (
          <div className={`archive-grid${cardDensity === 'normal' ? '' : ` density-${cardDensity}`}`}>
            {displayArchives.map(a => (
              <Fragment key={a.id}>
                <ArchiveCard
                  a={a}
                  compact={cardDensity === 'compact'}
                  isSelected={selectedIds.has(a.id)}
                  isKbFocused={kbIndex >= 0 && displayArchives[kbIndex]?.id === a.id}
                  selectMode={selectMode}
                  isExpanded={a._isGroup && expandedGroup === (a._autoGroup ? a._autoKey : `g:${a.id}`)}
                  onOpen={openArchive}
                  onToggleGroup={toggleGroup}
                  onToggleSelect={handleToggleSelect}
                  onTag={handleOpenTagPicker}
                  onCategory={handleOpenCategoryPicker}
                  onRename={handleOpenRename}
                  onRemove={handleRemoveArchive}
                  onOpenSheet={setSheetTarget}
                />
                {a._isGroup && expandedGroup === (a._autoGroup ? a._autoKey : `g:${a.id}`) && (
                  <GroupChapterPanel
                    loading={groupLoading}
                    members={groupMembers}
                    onOpenChapter={openArchive}
                    onTag={handleOpenTagPicker}
                    onCategory={handleOpenCategoryPicker}
                    onRename={handleOpenRename}
                    onRemove={handleRemoveArchive}
                  />
                )}
              </Fragment>
            ))}
          </div>
        ) : (
          <div className="archive-list">
            {displayArchives.map(a => (
              <Fragment key={a.id}>
                <ArchiveListItem
                  a={a}
                  isSelected={selectedIds.has(a.id)}
                  isKbFocused={kbIndex >= 0 && displayArchives[kbIndex]?.id === a.id}
                  selectMode={selectMode}
                  isExpanded={a._isGroup && expandedGroup === (a._autoGroup ? a._autoKey : `g:${a.id}`)}
                  onOpen={openArchive}
                  onToggleGroup={toggleGroup}
                  onToggleSelect={handleToggleSelect}
                  onTag={handleOpenTagPicker}
                  onCategory={handleOpenCategoryPicker}
                  onRename={handleOpenRename}
                  onRemove={handleRemoveArchive}
                  onOpenSheet={setSheetTarget}
                />
                {a._isGroup && expandedGroup === (a._autoGroup ? a._autoKey : `g:${a.id}`) && (
                  <GroupChapterPanel
                    loading={groupLoading}
                    members={groupMembers}
                    onOpenChapter={openArchive}
                    onTag={handleOpenTagPicker}
                    onCategory={handleOpenCategoryPicker}
                    onRename={handleOpenRename}
                    onRemove={handleRemoveArchive}
                  />
                )}
              </Fragment>
            ))}
          </div>
        )}

        {/* 加载更多按钮 */}
        {hasMore && displayArchives.length > 0 && (
          <>
            <div style={{ display: 'flex', justifyContent: 'center', padding: '24px 0' }}>
              <button
                className="btn btn-secondary"
                onClick={handleLoadMore}
                disabled={loadingMore}
              >
                {loadingMore ? '加载中...' : `加载更多 (已显示 ${displayArchives.length})`}
              </button>
            </div>
            {/* 触底自动加载哨兵 */}
            <div ref={loadMoreSentinelRef} aria-hidden="true" style={{ height: 1 }} />
          </>
        )}
      </div>

      {/* 多选合并浮动工具栏 */}
      {selectMode && (
        <div className="select-toolbar">
          <span>已选 {selectedIds.size} 个</span>
          <button className="btn" onClick={handleMerge} disabled={selectedIds.size < 2}>
            合并
          </button>
          <button className="btn btn-secondary" onClick={() => setBatchTagPickerOpen(true)} disabled={selectedIds.size === 0}>
            打标签
          </button>
          <button className="btn btn-secondary" onClick={() => setBatchCategoryPickerOpen(true)} disabled={selectedIds.size === 0}>
            分类
          </button>
          <button className="btn btn-secondary" onClick={() => setConvertConfirmOpen(true)} disabled={selectedIds.size === 0}>
            转为 CBZ
          </button>
          <button className="btn btn-danger" onClick={() => setBatchDeleteConfirmOpen(true)} disabled={selectedIds.size === 0}>
            删除
          </button>
          <button className="btn btn-secondary" onClick={handleExitSelectMode}>
            取消
          </button>
        </div>
      )}

      {/* 批量打标签 / 批量分类弹窗 */}
      {batchTagPickerOpen && (
        <TagPicker archiveIds={Array.from(selectedIds)} onClose={handleCloseBatchTagPicker} />
      )}
      {batchCategoryPickerOpen && (
        <CategoryPicker archiveIds={Array.from(selectedIds)} onClose={handleCloseBatchCategoryPicker} />
      )}

      {/* 批量删除确认 */}
      <ConfirmDialog
        open={batchDeleteConfirmOpen}
        title="批量删除"
        message={`确定要从库中移除已选的 ${selectedIds.size} 个档案吗？此操作不会删除磁盘上的源文件。`}
        confirmText="删除"
        danger
        onConfirm={handleBatchDelete}
        onCancel={() => setBatchDeleteConfirmOpen(false)}
      />

      {/* 转换为 CBZ 确认（成功后删除原文件） */}
      <ConfirmDialog
        open={convertConfirmOpen}
        title="转换为 CBZ"
        // 列入真实文件名：这是唯一真正删磁盘文件的操作，只说"N 个"等于让用户
        // 在看不见代价的情况下按下不可撤销的按钮
        message={(
          <>
            将把选中的 {selectedIds.size} 个档案转换为同目录 CBZ，并在成功后删除原文件。
            此操作不可撤销（标签 / 阅读历史 / 书签会保留）。
            {convertTargetTitles.length > 0 && (
              <span style={{ display: 'block', marginTop: 8, color: 'var(--text-primary)' }}>
                将删除以下原文件：
                {convertTargetTitles.slice(0, 5).map(t => (
                  <span key={t} style={{ display: 'block', fontSize: 12 }}>· {t}</span>
                ))}
                {convertTargetTitles.length > 5 && (
                  <span style={{ display: 'block', fontSize: 12 }}>
                    · 等共 {convertTargetTitles.length} 个
                  </span>
                )}
              </span>
            )}
          </>
        )}
        confirmText="开始转换"
        danger
        onConfirm={() => {
          setConvertConfirmOpen(false);
          const ids = Array.from(selectedIds);
          handleExitSelectMode();
          startConvert(ids);
        }}
        onCancel={() => setConvertConfirmOpen(false)}
      />

      {/* 转换进度浮动面板（后台任务，切页也持续） */}
      {convertingCbz && (
        <div className="cbz-convert-float">
          <CbzConvertPanel info={convertInfo} onCancel={cancelConvert} />
        </div>
      )}

      {/* 搜索语法 + 快捷键帮助 */}
      {showSearchHelp && (
        <Modal onClose={() => setShowSearchHelp(false)} ariaLabel="搜索语法与快捷键">
          <h3 style={{ marginBottom: 12 }}>🔍 搜索语法与快捷键</h3>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <tbody>
              {[
                ['关键词', '匹配标题或标签名；空格分隔多个词表示「并且」'],
                ['tag:xxx', '只匹配带该标签的（支持 tag:artist:作者名）'],
                ['-xxx', '排除标题或标签名里含 xxx 的'],
                ['', ''],
                ['/', '聚焦搜索框'],
                ['j / k', '在结果里上下移动高亮'],
                ['Enter', '打开高亮的漫画（组则展开章节）'],
                ['x', '把高亮的漫画加入多选'],
                ['Esc', '退出多选 / 取消高亮'],
                ['?', '打开这个面板'],
                ['⌘K / Ctrl-K', '命令面板（所有操作的统一入口）'],
              ].map(([key, desc], i) => (
                key === '' ? <tr key={i}><td colSpan={2} style={{ height: 8 }} /></tr> : (
                  <tr key={i}>
                    <td style={{ padding: '5px 12px 5px 0', fontFamily: 'monospace', fontWeight: 600, whiteSpace: 'nowrap', color: 'var(--accent)' }}>{key}</td>
                    <td style={{ padding: '5px 0', color: 'var(--text-secondary)' }}>{desc}</td>
                  </tr>
                )
              ))}
            </tbody>
          </table>
        </Modal>
      )}

      {/* 卡片操作面板（触屏：长按卡片或点「⋯」） */}
      {sheetTarget && (
        <CardActionSheet
          title={sheetTarget.title}
          subtitle={sheetTarget._isGroup ? `${sheetTarget.chapter_count} 话` : `${sheetTarget.page_count} 页`}
          items={sheetItems}
          onClose={() => setSheetTarget(null)}
        />
      )}

      {/* 整理模式（键盘逐本打标签） */}
      {showTriage && (
        <TagTriage
          sortBy={sortBy}
          sortOrder={sortOrder}
          onClose={() => {
            setShowTriage(false);
            // 整理期间打过的书已离开「未打标签」，后台列表要跟上
            loadArchives({ search: searchRef.current, tag: selectedTagRef.current });
          }}
        />
      )}

      {/* 重命名弹窗 */}
      {renamingId && (
        <Modal onClose={() => setRenamingId(null)} ariaLabel="重命名漫画">
          <div className="modal-title">重命名漫画</div>
          <div className="modal-body">
              <p style={{ color: 'var(--text-secondary)', fontSize: 13, marginBottom: 12 }}>
                输入新名称，或点击下方路径中的某一层快速采用
              </p>
              <input
                className="modal-input"
                value={renameValue}
                onChange={(e) => setRenameValue(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleConfirmRename()}
                autoFocus
                style={{ width: '100%', boxSizing: 'border-box', marginBottom: 12 }}
              />
              {(() => {
                const a = archives.find(x => x.id === renamingId);
                if (!a || !a.path) return null;
                // Windows 路径为反斜杠，需按两种分隔符拆分
                const allParts = splitPathParts(a.path);
                const depth = parseInt(settings.rename_suggest_depth, 10) || 3;
                const parts = depth > 0 ? allParts.slice(-depth) : allParts;
                if (parts.length <= 1) return null;
                return (
                  <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                    {parts.map((p, i) => (
                      <button
                        key={i}
                        className="btn btn-secondary btn-sm"
                        onClick={() => setRenameValue(p)}
                        title={parts.slice(0, i + 1).join('/')}
                      >
                        {p}
                      </button>
                    ))}
                  </div>
                );
              })()}
            </div>
            <div className="modal-actions">
              <button className="btn btn-secondary" onClick={() => setRenamingId(null)}>取消</button>
              <button className="btn" onClick={handleConfirmRename} disabled={!renameValue.trim()}>确认</button>
            </div>
        </Modal>
      )}

      {/* 打开文件弹窗 */}
      {showOpenModal && (
        <Modal onClose={() => setShowOpenModal(false)} ariaLabel="打开漫画文件">
          <div className="modal-title">打开漫画文件</div>
          <div className="modal-body">
              <p style={{ color: 'var(--text-secondary)', fontSize: 13, marginBottom: 12 }}>
                输入文件或文件夹路径，支持图片文件夹和压缩包 (ZIP/CBZ/RAR/CBR/7Z)
              </p>
              <input
                className="modal-input"
                placeholder={'例: D:\\Manga\\Title 或 C:\\Manga\\comic.cbz（也支持 macOS/Linux 路径）'}
                value={openPath}
                onChange={(e) => setOpenPath(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleOpenFile()}
                autoFocus
                style={{ width: '100%', boxSizing: 'border-box' }}
              />
            </div>
            <div className="modal-actions">
              <button className="btn btn-secondary" onClick={() => setShowOpenModal(false)}>取消</button>
              <button className="btn" onClick={handleOpenFile} disabled={opening || !openPath.trim()}>
                {opening ? '打开中...' : '打开'}
              </button>
            </div>
        </Modal>
      )}

      {/* CBZ 打包全局遮罩 */}
      {packingCbz && (
        <div className="modal-overlay" style={{ cursor: 'wait' }}>
          <div style={{ textAlign: 'center', color: '#fff' }}>
            <div style={{ fontSize: 48, marginBottom: 16 }}>📦</div>
            <div style={{ fontSize: 16, fontWeight: 600 }}>正在打包为 CBZ...</div>
            <div style={{ fontSize: 13, marginTop: 8, opacity: 0.7 }}>请勿关闭窗口</div>
          </div>
        </div>
      )}

      {/* 标签选择弹窗 */}
      {tagPickerArchiveId && (
        <TagPicker archiveId={tagPickerArchiveId} onClose={handleCloseTagPicker} />
      )}

      {/* 分类选择弹窗 */}
      {categoryPickerArchiveId && (
        <CategoryPicker archiveId={categoryPickerArchiveId} onClose={handleCloseCategoryPicker} />
      )}

      {/* 删除确认弹窗 */}
      <ConfirmDialog
        open={confirmOpen}
        title="移除漫画"
        message="确定从库中移除该漫画？此操作不会删除实际文件。"
        danger
        confirmText="移除"
        onConfirm={handleConfirmRemove}
        onCancel={() => setConfirmOpen(false)}
      />
    </div>
  );
}

// 命名空间标签默认分组 key
const NS_OTHER = '_other';

// 漫画库操作按钮组（桌面 / 移动端共用）
function ArchiveActionButtons({ isTauri, opening, packingCbz, scanning, variant, onScanDir, onOpenFolder, onOpenArchive, onConvertCbz }) {
  const sizeClass = variant === 'mobile' ? 'btn-sm' : '';

  return (
    <>
      {/* 扫描目录放最前：它是唯一能一次加入整批漫画的动作 */}
      {isTauri && (
        <button className={`btn btn-secondary ${sizeClass}`} onClick={onScanDir} disabled={scanning}>
          {scanning ? '⏳ 扫描中...' : '🗂️ 扫描目录'}
        </button>
      )}
      <button className={`btn btn-secondary ${sizeClass}`} onClick={onOpenFolder} disabled={opening}>
        📁 打开文件夹
      </button>
      <button className={`btn btn-secondary ${sizeClass}`} onClick={onOpenArchive} disabled={opening}>
        📄 打开压缩包
      </button>
      {isTauri && (
        <button className={`btn btn-secondary ${sizeClass}`} onClick={onConvertCbz} disabled={packingCbz}>
          {packingCbz ? '⏳ 打包中...' : '📦 转换 CBZ'}
        </button>
      )}
    </>
  );
}
