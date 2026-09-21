import { useCallback, useEffect, useRef, useState } from 'react';
import api from '../utils/api';

const DEFAULT_INFO = {
  total: 0,
  done: 0,
  converted: 0,
  skipped: 0,
  failed: 0,
  current: '',
  errors: [],
};

/**
 * 批量把非 CBZ 压缩档案（7z/RAR/CBR/ZIP）转换为 CBZ：启动后台任务并轮询进度。
 *
 * 转换是后台任务，前端启动后按 1s 轮询 `/archives/convert-cbz/status`，
 * 任务结束后停止轮询并刷新统计。组件卸载时清理定时器。
 */
export default function useCbzConvert({ toast, onStatsRefresh }) {
  const [converting, setConverting] = useState(false);
  const [info, setInfo] = useState(DEFAULT_INFO);
  const pollRef = useRef(null);

  const stopPolling = useCallback(() => {
    if (pollRef.current) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
  }, []);

  useEffect(() => stopPolling, [stopPolling]);

  const poll = useCallback(async () => {
    try {
      const s = await api.convertCbzStatus();
      setInfo({
        total: s.total || 0,
        done: s.done || 0,
        converted: s.converted || 0,
        skipped: s.skipped || 0,
        failed: s.failed || 0,
        current: s.current || '',
        errors: s.errors || [],
      });
      if (!s.running) stopPolling();
    } catch (e) {
      /* 轮询失败忽略，下一轮再试 */
    }
  }, [stopPolling]);

  const startConvert = useCallback(async () => {
    if (converting) return;
    setConverting(true);
    setInfo({ ...DEFAULT_INFO, current: '准备中...' });
    poll();
    pollRef.current = setInterval(poll, 1000);
    try {
      const r = await api.convertCbzStart();
      if (r && r.total === 0) {
        toast('没有可转换的档案（仅支持 7z / RAR / CBR / ZIP）', 'info');
      } else {
        toast('CBZ 转换完成', 'success');
      }
      api.getStats().then(onStatsRefresh).catch(() => {});
    } catch (e) {
      toast(e.message, 'error');
    } finally {
      stopPolling();
      setConverting(false);
    }
  }, [converting, toast, onStatsRefresh, poll, stopPolling]);

  const cancelConvert = useCallback(async () => {
    try {
      await api.convertCbzCancel();
    } catch (e) {
      toast(e.message, 'error');
    }
  }, [toast]);

  return { converting, info, startConvert, cancelConvert };
}
