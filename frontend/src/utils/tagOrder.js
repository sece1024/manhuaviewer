/**
 * 打标签弹窗/整理模式的标签排序：按使用频次从高到低，最常用的排在最上方。
 *
 * 频次来自 `GET /api/tags` 的 `archive_count`（全库有多少档案带这个标签），
 * 缺失时（老响应、测试桩）按 0 处理，不要因此把列表整体打乱。
 *
 * 几个刻意的选择：
 * - **相同频次保持后端顺序**（后端 `ORDER BY namespace, name`）：`Array#sort`
 *   自 ES2019 起是稳定排序，所以只在频次上比较，不再补一层按名字比较——否则
 *   同一批 0 次的标签会与后端顺序不一致，看起来像随机跳动。
 * - **可选 `pinnedIds`**：本次会话里刚创建的标签钉在最上方。新建标签的频次只有
 *   1，按频次排会沉到列表中部，用户看不见就以为没建上，于是重复创建同名标签。
 * - 排序是**纯函数**，不读 state：调用方只在数据变化时排一次，用户在弹窗里来回
 *   勾选时列表不会重排（边点边跳会点到相邻项上）。
 */

/** 标签的全库使用次数；非数字/负数一律当 0 */
export function tagUsageCount(tag) {
  const n = Number(tag && tag.archive_count);
  return Number.isFinite(n) && n > 0 ? n : 0;
}

/**
 * 返回按使用频次降序排列的新数组（不修改入参）。
 * @param {Array} tags 标签列表
 * @param {Iterable|Set} [pinnedIds] 需要钉在最上方的标签 id（本次会话新建的）
 */
export function sortTagsByUsage(tags, pinnedIds) {
  if (!Array.isArray(tags)) return [];
  const pinned = pinnedIds instanceof Set ? pinnedIds : new Set(pinnedIds || []);
  return [...tags].sort((a, b) => {
    const pa = pinned.has(a && a.id);
    const pb = pinned.has(b && b.id);
    if (pa !== pb) return pa ? -1 : 1;
    return tagUsageCount(b) - tagUsageCount(a);
  });
}
