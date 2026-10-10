import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import Settings from '../pages/Settings';
import { ToastProvider } from '../components/Toast';
import { SettingsProvider } from '../hooks/useSettings';
import { TagsProvider } from '../hooks/useTags';
import { resetJobsStore } from '../hooks/useJobs';

jest.mock('../utils/api');
const api = require('../utils/api').default;

function renderSettings() {
  return render(
    <MemoryRouter>
      <SettingsProvider>
        <TagsProvider>
          <ToastProvider>
            <Settings />
          </ToastProvider>
        </TagsProvider>
      </SettingsProvider>
    </MemoryRouter>
  );
}

describe('Settings 页面', () => {
  beforeEach(() => {
    resetJobsStore();
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({ page_direction: 'rtl', reader_fit: 'height', theme: 'dark' });
    api.getStats.mockResolvedValue({ total_archives: 10, total_pages: 500, total_size: 1024000, total_tags: 5, total_categories: 3, history_count: 20 });
    api.getTags.mockResolvedValue([
      // 与后端 /api/tags 实际返回一致：{id, namespace, name, color, archive_count}，无 full_name
      { id: 1, namespace: 'artist', name: '测试作者', color: '#ff0000', archive_count: 3 },
    ]);
    api.getCategories.mockResolvedValue([
      { id: 1, name: '动作', color: '#00ff00', pinned: 0, archive_count: 5 },
    ]);
    api.getLanIps.mockResolvedValue({ ipv4: [], port: 5002 });
  });

  test('加载并显示统计数据（键与后端一致）', async () => {
    renderSettings();
    await waitFor(() => {
      expect(screen.getByText('10')).toBeInTheDocument(); // 漫画总数
    });
    expect(screen.getByText('500')).toBeInTheDocument();     // 总页数（toLocaleString）
    expect(screen.getByText('5')).toBeInTheDocument();       // 标签数
    expect(screen.getByText('3')).toBeInTheDocument();       // 分类数
    expect(screen.getByText('20')).toBeInTheDocument();      // 阅读记录
    expect(screen.getByText('1000.0 KB')).toBeInTheDocument(); // 总大小（formatSize(1024000) → 1000.0 KB）
  });

  test('显示设置区域标题', async () => {
    renderSettings();
    await waitFor(() => {
      expect(screen.getByText('📂 分类管理')).toBeInTheDocument();
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

  test('点击立即扫描触发扫描接口并提示结果', async () => {
    api.getSettings.mockResolvedValue({ root_dir: '/library', scan_depth: '2', theme: 'dark' });
    api.scan.mockResolvedValue({ message: '扫描完成：共 3 个档案' });
    api.scanStatus.mockResolvedValue({ running: false, total: 0, done: 0 });
    renderSettings();

    // 等服务端设置回填根目录后再触发，确保使用已持久化的目录/深度
    await waitFor(() => expect(screen.getByDisplayValue('/library')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: '立即扫描' }));

    await waitFor(() => expect(api.scan).toHaveBeenCalledWith('/library', 2));
    expect(await screen.findByText('扫描完成：共 3 个档案')).toBeInTheDocument();
  });

  test('转换为 CBZ：确认后启动转换任务', async () => {
    api.convertCbzStart.mockResolvedValue({ started: true, total: 3 });
    api.convertCbzStatus.mockResolvedValue({
      running: false, total: 0, done: 0, converted: 0, skipped: 0, failed: 0, current: '', errors: [],
    });
    renderSettings();

    fireEvent.click(await screen.findByRole('button', { name: '转换为 CBZ' }));
    fireEvent.click(await screen.findByRole('button', { name: '开始转换' }));

    await waitFor(() => expect(api.convertCbzStart).toHaveBeenCalled());
  });
});

describe('Settings 扫描目录（多根目录记忆）', () => {
  const twoRoots = JSON.stringify([{ path: '/a', depth: 1 }, { path: '/b', depth: 3 }]);

  beforeEach(() => {
    resetJobsStore();
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({ scan_roots: twoRoots, root_dir: '/a', scan_depth: '1', theme: 'dark' });
    api.getStats.mockResolvedValue({});
    api.getTags.mockResolvedValue([]);
    api.getCategories.mockResolvedValue([]);
    api.getLanIps.mockResolvedValue({ ipv4: [], port: 5002 });
    api.scanStatus.mockResolvedValue({ running: false, total: 0, done: 0 });
    api.syncStatus.mockResolvedValue({ running: false });
    api.convertCbzStatus.mockResolvedValue({ running: false });
  });

  test('列出记住的每个目录，各自按自己的深度独立扫描', async () => {
    renderSettings();
    await waitFor(() => expect(screen.getByText('/a')).toBeInTheDocument());
    expect(screen.getByText('/b')).toBeInTheDocument();

    const buttons = screen.getAllByRole('button', { name: '立即扫描' });
    expect(buttons).toHaveLength(2);
    // 第 2 条是 /b（深度 3）：扫描必须带上它自己的深度，而不是"当前深度"
    fireEvent.click(buttons[1]);
    await waitFor(() => expect(api.scan).toHaveBeenCalledWith('/b', 3));
    expect(api.updateSettings).toHaveBeenCalledWith({ scan_depth: '3' });
  });

  test('「加入列表」把新目录写进 scan_roots（深度转字符串，后端设置表只收 string）', async () => {
    renderSettings();
    await waitFor(() => expect(screen.getByText('/a')).toBeInTheDocument());

    fireEvent.change(screen.getByLabelText('扫描目录路径'), { target: { value: '/new' } });
    fireEvent.click(screen.getByRole('button', { name: '加入列表' }));

    await waitFor(() => expect(api.updateSettings).toHaveBeenCalledWith({
      scan_roots: JSON.stringify([
        { path: '/new', depth: 1 },
        { path: '/a', depth: 1 },
        { path: '/b', depth: 3 },
      ]),
    }));
  });

  test('「移除」忘记该目录；移除 root_dir 指向的那条同时清空镜像', async () => {
    renderSettings();
    await waitFor(() => expect(screen.getByText('/a')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: '移除扫描目录 /a' }));

    // root_dir 是「最近使用」的镜像；不清空它，下次渲染会用它把 /a 重新种回列表，
    // 看起来就像"移除没生效"
    await waitFor(() => expect(api.updateSettings).toHaveBeenCalledWith({ root_dir: '' }));
    expect(api.updateSettings).toHaveBeenCalledWith({
      scan_roots: JSON.stringify([{ path: '/b', depth: 3 }]),
    });
  });

  test('列表为空时用旧的 root_dir 种一条（升级不丢扫描目录）', async () => {
    api.getSettings.mockResolvedValue({ scan_roots: '[]', root_dir: '/legacy', scan_depth: '2' });
    renderSettings();
    await waitFor(() => expect(screen.getByText('/legacy')).toBeInTheDocument());
    expect(screen.getAllByRole('button', { name: '立即扫描' })).toHaveLength(1);
  });

  test('完全没有目录时给出引导，「加入列表」在输入为空时不可点', async () => {
    api.getSettings.mockResolvedValue({ scan_roots: '[]', root_dir: '' });
    renderSettings();
    await waitFor(() => expect(screen.getByText(/还没有扫描目录/)).toBeInTheDocument());
    expect(screen.queryByRole('button', { name: '立即扫描' })).toBeNull();
    expect(screen.getByRole('button', { name: '加入列表' })).toBeDisabled();
  });
});

// 设置页有 11 个分区、30 多个设置项，此前只能靠滚动找；而「局域网访问」那三项还被塞在
// 「漫画库」分区末尾，锚点也叫「漫画库」——按"局域网"找根本找不到。
describe('Settings 分区与搜索', () => {
  beforeEach(() => {
    resetJobsStore();
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({ root_dir: '/library', scan_depth: '2', theme: 'dark' });
    api.getStats.mockResolvedValue({ total_archives: 1, total_pages: 1, total_size: 1024, total_tags: 0, total_categories: 0, history_count: 0 });
    api.getTags.mockResolvedValue([]);
    api.getCategories.mockResolvedValue([]);
    api.getLanIps.mockResolvedValue({ ipv4: [], port: 5002 });
    api.scanStatus.mockResolvedValue({ running: false });
    api.syncStatus.mockResolvedValue({ running: false });
    api.convertCbzStatus.mockResolvedValue({ running: false });
  });

  test('局域网相关设置独立成区，并出现在导航里（此前埋在「漫画库」下）', async () => {
    // 「访问地址」只在回环请求下渲染（LAN 设备看不到宿主网卡地址），这里模拟桌面端
    api.getLanIps.mockResolvedValue({ loopback: true, ipv4: ['192.168.1.5'], port: 5002 });
    renderSettings();
    await waitFor(() => expect(screen.getByText('🗂️ 漫画库')).toBeInTheDocument());

    const lanSection = document.getElementById('settings-section-lan');
    expect(lanSection).not.toBeNull();
    // 三项局域网设置都在这个分区里，而不是在漫画库分区里
    expect(lanSection.textContent).toContain('允许局域网设备访问');
    expect(lanSection.textContent).toContain('访问地址');
    expect(lanSection.textContent).toContain('局域网访问口令');
    expect(document.getElementById('settings-section-library').textContent).not.toContain('局域网访问口令');

    expect(screen.getByRole('link', { name: '局域网与访问' })).toHaveAttribute('href', '#settings-section-lan');
  });

  test('搜索设置项：只留下命中的行，整节无命中就隐藏整节（连导航项一起）', async () => {
    renderSettings();
    await waitFor(() => expect(screen.getByText('🎨 外观')).toBeInTheDocument());

    fireEvent.change(screen.getByLabelText('搜索设置项'), { target: { value: '口令' } });

    // 命中的行可见
    const lanSection = document.getElementById('settings-section-lan');
    expect(lanSection.className).not.toContain('settings-filtered-out');
    // 没命中的分区整节隐藏，导航项也隐藏
    expect(document.getElementById('settings-section-appearance').className).toContain('settings-filtered-out');
    expect(screen.getByRole('link', { name: '外观' }).className).toContain('settings-filtered-out');
    // 该分区内没命中的行也被隐藏
    const hiddenRows = lanSection.querySelectorAll('.settings-row.settings-filtered-out');
    expect(hiddenRows.length).toBeGreaterThan(0);
  });

  test('清空搜索后所有分区恢复可见', async () => {
    renderSettings();
    await waitFor(() => expect(screen.getByText('🎨 外观')).toBeInTheDocument());

    const input = screen.getByLabelText('搜索设置项');
    fireEvent.change(input, { target: { value: '口令' } });
    expect(document.getElementById('settings-section-appearance').className).toContain('settings-filtered-out');

    fireEvent.change(input, { target: { value: '' } });
    expect(document.getElementById('settings-section-appearance').className).not.toContain('settings-filtered-out');
  });

  test('搜不到时给出明确空态，而不是一片空白', async () => {
    renderSettings();
    await waitFor(() => expect(screen.getByText('🎨 外观')).toBeInTheDocument());

    fireEvent.change(screen.getByLabelText('搜索设置项'), { target: { value: 'zzzz不存在' } });

    const hint = document.querySelector('[data-settings-no-match]');
    expect(hint.className).not.toContain('settings-filtered-out');
    expect(hint.textContent).toContain('没有匹配的设置项');
  });
});
