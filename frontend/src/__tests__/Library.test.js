import React from 'react';
import { render, screen, waitFor, act } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import Library from '../pages/Library';
import { ToastProvider } from '../components/Toast';

jest.mock('../utils/api');
const api = require('../utils/api').default;

function renderLibrary() {
  return render(
    <MemoryRouter>
      <ToastProvider>
        <Library />
      </ToastProvider>
    </MemoryRouter>
  );
}

describe('Library 页面', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getConfig.mockResolvedValue({ root_dir: '/test/manga' });
    api.getArchives.mockResolvedValue([
      { id: 1, title: '测试漫画', archive_type: 'folder', page_count: 10, cover_url: '/api/archives/1/cover', tags: [] },
    ]);
    api.getTags.mockResolvedValue([]);
  });

  test('显示欢迎界面当无根目录', async () => {
    api.getConfig.mockResolvedValue({ root_dir: '' });
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText(/请先配置漫画存放的根目录/)).toBeInTheDocument();
    });
  });

  test('加载并显示漫画列表', async () => {
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText('测试漫画')).toBeInTheDocument();
    });
    expect(api.getArchives).toHaveBeenCalled();
  });

  test('搜索输入框存在', async () => {
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText('测试漫画')).toBeInTheDocument();
    });
    expect(screen.getByPlaceholderText(/搜索/)).toBeInTheDocument();
  });

  test('扫描按钮触发扫描', async () => {
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText('测试漫画')).toBeInTheDocument();
    });
    const scanBtn = screen.getByRole('button', { name: /扫描/ });
    act(() => {
      scanBtn.click();
    });
    await waitFor(() => {
      expect(api.scan).toHaveBeenCalled();
    });
  });
});
