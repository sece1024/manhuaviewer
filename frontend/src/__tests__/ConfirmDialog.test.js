import { render, fireEvent } from '@testing-library/react';
import ConfirmDialog from '../components/ConfirmDialog';

describe('ConfirmDialog 确认弹层', () => {
  test('焦点默认在「取消」；Enter 不再被全局劫持为确认', () => {
    const onConfirm = jest.fn();
    const onCancel = jest.fn();
    render(
      <ConfirmDialog open title="删除" message="确认？" onConfirm={onConfirm} onCancel={onCancel} />
    );
    // 危险操作默认聚焦取消：此时按 Enter 应跟随焦点（取消），绝不能被 window 级
    // 处理器改写成确认——旧实现正是这样让"按回车取消"变成了"按回车删除"。
    expect(document.activeElement.textContent).toBe('取消');
    fireEvent.keyDown(document.activeElement, { key: 'Enter' });
    expect(onConfirm).not.toHaveBeenCalled();
  });

  test('点击确认才执行 onConfirm', () => {
    const onConfirm = jest.fn();
    render(
      <ConfirmDialog open title="删除" message="确认？" confirmText="删除" danger onConfirm={onConfirm} onCancel={() => {}} />
    );
    fireEvent.click(document.querySelector('.btn-danger'));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  test('Esc 触发取消', () => {
    const onConfirm = jest.fn();
    const onCancel = jest.fn();
    render(
      <ConfirmDialog open title="删除" message="确认？" onConfirm={onConfirm} onCancel={onCancel} />
    );
    fireEvent.keyDown(document.activeElement, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onConfirm).not.toHaveBeenCalled();
  });
});
