import React, { useState, useEffect, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import api from '../utils/api';
import { useToast } from '../components/Toast';

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
  const navigate = useNavigate();
  const toast = useToast();

  useEffect(() => {
    api.getConfig().then(c => setRootDir(c.root_dir));
    loadArchives();
    api.getTags().then(setTags).catch(() => {});
  }, []);

  const loadArchives = async (params = {}) => {
    try {
      const data = await api.getArchives({ sort_by: sortBy, ...params });
      setArchives(data);
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  useEffect(() => {
    loadArchives({ search, tag: selectedTag });
  }, [sortBy, selectedTag]); // eslint-disable-line

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

  const handleSearch = (val) => {
    setSearch(val);
    loadArchives({ search: val, tag: selectedTag });
  };

  const handleViewMode = (mode) => {
    setViewMode(mode);
    localStorage.setItem('viewMode', mode);
  };

  const handleTagFilter = (tagName) => {
    const next = selectedTag === tagName ? '' : tagName;
    setSelectedTag(next);
  };

  // 按命名空间分组标签
  const tagsByNamespace = useMemo(() => {
    const map = {};
    for (const t of tags) {
      const ns = t.namespace || '_other';
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
            支持文件夹、ZIP/CBZ、RAR/CBR 压缩包
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
                {ns === '_other' ? '标签' : ns}
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
            placeholder="搜索漫画..."
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
              <div key={a.id} className="archive-card" onClick={() => navigate(`/reader/${a.id}`)}>
                <div className="archive-card-cover">
                  <CoverImage src={a.cover_url} alt={a.title} />
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
                  <CoverImage src={a.cover_url} alt={a.title} />
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
    </div>
  );
}

// 封面图组件（带 fallback）
function CoverImage({ src, alt }) {
  const [error, setError] = useState(false);
  if (error || !src) {
    return <div className="no-cover">📖</div>;
  }
  return <img src={src} alt={alt} onError={() => setError(true)} loading="lazy" />;
}

// 格式化文件大小
function formatSize(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
}
