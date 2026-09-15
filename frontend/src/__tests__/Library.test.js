import React from 'react';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter, Routes, Route, useNavigate } from 'react-router-dom';
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
  const groupMembers = [
    { id: 1, title: '海贼王', path: '/manhua/海贼王/01', archive_type: 'folder', page_count: 10, cover_url: '/api/archives/1/cover', tags: [] },
    { id: 2, title: '海贼王', path: '/manhua/海贼王/02', archive_type: 'folder', page_count: 12, cover_url: '/api/archives/2/cover', tags: [] },
  ];
  const groupItem = {
    ...groupMembers[0],
    _isGroup: true,
    chapter_count: 2,
    _autoGroup: true,
    _autoKey: 'KEY',
    _parentDir: '/manhua/海贼王',
  };

  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getCategories.mockResolvedValue([]);
    api.getTags.mockResolvedValue([]);
  });

  test('同标题且同父目录时只渲染一张组卡片', async () => {
    api.getArchives.mockResolvedValue([groupItem]);
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText(/海贼王/)).toBeInTheDocument();
    });
    expect(screen.getAllByText(/海贼王/)).toHaveLength(1);
    expect(screen.getByText('2 话')).toBeInTheDocument();
  });

  test('标题相同但父目录不同时不合并', async () => {
    api.getArchives.mockResolvedValue([
      { ...groupMembers[0] },
      { ...groupMembers[1], path: '/other/海贼王/02' },
    ]);
    renderLibrary();
    await waitFor(() => {
      expect(screen.getAllByText('海贼王')).toHaveLength(2);
    });
  });

  test('标题不同时不合并', async () => {
    api.getArchives.mockResolvedValue([
      { ...groupMembers[0] },
      { ...groupMembers[1], title: '火影忍者', path: '/manhua/火影忍者/01' },
    ]);
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText('海贼王')).toBeInTheDocument();
      expect(screen.getByText('火影忍者')).toBeInTheDocument();
    });
  });

  test('点击组卡片就地展开子目录名称', async () => {
    api.getArchives.mockResolvedValue([groupItem]);
    api.getArchivesByTitle.mockResolvedValue(groupMembers);
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
    api.getArchives.mockResolvedValue([groupItem]);
    api.getArchivesByTitle.mockResolvedValue(groupMembers);
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

describe('Library 统一书库（合并原“漫画库/文件夹”双 tab）', () => {
  const folder = { id: 1, title: '文件夹漫画', archive_type: 'folder', page_count: 10, cover_url: '/api/archives/1/cover', tags: [] };
  const cbz = { id: 2, title: '压缩包漫画', archive_type: 'cbz', page_count: 20, cover_url: '/api/archives/2/cover', tags: [] };

  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getCategories.mockResolvedValue([]);
    api.getTags.mockResolvedValue([]);
    api.getArchives.mockResolvedValue([folder, cbz]);
  });

  test('默认“全部类型”同时展示文件夹与压缩包档案', async () => {
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText('文件夹漫画')).toBeInTheDocument();
    });
    // 关键回归：类型不再是顶层导航切分，两类档案在同一列表共存
    expect(screen.getByText('压缩包漫画')).toBeInTheDocument();
  });

  test('类型筛选可收窄为仅文件夹 / 仅压缩包', async () => {
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText('文件夹漫画')).toBeInTheDocument();
    });

    fireEvent.change(screen.getByLabelText('档案类型'), { target: { value: 'folder' } });
    await waitFor(() => {
      expect(screen.queryByText('压缩包漫画')).toBeNull();
    });
    expect(screen.getByText('文件夹漫画')).toBeInTheDocument();
    expect(api.updateSettings).toHaveBeenCalledWith({ type_filter: 'folder' });

    fireEvent.change(screen.getByLabelText('档案类型'), { target: { value: 'archive' } });
    await waitFor(() => {
      expect(screen.queryByText('文件夹漫画')).toBeNull();
    });
    expect(screen.getByText('压缩包漫画')).toBeInTheDocument();
  });
});

describe('Library 会话恢复（从阅读器返回不丢翻页位置）', () => {
  // 两页数据：第 2 页的档案与第 1 页不同（模拟“翻页后进入漫画再退出”）
  const page1 = Array.from({ length: 50 }, (_, i) => ({
    id: i + 1, title: `第1页漫画${i + 1}`, archive_type: 'folder', page_count: 10,
    cover_url: `/api/archives/${i + 1}/cover`, tags: [],
  }));
  const page2 = Array.from({ length: 20 }, (_, i) => ({
    id: 100 + i, title: `第2页漫画${i + 1}`, archive_type: 'folder', page_count: 10,
    cover_url: `/api/archives/${100 + i}/cover`, tags: [],
  }));

  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getCategories.mockResolvedValue([]);
    api.getTags.mockResolvedValue([]);
  });

  function renderWithSession() {
    function ReaderStub() {
      const navigate = useNavigate();
      return <button onClick={() => navigate('/')}>返回书库</button>;
    }
    return render(
      <MemoryRouter initialEntries={['/']}>
        <Routes>
          <Route path="/" element={
            <SettingsProvider>
              <TagsProvider>
                <ToastProvider>
                  <Library enableSession />
                </ToastProvider>
              </TagsProvider>
            </SettingsProvider>
          } />
          <Route path="/reader/:id" element={<ReaderStub />} />
        </Routes>
      </MemoryRouter>
    );
  }

  test('翻到第 2 页后进入阅读器再返回：保留已加载分页与位置（顺序变化不回顶）', async () => {
    const all = [...page1, ...page2];
    api.getArchives.mockImplementation((params = {}) => {
      const page = Number(params.page || 1);
      const limit = Number(params.limit || 50);
      if (page === 2) return Promise.resolve(page2);
      // 真实服务端按 limit 返回；比对请求（limit=已加载条数）顺序打乱但成员不变，
      // 模拟“刚读完的漫画在最近阅读排序里前移”
      const shuffled = [all[5], ...all.slice(0, 5), ...all.slice(6)];
      return Promise.resolve(shuffled.slice(0, limit));
    });

    renderWithSession();
    await waitFor(() => {
      expect(screen.getByText('第1页漫画1')).toBeInTheDocument();
    });

    // 手动加载第 2 页（触底自动加载在测试环境不触发）
    fireEvent.click(screen.getByText(/加载更多/));
    await waitFor(() => {
      expect(screen.getByText('第2页漫画1')).toBeInTheDocument();
    });

    // 进入阅读器（Library 卸载 → 写入浏览会话）
    fireEvent.click(screen.getByText('第2页漫画1'));
    await waitFor(() => {
      expect(screen.getByText('返回书库')).toBeInTheDocument();
    });

    // 返回书库：恢复会话 + 后台一致性比对
    fireEvent.click(screen.getByText('返回书库'));

    await waitFor(() => {
      // 关键断言：仍保留第 2 页已加载内容，而不是被踢回第一页
      expect(screen.getByText('第2页漫画1')).toBeInTheDocument();
    });
    expect(screen.getByText('第1页漫画1')).toBeInTheDocument();
  });
});
describe('Library 卡片密度', () => {
  const tagged = {
    id: 1,
    title: '测试漫画',
    archive_type: 'folder',
    page_count: 10,
    cover_url: '/api/archives/1/cover',
    tags: [{ name: '日常', color: '#4a86e8', namespace: '' }],
  };

  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getCategories.mockResolvedValue([]);
    api.getArchives.mockResolvedValue([tagged]);
    api.getTags.mockResolvedValue([]);
  });

  test('切换紧凑密度：容器加 density-compact，标签折叠为色点', async () => {
    const { container } = renderLibrary();
    await waitFor(() => {
      expect(screen.getByText('测试漫画')).toBeInTheDocument();
    });
    // 标准网格：文字标签可见
    expect(container.querySelector('.archive-grid')).toBeTruthy();
    expect(container.querySelector('.archive-card-tags')).toBeTruthy();

    fireEvent.click(screen.getByLabelText('紧凑封面'));

    await waitFor(() => {
      expect(container.querySelector('.archive-grid.density-compact')).toBeTruthy();
    });
    expect(container.querySelector('.archive-card-tags')).toBeNull();
    expect(container.querySelector('.archive-card-tagdots')).toBeTruthy();
    expect(api.updateSettings).toHaveBeenCalledWith({ card_density: 'compact' });
  });

  test('切换大封面并持久化', async () => {
    const { container } = renderLibrary();
    await waitFor(() => {
      expect(screen.getByText('测试漫画')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByLabelText('大封面'));

    await waitFor(() => {
      expect(container.querySelector('.archive-grid.density-large')).toBeTruthy();
    });
    expect(api.updateSettings).toHaveBeenCalledWith({ card_density: 'large' });
  });

  test('密度按钮仅在网格视图显示', async () => {
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText('测试漫画')).toBeInTheDocument();
    });
    expect(screen.getByLabelText('紧凑封面')).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText('列表视图'));
    await waitFor(() => {
      expect(screen.queryByLabelText('紧凑封面')).toBeNull();
    });
  });
});
