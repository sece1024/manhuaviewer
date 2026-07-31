import React, { useState, useEffect, useCallback } from 'react';
import api from '../utils/api';

/**
 * CategoryPicker — 弹窗组件，用于给指定漫画分配/取消分类
 * Props:
 *   archiveId  — 单个漫画 ID（与 archiveIds 二选一）
 *   archiveIds — 多个漫画 ID 数组，提供时进入"批量分类"模式
 *   onClose    — 关闭回调（带 changed 参数指示是否有改动）
 */
export default function CategoryPicker({ archiveId, archiveIds, onClose }) {
  const isBatch = Array.isArray(archiveIds) && archiveIds.length > 0;
  const [allCategories, setAllCategories] = useState([]);
  const [assignedIds, setAssignedIds] = useState(new Set());
  const [loading, setLoading] = useState(true);
  const [newName, setNewName] = useState('');
  const [creating, setCreating] = useState(false);
  const [changed, setChanged] = useState(false);

  useEffect(() => {
    let cancelled = false;
    if (isBatch) {
      api.getCategories().then(categories => {
        if (cancelled) return;
        setAllCategories(categories);
        setLoading(false);
      }).catch(() => setLoading(false));
    } else {
      Promise.all([api.getCategories(), api.getArchiveCategories(archiveId)])
        .then(([categories, assigned]) => {
          if (cancelled) return;
          setAllCategories(categories);
          setAssignedIds(new Set(assigned.map(c => c.id)));
          setLoading(false);
        })
        .catch(() => setLoading(false));
    }
    return () => { cancelled = true; };
  }, [archiveId, isBatch]);

  const toggle = useCallback(async (category) => {
    // 动态分类（配置了 search 表达式）由系统自动归类，不支持手动分配/取消
    if (category.search) return;
    if (isBatch) {
      try {
        await api.batchAssignCategory(archiveIds, category.id);
        setChanged(true);
      } catch (e) {
        // 静默失败
      }
      return;
    }
    const isAssigned = assignedIds.has(category.id);
    try {
      if (isAssigned) {
        await api.removeCategory(archiveId, category.id);
        setAssignedIds(prev => { const s = new Set(prev); s.delete(category.id); return s; });
      } else {
        await api.assignCategory(archiveId, category.id);
        setAssignedIds(prev => new Set(prev).add(category.id));
      }
      setChanged(true);
    } catch (e) {
      // 静默失败，保持 UI 一致
    }
  }, [archiveId, archiveIds, assignedIds, isBatch]);

  const removeBatch = useCallback(async (e, categoryId) => {
    e.stopPropagation();
    try {
      await api.batchRemoveCategory(archiveIds, categoryId);
      setChanged(true);
    } catch (err) {
      // 静默失败
    }
  }, [archiveIds]);

  const handleCreate = async () => {
    const name = newName.trim();
    if (!name || creating) return;
    setCreating(true);
    try {
      const result = await api.createCategory({ name });
      const newCategory = result.data || result;
      setAllCategories(prev => [...prev, { ...newCategory, archive_count: 0 }]);
      if (isBatch) {
        await api.batchAssignCategory(archiveIds, newCategory.id);
      } else {
        await api.assignCategory(archiveId, newCategory.id);
        setAssignedIds(prev => new Set(prev).add(newCategory.id));
      }
      setNewName('');
      setChanged(true);
    } catch (e) {
      // ignore
    }
    setCreating(false);
  };

  return (
    <div className="modal-overlay" onClick={() => onClose(changed)}>
      <div className="modal tag-picker-modal" onClick={e => e.stopPropagation()}>
        <div className="modal-title">📂 {isBatch ? `批量分类（已选 ${archiveIds.length} 个）` : '管理分类'}</div>
        <div className="modal-body">
          {loading ? (
            <div style={{ textAlign: 'center', padding: 20, color: 'var(--text-secondary)' }}>加载中...</div>
          ) : (
            <>
              {allCategories.length === 0 ? (
                <div style={{ color: 'var(--text-tertiary)', fontSize: 13, textAlign: 'center', padding: 16 }}>
                  暂无分类，在下方创建
                </div>
              ) : (
                <div className="tag-picker-list">
                  {allCategories.map(c => {
                    const checked = !isBatch && assignedIds.has(c.id);
                    const isDynamic = !!c.search;
                    return (
                      <div
                        key={c.id}
                        className={`tag-picker-item ${checked ? 'checked' : ''}`}
                        onClick={() => toggle(c)}
                        title={isDynamic ? `动态分类，按"${c.search}"自动匹配` : undefined}
                        style={isDynamic ? { opacity: 0.6, cursor: 'default' } : undefined}
                      >
                        <span className="tag-picker-check">{checked ? '✓' : ''}</span>
                        <span className="tag-picker-color" style={{ background: c.color }} />
                        <span className="tag-picker-name">{c.name}{isDynamic ? ' (动态)' : ''}</span>
                        {isBatch && !isDynamic && (
                          <button
                            className="btn btn-sm btn-secondary"
                            style={{ marginLeft: 'auto' }}
                            onClick={(e) => removeBatch(e, c.id)}
                            title="从所选漫画中移除该分类"
                          >
                            移除
                          </button>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}

              {/* 快速创建 */}
              <div className="tag-picker-create">
                <input
                  placeholder="新建分类名称"
                  value={newName}
                  onChange={e => setNewName(e.target.value)}
                  onKeyDown={e => e.key === 'Enter' && handleCreate()}
                  autoFocus={allCategories.length === 0}
                />
                <button className="btn btn-sm" onClick={handleCreate} disabled={creating || !newName.trim()}>
                  创建
                </button>
              </div>
            </>
          )}
        </div>
        <div className="modal-actions">
          <button className="btn" onClick={() => onClose(changed)}>完成</button>
        </div>
      </div>
    </div>
  );
}
