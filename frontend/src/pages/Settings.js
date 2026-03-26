import React, { useState, useEffect } from 'react';
import api from '../utils/api';
import { useToast } from '../components/Toast';

export default function Settings() {
  const [settings, setSettings] = useState({});
  const [rootDir, setRootDir] = useState('');
  const [stats, setStats] = useState(null);
  const [tags, setTags] = useState([]);
  const [newTagName, setNewTagName] = useState('');
  const [newTagColor, setNewTagColor] = useState('#6366f1');
  const [categories, setCategories] = useState([]);
  const [newCatName, setNewCatName] = useState('');
  const [newCatColor, setNewCatColor] = useState('#6366f1');
  const toast = useToast();

  useEffect(() => {
    api.getSettings().then(setSettings).catch(() => {});
    api.getConfig().then(c => setRootDir(c.root_dir)).catch(() => {});
    api.getStats().then(setStats).catch(() => {});
    api.getTags().then(setTags).catch(() => {});
    api.getCategories().then(setCategories).catch(() => {});
  }, []);

  const updateSetting = async (key, value) => {
    try {
      await api.updateSettings({ [key]: value });
      setSettings(prev => ({ ...prev, [key]: value }));
      toast('已保存', 'success');
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  const handleSaveRoot = async () => {
    try {
      await api.updateConfig(rootDir);
      toast('根目录已更新', 'success');
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  const handleCreateTag = async () => {
    if (!newTagName.trim()) return;
    try {
      const tag = await api.createTag({ name: newTagName.trim(), color: newTagColor });
      setTags(prev => [...prev, { ...tag, archive_count: 0 }]);
      setNewTagName('');
      toast('标签已创建', 'success');
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  const handleDeleteTag = async (id) => {
    if (!window.confirm('确定删除此标签？')) return;
    try {
      await api.deleteTag(id);
      setTags(prev => prev.filter(t => t.id !== id));
      toast('标签已删除', 'success');
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  const handleCreateCategory = async () => {
    if (!newCatName.trim()) return;
    try {
      const cat = await api.createCategory({ name: newCatName.trim(), color: newCatColor });
      setCategories(prev => [...prev, { ...cat, archive_count: 0 }]);
      setNewCatName('');
      toast('分类已创建', 'success');
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  const handleDeleteCategory = async (id) => {
    if (!window.confirm('确定删除此分类？')) return;
    try {
      await api.deleteCategory(id);
      setCategories(prev => prev.filter(c => c.id !== id));
      toast('分类已删除', 'success');
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  return (
    <div className="settings-page">
      <h2 style={{ fontWeight: 700, marginBottom: 20 }}>设置</h2>

      {/* 目录设置 */}
      <div className="settings-section">
        <div className="settings-section-title">📂 目录</div>
        <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
          <input
            value={rootDir}
            onChange={(e) => setRootDir(e.target.value)}
            placeholder="漫画根目录路径"
            style={{ flex: 1 }}
          />
          <button className="btn btn-sm" onClick={handleSaveRoot}>保存</button>
        </div>
        <div className="settings-row-desc">支持文件夹和 ZIP/CBZ/RAR/CBR 压缩包</div>
      </div>

      {/* 阅读器设置 */}
      <div className="settings-section">
        <div className="settings-section-title">📖 阅读器</div>
        <div className="settings-row">
          <div>
            <div className="settings-row-label">翻页方向</div>
            <div className="settings-row-desc">日漫从右往左翻</div>
          </div>
          <select value={settings.page_direction || 'rtl'} onChange={(e) => updateSetting('page_direction', e.target.value)}>
            <option value="rtl">从右到左 (日漫)</option>
            <option value="ltr">从左到右</option>
          </select>
        </div>
        <div className="settings-row">
          <div>
            <div className="settings-row-label">默认适应模式</div>
          </div>
          <select value={settings.reader_fit || 'height'} onChange={(e) => updateSetting('reader_fit', e.target.value)}>
            <option value="height">适应高度</option>
            <option value="width">适应宽度</option>
            <option value="original">原始大小</option>
          </select>
        </div>
        <div className="settings-row">
          <div>
            <div className="settings-row-label">阅读器背景色</div>
          </div>
          <input type="color" value={settings.reader_bg || '#1a1a1a'} onChange={(e) => updateSetting('reader_bg', e.target.value)} style={{ width: 50, padding: 2 }} />
        </div>
      </div>

      {/* 外观设置 */}
      <div className="settings-section">
        <div className="settings-section-title">🎨 外观</div>
        <div className="settings-row">
          <div>
            <div className="settings-row-label">默认视图</div>
          </div>
          <select value={settings.view_mode || 'grid'} onChange={(e) => updateSetting('view_mode', e.target.value)}>
            <option value="grid">网格</option>
            <option value="list">列表</option>
          </select>
        </div>
        <div className="settings-row">
          <div>
            <div className="settings-row-label">默认排序</div>
          </div>
          <select value={settings.sort_by || 'updated'} onChange={(e) => updateSetting('sort_by', e.target.value)}>
            <option value="updated">最近阅读</option>
            <option value="name">名称</option>
            <option value="created">添加时间</option>
            <option value="pages">页数</option>
            <option value="size">文件大小</option>
          </select>
        </div>
        <div className="settings-row">
          <div>
            <div className="settings-row-label">排序顺序</div>
          </div>
          <select value={settings.sort_order || 'desc'} onChange={(e) => updateSetting('sort_order', e.target.value)}>
            <option value="desc">降序</option>
            <option value="asc">升序</option>
          </select>
        </div>
      </div>

      {/* 标签管理 */}
      <div className="settings-section">
        <div className="settings-section-title">🏷️ 标签管理</div>
        <div style={{ display: 'flex', gap: 8, marginBottom: 16 }}>
          <input value={newTagName} onChange={(e) => setNewTagName(e.target.value)} placeholder="新标签名称（支持 namespace:name）" style={{ flex: 1 }} onKeyDown={(e) => e.key === 'Enter' && handleCreateTag()} />
          <input type="color" value={newTagColor} onChange={(e) => setNewTagColor(e.target.value)} style={{ width: 40, padding: 2 }} />
          <button className="btn btn-sm" onClick={handleCreateTag}>添加</button>
        </div>
        {tags.length === 0 ? (
          <div style={{ color: 'var(--text-tertiary)', fontSize: 13, textAlign: 'center', padding: 20 }}>暂无标签</div>
        ) : (
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
            {tags.map(t => (
              <div key={t.id} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 10px', background: 'var(--bg-primary)', borderRadius: 20, fontSize: 13 }}>
                <span style={{ width: 10, height: 10, borderRadius: '50%', background: t.color, flexShrink: 0 }} />
                <span>{t.full_name || t.name}</span>
                <span style={{ color: 'var(--text-tertiary)', fontSize: 11 }}>({t.archive_count})</span>
                <button onClick={() => handleDeleteTag(t.id)} style={{ background: 'none', border: 'none', color: 'var(--text-tertiary)', cursor: 'pointer', padding: 0, fontSize: 14 }}>×</button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* 分类管理 */}
      <div className="settings-section">
        <div className="settings-section-title">📂 分类管理</div>
        <div style={{ display: 'flex', gap: 8, marginBottom: 16 }}>
          <input value={newCatName} onChange={(e) => setNewCatName(e.target.value)} placeholder="新分类名称" style={{ flex: 1 }} onKeyDown={(e) => e.key === 'Enter' && handleCreateCategory()} />
          <input type="color" value={newCatColor} onChange={(e) => setNewCatColor(e.target.value)} style={{ width: 40, padding: 2 }} />
          <button className="btn btn-sm" onClick={handleCreateCategory}>添加</button>
        </div>
        {categories.length === 0 ? (
          <div style={{ color: 'var(--text-tertiary)', fontSize: 13, textAlign: 'center', padding: 20 }}>暂无分类</div>
        ) : (
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
            {categories.map(c => (
              <div key={c.id} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 10px', background: 'var(--bg-primary)', borderRadius: 20, fontSize: 13 }}>
                <span style={{ width: 10, height: 10, borderRadius: '50%', background: c.color, flexShrink: 0 }} />
                <span>{c.name}</span>
                <span style={{ color: 'var(--text-tertiary)', fontSize: 11 }}>({c.archive_count})</span>
                <button onClick={() => handleDeleteCategory(c.id)} style={{ background: 'none', border: 'none', color: 'var(--text-tertiary)', cursor: 'pointer', padding: 0, fontSize: 14 }}>×</button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* 统计信息 */}
      {stats && (
        <div className="settings-section">
          <div className="settings-section-title">📊 统计</div>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))', gap: 12 }}>
            <StatCard label="漫画总数" value={stats.archives} icon="📚" />
            <StatCard label="总页数" value={stats.total_pages.toLocaleString()} icon="📄" />
            <StatCard label="标签数" value={stats.tags} icon="🏷️" />
            <StatCard label="分类数" value={stats.categories} icon="📂" />
            <StatCard label="阅读记录" value={stats.history} icon="📖" />
            <StatCard label="总大小" value={formatSize(stats.total_size)} icon="💾" />
          </div>
        </div>
      )}
    </div>
  );
}

function StatCard({ label, value, icon }) {
  return (
    <div style={{ background: 'var(--bg-primary)', borderRadius: 'var(--radius-sm)', padding: 16, textAlign: 'center' }}>
      <div style={{ fontSize: 24, marginBottom: 4 }}>{icon}</div>
      <div style={{ fontSize: 20, fontWeight: 700 }}>{value}</div>
      <div style={{ fontSize: 12, color: 'var(--text-secondary)', marginTop: 2 }}>{label}</div>
    </div>
  );
}

function formatSize(bytes) {
  if (!bytes) return '0 B';
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
}
