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
