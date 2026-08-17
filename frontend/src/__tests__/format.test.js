import { formatSize, formatDate, splitPathParts, lastPathPart } from '../utils/format';

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
