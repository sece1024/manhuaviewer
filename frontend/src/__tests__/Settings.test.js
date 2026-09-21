import React from 'react';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import Settings from '../pages/Settings';
import { ToastProvider } from '../components/Toast';
import { SettingsProvider } from '../hooks/useSettings';
import { TagsProvider } from '../hooks/useTags';

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
});
