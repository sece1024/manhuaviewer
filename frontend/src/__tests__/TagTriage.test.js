import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import TagTriage from '../components/TagTriage';
import { ToastProvider } from '../components/Toast';
import { TagsProvider, resetTagsCache } from '../hooks/useTags';

jest.mock('../utils/api');
const api = require('../utils/api').default;

/**
 * 整理模式的关键是「一本一本来，键盘完成」，所以这里的用例都盯着三件事：
 * 打标后自动前进、跳过不打标、以及失败时**不前进**（否则用户以为打上了）。
 */

const TAGS = [
  { id: 11, namespace: '', name: '动作', color: '#f00' },
  { id: 12, namespace: 'artist', name: '作者A', color: '#0f0' },
];

const ITEMS = [
  { id: 1, title: '第一本', archive_type: 'cbz', page_count: 10, cover_url: '/c1' },
  { id: 2, title: '第二本', archive_type: 'cbz', page_count: 20, cover_url: '/c2' },
];

function renderTriage() {
  const onClose = jest.fn();
  render(
    <ToastProvider>
      <TagsProvider>
        <TagTriage sortBy="created" sortOrder="desc" onClose={onClose} />
      </TagsProvider>
    </ToastProvider>
  );
  return { onClose };
}

/// 等第一本上屏后返回输入框
async function ready() {
  await waitFor(() => expect(screen.getByText('第一本')).toBeInTheDocument());
  return screen.getByLabelText('标签');
}

describe('TagTriage 整理模式', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    resetTagsCache();
    api.getTags.mockResolvedValue(TAGS);
    api.getArchives.mockResolvedValue(ITEMS);
    api.assignTag.mockResolvedValue({});
    api.createTag.mockResolvedValue({ data: { id: 99, namespace: '', name: '恐怖' } });
  });

  test('只取「未打标签」的档案，并显示还剩多少本', async () => {
    renderTriage();
    await ready();
    expect(api.getArchives).toHaveBeenCalledWith(expect.objectContaining({
      tag_state: 'untagged',
      sort_by: 'created',
      sort_order: 'desc',
    }));
    expect(screen.getByText(/还剩 2 本待整理/)).toBeInTheDocument();
    // 候选默认全部标签
    expect(screen.getByText('动作')).toBeInTheDocument();
    expect(screen.getByText('artist:作者A')).toBeInTheDocument();
  });

  test('回车给高亮候选打标并自动前进到下一本', async () => {
    renderTriage();
    const input = await ready();

    fireEvent.keyDown(input, { key: 'Enter' });

    await waitFor(() => expect(api.assignTag).toHaveBeenCalledWith(1, 11));
    await waitFor(() => expect(screen.getByText('第二本')).toBeInTheDocument());
    expect(screen.getByText(/已整理 1 本/)).toBeInTheDocument();
    // 输入被清空，下一本可以立刻重新输入
    expect(input.value).toBe('');
  });

  test('输入的标签名不存在时，回车即创建并打标（一次输入完成"新标签+打标"）', async () => {
    renderTriage();
    const input = await ready();

    fireEvent.change(input, { target: { value: '恐怖' } });
    expect(screen.getByText('新建「恐怖」')).toBeInTheDocument();

    fireEvent.keyDown(input, { key: 'Enter' });

    await waitFor(() => expect(api.createTag).toHaveBeenCalledWith({ namespace: '', name: '恐怖' }));
    await waitFor(() => expect(api.assignTag).toHaveBeenCalledWith(1, 99));
    await waitFor(() => expect(screen.getByText('第二本')).toBeInTheDocument());
  });

  test('Tab 切换候选（焦点不会逃出面板），回车打的是切换后的那个', async () => {
    renderTriage();
    const input = await ready();

    fireEvent.keyDown(input, { key: 'Tab' });
    fireEvent.keyDown(input, { key: 'Enter' });

    await waitFor(() => expect(api.assignTag).toHaveBeenCalledWith(1, 12));
  });

  test('Esc 是跳过（不打标但前进），不是关闭', async () => {
    const { onClose } = renderTriage();
    const input = await ready();

    fireEvent.keyDown(input, { key: 'Escape' });

    await waitFor(() => expect(screen.getByText('第二本')).toBeInTheDocument());
    expect(api.assignTag).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByText(/跳过 1/)).toBeInTheDocument();
  });

  test('Shift+Esc 退出（Esc 被跳过占用，退出需要显式的组合键或按钮）', async () => {
    const { onClose } = renderTriage();
    const input = await ready();

    fireEvent.keyDown(input, { key: 'Escape', shiftKey: true });

    expect(onClose).toHaveBeenCalled();
  });

  test('打标失败时不前进，避免用户以为已经打上', async () => {
    renderTriage();
    const input = await ready();
    api.assignTag.mockRejectedValue(new Error('写入失败'));

    fireEvent.keyDown(input, { key: 'Enter' });

    await waitFor(() => expect(screen.getByText('写入失败')).toBeInTheDocument());
    expect(screen.getByText('第一本')).toBeInTheDocument();
    expect(screen.queryByText('第二本')).toBeNull();
    expect(screen.getByText(/已整理 0 本/)).toBeInTheDocument();
  });

  test('没有未打标签的档案时给出完成态，而不是空面板', async () => {
    api.getArchives.mockResolvedValue([]);
    renderTriage();
    await waitFor(() => expect(screen.getByText(/这一批整理完了/)).toBeInTheDocument());
    expect(screen.queryByLabelText('标签')).toBeNull();
  });
});
