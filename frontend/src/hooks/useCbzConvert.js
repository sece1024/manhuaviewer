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

function normalize(s) {
  return {
    total: s?.total || 0,
    done: s?.done || 0,
    converted: s?.converted || 0,
    skipped: s?.skipped || 0,
    failed: s?.failed || 0,
    current: s?.current || '',
    errors: s?.errors || [],
  };
}

/**
 * 批量把非 CBZ 压缩档案（7z/RAR/CBR/ZIP）转换为 CBZ。
 *
 * - `startConvert(ids)`：ids 省略/为空时转换全部，否则只转换指定档案；
 * - 任务是后台作业，启动成功后按 1s 轮询状态，直到 status.running=false 才收尾；
 * - 进入页面时若已有任务在跑（例如在书库发起、切到设置页），自动接管进度轮询。
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
      setInfo(normalize(s));
      if (!s.running) {
        stopPolling();
        setConverting(false);
        if (onStatsRefresh) api.getStats().then(onStatsRefresh).catch(() => {});
      }
    } catch (e) {
      /* 轮询失败忽略，下一轮再试 */
    }
  }, [stopPolling, onStatsRefresh]);

  // 进入页面时若已有转换任务在跑，接管进度
  useEffect(() => {
    let cancelled = false;
    Promise.resolve(api.convertCbzStatus())
      .then((s) => {
        if (cancelled || !s || !s.running) return;
        setConverting(true);
        setInfo(normalize(s));
        pollRef.current = setInterval(poll, 1000);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [poll]);

  const startConvert = useCallback(
    async (ids) => {
      if (converting) return;
      setConverting(true);
      setInfo({ ...DEFAULT_INFO, current: '准备中...' });
      try {
        const r = await api.convertCbzStart(ids);
        if (!r || r.total === 0) {
          setConverting(false);
          toast('没有可转换的档案（仅支持 7z / RAR / CBR / ZIP）', 'info');
          return;
        }
        // 任务已注册：立即取一次进度，并按 1s 轮询直到 running=false
        poll();
        pollRef.current = setInterval(poll, 1000);
      } catch (e) {
        setConverting(false);
        toast(e.message, 'error');
      }
    },
    [converting, toast, poll]
  );

  const cancelConvert = useCallback(async () => {
    try {
      await api.convertCbzCancel();
    } catch (e) {
      toast(e.message, 'error');
    }
  }, [toast]);

  return { converting, info, startConvert, cancelConvert };
}
