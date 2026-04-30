import React from 'react';
import { render, screen, waitFor, act } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import History from '../pages/History';
import { ToastProvider } from '../components/Toast';

jest.mock('../utils/api');
const api = require('../utils/api').default;

function renderHistory() {
  return render(
    <MemoryRouter>
      <ToastProvider>
        <History />
      </ToastProvider>
    </MemoryRouter>
  );
}

describe('History 页面', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getHistory.mockResolvedValue([
      {
        archive_id: 1,
        title: '测试漫画',
        page_index: 5,
        total_pages: 10,
        page_count: 10,
        archive_type: 'folder',
        cover_url: '/api/archives/1/cover',
        updated_at: new Date().toISOString(),
        tags: [{ namespace: '', name: '已读', color: '#4a86e8' }],
      },
    ]);
  });

  test('加载并显示历史记录', async () => {
    renderHistory();
    await waitFor(() => {
      expect(screen.getByText('测试漫画')).toBeInTheDocument();
    });
  });

  test('显示阅读进度', async () => {
    renderHistory();
    await waitFor(() => {
      expect(screen.getByText(/第 6\/10 页/)).toBeInTheDocument();
    });
  });

  test('显示标签', async () => {
    renderHistory();
    await waitFor(() => {
      expect(screen.getByText('已读')).toBeInTheDocument();
    });
  });

  test('空历史显示提示', async () => {
    api.getHistory.mockResolvedValue([]);
    renderHistory();
    await waitFor(() => {
      expect(screen.getByText('暂无阅读记录')).toBeInTheDocument();
    });
  });

  test('删除按钮存在', async () => {
    renderHistory();
    await waitFor(() => {
      expect(screen.getByText('测试漫画')).toBeInTheDocument();
    });
    expect(screen.getByText('删除')).toBeInTheDocument();
  });
});
