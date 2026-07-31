import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import Library from '../pages/Library';
import { ToastProvider } from '../components/Toast';
import { SettingsProvider } from '../hooks/useSettings';
import { TagsProvider } from '../hooks/useTags';

jest.mock('../utils/api');
const api = require('../utils/api').default;

function renderLibrary() {
  return render(
    <MemoryRouter>
      <SettingsProvider>
        <TagsProvider>
          <ToastProvider>
            <Library />
          </ToastProvider>
        </TagsProvider>
      </SettingsProvider>
    </MemoryRouter>
  );
}

describe('Library 页面', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getArchives.mockResolvedValue([
      { id: 1, title: '测试漫画', archive_type: 'folder', page_count: 10, cover_url: '/api/archives/1/cover', tags: [] },
    ]);
    api.getTags.mockResolvedValue([]);
  });

  test('显示欢迎界面当无漫画', async () => {
    api.getArchives.mockResolvedValue([]);
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText(/欢迎使用 MangaViewer/)).toBeInTheDocument();
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
});
