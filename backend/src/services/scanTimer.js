/**
 * scanTimer.js — 自动扫描定时器（独立模块，避免循环依赖）
 */
const { getDb } = require('../db/database');
const { scanRoot } = require('./scanService');
const logger = require('../config/logger');

let scanTimer = null;

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

module.exports = { startAutoScanTimer };
