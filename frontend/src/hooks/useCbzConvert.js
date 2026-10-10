import { useCallback, useEffect, useRef } from 'react';
import api from '../utils/api';
import useJobs from './useJobs';

/**
 * 批量把非 CBZ 压缩档案（7z/RAR/CBR/ZIP）转换为 CBZ。
 *
 * - `startConvert(ids)`：ids 省略/为空时转换全部，否则只转换指定档案；
 * - 进度/取消由模块级任务层（useJobs）统一持有：书库页与设置页看到的是同一份状态，
 *   在书库发起、切到设置页（或反过来）都能继续看到进度，不再各自轮询；
 * - 结束提示按任务层的 finishedAt 触发：任务在别的页面结束时不补一条过期提示。
 */
export default function useCbzConvert({ toast, onStatsRefresh }) {
  const { jobs, startConvert: startJob, cancelConvert } = useJobs();
  const info = jobs.convert;
  const converting = info.running;
  const seenFinishedAt = useRef(info.finishedAt);

  useEffect(() => {
    if (info.finishedAt === seenFinishedAt.current) return;
    seenFinishedAt.current = info.finishedAt;
    if (info.error) {
      toast(info.error, 'error');
      return;
    }
    if (onStatsRefresh) {
      Promise.resolve(api.getStats()).then(onStatsRefresh).catch(() => {});
    }
  }, [info.finishedAt, info.error, toast, onStatsRefresh]);

  const startConvert = useCallback(async (ids) => {
    if (converting) return;
    // 任务层在 total=0 时会直接回到空闲，这里只负责把原因告诉用户
    const r = await startJob(ids);
    if (!r || r.total === 0) {
      toast('没有可转换的档案（仅支持 7z / RAR / CBR / ZIP）', 'info');
    }
  }, [converting, toast, startJob]);

  return { converting, info, startConvert, cancelConvert };
}
