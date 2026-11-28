import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import TagPicker from '../components/TagPicker';
import { ToastProvider } from '../components/Toast';

jest.mock('../utils/api');
const api = require('../utils/api').default;

/**
 * 这组用例盯的是批量模式的诚实性：界面必须能看出「哪些标签已在选中项上」
 * （全部包含 / 部分包含 / 未包含三种状态），点错了能用再点一次撤销，
 * 失败也要看得见——此前这三件事都不成立（checked 写死 false、只能"加"、静默 catch）。
 */

const TAGS = [
  { id: 11, namespace: '', name: '动作', color: '#f00' },
  { id: 12, namespace: 'artist', name: '作者A', color: '#0f0' },
  { id: 13, namespace: '', name: '没出现过', color: '#00f' },
];

function renderPicker(props) {
  const onClose = jest.fn();
  render(
    <ToastProvider>
      <TagPicker {...props} onClose={onClose} />
    </ToastProvider>
  );
  return onClose;
}

const itemOf = (text) => screen.getByText(text).closest('.tag-picker-item');

describe('TagPicker 单本模式', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getTags.mockResolvedValue(TAGS);
    api.getArchiveTags.mockResolvedValue([{ id: 11, namespace: '', name: '动作', color: '#f00' }]);
    api.assignTag.mockResolvedValue({});
    api.removeTag.mockResolvedValue({});
    api.createTag.mockResolvedValue({ data: { id: 99, namespace: 'artist', name: '作者B' } });
  });

  test('已分配的标签显示为勾选，点它即移除；未分配的点击即加上', async () => {
    renderPicker({ archiveId: 1 });
    await waitFor(() => expect(screen.getByText('动作')).toBeInTheDocument());

    const assigned = itemOf('动作');
    expect(assigned.className).toContain('checked');
    fireEvent.click(assigned);
    await waitFor(() => expect(api.removeTag).toHaveBeenCalledWith(1, 11));
    await waitFor(() => expect(itemOf('动作').className).not.toContain('checked'));

    const unassigned = itemOf('artist:作者A');
    expect(unassigned.className).not.toContain('checked');
    fireEvent.click(unassigned);
    await waitFor(() => expect(api.assignTag).toHaveBeenCalledWith(1, 12));
  });

  test('操作失败会报错，不再静默吞掉', async () => {
    api.assignTag.mockRejectedValue(new Error('标签写入失败'));
    renderPicker({ archiveId: 1 });
    await waitFor(() => expect(screen.getByText('artist:作者A')).toBeInTheDocument());

    fireEvent.click(itemOf('artist:作者A'));

    await waitFor(() => expect(screen.getByText('标签写入失败')).toBeInTheDocument());
  });

  test('新建标签（支持 ns:name）后自动分配给当前档案', async () => {
    renderPicker({ archiveId: 1 });
    await waitFor(() => expect(screen.getByText('动作')).toBeInTheDocument());

    fireEvent.change(screen.getByLabelText('新建标签'), { target: { value: 'artist:作者B' } });
    fireEvent.click(screen.getByRole('button', { name: '创建' }));

    await waitFor(() => expect(api.createTag).toHaveBeenCalledWith({ namespace: 'artist', name: '作者B' }));
    await waitFor(() => expect(api.assignTag).toHaveBeenCalledWith(1, 99));
  });

  test('标签很多时出现过滤框，且只按输入收窄列表', async () => {
    const many = Array.from({ length: 9 }, (_, i) => ({
      id: 100 + i, namespace: '', name: `标签${i}`, color: '#ccc',
    }));
    api.getTags.mockResolvedValue(many);
    api.getArchiveTags.mockResolvedValue([]);
    renderPicker({ archiveId: 1 });
    await waitFor(() => expect(screen.getByText('标签0')).toBeInTheDocument());

    fireEvent.change(screen.getByLabelText('按名称过滤标签'), { target: { value: '标签3' } });

    await waitFor(() => expect(screen.getByText('标签3')).toBeInTheDocument());
    expect(screen.queryByText('标签0')).toBeNull();
    expect(screen.queryByText('标签8')).toBeNull();
  });
});

describe('TagPicker 标签排序（按使用频次）', () => {
  // 频次来自后端 /api/tags 的 archive_count（全库有多少本带这个标签）
  const COUNTED = [
    { id: 21, namespace: '', name: '冷门', color: '#111', archive_count: 1 },
    { id: 22, namespace: '', name: '常用', color: '#222', archive_count: 42 },
    { id: 23, namespace: '', name: '中等', color: '#333', archive_count: 7 },
  ];

  beforeEach(() => {
    jest.clearAllMocks();
    api.getTags.mockResolvedValue(COUNTED);
    api.getArchiveTags.mockResolvedValue([]);
    api.assignTag.mockResolvedValue({});
    api.createTag.mockResolvedValue({ data: { id: 99, namespace: '', name: '刚建的' } });
  });

  const renderedNames = () =>
    Array.from(document.querySelectorAll('.tag-picker-name')).map(n => n.textContent);
  const renderedCounts = () =>
    Array.from(document.querySelectorAll('.tag-picker-count')).map(n => n.textContent);

  test('最常用的标签排在最上方，并把使用次数显示出来', async () => {
    renderPicker({ archiveId: 1 });
    await waitFor(() => expect(screen.getByText('常用')).toBeInTheDocument());

    expect(renderedNames()).toEqual(['常用', '中等', '冷门']);
    expect(renderedCounts()).toEqual(['42', '7', '1']);
  });

  test('勾选/取消标签不会让列表重排（否则会在点击途中点到相邻项）', async () => {
    renderPicker({ archiveId: 1 });
    await waitFor(() => expect(screen.getByText('冷门')).toBeInTheDocument());

    fireEvent.click(itemOf('冷门'));

    await waitFor(() => expect(api.assignTag).toHaveBeenCalledWith(1, 21));
    expect(renderedNames()).toEqual(['常用', '中等', '冷门']);
  });

  test('本次新建的标签置顶，且按实际分配到的档案数计数', async () => {
    renderPicker({ archiveId: 1 });
    await waitFor(() => expect(screen.getByText('常用')).toBeInTheDocument());

    fireEvent.change(screen.getByLabelText('新建标签'), { target: { value: '刚建的' } });
    fireEvent.click(screen.getByRole('button', { name: '创建' }));

    await waitFor(() => expect(api.assignTag).toHaveBeenCalledWith(1, 99));
    // 频次只有 1，纯按频次排会沉到列表中部，用户看不见就以为没建上
    await waitFor(() => expect(renderedNames()[0]).toBe('刚建的'));
    expect(renderedCounts()[0]).toBe('1');
  });

  test('批量模式下同时显示全库次数与选中项命中数', async () => {
    api.getTagCounts.mockResolvedValue({ total: 3, counts: { 21: 2 } });
    renderPicker({ archiveIds: [1, 2, 3] });
    await waitFor(() => expect(screen.getByText('冷门')).toBeInTheDocument());

    const row = itemOf('冷门');
    expect(row.querySelector('.tag-picker-count').textContent).toBe('1');
    expect(row.querySelector('.tag-picker-partial-hint').textContent).toBe('2/3');
  });
});

describe('TagPicker 批量模式（三态）', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getTags.mockResolvedValue(TAGS);
    api.getTagCounts.mockResolvedValue({ total: 3, counts: { 11: 3, 12: 1 } });
    api.batchAssignTag.mockResolvedValue({});
    api.batchRemoveTag.mockResolvedValue({});
  });

  test('区分全部包含（✓）/ 部分包含（− 并给出 1/3）/ 都没有，点击方向相反', async () => {
    renderPicker({ archiveIds: [1, 2, 3] });
    await waitFor(() => expect(screen.getByText('动作')).toBeInTheDocument());

    const all = itemOf('动作');
    expect(all.className).toContain('checked');

    const some = itemOf('artist:作者A');
    expect(some.className).toContain('partial');
    expect(some.textContent).toContain('1/3');

    const none = itemOf('没出现过');
    expect(none.className).not.toContain('checked');
    expect(none.className).not.toContain('partial');

    // 部分包含 → 点击补成"全部包含"，而不是移除（这才是用户想要的默认方向）
    fireEvent.click(some);
    await waitFor(() => expect(api.batchAssignTag).toHaveBeenCalledWith([1, 2, 3], 12));
    expect(api.batchRemoveTag).not.toHaveBeenCalled();

    // 全部包含 → 点击移除
    fireEvent.click(all);
    await waitFor(() => expect(api.batchRemoveTag).toHaveBeenCalledWith([1, 2, 3], 11));
  });

  test('计数接口不可用时降级为「都没有」，不把选中项误标成已包含', async () => {
    api.getTagCounts.mockResolvedValue(undefined);
    renderPicker({ archiveIds: [1, 2] });
    await waitFor(() => expect(screen.getByText('动作')).toBeInTheDocument());

    expect(itemOf('动作').className).not.toContain('checked');
    expect(itemOf('动作').className).not.toContain('partial');
  });

  test('批量打标失败会报错', async () => {
    api.batchAssignTag.mockRejectedValue(new Error('批量写入失败'));
    renderPicker({ archiveIds: [1, 2, 3] });
    await waitFor(() => expect(screen.getByText('artist:作者A')).toBeInTheDocument());

    fireEvent.click(itemOf('artist:作者A'));

    await waitFor(() => expect(screen.getByText('批量写入失败')).toBeInTheDocument());
  });
});
