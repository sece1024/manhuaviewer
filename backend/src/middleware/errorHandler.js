const logger = require('../config/logger');

function errorHandler(err, req, res, _next) {
  logger.error(`${req.method} ${req.url} — ${err.message}`);
  const status = err.status || 500;
  // 生产环境不暴露内部错误详情
  const message = status === 500 ? '服务器内部错误' : (err.message || '服务器内部错误');
  res.status(status).json({ error: message });
}

module.exports = errorHandler;
