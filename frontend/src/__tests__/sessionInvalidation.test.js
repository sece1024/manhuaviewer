/**
 * 用真实 api.js（不 automock）验证：影响成员集合的写操作确实推进成员代际，
 * 从而让 useLibrarySession 的会话失效判定生效。这是 Library.test.js 里用
 * mocked 版本无法覆盖的“接线是否接上”的部分。
 */
import api, { membershipGeneration, invalidateLibrarySessions } from '../utils/api';

// 真实 api.js 会发 HTTP；这里只关心失效副作用，故拦截 fetch 返回成功响应。
const okJson = (body) => Promise.resolve({ ok: true, status: 200, json: () => Promise.resolve(body) });

describe('成员代际与浏览会话失效', () => {
  beforeEach(() => {
    global.fetch = jest.fn(() => okJson({ success: true, scanned: 1 }));
  });

  test('显式失效递增代际', () => {
    const before = membershipGeneration();
    invalidateLibrarySessions();
    expect(membershipGeneration()).toBe(before + 1);
  });

  test('扫描（写操作）经 _invalidate(/archives) 自动递增代际', async () => {
    const before = membershipGeneration();
    await api.scan('/some/root', 1);
    expect(membershipGeneration()).toBe(before + 1);
  });

  test('删除档案同样递增代际', async () => {
    const before = membershipGeneration();
    await api.deleteArchive(7);
    expect(membershipGeneration()).toBe(before + 1);
  });

  test('只读请求不改变代际', async () => {
    global.fetch = jest.fn(() => okJson([]));
    const before = membershipGeneration();
    await api.getArchives({ limit: 10 });
    expect(membershipGeneration()).toBe(before);
  });
});
