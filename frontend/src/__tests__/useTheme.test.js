import { renderHook, act } from '@testing-library/react';
import useTheme, { ThemeProvider } from '../hooks/useTheme';

jest.mock('../utils/api');
const apiModule = require('../utils/api');
const api = apiModule.default;

// automock 把 localStorage 读写也变成 no-op；换回真实实现才能测到持久化。
beforeEach(() => {
  jest.clearAllMocks();
  window.localStorage.clear();
  apiModule.localStorageGet.mockImplementation((k) => window.localStorage.getItem(k) || '');
  apiModule.localStorageSet.mockImplementation((k, v) => {
    if (v) window.localStorage.setItem(k, v);
    else window.localStorage.removeItem(k);
  });
});

const originalMatchMedia = window.matchMedia;

afterEach(() => {
  window.matchMedia = originalMatchMedia;
});

describe('useTheme 主题状态', () => {
  test('默认深色；切换后同时更新 state、localStorage 与 <html data-theme>', () => {
    const { result } = renderHook(() => useTheme(), { wrapper: ThemeProvider });
    expect(result.current.theme).toBe('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');

    act(() => result.current.setTheme('eye-care'));

    expect(result.current.theme).toBe('eye-care');
    expect(window.localStorage.getItem('theme')).toBe('eye-care');
    expect(document.documentElement.getAttribute('data-theme')).toBe('eye-care');
  });

  test('跟随系统：按 prefers-color-scheme 解析，且不写服务端设置', () => {
    window.matchMedia = jest.fn().mockReturnValue({
      matches: true,
      addEventListener: jest.fn(),
      removeEventListener: jest.fn(),
    });

    const { result } = renderHook(() => useTheme(), { wrapper: ThemeProvider });
    act(() => result.current.setTheme('system'));

    expect(result.current.theme).toBe('system');
    expect(result.current.resolvedTheme).toBe('light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
    expect(api.updateSettings).not.toHaveBeenCalled();
  });

  test('非法取值回退为深色，避免把脏 localStorage 原样写进 data-theme', () => {
    window.localStorage.setItem('theme', 'neon');
    const { result } = renderHook(() => useTheme(), { wrapper: ThemeProvider });
    expect(result.current.theme).toBe('dark');
  });
});
