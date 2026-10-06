import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import App from '../App';

jest.mock('../utils/api');
const apiModule = require('../utils/api');
const api = apiModule.default;

// App 侧边栏收起状态存 localStorage（与主题一样是纯客户端偏好）：automock 会把
// localStorageGet/Set 变成 no-op，这里换回真实实现，否则测不到"跨会话记忆"。
const realLocalStorageGet = (k) => {
  try { return window.localStorage.getItem(k) || ''; } catch (e) { return ''; }
};
const realLocalStorageSet = (k, v) => {
  try { if (v) window.localStorage.setItem(k, v); else window.localStorage.removeItem(k); } catch (e) { /* 忽略 */ }
};

describe('App 侧边栏收起', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    window.localStorage.clear();
    apiModule.localStorageGet.mockImplementation(realLocalStorageGet);
    apiModule.localStorageSet.mockImplementation(realLocalStorageSet);
    // automock：默认所有接口返回空数组，避免某个 .then 拿到 undefined
    Object.values(api).forEach(fn => { fn.mockResolvedValue([]); });
    api.getSettings.mockResolvedValue({});
    api.getLanIps.mockResolvedValue({ ipv4: [], port: 5002 });
    api.getArchives.mockResolvedValue([]);
    api.getCategories.mockResolvedValue([]);
    api.getTags.mockResolvedValue([]);
  });

  test('点击按钮在“展开 ↔ 收起窄栏”之间切换，并记住选择', async () => {
    const { container } = render(<App />);
    const layout = () => container.querySelector('.app-layout');

    await waitFor(() => {
      expect(screen.getByRole('button', { name: '收起侧边栏' })).toBeInTheDocument();
    });
    expect(layout().className).not.toContain('sidebar-collapsed');

    fireEvent.click(screen.getByRole('button', { name: '收起侧边栏' }));
    expect(layout().className).toContain('sidebar-collapsed');
    expect(window.localStorage.getItem('sidebar_collapsed')).toBe('1');

    // 收起后按钮变成“展开”，再点一次回到展开态
    fireEvent.click(screen.getByRole('button', { name: '展开侧边栏' }));
    expect(layout().className).not.toContain('sidebar-collapsed');
    expect(window.localStorage.getItem('sidebar_collapsed')).toBe('0');
  });

  test('收起状态跨会话记忆：重新打开直接是收起态', async () => {
    window.localStorage.setItem('sidebar_collapsed', '1');
    const { container } = render(<App />);
    await waitFor(() => {
      expect(container.querySelector('.app-layout')).toBeInTheDocument();
    });
    expect(container.querySelector('.app-layout').className).toContain('sidebar-collapsed');
    expect(screen.getByRole('button', { name: '展开侧边栏' })).toBeInTheDocument();
  });

  test('阅读器路由整条侧边栏隐藏（沉浸阅读），且不显示收起按钮', async () => {
    window.history.pushState({}, '', '/reader/1');
    const { container } = render(<App />);
    await waitFor(() => {
      expect(container.querySelector('.app-layout').className).toContain('reader-immersive');
    });
    // 侧边栏仍在 DOM 中，由 .reader-immersive .sidebar { display: none } 隐藏
    expect(container.querySelector('.app-layout .sidebar')).not.toBeNull();
    window.history.pushState({}, '', '/');
  });
});
