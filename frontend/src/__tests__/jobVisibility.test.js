import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import Settings from '../pages/Settings';
import JobIndicator from '../components/JobIndicator';
import { ToastProvider } from '../components/Toast';
import { SettingsProvider } from '../hooks/useSettings';
import { TagsProvider } from '../hooks/useTags';
import { resetJobsStore } from '../hooks/useJobs';

jest.mock('../utils/api');
const api = require('../utils/api').default;

/**
 * 回归：长任务的状态曾经活在「发起它的页面」里（useScan/useSync 各自的 useState +
 * 1s 轮询），于是离开设置页就看不到进度、回来时组件重新挂载又把按钮变回可启动，
 * 用户完全看不出任务还在跑。现在状态归任务层（useJobs）所有，本文件守住这条线。
 */

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

const runningScan = {
  running: true, total: 10, done: 3, current: '扫描中',
  added: 1, updated: 0, unchanged: 2, removed: 0, skipped: 0,
};

describe('长任务跨页面可见性', () => {
  beforeEach(() => {
    resetJobsStore();
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({ root_dir: '/library', scan_depth: '2', theme: 'dark' });
    api.getStats.mockResolvedValue({ total_archives: 10, total_pages: 500, total_size: 1024, total_tags: 1, total_categories: 1, history_count: 1 });
    api.getTags.mockResolvedValue([]);
    api.getCategories.mockResolvedValue([]);
    api.getLanIps.mockResolvedValue({ ipv4: [], port: 5002 });
    api.scanStatus.mockResolvedValue(runningScan);
    api.syncStatus.mockResolvedValue({ running: false });
    api.convertCbzStatus.mockResolvedValue({ running: false });
  });

  afterEach(() => {
    resetJobsStore();
  });

  test('扫描进行中离开设置页再回来：进度仍在，按钮不会变回可启动', async () => {
    const first = renderSettings();
    await waitFor(() => expect(screen.getByText(/已扫描 3 \/ 10/)).toBeInTheDocument());
    expect(screen.getByRole('button', { name: '扫描中...' })).toBeDisabled();

    // 切到书库 = 设置页卸载；任务仍在后端跑
    first.unmount();

    // 切回设置页：重新挂载也要立刻显示真实进度（此前这里是「立即扫描」可点 + 无进度）
    renderSettings();
    await waitFor(() => expect(screen.getByText(/已扫描 3 \/ 10/)).toBeInTheDocument());
    expect(screen.getByRole('button', { name: '扫描中...' })).toBeDisabled();
  });

  test('任务指示器在任意页面显示在跑的任务并可取消', async () => {
    // 先让任务层水合到一个在跑的扫描上（模拟"任务是在别的页面发起的"）
    render(<JobIndicator />);
    await waitFor(() => expect(screen.getByText('扫描书库')).toBeInTheDocument());
    expect(screen.getByText('3/10')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '取消扫描书库' }));
    await waitFor(() => expect(api.scanCancel).toHaveBeenCalled());
  });

  test('没有任务在跑时不渲染指示器（不占屏幕）', async () => {
    api.scanStatus.mockResolvedValue({ running: false });
    const { container } = render(<JobIndicator />);
    await waitFor(() => expect(api.scanStatus).toHaveBeenCalled());
    expect(container.querySelector('.job-indicator')).toBeNull();
    expect(screen.queryByText('扫描书库')).toBeNull();
  });
});
