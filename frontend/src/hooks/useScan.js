import { useCallback, useEffect, useRef, useState } from 'react';
import api from '../utils/api';

const DEFAULT_SCAN_INFO = {
  total: 0,
  done: 0,
  added: 0,
  updated: 0,
  unchanged: 0,
  removed: 0,
  skipped: 0,
  current: '',
};

/**
 * 书库扫描：表单持久化、进度轮询与取消。
 *
 * - `handleScan` 先持久化根目录/深度，再触发扫描并接管 1s 轮询；
 * - 扫描是同步 POST（返回最终汇总），期间用 `/scan/status` 展示实时进度；
 * - 任务结束或组件卸载时清除轮询，避免离开设置页后泄漏定时器。
 */
export default function useScan({ updateSetting, toast, onStatsRefresh }) {
  const [scanning, setScanning] = useState(false);
  const [scanInfo, setScanInfo] = useState(DEFAULT_SCAN_INFO);
  const pollRef = useRef(null);

  const stopPolling = useCallback(() => {
    if (pollRef.current) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
  }, []);

  // 卸载时停止轮询
  useEffect(() => stopPolling, [stopPolling]);

  const pollScanStatus = useCallback(async () => {
    try {
      const s = await api.scanStatus();
      setScanInfo({
        total: s.total || 0,
        done: s.done || 0,
        added: s.added || 0,
        updated: s.updated || 0,
        unchanged: s.unchanged || 0,
        removed: s.removed || 0,
        skipped: s.skipped || 0,
        current: s.current || '',
      });
      // 服务端已收尾：停止轮询（最终状态由扫描请求的返回决定）
      if (!s.running) stopPolling();
    } catch (e) {
      /* 轮询失败忽略，下一轮再试 */
    }
  }, [stopPolling]);

  const handleScan = useCallback(
    async (dir, depth) => {
      const trimmed = (dir || '').trim();
      if (!trimmed) {
        toast('请先设置书库根目录', 'warning');
        return;
      }
      if (scanning) return;
      setScanning(true);
      setScanInfo({ ...DEFAULT_SCAN_INFO, current: '准备中...' });
      pollScanStatus();
      pollRef.current = setInterval(pollScanStatus, 1000);
      try {
        await updateSetting('root_dir', trimmed);
        await updateSetting('scan_depth', depth);
        const r = await api.scan(trimmed, Number(depth) || 1);
        toast(r.message || '扫描完成', 'success');
        api.getStats().then(onStatsRefresh).catch(() => {});
      } catch (e) {
        toast(e.message, 'error');
      } finally {
        stopPolling();
        setScanning(false);
      }
    },
    [scanning, updateSetting, toast, onStatsRefresh, pollScanStatus, stopPolling]
  );

  const handleScanCancel = useCallback(async () => {
    try {
      await api.scanCancel();
    } catch (e) {
      toast(e.message, 'error');
    }
  }, [toast]);

  return { scanning, scanInfo, handleScan, handleScanCancel };
}
