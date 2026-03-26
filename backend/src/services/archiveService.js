/**
 * archiveService.js — 压缩包解压服务
 * 支持 ZIP/CBZ 和 RAR/CBR 格式，提取图片并生成封面
 */
const fs = require('fs');
const path = require('path');
const AdmZip = require('adm-zip');
const logger = require('../config/logger');
const { DATA_DIR } = require('../db/database');

const IMAGE_EXTS = new Set(['.jpg', '.jpeg', '.png', '.bmp', '.webp', '.gif', '.tiff', '.avif']);

/**
 * 判断文件是否为支持的压缩包
 */
function isArchive(filename) {
  const ext = path.extname(filename).toLowerCase();
  return ['.zip', '.cbz', '.rar', '.cbr', '.7z'].includes(ext);
}

/**
 * 判断文件是否为图片
 */
function isImage(filename) {
  return IMAGE_EXTS.has(path.extname(filename).toLowerCase());
}

/**
 * 获取压缩包内的图片列表（自然排序）
 */
async function getImageList(archivePath) {
  const ext = path.extname(archivePath).toLowerCase();

  if (['.zip', '.cbz'].includes(ext)) {
    return getImageListZip(archivePath);
  } else if (['.rar', '.cbr'].includes(ext)) {
    return getImageListRar(archivePath);
  }

  throw new Error(`不支持的格式: ${ext}`);
}

function getImageListZip(archivePath) {
  const zip = new AdmZip(archivePath);
  const entries = zip.getEntries();
  const images = [];

  for (const entry of entries) {
    if (!entry.isDirectory && isImage(entry.entryName)) {
      images.push({
        name: path.basename(entry.entryName),
        path: entry.entryName,
        size: entry.header.size,
      });
    }
  }

  images.sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));
  return images;
}

async function getImageListRar(archivePath) {
  try {
    const { createExtractorFromFile } = require('node-unrar-js');
    const extractor = await createExtractorFromFile({ filepath: archivePath });
    const list = extractor.getFileList();
    const files = [...list];
    const images = [];

    for (const file of files) {
      if (!file.flags.directory && isImage(file.name)) {
        images.push({
          name: path.basename(file.name),
          path: file.name,
          size: Number(file.unpSize) || 0,
        });
      }
    }

    images.sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));
    return images;
  } catch (err) {
    logger.warn(`RAR 解析失败: ${archivePath} — ${err.message}`);
    return [];
  }
}

/**
 * 从压缩包提取单个文件，返回 Buffer
 */
async function extractFile(archivePath, entryPath) {
  const ext = path.extname(archivePath).toLowerCase();

  if (['.zip', '.cbz'].includes(ext)) {
    return extractFileZip(archivePath, entryPath);
  } else if (['.rar', '.cbr'].includes(ext)) {
    return extractFileRar(archivePath, entryPath);
  }

  throw new Error(`不支持的格式: ${ext}`);
}

function extractFileZip(archivePath, entryPath) {
  const zip = new AdmZip(archivePath);
  const entry = zip.getEntry(entryPath);
  if (!entry) throw new Error(`找不到条目: ${entryPath}`);
  return entry.getData();
}

async function extractFileRar(archivePath, entryPath) {
  const { createExtractorFromFile } = require('node-unrar-js');
  const extractor = await createExtractorFromFile({ filepath: archivePath });
  const list = extractor.getFileList();
  const files = [...list];
  const target = files.find(f => f.name === entryPath);
  if (!target) throw new Error(`找不到条目: ${entryPath}`);

  const extracted = extractor.extract({ files: [target.name] });
  const result = [...extracted][0];
  if (!result || !result.extraction) throw new Error('解压失败');

  return Buffer.from(result.extraction);
}

/**
 * 提取封面图（第一页），生成缩略图，返回缩略图路径
 */
async function extractCover(archivePath, archiveId) {
  const images = await getImageList(archivePath);
  if (images.length === 0) return null;

  const thumbDir = path.join(DATA_DIR, 'thumbnails');
  fs.mkdirSync(thumbDir, { recursive: true });

  const thumbPath = path.join(thumbDir, `${archiveId}_cover.jpg`);
  // 如果已有缓存封面，跳过
  if (fs.existsSync(thumbPath)) return thumbPath;

  try {
    const data = await extractFile(archivePath, images[0].path);
    const sharp = require('sharp');
    await sharp(data)
      .resize(300, 420, { fit: 'inside', withoutEnlargement: true })
      .jpeg({ quality: 75 })
      .toFile(thumbPath);
    return thumbPath;
  } catch (err) {
    logger.warn(`封面生成失败: ${archivePath} — ${err.message}`);
    return null;
  }
}

/**
 * 扫描文件夹类型的封面（第 1 张图片）
 */
async function extractFolderCover(folderPath, archiveId) {
  const thumbDir = path.join(DATA_DIR, 'thumbnails');
  fs.mkdirSync(thumbDir, { recursive: true });
  const thumbPath = path.join(thumbDir, `${archiveId}_cover.jpg`);

  if (fs.existsSync(thumbPath)) return thumbPath;

  try {
    const files = fs.readdirSync(folderPath)
      .filter(f => isImage(f))
      .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));

    if (files.length === 0) return null;

    const firstImage = path.join(folderPath, files[0]);
    const sharp = require('sharp');
    await sharp(firstImage)
      .resize(300, 420, { fit: 'inside', withoutEnlargement: true })
      .jpeg({ quality: 75 })
      .toFile(thumbPath);
    return thumbPath;
  } catch (err) {
    logger.warn(`文件夹封面生成失败: ${folderPath} — ${err.message}`);
    return null;
  }
}

module.exports = {
  isArchive,
  isImage,
  getImageList,
  extractFile,
  extractCover,
  extractFolderCover,
  IMAGE_EXTS,
};
