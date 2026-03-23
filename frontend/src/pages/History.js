import React, { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import api from '../utils/api';
import { useToast } from '../components/Toast';

export default function History() {
  const [history, setHistory] = useState([]);
  const [search, setSearch] = useState('');
  const navigate = useNavigate();
  const toast = useToast();

  useEffect(() => {
    loadHistory();
  }, []);

  const loadHistory = async () => {
    try {
      const data = await api.getHistory();
      setHistory(data);
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  const handleDelete = async (folderId) => {
    if (!window.confirm('确定删除该记录？')) return;
    try {
      await api.deleteHistory(folderId);
      loadHistory();
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  const filtered = history.filter((h) => {
    if (!search) return true;
    const s = search.toLowerCase();
    return h.name.toLowerCase().includes(s) || h.tags.some((t) => t.name.toLowerCase().includes(s));
  });

  return (
    <div>
      <h2 style={{ marginBottom: 16 }}>阅读历史</h2>
      <div style={{ marginBottom: 16 }}>
        <input
          placeholder="🔍 搜索文件夹名称或标签..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          style={{ width: 400 }}
        />
      </div>

      {filtered.length === 0 ? (
        <div style={{ textAlign: 'center', padding: 60, color: 'var(--text-secondary)' }}>暂无阅读记录</div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          {filtered.map((h) => (
            <div
              key={h.folder_id}
              className="card"
              style={{ display: 'flex', alignItems: 'center', gap: 16, cursor: 'pointer' }}
              onClick={() => navigate(`/reader/${h.folder_id}`)}
            >
              <div style={{ flex: 1 }}>
                <div style={{ fontWeight: 600 }}>{h.name}</div>
                <div style={{ fontSize: 13, color: 'var(--text-secondary)' }}>
                  第 {h.page_index + 1}/{h.total_pages} 页 · {h.image_count} 张图片
                </div>
                {h.tags.length > 0 && (
                  <div style={{ marginTop: 4 }}>
                    {h.tags.map((t) => (
                      <span key={t.name} className="tag" style={{ background: t.color }}>{t.name}</span>
                    ))}
                  </div>
                )}
              </div>
              <div style={{ fontSize: 12, color: 'var(--text-secondary)' }}>
                {h.updated_at ? new Date(h.updated_at).toLocaleString() : ''}
              </div>
              <button
                className="btn btn-danger"
                style={{ padding: '4px 10px', fontSize: 12 }}
                onClick={(e) => { e.stopPropagation(); handleDelete(h.folder_id); }}
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
