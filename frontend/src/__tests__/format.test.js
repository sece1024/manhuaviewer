import { formatSize, formatDate, formatDateShort, splitPathParts, lastPathPart } from '../utils/format';

describe('formatSize', () => {
  test('空值返回空字符串', () => {
    expect(formatSize(0)).toBe('');
    expect(formatSize(null)).toBe('');
    expect(formatSize(undefined)).toBe('');
  });

  test('字节单位', () => {
    expect(formatSize(500)).toBe('500 B');
  });

  test('KB 单位', () => {
    expect(formatSize(1536)).toBe('1.5 KB');
  });

  test('MB 单位', () => {
    expect(formatSize(1048576)).toBe('1.0 MB');
  });

  test('GB 单位', () => {
    expect(formatSize(1073741824)).toBe('1.00 GB');
  });
});

describe('formatDate', () => {
  test('空值返回空字符串', () => {
    expect(formatDate('')).toBe('');
    expect(formatDate(null)).toBe('');
  });

  test('格式化有效日期', () => {
    const result = formatDate('2026-04-30T12:00:00');
    expect(result).toContain('2026');
    expect(result).toContain('04');
    expect(result).toContain('30');
  });

  test('无效日期返回原值', () => {
    expect(formatDate('not-a-date')).toBe('not-a-date');
  });
});

// 后端时间列由 SQLite datetime('now') 写入 = UTC 无时区标记；
// 下列断言全部用「同一时刻的两种写法结果一致」的形式，与运行机时区无关。
describe('数据库时间的 UTC 解析', () => {
  test('naive 空格分隔与 T 分隔 + Z 是同一时刻', () => {
    expect(formatDate('2026-04-30 12:00:00')).toBe(formatDate('2026-04-30T12:00:00Z'));
    expect(formatDateShort('2026-04-30 12:00:00')).toBe(formatDateShort('2026-04-30T12:00:00Z'));
  });

  test('已带时区标记的字符串不做二次修正', () => {
    // 2026-04-30T04:00:00Z 与 2026-04-30T12:00:00+08:00 是同一时刻
    expect(formatDate('2026-04-30T12:00:00+08:00')).toBe(formatDate('2026-04-30T04:00:00Z'));
  });

  test('formatDateShort 输出本地时区的 YYYY-MM-DD', () => {
    const full = formatDate('2026-04-30T12:00:00Z');
    expect(formatDateShort('2026-04-30T12:00:00Z')).toBe(full.slice(0, 10));
  });

  test('空值与无效输入的契约与 formatDate 一致', () => {
    expect(formatDateShort('')).toBe('');
    expect(formatDateShort(null)).toBe('');
    expect(formatDateShort('not-a-date')).toBe('not-a-date');
  });
});

describe('splitPathParts', () => {
  test('Windows 反斜杠路径', () => {
    expect(splitPathParts('C:\\Manga\\Title\\Chapter1')).toEqual(['C:', 'Manga', 'Title', 'Chapter1']);
  });

  test('Unix 正斜杠路径', () => {
    expect(splitPathParts('/Manga/Title/Chapter1')).toEqual(['Manga', 'Title', 'Chapter1']);
  });

  test('混合分隔符与尾部斜杠', () => {
    expect(splitPathParts('Manga\\Title/Chapter1\\')).toEqual(['Manga', 'Title', 'Chapter1']);
  });

  test('空值', () => {
    expect(splitPathParts('')).toEqual([]);
    expect(splitPathParts(null)).toEqual([]);
  });
});

describe('lastPathPart', () => {
  test('目录名（子目录）', () => {
    expect(lastPathPart('/manhua/海贼王/01')).toBe('01');
  });

  test('压缩包剥扩展名', () => {
    expect(lastPathPart('C:\\Manga\\01.cbz')).toBe('01');
    expect(lastPathPart('/manhua/chapter02.zip')).toBe('chapter02');
  });

  test('文件夹名保留点号（stripExtension=false）', () => {
    expect(lastPathPart('/manhua/海贼王/01.5', false)).toBe('01.5');
    expect(lastPathPart('/manhua/海贼王/01')).toBe('01');
  });

  test('空值', () => {
    expect(lastPathPart('')).toBe('');
    expect(lastPathPart(null)).toBe('');
  });
});
