const express = require('express');
const cors = require('cors');
const path = require('path');
require('dotenv').config();
const logger = require('./config/logger');
const { initDatabase, getDb } = require('./db/database');
const { scanRoot } = require('./services/scanService');
const apiRoutes = require('./routes/api');
const errorHandler = require('./middleware/errorHandler');

const PORT = process.env.PORT || 5002;
let scanTimer = null;

const app = express();

// 中间件
app.use(cors());
app.use(express.json({ limit: '50mb' }));

// 前端静态文件（构建后放置在 ../frontend/build）
const staticDir = path.join(__dirname, '../../frontend/build');
app.use(express.static(staticDir));

// API 路由
app.use('/api', apiRoutes);

// 所有未匹配路由返回 index.html（SPA 支持）
app.get('*', (req, res) => {
  res.sendFile(path.join(staticDir, 'index.html'));
});

// 错误处理
app.use(errorHandler);

// 自动扫描定时器
function startAutoScanTimer() {
  clearInterval(scanTimer);
  const db = getDb();
  const row = db.prepare("SELECT value FROM settings WHERE key = 'auto_scan_interval'").get();
  const intervalMin = row ? parseInt(row.value) : 0;

  if (intervalMin <= 0) return;

  const intervalMs = intervalMin * 60 * 1000;
  logger.info(`自动扫描已启用，间隔 ${intervalMin} 分钟`);

  scanTimer = setInterval(async () => {
    try {
      const rootRow = db.prepare("SELECT value FROM settings WHERE key = 'root_dir'").get();
      const rootDir = rootRow ? rootRow.value : '';
      if (rootDir) {
        logger.info('自动扫描触发...');
        const result = await scanRoot(rootDir);
        logger.info(`自动扫描完成: ${result.message}`);
      }
    } catch (err) {
      logger.error(`自动扫描失败: ${err.message}`);
    }
  }, intervalMs);
}

// 启动
async function startServer() {
  await initDatabase();
  app.listen(PORT, () => {
    logger.info(`MangaViewer v2 运行在 http://localhost:${PORT}`);
    startAutoScanTimer();
  });
}

module.exports = { startAutoScanTimer };

startServer();
