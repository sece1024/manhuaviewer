import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import Modal from '../components/Modal';

describe('Modal 弹层', () => {
  test('Esc 关闭弹层', () => {
    const onClose = jest.fn();
    render(
      <Modal onClose={onClose} ariaLabel="测试弹窗">
        <button>内容</button>
      </Modal>
    );
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  test('点击遮罩关闭、点击内容不关闭', () => {
    const onClose = jest.fn();
    const { container } = render(
      <Modal onClose={onClose} ariaLabel="测试弹窗">
        <button>内容</button>
      </Modal>
    );
    fireEvent.click(container.querySelector('.modal-overlay'));
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByText('内容'));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  test('打开时锁定 body 滚动，关闭后还原', () => {
    const { unmount } = render(
      <Modal onClose={() => {}} ariaLabel="测试弹窗">
        <button>内容</button>
      </Modal>
    );
    expect(document.body.style.overflow).toBe('hidden');
    unmount();
    expect(document.body.style.overflow).toBe('');
  });

  test('打开时焦点收敛到弹层内首个可聚焦元素', () => {
    render(
      <Modal onClose={() => {}} ariaLabel="测试弹窗">
        <button>确认</button>
        <button>取消</button>
      </Modal>
    );
    expect(document.activeElement.textContent).toBe('确认');
  });
});