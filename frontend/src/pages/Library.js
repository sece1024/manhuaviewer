import React, { useState, useEffect, useMemo, useRef, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import api from '../utils/api';
import { formatSize } from '../utils/format';
import { useToast } from '../components/Toast';
import useSettings from '../hooks/useSettings';
import useTags from '../hooks/useTags';
import LazyImage from '../components/LazyImage';
import TagPicker from '../components/TagPicker';
import CategoryPicker from '../components/CategoryPicker';
import ConfirmDialog from '../components/ConfirmDialog';

// 检测是否在 Tauri 环境中
const isTauri = window.__TAURI__ !== undefined;

export default function Library({ mode = 'library' }) {
  const { settings, updateSetting } = useSettings();
  const { tags, reload: reloadTags } = useTags();
  const [archives, setArchives] = useState([]);
  const [rootDir, setRootDir] = useState('');
  const [editingRoot, setEditingRoot] = useState(false);
  const [tempRoot, setTempRoot] = useState('');
  const [search, setSearch] = useState('');
  const [loading, setLoading] = useState(false);
  const [viewMode, setViewMode] = useState(() => settings.view_mode || 'grid');
  const [sortBy, setSortBy] = useState(() => settings.sort_by || 'updated');
  const [sortOrder, setSortOrder] = useState(() => settings.sort_order || 'desc');
  const [selectedTag, setSelectedTag] = useState('');
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
  // 窄屏：把次要操作收进 ⋯ 菜单
  const [isNarrow, setIsNarrow] = useState(() => typeof window !== 'undefined' && window.innerWidth < 768);
  // 分页状态
  const PAGE_SIZE = 50;
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const searchDebounceRef = useRef(null);
  const sortByRef = useRef(sortBy);
  const sortOrderRef = useRef(sortOrder);
  const selectedTagRef = useRef(selectedTag);
  const selectedCategoryRef = useRef(selectedCategory);
  const searchRef = useRef(search);
  const requestIdRef = useRef(0);
  const navigate = useNavigate();
  const toast = useToast();

  // 保持 refs 同步
  useEffect(() => { sortByRef.current = sortBy; }, [sortBy]);
  useEffect(() => { sortOrderRef.current = sortOrder; }, [sortOrder]);
  useEffect(() => { selectedTagRef.current = selectedTag; }, [selectedTag]);
  useEffect(() => { selectedCategoryRef.current = selectedCategory; }, [selectedCategory]);
  useEffect(() => { searchRef.current = search; }, [search]);

  const reloadCategories = useCallback(() => {
    return api.getCategories().then(data => { setCategories(data); return data; }).catch(() => []);
  }, []);

  useEffect(() => {
    api.getConfig().then(c => setRootDir(c.root_dir));
    loadArchives();
    reloadCategories();
    return () => clearTimeout(searchDebounceRef.current);
  }, []);

  useEffect(() => {
    const check = () => setIsNarrow(window.innerWidth < 768);
    window.addEventListener('resize', check);
    return () => window.removeEventListener('resize', check);
  }, []);

  const loadArchives = async (params = {}, append = false) => {
    const id = ++requestIdRef.current;
    if (append) {
      setLoadingMore(true);
    } else {
      setLoading(true);
    }
    try {
      const nextPage = append ? page + 1 : 1;
      const categoryId = params.category_id !== undefined ? params.category_id : selectedCategoryRef.current;
      const baseParams = {
        sort_by: sortByRef.current,
        sort_order: sortOrderRef.current,
        limit: PAGE_SIZE,
        page: nextPage,
        ...params,
      };
      if (categoryId) baseParams.category_id = categoryId;
      else delete baseParams.category_id;
      const data = await api.getArchives(baseParams);
      if (id !== requestIdRef.current) return;
      setArchives(prev => append ? [...prev, ...data] : data);
      setPage(nextPage);
      setHasMore(data.length >= PAGE_SIZE);
    } catch (e) {
      if (id === requestIdRef.current) toast(e.message, 'error');
    } finally {
      if (id === requestIdRef.current) {
        if (append) setLoadingMore(false);
        else setLoading(false);
      }
    }
  };

  useEffect(() => {
    loadArchives({ search: searchRef.current, tag: selectedTag, category_id: selectedCategory });
  }, [sortBy, sortOrder, selectedTag, selectedCategory]);

  const handleSaveRoot = async () => {
    try {
      await api.updateConfig(tempRoot);
      setRootDir(tempRoot);
      setEditingRoot(false);
      toast('根目录已更新', 'success');
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  const handleScan = async () => {
    setLoading(true);
    try {
      const result = await api.scan();
      toast(result.message, 'success');
      await loadArchives({ search, tag: selectedTag });
    } catch (e) {
      toast(e.message, 'error');
    }
    setLoading(false);
  };

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

  const handleRemoveArchive = (e, id) => {
    e.stopPropagation();
    setConfirmTarget(id);
    setConfirmOpen(true);
  };

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
  const handleOpenTagPicker = (e, id) => {
    e.stopPropagation();
    setTagPickerArchiveId(id);
  };
  const handleCloseTagPicker = (changed) => {
    setTagPickerArchiveId(null);
    if (changed) {
      loadArchives({ search, tag: selectedTag });
      reloadTags();
    }
  };

  // CategoryPicker 状态
  const [categoryPickerArchiveId, setCategoryPickerArchiveId] = useState(null);
  const handleOpenCategoryPicker = (e, id) => {
    e.stopPropagation();
    setCategoryPickerArchiveId(id);
  };
  const handleCloseCategoryPicker = (changed) => {
    setCategoryPickerArchiveId(null);
    if (changed) {
      loadArchives({ search, tag: selectedTag });
      reloadCategories();
    }
  };

  // 重命名
  const handleOpenRename = (e, a) => {
    e.stopPropagation();
    setRenamingId(a.id);
    setRenameValue(a.title);
  };
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
  const handleToggleSelect = (e, id) => {
    e.stopPropagation();
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };
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
    if (changed) {
      loadArchives({ search, tag: selectedTag });
      reloadTags();
    }
  };
  const handleCloseBatchCategoryPicker = (changed) => {
    setBatchCategoryPickerOpen(false);
    if (changed) {
      loadArchives({ search, tag: selectedTag });
      reloadCategories();
    }
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

  // 将档案按 group_id 聚合：同一组显示为一个卡片
  const groupedArchives = useMemo(() => {
    const groups = new Map(); // group_id -> [archives]
    const singles = [];

    for (const a of archives) {
      const gid = a.group_id;
      if (gid && gid === a.id) {
        // 主档案：以其为组的代表
        if (!groups.has(gid)) groups.set(gid, []);
        groups.get(gid).push(a);
      } else if (gid) {
        // 子档案：加入组
        if (!groups.has(gid)) groups.set(gid, []);
        groups.get(gid).push(a);
      } else {
        singles.push(a);
      }
    }

    // 每组只显示主档案卡片，附加 chapter_count
    const result = [];
    for (const [gid, members] of groups) {
      const primary = members.find(a => a.id === gid) || members[0];
      result.push({ ...primary, chapter_count: members.length, _isGroup: true });
    }
    for (const a of singles) {
      result.push(a);
    }
    return result;
  }, [archives]);

  // 将档案按类型分成收藏（压缩包）和文件夹两组
  const compressedArchives = useMemo(() => groupedArchives.filter(a => a.archive_type !== 'folder'), [groupedArchives]);
  const folderArchives = useMemo(() => groupedArchives.filter(a => a.archive_type === 'folder'), [groupedArchives]);

  // 根据 mode 决定展示哪组
  const isCollection = mode === 'collection';
  const displayArchives = isCollection ? compressedArchives : folderArchives;

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

  // Welcome screen — 仅漫画库模式下，无漫画且未配置根目录时显示
  if (!isCollection && !rootDir && !editingRoot && archives.length === 0) {
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

        {/* 配置根目录（折叠） */}
        <div className="welcome-screen-root">
          <p className="welcome-screen-root-hint">
            也可以配置漫画根目录，批量扫描导入
          </p>
          <div style={{ display: 'flex', gap: 8, justifyContent: 'center', flexWrap: 'wrap' }}>
            <input
              className="welcome-screen-root-input"
              placeholder="例: /home/user/manga"
              value={tempRoot}
              onChange={(e) => setTempRoot(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSaveRoot()}
            />
            <button className="btn btn-secondary" onClick={handleSaveRoot}>确认</button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="library-layout">
      {/* 侧边栏过滤器 */}
      {showSidebar && (
        <div className="library-sidebar">
          {/* 目录配置 — 仅漫画库模式 */}
          {!isCollection && rootDir && (
            <div className="filter-section">
              <div className="filter-section-title">目录</div>
              {editingRoot ? (
                <>
                  <input value={tempRoot} onChange={(e) => setTempRoot(e.target.value)} style={{ width: '100%', marginBottom: 8 }} />
                  <div style={{ display: 'flex', gap: 4 }}>
                    <button className="btn btn-sm" onClick={handleSaveRoot}>保存</button>
                    <button className="btn btn-sm btn-secondary" onClick={() => setEditingRoot(false)}>取消</button>
                  </div>
                </>
              ) : (
                <div
                  className="filter-tag"
                  onClick={() => { setTempRoot(rootDir); setEditingRoot(true); }}
                  title={rootDir}
                >
                  📂 {rootDir.length > 20 ? rootDir.slice(0, 20) + '...' : rootDir}
                </div>
              )}
            </div>
          )}

          {/* 分类过滤 */}
          {categories.length > 0 && (
            <div className="filter-section">
              <div className="filter-section-title">分类</div>
              {[...categories].sort((a, b) => (b.pinned ? 1 : 0) - (a.pinned ? 1 : 0)).map(c => (
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
      <div className="library-main">
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

          <select value={sortBy} onChange={(e) => setSortBy(e.target.value)} style={{ minWidth: 100 }} aria-label="排序方式">
            <option value="updated">最近阅读</option>
            <option value="name">名称</option>
            <option value="created">添加时间</option>
            <option value="pages">页数</option>
            <option value="size">大小</option>
          </select>

          <div className="toggle-group" role="group" aria-label="视图模式">
            <button className={viewMode === 'grid' ? 'active' : ''} onClick={() => handleViewMode('grid')} title="网格" aria-label="网格视图">▦</button>
            <button className={viewMode === 'list' ? 'active' : ''} onClick={() => handleViewMode('list')} title="列表" aria-label="列表视图">☰</button>
          </div>

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
              isCollection={isCollection}
              isTauri={isTauri}
              opening={opening}
              loading={loading}
              packingCbz={packingCbz}
              onOpenFolder={() => handleQuickOpen('folder')}
              onOpenArchive={() => handleQuickOpen('archive')}
              onConvertCbz={handleConvertFolderToCbz}
              onScan={handleScan}
            />
          )}
        </div>

        {/* 窄屏：折叠次要操作 */}
        {isNarrow && showMobileMenu && (
          <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', padding: '8px 0', borderBottom: '1px solid var(--border)', marginBottom: 8 }}>
            <ArchiveActionButtons
              isCollection={isCollection}
              isTauri={isTauri}
              opening={opening}
              loading={loading}
              packingCbz={packingCbz}
              variant="mobile"
              onOpenFolder={() => { handleQuickOpen('folder'); setShowMobileMenu(false); }}
              onOpenArchive={() => { handleQuickOpen('archive'); setShowMobileMenu(false); }}
              onConvertCbz={() => { handleConvertFolderToCbz(); setShowMobileMenu(false); }}
              onScan={() => { handleScan(); setShowMobileMenu(false); }}
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
            <div className="empty-state-icon">{isCollection ? '📦' : '📚'}</div>
            <div className="empty-state-text">
              {search || selectedTag ? '没有匹配的漫画' : isCollection ? '暂无收藏' : rootDir ? '暂无漫画，点击「扫描」按钮' : '点击「打开文件」添加漫画'}
            </div>
          </div>
        ) : viewMode === 'grid' ? (
          <div className="archive-grid">
            {displayArchives.map(a => (
              <div
                key={a.id}
                className={`archive-card ${selectMode && selectedIds.has(a.id) ? 'archive-card-selected' : ''}`}
                onClick={(e) => {
                  if (selectMode) { handleToggleSelect(e, a.id); return; }
                  navigate(`/reader/${a.id}`);
                }}
                tabIndex={0}
                role="button"
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    if (selectMode) { handleToggleSelect(e, a.id); return; }
                    navigate(`/reader/${a.id}`);
                  }
                }}
              >
                <div className="archive-card-cover">
                  <LazyImage src={a.cover_url} alt={a.title} />
                  {selectMode && (
                    <div className={`archive-select-check ${selectedIds.has(a.id) ? 'checked' : ''}`}>
                      {selectedIds.has(a.id) ? '✓' : ''}
                    </div>
                  )}
                  {!selectMode && (
                    <>
                      <button className="archive-tag-btn" onClick={(e) => handleOpenTagPicker(e, a.id)} title="标签">🏷️</button>
                      <button className="archive-tag-btn" onClick={(e) => handleOpenCategoryPicker(e, a.id)} title="分类">📂</button>
                      <button className="archive-rename-btn" onClick={(e) => handleOpenRename(e, a)} title="重命名">✏️</button>
                      <button className="archive-remove-btn" onClick={(e) => handleRemoveArchive(e, a.id)} title="移除">✕</button>
                    </>
                  )}
                  {a.read_page > 0 && (
                    <div className="archive-card-progress">
                      <div className="archive-card-progress-bar" style={{ width: `${(a.read_page / (a.page_count || 1)) * 100}%` }} />
                    </div>
                  )}
                </div>
                <div className="archive-card-info">
                  <div className="archive-card-title" title={a.title}>{a.title}</div>
                  <div className="archive-card-meta">
                    {a._isGroup ? (
                      <span>{a.chapter_count} 话</span>
                    ) : (
                      <span>{a.page_count} 页</span>
                    )}
                    {a.file_size > 0 && <span>· {formatSize(a.file_size)}</span>}
                  </div>
                  {a.tags && a.tags.length > 0 && (
                    <div className="archive-card-tags">
                      {a.tags.slice(0, 3).map(t => (
                        <span key={t.name} className="tag" style={{ background: t.color }}>
                          {t.namespace && <span className="tag-namespace">{t.namespace}:</span>}
                          {t.name}
                        </span>
                      ))}
                      {a.tags.length > 3 && <span className="tag" style={{ background: 'var(--text-tertiary)' }}>+{a.tags.length - 3}</span>}
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="archive-list">
            {displayArchives.map(a => (
              <div
                key={a.id}
                className={`archive-list-item ${selectMode && selectedIds.has(a.id) ? 'archive-list-item-selected' : ''}`}
                onClick={(e) => {
                  if (selectMode) { handleToggleSelect(e, a.id); return; }
                  navigate(`/reader/${a.id}`);
                }}
              >
                {selectMode && (
                  <div className={`archive-select-check-list ${selectedIds.has(a.id) ? 'checked' : ''}`}>
                    {selectedIds.has(a.id) ? '✓' : ''}
                  </div>
                )}
                <div className="archive-list-thumb">
                  <LazyImage src={a.cover_url} alt={a.title} />
                </div>
                <div className="archive-list-info">
                  <div className="archive-list-title">{a.title}</div>
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
                    <button className="archive-tag-btn-list" onClick={(e) => handleOpenTagPicker(e, a.id)} title="标签">🏷️</button>
                    <button className="archive-tag-btn-list" onClick={(e) => handleOpenCategoryPicker(e, a.id)} title="分类">📂</button>
                    <button className="archive-rename-btn-list" onClick={(e) => handleOpenRename(e, a)} title="重命名">✏️</button>
                    <button className="archive-remove-btn-list" onClick={(e) => handleRemoveArchive(e, a.id)} title="移除">✕</button>
                  </>
                )}
              </div>
            ))}
          </div>
        )}

        {/* 加载更多按钮 */}
        {hasMore && displayArchives.length > 0 && (
          <div style={{ display: 'flex', justifyContent: 'center', padding: '24px 0' }}>
            <button
              className="btn btn-secondary"
              onClick={handleLoadMore}
              disabled={loadingMore}
            >
              {loadingMore ? '加载中...' : `加载更多 (已显示 ${displayArchives.length})`}
            </button>
          </div>
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
        <div className="modal-overlay" onClick={() => setRenamingId(null)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
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
                const rel = rootDir && a.path.startsWith(rootDir)
                  ? a.path.slice(rootDir.length).replace(/^\//, '')
                  : a.path;
                const parts = rel.split('/').filter(Boolean);
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
          </div>
        </div>
      )}

      {/* 打开文件弹窗 */}
      {showOpenModal && (
        <div className="modal-overlay" onClick={() => setShowOpenModal(false)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <div className="modal-title">打开漫画文件</div>
            <div className="modal-body">
              <p style={{ color: 'var(--text-secondary)', fontSize: 13, marginBottom: 12 }}>
                输入文件或文件夹路径，支持图片文件夹和压缩包 (ZIP/CBZ/RAR/CBR/7Z)
              </p>
              <input
                className="modal-input"
                placeholder="例: /Users/me/manga 或 /Users/me/manga/comic.cbz"
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
          </div>
        </div>
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
function ArchiveActionButtons({ isCollection, isTauri, opening, loading, packingCbz, variant, onOpenFolder, onOpenArchive, onConvertCbz, onScan }) {
  const sizeClass = variant === 'mobile' ? 'btn-sm' : '';

  return (
    <>
      <button className={`btn btn-secondary ${sizeClass}`} onClick={onOpenFolder} disabled={opening}>
        📁 打开文件夹
      </button>
      <button className={`btn btn-secondary ${sizeClass}`} onClick={onOpenArchive} disabled={opening}>
        📄 打开压缩包
      </button>
      {!isCollection && isTauri && (
        <button className={`btn btn-secondary ${sizeClass}`} onClick={onConvertCbz} disabled={packingCbz}>
          {packingCbz ? '⏳ 打包中...' : '📦 转换 CBZ'}
        </button>
      )}
      {!isCollection && (
        <button className={`btn ${sizeClass}`} onClick={onScan} disabled={loading}>
          {loading ? '扫描中...' : '🔄 扫描'}
        </button>
      )}
    </>
  );
}
