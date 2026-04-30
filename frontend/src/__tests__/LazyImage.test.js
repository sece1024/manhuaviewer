import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import LazyImage from '../components/LazyImage';

// Mock IntersectionObserver
class MockIntersectionObserver {
  constructor(callback) {
    this.callback = callback;
    this.elements = new Set();
  }
  observe(el) {
    this.elements.add(el);
    // 模拟立即进入视口
    setTimeout(() => {
      this.callback([{ isIntersecting: true, target: el }]);
    }, 0);
  }
  unobserve(el) { this.elements.delete(el); }
  disconnect() { this.elements.clear(); }
}

beforeEach(() => {
  global.IntersectionObserver = MockIntersectionObserver;
});

describe('LazyImage', () => {
  test('无 src 时显示 fallback', () => {
    render(<LazyImage src="" alt="test" />);
    expect(screen.getByText('📖')).toBeInTheDocument();
  });

  test('有 src 时加载图片', async () => {
    render(<LazyImage src="/test.jpg" alt="测试图片" style={{ width: 100, height: 100 }} />);
    await waitFor(() => {
      const img = screen.getByAltText('测试图片');
      expect(img).toBeInTheDocument();
      expect(img).toHaveAttribute('src', '/test.jpg');
    });
  });

  test('图片加载失败显示 fallback', async () => {
    render(<LazyImage src="/broken.jpg" alt="broken" />);
    await waitFor(() => {
      const img = screen.getByAltText('broken');
      // 触发 error 事件
      const event = new Event('error');
      img.dispatchEvent(event);
    });
    await waitFor(() => {
      expect(screen.getByText('📖')).toBeInTheDocument();
    });
  });

  test('点击事件传递', async () => {
    const handleClick = jest.fn();
    render(<LazyImage src="/test.jpg" alt="clickable" onClick={handleClick} style={{ width: 100, height: 100 }} />);
    await waitFor(() => {
      const img = screen.getByAltText('clickable');
      img.click();
    });
    expect(handleClick).toHaveBeenCalledTimes(1);
  });
});
