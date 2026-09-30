// 本地自定义规则：调用 `api.xxx(...)` 时，校验 `xxx` 确实是 utils/api.js 默认导出
// 对象上的成员。
//
// 背景：`setServerToken` 等函数是「具名导出」，不在默认导出的 api 对象上。
// App.js 曾写 api.setServerToken(...)，运行时抛 TypeError，而 eslint 内置的
// no-undef 只能发现「未声明的标识符」，无法发现「对象上不存在的属性」，
// 因此这类缺陷能一路逃到用户面前。此规则把该契约变成可静态检查的。
import fs from 'node:fs';
import path from 'node:path';

/** 从 api.js 源码里取出默认导出对象的方法名集合。 */
function readApiMembers(apiFilePath) {
  const src = fs.readFileSync(apiFilePath, 'utf8');
  const start = src.indexOf('const api = {');
  if (start === -1) return null;
  const braceStart = src.indexOf('{', start);
  let depth = 0;
  let braceEnd = -1;
  for (let i = braceStart; i < src.length; i++) {
    if (src[i] === '{') depth++;
    else if (src[i] === '}') {
      depth--;
      if (depth === 0) { braceEnd = i; break; }
    }
  }
  if (braceEnd === -1) return null;
  const body = src.slice(braceStart, braceEnd + 1);
  const members = new Set();
  const re = /^\s{2}(?:async\s+)?([A-Za-z_$][\w$]*)\s*:/gm;
  let m;
  while ((m = re.exec(body)) !== null) members.add(m[1]);
  // 展开语法（如 ...）无法静态枚举，标记为不可判定
  if (/^\s{2}\.\.\./m.test(body)) return null;
  return members;
}

export default {
  meta: {
    type: 'problem',
    docs: {
      description:
        'api.<member> 必须是 utils/api.js 默认导出对象上的成员（具名导出的函数不在其中）',
    },
    schema: [],
    messages: {
      unknownMember:
        "api.{{name}} 不在 utils/api.js 的默认导出对象上。若它是具名导出，请改用具名导入，例如 import api, { {{name}} } from '<路径>'。",
    },
  },
  create(context) {
    const filename = context.getFilename();
    if (!filename || filename.includes('utils/api.js')) return {};

    // 定位 api.js：从当前文件向上找到 utils/api.js
    const apiPath = path.resolve(path.dirname(filename), '..', 'utils', 'api.js');
    if (!fs.existsSync(apiPath)) return {};
    const members = readApiMembers(apiPath);
    if (!members) return {};

    const apiLocalNames = new Set();
    return {
      ImportDeclaration(node) {
        const src = node.source.value;
        if (!src || !src.endsWith('utils/api') && !src.endsWith('/api')) return;
        if (!/utils\/api$|^\.\.?\/.*\/api$|^\.\.?\/api$/.test(src)) return;
        for (const spec of node.specifiers) {
          // 默认导入：`import api from '../utils/api'`
          if (spec.type === 'ImportDefaultSpecifier') apiLocalNames.add(spec.local.name);
        }
      },
      MemberExpression(node) {
        if (node.computed) return;
        if (node.object.type !== 'Identifier') return;
        if (!apiLocalNames.has(node.object.name)) return;
        if (node.property.type !== 'Identifier') return;
        const prop = node.property.name;
        if (!members.has(prop)) {
          context.report({ node: node.property, messageId: 'unknownMember', data: { name: prop } });
        }
      },
    };
  },
};
