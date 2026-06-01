import React, { useState, useEffect, useMemo, useRef, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import api from '../utils/api';
import { formatSize } from '../utils/format';
import { useToast } from '../components/Toast';
import useSettings from '../hooks/useSettings';
import LazyImage from '../components/LazyImage';
import TagPicker from '../components/TagPicker';

// 检测是否在 Tauri 环境中
const isTauri = window.__TAURI__ !== undefined;

export default function Library({ mode = 'library' }) {
  const { settings, updateSetting } = useSettings();
  const [archives, setArchives] = useState([]);
  const [rootDir, setRootDir] = useState('');
  const [editingRoot, setEditingRoot] = useState(false);
  const [tempRoot, setTempRoot] = useState('');
  const [search, setSearch] = useState('');
  const [loading, setLoading] = useState(false);
  const [viewMode, setViewMode] = useState(() => settings.view_mode || 'grid');
  const [sortBy, setSortBy] = useState(() => settings.sort_by || 'updated');
  const [tags, setTags] = useState([]);
  const [selectedTag, setSelectedTag] = useState('');
  const [showSidebar, setShowSidebar] = useState(true);
  const [showOpenModal, setShowOpenModal] = useState(false);
  const [openPath, setOpenPath] = useState('');
  const [opening, setOpening] = useState(false);
  const [packingCbz, setPackingCbz] = useState(false);
  const [showMobileMenu, setShowMobileMenu] = useState(false);
  // 窄屏：把次要操作收进 ⋯ 菜单
  const [isNarrow, setIsNarrow] = useState(() => typeof window !== 'undefined' && window.innerWidth < 768);
  // 分页状态
  const PAGE_SIZE = 50;
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const searchDebounceRef = useRef(null);
  const sortByRef = useRef(sortBy);
  const selectedTagRef = useRef(selectedTag);
  const requestIdRef = useRef(0);
  const navigate = useNavigate();
  const toast = useToast();

  // 保持 refs 同步
  useEffect(() => { sortByRef.current = sortBy; }, [sortBy]);
  useEffect(() => { selectedTagRef.current = selectedTag; }, [selectedTag]);

  useEffect(() => {
    api.getConfig().then(c => setRootDir(c.root_dir));
    loadArchives();
    api.getTags().then(setTags).catch(() => {});
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
      const data = await api.getArchives({
        sort_by: sortByRef.current,
        limit: PAGE_SIZE,
        page: nextPage,
        ...params,
      });
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
    loadArchives({ search, tag: selectedTag });
  }, [sortBy, selectedTag, search]);

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
    loadArchives({ search, tag: selectedTag }, true);
  }, [search, selectedTag]);

  const handleViewMode = (mode) => {
    setViewMode(mode);
    updateSetting('view_mode', mode);
  };

  const handleTagFilter = (tagName) => {
    clearTimeout(searchDebounceRef.current);
    const next = selectedTag === tagName ? '' : tagName;
    setSelectedTag(next);
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

  const handleSelectFolder = async () => {
    if (!isTauri) {
      toast('文件夹选择仅在桌面应用中可用', 'warning');
      return;
    }
    try {
      const selected = await window.__TAURI__.dialog.open({
        directory: true,
        multiple: false,
        title: '选择漫画文件夹',
      });
      if (selected) {
        setOpenPath(selected);
      }
    } catch (e) {
      toast('选择文件夹失败: ' + e.message, 'error');
    }
  };

  const handleSelectFile = async () => {
    if (!isTauri) {
      toast('文件选择仅在桌面应用中可用', 'warning');
      return;
    }
    try {
      const selected = await window.__TAURI__.dialog.open({
        multiple: false,
        title: '选择漫画文件',
        filters: [{
          name: '漫画文件',
          extensions: ['zip', 'cbz', 'rar', 'cbr', '7z']
        }]
      });
      if (selected) {
        setOpenPath(selected);
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

  const handleRemoveArchive = async (e, id) => {
    e.stopPropagation();
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
      api.getTags().then(setTags).catch(() => {});
    }
  };

  // 将档案按类型分成收藏（压缩包）和文件夹两组
  const compressedArchives = useMemo(() => archives.filter(a => a.archive_type !== 'folder'), [archives]);
  const folderArchives = useMemo(() => archives.filter(a => a.archive_type === 'folder'), [archives]);

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

  // 欢迎屏幕上直接选择文件夹打开
  const handleWelcomeSelectFolder = async () => {
    if (!isTauri) {
      toast('文件夹选择仅在桌面应用中可用', 'warning');
      return;
    }
    try {
      const selected = await window.__TAURI__.dialog.open({
        directory: true,
        multiple: false,
        title: '选择漫画文件夹',
      });
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
      toast('选择文件夹失败: ' + e.message, 'error');
    }
  };

  // 欢迎屏幕上直接选择压缩包打开
  const handleWelcomeSelectFile = async () => {
    if (!isTauri) {
      toast('文件选择仅在桌面应用中可用', 'warning');
      return;
    }
    try {
      const selected = await window.__TAURI__.dialog.open({
        multiple: false,
        title: '选择漫画文件',
        filters: [{
          name: '漫画文件',
          extensions: ['zip', 'cbz', 'rar', 'cbr', '7z']
        }]
      });
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

  // Welcome screen — 仅漫画库模式下，无漫画且未配置根目录时显示
  if (!isCollection && !rootDir && !editingRoot && archives.length === 0) {
    return (
      <div style={{ maxWidth: 500, margin: '80px auto', textAlign: 'center', padding: '0 16px' }}>
        <div style={{ fontSize: 64, marginBottom: 16 }}>📚</div>
        <h2 style={{ marginBottom: 12, fontWeight: 700 }}>欢迎使用 MangaViewer</h2>
        <p style={{ color: 'var(--text-secondary)', marginBottom: 24, fontSize: 14 }}>
          打开漫画文件夹或压缩包即可开始阅读<br />
          <span style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>
            支持文件夹、ZIP/CBZ、RAR/CBR、7Z 压缩包
          </span>
        </p>

        {/* 直接打开文件 */}
        {isTauri && (
          <div style={{ display: 'flex', gap: 8, justifyContent: 'center', marginBottom: 24 }}>
            <button className="btn" onClick={handleWelcomeSelectFolder} disabled={opening}>
              📁 打开文件夹
            </button>
            <button className="btn" onClick={handleWelcomeSelectFile} disabled={opening}>
              📄 打开压缩包
            </button>
          </div>
        )}

        {/* 配置根目录（折叠） */}
        <div style={{ borderTop: '1px solid var(--border)', paddingTop: 16, marginTop: 8 }}>
          <p style={{ color: 'var(--text-tertiary)', fontSize: 12, marginBottom: 8 }}>
            也可以配置漫画根目录，批量扫描导入
          </p>
          <div style={{ display: 'flex', gap: 8, justifyContent: 'center', flexWrap: 'wrap' }}>
            <input
              placeholder="例: /home/user/manga"
              value={tempRoot}
              onChange={(e) => setTempRoot(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSaveRoot()}
              style={{ flex: 1, minWidth: 0, maxWidth: 360 }}
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

          {!isNarrow && (
            <>
              <button className="btn btn-secondary" onClick={() => setShowOpenModal(true)}>
                📂 打开文件
              </button>

              {!isCollection && isTauri && (
                <button className="btn btn-secondary" onClick={handleConvertFolderToCbz} disabled={packingCbz}>
                  📦 转换 CBZ
                </button>
              )}

              {!isCollection && (
                <button className="btn" onClick={handleScan} disabled={loading}>
                  {loading ? '扫描中...' : '🔄 扫描'}
                </button>
              )}
            </>
          )}

          {isNarrow && (
            <button
              className="btn btn-secondary btn-icon"
              onClick={() => setShowMobileMenu(v => !v)}
              title="更多操作"
              aria-label="打开更多操作菜单"
              aria-expanded={showMobileMenu}
            >⋯</button>
          )}
        </div>

        {/* 窄屏：折叠次要操作 */}
        {isNarrow && showMobileMenu && (
          <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', padding: '8px 0', borderBottom: '1px solid var(--border)', marginBottom: 8 }}>
            <button className="btn btn-secondary btn-sm" onClick={() => { setShowOpenModal(true); setShowMobileMenu(false); }}>📂 打开文件</button>
            {!isCollection && isTauri && (
              <button className="btn btn-secondary btn-sm" onClick={() => { handleConvertFolderToCbz(); setShowMobileMenu(false); }} disabled={packingCbz}>
                {packingCbz ? '⏳ 打包中...' : '📦 转换 CBZ'}
              </button>
            )}
            {!isCollection && (
              <button className="btn btn-sm" onClick={() => { handleScan(); setShowMobileMenu(false); }} disabled={loading}>
                {loading ? '⏳ 扫描中...' : '🔄 扫描'}
              </button>
            )}
          </div>
        )}

        {/* 档案列表 */}
        {displayArchives.length === 0 ? (
          <div className="empty-state">
            <div className="empty-state-icon">{isCollection ? '📦' : '📚'}</div>
            <div className="empty-state-text">
              {search || selectedTag ? '没有匹配的漫画' : isCollection ? '暂无收藏' : rootDir ? '暂无漫画，点击「扫描」按钮' : '点击「打开文件」添加漫画'}
            </div>
          </div>
        ) : viewMode === 'grid' ? (
          <div className="archive-grid">
            {displayArchives.map(a => (
              <div key={a.id} className="archive-card" onClick={() => navigate(`/reader/${a.id}`)} tabIndex={0} role="button" onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); navigate(`/reader/${a.id}`); } }}>
                <div className="archive-card-cover">
                  <LazyImage src={a.cover_url} alt={a.title} />
                  <button className="archive-tag-btn" onClick={(e) => handleOpenTagPicker(e, a.id)} title="标签">🏷️</button>
                  <button className="archive-remove-btn" onClick={(e) => handleRemoveArchive(e, a.id)} title="移除">✕</button>
                  {a.read_page > 0 && (
                    <div className="archive-card-progress">
                      <div className="archive-card-progress-bar" style={{ width: `${(a.read_page / (a.page_count || 1)) * 100}%` }} />
                    </div>
                  )}
                </div>
                <div className="archive-card-info">
                  <div className="archive-card-title" title={a.title}>{a.title}</div>
                  <div className="archive-card-meta">
                    <span>{a.page_count} 页</span>
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
              <div key={a.id} className="archive-list-item" onClick={() => navigate(`/reader/${a.id}`)}>
                <div className="archive-list-thumb">
                  <LazyImage src={a.cover_url} alt={a.title} />
                </div>
                <div className="archive-list-info">
                  <div className="archive-list-title">{a.title}</div>
                  <div className="archive-list-meta">
                    {a.page_count} 页 · {a.archive_type === 'folder' ? '文件夹' : '压缩包'}
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
                <button className="archive-tag-btn-list" onClick={(e) => handleOpenTagPicker(e, a.id)} title="标签">🏷️</button>
                <button className="archive-remove-btn-list" onClick={(e) => handleRemoveArchive(e, a.id)} title="移除">✕</button>
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
      {/* 打开文件弹窗 */}
      {showOpenModal && (
        <div className="modal-overlay" onClick={() => setShowOpenModal(false)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <div className="modal-title">打开漫画文件</div>
            <div className="modal-body">
              <p style={{ color: 'var(--text-secondary)', fontSize: 13, marginBottom: 12 }}>
                输入文件或文件夹的绝对路径，支持图片文件夹和压缩包 (ZIP/CBZ/RAR/CBR/7Z)
              </p>
              <div style={{ display: 'flex', gap: 8, marginBottom: 8 }}>
                <input
                  className="modal-input"
                  style={{ flex: 1 }}
                  placeholder="例: /Users/me/manga/comic.cbz"
                  value={openPath}
                  onChange={(e) => setOpenPath(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleOpenFile()}
                  autoFocus
                />
              </div>
              {isTauri && (
                <div style={{ display: 'flex', gap: 8 }}>
                  <button
                    className="btn btn-secondary btn-sm"
                    onClick={handleSelectFolder}
                    style={{ flex: 1 }}
                  >
                    📁 选择文件夹
                  </button>
                  <button
                    className="btn btn-secondary btn-sm"
                    onClick={handleSelectFile}
                    style={{ flex: 1 }}
                  >
                    📄 选择压缩包
                  </button>
                </div>
              )}
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
    </div>
  );
}

// 命名空间标签默认分组 key
const NS_OTHER = '_other';
