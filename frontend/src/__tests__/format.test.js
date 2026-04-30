import { formatSize, formatDate } from '../utils/format';

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
