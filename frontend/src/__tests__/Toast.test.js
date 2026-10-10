import { render, screen, act, fireEvent } from '@testing-library/react';
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
    // 触发按钮与 toast 消息都包含 "成功"；取消息外层 .toast 容器验证类型 class
    const msgEl = [...screen.getAllByText('成功')].find(el => el.classList.contains('toast-message'));
    expect(msgEl).toBeTruthy();
    const toastEl = msgEl.closest('.toast');
    expect(toastEl).not.toBeNull();
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

describe('Toast 动作（撤销）', () => {
  function ActionComponent({ onClick }) {
    const toast = useToast();
    return (
      <button onClick={() => toast('已移除《漫画A》', 'success', undefined, { label: '撤销', onClick })}>
        触发
      </button>
    );
  }

  test('带动作的提示渲染出可点按钮，点击后执行动作并收起提示', () => {
    const onClick = jest.fn();
    render(
      <ToastProvider>
        <ActionComponent onClick={onClick} />
      </ToastProvider>
    );

    fireEvent.click(screen.getByText('触发'));

    const action = screen.getByRole('button', { name: '撤销' });
    expect(screen.getByText('已移除《漫画A》')).toBeInTheDocument();

    fireEvent.click(action);
    expect(onClick).toHaveBeenCalledTimes(1);
    // 动作已执行：提示立即收起，免得用户怀疑有没有生效
    expect(screen.queryByText('已移除《漫画A》')).toBeNull();
  });

  test('不带动作的提示不会凭空多出按钮（第 3 参仍是时长，不是动作）', () => {
    function PlainComponent() {
      const toast = useToast();
      return <button onClick={() => toast('普通提示', 'success', 3000)}>普通触发</button>;
    }
    render(
      <ToastProvider>
        <PlainComponent />
      </ToastProvider>
    );
    fireEvent.click(screen.getByText('普通触发'));
    expect(screen.getByText('普通提示')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '撤销' })).toBeNull();
  });
});
