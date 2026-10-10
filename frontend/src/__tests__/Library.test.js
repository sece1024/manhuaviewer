import { render, screen, waitFor, fireEvent, act, renderHook, within } from '@testing-library/react';
import { MemoryRouter, Routes, Route, useNavigate } from 'react-router-dom';
import Library from '../pages/Library';
import { clearLibrarySessions } from '../hooks/useLibrarySession';
import { resetJobsStore } from '../hooks/useJobs';
import useJobs from '../hooks/useJobs';
import { ToastProvider } from '../components/Toast';
import { SettingsProvider } from '../hooks/useSettings';
import { TagsProvider } from '../hooks/useTags';

jest.mock('../utils/api');
const api = require('../utils/api').default;
const { membershipGeneration } = require('../utils/api');

// 任务层是模块级单例，会跨用例存活（例如"批量转 CBZ"用例会把转换任务留在跑），
// 每个用例都从干净的任务状态开始
beforeEach(() => {
  resetJobsStore();
});

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

  test('卡片元信息显示添加时间（created_at 按本地时区展示）', async () => {
    api.getArchives.mockResolvedValue([
      { id: 5, title: '带时间的漫画', archive_type: 'cbz', page_count: 10, cover_url: '/api/archives/5/cover', tags: [], created_at: '2026-04-30 12:00:00' },
    ]);
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText('带时间的漫画')).toBeInTheDocument();
    });
    // 期望日期 = 同一 UTC 时刻按运行机时区格式化，断言与时区无关
    const d = new Date('2026-04-30T12:00:00Z');
    const pad = n => String(n).padStart(2, '0');
    const expected = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
    expect(screen.getByText(`· ${expected}`)).toBeInTheDocument();
  });

  test('无 created_at 的旧数据不渲染日期（兼容旧接口载荷）', async () => {
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText('测试漫画')).toBeInTheDocument();
    });
    expect(screen.queryByText(/· \d{4}-\d{2}-\d{2}/)).not.toBeInTheDocument();
  });

  test('日期树：展开月份并点选后按添加日期过滤', async () => {
    api.getAddedTree.mockResolvedValue({
      years: [{ year: 2026, count: 3, months: [{ month: 4, count: 2 }, { month: 5, count: 1 }] }],
    });
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText('2026年')).toBeInTheDocument();
    });
    // 默认自动展开最新一年 → 点 4月 → 请求带上本地日期边界（from 含、to 不含）
    fireEvent.click(await screen.findByText('4月'));
    await waitFor(() => {
      expect(api.getArchives).toHaveBeenCalledWith(
        expect.objectContaining({ added_from: '2026-04-01', added_to: '2026-05-01' })
      );
    });
    // 再点同一节点 → 取消过滤 → 请求不再带日期参数
    fireEvent.click(screen.getByText('4月'));
    await waitFor(() => {
      expect(api.getArchives).toHaveBeenCalledWith(
        expect.not.objectContaining({ added_from: expect.anything() })
      );
    });
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

  /// 回归：手动删除磁盘档案后扫描清理，回到书库不得再显示已删除的漫画名。
  /// 此前浏览会话是模块级缓存，扫描（写操作）不会作废它，返回书库时会先用旧列表
  /// 秒开——已删档案名会重新出现，比对失败或中途切页时还会被再次写回。
  test('扫描（写操作）后返回书库：不再显示已删除的漫画', async () => {
    const survivors = page1; // 扫描后服务端只剩第 1 页这批
    // 首次进入：服务端返回含“待删除漫画”的列表
    const withDeleted = [
      { id: 999, title: '待删除漫画', archive_type: 'folder', page_count: 10, cover_url: '/api/archives/999/cover', tags: [] },
      ...survivors,
    ];
    api.getArchives.mockResolvedValue(withDeleted);

    // 用真实 api.js 的成员代际（automock 会把它变成 jest.fn() 而失去语义），
    // 并清掉上一个用例遗留的会话缓存，保证本次从干净状态开始。
    const realGeneration = jest.requireActual('../utils/api').membershipGeneration;
    membershipGeneration.mockImplementation(() => realGeneration());
    clearLibrarySessions();

    renderWithSession();
    await waitFor(() => {
      expect(screen.getByText('待删除漫画')).toBeInTheDocument();
    });

    // 进入阅读器 → Library 卸载，当前列表（含待删除漫画）写入浏览会话
    fireEvent.click(screen.getByText('待删除漫画'));
    await waitFor(() => {
      expect(screen.getByText('返回书库')).toBeInTheDocument();
    });

    // 扫描清理（模拟真实流程：扫描是写操作 → 成员代际递增 → 浏览会话失效）
    const realInvalidate = jest.requireActual('../utils/api').invalidateLibrarySessions;
    realInvalidate();
    await api.scan();

    // 返回书库：服务端已无该档案
    api.getArchives.mockResolvedValue(survivors);
    fireEvent.click(screen.getByText('返回书库'));

    await waitFor(() => {
      expect(screen.getByText('第1页漫画1')).toBeInTheDocument();
    });
    expect(screen.queryByText('待删除漫画')).toBeNull();
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

describe('Library 批量转换为 CBZ', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getCategories.mockResolvedValue([]);
    api.getTags.mockResolvedValue([]);
    api.getArchives.mockResolvedValue([
      { id: 7, title: '待转换', archive_type: '7z', page_count: 10, cover_url: '/api/archives/7/cover', tags: [] },
    ]);
    api.convertCbzStart.mockResolvedValue({ started: true, total: 1 });
    api.convertCbzStatus.mockResolvedValue({
      running: false, total: 0, done: 0, converted: 0, skipped: 0, failed: 0, current: '', errors: [],
    });
  });

  test('多选后可将选中项转为 CBZ', async () => {
    renderLibrary();
    await waitFor(() => {
      expect(screen.getByText('待转换')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: '选择' }));
    fireEvent.click(screen.getByText('待转换'));
    fireEvent.click(screen.getByRole('button', { name: '转为 CBZ' }));
    fireEvent.click(await screen.findByRole('button', { name: '开始转换' }));

    await waitFor(() => expect(api.convertCbzStart).toHaveBeenCalledWith([7]));
  });
});

// 浏览会话（跨路由记住列表/筛选/滚动位置）。测试环境默认关闭（见 Library.js 的 IS_TEST），
// 这里显式 enableSession 打开，并清空模块级会话缓存保证用例隔离。
describe('Library 浏览会话：滚动位置', () => {
  const makeArchives = (n) => Array.from({ length: n }, (_, i) => ({
    id: i + 1,
    title: `漫画-${i + 1}`,
    archive_type: 'folder',
    page_count: 10,
    cover_url: `/api/archives/${i + 1}/cover`,
    tags: [],
  }));

  const renderWithReader = () => {
    function ReaderStub() {
      const navigate = useNavigate();
      return <button onClick={() => navigate('/')}>退出阅读器</button>;
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
  };

  beforeEach(() => {
    jest.clearAllMocks();
    clearLibrarySessions();
    api.getSettings.mockResolvedValue({});
    api.getCategories.mockResolvedValue([]);
    api.getTags.mockResolvedValue([]);
    api.getArchives.mockResolvedValue(makeArchives(60));
    api.saveHistory.mockResolvedValue({});
  });

  afterEach(() => clearLibrarySessions());

  test('进阅读器再返回，恢复原来的滚动位置（回归：卸载时读 ref 恒为 null → 存成 0）', async () => {
    const { container } = renderWithReader();
    await waitFor(() => {
      expect(screen.getByText('漫画-1')).toBeInTheDocument();
    });

    const list = container.querySelector('.library-main');
    expect(list).not.toBeNull();
    list.scrollTop = 720;
    fireEvent.scroll(list); // 滚动时镜像位置（卸载清理里已读不到 DOM）

    // 点卡片进阅读器 → Library 卸载并写入会话
    fireEvent.click(screen.getByText('漫画-1'));
    await waitFor(() => {
      expect(screen.getByText('退出阅读器')).toBeInTheDocument();
    });

    // 返回书库 → 恢复列表与滚动位置
    fireEvent.click(screen.getByText('退出阅读器'));
    await waitFor(() => {
      expect(screen.getByText('漫画-1')).toBeInTheDocument();
    });
    await waitFor(() => {
      expect(container.querySelector('.library-main').scrollTop).toBe(720);
    });
  });

  test('恢复位置后再离开书库，位置不会被写回 0', async () => {
    const { container } = renderWithReader();
    await waitFor(() => expect(screen.getByText('漫画-1')).toBeInTheDocument());
    const list = container.querySelector('.library-main');
    list.scrollTop = 480;
    fireEvent.scroll(list);

    fireEvent.click(screen.getByText('漫画-1'));
    await waitFor(() => expect(screen.getByText('退出阅读器')).toBeInTheDocument());
    fireEvent.click(screen.getByText('退出阅读器'));
    await waitFor(() => expect(container.querySelector('.library-main').scrollTop).toBe(480));

    // 再次进入阅读器再返回：位置仍是 480（程序化恢复不派发 scroll 事件也不会丢）
    fireEvent.click(screen.getByText('漫画-1'));
    await waitFor(() => expect(screen.getByText('退出阅读器')).toBeInTheDocument());
    fireEvent.click(screen.getByText('退出阅读器'));
    await waitFor(() => expect(container.querySelector('.library-main').scrollTop).toBe(480));
  });

  test('超大书库的会话快照截断到 500 条，避免整份列表常驻内存', async () => {
    // 一次返回 550 条（PAGE_SIZE=50 时 hasMore=true），模拟用户已滚了很久加载了大量条目
    api.getArchives.mockResolvedValue(makeArchives(550));

    const { container } = renderWithReader();
    await waitFor(() => expect(screen.getByText('漫画-550')).toBeInTheDocument());

    // 进阅读器（卸载 → 写会话，截断到 500）再返回
    fireEvent.click(screen.getByText('漫画-1'));
    await waitFor(() => expect(screen.getByText('退出阅读器')).toBeInTheDocument());
    fireEvent.click(screen.getByText('退出阅读器'));
    await waitFor(() => expect(screen.getByText('漫画-1')).toBeInTheDocument());

    // 恢复后：前 500 条秒开，500 之后的条目被截断（由触底哨兵按 page 续拉）
    expect(screen.getByText('漫画-500')).toBeInTheDocument();
    expect(screen.queryByText('漫画-501')).toBeNull();
    expect(screen.queryByText('漫画-550')).toBeNull();
    // 列表仍知道还有更多（hasMore 保持 true）
    expect(container.querySelector('.library-main')).not.toBeNull();
  });
});

describe('Library 阅读状态（未读 / 在读 / 已读完）', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getCategories.mockResolvedValue([]);
    api.getTags.mockResolvedValue([]);
  });

  test('卡片进度按 1 基页码显示（read_page 是 0 基索引，此前少显示一页）', async () => {
    api.getArchives.mockResolvedValue([
      { id: 1, title: '读到一半', archive_type: 'cbz', page_count: 180, read_page: 11, cover_url: '/c', tags: [] },
    ]);
    renderLibrary();
    await waitFor(() => expect(screen.getByText('读到一半')).toBeInTheDocument());
    expect(screen.getByText('· 第 12/180 页')).toBeInTheDocument();
  });

  test('翻开第 1 页就显示进度（此前 read_page=0 被当成未读、整条隐藏）', async () => {
    api.getArchives.mockResolvedValue([
      { id: 1, title: '刚翻开', archive_type: 'cbz', page_count: 20, read_page: 0, cover_url: '/c', tags: [] },
    ]);
    const { container } = renderLibrary();
    await waitFor(() => expect(screen.getByText('刚翻开')).toBeInTheDocument());
    expect(screen.getByText('· 第 1/20 页')).toBeInTheDocument();
    expect(container.querySelector('.archive-card-progress')).not.toBeNull();
  });

  test('读完的卡片标注「已读完」并改用完成态进度条', async () => {
    api.getArchives.mockResolvedValue([
      { id: 1, title: '看完了', archive_type: 'cbz', page_count: 10, read_page: 9, cover_url: '/c', tags: [] },
    ]);
    const { container } = renderLibrary();
    await waitFor(() => expect(screen.getByText('看完了')).toBeInTheDocument());
    expect(screen.getByText('· 已读完')).toBeInTheDocument();
    expect(container.querySelector('.archive-card-progress.is-finished')).not.toBeNull();
  });

  test('未读的卡片不显示进度文案', async () => {
    api.getArchives.mockResolvedValue([
      { id: 1, title: '还没看', archive_type: 'cbz', page_count: 10, cover_url: '/c', tags: [] },
    ]);
    const { container } = renderLibrary();
    await waitFor(() => expect(screen.getByText('还没看')).toBeInTheDocument());
    // 只看卡片元信息行，避免误匹配「阅读状态」下拉里的选项文案
    const meta = container.querySelector('.archive-card-meta');
    expect(meta.textContent).not.toMatch(/已读完|第 \d+\/\d+ 页/);
    expect(container.querySelector('.archive-card-progress')).toBeNull();
  });

  test('阅读状态下拉只有三个精确取值，并把选中的值传给后端 read 参数', async () => {
    api.getArchives.mockResolvedValue([
      { id: 1, title: '漫画A', archive_type: 'cbz', page_count: 10, cover_url: '/c', tags: [] },
    ]);
    renderLibrary();
    await waitFor(() => expect(screen.getByText('漫画A')).toBeInTheDocument());

    const select = screen.getByLabelText('阅读状态');
    expect(Array.from(select.options).map(o => o.textContent)).toEqual(['全部', '未读', '在读', '已读完']);
    // 旧的「已读」取值（等价于 有阅读记录）不应再出现在 UI 里
    expect(Array.from(select.options).map(o => o.value)).not.toContain('read');

    fireEvent.change(select, { target: { value: 'in_progress' } });
    await waitFor(() => {
      expect(api.getArchives).toHaveBeenCalledWith(expect.objectContaining({ read: 'in_progress' }));
    });

    fireEvent.change(select, { target: { value: 'finished' } });
    await waitFor(() => {
      expect(api.getArchives).toHaveBeenCalledWith(expect.objectContaining({ read: 'finished' }));
    });
  });
});

describe('Library 继续阅读横条', () => {
  function renderWithReader() {
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
          <Route path="/reader/:archiveId" element={
            <div>
              READER_PAGE
              <button onClick={() => window.history.back()}>退出阅读器</button>
            </div>
          } />
        </Routes>
      </MemoryRouter>
    );
  }

  const reading = {
    id: 7, title: '读到一半的书', archive_type: 'cbz', page_count: 180,
    read_page: 11, cover_url: '/api/archives/7/cover', tags: [],
  };

  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getCategories.mockResolvedValue([]);
    api.getTags.mockResolvedValue([]);
    api.getArchives.mockResolvedValue([
      { id: 1, title: '列表里的书', archive_type: 'cbz', page_count: 10, cover_url: '/c', tags: [] },
    ]);
  });

  test('展示在读项与进度，点击直接续读', async () => {
    api.getContinueReading.mockResolvedValue([reading]);
    renderWithReader();
    await waitFor(() => expect(screen.getByText('继续阅读')).toBeInTheDocument());
    // 进度按 1 基页码显示，用户能看到"读到哪了"
    expect(screen.getByText('第 12/180 页')).toBeInTheDocument();

    fireEvent.click(screen.getByTitle('继续阅读《读到一半的书》'));
    // 一律进阅读器（组卡片也直接续读在读到的那一话，而不是先展开章节）
    await waitFor(() => expect(screen.getByText('READER_PAGE')).toBeInTheDocument());
  });

  test('没有任何在读档案时不渲染横条', async () => {
    api.getContinueReading.mockResolvedValue([]);
    renderWithReader();
    await waitFor(() => expect(screen.getByText('列表里的书')).toBeInTheDocument());
    expect(screen.queryByText('继续阅读')).toBeNull();
  });

  test('接口未就绪（返回非数组）时静默隐藏，不影响书库主列表', async () => {
    api.getContinueReading.mockResolvedValue(undefined);
    renderWithReader();
    await waitFor(() => expect(screen.getByText('列表里的书')).toBeInTheDocument());
    expect(screen.queryByText('继续阅读')).toBeNull();
  });

  test('用户加了筛选后隐藏，避免与列表表达两套筛选条件', async () => {
    api.getContinueReading.mockResolvedValue([reading]);
    renderWithReader();
    await waitFor(() => expect(screen.getByText('继续阅读')).toBeInTheDocument());

    fireEvent.change(screen.getByLabelText('阅读状态'), { target: { value: 'finished' } });
    await waitFor(() => expect(screen.queryByText('继续阅读')).toBeNull());
  });

  test('「查看全部在读」把筛选切到在读', async () => {
    api.getContinueReading.mockResolvedValue([reading]);
    renderWithReader();
    await waitFor(() => expect(screen.getByText('继续阅读')).toBeInTheDocument());

    fireEvent.click(screen.getByText('查看全部在读'));
    await waitFor(() => {
      // limit=50 用于区分主列表请求与横条自身的 limit=6 请求
      expect(api.getArchives).toHaveBeenCalledWith(
        expect.objectContaining({ read: 'in_progress', limit: 50 })
      );
    });
  });
});

describe('Library 内容入口（批量入库）', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getCategories.mockResolvedValue([]);
    api.getTags.mockResolvedValue([]);
    api.getContinueReading.mockResolvedValue([]);
    api.scanStatus.mockResolvedValue({ running: false });
    api.syncStatus.mockResolvedValue({ running: false });
    api.convertCbzStatus.mockResolvedValue({ running: false });
  });

  test('书库彻底为空时引导「先把漫画加入书库」（此前只教一次打开一个）', async () => {
    api.getArchives.mockResolvedValue([]);
    renderLibrary();
    await waitFor(() => expect(screen.getByText(/先把漫画加入书库/)).toBeInTheDocument());
    expect(screen.getByText(/ZIP\/CBZ/)).toBeInTheDocument();
  });

  test('扫描结束后重拉列表，让新入库的条目直接可见', async () => {
    api.getArchives.mockResolvedValue([
      { id: 1, title: '原有漫画', archive_type: 'folder', page_count: 10, cover_url: '/c', tags: [] },
    ]);
    api.scan.mockResolvedValue({ message: '扫描完成：新增 3' });

    renderLibrary();
    await waitFor(() => expect(screen.getByText('原有漫画')).toBeInTheDocument());
    const before = api.getArchives.mock.calls.length;

    // 等价于「任务在别的页面/别的设备上被发起并结束」：任务层是唯一状态源，
    // 书库不该因为"不是我点的扫描"就继续显示旧列表
    const jobs = renderHook(() => useJobs());
    await act(async () => { await jobs.result.current.startScan('/lib', 1); });

    await waitFor(() => expect(api.getArchives.mock.calls.length).toBeGreaterThan(before));
  });
});

describe('Library 标签状态筛选（整理模式的输入集合）', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getCategories.mockResolvedValue([]);
    api.getTags.mockResolvedValue([]);
    api.getContinueReading.mockResolvedValue([]);
    api.getArchives.mockResolvedValue([
      { id: 1, title: '漫画A', archive_type: 'cbz', page_count: 10, cover_url: '/c', tags: [] },
    ]);
  });

  test('三个取值与后端 tag_state 一致，只把非「全部」的值传给后端', async () => {
    renderLibrary();
    await waitFor(() => expect(screen.getByText('漫画A')).toBeInTheDocument());

    const select = screen.getByLabelText('标签状态');
    expect(Array.from(select.options).map(o => o.value)).toEqual(['all', 'untagged', 'tagged']);
    // 默认「全部」不应往请求里塞参数，避免每条列表请求都被当成筛选
    expect(api.getArchives).toHaveBeenCalledWith(expect.not.objectContaining({ tag_state: expect.anything() }));

    fireEvent.change(select, { target: { value: 'untagged' } });
    await waitFor(() => {
      expect(api.getArchives).toHaveBeenCalledWith(expect.objectContaining({ tag_state: 'untagged' }));
    });

    fireEvent.change(select, { target: { value: 'tagged' } });
    await waitFor(() => {
      expect(api.getArchives).toHaveBeenCalledWith(expect.objectContaining({ tag_state: 'tagged' }));
    });
  });

  test('「整理」入口：切到未打标签并打开整理模式（列表与整理模式说同一件事）', async () => {
    renderLibrary();
    await waitFor(() => expect(screen.getByText('漫画A')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /整理/ }));

    await waitFor(() => expect(screen.getByRole('dialog', { name: '整理标签' })).toBeInTheDocument());
    expect(screen.getByLabelText('标签状态').value).toBe('untagged');
  });

  test('标签状态属于筛选：生效时隐藏「继续阅读」横条', async () => {
    api.getContinueReading.mockResolvedValue([
      { id: 9, title: '读到一半', page_count: 100, read_page: 10, cover_url: '/c', tags: [] },
    ]);
    renderLibrary();
    await waitFor(() => expect(screen.getByText('继续阅读')).toBeInTheDocument());

    fireEvent.change(screen.getByLabelText('标签状态'), { target: { value: 'untagged' } });
    await waitFor(() => expect(screen.queryByText('继续阅读')).toBeNull());
  });
});

// 触屏平权：卡片上的四个操作按钮此前只靠 :hover 显形（且「标签」被「分类」完全盖住），
// iPad 上完全不可达；现在提供常显的「⋯」与长按两条路径，都通向同一个操作面板。
describe('Library 卡片操作面板（触屏可达性）', () => {
  function renderWithReader() {
    return render(
      <MemoryRouter>
        <Routes>
          <Route path="/" element={
            <SettingsProvider>
              <TagsProvider>
                <ToastProvider>
                  <Library enableSession={false} />
                </ToastProvider>
              </TagsProvider>
            </SettingsProvider>
          } />
          <Route path="/reader/:archiveId" element={<div>READER_PAGE</div>} />
        </Routes>
      </MemoryRouter>
    );
  }

  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getCategories.mockResolvedValue([]);
    api.getTags.mockResolvedValue([]);
    api.getContinueReading.mockResolvedValue([]);
    api.getArchives.mockResolvedValue([
      { id: 1, title: '漫画A', archive_type: 'cbz', page_count: 10, cover_url: '/c', tags: [] },
    ]);
  });

  const cardOf = (container) => container.querySelector('.archive-card');

  test('「⋯」按钮打开操作面板（看得见的入口，不必靠长按猜）', async () => {
    const { container } = renderWithReader();
    await waitFor(() => expect(screen.getByText('漫画A')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: '更多操作：漫画A' }));

    const sheet = screen.getByRole('dialog', { name: '漫画操作' });
    expect(sheet).toBeInTheDocument();
    expect(within(sheet).getByRole('button', { name: /标签/ })).toBeInTheDocument();
    expect(within(sheet).getByRole('button', { name: /分类/ })).toBeInTheDocument();
    expect(within(sheet).getByRole('button', { name: /重命名/ })).toBeInTheDocument();
    expect(within(sheet).getByRole('button', { name: /从库中移除/ })).toBeInTheDocument();
    expect(cardOf(container)).not.toBeNull();
  });

  test('长按卡片打开面板，且随后的 click 不会把用户带进阅读器', async () => {
    const { container } = renderWithReader();
    await waitFor(() => expect(screen.getByText('漫画A')).toBeInTheDocument());
    const card = cardOf(container);

    fireEvent.touchStart(card, { touches: [{ clientX: 20, clientY: 20 }] });
    await waitFor(
      () => expect(screen.getByRole('dialog', { name: '漫画操作' })).toBeInTheDocument(),
      { timeout: 2000 }
    );

    // 长按抬手后浏览器会补一个 click：必须被吞掉，否则"弹了面板又进了阅读器"
    fireEvent.touchEnd(card);
    fireEvent.click(card);
    expect(screen.queryByText('READER_PAGE')).toBeNull();
  });

  test('滑动（手指跑远）不会弹面板——滚动列表不该被打断', async () => {
    const { container } = renderWithReader();
    await waitFor(() => expect(screen.getByText('漫画A')).toBeInTheDocument());
    const card = cardOf(container);

    fireEvent.touchStart(card, { touches: [{ clientX: 20, clientY: 20 }] });
    fireEvent.touchMove(card, { touches: [{ clientX: 20, clientY: 120 }] });
    await new Promise(r => setTimeout(r, 600));

    expect(screen.queryByRole('dialog', { name: '漫画操作' })).toBeNull();
  });

  test('普通点击仍然直接进阅读器（长按逻辑不能吃掉正常点按）', async () => {
    renderWithReader();
    await waitFor(() => expect(screen.getByText('漫画A')).toBeInTheDocument());

    fireEvent.click(screen.getByText('漫画A'));

    await waitFor(() => expect(screen.getByText('READER_PAGE')).toBeInTheDocument());
  });

  test('面板里的「重命名」接上既有弹窗（此前触屏完全没有入口）', async () => {
    renderWithReader();
    await waitFor(() => expect(screen.getByText('漫画A')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: '更多操作：漫画A' }));
    const sheet = screen.getByRole('dialog', { name: '漫画操作' });
    fireEvent.click(within(sheet).getByRole('button', { name: /重命名/ }));

    await waitFor(() => expect(screen.getByText('重命名漫画')).toBeInTheDocument());
    expect(screen.queryByRole('dialog', { name: '漫画操作' })).toBeNull();
  });

  test('面板里的「从库中移除」走既有二次确认', async () => {
    renderWithReader();
    await waitFor(() => expect(screen.getByText('漫画A')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: '更多操作：漫画A' }));
    const sheet = screen.getByRole('dialog', { name: '漫画操作' });
    fireEvent.click(within(sheet).getByRole('button', { name: /从库中移除/ }));

    await waitFor(() => expect(screen.getByText('移除漫画')).toBeInTheDocument());
  });
});
