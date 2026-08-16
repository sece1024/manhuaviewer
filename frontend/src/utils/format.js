/**
 * format.js — 通用格式化工具函数
 */

/**
 * 将文件系统路径拆分为各层名称（兼容 Windows 反斜杠与 Unix 正斜杠）。
 * 例如 "C:\\Manga\\Title\\Chapter1" 与 "/Manga/Title/Chapter1" 都得到
 * ["C:", "Manga", "Title", "Chapter1"]（空段被过滤）。
 */
export function splitPathParts(path) {
  if (!path) return [];
  return path.split(/[\\/]/).filter(Boolean);
}

/**
 * 返回路径的父目录（完整目录路径，兼容 Windows/Unix）。
 * 例如 "/manhua/01" -> "/manhua"；"C:\\Manga\\Title" -> "C:\\Manga"。
 */
export function pathDirname(path) {
  if (!path) return '';
  const parts = splitPathParts(path);
  parts.pop();
  if (parts.length === 0) return '';
  const sep = path.includes('\\') ? '\\' : '/';
  const prefix = parts.length > 1 && /^[A-Za-z]:$/.test(parts[0]) ? '' : sep;
  return prefix + parts.join(sep);
}

/**
 * 返回路径最后一段（子目录名 / 文件名，文件时剥掉扩展名）。
 * 例如 "/manhua/01" -> "01"；"C:\\Manga\\01.cbz" -> "01"。
 */
export function lastPathPart(path) {
  if (!path) return '';
  const parts = splitPathParts(path);
  const last = parts[parts.length - 1];
  if (!last) return '';
  if (path[path.length - 1] !== '/' && path[path.length - 1] !== '\\') {
    const dot = last.lastIndexOf('.');
    if (dot > 0) return last.slice(0, dot);
  }
  return last;
}

export function formatSize(bytes) {
  if (!bytes || bytes <= 0) return '';
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
}

export function formatRelativeTime(dateStr) {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now - date;
  const diffMin = Math.floor(diffMs / 60000);
  const diffHour = Math.floor(diffMs / 3600000);
  const diffDay = Math.floor(diffMs / 86400000);

  if (diffMin < 1) return '刚刚';
  if (diffMin < 60) return `${diffMin} 分钟前`;
  if (diffHour < 24) return `${diffHour} 小时前`;
  if (diffDay < 7) return `${diffDay} 天前`;
  return date.toLocaleDateString();
}

export function formatDate(dateStr) {
  if (!dateStr) return '';
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return dateStr;
  const pad = n => String(n).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}
