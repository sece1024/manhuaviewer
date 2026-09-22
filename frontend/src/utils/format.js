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
 * 返回路径最后一段（子目录名 / 文件名）。
 * 当 stripExtension 为 true 时剥掉文件扩展名（用于压缩包）。
 * 例如 "/manhua/01" -> "01"；"C:\\Manga\\01.cbz" -> "01"。
 */
export function lastPathPart(path, stripExtension = true) {
  if (!path) return '';
  const parts = splitPathParts(path);
  const last = parts[parts.length - 1];
  if (!last) return '';
  if (stripExtension) {
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

/**
 * 解析时间字符串。后端时间列均由 SQLite datetime('now') 写入 = **UTC 且不带时区
 * 标记**；naive 形式（"YYYY-MM-DD HH:MM[:SS]" 或 "…T…"）若直接 new Date() 会被
 * 当成本地时间，显示偏掉一个时区（还可能跨天）。这里给 naive 形式补 'Z' 按 UTC
 * 解析；已带时区标记（Z / ±HH:MM 结尾）的原样交给 Date 原生解析。
 * 无法解析时返回 null。
 */
export function parseDbTime(dateStr) {
  if (!dateStr) return null;
  const s = String(dateStr).trim();
  if (!s) return null;
  const hasTz = /(?:Z|[+-]\d{2}:?\d{2})$/.test(s);
  const isNaive = /^\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}(:\d{2}(\.\d+)?)?$/.test(s);
  if (!hasTz && isNaive) {
    const d = new Date(`${s.replace(' ', 'T')}Z`);
    return isNaN(d.getTime()) ? null : d;
  }
  const d = new Date(s);
  return isNaN(d.getTime()) ? null : d;
}

export function formatRelativeTime(dateStr) {
  const date = parseDbTime(dateStr) ?? new Date(NaN);
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
  const date = parseDbTime(dateStr);
  if (!date) return dateStr;
  const pad = n => String(n).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/**
 * 紧凑日期：只保留 YYYY-MM-DD（按本地时区展示 UTC 时间戳）。
 * 空值返回 ''、无效输入返回原值——与 formatDate 的契约一致。
 */
export function formatDateShort(dateStr) {
  if (!dateStr) return '';
  const date = parseDbTime(dateStr);
  if (!date) return dateStr;
  const pad = n => String(n).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}
