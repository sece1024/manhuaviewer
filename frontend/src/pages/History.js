import React, { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import api from '../utils/api';
import { useToast } from '../components/Toast';
import LazyImage from '../components/LazyImage';
import { formatRelativeTime } from '../utils/format';

export default function History() {
  const [history, setHistory] = useState([]);
  const [search, setSearch] = useState('');
  const navigate = useNavigate();
  const toast = useToast();

  useEffect(() => { loadHistory(); }, []);

  const loadHistory = async () => {
    try {
      const data = await api.getHistory();
      setHistory(data);
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  const handleDelete = async (archiveId) => {
    if (!window.confirm('确定删除该记录？')) return;
    try {
      await api.deleteHistory(archiveId);
      loadHistory();
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  const handleClearAll = async () => {
    if (!window.confirm('确定清空所有阅读记录？')) return;
    try {
      await api.clearHistory();
      setHistory([]);
      toast('已清空所有记录', 'success');
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  const filtered = history.filter(h => {
    if (!search) return true;
    const s = search.toLowerCase();
    return h.title.toLowerCase().includes(s) || h.tags.some(t => t.name.toLowerCase().includes(s));
  });

  return (
    <div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 20, flexWrap: 'wrap' }}>
        <h2 style={{ fontWeight: 700 }}>阅读历史</h2>
        <div style={{ flex: 1 }} />
        <input
          className="search-input"
          placeholder="搜索文件夹名称或标签..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          style={{ maxWidth: 320 }}
        />
        {history.length > 0 && (
          <button className="btn btn-danger btn-sm" onClick={handleClearAll}>清空全部</button>
        )}
      </div>

      {filtered.length === 0 ? (
        <div className="empty-state">
          <div className="empty-state-icon">📖</div>
          <div className="empty-state-text">{search ? '没有匹配的记录' : '暂无阅读记录'}</div>
        </div>
      ) : (
        <div className="history-list">
          {filtered.map(h => (
            <div key={h.archive_id} className="history-item" onClick={() => navigate(`/reader/${h.archive_id}`)}>
              <div className="history-thumb">
                <LazyImage src={h.cover_url} alt={h.title} style={{ width: '100%', height: '100%' }} />
              </div>
              <div className="history-info">
                <div className="history-title">{h.title}</div>
                <div className="history-meta">
                  第 {h.page_index + 1}/{h.total_pages || h.page_count} 页 · {h.page_count} 张图片
                  {h.archive_type !== 'folder' && ` · ${h.archive_type.toUpperCase()}`}
                </div>
                {h.tags.length > 0 && (
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4, marginTop: 4 }}>
                    {h.tags.map(t => (
                      <span key={t.name} className="tag" style={{ background: t.color }}>
                        {t.namespace && <span className="tag-namespace">{t.namespace}:</span>}
                        {t.name}
                      </span>
                    ))}
                  </div>
                )}
              </div>
              <div className="history-date">
                {h.updated_at ? formatRelativeTime(h.updated_at) : ''}
              </div>
              <button
                className="btn btn-danger btn-sm"
                onClick={(e) => { e.stopPropagation(); handleDelete(h.archive_id); }}
              >
                删除
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
