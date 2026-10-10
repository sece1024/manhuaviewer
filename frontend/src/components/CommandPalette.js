import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';
import { useNavigate } from 'react-router-dom';
import api from '../utils/api';
import useSettings from '../hooks/useSettings';
import useModalKeyboard from '../hooks/useModalKeyboard';
import { getPageCommands, subscribeCommands } from '../hooks/useCommands';

/**
 * CommandPalette — 全局命令面板（⌘K / Ctrl-K）。
 *
 * 解决的问题：书库一屏有 20 多个操作，散在顶栏、侧栏、卡片、长按面板与整理模式里，
 * 用户必须记住"这个功能在哪一栏"；而"找到某本书并打开"这条最高频的路径，在换页/
 * 换筛选之后甚至要先清掉筛选才能用。面板把两件事合成一条通道：输入 → 回车。
 *
 * 三类条目：
 * 1. 导航（书库/历史/设置）——外壳自带，任何页面都在；
 * 2. 当前页面的操作——页面通过 useCommands 登记（它在页面内部，才拿得到 setState）；
 * 3. 漫画——按标题实时搜索，回车直接进阅读器。
 */

const EMPTY_LIST = [];

export default function CommandPalette({ onClose }) {
  const navigate = useNavigate();
  const { settings, updateSetting } = useSettings();
  const pageCommands = useSyncExternalStore(subscribeCommands, getPageCommands);
  const [query, setQuery] = useState('');
  const [highlight, setHighlight] = useState(0);
  const [archives, setArchives] = useState(EMPTY_LIST);
  const panelRef = useModalKeyboard(onClose);
  const searchTimerRef = useRef(null);

  const theme = settings.theme || 'dark';

  /// 面板自己就能做的全局命令；页面专属的由页面登记
  const globalCommands = useMemo(() => {
    const nav = [
      { id: 'nav-library', group: '导航', icon: '📚', label: '去书库', keywords: 'library 漫画库', run: () => navigate('/') },
      { id: 'nav-history', group: '导航', icon: '📖', label: '去阅读历史', keywords: 'history', run: () => navigate('/history') },
      { id: 'nav-settings', group: '导航', icon: '⚙️', label: '去设置', keywords: 'settings 偏好', run: () => navigate('/settings') },
    ];
    const themes = [
      ['light', '浅色'], ['dark', '深色'], ['eye-care', '护眼'],
    ].map(([value, label]) => ({
      id: `theme-${value}`,
      group: '外观',
      icon: theme === value ? '✅' : '🎨',
      label: `主题：${label}`,
      keywords: 'theme 主题 外观',
      run: () => updateSetting('theme', value),
    }));
    return [...nav, ...themes];
  }, [navigate, theme, updateSetting]);

  // 漫画实时搜索：只在有输入时发请求，并防抖（面板是逐字输入的）
  useEffect(() => {
    clearTimeout(searchTimerRef.current);
    const q = query.trim();
    if (!q) {
      setArchives(EMPTY_LIST);
      return undefined;
    }
    searchTimerRef.current = setTimeout(() => {
      Promise.resolve(api.getArchives({ search: q, limit: 8, page: 1 }))
        .then(list => setArchives(Array.isArray(list) ? list : EMPTY_LIST))
        .catch(() => setArchives(EMPTY_LIST));
    }, 150);
    return () => clearTimeout(searchTimerRef.current);
  }, [query]);

  const commands = useMemo(() => {
    const all = [...globalCommands, ...(pageCommands || [])];
    const q = query.trim().toLowerCase();
    const matched = q
      ? all.filter(c => `${c.label} ${c.keywords || ''} ${c.group || ''}`.toLowerCase().includes(q))
      : all;
    const archiveCommands = archives.map(a => ({
      id: `open-${a.id}`,
      group: '漫画',
      icon: '📖',
      label: `打开《${a.title}》`,
      hint: a._isGroup ? `${a.chapter_count} 话` : `${a.page_count} 页`,
      run: () => navigate(`/reader/${a.id}`),
    }));
    return [...matched, ...archiveCommands];
  }, [archives, globalCommands, navigate, pageCommands, query]);

  // 输入或结果集变化后把高亮收回第一项：回车永远是"最匹配的那个"
  useEffect(() => { setHighlight(0); }, [query, commands.length]);

  const runCommand = useCallback((cmd) => {
    onClose();
    cmd.run();
  }, [onClose]);

  const onKeyDown = (e) => {
    if (e.key === 'ArrowDown' || e.key === 'Tab') {
      e.preventDefault();
      if (commands.length) setHighlight(h => (h + 1) % commands.length);
      return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (commands.length) setHighlight(h => (h - 1 + commands.length) % commands.length);
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      const cmd = commands[Math.min(highlight, commands.length - 1)];
      if (cmd) runCommand(cmd);
    }
  };

  // 分组渲染：保持登记顺序，同一 group 连续出现
  const groups = [];
  commands.forEach((cmd, index) => {
    const name = cmd.group || '操作';
    const last = groups[groups.length - 1];
    if (last && last.name === name) last.items.push({ cmd, index });
    else groups.push({ name, items: [{ cmd, index }] });
  });

  return (
    <div className="modal-overlay palette-overlay" onClick={onClose}>
      <div
        ref={panelRef}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-label="命令面板"
        className="palette"
        style={{ outline: 'none' }}
        onClick={e => e.stopPropagation()}
      >
        <input
          className="palette-input"
          value={query}
          onChange={e => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder="输入命令或漫画名…（↑↓ 选择，回车执行）"
          aria-label="搜索命令或漫画"
          autoFocus
        />
        <div className="palette-list" role="listbox" aria-label="命令列表">
          {commands.length === 0 ? (
            <div className="palette-empty">没有匹配的条目</div>
          ) : (
            groups.map(group => (
              <div key={group.name} className="palette-group">
                <div className="palette-group-title">{group.name}</div>
                {group.items.map(({ cmd, index }) => (
                  <button
                    key={cmd.id}
                    type="button"
                    role="option"
                    aria-selected={index === highlight}
                    className={`palette-item ${index === highlight ? 'active' : ''}`}
                    onMouseEnter={() => setHighlight(index)}
                    onClick={() => runCommand(cmd)}
                  >
                    <span className="palette-icon" aria-hidden="true">{cmd.icon || '•'}</span>
                    <span className="palette-label">{cmd.label}</span>
                    {cmd.hint && <span className="palette-hint">{cmd.hint}</span>}
                  </button>
                ))}
              </div>
            ))
          )}
        </div>
        <div className="palette-foot">
          <kbd>↑</kbd><kbd>↓</kbd> 选择 · <kbd>Enter</kbd> 执行 · <kbd>Esc</kbd> 关闭
        </div>
      </div>
    </div>
  );
}

// 供外壳复用：命令面板的快捷键提示文案
export const PALETTE_HINT = '⌘K';
