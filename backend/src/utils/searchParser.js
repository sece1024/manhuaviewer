/**
 * searchParser.js — 搜索语法解析器（独立模块，方便测试）
 *
 * 支持格式:
 *   "keyword"         — 普通文本搜索
 *   "tag:xxx"         — 搜索标签（无命名空间）
 *   "artist:xxx"      — 搜索命名空间标签
 *   "-tag:xxx"        — 排除标签
 *   "-keyword"        — 排除关键词
 *   以上可组合使用，如 "artist:mika -已读"
 */
function parseSearchSyntax(searchStr) {
  const textTerms = [];
  const includeTags = [];
  const excludeTags = [];
  const excludeText = [];

  if (!searchStr) return { textTerms, includeTags, excludeTags, excludeText };

  const tokens = searchStr.split(/\s+/).filter(Boolean);
  for (const token of tokens) {
    if (token.startsWith('-')) {
      const inner = token.slice(1);
      if (inner.includes(':')) {
        excludeTags.push(inner);
      } else if (inner) {
        excludeText.push(inner);
      }
    } else if (token.includes(':') && !token.startsWith(':')) {
      includeTags.push(token);
    } else {
      textTerms.push(token);
    }
  }

  return { textTerms, includeTags, excludeTags, excludeText };
}

module.exports = { parseSearchSyntax };
