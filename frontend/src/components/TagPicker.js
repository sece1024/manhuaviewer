import React, { useState, useEffect, useCallback } from 'react';
import api from '../utils/api';
import useModalKeyboard from '../hooks/useModalKeyboard';

/**
 * TagPicker — 弹窗组件，用于给指定漫画分配/取消标签
 * Props:
 *   archiveId  — 单个漫画 ID（与 archiveIds 二选一）
 *   archiveIds — 多个漫画 ID 数组，提供时进入"批量打标签"模式
 *   onClose    — 关闭回调（带 changed 参数指示是否有改动）
 */
export default function TagPicker({ archiveId, archiveIds, onClose }) {
  const isBatch = Array.isArray(archiveIds) && archiveIds.length > 0;
  const [allTags, setAllTags] = useState([]);
  const [assignedIds, setAssignedIds] = useState(new Set());
  const [loading, setLoading] = useState(true);
  const [newName, setNewName] = useState('');
  const [creating, setCreating] = useState(false);
  const [changed, setChanged] = useState(false);
  const panelRef = useModalKeyboard(() => onClose(changed));

  useEffect(() => {
    let cancelled = false;
    if (isBatch) {
      api.getTags().then(tags => {
        if (cancelled) return;
        setAllTags(tags);
        setLoading(false);
      }).catch(() => setLoading(false));
    } else {
      Promise.all([api.getTags(), api.getArchiveTags(archiveId)])
        .then(([tags, assigned]) => {
          if (cancelled) return;
          setAllTags(tags);
          setAssignedIds(new Set(assigned.map(t => t.id)));
          setLoading(false);
        })
        .catch(() => setLoading(false));
    }
    return () => { cancelled = true; };
  }, [archiveId, isBatch]);

  const toggle = useCallback(async (tagId) => {
    if (isBatch) {
      try {
        await api.batchAssignTag(archiveIds, tagId);
        setChanged(true);
      } catch (e) {
        // 静默失败
      }
      return;
    }
    const isAssigned = assignedIds.has(tagId);
    try {
      if (isAssigned) {
        await api.removeTag(archiveId, tagId);
        setAssignedIds(prev => { const s = new Set(prev); s.delete(tagId); return s; });
      } else {
        await api.assignTag(archiveId, tagId);
        setAssignedIds(prev => new Set(prev).add(tagId));
      }
      setChanged(true);
    } catch (e) {
      // 静默失败，保持 UI 一致
    }
  }, [archiveId, archiveIds, assignedIds, isBatch]);

  const removeBatch = useCallback(async (e, tagId) => {
    e.stopPropagation();
    try {
      await api.batchRemoveTag(archiveIds, tagId);
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
      // 支持 namespace:name 格式
      let namespace = '';
      let tagName = name;
      if (name.includes(':')) {
        const idx = name.indexOf(':');
        namespace = name.slice(0, idx);
        tagName = name.slice(idx + 1);
      }
      const result = await api.createTag({ namespace, name: tagName });
      const newTag = result.data || result;
      setAllTags(prev => [...prev, { ...newTag, archive_count: 0 }]);
      // 自动分配
      if (isBatch) {
        await api.batchAssignTag(archiveIds, newTag.id);
      } else {
        await api.assignTag(archiveId, newTag.id);
        setAssignedIds(prev => new Set(prev).add(newTag.id));
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
      <div ref={panelRef} tabIndex={-1} className="modal tag-picker-modal" style={{ outline: 'none' }} onClick={e => e.stopPropagation()}>
        <div className="modal-title">🏷️ {isBatch ? `批量打标签（已选 ${archiveIds.length} 个）` : '管理标签'}</div>
        <div className="modal-body">
          {loading ? (
            <div style={{ textAlign: 'center', padding: 20, color: 'var(--text-secondary)' }}>加载中...</div>
          ) : (
            <>
              {allTags.length === 0 ? (
                <div style={{ color: 'var(--text-tertiary)', fontSize: 13, textAlign: 'center', padding: 16 }}>
                  暂无标签，在下方创建
                </div>
              ) : (
                <div className="tag-picker-list">
                  {allTags.map(t => {
                    const fullName = t.namespace ? `${t.namespace}:${t.name}` : t.name;
                    const checked = !isBatch && assignedIds.has(t.id);
                    return (
                      <div
                        key={t.id}
                        className={`tag-picker-item ${checked ? 'checked' : ''}`}
                        onClick={() => toggle(t.id)}
                      >
                        <span className="tag-picker-check">{checked ? '✓' : ''}</span>
                        <span className="tag-picker-color" style={{ background: t.color }} />
                        <span className="tag-picker-name">{fullName}</span>
                        {isBatch && (
                          <button
                            className="btn btn-sm btn-secondary"
                            style={{ marginLeft: 'auto' }}
                            onClick={(e) => removeBatch(e, t.id)}
                            title="从所选漫画中移除该标签"
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
                  placeholder="新建标签（支持 ns:name）"
                  value={newName}
                  onChange={e => setNewName(e.target.value)}
                  onKeyDown={e => e.key === 'Enter' && handleCreate()}
                  autoFocus={allTags.length === 0}
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
