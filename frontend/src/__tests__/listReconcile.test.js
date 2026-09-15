import { membershipChanged, idsWithin } from '../utils/listReconcile';

describe('listReconcile 书库会话一致性比对', () => {
  test('成员相同、顺序变化 → 不刷新（阅读只改“最近阅读”排序位置，不能把用户踢回第一页）', () => {
    const saved = idsWithin([{ id: 1 }, { id: 2 }, { id: 3 }], 50);
    const freshReordered = idsWithin([{ id: 3 }, { id: 1 }, { id: 2 }], 50);
    expect(membershipChanged(saved, freshReordered)).toBe(false);
  });

  test('新增档案 → 刷新', () => {
    expect(membershipChanged(idsWithin([{ id: 1 }, { id: 2 }], 50), idsWithin([{ id: 1 }, { id: 2 }, { id: 3 }], 50))).toBe(true);
  });

  test('删除档案 → 刷新', () => {
    expect(membershipChanged(idsWithin([{ id: 1 }, { id: 2 }, { id: 3 }], 50), idsWithin([{ id: 1 }, { id: 2 }], 50))).toBe(true);
  });

  test('替换档案（同数量不同成员）→ 刷新', () => {
    expect(membershipChanged(idsWithin([{ id: 1 }, { id: 2 }], 50), idsWithin([{ id: 1 }, { id: 9 }], 50))).toBe(true);
  });

  test('窗口只取前 N 个 id，超出部分不参与比对', () => {
    const saved = idsWithin([{ id: 6 }, { id: 7 }, { id: 8 }], 2);
    expect(saved.has(6)).toBe(true);
    expect(saved.has(7)).toBe(true);
    expect(saved.has(8)).toBe(false);
  });

  test('窗口内顺序打乱 + 字段变化，仍判定为“不刷新”', () => {
    const saved = idsWithin([{ id: 1 }, { id: 2 }], 50);
    const fresh = idsWithin([{ id: 2 }, { id: 1 }], 50);
    expect(membershipChanged(saved, fresh)).toBe(false);
  });
});