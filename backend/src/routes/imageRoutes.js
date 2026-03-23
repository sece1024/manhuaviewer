const express = require('express');
const router = express.Router();
const path = require('path');
const fs = require('fs');
const { getDb } = require('../db/database');

// 获取文件夹的图片列表
router.get('/folders/:id/images', (req, res) => {
  const db = getDb();
  const folderId = parseInt(req.params.id);

  const folder = db.prepare('SELECT * FROM folders WHERE id = ?').get(folderId);
  if (!folder) {
    return res.status(404).json({ error: '文件夹不存在' });
  }

  const images = db.prepare(
    'SELECT id, filename, sort_order, width, height, file_size FROM images WHERE folder_id = ? ORDER BY sort_order'
  ).all(folderId);

  res.json({ folder, images });
});

// 获取原图
router.get('/images/:id', (req, res) => {
  const db = getDb();
  const imageId = parseInt(req.params.id);

  const image = db.prepare('SELECT * FROM images WHERE id = ?').get(imageId);
  if (!image) {
    return res.status(404).json({ error: '图片不存在' });
  }

  if (!fs.existsSync(image.filepath)) {
    return res.status(404).json({ error: '图片文件不存在' });
  }

  res.sendFile(path.resolve(image.filepath));
});

// 获取缩略图
router.get('/images/:id/thumbnail', async (req, res) => {
  const db = getDb();
  const imageId = parseInt(req.params.id);

  const image = db.prepare('SELECT * FROM images WHERE id = ?').get(imageId);
  if (!image) {
    return res.status(404).json({ error: '图片不存在' });
  }

  if (!fs.existsSync(image.filepath)) {
    return res.status(404).json({ error: '图片文件不存在' });
  }

  const thumbDir = path.join(__dirname, '../../data/thumbnails');
  fs.mkdirSync(thumbDir, { recursive: true });
  const thumbPath = path.join(thumbDir, `${imageId}.jpg`);

  if (fs.existsSync(thumbPath)) {
    return res.sendFile(path.resolve(thumbPath));
  }

  try {
    const sharp = require('sharp');
    await sharp(image.filepath)
      .resize(150, 150, { fit: 'inside' })
      .jpeg({ quality: 70 })
      .toFile(thumbPath);
    res.sendFile(path.resolve(thumbPath));
  } catch (err) {
    // sharp 不可用时返回原图
    res.sendFile(path.resolve(image.filepath));
  }
});

module.exports = router;
