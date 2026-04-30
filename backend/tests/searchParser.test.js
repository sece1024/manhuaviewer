const { parseSearchSyntax } = require('../src/utils/searchParser');

describe('parseSearchSyntax', () => {
  test('空字符串返回空结果', () => {
    const result = parseSearchSyntax('');
    expect(result).toEqual({ textTerms: [], includeTags: [], excludeTags: [], excludeText: [] });
  });

  test('null/undefined 返回空结果', () => {
    expect(parseSearchSyntax(null)).toEqual({ textTerms: [], includeTags: [], excludeTags: [], excludeText: [] });
    expect(parseSearchSyntax(undefined)).toEqual({ textTerms: [], includeTags: [], excludeTags: [], excludeText: [] });
  });

  test('普通关键词', () => {
    const result = parseSearchSyntax('龙珠');
    expect(result.textTerms).toEqual(['龙珠']);
    expect(result.includeTags).toEqual([]);
  });

  test('多个普通关键词', () => {
    const result = parseSearchSyntax('龙珠 超');
    expect(result.textTerms).toEqual(['龙珠', '超']);
  });

  test('命名空间标签搜索 artist:xxx', () => {
    const result = parseSearchSyntax('artist:mika');
    expect(result.includeTags).toEqual(['artist:mika']);
    expect(result.textTerms).toEqual([]);
  });

  test('无命名空间标签搜索 tag:xxx', () => {
    const result = parseSearchSyntax('tag:已读');
    expect(result.includeTags).toEqual(['tag:已读']);
  });

  test('排除关键词 -keyword', () => {
    const result = parseSearchSyntax('-已读');
    expect(result.excludeText).toEqual(['已读']);
    expect(result.textTerms).toEqual([]);
  });

  test('排除标签 -tag:xxx', () => {
    const result = parseSearchSyntax('-artist:mika');
    expect(result.excludeTags).toEqual(['artist:mika']);
  });

  test('组合查询：artist:mika -已读 龙珠', () => {
    const result = parseSearchSyntax('artist:mika -已读 龙珠');
    expect(result.textTerms).toEqual(['龙珠']);
    expect(result.includeTags).toEqual(['artist:mika']);
    expect(result.excludeText).toEqual(['已读']);
    expect(result.excludeTags).toEqual([]);
  });

  test('组合查询：多标签包含和排除', () => {
    const result = parseSearchSyntax('series:naruto -tag:番外 action');
    expect(result.includeTags).toEqual(['series:naruto']);
    expect(result.excludeTags).toEqual(['tag:番外']);
    expect(result.textTerms).toEqual(['action']);
  });

  test('多余空格被忽略', () => {
    const result = parseSearchSyntax('  龙珠   超  ');
    expect(result.textTerms).toEqual(['龙珠', '超']);
  });

  test('仅冒号开头的 token 不视为标签', () => {
    const result = parseSearchSyntax(':notAtag');
    expect(result.includeTags).toEqual([]);
    expect(result.textTerms).toEqual([':notAtag']);
  });

  test('孤立的 - 符号被忽略', () => {
    const result = parseSearchSyntax('-');
    expect(result.excludeText).toEqual([]);
    expect(result.textTerms).toEqual([]);
  });
});
