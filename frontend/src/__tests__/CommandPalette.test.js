import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter, Routes, Route } from 'react-router-dom';
import CommandPalette from '../components/CommandPalette';
import { SettingsProvider } from '../hooks/useSettings';
import { registerCommands, resetCommands } from '../hooks/useCommands';

jest.mock('../utils/api');
const api = require('../utils/api').default;

/**
 * 命令面板要解决的是"这个功能在哪一栏"：三类条目（导航 / 当前页面登记的操作 /
 * 实时搜到的漫画）都必须在同一条通道里可达，且键盘全程够用。
 */

function renderPalette(onClose = jest.fn()) {
  render(
    <MemoryRouter initialEntries={['/']}>
      <SettingsProvider>
        <Routes>
          <Route path="/" element={<CommandPalette onClose={onClose} />} />
          <Route path="/history" element={<div>HISTORY_PAGE</div>} />
          <Route path="/reader/:archiveId" element={<div>READER_PAGE</div>} />
        </Routes>
      </SettingsProvider>
    </MemoryRouter>
  );
  return onClose;
}

describe('CommandPalette 命令面板', () => {
  beforeEach(() => {
    resetCommands();
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({ theme: 'dark' });
    api.getArchives.mockResolvedValue([]);
    api.updateSettings.mockResolvedValue({});
  });

  afterEach(() => {
    resetCommands();
  });

  test('列出导航命令与当前页面登记的操作，并按键分组', async () => {
    registerCommands([
      { id: 'lib-scan', group: '书库', icon: '🗂️', label: '扫描目录…', run: jest.fn() },
    ]);
    renderPalette();

    expect(screen.getByText('去书库')).toBeInTheDocument();
    expect(screen.getByText('去阅读历史')).toBeInTheDocument();
    expect(screen.getByText('扫描目录…')).toBeInTheDocument();
    expect(screen.getByText('导航')).toBeInTheDocument();
    expect(screen.getByText('书库')).toBeInTheDocument();
  });

  test('输入即过滤命令', async () => {
    renderPalette();
    fireEvent.change(screen.getByLabelText('搜索命令或漫画'), { target: { value: '历史' } });

    await waitFor(() => expect(screen.getByText('去阅读历史')).toBeInTheDocument());
    expect(screen.queryByText('去设置')).toBeNull();
  });

  test('回车执行高亮项，并先关闭面板', async () => {
    const run = jest.fn();
    registerCommands([{ id: 'only', group: '书库', label: '执行我', run }]);
    const onClose = renderPalette();

    const input = screen.getByLabelText('搜索命令或漫画');
    // 先收窄到目标命令：导航命令也在列表里（且排在页面命令之前）
    fireEvent.change(input, { target: { value: '执行我' } });
    fireEvent.keyDown(input, { key: 'Enter' });

    expect(run).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  test('↓/Tab 移动高亮，回车执行的是移动后的那条', async () => {
    const first = jest.fn();
    const second = jest.fn();
    registerCommands([
      { id: 'a', group: '书库', label: '条目甲', run: first },
      { id: 'b', group: '书库', label: '条目乙', run: second },
    ]);
    renderPalette();

    const input = screen.getByLabelText('搜索命令或漫画');
    fireEvent.change(input, { target: { value: '条目' } });
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    fireEvent.keyDown(input, { key: 'Enter' });

    expect(second).toHaveBeenCalledTimes(1);
    expect(first).not.toHaveBeenCalled();
  });

  test('输入漫画名时实时搜索，并能直接进阅读器', async () => {
    api.getArchives.mockResolvedValue([
      { id: 7, title: '想看的漫画', archive_type: 'cbz', page_count: 12 },
    ]);
    renderPalette();

    fireEvent.change(screen.getByLabelText('搜索命令或漫画'), { target: { value: '想看' } });

    await waitFor(() => expect(screen.getByText('打开《想看的漫画》')).toBeInTheDocument());
    expect(api.getArchives).toHaveBeenCalledWith(expect.objectContaining({ search: '想看' }));

    fireEvent.click(screen.getByText('打开《想看的漫画》'));
    await waitFor(() => expect(screen.getByText('READER_PAGE')).toBeInTheDocument());
  });

  test('没有输入时不发漫画搜索请求（不给后端添无谓的查询）', async () => {
    renderPalette();
    await waitFor(() => expect(screen.getByText('去书库')).toBeInTheDocument());
    expect(api.getArchives).not.toHaveBeenCalled();
  });

  test('搜不到时给出空态而不是空白面板', async () => {
    renderPalette();
    fireEvent.change(screen.getByLabelText('搜索命令或漫画'), { target: { value: 'zzzzz' } });
    await waitFor(() => expect(screen.getByText('没有匹配的条目')).toBeInTheDocument());
  });

  test('主题命令把当前主题标出来，执行后写入设置', async () => {
    renderPalette();
    expect(screen.getByText('主题：深色')).toBeInTheDocument();

    fireEvent.click(screen.getByText('主题：护眼'));
    await waitFor(() => expect(api.updateSettings).toHaveBeenCalledWith({ theme: 'eye-care' }));
  });
});
