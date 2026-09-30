// ESLint 配置（flat config，ESLint 8.57+ 支持；9.x 亦兼容）。
//
// 目的：给前端补上静态检查门禁。此前前端没有任何 ESLint 配置，CI 的 build 步骤
// 只做编译（Vite 不内置 lint），因此像「调用未导出的 api.setServerToken」这类
// 错误只能靠运行时才发现。
//
// 规则取向：只开「真实缺陷」类规则，不引入风格规则（项目现有代码风格由约定维持，
// 一次性引入风格规则会产生大量无意义改动）。no-undef 是这里最关键的一条。
import react from 'eslint-plugin-react';
import reactHooks from 'eslint-plugin-react-hooks';
import apiMembers from './eslint-rules/api-members.mjs';

export default [
  {
    ignores: ['build/**', 'node_modules/**', 'coverage/**'],
  },
  {
    files: ['src/**/*.{js,jsx}'],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      parserOptions: {
        ecmaFeatures: { jsx: true },
      },
      globals: {
        // 浏览器
        window: 'readonly', document: 'readonly', navigator: 'readonly',
        localStorage: 'readonly', sessionStorage: 'readonly', location: 'readonly',
        history: 'readonly', fetch: 'readonly', console: 'readonly',
        setTimeout: 'readonly', clearTimeout: 'readonly',
        setInterval: 'readonly', clearInterval: 'readonly',
        requestAnimationFrame: 'readonly', cancelAnimationFrame: 'readonly',
        IntersectionObserver: 'readonly', ResizeObserver: 'readonly',
        Image: 'readonly', AbortController: 'readonly', CustomEvent: 'readonly',
        Event: 'readonly', KeyboardEvent: 'readonly', MouseEvent: 'readonly',
        TouchEvent: 'readonly', WheelEvent: 'readonly', DragEvent: 'readonly',
        URLSearchParams: 'readonly', Blob: 'readonly',
        FileReader: 'readonly', alert: 'readonly', matchMedia: 'readonly',
        getComputedStyle: 'readonly', DOMException: 'readonly',
        MutationObserver: 'readonly', performance: 'readonly',
        process: 'readonly',
      },
    },
    plugins: {
      react,
      'react-hooks': reactHooks,
      local: { rules: { 'api-members': apiMembers } },
    },
    settings: { react: { version: 'detect' } },
    rules: {
      // —— 真实缺陷类（error）——
      'no-undef': 'error', // 引用不存在的变量/未导入的绑定
      'no-unused-vars': ['error', {
        args: 'after-used',
        ignoreRestSiblings: true,
        varsIgnorePattern: '^_',
        argsIgnorePattern: '^_',
      }],
      'no-dupe-keys': 'error',
      'no-dupe-args': 'error',
      'no-dupe-class-members': 'error',
      'no-unreachable': 'error',
      'no-cond-assign': ['error', 'except-parens'],
      'no-constant-condition': ['error', { checkLoops: false }],
      'no-self-assign': 'error',
      'no-self-compare': 'error',
      'no-unsafe-negation': 'error',
      'no-useless-escape': 'error',
      'no-empty': ['error', { allowEmptyCatch: true }],
      'valid-typeof': 'error',
      'use-isnan': 'error',
      'no-fallthrough': 'error',
      'no-redeclare': 'error',
      'no-obj-calls': 'error',
      'no-sparse-arrays': 'error',
      'require-atomic-updates': 'off', // 误报较多，且本项目无相关模式

      // —— React ——
      'react/jsx-uses-react': 'off',      // React 19 + 新 JSX 转换，无需 import React
      'react/jsx-uses-vars': 'error',     // 避免 JSX 中使用的组件被判为未使用
      'react/jsx-key': 'error',           // 列表缺 key
      'react/jsx-no-duplicate-props': 'error',
      'react/jsx-no-undef': 'error',      // JSX 中引用未定义组件
      // 自定义：api.<x> 必须存在于 utils/api.js 的默认导出对象上
      // （内置 no-undef 只能发现未声明的标识符，发现不了不存在的属性）
      'local/api-members': 'error',
      'react/no-children-prop': 'error',
      'react/no-unknown-property': 'error',
      'react-hooks/rules-of-hooks': 'error', // hooks 调用位置错误
      // 依赖数组问题：本项目大量使用 ref 读取 + 刻意省略依赖（见 Reader/Library），
      // 一次性开启会产生成百条告警，故先设为 warn 作为提示，不阻塞 CI。
      'react-hooks/exhaustive-deps': 'warn',
    },
  },
  {
    // 测试文件：声明 jest 全局
    files: ['src/**/__tests__/**/*.js', 'src/**/*.test.js', 'src/setupTests.js'],
    languageOptions: {
      globals: {
        jest: 'readonly', describe: 'readonly', test: 'readonly', it: 'readonly',
        expect: 'readonly', beforeEach: 'readonly', afterEach: 'readonly',
        beforeAll: 'readonly', afterAll: 'readonly', require: 'readonly',
        global: 'readonly', module: 'readonly',
      },
    },
  },
];
