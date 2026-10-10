import { renderHook, act, waitFor } from '@testing-library/react';
import useProgressPersistence from '../hooks/useProgressPersistence';

jest.mock('../utils/api');
const api = require('../utils/api').default;

/**
 * 阅读进度是用户唯一真正在意、又完全看不见的持久化状态：保存失败时本机翻页一切正常，
 * 用户不会有任何察觉，直到下次打开发现进度退回。所以"失败要看得见"必须有用例守着，
 * 同时不能变成每翻一页都弹一次。
 */
describe('useProgressPersistence 保存失败可见', () => {
  const archive = { id: 1 };
  const pages = [{}, {}];

  beforeEach(() => {
    jest.clearAllMocks();
  });

  test('首次保存失败会回调一次；之后继续失败不再刷屏', async () => {
    api.saveHistory.mockRejectedValue(new Error('写入失败'));
    const onSaveError = jest.fn();

    const { rerender } = renderHook(
      ({ index }) => useProgressPersistence({
        archive, archiveId: '1', pages, currentIndex: index, onSaveError,
      }),
      { initialProps: { index: 0 } }
    );

    // 防抖 1s 后才会真正写
    await waitFor(() => expect(onSaveError).toHaveBeenCalledTimes(1), { timeout: 2500 });

    // 再翻一页：仍然失败，但不该再提示一次
    rerender({ index: 1 });
    await act(async () => { await new Promise(r => setTimeout(r, 1200)); });
    expect(onSaveError).toHaveBeenCalledTimes(1);
  });

  test('保存成功不回调错误（正常阅读不该有噪音）', async () => {
    api.saveHistory.mockResolvedValue({});
    const onSaveError = jest.fn();

    renderHook(() => useProgressPersistence({
      archive, archiveId: '1', pages, currentIndex: 0, onSaveError,
    }));

    await act(async () => { await new Promise(r => setTimeout(r, 1200)); });
    expect(onSaveError).not.toHaveBeenCalled();
  });

  test('未传 onSaveError 时失败也不抛错（回调是可选的）', async () => {
    api.saveHistory.mockRejectedValue(new Error('写入失败'));
    renderHook(() => useProgressPersistence({
      archive, archiveId: '1', pages, currentIndex: 0,
    }));
    await act(async () => { await new Promise(r => setTimeout(r, 1200)); });
    // 没有断言崩溃即通过
  });
});
