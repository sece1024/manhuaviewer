import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import api from '../utils/api';
import { useToast } from '../components/Toast';
import LazyImage from '../components/LazyImage';
import { formatRelativeTime } from '../utils/format';
import ConfirmDialog from '../components/ConfirmDialog';

const PAGE_SIZE = 50;

export default function History() {
  const [history, setHistory] = useState([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(true);
  const [search, setSearch] = useState('');
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [confirmTarget, setConfirmTarget] = useState(null);
  const navigate = useNavigate();
  const toast = useToast();
  const requestIdRef = useRef(0);
  const searchDebounceRef = useRef(null);

  const loadHistory = useCallback(async (searchValue, append = false) => {
    const id = ++requestIdRef.current;
    if (append) setLoadingMore(true); else setLoading(true);
    try {
      const nextPage = append ? page + 1 : 1;
      const res = await api.getHistory({ search: searchValue, limit: PAGE_SIZE, page: nextPage });
      if (id !== requestIdRef.current) return;
      setHistory(prev => append ? [...prev, ...res.items] : res.items);
      setTotal(res.total);
      setPage(nextPage);
      setHasMore(res.items.length >= PAGE_SIZE);
    } catch (e) {
      if (id === requestIdRef.current) toast(e.message, 'error');
    } finally {
      if (id === requestIdRef.current) {
        if (append) setLoadingMore(false); else setLoading(false);
      }
    }
    // eslint-disable-next-line
  }, [page]);

  // eslint-disable-next-line
  useEffect(() => { loadHistory(''); }, []);

  const handleSearchChange = (value) => {
    setSearch(value);
    if (searchDebounceRef.current) clearTimeout(searchDebounceRef.current);
    searchDebounceRef.current = setTimeout(() => loadHistory(value), 300);
  };

  const handleLoadMore = () => loadHistory(search, true);

  const handleDelete = useCallback((archiveId) => {
    setConfirmTarget({ type: 'single', id: archiveId });
    setConfirmOpen(true);
  }, []);

  const handleClearAll = useCallback(() => {
    setConfirmTarget({ type: 'all' });
    setConfirmOpen(true);
  }, []);

  const handleConfirm = async () => {
    setConfirmOpen(false);
    try {
      if (confirmTarget.type === 'single') {
        await api.deleteHistory(confirmTarget.id);
        loadHistory(search);
      } else {
        await api.clearHistory();
        setHistory([]);
        setTotal(0);
        setHasMore(false);
        toast('已清空所有记录', 'success');
      }
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  return (
    <div>
      <ConfirmDialog
        open={confirmOpen}
        title={confirmTarget?.type === 'all' ? '清空阅读记录' : '删除记录'}
        message={confirmTarget?.type === 'all' ? '确定清空所有阅读记录？此操作不可撤销。' : '确定删除该阅读记录？'}
        danger
        confirmText="确定"
        onConfirm={handleConfirm}
        onCancel={() => setConfirmOpen(false)}
      />

      <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 20, flexWrap: 'wrap' }}>
        <h2 style={{ fontWeight: 700 }}>阅读历史</h2>
        <div style={{ flex: 1 }} />
        <input
          className="search-input"
          placeholder="搜索文件夹名称或标签..."
          value={search}
          onChange={(e) => handleSearchChange(e.target.value)}
          style={{ maxWidth: 320 }}
        />
        {total > 0 && (
          <button className="btn btn-danger btn-sm" onClick={handleClearAll}>清空全部</button>
        )}
      </div>

      {loading ? (
        <div className="empty-state">
          <div className="empty-state-icon">⏳</div>
          <div className="empty-state-text">加载中...</div>
        </div>
      ) : history.length === 0 ? (
        <div className="empty-state">
          <div className="empty-state-icon">📖</div>
          <div className="empty-state-text">{search ? '没有匹配的记录' : '暂无阅读记录'}</div>
        </div>
      ) : (
        <>
          <div className="history-list">
            {history.map(h => (
              <div key={h.archive_id} className="history-item" onClick={() => navigate(`/reader/${h.archive_id}`)} tabIndex={0} role="button" onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); navigate(`/reader/${h.archive_id}`); } }}>
                <div className="history-thumb">
                  <LazyImage src={h.cover_url} alt={h.title} style={{ width: '100%', height: '100%' }} />
                </div>
                <div className="history-info">
                  <div className="history-title">{h.title}</div>
                  <div className="history-meta">
                    第 {h.page_index + 1}/{h.total_pages || h.page_count} 页 · {h.page_count} 张图片
                    {h.archive_type !== 'folder' && ` · ${h.archive_type.toUpperCase()}`}
                  </div>
                  {(h.tags || []).length > 0 && (
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

          {hasMore && (
            <div style={{ display: 'flex', justifyContent: 'center', padding: '24px 0' }}>
              <button
                className="btn btn-secondary"
                onClick={handleLoadMore}
                disabled={loadingMore}
              >
                {loadingMore ? '加载中...' : `加载更多 (已显示 ${history.length}/${total})`}
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}

