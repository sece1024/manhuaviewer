const express = require('express');
const cors = require('cors');
const helmet = require('helmet');
const path = require('path');
const fs = require('fs');
require('dotenv').config();
const logger = require('./config/logger');
const { initDatabase, getDb, setDataDir } = require('./db/database');
const apiRoutes = require('./routes/api');
const errorHandler = require('./middleware/errorHandler');
const { startAutoScanTimer } = require('./services/scanTimer');

const app = express();

// 安全中间件
app.use(helmet({
  contentSecurityPolicy: false,   // 允许内联 SVG 封面
  crossOriginEmbedderPolicy: false, // 允许加载跨域图片
}));

// 中间件
app.use(cors());
app.use(express.json({ limit: '10mb' }));

// 前端静态文件（构建后放置在 ../frontend/build）
const staticDir = path.join(__dirname, '../../frontend/build');
app.use(express.static(staticDir));

// API 路由
app.use('/api', apiRoutes);

// OPDS 路由（根路径，第三方阅读器需要）
const opdsRoutes = require('./routes/opdsRoutes');
app.use('/', opdsRoutes);

// 所有未匹配路由返回 index.html（SPA 支持）
const indexHtml = path.join(staticDir, 'index.html');
app.get('*', (req, res) => {
  if (fs.existsSync(indexHtml)) {
    res.sendFile(indexHtml);
  } else {
    res.status(404).json({ error: '前端未构建，请先运行 npm run build' });
  }
});

// 错误处理
app.use(errorHandler);

// 启动服务器，返回 Promise 供 Electron 等外部调用
async function startServer(port) {
  // 支持通过环境变量设置数据目录（Electron 使用）
  if (process.env.DATA_DIR) {
    setDataDir(process.env.DATA_DIR);
  }

  await initDatabase();

  const listenPort = port || process.env.PORT || 5002;
  return new Promise((resolve) => {
    const server = app.listen(listenPort, () => {
      logger.info(`MangaViewer v2 运行在 http://localhost:${listenPort}`);
      startAutoScanTimer();
      resolve(server);
    });
  });
}

// 直接运行时启动服务器
if (require.main === module) {
  startServer();
}

module.exports = { app, startServer };
