import useJobs from '../hooks/useJobs';

/**
 * JobIndicator — 常驻的「长任务」指示器。
 *
 * 为什么放在应用外壳而不是设置页：扫描/同步/转 CBZ 都是分钟级的后台任务，用户不会
 * 一直盯着设置页。此前进度只活在发起它的页面里，切到书库就人间蒸发，也看不出任务
 * 到底还在不在跑。这里统一显示当前所有在跑的任务、进度与取消入口，任何页面都能看到。
 */

const LABELS = { scan: '扫描书库', sync: '跨机同步', convert: '转换为 CBZ' };
const KINDS = ['scan', 'sync', 'convert'];

function percentOf(job) {
  if (!job.total || job.total <= 0) return 0;
  return Math.min(100, Math.round((job.done / job.total) * 100));
}

export default function JobIndicator() {
  const { jobs, cancelScan, cancelSync, cancelConvert } = useJobs();
  const CANCEL = { scan: cancelScan, sync: cancelSync, convert: cancelConvert };
  const active = KINDS.filter(k => jobs[k].running);
  if (active.length === 0) return null;

  return (
    <div className="job-indicator" role="status" aria-live="polite" aria-label="进行中的任务">
      {active.map(kind => {
        const job = jobs[kind];
        const done = job.total > 0 ? `${job.done}/${job.total}` : '准备中…';
        return (
          <div className="job-indicator-row" key={kind}>
            <div className="job-indicator-head">
              <span className="job-indicator-label">{LABELS[kind]}</span>
              <span className="job-indicator-count">{done}</span>
              <button
                type="button"
                className="btn btn-sm btn-secondary"
                onClick={() => CANCEL[kind]()}
                aria-label={`取消${LABELS[kind]}`}
              >
                取消
              </button>
            </div>
            <div className="job-indicator-bar">
              <div className="job-indicator-bar-inner" style={{ width: `${percentOf(job)}%` }} />
            </div>
            {job.current && (
              <div className="job-indicator-current" title={job.current}>{job.current}</div>
            )}
          </div>
        );
      })}
    </div>
  );
}
