import React from 'react';
import { render, fireEvent } from '@testing-library/react';
import ConfirmDialog from '../components/ConfirmDialog';

describe('ConfirmDialog 确认弹层', () => {
  test('Enter 只触发确认，不误触取消（焦点默认在取消按钮上）', () => {
    const onConfirm = jest.fn();
    const onCancel = jest.fn();
    render(
      <ConfirmDialog open title="删除" message="确认？" onConfirm={onConfirm} onCancel={onCancel} />
    );
    // 取消按钮自动聚焦：Enter 的默认激活会触发它，处理器必须 preventDefault 避免双触发
    expect(document.activeElement.textContent).toBe('取消');
    fireEvent.keyDown(window, { key: 'Enter' });
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onCancel).not.toHaveBeenCalled();
  });

  test('Esc 触发取消', () => {
    const onConfirm = jest.fn();
    const onCancel = jest.fn();
    render(
      <ConfirmDialog open title="删除" message="确认？" onConfirm={onConfirm} onCancel={onCancel} />
    );
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onConfirm).not.toHaveBeenCalled();
  });
});