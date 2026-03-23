# MangaViewer Web 版

基于 React + Express 的前后端分离漫画阅读器，可通过浏览器访问。

## 快速开始

```bash
# 安装依赖
npm install
cd backend && npm install
cd ../frontend && npm install

# 开发模式（前后端同时启动）
cd .. && npm start

# 或者分别启动
cd backend && npm run dev    # 后端 http://localhost:5002
cd frontend && npm start     # 前端 http://localhost:3000（代理到 5002）

# 生产构建
cd frontend && npm run build
cd ../backend && npm start   # 单端口启动，托管前端构建产物
```

## 功能

- 📚 漫画文件夹管理（扫描、搜索）
- 📖 单页/双页/长图阅读模式
- 🔍 缩放、旋转、拖拽
- ⚡ 预加载 + 缩略图
- 🏷️ 标签分类
- 📋 阅读历史与进度记忆
- 🎨 主题切换（浅色/深色/护眼）
- ⌨️ 完整快捷键支持

## 快捷键

| 按键 | 功能 |
|------|------|
| ← / A | 上一页 |
| → / Space | 下一页 |
| D | 切换双页模式 |
| L | 切换长图模式 |
| R / Shift+R | 旋转 |
| T | 缩略图总览 |
| G | 跳转到页 |
| F11 | 全屏 |
| Home / End | 首尾页 |

## 项目结构

```
manhuaviewer/
├── backend/          # Express 后端
│   └── src/
│       ├── index.js          # 入口
│       ├── db/database.js    # SQLite 数据库
│       └── routes/           # API 路由
├── frontend/         # React 前端
│   └── src/
│       ├── App.js            # 路由 + 布局
│       ├── pages/            # 页面组件
│       ├── components/       # 通用组件
│       └── utils/api.js      # API 封装
└── doc/              # 文档
```

## API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | /api/config | 获取根目录配置 |
| PUT | /api/config | 更新根目录 |
| POST | /api/scan | 扫描漫画文件夹 |
| GET | /api/folders | 文件夹列表 |
| GET | /api/folders/:id/images | 图片列表 |
| GET | /api/images/:id | 获取原图 |
| GET | /api/images/:id/thumbnail | 获取缩略图 |
| GET/POST/DELETE | /api/history | 阅读历史 |
| GET/POST/PUT/DELETE | /api/tags | 标签管理 |
| GET/PUT | /api/settings | 用户设置 |
