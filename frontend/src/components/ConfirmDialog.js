import useModalKeyboard from '../hooks/useModalKeyboard';

/**
 * ConfirmDialog — 删除/清空等破坏性操作的确认弹层。
 *
 * 键盘语义刻意保持"浏览器原生"：焦点默认落在「取消」上（由 useModalKeyboard
 * 收敛到第一个可聚焦元素），此时按 Enter 触发的是取消，而不是确认。此前用
 * window 级 Enter=onConfirm，导致焦点明明在「取消」上、一按回车却删除了数据。
 * Esc 关闭、Tab 焦点环、背景滚动锁由 useModalKeyboard 统一负责。
 *
 * 面板拆成内部组件、仅在 open 时挂载：useModalKeyboard 的 effect 只跑一次，
 * 若在 open=false 时就挂载，等真正打开时它不会重新初始化（焦点/Esc 都会失效）。
 */
export default function ConfirmDialog({ open, ...props }) {
  if (!open) return null;
  return <ConfirmDialogPanel {...props} />;
}

function ConfirmDialogPanel({ title, message, confirmText = '确定', cancelText = '取消', danger, extra, onConfirm, onCancel }) {
  const panelRef = useModalKeyboard(onCancel);

  return (
    <div className="modal-overlay" onClick={onCancel} role="dialog" aria-modal="true" aria-label={title}>
      <div ref={panelRef} tabIndex={-1} className="modal" style={{ outline: 'none' }} onClick={e => e.stopPropagation()}>
        <h3 style={{ margin: '0 0 8px' }}>{title}</h3>
        <p style={{ color: 'var(--text-secondary)', margin: '0 0 20px' }}>{message}</p>
        {extra && <div style={{ margin: '-8px 0 20px' }}>{extra}</div>}
        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8 }}>
          <button className="btn" onClick={onCancel}>{cancelText}</button>
          <button className={`btn ${danger ? 'btn-danger' : 'btn-primary'}`} onClick={onConfirm}>{confirmText}</button>
        </div>
      </div>
    </div>
  );
}
