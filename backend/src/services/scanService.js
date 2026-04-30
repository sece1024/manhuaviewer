/**
 * scanService.js — 扫描根目录，发现漫画（文件夹 + 压缩包）
 * 支持可配置的递归扫描深度
 */
const fs = require('fs');
const path = require('path');
const { getDb } = require('../db/database');
const archiveService = require('./archiveService');
const logger = require('../config/logger');

/**
 * 扫描单个目录，返回发现的漫画条目（文件夹 + 压缩包）
 */
async function scanDir(dirPath, depth, maxDepth) {
  const results = [];
  let entries;
  try {
    entries = fs.readdirSync(dirPath, { withFileTypes: true });
  } catch (e) {
    logger.debug(`读取目录失败: ${dirPath} — ${e.message}`);
    return results;
  }

  for (const entry of entries) {
    if (entry.name.startsWith('.')) continue;

    const fullPath = path.join(dirPath, entry.name);

    if (entry.isDirectory()) {
      // 检查当前子目录是否包含图片（作为漫画文件夹）
      const files = fs.readdirSync(fullPath)
        .filter(f => archiveService.isImage(f))
        .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));

      if (files.length > 0) {
        // 当前目录包含图片，注册为漫画
        let totalSize = 0;
        for (const f of files) {
          try { totalSize += fs.statSync(path.join(fullPath, f)).size; } catch {}
        }
        results.push({ type: 'folder', path: fullPath, name: entry.name, pageCount: files.length, fileSize: totalSize });
      }

      // 如果还没达到最大深度，继续递归扫描子目录
      if (depth < maxDepth) {
        const subResults = await scanDir(fullPath, depth + 1, maxDepth);
        results.push(...subResults);
      }

    } else if (entry.isFile() && archiveService.isArchive(entry.name)) {
      // 压缩包文件
      let fileSize = 0;
      try { fileSize = fs.statSync(fullPath).size; } catch {}

      let images;
      try {
        images = await archiveService.getImageList(fullPath);
      } catch (err) {
        logger.warn(`无法读取压缩包: ${fullPath} — ${err.message}`);
        continue;
      }

      if (images.length === 0) continue;

      const ext = path.extname(entry.name).toLowerCase().replace('.', '');
      const archiveType = ext === 'cbz' ? 'zip' : ext === 'cbr' ? 'rar' : ext;
      const title = path.basename(entry.name, path.extname(entry.name));
      results.push({ type: 'archive', path: fullPath, name: title, archiveType, images, fileSize });
    }
  }

  return results;
}

/**
 * 扫描根目录，更新数据库
 * @param {string} rootDir - 根目录路径
 * @param {number} maxDepth - 最大递归深度（从 settings 读取）
 */
async function scanRoot(rootDir, maxDepth) {
  if (!rootDir || !fs.existsSync(rootDir)) {
    throw new Error('根目录未配置或不存在');
  }

  // 如果未传入 maxDepth，从数据库读取
  if (maxDepth === undefined) {
    const db = getDb();
    const row = db.prepare("SELECT value FROM settings WHERE key = 'scan_depth'").get();
    maxDepth = row ? parseInt(row.value) || 0 : 0;
  }

  const db = getDb();
  let scanned = 0;

  // 从根目录开始扫描，depth=0 表示只扫描根目录本身
  const items = await scanDir(rootDir, 0, maxDepth);

  for (const item of items) {
    if (item.type === 'folder') {
      const existing = db.prepare('SELECT id FROM archives WHERE path = ?').get(item.path);
      if (existing) {
        db.prepare("UPDATE archives SET title = ?, page_count = ?, file_size = ?, updated_at = datetime('now') WHERE id = ?")
          .run(item.name, item.pageCount, item.fileSize, existing.id);
      } else {
        db.prepare(`INSERT INTO archives (title, path, archive_type, page_count, file_size)
          VALUES (?, ?, 'folder', ?, ?)`).run(item.name, item.path, item.pageCount, item.fileSize);
      }

      const archive = db.prepare('SELECT id FROM archives WHERE path = ?').get(item.path);
      archiveService.extractFolderCover(item.path, archive.id).catch(e => {
        logger.debug(`文件夹封面生成失败: ${item.path} — ${e.message}`);
      });
      scanned++;

    } else if (item.type === 'archive') {
      const existing = db.prepare('SELECT id FROM archives WHERE path = ?').get(item.path);
      let archiveId;

      if (existing) {
        db.prepare("UPDATE archives SET title = ?, page_count = ?, file_size = ?, archive_type = ?, updated_at = datetime('now') WHERE id = ?")
          .run(item.name, item.images.length, item.fileSize, item.archiveType, existing.id);
        archiveId = existing.id;
      } else {
        const info = db.prepare(`INSERT INTO archives (title, path, archive_type, page_count, file_size)
          VALUES (?, ?, ?, ?, ?)`).run(item.name, item.path, item.archiveType, item.images.length, item.fileSize);
        archiveId = info.lastInsertRowid;
      }

      // 更新 pages 表
      db.prepare('DELETE FROM pages WHERE archive_id = ?').run(archiveId);
      try {
        const insertPage = db.prepare(
          'INSERT INTO pages (archive_id, filename, filepath, sort_order, file_size) VALUES (?, ?, ?, ?, ?)'
        );
        const insertAll = db.transaction(() => {
          for (let i = 0; i < item.images.length; i++) {
            insertPage.run(archiveId, item.images[i].name, item.images[i].path, i, item.images[i].size);
          }
        });
        insertAll();
      } catch (e) {
        logger.warn(`页面写入失败: ${item.path} — ${e.message}`);
      }

      archiveService.extractCover(item.path, archiveId).catch(e => {
        logger.debug(`封面生成失败: ${item.path} — ${e.message}`);
      });
      scanned++;
    }
  }

  logger.info(`扫描完成: 发现 ${scanned} 个漫画 (深度=${maxDepth})`);
  return { scanned, message: `扫描完成，发现 ${scanned} 个漫画` };
}

module.exports = { scanRoot };
