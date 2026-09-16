import React from 'react';
import { render, screen, waitFor, act, fireEvent } from '@testing-library/react';
import { MemoryRouter, Routes, Route, useNavigate } from 'react-router-dom';
import Reader from '../pages/Reader';
import { ToastProvider } from '../components/Toast';
import { SettingsProvider } from '../hooks/useSettings';

jest.mock('../utils/api');
const api = require('../utils/api').default;

// jsdom 没有 ResizeObserver，Reader 挂载即 new ResizeObserver → 空实现顶替
class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}
global.ResizeObserver = ResizeObserverMock;

function makePages(n) {
  return Array.from({ length: n }, (_, i) => ({
    id: i + 1,
    url: `/api/archives/1/pages/${i}`,
    thumb_url: `/api/archives/1/pages/${i}/thumb`,
    filename: `page-${i + 1}.jpg`,
  }));
}

function renderReader(readPage = 0, pageCount = 6) {
  api.getPages.mockResolvedValue({
    archive: { id: 1, title: '测试漫画', archive_type: 'folder', group_id: null },
    pages: makePages(pageCount),
    read_page: readPage,
  });
  return render(
    <SettingsProvider>
      <ToastProvider>
        <MemoryRouter initialEntries={['/reader/1']}>
          <Routes>
            <Route path="/reader/:archiveId" element={<Reader />} />
          </Routes>
        </MemoryRouter>
      </ToastProvider>
    </SettingsProvider>
  );
}

const pressKey = (key) => {
  act(() => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key }));
  });
};

describe('Reader 双页模式', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getBookmarks.mockResolvedValue({ pages: [] });
    api.saveHistory.mockResolvedValue({});
    api.updateSettings.mockResolvedValue({});
    api.getGroupChapters.mockResolvedValue([]);
  });

  test('开启双页后一次显示两张跨页图（不再回退到单页布局）', async () => {
    const { container } = renderReader();
    await waitFor(() => {
      expect(screen.getByRole('region', { name: /页面阅读区/ })).toBeInTheDocument();
    });

    await act(async () => {
      fireEvent.click(screen.getByLabelText('启用双页模式'));
    });

    // RTL：右页=当前页(page-1)，左页=下一页(page-2)
    expect(screen.getByAltText('page-1.jpg')).toBeInTheDocument();
    expect(screen.getByAltText('page-2.jpg')).toBeInTheDocument();
    expect(container.querySelector('.reader-page-wrapper')).toBeNull(); // 不是单页布局
  });

  test('双页模式翻页仍保持双页布局，步进 2 页', async () => {
    const { container } = renderReader();
    await waitFor(() => {
      expect(screen.getByRole('region', { name: /页面阅读区/ })).toBeInTheDocument();
    });
    await act(async () => {
      fireEvent.click(screen.getByLabelText('启用双页模式'));
    });

    pressKey('ArrowRight'); // 双页 RTL → currentIndex += 2 → 跨页 {page-3, page-4}

    expect(screen.getByAltText('page-3.jpg')).toBeInTheDocument();
    expect(screen.getByAltText('page-4.jpg')).toBeInTheDocument();
    expect(container.querySelector('.reader-page-wrapper')).toBeNull();
  });

  test('末页缺一张时保留双页布局（空位占位），不掉回单页造成布局切换闪烁', async () => {
    const { container } = renderReader();
    await waitFor(() => {
      expect(screen.getByRole('region', { name: /页面阅读区/ })).toBeInTheDocument();
    });
    await act(async () => {
      fireEvent.click(screen.getByLabelText('启用双页模式'));
    });

    pressKey('End'); // 跳到最后一页（index 5），RTL 下仅右页存在

    const lone = screen.getByAltText('page-6.jpg');
    const row = lone.parentElement;
    // 双页 flex 行：1 张图 + 1 个末页空位占位（aria-hidden），不存在第二张图
    expect(row.querySelectorAll('img').length).toBe(1);
    expect(row.querySelector('div[aria-hidden="true"]')).not.toBeNull();
    expect(container.querySelector('.reader-page-wrapper')).toBeNull(); // 未回退到单页
  });

  test('单页模式：末页继续翻环回第一页，首页往回翻环回末页', async () => {
    renderReader();
    await waitFor(() => {
      expect(screen.getByRole('region', { name: /页面阅读区/ })).toBeInTheDocument();
    });

    pressKey('End'); // index 5
    expect(screen.getByAltText('page-6.jpg')).toBeInTheDocument();

    pressKey('ArrowRight'); // 末页继续 → 环回本册第一页
    expect(screen.getByAltText('page-1.jpg')).toBeInTheDocument();

    pressKey('ArrowLeft'); // 第一页往回 → 环回本册末页
    expect(screen.getByAltText('page-6.jpg')).toBeInTheDocument();
  });

  test('双页模式：末页继续翻环回第一跨页，首页往回翻环回末跨页', async () => {
    renderReader();
    await waitFor(() => {
      expect(screen.getByRole('region', { name: /页面阅读区/ })).toBeInTheDocument();
    });
    await act(async () => {
      fireEvent.click(screen.getByLabelText('启用双页模式'));
    });

    pressKey('End'); // index 5（末页单张，RTL 右=page-6）
    expect(screen.getByAltText('page-6.jpg')).toBeInTheDocument();

    pressKey('ArrowRight'); // 末页继续 → 环回第一跨页 (0,1)
    expect(screen.getByAltText('page-1.jpg')).toBeInTheDocument();
    expect(screen.getByAltText('page-2.jpg')).toBeInTheDocument();

    pressKey('ArrowLeft'); // 第一跨页往回 → 环回末跨页（index 4 → 右=page-5，左=page-6）
    expect(screen.getByAltText('page-5.jpg')).toBeInTheDocument();
    expect(screen.getByAltText('page-6.jpg')).toBeInTheDocument();
  });

  test('切换档案：旧档案进度先落盘，且不把旧页码写进新档案（回归：同路由换档损坏进度）', async () => {
    jest.useFakeTimers();
    // 每个档案独立返回（id 随路由变化）
    api.getPages.mockImplementation((id) => Promise.resolve({
      archive: { id: Number(id), title: `档案${id}`, archive_type: 'folder', group_id: null },
      pages: makePages(6),
      read_page: 0,
    }));

    function GoButton({ to, label }) {
      const navigate = useNavigate();
      return <button onClick={() => navigate(to)}>{label}</button>;
    }

    render(
      <SettingsProvider>
        <ToastProvider>
          <MemoryRouter initialEntries={['/reader/1']}>
            <Routes>
              <Route path="/reader/:archiveId" element={(
                <>
                  <GoButton to="/reader/2" label="切到档案2" />
                  <Reader />
                </>
              )} />
            </Routes>
          </MemoryRouter>
        </ToastProvider>
      </SettingsProvider>
    );

    await waitFor(() => {
      expect(screen.getByRole('region', { name: /页面阅读区/ })).toBeInTheDocument();
    });

    // 档案 1 翻 3 页到 index 3（第 4 页），防抖尚未触发
    pressKey('ArrowRight');
    pressKey('ArrowRight');
    pressKey('ArrowRight');

    // 立刻切到档案 2：旧档案进度应立即落盘（flush），而不是 1s 后才被防抖覆盖
    await act(async () => {
      fireEvent.click(screen.getByText('切到档案2'));
    });
    await act(async () => {
      jest.advanceTimersByTime(1000); // 让新档案加载 + 防抖落定
    });

    const calls = api.saveHistory.mock.calls.map(c => c.slice(0, 3));
    expect(calls).toContainEqual([1, 3, 6]);     // 旧档案 1 的最后位置已保存
    expect(calls).not.toContainEqual([2, 3, 6]); // 旧页码绝不能写进新档案 2
    expect(calls).toContainEqual([2, 0, 6]);     // 新档案 2 正常保存自己的（首页）进度

    jest.useRealTimers();
  });

  test('组主档案（group_id===id）显示章节列表而非阅读器', async () => {
    // 与后端 /pages 响应一致：archive 包含 group_id（此前 mock 带而真实响应缺，
    // 掩盖了“组主档案章节列表永不触发”的缺陷；后端已修复，这里做回归保护）
    api.getPages.mockResolvedValue({
      archive: { id: 5, title: '组测试', archive_type: 'folder', group_id: 5 },
      pages: makePages(3),
      read_page: 0,
    });
    api.getGroupChapters.mockResolvedValue([
      { id: 5, title: '组测试', page_count: 3, read_page: 1, archive_type: 'folder' },
      { id: 6, title: '第2话', page_count: 4, read_page: 0, archive_type: 'folder' },
    ]);

    render(
      <SettingsProvider>
        <ToastProvider>
          <MemoryRouter initialEntries={['/reader/5']}>
            <Routes>
              <Route path="/reader/:archiveId" element={<Reader />} />
            </Routes>
          </MemoryRouter>
        </ToastProvider>
      </SettingsProvider>
    );
    await waitFor(() => {
      expect(screen.getByText(/2 话/)).toBeInTheDocument(); // 章节列表头部
    });
    expect(screen.getByText('第2话')).toBeInTheDocument();
    expect(screen.queryByRole('region', { name: /页面阅读区/ })).toBeNull(); // 不是阅读器
  });
});