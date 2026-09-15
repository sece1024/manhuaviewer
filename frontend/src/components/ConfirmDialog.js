import React, { useEffect, useRef } from 'react';

export default function ConfirmDialog({ open, title, message, confirmText = '确定', cancelText = '取消', danger, onConfirm, onCancel }) {
  const cancelRef = useRef(null);

  useEffect(() => {
    if (open && cancelRef.current) cancelRef.current.focus();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const handleKey = (e) => {
      // Enter 走 confirm、Esc 走 cancel，两者都必须 preventDefault：
      // 焦点默认落在“取消”按钮上，不拦截的话 Enter 会同时触发本处理器的
      // onConfirm 与取消按钮的原生 click（一次回车确认又取消）。
      if (e.key === 'Escape') {
        e.preventDefault();
        onCancel();
      }
      if (e.key === 'Enter') {
        e.preventDefault();
        onConfirm();
      }
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, [open, onConfirm, onCancel]);

  if (!open) return null;

  return (
    <div className="modal-overlay" onClick={onCancel} role="dialog" aria-modal="true" aria-label={title}>
      <div className="modal" onClick={e => e.stopPropagation()}>
        <h3 style={{ margin: '0 0 8px' }}>{title}</h3>
        <p style={{ color: 'var(--text-secondary)', margin: '0 0 20px' }}>{message}</p>
        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8 }}>
          <button ref={cancelRef} className="btn" onClick={onCancel}>{cancelText}</button>
          <button className={`btn ${danger ? 'btn-danger' : 'btn-primary'}`} onClick={onConfirm}>{confirmText}</button>
        </div>
      </div>
    </div>
  );
}
