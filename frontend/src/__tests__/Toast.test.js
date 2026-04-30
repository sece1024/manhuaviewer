import React from 'react';
import { render, screen, act } from '@testing-library/react';
import { ToastProvider, useToast } from '../components/Toast';

function TestComponent() {
  const toast = useToast();
  return (
    <div>
      <button onClick={() => toast('测试消息', 'info')}>显示Toast</button>
      <button onClick={() => toast('成功', 'success')}>成功</button>
      <button onClick={() => toast('错误', 'error')}>错误</button>
    </div>
  );
}

describe('Toast 组件', () => {
  test('useToast 返回函数', () => {
    render(
      <ToastProvider>
        <TestComponent />
      </ToastProvider>
    );
    expect(screen.getByText('显示Toast')).toBeInTheDocument();
  });

  test('点击按钮显示 toast', () => {
    render(
      <ToastProvider>
        <TestComponent />
      </ToastProvider>
    );
    act(() => {
      screen.getByText('显示Toast').click();
    });
    expect(screen.getByText('测试消息')).toBeInTheDocument();
  });

  test('不同类型的 toast 有对应 class', () => {
    render(
      <ToastProvider>
        <TestComponent />
      </ToastProvider>
    );
    act(() => {
      screen.getByText('成功').click();
    });
    const toasts = screen.getAllByText('成功');
    const toastEl = toasts.find(el => el.classList.contains('toast'));
    expect(toastEl).toHaveClass('toast-success');
  });

  test('toast 自动消失', () => {
    jest.useFakeTimers();
    render(
      <ToastProvider>
        <TestComponent />
      </ToastProvider>
    );
    act(() => {
      screen.getByText('显示Toast').click();
    });
    expect(screen.getByText('测试消息')).toBeInTheDocument();
    act(() => {
      jest.advanceTimersByTime(3000);
    });
    expect(screen.queryByText('测试消息')).not.toBeInTheDocument();
    jest.useRealTimers();
  });
});
