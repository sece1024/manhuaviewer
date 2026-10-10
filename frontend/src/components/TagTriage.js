import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import api from '../utils/api';
import { useToast } from './Toast';
import useTags from '../hooks/useTags';
import LazyImage from './LazyImage';
import { formatReadProgress } from '../utils/format';

/**
 * TagTriage — 键盘驱动的「整理模式」：一次一本，给未打标签的漫画打标签。
 *
 * 此前给 N 本打标签的成本是每本 5 步（悬停卡片 → 点标签按钮 → 弹窗里找标签 →
 * 点击 → 关闭再找下一本），而且大量时间花在"找到刚才那本"上。整理模式把它压成
 * 每个条目 1–2 次击键：输入即筛选候选，回车打标并自动前进。
 *
 * 键盘约定（刻意与弹层惯例不同，见下）：
 * - 输入框内 ↑/↓ 或 Tab/Shift+Tab：切换候选（Tab 被 preventDefault，焦点不会逃出面板）
 * - Enter：给高亮的候选打标并前进；输入的内容不是已有标签时，候选末位是「新建 …」
 * - Esc：**跳过**这一本（继续下一本），不是关闭
 * - Shift+Esc：退出整理
 *
 * 为什么 Esc 不是关闭：整理流程里"这本我一时想不出该打什么"远比"我要退出"频繁，
 * 让高频动作占用最顺手的键更合理。因此这里**不能**用 useModalKeyboard —— 它在
 * document capture 阶段把 Esc 固定成关闭并 stopPropagation，会和跳过打架。
 * 退出改由明确按钮 + Shift+Esc 承担，并在面板底部把键位写出来。
 */

const PAGE_SIZE = 200; // 一次取一批未打标签的档案（见 refill 注释：不做二次分页）
const REFILL_THRESHOLD = 5;

/// 把标签整理成候选：namespace:name 显示与匹配都用全名
const fullName = (t) => (t.namespace ? `${t.namespace}:${t.name}` : t.name);

export default function TagTriage({ sortBy = 'created', sortOrder = 'desc', onClose }) {
  const toast = useToast();
  const { tags, reload: reloadTags } = useTags();
  const [queue, setQueue] = useState([]);
  const [loading, setLoading] = useState(true);
  const [index, setIndex] = useState(0);
  const [query, setQuery] = useState('');
  const [highlight, setHighlight] = useState(0);
  const [busy, setBusy] = useState(false);
  const [taggedCount, setTaggedCount] = useState(0);
  const [skippedCount, setSkippedCount] = useState(0);
  const [refilling, setRefilling] = useState(false);
  const queueIdsRef = useRef(new Set());
  // 上一批是否取满：只有取满才可能还有下一批，否则自动补批会对着"没有新条目"
  // 反复触发并弹出一串无意义提示
  const batchFullRef = useRef(false);

  const current = queue[index] || null;
  const exhausted = !loading && index >= queue.length;

  // 队列：一次取一批「未打标签」的档案。刻意不做「翻页取下一页」——
  // 打标签会让条目离开 tag_state=untagged 这个集合，用 offset 分页会在中途漏书；
  // 补充批次一律重新取第 1 页，再按 id 去重追加（见 continueBatch）。
  const fetchBatch = useCallback(async () => {
    const items = await api.getArchives({
      tag_state: 'untagged',
      sort_by: sortBy,
      sort_order: sortOrder,
      limit: PAGE_SIZE,
      page: 1,
    });
    return Array.isArray(items) ? items : [];
  }, [sortBy, sortOrder]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    fetchBatch()
      .then(items => {
        if (cancelled) return;
        queueIdsRef.current = new Set(items.map(i => i.id));
        batchFullRef.current = items.length >= PAGE_SIZE;
        setQueue(items);
        setIndex(0);
      })
      .catch(e => {
        if (!cancelled) toast(e.message || '读取未打标签的档案失败', 'error');
      })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
    // 只在打开时取一次；补充批次由 continueBatch 显式触发
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const continueBatch = useCallback(async ({ manual = false } = {}) => {
    if (refilling) return;
    setRefilling(true);
    try {
      const items = await fetchBatch();
      const fresh = items.filter(i => !queueIdsRef.current.has(i.id));
      fresh.forEach(i => queueIdsRef.current.add(i.id));
      batchFullRef.current = items.length >= PAGE_SIZE && fresh.length > 0;
      if (fresh.length > 0) {
        setQueue(prev => [...prev, ...fresh]);
      } else if (manual) {
        toast('没有读到新的未打标签档案（剩下的可能都被跳过了）', 'info');
      }
    } catch (e) {
      toast(e.message || '继续读取失败', 'error');
    } finally {
      setRefilling(false);
    }
  }, [fetchBatch, refilling, toast]);

  // 候选 = 过滤后的已有标签（+ 末位的「新建」）
  const candidates = useMemo(() => {
    const q = query.trim().toLowerCase();
    const matched = tags.filter(t => !q || fullName(t).toLowerCase().includes(q));
    const list = matched.map(t => ({ kind: 'tag', tagId: t.id, label: fullName(t), color: t.color }));
    if (q) {
      const exact = tags.some(t => fullName(t).toLowerCase() === q);
      if (!exact) {
        const idx = query.trim().indexOf(':');
        const namespace = idx > 0 ? query.trim().slice(0, idx) : '';
        const name = idx > 0 ? query.trim().slice(idx + 1) : query.trim();
        list.push({ kind: 'create', label: `新建「${query.trim()}」`, namespace, name });
      }
    }
    return list;
  }, [tags, query]);

  // 候选变化后把高亮夹回范围内（输入筛选时默认落回第一个 = 回车即可打标）
  useEffect(() => { setHighlight(0); }, [query]);

  const advance = useCallback(() => {
    setQuery('');
    setIndex(i => i + 1);
  }, []);

  const skip = useCallback(() => {
    setSkippedCount(n => n + 1);
    advance();
  }, [advance]);

  const apply = useCallback(async (explicit) => {
    if (busy || !current) return;
    // 点击候选时传入被点的那个；键盘回车用高亮的那个
    const cand = explicit || candidates[Math.min(highlight, candidates.length - 1)];
    if (!cand) return; // 库里还没有任何标签且没有输入：无从下手（面板里有提示）
    setBusy(true);
    try {
      let tagId = cand.tagId;
      if (cand.kind === 'create') {
        const created = await api.createTag({ namespace: cand.namespace, name: cand.name });
        tagId = (created?.data || created)?.id;
        if (!tagId) throw new Error('创建标签失败');
        // 新建的标签要立刻进候选与侧栏，否则下一本还得重新输入
        await reloadTags();
      }
      await api.assignTag(current.id, tagId);
      setTaggedCount(n => n + 1);
      advance();
    } catch (e) {
      // 失败**不前进**：否则用户以为打上了，实际上这本会被永远跳过
      toast(e.message || '打标签失败', 'error');
    } finally {
      setBusy(false);
    }
  }, [advance, busy, candidates, current, highlight, reloadTags, toast]);

  const onKeyDown = (e) => {
    if (e.key === 'Escape' && e.shiftKey) {
      e.preventDefault();
      onClose();
      return;
    }
    if (e.key === 'Escape') {
      // Esc = 跳过（不是关闭），见文件头注释
      e.preventDefault();
      e.stopPropagation();
      skip();
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      apply();
      return;
    }
    if (e.key === 'Tab' || e.key === 'ArrowDown') {
      e.preventDefault();
      if (candidates.length > 0) setHighlight(h => (h + 1) % candidates.length);
      return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (candidates.length > 0) setHighlight(h => (h - 1 + candidates.length) % candidates.length);
    }
  };

  // 队列快见底就自动补一批，用户不必手动点「继续」（只有上一批取满时才可能还有）
  useEffect(() => {
    if (loading || exhausted || !batchFullRef.current) return;
    if (queue.length > 0 && queue.length - index <= REFILL_THRESHOLD) {
      continueBatch();
    }
  }, [continueBatch, exhausted, index, loading, queue.length]);

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal tag-triage"
        role="dialog"
        aria-modal="true"
        aria-label="整理标签"
        onClick={e => e.stopPropagation()}
      >
        <div className="tag-triage-head">
          <span className="tag-triage-title">🏷️ 整理标签</span>
          <span className="tag-triage-count">
            已整理 {taggedCount} 本{skippedCount > 0 ? ` · 跳过 ${skippedCount}` : ''}
          </span>
          <button className="btn btn-sm btn-secondary" onClick={onClose}>退出</button>
        </div>

        {loading ? (
          <div className="empty-state" style={{ padding: 24 }}>加载中…</div>
        ) : exhausted ? (
          <div className="tag-triage-done">
            <div className="tag-triage-done-icon">✅</div>
            <div>这一批整理完了：打标 {taggedCount} 本{skippedCount > 0 ? `，跳过 ${skippedCount} 本` : ''}</div>
            <div className="tag-triage-hint">
              跳过的书仍留在「未打标签」里；如果还有更多，可以继续下一批。
            </div>
            <div style={{ display: 'flex', gap: 8, justifyContent: 'center', marginTop: 12 }}>
              <button className="btn btn-secondary" onClick={() => continueBatch({ manual: true })} disabled={refilling}>
                {refilling ? '读取中…' : '继续下一批'}
              </button>
              <button className="btn" onClick={onClose}>完成</button>
            </div>
          </div>
        ) : (
          <>
            <div className="tag-triage-item">
              <div className="tag-triage-cover">
                <LazyImage src={current.cover_url} alt="" />
              </div>
              <div className="tag-triage-meta">
                <div className="tag-triage-name" title={current.title}>{current.title}</div>
                <div className="tag-triage-sub">
                  {current._isGroup ? `${current.chapter_count} 话` : `${current.page_count} 页`}
                  {' · '}{current.archive_type === 'folder' ? '文件夹' : '压缩包'}
                  {current.read_page !== undefined && current.read_page !== null
                    ? ` · ${formatReadProgress(current)}` : ''}
                </div>
                <div className="tag-triage-sub">
                  还剩 {queue.length - index} 本待整理
                </div>
              </div>
            </div>

            <input
              className="tag-triage-input"
              value={query}
              onChange={e => setQuery(e.target.value)}
              onKeyDown={onKeyDown}
              placeholder="输入标签名筛选，回车打标并下一本"
              aria-label="标签"
              autoFocus
            />

            <div className="tag-triage-candidates" role="listbox" aria-label="标签候选">
              {candidates.length === 0 ? (
                <div className="tag-triage-hint">
                  {tags.length === 0
                    ? '书库还没有标签：直接输入名字，回车即创建并打标'
                    : '没有匹配的标签：换个词，或清空输入看全部标签'}
                </div>
              ) : (
                candidates.map((c, i) => (
                  <button
                    key={c.kind === 'create' ? '__create__' : c.tagId}
                    type="button"
                    role="option"
                    aria-selected={i === highlight}
                    className={`tag-triage-cand ${i === highlight ? 'active' : ''}`}
                    onMouseEnter={() => setHighlight(i)}
                    onClick={() => apply(c)}
                  >
                    {c.kind === 'tag' && (
                      <span className="tag-picker-color" style={{ background: c.color }} />
                    )}
                    <span>{c.label}</span>
                  </button>
                ))
              )}
            </div>

            <div className="tag-triage-keys">
              <kbd>Enter</kbd> 打标并下一本 · <kbd>Tab</kbd>/<kbd>↑↓</kbd> 切换候选 ·{' '}
              <kbd>Esc</kbd> 跳过 · <kbd>Shift</kbd>+<kbd>Esc</kbd> 退出
            </div>
            <div className="tag-triage-actions">
              <button className="btn btn-secondary btn-sm" onClick={skip}>跳过这本</button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
