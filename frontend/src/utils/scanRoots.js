/**
 * scanRoots.js — 扫描根目录列表（纯函数，可单测）。
 *
 * 背景：扫描目录此前只有一个 `root_dir` 设置。后端本来就按根目录界定清理范围
 * （`scan.rs` 的孤儿清理带「只清本 root」注释），也就是说**多根在数据层一直是安全的**，
 * 限制纯粹来自界面：想换一个盘就得把旧路径覆盖掉，于是用户只能记住路径、来回粘贴。
 *
 * 这里把它变成一个「记住的目录列表」：`root_dir`/`scan_depth` 保留为「最近一次使用」
 * 的镜像（后端 `/api/scan` 缺省就读它），列表本身存在 `scan_roots` 里。
 */

export const MAX_SCAN_ROOTS = 8;
export const MIN_SCAN_DEPTH = 1;
export const MAX_SCAN_DEPTH = 5;

// 扫描深度钳到下拉框提供的范围（1..5 层）
export function clampDepth(value, fallback = MIN_SCAN_DEPTH) {
  const n = parseInt(value, 10);
  if (!Number.isFinite(n)) return fallback;
  return Math.min(MAX_SCAN_DEPTH, Math.max(MIN_SCAN_DEPTH, n));
}

/**
 * 解析 `scan_roots` 设置（JSON 数组字符串）。
 *
 * 容错优先：非法 JSON、不是数组、元素没有 path 一律丢弃——设置是用户可手改的
 * key-value，解析失败不该让整个扫描区渲染不出来。
 *
 * 列表为空且给了 `fallbackRoot` 时用「旧的单根设置」种一条：
 * 升级前 `scan_roots` 并不存在，`root_dir` 就是当时唯一的扫描目录，
 * 不种进去会让老用户以为自己的扫描目录丢了。
 */
export function parseScanRoots(raw, fallbackRoot = '', fallbackDepth = MIN_SCAN_DEPTH) {
  let list = [];
  if (typeof raw === 'string' && raw.trim()) {
    try {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) {
        list = parsed
          .map(item => {
            const path = typeof item?.path === 'string' ? item.path.trim() : '';
            if (!path) return null;
            return { path, depth: clampDepth(item?.depth, fallbackDepth) };
          })
          .filter(Boolean);
      }
    } catch (e) {
      list = []; // 坏值当空列表处理，下面还有 fallback 兜底
    }
  }
  if (list.length > 0) return list.slice(0, MAX_SCAN_ROOTS);

  const seeded = typeof fallbackRoot === 'string' ? fallbackRoot.trim() : '';
  return seeded ? [{ path: seeded, depth: clampDepth(fallbackDepth) }] : [];
}

/**
 * 新增（或更新）一个扫描目录。同一路径只保留一条并移到最前，
 * 超出上限丢最旧的——列表是「最近用过的几个」，不是归档。
 *
 * 路径只做 trim，不归一化尾部分隔符：Windows 的 `C:\` 去掉尾部分隔符会变成
 * `C:`（相对路径），为了"看起来更整齐"而制造这种歧义不值得。
 */
export function addScanRoot(roots, path, depth = MIN_SCAN_DEPTH) {
  const trimmed = typeof path === 'string' ? path.trim() : '';
  if (!trimmed) return Array.isArray(roots) ? roots : [];
  const rest = (Array.isArray(roots) ? roots : []).filter(r => r.path !== trimmed);
  return [{ path: trimmed, depth: clampDepth(depth) }, ...rest].slice(0, MAX_SCAN_ROOTS);
}

// 移除一个扫描目录（按路径全等）
export function removeScanRoot(roots, path) {
  return (Array.isArray(roots) ? roots : []).filter(r => r.path !== path);
}

// 改某一根目录的扫描深度（路径不存在时原样返回）
export function updateScanRootDepth(roots, path, depth) {
  return (Array.isArray(roots) ? roots : []).map(r =>
    r.path === path ? { ...r, depth: clampDepth(depth, r.depth) } : r
  );
}

// 序列化回设置值
export function serializeScanRoots(roots) {
  return JSON.stringify(Array.isArray(roots) ? roots : []);
}
