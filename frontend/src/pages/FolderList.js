import React, { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import api from '../utils/api';
import { useToast } from '../components/Toast';

export default function FolderList() {
  const [folders, setFolders] = useState([]);
  const [rootDir, setRootDir] = useState('');
  const [editingRoot, setEditingRoot] = useState(false);
  const [tempRoot, setTempRoot] = useState('');
  const [search, setSearch] = useState('');
  const [loading, setLoading] = useState(false);
  const navigate = useNavigate();
  const toast = useToast();

  useEffect(() => {
    api.getConfig().then((c) => setRootDir(c.root_dir));
    loadFolders();
  }, []);

  const loadFolders = async (params = {}) => {
    try {
      const data = await api.getFolders(params);
      setFolders(data);
    } catch (e) {
      toast(e.message, 'error');
    }
  };

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
      await loadFolders({ search });
    } catch (e) {
      toast(e.message, 'error');
    }
    setLoading(false);
  };

  const handleSearch = (val) => {
    setSearch(val);
    loadFolders({ search: val });
  };

  if (!rootDir && !editingRoot) {
    return (
      <div style={{ maxWidth: 600, margin: '80px auto', textAlign: 'center' }}>
        <h2 style={{ marginBottom: 16 }}>欢迎使用 MangaViewer</h2>
        <p style={{ color: 'var(--text-secondary)', marginBottom: 24 }}>
          请先配置漫画存放的根目录（包含多个漫画子文件夹的目录）
        </p>
        <div style={{ display: 'flex', gap: 8, justifyContent: 'center' }}>
          <input
            placeholder="例: /home/user/manga"
            value={tempRoot}
            onChange={(e) => setTempRoot(e.target.value)}
            style={{ width: 400 }}
          />
          <button className="btn" onClick={handleSaveRoot}>确认</button>
        </div>
      </div>
    );
  }

  return (
    <div>
      {/* 顶栏 */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 24, flexWrap: 'wrap' }}>
        {editingRoot ? (
          <>
            <input value={tempRoot} onChange={(e) => setTempRoot(e.target.value)} style={{ flex: 1, minWidth: 200 }} />
            <button className="btn" onClick={handleSaveRoot}>保存</button>
            <button className="btn btn-secondary" onClick={() => setEditingRoot(false)}>取消</button>
          </>
        ) : (
          <>
            <span style={{ fontSize: 14, color: 'var(--text-secondary)' }}>📂 {rootDir}</span>
            <button className="btn btn-secondary" onClick={() => { setTempRoot(rootDir); setEditingRoot(true); }}>修改</button>
          </>
        )}
        <button className="btn" onClick={handleScan} disabled={loading}>
          {loading ? '扫描中...' : '🔄 扫描'}
        </button>
        <input
          placeholder="🔍 搜索..."
          value={search}
          onChange={(e) => handleSearch(e.target.value)}
          style={{ width: 200, marginLeft: 'auto' }}
        />
      </div>

      {/* 文件夹网格 */}
      {folders.length === 0 ? (
        <div style={{ textAlign: 'center', padding: 60, color: 'var(--text-secondary)' }}>
          暂无漫画，点击「扫描」按钮扫描根目录
        </div>
      ) : (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(260px, 1fr))', gap: 16 }}>
          {folders.map((f) => (
            <div
              key={f.id}
              className="card"
              style={{ cursor: 'pointer', transition: 'box-shadow 0.15s' }}
              onClick={() => navigate(`/reader/${f.id}`)}
              onMouseEnter={(e) => e.currentTarget.style.boxShadow = '0 4px 12px rgba(0,0,0,0.1)'}
              onMouseLeave={(e) => e.currentTarget.style.boxShadow = 'none'}
            >
              <div style={{ fontWeight: 600, marginBottom: 8 }}>{f.name}</div>
              <div style={{ fontSize: 13, color: 'var(--text-secondary)' }}>
                {f.image_count} 张图片
                {f.page_index != null && f.page_index > 0 && (
                  <span> · 已读到第 {f.page_index + 1}/{f.total_pages} 页</span>
                )}
              </div>
              {f.tags && f.tags.length > 0 && (
                <div style={{ marginTop: 8 }}>
                  {f.tags.map((t) => (
                    <span key={t.name} className="tag" style={{ background: t.color }}>{t.name}</span>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
