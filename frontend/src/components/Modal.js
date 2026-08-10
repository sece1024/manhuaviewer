import React from 'react';

/**
 * Modal — 通用模态弹层（遮罩 + 内容面板）。
 * 点击遮罩关闭；内容区点击不冒泡。可通过 innerStyle/overlayStyle 微调。
 */
export default function Modal({ onClose, ariaLabel, innerStyle, overlayStyle, children }) {
  return (
    <div
      className="modal-overlay"
      style={overlayStyle}
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-label={ariaLabel}
    >
      <div className="modal" style={innerStyle} onClick={e => e.stopPropagation()}>
        {children}
      </div>
    </div>
  );
}
