const express = require('express');
const cors = require('cors');
const path = require('path');
require('dotenv').config();
const logger = require('./config/logger');
const { initDatabase, getDb } = require('./db/database');
const apiRoutes = require('./routes/api');
const errorHandler = require('./middleware/errorHandler');
const { startAutoScanTimer } = require('./services/scanTimer');

const PORT = process.env.PORT || 5002;

const app = express();

// 中间件
app.use(cors());
app.use(express.json({ limit: '50mb' }));

// 前端静态文件（构建后放置在 ../frontend/build）
const staticDir = path.join(__dirname, '../../frontend/build');
app.use(express.static(staticDir));

// API 路由
app.use('/api', apiRoutes);

// OPDS 路由（根路径，第三方阅读器需要）
const opdsRoutes = require('./routes/opdsRoutes');
app.use('/', opdsRoutes);

// 所有未匹配路由返回 index.html（SPA 支持）
app.get('*', (req, res) => {
  res.sendFile(path.join(staticDir, 'index.html'));
});

// 错误处理
app.use(errorHandler);

// 启动
async function startServer() {
  await initDatabase();
  app.listen(PORT, () => {
    logger.info(`MangaViewer v2 运行在 http://localhost:${PORT}`);
    startAutoScanTimer();
  });
}

startServer();
