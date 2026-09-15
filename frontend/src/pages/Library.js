import React, { useState, useEffect, useMemo, useRef, useCallback, Fragment } from 'react';
import { useNavigate } from 'react-router-dom';
import api from '../utils/api';
import { formatSize, splitPathParts, lastPathPart } from '../utils/format';
import { useToast } from '../components/Toast';
import useSettings from '../hooks/useSettings';
import useTags from '../hooks/useTags';
import useLibrarySession from '../hooks/useLibrarySession';
import LazyImage from '../components/LazyImage';
import TagPicker from '../components/TagPicker';
import CategoryPicker from '../components/CategoryPicker';
import ConfirmDialog from '../components/ConfirmDialog';
import Modal from '../components/Modal';

// 检测是否在 Tauri 环境中
const isTauri = window.__TAURI__ !== undefined;

// 网格卡片：memoized，避免多选切换时整屏重渲染。
// 所有回调通过 props 传入（父组件 useCallback 稳定引用）。
const ArchiveCard = React.memo(function ArchiveCard({ a, compact, isSelected, selectMode, isExpanded, onOpen, onToggleGroup, onToggleSelect, onTag, onCategory, onRename, onRemove }) {
  return (
    <div
      className={`archive-card ${selectMode && isSelected ? 'archive-card-selected' : ''}`}
      onClick={(e) => {
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
        {a.read_page > 0 && (
          <div className="archive-card-progress">
            <div className="archive-card-progress-bar" style={{ width: `${(a.read_page / (a.page_count || 1)) * 100}%` }} />
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
          {a.file_size > 0 && <span>· {formatSize(a.file_size)}</span>}
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
const ArchiveListItem = React.memo(function ArchiveListItem({ a, isSelected, selectMode, isExpanded, onOpen, onToggleGroup, onToggleSelect, onTag, onCategory, onRename, onRemove }) {
  return (
    <div
      className={`archive-list-item ${selectMode && isSelected ? 'archive-list-item-selected' : ''}`}
      onClick={(e) => {
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
          {a.read_page > 0 && ` · 已读 ${a.read_page}/${a.page_count || '?'}`}
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
  const [readFilter, setReadFilter] = useState('all'); // all | read | unread
  // 档案类型筛选：all | folder | archive（压缩包）。统一书库默认展示全部类型，
  // 该筛选仅收窄显示（原“漫画库/收藏”双 tab 合并而来，类型不再是顶层导航位）。
  const [typeFilter, setTypeFilter] = useState(() => {
    const v = settings.type_filter;
    return v === 'folder' || v === 'archive' ? v : 'all';
  });
  const [categories, setCategories] = useState([]);
  const [selectedCategory, setSelectedCategory] = useState(null);
  const [showSidebar, setShowSidebar] = useState(true);
  const [showOpenModal, setShowOpenModal] = useState(false);
  const [openPath, setOpenPath] = useState('');
  const [opening, setOpening] = useState(false);
  const [packingCbz, setPackingCbz] = useState(false);
  const [showMobileMenu, setShowMobileMenu] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [confirmTarget, setConfirmTarget] = useState(null);
  // 重命名弹窗
  const [renamingId, setRenamingId] = useState(null);
  const [renameValue, setRenameValue] = useState('');
  // 多选模式
  const [selectMode, setSelectMode] = useState(false);
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
  const typeFilterRef = useRef(typeFilter);
  const selectedCategoryRef = useRef(selectedCategory);
  const searchRef = useRef(search);
  const requestIdRef = useRef(0);
  const appendLockRef = useRef(false); // 防触底自动加载与按钮点击重复追加同一页
  const loadMoreSentinelRef = useRef(null); // 触底自动加载观察哨兵
  const listScrollRef = useRef(null); // 列表滚动容器
  // 会话恢复写入的筛选值快照：用于在恢复后跳过“筛选变化重拉”，避免覆盖恢复的分页
  const restoredFiltersRef = useRef(null);
  const navigate = useNavigate();
  const toast = useToast();

  // 浏览会话：卸载时保存（含滚动位置）、进入时后台与服务器比对
  const { librarySessions, reconcileLibrary } = useLibrarySession({
    mode,
    sessionEnabled,
    listScrollRef,
    snapshot: {
      archives, page: pageRef.current, hasMore,
      search, sortBy, sortOrder, selectedTag, readFilter, typeFilter, selectedCategory,
      expandedGroup, groupMembers,
    },
    filterRefs: { sortByRef, searchRef, selectedTagRef, readFilterRef, selectedCategoryRef },
    pageRef,
    pageSize: PAGE_SIZE,
    setArchives, setHasMore, setExpandedGroup, setGroupMembers,
  });

  // 保持 refs 同步
  useEffect(() => { sortByRef.current = sortBy; }, [sortBy]);
  useEffect(() => { sortOrderRef.current = sortOrder; }, [sortOrder]);
  useEffect(() => { selectedTagRef.current = selectedTag; }, [selectedTag]);
  useEffect(() => { readFilterRef.current = readFilter; }, [readFilter]);
  useEffect(() => { typeFilterRef.current = typeFilter; }, [typeFilter]);
  useEffect(() => { selectedCategoryRef.current = selectedCategory; }, [selectedCategory]);
  useEffect(() => { searchRef.current = search; }, [search]);
  useEffect(() => { expandedGroupRef.current = expandedGroup; }, [expandedGroup]);

  const reloadCategories = useCallback(() => {
    return api.getCategories().then(data => { setCategories(data); return data; }).catch(() => []);
  }, []);

  useEffect(() => {
    const s = sessionEnabled ? librarySessions[mode] : null;
    if (s && s.archives && s.archives.length > 0) {
      // 恢复浏览会话：秒开旧列表，保留已加载分页、展开状态与滚动位置
      setSearch(s.search); searchRef.current = s.search;
      setSortBy(s.sortBy); sortByRef.current = s.sortBy;
      setSortOrder(s.sortOrder); sortOrderRef.current = s.sortOrder;
      setSelectedTag(s.selectedTag); selectedTagRef.current = s.selectedTag;
      setReadFilter(s.readFilter || 'all'); readFilterRef.current = s.readFilter || 'all';
      setTypeFilter(s.typeFilter || 'all'); typeFilterRef.current = s.typeFilter || 'all';
      setSelectedCategory(s.selectedCategory); selectedCategoryRef.current = s.selectedCategory;
      // 记录本轮恢复写入的筛选值：上述 setState 提交后，“筛选变化重拉”effect 会看到
      // 与这里相同的值并跳过，避免把恢复好的分页/滚动位置覆盖成第 1 页
      restoredFiltersRef.current = {
        sortBy: s.sortBy, sortOrder: s.sortOrder, selectedTag: s.selectedTag,
        readFilter: s.readFilter || 'all', selectedCategory: s.selectedCategory,
        typeFilter: s.typeFilter || 'all',
      };
      setArchives(s.archives);
      pageRef.current = s.page;
      setHasMore(s.hasMore);
      if (s.expandedGroup) {
        setExpandedGroup(s.expandedGroup);
        if (s.groupMembers) setGroupMembers(s.groupMembers);
      }
      if (s.scrollTop) {
        requestAnimationFrame(() => {
          const el = listScrollRef.current;
          if (el) el.scrollTop = s.scrollTop;
        });
      }
      // 后台与服务器比对（仅内容重排时整体刷新，否则合并字段）
      reconcileLibrary(s);
    } else {
      loadArchives();
    }
    reloadCategories();
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
      const data = await api.getArchives(baseParams);
      if (id !== requestIdRef.current) return;
      setArchives(prev => append ? [...prev, ...data] : data);
      pageRef.current = nextPage;
      setHasMore(data.length >= PAGE_SIZE);
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
        r.selectedTag === selectedTag && r.readFilter === readFilter &&
        r.selectedCategory === selectedCategory && r.typeFilter === typeFilter;
      if (same) {
        // 仍是恢复写入的那组值：跳过重拉；用户一旦改动筛选，下次运行不再匹配即正常重拉
        return;
      }
      restoredFiltersRef.current = null;
    }
    loadArchives({ search: searchRef.current, tag: selectedTag, category_id: selectedCategory });
  }, [sortBy, sortOrder, selectedTag, readFilter, selectedCategory, typeFilter]);

  const handleSearch = useCallback((val) => {
    setSearch(val);
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

  const handleRemoveArchive = useCallback((e, id) => {
    e.stopPropagation();
    setConfirmTarget(id);
    setConfirmOpen(true);
  }, []);

  const handleConfirmRemove = async () => {
    setConfirmOpen(false);
    const id = confirmTarget;
    if (!id) return;
    try {
      await api.deleteArchive(id);
      toast('已移除', 'success');
      loadArchives({ search, tag: selectedTag });
    } catch (err) {
      toast(err.message, 'error');
    }
  };

  // TagPicker 状态
  const [tagPickerArchiveId, setTagPickerArchiveId] = useState(null);
  const handleOpenTagPicker = useCallback((e, id) => {
    e.stopPropagation();
    setTagPickerArchiveId(id);
  }, []);
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
  const handleOpenCategoryPicker = useCallback((e, id) => {
    e.stopPropagation();
    setCategoryPickerArchiveId(id);
  }, []);
  const handleCloseCategoryPicker = (changed) => {
    setCategoryPickerArchiveId(null);
    reloadAfterPick(changed, reloadCategories);
  };

  // 重命名
  const handleOpenRename = useCallback((e, a) => {
    e.stopPropagation();
    setRenamingId(a.id);
    setRenameValue(a.title);
  }, []);
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
  const handleToggleSelect = useCallback((e, id) => {
    e.stopPropagation();
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

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
      await api.batchDeleteArchives(ids);
      toast(`已删除 ${ids.length} 个档案`, 'success');
      handleExitSelectMode();
      loadArchives({ search, tag: selectedTag });
    } catch (e) {
      toast(e.message, 'error');
    }
  };

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

  // Welcome screen — 书库彻底为空时显示（合并“漫画库/收藏”双 tab 后不再按类型区分）
  if (archives.length === 0) {
    return (
      <div className="welcome-screen">
        <div className="welcome-screen-icon">📚</div>
        <h2>欢迎使用 MangaViewer</h2>
        <p className="welcome-screen-desc">
          打开漫画文件夹或压缩包即可开始阅读<br />
          <span className="welcome-screen-sub">
            支持文件夹、ZIP/CBZ、RAR/CBR、7Z 压缩包
          </span>
        </p>

        {/* 直接打开文件 */}
        {isTauri && (
          <div className="welcome-screen-actions">
            <button className="btn" onClick={() => handleQuickOpen('folder')} disabled={opening}>
              📁 打开文件夹
            </button>
            <button className="btn" onClick={() => handleQuickOpen('archive')} disabled={opening}>
              📄 打开压缩包
            </button>
          </div>
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
      <div className="library-main" ref={listScrollRef}>
        {/* 顶栏 */}
        <div className="library-header">
          <input
            className="search-input"
            placeholder="搜索漫画... (支持 tag:xxx、-排除)"
            value={search}
            onChange={(e) => handleSearch(e.target.value)}
            style={{ maxWidth: 280 }}
          />

          <div className="spacer" />

          <select value={typeFilter} onChange={(e) => handleTypeFilter(e.target.value)} style={{ minWidth: 92 }} aria-label="档案类型">
            <option value="all">全部类型</option>
            <option value="folder">文件夹</option>
            <option value="archive">压缩包</option>
          </select>

          <select value={readFilter} onChange={(e) => setReadFilter(e.target.value)} style={{ minWidth: 88 }} aria-label="阅读状态">
            <option value="all">全部</option>
            <option value="unread">未读</option>
            <option value="read">已读</option>
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

          <button className="btn btn-secondary" onClick={() => setShowSidebar(v => !v)} title="过滤器" aria-label={showSidebar ? '隐藏过滤器' : '显示过滤器'}>
            {showSidebar ? '◁' : '▷'}
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
              onOpenFolder={() => handleQuickOpen('folder')}
              onOpenArchive={() => handleQuickOpen('archive')}
              onConvertCbz={handleConvertFolderToCbz}
            />
          )}
        </div>

        {/* 窄屏：折叠次要操作 */}
        {isNarrow && showMobileMenu && (
          <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', padding: '8px 0', borderBottom: '1px solid var(--border)', marginBottom: 8 }}>
            <ArchiveActionButtons
              isTauri={isTauri}
              opening={opening}
              loading={loading}
              packingCbz={packingCbz}
              variant="mobile"
              onOpenFolder={() => { handleQuickOpen('folder'); setShowMobileMenu(false); }}
              onOpenArchive={() => { handleQuickOpen('archive'); setShowMobileMenu(false); }}
              onConvertCbz={() => { handleConvertFolderToCbz(); setShowMobileMenu(false); }}
            />
          </div>
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
            <div className="empty-state-icon">{typeFilter === 'archive' ? '📦' : '📚'}</div>
            <div className="empty-state-text">
              {search || selectedTag
                ? '没有匹配的漫画'
                : typeFilter === 'archive'
                  ? '暂无压缩包档案（CBZ/RAR/7Z）；可在左侧切换为「全部」查看文件夹漫画'
                  : typeFilter === 'folder'
                    ? '暂无文件夹档案；可在左侧切换为「全部」查看压缩包漫画'
                    : '点击「打开文件」添加漫画'}
            </div>
          </div>
        ) : viewMode === 'grid' ? (
          <div className={`archive-grid${cardDensity === 'normal' ? '' : ` density-${cardDensity}`}`}>
            {displayArchives.map(a => (
              <Fragment key={a.id}>
                <ArchiveCard
                  a={a}
                  compact={cardDensity === 'compact'}
                  isSelected={selectedIds.has(a.id)}
                  selectMode={selectMode}
                  isExpanded={a._isGroup && expandedGroup === (a._autoGroup ? a._autoKey : `g:${a.id}`)}
                  onOpen={openArchive}
                  onToggleGroup={toggleGroup}
                  onToggleSelect={handleToggleSelect}
                  onTag={handleOpenTagPicker}
                  onCategory={handleOpenCategoryPicker}
                  onRename={handleOpenRename}
                  onRemove={handleRemoveArchive}
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
                  selectMode={selectMode}
                  isExpanded={a._isGroup && expandedGroup === (a._autoGroup ? a._autoKey : `g:${a.id}`)}
                  onOpen={openArchive}
                  onToggleGroup={toggleGroup}
                  onToggleSelect={handleToggleSelect}
                  onTag={handleOpenTagPicker}
                  onCategory={handleOpenCategoryPicker}
                  onRename={handleOpenRename}
                  onRemove={handleRemoveArchive}
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
function ArchiveActionButtons({ isTauri, opening, packingCbz, variant, onOpenFolder, onOpenArchive, onConvertCbz }) {
  const sizeClass = variant === 'mobile' ? 'btn-sm' : '';

  return (
    <>
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
