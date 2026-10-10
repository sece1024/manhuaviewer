/**
 * seriesOrder.js — 同目录档案的「系列顺序」判定（纯函数，可单测）。
 *
 * 用途：阅读器读到末页时提供「下一卷」。后端只把候选范围收窄到**同目录**
 * （`GET /api/archives/:id/siblings`），真正的系列归属在这里判定。
 *
 * 判定为什么只基于标题：标题是跨机同步的主键（见 AGENTS.md 的 tag mirror 语义），
 * 引入任何新的持久化「系列」概念都会同步不过去，所以顺序必须在本地算出来。
 *
 * 设计原则：**只用于提示，绝不用于自动跳转**。因此宁可判不出来（返回 null，
 * 界面不显示按钮），也不要判错——判错的最坏后果只是少一个按钮，而不是把用户
 * 送进一本无关的书。
 */

/** 卷/话标记的尾巴：`系列 01`、`系列 第3卷`、`系列 vol.2`、`系列 ch12`、`系列 (4)`。 */
const BRACKETED_NUMBER = /\s*[(（[【]\s*\d{1,4}\s*[)）\]】]\s*$/;
const VOLUME_SUFFIX = /[\s._-]*(?:第\s*)?(?:v(?:ol)?\.?\s*|ch(?:apter)?\.?\s*)?\d{1,4}(?:\s*(?:卷|話|话|集|册|冊|回|章))?\s*$/i;

/**
 * 把标题归一到「系列键」：剥掉尾部的卷/话编号与括号编号，再小写化。
 *
 * 例：`系列 01` / `系列 02` → `系列`；`航海王 vol.10` → `航海王`。
 *
 * 只处理数字编号，**不处理罗马数字**：`[ivxlc]` 这种模式会把普通单词的尾字母
 * （如 `Panic` 的 `c`）当成卷号剥掉，误伤概率远高于收益。
 *
 * 剥到空串时退回原标题：像「86」「Vol.1」这种标题本身就是编号，若都归到空键上，
 * 会把互不相干的书判成同一系列。
 */
export function seriesKey(title) {
  const raw = String(title ?? '').trim();
  if (!raw) return '';
  let s = raw;
  let prev;
  do {
    prev = s;
    s = s.replace(BRACKETED_NUMBER, '').replace(VOLUME_SUFFIX, '');
  } while (s !== prev && s.length > 0);
  return (s.trim() || raw).toLowerCase();
}

/**
 * 数字感知的字符串比较：让「第 10 话」排在「第 2 话」之后。
 *
 * 刻意不用 `localeCompare`：ICU 数据随运行环境变化，同一份数据在不同 Node/浏览器
 * 上可能给出不同顺序，会让「下一卷」的选择变得不可复现。这里按码点比较，确定。
 */
export function naturalCompare(a, b) {
  const chunks = (s) => String(s ?? '').match(/\d+|\D+/g) || [];
  const ca = chunks(a);
  const cb = chunks(b);
  const len = Math.max(ca.length, cb.length);
  for (let i = 0; i < len; i += 1) {
    const x = ca[i];
    const y = cb[i];
    if (x === undefined) return -1; // 前缀短的排前面：`系列` < `系列 01`
    if (y === undefined) return 1;
    if (/^\d/.test(x) && /^\d/.test(y)) {
      const diff = Number(x) - Number(y);
      if (diff !== 0) return diff < 0 ? -1 : 1;
    } else if (x !== y) {
      return x < y ? -1 : 1;
    }
  }
  return 0;
}

/** 标题自然序 + 路径兜底，保证同标题（同系列重复下载）时顺序稳定。 */
function compareSeriesItems(a, b) {
  const byTitle = naturalCompare(a.title, b.title);
  if (byTitle !== 0) return byTitle;
  return naturalCompare(a.path, b.path);
}

/**
 * 从同目录档案里挑出「当前卷的下一卷」。
 *
 * @param siblings 同目录档案列表（后端 `/siblings` 返回，含当前档案）
 * @param currentId 当前档案 id
 * @returns 下一卷的档案对象；同系列里已是最后一卷、或压根没找到同系列时返回 null
 *
 * 注意「同系列」由标题前缀一致决定，所以 `系列 02` 的下一卷是 `系列 10` 而不是
 * `系列 03`——自然序保证按数字大小读，符合"接下来的内容"的直觉。
 */
export function nextInSeries(siblings, currentId) {
  const list = Array.isArray(siblings) ? siblings : [];
  const current = list.find(s => s.id === currentId);
  if (!current) return null;

  const key = seriesKey(current.title);
  const ordered = list
    .filter(s => seriesKey(s.title) === key)
    .sort(compareSeriesItems);

  const idx = ordered.findIndex(s => s.id === currentId);
  if (idx < 0 || idx + 1 >= ordered.length) return null;
  return ordered[idx + 1];
}
