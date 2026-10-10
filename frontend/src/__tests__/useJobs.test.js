import { renderHook, act } from '@testing-library/react';
import useJobs, { resetJobsStore } from '../hooks/useJobs';

jest.mock('../utils/api');
const api = require('../utils/api').default;

// 任务层从 api 具名导入 invalidateLibrarySessions；automock 会让它变成 no-op，
// 这里拿真实实现来断言「任务结束会作废浏览会话」。
const { invalidateLibrarySessions } = require('../utils/api');

/**
 * 这组用例覆盖的是原先写在 useSync 里的轮询竞态回归。状态所有者搬到任务层
 * （useJobs）之后，回归点也一并搬过来：任务层是唯一的轮询者。
 */
describe('useJobs 任务层：轮询与结束判定', () => {
  let pollCb; // 拦截 setInterval 捕获到的轮询回调
  let clearSpy;

  beforeEach(() => {
    resetJobsStore();
    jest.clearAllMocks();
    // 任务层里的 1s 定时器不真实计时，手动驱动，异步状态机才完全确定
    pollCb = null;
    clearSpy = jest.fn();
    jest.spyOn(global, 'setInterval').mockImplementation((cb) => { pollCb = cb; return 123; });
    jest.spyOn(global, 'clearInterval').mockImplementation(clearSpy);
  });

  afterEach(() => {
    jest.restoreAllMocks();
    resetJobsStore();
  });

  test('启动瞬间 status 尚未置 running=true 不会误判完成（回归）', async () => {
    // 首轮水合：三个任务都空闲（不消耗下面的状态序列）
    api.scanStatus.mockResolvedValue({ running: false });
    api.convertCbzStatus.mockResolvedValue({ running: false });
    api.syncStatus.mockResolvedValue({ running: false, total: 0, done: 0, current: '' });
    api.syncStart.mockResolvedValue({ started: true });

    const { result } = renderHook(() => useJobs());
    await act(async () => {}); // 让水合落地

    // 启动后：第 1 次后端还没置 running；第 2 次 running=true；第 3 次真正结束
    let call = 0;
    const seq = [
      { running: false, total: 0, done: 0, current: '排队中' },
      { running: true, total: 10, done: 3, current: '下载中' },
      { running: false, total: 10, done: 10, current: '完成' },
    ];
    api.syncStatus.mockImplementation(() => Promise.resolve(seq[Math.min(call++, seq.length - 1)]));

    await act(async () => {
      await result.current.startSync({ url: 'http://192.168.1.9:5002', dir: '/data/sync' });
    });

    // startSync 内部会顺手 poll 一次（seq[0] = running:false）：不能就此判完成
    expect(call).toBe(1);
    expect(result.current.jobs.sync.running).toBe(true);
    expect(result.current.jobs.sync.finishedAt).toBe(0);
    expect(clearSpy).not.toHaveBeenCalled();

    // 第 2 次轮询：running=true —— 仍然在跑
    await act(async () => { await pollCb(); });
    expect(result.current.jobs.sync.running).toBe(true);
    expect(result.current.jobs.sync.finishedAt).toBe(0);
    expect(clearSpy).not.toHaveBeenCalled();

    // 第 3 次轮询：running=false 且此前见过 running → 判定结束，只计一次完成
    await act(async () => { await pollCb(); });
    expect(result.current.jobs.sync.running).toBe(false);
    expect(result.current.jobs.sync.finishedAt).toBe(1);
    expect(invalidateLibrarySessions).toHaveBeenCalled();
    expect(clearSpy).toHaveBeenCalled();
  });

  test('空闲时水合两次也不会计入完成，回到页面不会补提示', async () => {
    api.scanStatus.mockResolvedValue({ running: false, total: 0, done: 0, current: '' });
    api.convertCbzStatus.mockResolvedValue({ running: false, total: 0, done: 0, current: '' });
    api.syncStatus.mockResolvedValue({ running: false, total: 0, done: 0, current: '' });

    const { result } = renderHook(() => useJobs());
    await act(async () => {});

    expect(result.current.anyRunning).toBe(false);
    expect(result.current.jobs.scan.finishedAt).toBe(0);
    expect(result.current.jobs.sync.finishedAt).toBe(0);
    expect(result.current.jobs.convert.finishedAt).toBe(0);
    expect(invalidateLibrarySessions).not.toHaveBeenCalled();
  });

  test('水合时若任务已在跑：立刻可见，并且结束后计一次完成', async () => {
    api.scanStatus.mockResolvedValue({ running: true, total: 4, done: 1, current: '扫描中', added: 1 });
    api.convertCbzStatus.mockResolvedValue({ running: false });
    api.syncStatus.mockResolvedValue({ running: false });

    const { result } = renderHook(() => useJobs());
    await act(async () => {});

    // 别的页面/上一个会话启动的任务，进入任意页面都能看到
    expect(result.current.jobs.scan.running).toBe(true);
    expect(result.current.jobs.scan.total).toBe(4);
    expect(result.current.anyRunning).toBe(true);

    api.scanStatus.mockResolvedValue({ running: false, total: 4, done: 4, current: '' });
    await act(async () => { await pollCb(); });
    expect(result.current.jobs.scan.running).toBe(false);
    expect(result.current.jobs.scan.finishedAt).toBe(1);
  });

  test('扫描以自身 POST 返回为结束信号：结束后不会因轮询再计一次完成', async () => {
    api.syncStatus.mockResolvedValue({ running: false });
    api.convertCbzStatus.mockResolvedValue({ running: false });
    // 扫描：POST 一直挂到任务结束才返回（这里立即返回一个汇总）
    api.scan.mockResolvedValue({ message: '扫描完成：新增 2，清理 1', added: 2, removed: 1 });
    api.scanStatus.mockResolvedValue({ running: false, total: 0, done: 0, current: '' });

    const { result } = renderHook(() => useJobs());
    await act(async () => {});

    await act(async () => {
      await result.current.startScan('/lib', 2);
    });

    expect(result.current.jobs.scan.running).toBe(false);
    expect(result.current.jobs.scan.finishedAt).toBe(1);
    expect(result.current.jobs.scan.result.message).toBe('扫描完成：新增 2，清理 1');
  });

  test('批量转 CBZ 在 total=0 时不进入任务态（否则面板会白闪一下）', async () => {
    api.scanStatus.mockResolvedValue({ running: false });
    api.syncStatus.mockResolvedValue({ running: false });
    api.convertCbzStatus.mockResolvedValue({ running: false });
    api.convertCbzStart.mockResolvedValue({ started: false, total: 0 });

    const { result } = renderHook(() => useJobs());
    await act(async () => {});

    let returned;
    await act(async () => {
      returned = await result.current.startConvert([]);
    });

    expect(returned.total).toBe(0);
    expect(result.current.jobs.convert.running).toBe(false);
    expect(result.current.jobs.convert.finishedAt).toBe(0);
  });
});
