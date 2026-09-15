// 书库会话一致性比对：只有当“成员集合”变化（档案增删/替换）时，后台比对才判定
// 为需要整体刷新回第一页。
//
// 顺序变化不算“变化”：默认“最近阅读”排序（COALESCE(last_read_at, updated_at)）下，
// 读完一本漫画会更新它的 last_read_at 使其在列表里移动——如果按有序序列比较第一页，
// 每次从阅读器返回都会命中“内容变了”→ 整体刷新，把用户从已翻到的页码踢回第一页。
//
// @param {Set<number|string>} savedIds  会话中已加载窗口（上限内的）档案 id 集合
// @param {Set<number|string>} freshIds  本次比对拉取到的同窗口档案 id 集合
// @returns {boolean} true = 成员变化，应整体刷新；false = 仅顺序/字段变化，应保留位置
export function membershipChanged(savedIds, freshIds) {
  if (savedIds.size !== freshIds.size) return true;
  for (const id of freshIds) {
    if (!savedIds.has(id)) return true;
  }
  return false;
}

// 从列表取“比对窗口”内的 id 集合（slice 到窗口大小，避免对比未加载的部分）。
export function idsWithin(list, windowSize) {
  const set = new Set();
  for (const item of (list || []).slice(0, windowSize)) {
    if (item && item.id !== undefined && item.id !== null) set.add(item.id);
  }
  return set;
}