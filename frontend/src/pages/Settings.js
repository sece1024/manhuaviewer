import React, { useState, useEffect, useCallback } from 'react';
import api from '../utils/api';
import { formatSize } from '../utils/format';
import { useToast } from '../components/Toast';
import useSettings from '../hooks/useSettings';
import useTags from '../hooks/useTags';
import ConfirmDialog from '../components/ConfirmDialog';

export default function Settings() {
  const { settings, updateSetting } = useSettings();
  const { tags, reload: reloadTags } = useTags();
  const [stats, setStats] = useState(null);
  const [newTagName, setNewTagName] = useState('');
  const [newTagColor, setNewTagColor] = useState('#6366f1');
  const [categories, setCategories] = useState([]);
  const [cbzDirInput, setCbzDirInput] = useState(settings.cbz_export_dir || '');
  const [newCatName, setNewCatName] = useState('');
  const [newCatColor, setNewCatColor] = useState('#6366f1');
  const [importing, setImporting] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [confirmTarget, setConfirmTarget] = useState(null);
  const [cbzFiles, setCbzFiles] = useState([]);
  const toast = useToast();

  // 检测 Tauri 环境
  const isTauri = window.__TAURI__ !== undefined;

  const loadCbzFiles = useCallback(() => {
    if (settings.cbz_export_dir) {
      api.listCbz().then(setCbzFiles).catch(() => setCbzFiles([]));
    }
  }, [settings.cbz_export_dir]);

  useEffect(() => {
    api.getStats().then(setStats).catch(() => {});
    reloadTags();
    api.getCategories().then(setCategories).catch(() => {});
    loadCbzFiles();
  }, [reloadTags, loadCbzFiles]);

  useEffect(() => {
    setCbzDirInput(settings.cbz_export_dir || '');
  }, [settings.cbz_export_dir]);

  const handleUpdateSetting = async (key, value) => {
    try {
      await updateSetting(key, value);
      toast('已保存', 'success');
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  // 选择 CBZ 归档目录
  const handleSelectCbzDir = async () => {
    if (!isTauri) {
      toast('目录选择仅在桌面应用中可用', 'warning');
      return;
    }
    try {
      const selected = await window.__TAURI__.dialog.open({
        directory: true,
        multiple: false,
        title: '选择 CBZ 归档目录',
      });
      if (selected) {
        await handleUpdateSetting('cbz_export_dir', selected);
      }
    } catch (e) {
      toast('选择目录失败: ' + e.message, 'error');
    }
  };

  const handleCreateTag = async () => {
    if (!newTagName.trim()) return;
    try {
      await api.createTag({ name: newTagName.trim(), color: newTagColor });
      reloadTags();
      setNewTagName('');
      toast('标签已创建', 'success');
    } catch (e) {
      toast(e.message, 'error');
    }
  };

  const handleDeleteTag = useCallback(async (id) => {
    try {
      await api.deleteTag(id);
      reloadTags();
      toast('标签已删除', 'success');
    } catch (e) {
      toast(e.message, 'error');
    }
  }, [reloadTags, toast]);

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

  const handleDeleteCategory = useCallback(async (id) => {
    try {
      await api.deleteCategory(id);
      setCategories(prev => prev.filter(c => c.id !== id));
      toast('分类已删除', 'success');
    } catch (e) {
      toast(e.message, 'error');
    }
  }, [toast]);

  const handleConfirm = async () => {
    setConfirmOpen(false);
    if (confirmTarget?.type === 'tag') {
      await handleDeleteTag(confirmTarget.id);
    } else if (confirmTarget?.type === 'category') {
      await handleDeleteCategory(confirmTarget.id);
    }
  };

  const handleExportBackup = async () => {
    if (!isTauri) {
      // 浏览器模式：直接打开下载链接
      window.open(api.exportBackup());
      return;
    }
    // Tauri 模式：用 dialog.save 选择路径，再 fetch + fs.writeTextFile 写入
    try {
      const savePath = await window.__TAURI__.dialog.save({
        title: '导出备份',
        filters: [{ name: 'JSON', extensions: ['json'] }],
        defaultPath: `manhuaviewer-backup-${new Date().toISOString().slice(0, 10)}.json`,
      });
      if (!savePath) return;
      const res = await fetch(api.exportBackup());
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error || `HTTP ${res.status}`);
      }
      const data = await res.json();
      await window.__TAURI__.fs.writeTextFile(savePath, JSON.stringify(data, null, 2));
      toast(`备份已导出: ${savePath}`, 'success');
    } catch (e) {
      toast(`导出失败: ${e.message}`, 'error');
    }
  };

  const handleImportBackup = async (e) => {
    const file = e.target.files[0];
    if (!file) return;
    setImporting(true);
    try {
      const text = await file.text();
      const data = JSON.parse(text);
      const result = await api.importBackup(data);
      toast(`恢复成功: ${result.restored.archives} 漫画, ${result.restored.tags} 标签`, 'success');
      // 重新加载数据
      api.getStats().then(setStats).catch(() => {});
      reloadTags();
      api.getCategories().then(setCategories).catch(() => {});
    } catch (err) {
      toast(`导入失败: ${err.message}`, 'error');
    }
    setImporting(false);
    e.target.value = '';
  };

  return (
    <div className="settings-page">
      <ConfirmDialog
        open={confirmOpen}
        title={confirmTarget?.type === 'tag' ? '删除标签' : '删除分类'}
        message={`确定删除${confirmTarget?.type === 'tag' ? '标签' : '分类'}"${confirmTarget?.name || ''}"？`}
        danger
        confirmText="删除"
        onConfirm={handleConfirm}
        onCancel={() => setConfirmOpen(false)}
      />

      <h2 style={{ fontWeight: 700, marginBottom: 20 }}>设置</h2>

      <div className="settings-layout">
        <nav className="settings-nav" aria-label="设置分类">
          <a href="#settings-section-cbz">CBZ 归档</a>
          <a href="#settings-section-library">漫画库</a>
          <a href="#settings-section-reader">阅读器</a>
          <a href="#settings-section-appearance">外观</a>
          <a href="#settings-section-tags">标签管理</a>
          <a href="#settings-section-categories">分类管理</a>
          <a href="#settings-section-stats">统计</a>
          <a href="#settings-section-backup">备份与恢复</a>
        </nav>

        <div className="settings-content">
      {/* CBZ 归档设置 */}
      <div id="settings-section-cbz" className="settings-section">
        <div className="settings-section-title">📦 CBZ 归档</div>
        <div className="settings-row">
          <div style={{ flex: 1, minWidth: 0 }}>
            <div className="settings-row-label">归档导出目录</div>
            <div className="settings-row-desc">
              将漫画文件夹打包为 CBZ 时的输出目录
            </div>
            {settings.cbz_export_dir && (
              <div className="settings-row-desc" style={{ marginTop: 4, wordBreak: 'break-all' }}>
                当前: {settings.cbz_export_dir}
              </div>
            )}
          </div>
          <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
            {isTauri ? (
              <button className="btn btn-sm" onClick={handleSelectCbzDir}>浏览...</button>
            ) : (
              <>
                <input
                  value={cbzDirInput}
                  onChange={(e) => setCbzDirInput(e.target.value)}
                  placeholder="CBZ 导出目录路径"
                  style={{ minWidth: 200 }}
                />
                <button className="btn btn-sm" onClick={() => handleUpdateSetting('cbz_export_dir', cbzDirInput)}>
                  保存
                </button>
              </>
            )}
          </div>
        </div>
        {settings.cbz_export_dir && cbzFiles.length > 0 && (
          <div style={{ marginTop: 16 }}>
            <div className="settings-row-label" style={{ marginBottom: 8 }}>已导出文件</div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6, maxHeight: 240, overflow: 'auto' }}>
              {cbzFiles.map(f => (
                <div key={f.path} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 10px', background: 'var(--bg-tertiary)', borderRadius: 'var(--radius-sm)' }}>
                  <span style={{ flex: 1, fontSize: 13, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={f.path}>{f.name}</span>
                  <span style={{ fontSize: 12, color: 'var(--text-tertiary)', whiteSpace: 'nowrap' }}>{formatSize(f.size)}</span>
                  <button className="btn btn-sm" onClick={() => api.openFile(f.path).then(r => { toast(r.message || '已打开', 'success'); }).catch(e => toast(e.message, 'error'))}>打开</button>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* 漫画库设置 */}
      <div id="settings-section-library" className="settings-section">
        <div className="settings-section-title">🗂️ 漫画库</div>
        <div className="settings-row">
          <div>
            <div className="settings-row-label">重命名建议路径层数</div>
            <div className="settings-row-desc">重命名弹窗中，路径建议只显示最后 N 层目录（0=显示完整路径）</div>
          </div>
          <select value={settings.rename_suggest_depth || '3'} onChange={(e) => handleUpdateSetting('rename_suggest_depth', e.target.value)}>
            <option value="0">显示完整路径</option>
            <option value="1">1 层</option>
            <option value="2">2 层</option>
            <option value="3">3 层</option>
            <option value="4">4 层</option>
            <option value="5">5 层</option>
          </select>
        </div>
      </div>

      {/* 阅读器设置 */}
      <div id="settings-section-reader" className="settings-section">
        <div className="settings-section-title">📖 阅读器</div>
        <div className="settings-row">
          <div>
            <div className="settings-row-label">翻页方向</div>
            <div className="settings-row-desc">日漫从右往左翻</div>
          </div>
          <select value={settings.page_direction || 'rtl'} onChange={(e) => handleUpdateSetting('page_direction', e.target.value)}>
            <option value="rtl">从右到左 (日漫)</option>
            <option value="ltr">从左到右</option>
          </select>
        </div>
        <div className="settings-row">
          <div>
            <div className="settings-row-label">默认适应模式</div>
          </div>
          <select value={settings.reader_fit || 'height'} onChange={(e) => handleUpdateSetting('reader_fit', e.target.value)}>
            <option value="height">适应高度</option>
            <option value="width">适应宽度</option>
            <option value="original">原始大小</option>
          </select>
        </div>
        <div className="settings-row">
          <div>
            <div className="settings-row-label">阅读器背景色</div>
          </div>
          <input type="color" value={settings.reader_bg || '#1a1a1a'} onChange={(e) => handleUpdateSetting('reader_bg', e.target.value)} style={{ width: 50, padding: 2 }} />
        </div>
      </div>

      {/* 外观设置 */}
      <div id="settings-section-appearance" className="settings-section">
        <div className="settings-section-title">🎨 外观</div>
        <div className="settings-row">
          <div>
            <div className="settings-row-label">默认视图</div>
          </div>
          <select value={settings.view_mode || 'grid'} onChange={(e) => handleUpdateSetting('view_mode', e.target.value)}>
            <option value="grid">网格</option>
            <option value="list">列表</option>
          </select>
        </div>
        <div className="settings-row">
          <div>
            <div className="settings-row-label">默认排序</div>
          </div>
          <select value={settings.sort_by || 'updated'} onChange={(e) => handleUpdateSetting('sort_by', e.target.value)}>
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
          <select value={settings.sort_order || 'desc'} onChange={(e) => handleUpdateSetting('sort_order', e.target.value)}>
            <option value="desc">降序</option>
            <option value="asc">升序</option>
          </select>
        </div>
      </div>

      {/* 标签管理 */}
      <div id="settings-section-tags" className="settings-section">
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
                <button onClick={() => { setConfirmTarget({ type: 'tag', id: t.id, name: t.name }); setConfirmOpen(true); }} aria-label={`删除标签 ${t.name}`} style={{ background: 'none', border: 'none', color: 'var(--text-tertiary)', cursor: 'pointer', padding: 0, fontSize: 14 }}>×</button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* 分类管理 */}
      <div id="settings-section-categories" className="settings-section">
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
                <button onClick={() => { setConfirmTarget({ type: 'category', id: c.id, name: c.name }); setConfirmOpen(true); }} aria-label={`删除分类 ${c.name}`} style={{ background: 'none', border: 'none', color: 'var(--text-tertiary)', cursor: 'pointer', padding: 0, fontSize: 14 }}>×</button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* 统计信息 */}
      {stats && (
        <div id="settings-section-stats" className="settings-section">
          <div className="settings-section-title">📊 统计</div>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))', gap: 12 }}>
            <StatCard label="漫画总数" value={stats.archives} icon="📚" />
            <StatCard label="总页数" value={(stats.total_pages ?? 0).toLocaleString()} icon="📄" />
            <StatCard label="标签数" value={stats.tags} icon="🏷️" />
            <StatCard label="分类数" value={stats.categories} icon="📂" />
            <StatCard label="阅读记录" value={stats.history} icon="📖" />
            <StatCard label="总大小" value={formatSize(stats.total_size)} icon="💾" />
          </div>
        </div>
      )}

      {/* 备份与恢复 */}
      <div id="settings-section-backup" className="settings-section">
        <div className="settings-section-title">💾 备份与恢复</div>
        <div className="settings-row-desc" style={{ marginBottom: 12 }}>
          导出所有漫画元数据、标签、分类和阅读历史（不含图片文件）
        </div>
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
          <button className="btn btn-sm" onClick={handleExportBackup}>📦 导出备份</button>
          <label className="btn btn-sm btn-secondary" style={{ cursor: 'pointer', margin: 0 }}>
            📥 导入备份
            <input type="file" accept=".json" onChange={handleImportBackup} style={{ display: 'none' }} disabled={importing} />
          </label>
          {importing && <span style={{ fontSize: 13, color: 'var(--text-secondary)', alignSelf: 'center' }}>导入中...</span>}
        </div>
      </div>
        </div>
      </div>
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
