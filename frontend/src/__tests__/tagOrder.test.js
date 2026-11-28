import { sortTagsByUsage, tagUsageCount } from '../utils/tagOrder';

/**
 * 打标签时的列表顺序：最常用的标签在最上方。这组用例盯三件事——
 * 频次降序、频次相同保持后端顺序（稳定排序）、以及本次新建的标签被钉在最上方。
 */

const tag = (id, name, archive_count) => ({ id, namespace: '', name, color: '#000', archive_count });

describe('tagUsageCount', () => {
  test('读取 archive_count，缺失/非法按 0 处理', () => {
    expect(tagUsageCount(tag(1, 'a', 7))).toBe(7);
    expect(tagUsageCount({ id: 2, name: 'b' })).toBe(0);
    expect(tagUsageCount({ id: 3, name: 'c', archive_count: -4 })).toBe(0);
    expect(tagUsageCount({ id: 4, name: 'd', archive_count: 'x' })).toBe(0);
    expect(tagUsageCount(null)).toBe(0);
  });
});

describe('sortTagsByUsage', () => {
  test('按使用次数降序，且不修改入参', () => {
    const input = [tag(1, '冷门', 1), tag(2, '最常用', 42), tag(3, '中等', 7)];
    expect(sortTagsByUsage(input).map(t => t.name)).toEqual(['最常用', '中等', '冷门']);
    expect(input.map(t => t.name)).toEqual(['冷门', '最常用', '中等']);
  });

  test('次数相同的标签保持原顺序（后端已按 namespace,name 排好，别再打乱）', () => {
    const input = [tag(1, 'b', 0), tag(2, 'a', 0), tag(3, 'c', 5)];
    expect(sortTagsByUsage(input).map(t => t.id)).toEqual([3, 1, 2]);
  });

  test('没有 archive_count 的标签（旧响应/测试桩）不会顶掉有次数的标签', () => {
    const input = [tag(1, 'a'), tag(2, 'b', 3), tag(3, 'c')];
    expect(sortTagsByUsage(input).map(t => t.id)).toEqual([2, 1, 3]);
  });

  test('pinnedIds 里的标签钉在最上方（本次会话新建，频次低但用户要能看见）', () => {
    const input = [tag(1, '常用', 42), tag(2, '刚建的', 1), tag(3, '中等', 7)];
    expect(sortTagsByUsage(input, new Set([2])).map(t => t.id)).toEqual([2, 1, 3]);
    // 也接受数组，方便测试与序列化场景
    expect(sortTagsByUsage(input, [2]).map(t => t.id)).toEqual([2, 1, 3]);
  });

  test('null / 非数组输入返回空数组，不让弹窗崩', () => {
    expect(sortTagsByUsage(null)).toEqual([]);
    expect(sortTagsByUsage(undefined)).toEqual([]);
  });
});
