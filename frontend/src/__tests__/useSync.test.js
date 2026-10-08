import { renderHook, act } from '@testing-library/react';
import useSync from '../hooks/useSync';

jest.mock('../utils/api');
const api = require('../utils/api').default;

// useSync 从 api 具名导入 invalidateLibrarySessions；automock 会让它变成 no-op，
// 这里拿真实实现（同步任务结束时会调用它）。
const { invalidateLibrarySessions } = require('../utils/api');

describe('useSync 轮询竞态', () => {
  let pollCb; // 拦截 setInterval 捕获到的轮询回调
  let clearSpy;

  beforeEach(() => {
    jest.clearAllMocks();
    api.syncStart.mockResolvedValue({ started: true });
    api.syncCancel.mockResolvedValue({});
    api.getStats.mockResolvedValue({});

    // 拦截定时器：不真实计时，手动驱动回调，让异步状态机完全确定
    pollCb = null;
    clearSpy = jest.fn();
    jest.spyOn(global, 'setInterval').mockImplementation((cb) => { pollCb = cb; return 123; });
    jest.spyOn(global, 'clearInterval').mockImplementation(clearSpy);
  });

  afterEach(() => {
    jest.restoreAllMocks();
  });

  test('启动瞬间 status 尚未置 running=true 不会误停轮询（回归）', async () => {
    // 第 1 次（启动后立刻）：后端还没置 running；第 2 次：running=true；
    // 第 3 次：真正结束 running=false
    let call = 0;
    const seq = [
      { running: false, total: 0, done: 0, current: '排队中' },
      { running: true, total: 10, done: 3, current: '下载中' },
      { running: false, total: 10, done: 10, current: '完成' },
    ];
    api.syncStatus.mockImplementation(() => Promise.resolve(seq[Math.min(call++, seq.length - 1)]));

    const props = {
      settings: { sync_remote_url: 'http://192.168.1.9:5002', sync_dir: '/data/sync' },
      updateSetting: jest.fn().mockResolvedValue({}),
      toast: jest.fn(),
      onStatsRefresh: jest.fn(),
    };
    const { result } = renderHook(() => useSync(props));

    await act(async () => {
      await result.current.handleSyncStart();
    });

    // 第 1 次轮询已返回 running:false，但尚未见过 running=true → 不能停
    expect(result.current.syncRunning).toBe(true);
    expect(clearSpy).not.toHaveBeenCalled();

    // 手动触发第 2 次轮询：running=true
    await act(async () => { await pollCb(); });
    expect(result.current.syncRunning).toBe(true);
    expect(clearSpy).not.toHaveBeenCalled();

    // 手动触发第 3 次轮询：running=false 且此前见过 running → 停止
    await act(async () => { await pollCb(); });
    expect(result.current.syncRunning).toBe(false);
    expect(clearSpy).toHaveBeenCalled();
    expect(invalidateLibrarySessions).toHaveBeenCalled();
  });
});
