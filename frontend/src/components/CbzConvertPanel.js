import React from 'react';

/**
 * CBZ 转换进度面板：进度条 + 计数 + 逐项失败 + 取消按钮。
 * 供设置页与书库（浮动）复用，数据来自 useCbzConvert 的 info。
 */
export default function CbzConvertPanel({ info, onCancel }) {
  return (
    <div style={{ background: 'var(--bg-tertiary)', borderRadius: 'var(--radius-sm)', padding: 12, border: '1px solid var(--border)' }}>
      <div style={{ fontSize: 13, marginBottom: 6 }}>
        {info.total > 0
          ? `已处理 ${info.done} / ${info.total}（已转换 ${info.converted} · 跳过 ${info.skipped} · 失败 ${info.failed}）`
          : '准备中...'}
        {info.current && (
          <span style={{ color: 'var(--text-secondary)' }}> —— {info.current}</span>
        )}
      </div>
      {info.total > 0 && (
        <div style={{ height: 6, background: 'var(--border)', borderRadius: 3, overflow: 'hidden' }}>
          <div style={{
            height: '100%',
            background: 'var(--accent)',
            width: `${Math.min(100, (info.done / info.total) * 100)}%`,
            transition: 'width 0.3s',
          }} />
        </div>
      )}
      {info.errors.length > 0 && (
        <div style={{ marginTop: 8, fontSize: 12, color: '#e5484d' }}>
          失败 {info.failed} 项：
          <ul style={{ margin: '4px 0 0 18px' }}>
            {info.errors.slice(0, 5).map((err, i) => <li key={i}>{err}</li>)}
          </ul>
        </div>
      )}
      <div style={{ marginTop: 8, textAlign: 'right' }}>
        <button className="btn btn-sm" onClick={onCancel}>取消转换</button>
      </div>
    </div>
  );
}
