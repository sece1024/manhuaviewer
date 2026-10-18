import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import { localStorageGet, localStorageSet } from '../utils/api';

/**
 * useTheme — 主题（浅色 / 深色 / 护眼 / 跟随系统）。
 *
 * 主题是**纯客户端**设置（localStorage），不走服务端 settings：同一台机器上的
 * iPad 与桌面窗口可以各自选主题，也不会因为服务端不可用而卡在默认值。
 *
 * 之前的实现把主题 state 放在 App 里、把切换器放在侧栏 footer，导致：
 * - 命令面板写的是服务端 `theme`，App 从不读取（点了没反应）；
 * - ≤768px 时侧栏 footer 被隐藏，手机端完全没有主题入口。
 * 现在由 Provider 统一暴露 theme/setTheme，App、侧栏、设置页与命令面板共用。
 */
const THEME_KEY = 'theme';
const THEMES = ['light', 'dark', 'eye-care'];
const PREFERENCES = [...THEMES, 'system'];

const ThemeContext = createContext(null);

function readPreference() {
  const saved = localStorageGet(THEME_KEY);
  return PREFERENCES.includes(saved) ? saved : 'dark';
}

function systemTheme() {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return 'dark';
  return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

export function ThemeProvider({ children }) {
  const [theme, setThemeState] = useState(readPreference);
  const [systemResolved, setSystemResolved] = useState(systemTheme);

  // 只有「跟随系统」时需要监听，避免无谓的 media query 订阅
  useEffect(() => {
    if (theme !== 'system') return undefined;
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return undefined;
    const mq = window.matchMedia('(prefers-color-scheme: light)');
    const onChange = () => setSystemResolved(mq.matches ? 'light' : 'dark');
    onChange();
    if (mq.addEventListener) mq.addEventListener('change', onChange);
    else mq.addListener(onChange); // 旧 WebView 兜底
    return () => {
      if (mq.removeEventListener) mq.removeEventListener('change', onChange);
      else mq.removeListener(onChange);
    };
  }, [theme]);

  const resolvedTheme = theme === 'system' ? systemResolved : theme;

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', resolvedTheme);
  }, [resolvedTheme]);

  const setTheme = useCallback((next) => {
    const value = PREFERENCES.includes(next) ? next : 'dark';
    setThemeState(value);
    localStorageSet(THEME_KEY, value);
  }, []);

  const value = useMemo(
    () => ({ theme, resolvedTheme, setTheme }),
    [theme, resolvedTheme, setTheme]
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export default function useTheme() {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error('useTheme must be used within ThemeProvider');
  return ctx;
}
