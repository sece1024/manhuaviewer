/**
 * scanService.js — 扫描根目录，发现漫画（文件夹 + 压缩包）
 */
const fs = require('fs');
const path = require('path');
const { getDb } = require('../db/database');
const archiveService = require('./archiveService');
const logger = require('../config/logger');

/**
 * 扫描根目录，更新数据库
 */
async function scanRoot(rootDir) {
  if (!rootDir || !fs.existsSync(rootDir)) {
    throw new Error('根目录未配置或不存在');
  }

  const db = getDb();
  const entries = fs.readdirSync(rootDir, { withFileTypes: true });
  let scanned = 0;

  const SUPPORTED_IMG = ['.jpg', '.jpeg', '.png', '.bmp', '.webp', '.gif', '.tiff', '.avif'];

  for (const entry of entries) {
    if (entry.name.startsWith('.')) continue;

    const fullPath = path.join(rootDir, entry.name);

    if (entry.isDirectory()) {
      // 扫描子文件夹（文件夹类型的漫画）
      const files = fs.readdirSync(fullPath)
        .filter(f => SUPPORTED_IMG.includes(path.extname(f).toLowerCase()))
        .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));

      if (files.length === 0) continue;

      // 计算总大小
      let totalSize = 0;
      for (const f of files) {
        try { totalSize += fs.statSync(path.join(fullPath, f)).size; } catch {}
      }

      // 插入或更新
      const existing = db.prepare('SELECT id FROM archives WHERE path = ?').get(fullPath);
      if (existing) {
        db.prepare('UPDATE archives SET title = ?, page_count = ?, file_size = ?, updated_at = datetime(\'now\') WHERE id = ?')
          .run(entry.name, files.length, totalSize, existing.id);
      } else {
        db.prepare(`INSERT INTO archives (title, path, archive_type, page_count, file_size)
          VALUES (?, ?, 'folder', ?, ?)`).run(entry.name, fullPath, files.length, totalSize);
      }

      const archive = db.prepare('SELECT id FROM archives WHERE path = ?').get(fullPath);

      // 异步生成封面
      archiveService.extractFolderCover(fullPath, archive.id).catch(() => {});
      scanned++;

    } else if (entry.isFile() && archiveService.isArchive(entry.name)) {
      // 压缩包文件
      const ext = path.extname(entry.name).toLowerCase().replace('.', '');
      const archiveType = ext === 'cbz' ? 'zip' : ext === 'cbr' ? 'rar' : ext;

      let fileSize = 0;
      try { fileSize = fs.statSync(fullPath).size; } catch {}

      let pageCount = 0;
      try {
        const images = await archiveService.getImageList(fullPath);
        pageCount = images.length;
      } catch (err) {
        logger.warn(`无法读取压缩包: ${fullPath} — ${err.message}`);
        continue;
      }

      if (pageCount === 0) continue;

      const title = path.basename(entry.name, path.extname(entry.name));
      const existing = db.prepare('SELECT id FROM archives WHERE path = ?').get(fullPath);
      let archiveId;

      if (existing) {
        db.prepare('UPDATE archives SET title = ?, page_count = ?, file_size = ?, archive_type = ?, updated_at = datetime(\'now\') WHERE id = ?')
          .run(title, pageCount, fileSize, archiveType, existing.id);
        archiveId = existing.id;
      } else {
        const info = db.prepare(`INSERT INTO archives (title, path, archive_type, page_count, file_size)
          VALUES (?, ?, ?, ?, ?)`).run(title, fullPath, archiveType, pageCount, fileSize);
        archiveId = info.lastInsertRowid;
      }

      // 更新 pages 表
      db.prepare('DELETE FROM pages WHERE archive_id = ?').run(archiveId);
      try {
        const images = await archiveService.getImageList(fullPath);
        const insertPage = db.prepare(
          'INSERT INTO pages (archive_id, filename, filepath, sort_order, file_size) VALUES (?, ?, ?, ?, ?)'
        );
        const insertAll = db.transaction(() => {
          for (let i = 0; i < images.length; i++) {
            insertPage.run(archiveId, images[i].name, images[i].path, i, images[i].size);
          }
        });
        insertAll();
      } catch {}

      // 生成封面
      archiveService.extractCover(fullPath, archiveId).catch(() => {});
      scanned++;
    }
  }

  logger.info(`扫描完成: 发现 ${scanned} 个漫画`);
  return { scanned, message: `扫描完成，发现 ${scanned} 个漫画` };
}

module.exports = { scanRoot };
