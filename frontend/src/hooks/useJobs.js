import { useCallback, useSyncExternalStore } from 'react';
import api, { invalidateLibrarySessions } from '../utils/api';

/**
 * useJobs — 长任务（扫描 / 跨机同步 / 批量转 CBZ）的**唯一**状态所有者。
 *
 * 为什么必须是模块级单例、而不是各页面自己的 useState：
 * 此前扫描与同步的进度活在「启动它的那个组件」里（useScan / useSync 各有一套 1s 轮询，
 * 且卸载即停）。于是开始扫描后切到书库，进度面板消失；回到设置页时组件重新挂载，
 * `scanning` 又是 false——按钮重新可用、用户看不出任务还在跑，甚至可能再点一次。
 * 任务的生命周期属于后端，不属于某一页，所以状态也必须放在页面之外。
 *
 * 用模块级 store（而非 React Context）还有一个直接好处：`useJobs()` 在没有 Provider
 * 的测试里也能工作，既有 hook 的测试不必套一层 Provider。
 *
 * 对外形状：`{ jobs, anyRunning, startScan, startSync, startConvert, cancelScan, cancelSync, cancelConvert }`
 *   jobs[kind] = { running, total, done, current, finishedAt, result, error, …后端原始字段 }
 * `finishedAt` 是每完成一次就 +1 的计数器：页面在任务结束后才挂载时不会补一次假提示，
 * 而在页面上等到的结束会正好 +1（消费方用 ref 记下上次的值即可）。
 */

const POLL_MS = 1000;
/// 启动后允许几次「后端尚未置位 running」的轮询。超过它还没见过 running=true，
/// 就按「任务已经结束」处理——否则任务短到没被观测到 running 时会永远停在在跑状态。
const GRACE_POLLS = 2;
const KINDS = ['scan', 'sync', 'convert'];

/// 后端的三个 status 端点：形状都含 running/total/done/current，其余字段原样透传。
const STATUS = {
  scan: () => api.scanStatus(),
  sync: () => api.syncStatus(),
  convert: () => api.convertCbzStatus(),
};

/// 各任务特有的进度字段默认值：保持与后端 status 一致的形状，让消费方在任务空闲时
/// 也能安全读 `syncInfo.new` / `convertInfo.errors`（旧的 DEFAULT_INFO 就是这个作用）。
const DEFAULTS = {
  scan: { added: 0, updated: 0, unchanged: 0, removed: 0, skipped: 0 },
  sync: { new: 0, changed: 0, skipped: 0, failed: [] },
  convert: { converted: 0, skipped: 0, failed: 0, errors: [] },
};

const idleJob = (kind) => ({
  ...DEFAULTS[kind],
  running: false, total: 0, done: 0, current: '',
  finishedAt: 0, result: null, error: null,
});

let state = { scan: idleJob('scan'), sync: idleJob('sync'), convert: idleJob('convert') };
const listeners = new Set();
/// 是否见过 running=true。启动瞬间后端可能还没置位，只有先见过它，
/// `running=false` 才代表任务真正结束。
const sawRunning = { scan: false, sync: false, convert: false };
/// 本轮任务已经轮询过几次：兜住「任务短到一次 running=true 都没观测到」的情况，
/// 否则状态会永远停在 running 上（旧的 useSync 就有这个无限轮询的隐患）。
const pollsSinceStart = { scan: 0, sync: 0, convert: 0 };
let timer = null;
let started = false;

function emit() {
  listeners.forEach(l => l());
}

function patch(kind, next) {
  state = { ...state, [kind]: { ...state[kind], ...next } };
  emit();
}

function finishJob(kind, raw = {}) {
  // 幂等：一轮任务的结束会被两个来源同时观测到——扫描的 POST 返回、以及轮询到
  // `running=false`。两者都会走到这里，所以只在「当前确实在跑」时才计一次完成，
  // 否则 finishedAt 会 +2，消费方就会弹两次「扫描完成」。
  if (!state[kind].running) return;
  sawRunning[kind] = false;
  // 成员集合/进度可能已变：浏览会话不能再拿旧列表秒开，否则刚同步进来的档案
  // 在返回书库时会先以旧快照出现（甚至被重新写回会话）。
  invalidateLibrarySessions();
  patch(kind, { ...raw, running: false, finishedAt: state[kind].finishedAt + 1 });
}

function applyStatus(kind, raw) {
  if (!raw || typeof raw !== 'object') return; // 请求失败：保持现状，下一轮再试

  if (raw.running) {
    sawRunning[kind] = true;
    patch(kind, { ...raw, running: true });
    return;
  }

  if (sawRunning[kind] || pollsSinceStart[kind] >= GRACE_POLLS) {
    // 见过 running=true 之后报 false，或宽容期已过 → 判定结束。
    // 必须在把 running 写成 false 之前落完成：finishJob 的幂等判断依赖它。
    finishJob(kind, raw);
    patch(kind, { ...raw, running: false });
    return;
  }

  // 宽容期：刚发出启动请求，后端可能还没把 running 置位。此时保持「在跑」，
  // 只更新进度字段；否则会出现「按钮刚显示转换中、又立刻变回可点」。
  patch(kind, { ...raw, running: state[kind].running });
}

async function fetchStatus(kind) {
  try {
    return [kind, await STATUS[kind]()];
  } catch (e) {
    return [kind, null];
  }
}

function activeKinds() {
  return KINDS.filter(k => state[k].running || sawRunning[k]);
}

function stopTimer() {
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
}

function startTimer() {
  if (timer) return;
  timer = setInterval(poll, POLL_MS);
  // Node（jest）下不要因为一个 1s 定时器把进程挂住
  if (timer && typeof timer.unref === 'function') timer.unref();
}

async function poll() {
  const active = activeKinds();
  if (active.length === 0) {
    stopTimer();
    return;
  }
  active.forEach(k => { pollsSinceStart[k] += 1; });
  const results = await Promise.all(active.map(fetchStatus));
  results.forEach(([kind, raw]) => applyStatus(kind, raw));
  if (state.scan.running || state.sync.running || state.convert.running ||
      sawRunning.scan || sawRunning.sync || sawRunning.convert) {
    // 仍有任务在跑（或刚置位）：保持轮询
    startTimer();
  } else {
    stopTimer();
  }
}

/// 首次订阅时水合一次：应用启动前/在别的页面上启动的任务，回到任意页面都能看见进度。
function ensureStarted() {
  if (started) return;
  started = true;
  Promise.all(KINDS.map(fetchStatus)).then(results => {
    results.forEach(([kind, raw]) => applyStatus(kind, raw));
    if (activeKinds().length > 0) {
      startTimer();
    }
  });
}

function subscribe(listener) {
  listeners.add(listener);
  ensureStarted();
  return () => { listeners.delete(listener); };
}

function getSnapshot() {
  return state;
}

/**
 * 启动一个任务。
 *
 * `skipIf(result)` 用于「请求成功但其实没有活可干」的情况（批量转 CBZ 在全都不需要
 * 转换时返回 `total === 0`）：此时直接回到空闲，既不进入任务态也不计一次完成，
 * 否则面板会闪一下「转换中」再弹一条完成提示。
 *
 * 返回启动请求的结果，供调用方据此提示（例如「没有可转换的档案」）；重复启动返回 null。
 */
async function startJob(kind, starter, { skipIf } = {}) {
  if (state[kind].running) return null;
  pollsSinceStart[kind] = 0;
  sawRunning[kind] = false;
  patch(kind, {
    running: true, total: 0, done: 0, current: '准备中...',
    result: null, error: null,
  });
  startTimer();
  const pending = starter(); // 先发出启动请求，再取一次进度让面板立刻有内容
  poll();

  let result;
  try {
    result = await pending;
  } catch (e) {
    patch(kind, { error: (e && e.message) || '任务启动失败' });
    finishJob(kind);
    return null;
  }

  if (skipIf && skipIf(result)) {
    sawRunning[kind] = false;
    patch(kind, {
      running: false, total: 0, done: 0, current: '', result: null,
    });
    return result;
  }

  patch(kind, { result });
  // 扫描的 POST 一直挂到任务结束才返回：它的返回就是结束信号，不必等下一轮轮询
  if (kind === 'scan') finishJob('scan');
  return result;
}

async function cancelJob(kind) {
  const CANCEL = { scan: api.scanCancel, sync: api.syncCancel, convert: api.convertCbzCancel };
  try {
    await CANCEL[kind]();
  } catch (e) {
    patch(kind, { error: (e && e.message) || '取消失败' });
  }
}

/// 测试用：清空单例状态，避免用例之间互相污染。
export function resetJobsStore() {
  stopTimer();
  state = { scan: idleJob('scan'), sync: idleJob('sync'), convert: idleJob('convert') };
  KINDS.forEach(k => { sawRunning[k] = false; pollsSinceStart[k] = 0; });
  started = false;
  listeners.clear();
}

export default function useJobs() {
  const jobs = useSyncExternalStore(subscribe, getSnapshot);
  const startScan = useCallback((dir, depth) => startJob('scan', () => api.scan(dir, depth)), []);
  const startSync = useCallback((payload) => startJob('sync', () => api.syncStart(payload)), []);
  const startConvert = useCallback(
    (ids) => startJob('convert', () => api.convertCbzStart(ids), {
      // total=0：库里没有需要转换的档案，后端也没启动任何任务
      skipIf: (r) => !r || r.total === 0,
    }),
    []
  );
  const cancelScan = useCallback(() => cancelJob('scan'), []);
  const cancelSync = useCallback(() => cancelJob('sync'), []);
  const cancelConvert = useCallback(() => cancelJob('convert'), []);

  return {
    jobs,
    anyRunning: jobs.scan.running || jobs.sync.running || jobs.convert.running,
    startScan, startSync, startConvert,
    cancelScan, cancelSync, cancelConvert,
  };
}
