import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import Settings from '../pages/Settings';
import { ToastProvider } from '../components/Toast';

jest.mock('../utils/api');
const api = require('../utils/api').default;

function renderSettings() {
  return render(
    <MemoryRouter>
      <ToastProvider>
        <Settings />
      </ToastProvider>
    </MemoryRouter>
  );
}

describe('Settings 页面', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getConfig.mockResolvedValue({ root_dir: '/test/manga' });
    api.getSettings.mockResolvedValue({ page_direction: 'rtl', reader_fit: 'height', theme: 'dark' });
    api.getStats.mockResolvedValue({ archives: 10, tags: 5, categories: 3, history: 20, total_pages: 500, total_size: 1024000 });
    api.getTags.mockResolvedValue([
      { id: 1, namespace: 'artist', name: '测试作者', color: '#ff0000', full_name: 'artist:测试作者', archive_count: 3 },
    ]);
    api.getCategories.mockResolvedValue([
      { id: 1, name: '动作', color: '#00ff00', pinned: 0, archive_count: 5 },
    ]);
  });

  test('加载并显示统计数据', async () => {
    renderSettings();
    await waitFor(() => {
      expect(screen.getByText('10')).toBeInTheDocument();
    });
  });

  test('显示设置区域标题', async () => {
    renderSettings();
    await waitFor(() => {
      expect(screen.getByText('📂 目录')).toBeInTheDocument();
    });
  });

  test('显示标签列表', async () => {
    renderSettings();
    await waitFor(() => {
      expect(screen.getByText('artist:测试作者')).toBeInTheDocument();
    });
  });

  test('显示分类列表', async () => {
    renderSettings();
    await waitFor(() => {
      expect(screen.getByText('动作')).toBeInTheDocument();
    });
  });
});
