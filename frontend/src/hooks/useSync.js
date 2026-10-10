import { useCallback, useEffect, useRef, useState } from 'react';
import api from '../utils/api';
import useJobs from './useJobs';

/**
 * 跨机同步：表单状态、对比预览、启动/取消。
 *
 * 进度轮询不再由本 hook 持有：任务层（useJobs）水合 /sync/status 并统一轮询，
 * 所以离开设置页再回来仍能看到真实进度，也不会在任务仍在跑时把「开始同步」按钮
 * 变回可用（此前 syncRunning 是本地 state，重新挂载就丢）。
 *
 * - 表单三项（远端地址/口令/本地目录）以服务端设置为唯一数据源；
 * - `handleSyncStart` 先持久化表单再交给任务层；
 * - 同步结束时任务层会作废浏览会话（成员集合可能已变），本 hook 只负责刷新统计。
 */
export default function useSync({ settings, updateSetting, toast, onStatsRefresh }) {
  const { jobs, startSync: startJob, cancelSync } = useJobs();
  const syncInfo = jobs.sync;
  const syncRunning = syncInfo.running;
  const [syncUrl, setSyncUrl] = useState(settings.sync_remote_url || '');
  const [syncToken, setSyncToken] = useState(settings.sync_remote_token || '');
  const [syncDir, setSyncDir] = useState(settings.sync_dir || '');
  const [syncPlanResult, setSyncPlanResult] = useState(null); // {new:[],changed:[],up_to_date:[],total}
  const [planLoading, setPlanLoading] = useState(false);
  const seenFinishedAt = useRef(syncInfo.finishedAt);

  // 服务端设置就绪/变化后同步表单（设置是唯一数据源）
  useEffect(() => {
    setSyncUrl(settings.sync_remote_url || '');
    setSyncToken(settings.sync_remote_token || '');
    setSyncDir(settings.sync_dir || '');
  }, [settings.sync_remote_url, settings.sync_remote_token, settings.sync_dir]);

  useEffect(() => {
    if (syncInfo.finishedAt === seenFinishedAt.current) return;
    seenFinishedAt.current = syncInfo.finishedAt;
    if (syncInfo.error) {
      toast(syncInfo.error, 'error');
      return;
    }
    if (onStatsRefresh) {
      Promise.resolve(api.getStats()).then(onStatsRefresh).catch(() => {});
    }
  }, [syncInfo.finishedAt, syncInfo.error, toast, onStatsRefresh]);

  // 对比预览：只拉清单与本地比对，不下载
  const handleSyncCompare = async () => {
    if (planLoading || syncRunning) return;
    if (!syncUrl.trim() || !syncDir.trim()) {
      toast('请填写远端地址和本地同步目录', 'warning');
      return;
    }
    setPlanLoading(true);
    setSyncPlanResult(null);
    try {
      const plan = await api.syncPlan({ url: syncUrl.trim(), token: syncToken.trim(), dir: syncDir.trim() });
      setSyncPlanResult(plan);
    } catch (e) {
      toast(e.message || '对比失败', 'error');
    } finally {
      setPlanLoading(false);
    }
  };

  const handleSyncStart = async () => {
    if (syncRunning) return;
    if (!syncUrl.trim() || !syncDir.trim()) {
      toast('请填写远端地址和本地同步目录', 'warning');
      return;
    }
    try {
      await Promise.all([
        updateSetting('sync_remote_url', syncUrl.trim()),
        updateSetting('sync_remote_token', syncToken.trim()),
        updateSetting('sync_dir', syncDir.trim()),
      ]);
    } catch (e) {
      toast(e.message || '同步参数保存失败', 'error');
      return;
    }
    startJob({ url: syncUrl.trim(), token: syncToken.trim(), dir: syncDir.trim() });
  };

  const handleSyncCancel = useCallback(async () => {
    await cancelSync();
  }, [cancelSync]);

  return {
    syncUrl, setSyncUrl,
    syncToken, setSyncToken,
    syncDir, setSyncDir,
    syncRunning,
    syncInfo,
    syncPlanResult,
    planLoading,
    handleSyncCompare,
    handleSyncStart,
    handleSyncCancel,
  };
}
