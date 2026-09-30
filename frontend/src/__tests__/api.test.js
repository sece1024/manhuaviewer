/**
 * 用真实 api.js（不 automock，避免 automock 把不存在的方法也变成 jest.fn，
 * 从而掩盖“调用了未导出的方法”这类缺陷）验证客户端基础设施：
 * 导出面、LAN 口令头、GET 缓存失效、超时与错误状态码。
 */
import api, { setServerToken, getServerToken, localStorageGet } from '../utils/api';

const okJson = (body) =>
  Promise.resolve({ ok: true, status: 200, json: () => Promise.resolve(body) });
const errJson = (status, body) =>
  Promise.resolve({ ok: false, status, json: () => Promise.resolve(body) });

describe('api.js 导出面', () => {
  // App.js 曾调用 api.setServerToken（默认导出对象上不存在）→ TypeError 让口令无法保存
  test('默认导出对象上的每个成员都必须是函数（不存在的方法会在这里暴露）', () => {
    const bad = Object.entries(api)
      .filter(([, v]) => typeof v !== 'function')
      .map(([k]) => k);
    expect(bad).toEqual([]);
  });

  test('setServerToken 是具名导出，默认导出对象上确实没有它', () => {
    expect(typeof setServerToken).toBe('function');
    // 锁住约定：调用方必须用具名导入（App.js 曾误用 api.setServerToken）
    expect(api.setServerToken).toBeUndefined();
  });

  test('setServerToken 归一化并持久化到内存与 localStorage', () => {
    setServerToken('  secret-token  ');
    expect(getServerToken()).toBe('secret-token');
    expect(localStorageGet('mv_server_token')).toBe('secret-token');
    setServerToken(''); // 复位，避免污染其它用例
    expect(getServerToken()).toBe('');
  });
});

describe('api.js 请求行为', () => {
  beforeEach(() => {
    global.fetch = jest.fn(() => okJson([]));
    setServerToken('');
  });

  afterEach(() => {
    setServerToken('');
  });

  test('GET 走缓存：同 URL 第二次不再发请求', async () => {
    await api.getArchiveTags(1);
    await api.getArchiveTags(1);
    expect(global.fetch).toHaveBeenCalledTimes(1);
  });

  test('写操作作废相关缓存：scan 之后书库会重新拉取', async () => {
    global.fetch = jest.fn(() => okJson([]));
    await api.getArchives({ limit: 10 });
    const afterFirst = global.fetch.mock.calls.length;
    await api.scan('/root', 1); // 写操作应作废 /archives
    await api.getArchives({ limit: 10 });
    // 必须是“重新请求”而不是缓存命中：+1 次 scan、+1 次 getArchives
    expect(global.fetch.mock.calls.length).toBe(afterFirst + 2);
  });

  test('syncStart 作废书库与标签缓存（此前完全不作废）', async () => {
    global.fetch = jest.fn(() => okJson([]));
    await api.getTags(); // 填充 /tags 缓存
    await api.syncStart({ url: 'http://127.0.0.1:5002' });
    const before = global.fetch.mock.calls.length;
    await api.getTags();
    expect(global.fetch.mock.calls.length).toBe(before + 1); // 缓存已失效
  });

  test('错误对象携带 HTTP 状态码，供调用方区分 404 与 5xx', async () => {
    global.fetch = jest.fn(() => errJson(404, { error: 'Archive not found' }));
    await expect(api.getArchiveTags(999)).rejects.toMatchObject({ status: 404 });
    await expect(api.getArchiveTags(999)).rejects.toThrow('Archive not found');
  });

  test('5xx 对幂等请求会重试', async () => {
    global.fetch = jest.fn(() => errJson(503, { error: 'busy' }));
    await expect(api.getArchiveTags(7)).rejects.toMatchObject({ status: 503 });
    expect(global.fetch.mock.calls.length).toBeGreaterThan(1);
  });

  test('4xx 不重试（避免无谓放大请求）', async () => {
    global.fetch = jest.fn(() => errJson(404, { error: 'nope' }));
    await expect(api.getArchiveTags(7)).rejects.toBeTruthy();
    expect(global.fetch).toHaveBeenCalledTimes(1);
  });

  test('LAN 口令模式下带上 Authorization 头', async () => {
    setServerToken('lan-token');
    await api.getArchiveTags(3);
    const opts = global.fetch.mock.calls[0][1];
    expect(opts.headers.Authorization).toBe('Bearer lan-token');
  });

  test('请求带超时信号（无超时会让 _inflight 永久悬挂同一 URL）', async () => {
    await api.getArchiveTags(5);
    const opts = global.fetch.mock.calls[0][1];
    expect(opts.signal).toBeDefined();
  });
});
