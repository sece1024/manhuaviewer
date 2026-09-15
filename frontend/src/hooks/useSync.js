import { useCallback, useEffect, useRef, useState } from 'react';
import api from '../utils/api';

const DEFAULT_INFO = { total: 0, done: 0, new: 0, changed: 0, skipped: 0, current: '', failed: [] };

/**
 * 跨机同步：表单状态、对比预览、启动/取消与进度轮询。
 *
 * - 表单三项（远端地址/口令/本地目录）以服务端设置为唯一数据源；
 * - `handleSyncStart` 先持久化表单再启动任务并接管 1s 轮询，任务结束自动停止；
 * - 卸载时清除轮询，避免离开设置页后泄漏定时器。
 */
export default function useSync({ settings, updateSetting, toast, onStatsRefresh }) {
  const [syncUrl, setSyncUrl] = useState(settings.sync_remote_url || '');
  const [syncToken, setSyncToken] = useState(settings.sync_remote_token || '');
  const [syncDir, setSyncDir] = useState(settings.sync_dir || '');
  const [syncRunning, setSyncRunning] = useState(false);
  const [syncInfo, setSyncInfo] = useState(DEFAULT_INFO);
  const [syncPlanResult, setSyncPlanResult] = useState(null); // {new:[],changed:[],up_to_date:[],total}
  const [planLoading, setPlanLoading] = useState(false);
  const syncPollRef = useRef(null);

  // 服务端设置就绪/变化后同步表单（设置是唯一数据源）
  useEffect(() => {
    setSyncUrl(settings.sync_remote_url || '');
    setSyncToken(settings.sync_remote_token || '');
    setSyncDir(settings.sync_dir || '');
  }, [settings.sync_remote_url, settings.sync_remote_token, settings.sync_dir]);

  // 卸载时停止轮询
  useEffect(() => () => { if (syncPollRef.current) clearInterval(syncPollRef.current); }, []);

  const pollSyncStatus = useCallback(async () => {
    try {
      const s = await api.syncStatus();
      setSyncInfo({
        total: s.total || 0,
        done: s.done || 0,
        new: s.new || 0,
        changed: s.changed || 0,
        skipped: s.skipped || 0,
        current: s.current || '',
        failed: s.failed || [],
      });
      if (!s.running) {
        if (syncPollRef.current) { clearInterval(syncPollRef.current); syncPollRef.current = null; }
        setSyncRunning(false);
        api.getStats().then(onStatsRefresh).catch(() => {}); // 完成后刷新统计
      }
    } catch (e) { /* 轮询失败忽略，下一轮再试 */ }
  }, [onStatsRefresh]);

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
      await api.syncStart({ url: syncUrl.trim(), token: syncToken.trim(), dir: syncDir.trim() });
      setSyncRunning(true);
      setSyncInfo({ total: 0, done: 0, new: 0, changed: 0, skipped: 0, current: '连接远端...', failed: [] });
      pollSyncStatus();
      syncPollRef.current = setInterval(pollSyncStatus, 1000);
    } catch (e) {
      toast(e.message || '同步启动失败', 'error');
    }
  };

  const handleSyncCancel = async () => {
    try { await api.syncCancel(); } catch (e) { toast(e.message, 'error'); }
  };

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