import { useCallback, useEffect, useRef } from 'react';
import api from '../utils/api';
import useJobs from './useJobs';

/**
 * 书库扫描：表单持久化 + 进度展示 + 取消。
 *
 * 进度状态不再由本 hook 持有：任务的生命周期属于后端而不是某一页，所以放在模块级
 * 任务层（useJobs）里。这样离开设置页再回来能立刻看到真实进度，也不会因为组件
 * 重新挂载就把按钮变回「可启动」（此前会在任务仍在跑时允许再点一次）。
 *
 * - `handleScan` 先持久化 root_dir/scan_depth，再把任务交给任务层；
 * - 结束时按任务层的 finishedAt（完成计数器）提示一次。计数器在挂载时对齐初值，
 *   所以任务是在别的页面结束的，回到设置页不会补一条过期提示。
 */
export default function useScan({ updateSetting, toast, onStatsRefresh }) {
  const { jobs, startScan, cancelScan } = useJobs();
  const scanInfo = jobs.scan;
  const scanning = scanInfo.running;
  const seenFinishedAt = useRef(scanInfo.finishedAt);

  useEffect(() => {
    if (scanInfo.finishedAt === seenFinishedAt.current) return;
    seenFinishedAt.current = scanInfo.finishedAt;
    if (scanInfo.error) {
      toast(scanInfo.error, 'error');
      return;
    }
    const result = scanInfo.result;
    // 后端汇总文案（含「清理 N 个已删除档案」）优先，取不到再用兜底
    toast(result?.message || '扫描完成', result?.cancelled ? 'warning' : 'success');
    if (onStatsRefresh) {
      Promise.resolve(api.getStats()).then(onStatsRefresh).catch(() => {});
    }
  }, [scanInfo.finishedAt, scanInfo.error, scanInfo.result, toast, onStatsRefresh]);

  const handleScan = useCallback(
    async (dir, depth) => {
      const trimmed = (dir || '').trim();
      if (!trimmed) {
        toast('请先设置书库根目录', 'warning');
        return;
      }
      if (scanning) return;
      try {
        await updateSetting('root_dir', trimmed);
        await updateSetting('scan_depth', depth);
      } catch (e) {
        toast(e.message, 'error');
        return;
      }
      startScan(trimmed, Number(depth) || 1);
    },
    [scanning, updateSetting, toast, startScan]
  );

  return { scanning, scanInfo, handleScan, handleScanCancel: cancelScan };
}
