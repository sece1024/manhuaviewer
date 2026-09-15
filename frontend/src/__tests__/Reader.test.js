import React from 'react';
import { render, screen, waitFor, act, fireEvent } from '@testing-library/react';
import { MemoryRouter, Routes, Route } from 'react-router-dom';
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
});