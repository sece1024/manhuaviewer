import React, { useState, useEffect, useMemo, useRef, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import api from '../utils/api';
import { formatSize } from '../utils/format';
import { useToast } from '../components/Toast';
import LazyImage from '../components/LazyImage';

// 检测是否在 Tauri 环境中
const isTauri = window.__TAURI__ !== undefined;

export default function Library() {
  const [archives, setArchives] = useState([]);
  const [rootDir, setRootDir] = useState('');
  const [editingRoot, setEditingRoot] = useState(false);
  const [tempRoot, setTempRoot] = useState('');
  const [search, setSearch] = useState('');
  const [loading, setLoading] = useState(false);
  const [viewMode, setViewMode] = useState(() => localStorage.getItem('viewMode') || 'grid');
  const [sortBy, setSortBy] = useState('updated');
  const [tags, setTags] = useState([]);
  const [selectedTag, setSelectedTag] = useState('');
  const [showSidebar, setShowSidebar] = useState(true);
  const [showOpenModal, setShowOpenModal] = useState(false);
  const [openPath, setOpenPath] = useState('');
  const [opening, setOpening] = useState(false);
  const [packingCbz, setPackingCbz] = useState(false);
  const searchDebounceRef = useRef(null);
  const sortByRef = useRef(sortBy);
  const navigate = useNavigate();
  const toast = useToast();

  // 保持 sortByRef 同步
  useEffect(() => { sortByRef.current = sortBy; }, [sortBy]);

  useEffect(() => {
    api.getConfig().then(c => setRootDir(c.root_dir));
    loadArchives();
    api.getTags().then(setTags).catch(() => {});
    return () => clearTimeout(searchDebounceRef.current);
  }, []);

  const loadArchives = async (params = {}) => {
    try {
      const data = await api.getArchives({ sort_by: sortByRef.current, ...params });
      setArchives(data);
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  useEffect(() => {
    loadArchives({ search, tag: selectedTag });
  }, [sortBy, selectedTag]);

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
      loadArchives({ search: val, tag: selectedTag });
    }, 300);
  }, [selectedTag]);

  const handleViewMode = (mode) => {
    setViewMode(mode);
    localStorage.setItem('viewMode', mode);
  };

  const handleTagFilter = (tagName) => {
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

  // Welcome screen
  if (!rootDir && !editingRoot) {
    return (
      <div style={{ maxWidth: 500, margin: '80px auto', textAlign: 'center', padding: '0 16px' }}>
        <div style={{ fontSize: 64, marginBottom: 16 }}>📚</div>
        <h2 style={{ marginBottom: 12, fontWeight: 700 }}>欢迎使用 MangaViewer</h2>
        <p style={{ color: 'var(--text-secondary)', marginBottom: 24, fontSize: 14 }}>
          请先配置漫画存放的根目录<br />
          <span style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>
            支持文件夹、ZIP/CBZ、RAR/CBR、7Z 压缩包
          </span>
        </p>
        <div style={{ display: 'flex', gap: 8, justifyContent: 'center', flexWrap: 'wrap' }}>
          <input
            placeholder="例: /home/user/manga"
            value={tempRoot}
            onChange={(e) => setTempRoot(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSaveRoot()}
            style={{ flex: 1, minWidth: 0, maxWidth: 360 }}
            autoFocus
          />
          <button className="btn" onClick={handleSaveRoot}>确认</button>
        </div>
      </div>
    );
  }

  return (
    <div className="library-layout">
      {/* 侧边栏过滤器 */}
      {showSidebar && (
        <div className="library-sidebar">
          {/* 目录配置 */}
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

          {/* 标签过滤 */}
          {Object.entries(tagsByNamespace).map(([ns, nsTags]) => (
            <div className="filter-section" key={ns}>
              <div className="filter-section-title">
                {ns === NS_OTHER ? '标签' : ns}
              </div>
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
          ))}
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

          <select value={sortBy} onChange={(e) => setSortBy(e.target.value)} style={{ minWidth: 100 }}>
            <option value="updated">最近阅读</option>
            <option value="name">名称</option>
            <option value="created">添加时间</option>
            <option value="pages">页数</option>
            <option value="size">大小</option>
          </select>

          <div className="toggle-group">
            <button className={viewMode === 'grid' ? 'active' : ''} onClick={() => handleViewMode('grid')} title="网格">▦</button>
            <button className={viewMode === 'list' ? 'active' : ''} onClick={() => handleViewMode('list')} title="列表">☰</button>
          </div>

          <button className="btn btn-secondary" onClick={() => setShowSidebar(v => !v)} title="过滤器">
            {showSidebar ? '◁' : '▷'}
          </button>

          <button className="btn btn-secondary" onClick={() => setShowOpenModal(true)}>
            📂 打开文件
          </button>

          {isTauri && (
            <button className="btn btn-secondary" onClick={handleConvertFolderToCbz} disabled={packingCbz}>
              📦 转换 CBZ
            </button>
          )}

          <button className="btn" onClick={handleScan} disabled={loading}>
            {loading ? '扫描中...' : '🔄 扫描'}
          </button>
        </div>

        {/* 档案列表 */}
        {archives.length === 0 ? (
          <div className="empty-state">
            <div className="empty-state-icon">📚</div>
            <div className="empty-state-text">
              {search || selectedTag ? '没有匹配的漫画' : '暂无漫画，点击「扫描」按钮'}
            </div>
          </div>
        ) : viewMode === 'grid' ? (
          <div className="archive-grid">
            {archives.map(a => (
              <div key={a.id} className="archive-card" onClick={() => navigate(`/reader/${a.id}`)} tabIndex={0} role="button" onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); navigate(`/reader/${a.id}`); } }}>
                <div className="archive-card-cover">
                  <LazyImage src={a.cover_url} alt={a.title} />
                  <div className="archive-card-type">{a.archive_type === 'folder' ? '📁' : '📦'}</div>
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
            {archives.map(a => (
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
              </div>
            ))}
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
    </div>
  );
}

// 命名空间标签默认分组 key
const NS_OTHER = '_other';
