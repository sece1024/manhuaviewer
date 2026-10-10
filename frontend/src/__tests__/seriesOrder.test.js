import { seriesKey, naturalCompare, nextInSeries } from '../utils/seriesOrder';

const item = (id, title, path = `/lib/系列/${title}.cbz`) => ({ id, title, path });

describe('seriesKey', () => {
  test('剥掉尾部卷号，让同一系列收敛到同一个键', () => {
    expect(seriesKey('系列 01')).toBe('系列');
    expect(seriesKey('系列 02')).toBe('系列');
    expect(seriesKey('系列 10')).toBe('系列');
  });

  test('常见卷/话标记都能剥掉', () => {
    expect(seriesKey('航海王 vol.10')).toBe('航海王');
    expect(seriesKey('航海王 v2')).toBe('航海王');
    expect(seriesKey('某某 第3卷')).toBe('某某');
    expect(seriesKey('某某 第 12 話')).toBe('某某');
    expect(seriesKey('某某 ch12')).toBe('某某');
    expect(seriesKey('某某 (4)')).toBe('某某');
    expect(seriesKey('某某 [7]')).toBe('某某');
  });

  test('不把普通单词的尾字母当罗马数字卷号（Panic 的 c 不能被剥掉）', () => {
    expect(seriesKey('Panic')).toBe('panic');
    expect(seriesKey('xxxHOLiC')).toBe('xxxholic');
  });

  test('标题本身就是编号时退回原标题，避免不同系列都归到空键', () => {
    expect(seriesKey('86')).toBe('86');
    expect(seriesKey('Vol.1')).toBe('vol.1');
  });

  test('大小写与空值', () => {
    expect(seriesKey('One Piece 01')).toBe('one piece');
    expect(seriesKey('')).toBe('');
    expect(seriesKey(null)).toBe('');
    expect(seriesKey(undefined)).toBe('');
  });

  test('不同系列得到不同的键', () => {
    expect(seriesKey('系列A 01')).not.toBe(seriesKey('系列B 01'));
  });
});

describe('naturalCompare', () => {
  test('按数值比较，10 排在 2 之后', () => {
    expect(naturalCompare('系列 2', '系列 10')).toBeLessThan(0);
    expect(naturalCompare('系列 10', '系列 2')).toBeGreaterThan(0);
  });

  test('前缀更短的排前面', () => {
    expect(naturalCompare('系列', '系列 01')).toBeLessThan(0);
  });

  test('相同返回 0', () => {
    expect(naturalCompare('系列 01', '系列 01')).toBe(0);
  });
});

describe('nextInSeries', () => {
  const siblings = [item(1, '系列 01'), item(2, '系列 02'), item(3, '系列 10'), item(4, '别的书 01')];

  test('按自然序给出下一卷（02 之后是 10，而不是按字符串序）', () => {
    expect(nextInSeries(siblings, 2).id).toBe(3);
  });

  test('第一卷的下一卷是第二卷', () => {
    expect(nextInSeries(siblings, 1).id).toBe(2);
  });

  test('已是同系列最后一卷时返回 null（不环回第一卷）', () => {
    expect(nextInSeries(siblings, 3)).toBeNull();
  });

  test('同目录下不同系列不参与串联', () => {
    // 「别的书 01」的下一卷不存在，而不是接到「系列 01」
    expect(nextInSeries(siblings, 4)).toBeNull();
  });

  test('当前档案不在列表里（或列表不可用）时返回 null', () => {
    expect(nextInSeries(siblings, 999)).toBeNull();
    expect(nextInSeries(undefined, 1)).toBeNull();
    expect(nextInSeries(null, 1)).toBeNull();
  });

  test('只有一个成员时没有下一卷', () => {
    expect(nextInSeries([item(1, '系列 01')], 1)).toBeNull();
  });

  test('同标题重复下载时按路径兜底，顺序稳定', () => {
    const dup = [
      { id: 1, title: '系列 01', path: '/lib/系列/a.cbz' },
      { id: 2, title: '系列 01', path: '/lib/系列/b.cbz' },
    ];
    expect(nextInSeries(dup, 1).id).toBe(2);
    expect(nextInSeries(dup, 2)).toBeNull();
  });
});
