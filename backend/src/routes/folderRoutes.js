const express = require('express');
const router = express.Router();
const { getDb } = require('../db/database');

// 获取文件夹列表（支持搜索 + 标签筛选）
router.get('/folders', (req, res) => {
  const db = getDb();
  const { search, tag } = req.query;

  let sql = `
    SELECT f.*, h.page_index, h.total_pages, h.updated_at as history_updated
    FROM folders f
    LEFT JOIN history h ON h.folder_id = f.id
  `;
  const params = [];

  if (tag) {
    sql += `
      INNER JOIN folder_tags ft ON ft.folder_id = f.id
      INNER JOIN tags t ON t.id = ft.tag_id AND t.name = ?
    `;
    params.push(tag);
  }

  if (search) {
    sql += ` WHERE f.name LIKE ? `;
    params.push(`%${search}%`);
  }

  sql += ` ORDER BY COALESCE(h.updated_at, f.created_at) DESC`;

  const folders = db.prepare(sql).all(...params);

  // 获取每个文件夹的标签
  const tagStmt = db.prepare(`
    SELECT t.name, t.color FROM folder_tags ft
    JOIN tags t ON t.id = ft.tag_id
    WHERE ft.folder_id = ?
  `);

  const result = folders.map((f) => ({
    ...f,
    tags: tagStmt.all(f.id),
  }));

  res.json(result);
});

// 扫描根目录
router.post('/scan', (req, res) => {
  const fs = require('fs');
  const path = require('path');
  const db = getDb();

  const rootRow = db.prepare('SELECT value FROM settings WHERE key = ?').get('root_dir');
  const rootDir = rootRow ? rootRow.value : '';

  if (!rootDir || !fs.existsSync(rootDir)) {
    return res.status(400).json({ error: '根目录未配置或不存在' });
  }

  const SUPPORTED = ['.jpg', '.jpeg', '.png', '.bmp', '.webp', '.gif', '.tiff'];
  const entries = fs.readdirSync(rootDir, { withFileTypes: true });
  let scanned = 0;

  const upsertFolder = db.prepare(
    'INSERT INTO folders (name, path) VALUES (?, ?) ON CONFLICT(path) DO UPDATE SET name = excluded.name'
  );
  const deleteImages = db.prepare('DELETE FROM images WHERE folder_id = ?');
  const insertImage = db.prepare(
    'INSERT INTO images (folder_id, filename, filepath, sort_order, file_size) VALUES (?, ?, ?, ?, ?)'
  );
  const updateCount = db.prepare('UPDATE folders SET image_count = ? WHERE id = ?');

  const transaction = db.transaction(() => {
    for (const entry of entries) {
      if (!entry.isDirectory()) continue;

      const dirPath = path.join(rootDir, entry.name);
      const files = fs.readdirSync(dirPath)
        .filter((f) => SUPPORTED.includes(path.extname(f).toLowerCase()))
        .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));

      if (files.length === 0) continue;

      upsertFolder.run(entry.name, dirPath);
      const folder = db.prepare('SELECT id FROM folders WHERE path = ?').get(dirPath);

      deleteImages.run(folder.id);
      for (let i = 0; i < files.length; i++) {
        const fp = path.join(dirPath, files[i]);
        let fileSize = 0;
        try { fileSize = fs.statSync(fp).size; } catch {}
        insertImage.run(folder.id, files[i], fp, i, fileSize);
      }
      updateCount.run(files.length, folder.id);
      scanned++;
    }
  });

  transaction();
  res.json({ scanned, message: `扫描完成，发现 ${scanned} 个漫画文件夹` });
});

module.exports = router;
