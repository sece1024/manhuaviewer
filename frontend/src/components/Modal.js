import React from 'react';
import useModalKeyboard from '../hooks/useModalKeyboard';

/**
 * Modal — 通用模态弹层（遮罩 + 内容面板）。
 * 点击遮罩关闭；内容区点击不冒泡。可通过 innerStyle/overlayStyle 微调。
 * 键盘行为由 useModalKeyboard 统一：Esc 关闭、Tab 焦点环、背景滚动锁定。
 */
export default function Modal({ onClose, ariaLabel, innerStyle, overlayStyle, children }) {
  const panelRef = useModalKeyboard(onClose);

  return (
    <div
      className="modal-overlay"
      style={overlayStyle}
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-label={ariaLabel}
    >
      <div
        ref={panelRef}
        tabIndex={-1}
        className="modal"
        style={{ outline: 'none', ...innerStyle }}
        onClick={e => e.stopPropagation()}
      >
        {children}
      </div>
    </div>
  );
}