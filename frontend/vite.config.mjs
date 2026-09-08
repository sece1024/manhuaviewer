import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// MangaViewer 前端（CRA → Vite）。
// dev 端口保持 3000 与 src-tauri/tauri.conf.json 的 devUrl 一致；
// /api 与 /opds 代理到本地后端（替代旧 CRA 的 package.json proxy）。
export default defineConfig({
  plugins: [react()],
  // 历史代码均为 .js 且含 JSX：让 esbuild 按 jsx 解析 src 下的 .js
  esbuild: {
    loader: 'jsx',
    include: /src\/.*\.js$/,
    exclude: [],
  },
  server: {
    port: 3000,
    strictPort: true,
    proxy: {
      '/api': 'http://127.0.0.1:5002',
      '/opds': 'http://127.0.0.1:5002',
    },
  },
  build: {
    outDir: 'build',
    emptyOutDir: true,
  },
});
