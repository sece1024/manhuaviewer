import React from 'react';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter, Routes, Route } from 'react-router-dom';
import Library from '../pages/Library';
import { ToastProvider } from '../components/Toast';
import { SettingsProvider } from '../hooks/useSettings';
import { TagsProvider } from '../hooks/useTags';

jest.mock('../utils/api');
const api = require('../utils/api').default;

function renderLibrary() {
  return render(
    <MemoryRouter>
      <Routes>
        <Route path="/" element={
          <SettingsProvider>
            <TagsProvider>
              <ToastProvider>
                <Library />
              </ToastProvider>
            </TagsProvider>
          </SettingsProvider>
        } />
        <Route path="/reader/:id" element={<div>READER_PAGE</div>} />
      </Routes>
    </MemoryRouter>
  );
}

describe('Library 页面', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getCategories.mockResolvedValue([]);
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

describe('Library 同标题自动合并', () => {
  const groupArchives = [
    { id: 1, title: '海贼王', path: '/manhua/海贼王/01', archive_type: 'folder', page_count: 10, cover_url: '/api/archives/1/cover', tags: [] },
    { id: 2, title: '海贼王', path: '/manhua/海贼王/02', archive_type: 'folder', page_count: 12, cover_url: '/api/archives/2/cover', tags: [] },
  ];

  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getCategories.mockResolvedValue([]);
    api.getTags.mockResolvedValue([]);
  });

  test('同标题且同父目录时只渲染一张组卡片', async () => {
    api.getArchives.mockResolvedValue(groupArchives);
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText(/海贼王/)).toBeInTheDocument();
    });
    expect(screen.getAllByText(/海贼王/)).toHaveLength(1);
    expect(screen.getByText('2 话')).toBeInTheDocument();
  });

  test('标题相同但父目录不同时不合并', async () => {
    api.getArchives.mockResolvedValue([
      { ...groupArchives[0] },
      { ...groupArchives[1], path: '/other/海贼王/02' },
    ]);
    renderLibrary();
    await waitFor(() => {
      expect(screen.getAllByText('海贼王')).toHaveLength(2);
    });
  });

  test('标题不同时不合并', async () => {
    api.getArchives.mockResolvedValue([
      { ...groupArchives[0] },
      { ...groupArchives[1], title: '火影忍者', path: '/manhua/火影忍者/01' },
    ]);
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText('海贼王')).toBeInTheDocument();
      expect(screen.getByText('火影忍者')).toBeInTheDocument();
    });
  });

  test('点击组卡片就地展开子目录名称', async () => {
    api.getArchives.mockResolvedValue(groupArchives);
    api.getArchivesByTitle.mockResolvedValue(groupArchives);
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText(/海贼王/)).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText(/海贼王/));

    await waitFor(() => {
      expect(api.getArchivesByTitle).toHaveBeenCalledWith('海贼王', '/manhua/海贼王');
      expect(screen.getByText('01')).toBeInTheDocument();
      expect(screen.getByText('02')).toBeInTheDocument();
    });
  });

  test('点击展开后的章节进入阅读器', async () => {
    api.getArchives.mockResolvedValue(groupArchives);
    api.getArchivesByTitle.mockResolvedValue(groupArchives);
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText(/海贼王/)).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText(/海贼王/));
    await waitFor(() => {
      expect(screen.getByText('01')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('01'));
    await waitFor(() => {
      expect(screen.getByText('READER_PAGE')).toBeInTheDocument();
    });
  });
});
