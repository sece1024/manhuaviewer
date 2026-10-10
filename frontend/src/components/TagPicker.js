import { useState, useEffect, useCallback, useMemo } from 'react';
import api from '../utils/api';
import useModalKeyboard from '../hooks/useModalKeyboard';
import { useToast } from './Toast';

/**
 * TagPicker — 弹窗组件，用于给指定漫画分配/取消标签
 * Props:
 *   archiveId  — 单个漫画 ID（与 archiveIds 二选一）
 *   archiveIds — 多个漫画 ID 数组，提供时进入"批量打标签"模式
 *   onClose    — 关闭回调（带 changed 参数指示是否有改动）
 *
 * 批量模式的三种状态（靠 `/tags/counts` 一次取回计数）：
 *   ✓ 所有选中项都包含 → 点击=从所有选中项移除
 *   – 只有部分包含     → 点击=给所有选中项都加上
 *   （空）都没有       → 点击=给所有选中项都加上
 *
 * 此前批量模式只有"加"一个方向，`checked` 还被写死为 false：界面既看不出哪些标签
 * 已经在选中项上，点错了也无法用"再点一次"撤销，只能去按旁边那个 移除 按钮。
 */

const fullName = (t) => (t.namespace ? `${t.namespace}:${t.name}` : t.name);

export default function TagPicker({ archiveId, archiveIds, onClose }) {
  const isBatch = Array.isArray(archiveIds) && archiveIds.length > 0;
  const [allTags, setAllTags] = useState([]);
  const [assignedIds, setAssignedIds] = useState(new Set()); // 单本模式：该档案已有的标签
  const [counts, setCounts] = useState({}); // 批量模式：tagId → 命中的选中档案数
  const [batchTotal, setBatchTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [newName, setNewName] = useState('');
  const [tagSearch, setTagSearch] = useState('');
  const [creating, setCreating] = useState(false);
  const [changed, setChanged] = useState(false);
  const [busyTagId, setBusyTagId] = useState(null);
  const panelRef = useModalKeyboard(() => onClose(changed));
  const toast = useToast();

  useEffect(() => {
    let cancelled = false;
    if (isBatch) {
      // 标签列表 + 计数并行取回：计数决定每个标签显示三态中的哪一种
      Promise.all([api.getTags(), Promise.resolve(api.getTagCounts(archiveIds))])
        .then(([tags, res]) => {
          if (cancelled) return;
          setAllTags(Array.isArray(tags) ? tags : []);
          setCounts((res && res.counts) || {});
          setBatchTotal((res && res.total) || archiveIds.length);
          setLoading(false);
        })
        .catch(() => { if (!cancelled) setLoading(false); });
    } else {
      Promise.all([api.getTags(), api.getArchiveTags(archiveId)])
        .then(([tags, assigned]) => {
          if (cancelled) return;
          setAllTags(Array.isArray(tags) ? tags : []);
          setAssignedIds(new Set((assigned || []).map(t => t.id)));
          setLoading(false);
        })
        .catch(() => { if (!cancelled) setLoading(false); });
    }
    return () => { cancelled = true; };
  }, [archiveId, archiveIds, isBatch]);

  const tagStateOf = useCallback((tagId) => {
    if (!isBatch) return assignedIds.has(tagId) ? 'all' : 'none';
    const n = counts[tagId] || 0;
    if (n <= 0) return 'none';
    return n >= batchTotal ? 'all' : 'some';
  }, [assignedIds, batchTotal, counts, isBatch]);

  const toggle = useCallback(async (tagId) => {
    // 只挡住"同一个标签"的重复点击：如果挡住所有标签，用户连续点两个不同标签时
    // 第二次会被静默忽略（看起来像点击没生效）
    if (busyTagId === tagId) return;
    const state = tagStateOf(tagId);
    // 三态 → 两动作：已全部包含就移除；未包含/部分包含都补成"全部包含"
    const removing = state === 'all';
    setBusyTagId(tagId);
    try {
      if (isBatch) {
        if (removing) await api.batchRemoveTag(archiveIds, tagId);
        else await api.batchAssignTag(archiveIds, tagId);
        setCounts(prev => ({ ...prev, [tagId]: removing ? 0 : batchTotal }));
      } else if (removing) {
        await api.removeTag(archiveId, tagId);
        setAssignedIds(prev => { const s = new Set(prev); s.delete(tagId); return s; });
      } else {
        await api.assignTag(archiveId, tagId);
        setAssignedIds(prev => new Set(prev).add(tagId));
      }
      setChanged(true);
    } catch (e) {
      // 以前这里整段是静默 catch：用户点了没反应，也不知道没写上
      toast(e.message || '标签操作失败', 'error');
    } finally {
      setBusyTagId(null);
    }
  }, [archiveId, archiveIds, batchTotal, busyTagId, isBatch, tagStateOf, toast]);

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
      const newTag = result?.data || result;
      if (!newTag?.id) throw new Error('创建标签失败');
      setAllTags(prev => [...prev, { ...newTag, archive_count: 0 }]);
      // 自动分配
      if (isBatch) {
        await api.batchAssignTag(archiveIds, newTag.id);
        setCounts(prev => ({ ...prev, [newTag.id]: batchTotal }));
      } else {
        await api.assignTag(archiveId, newTag.id);
        setAssignedIds(prev => new Set(prev).add(newTag.id));
      }
      setNewName('');
      setChanged(true);
    } catch (e) {
      toast(e.message || '创建标签失败', 'error');
    }
    setCreating(false);
  };

  // 标签多时先给个搜索框：100+ 标签靠滚动找是纯浪费
  const visibleTags = useMemo(() => {
    const q = tagSearch.trim().toLowerCase();
    if (!q) return allTags;
    return allTags.filter(t => fullName(t).toLowerCase().includes(q));
  }, [allTags, tagSearch]);

  const allCount = batchTotal > 0 ? batchTotal : (archiveIds || []).length;

  return (
    <div className="modal-overlay" onClick={() => onClose(changed)}>
      <div ref={panelRef} tabIndex={-1} className="modal tag-picker-modal" style={{ outline: 'none' }} onClick={e => e.stopPropagation()}>
        <div className="modal-title">🏷️ {isBatch ? `批量打标签（已选 ${archiveIds.length} 个）` : '管理标签'}</div>
        <div className="modal-body">
          {loading ? (
            <div style={{ textAlign: 'center', padding: 20, color: 'var(--text-secondary)' }}>加载中...</div>
          ) : (
            <>
              {isBatch && allTags.length > 0 && (
                <div className="settings-row-desc" style={{ marginBottom: 8 }}>
                  选中项里全部都有 ✓（点击移除）· 部分有 − · 没有的点击即加上
                </div>
              )}
              {allTags.length === 0 ? (
                <div style={{ color: 'var(--text-tertiary)', fontSize: 13, textAlign: 'center', padding: 16 }}>
                  暂无标签，在下方创建
                </div>
              ) : (
                <>
                  {allTags.length > 8 && (
                    <input
                      type="text"
                      value={tagSearch}
                      onChange={e => setTagSearch(e.target.value)}
                      placeholder="过滤标签..."
                      aria-label="按名称过滤标签"
                      style={{ width: '100%', boxSizing: 'border-box', marginBottom: 8 }}
                    />
                  )}
                  {visibleTags.length === 0 ? (
                    <div style={{ color: 'var(--text-tertiary)', fontSize: 13, padding: 8 }}>无匹配标签</div>
                  ) : (
                    <div className="tag-picker-list">
                      {visibleTags.map(t => {
                        const state = tagStateOf(t.id);
                        return (
                          <div
                            key={t.id}
                            className={`tag-picker-item ${state === 'all' ? 'checked' : ''} ${state === 'some' ? 'partial' : ''} ${busyTagId === t.id ? 'busy' : ''}`}
                            onClick={() => toggle(t.id)}
                          >
                            <span className="tag-picker-check">
                              {state === 'all' ? '✓' : state === 'some' ? '−' : ''}
                            </span>
                            <span className="tag-picker-color" style={{ background: t.color }} />
                            <span className="tag-picker-name">{fullName(t)}</span>
                            {isBatch && state === 'some' && (
                              <span className="tag-picker-partial-hint" title={`选中项里有 ${counts[t.id] || 0} / ${allCount} 个带这个标签`}>
                                {counts[t.id] || 0}/{allCount}
                              </span>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  )}
                </>
              )}

              {/* 快速创建 */}
              <div className="tag-picker-create">
                <input
                  placeholder="新建标签（支持 ns:name）"
                  value={newName}
                  onChange={e => setNewName(e.target.value)}
                  onKeyDown={e => e.key === 'Enter' && handleCreate()}
                  aria-label="新建标签"
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
