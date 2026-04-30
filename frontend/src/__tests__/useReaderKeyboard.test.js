import { renderHook, act } from '@testing-library/react';
import useReaderKeyboard from '../hooks/useReaderKeyboard';

describe('useReaderKeyboard', () => {
  const defaultProps = {
    goPrev: jest.fn(),
    goNext: jest.fn(),
    goPage: jest.fn(),
    pagesLength: 10,
    longImage: false,
    pageDirection: 'rtl',
    showThumbnails: false,
    setShowThumbnails: jest.fn(),
    showJump: false,
    setShowJump: jest.fn(),
    showHelp: false,
    setShowHelp: jest.fn(),
    showMenu: false,
    setShowMenu: jest.fn(),
    setDoublePage: jest.fn(),
    setLongImage: jest.fn(),
    setRotation: jest.fn(),
    setFitMode: jest.fn(),
    showOverlay: jest.fn(),
    containerRef: { current: null },
  };

  beforeEach(() => {
    jest.clearAllMocks();
  });

  test('左箭头调用 goPrev', () => {
    renderHook(() => useReaderKeyboard(defaultProps));
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft' }));
    });
    expect(defaultProps.goPrev).toHaveBeenCalled();
  });

  test('右箭头调用 goNext', () => {
    renderHook(() => useReaderKeyboard(defaultProps));
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
    });
    expect(defaultProps.goNext).toHaveBeenCalled();
  });

  test('空格调用 goNext', () => {
    renderHook(() => useReaderKeyboard(defaultProps));
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: ' ' }));
    });
    expect(defaultProps.goNext).toHaveBeenCalled();
  });

  test('D 键切换双页模式', () => {
    renderHook(() => useReaderKeyboard(defaultProps));
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'd' }));
    });
    expect(defaultProps.setDoublePage).toHaveBeenCalled();
  });

  test('L 键切换长图模式', () => {
    renderHook(() => useReaderKeyboard(defaultProps));
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'l' }));
    });
    expect(defaultProps.setLongImage).toHaveBeenCalled();
  });

  test('T 键切换缩略图', () => {
    renderHook(() => useReaderKeyboard(defaultProps));
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 't' }));
    });
    expect(defaultProps.setShowThumbnails).toHaveBeenCalled();
  });

  test('G 键打开跳转', () => {
    renderHook(() => useReaderKeyboard(defaultProps));
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'g' }));
    });
    expect(defaultProps.setShowJump).toHaveBeenCalledWith(true);
  });

  test('Home 键跳到第一页', () => {
    renderHook(() => useReaderKeyboard(defaultProps));
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Home' }));
    });
    expect(defaultProps.goPage).toHaveBeenCalledWith(0);
  });

  test('End 键跳到最后一页', () => {
    renderHook(() => useReaderKeyboard(defaultProps));
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'End' }));
    });
    expect(defaultProps.goPage).toHaveBeenCalledWith(9);
  });

  test('F1 打开帮助', () => {
    renderHook(() => useReaderKeyboard(defaultProps));
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'F1' }));
    });
    expect(defaultProps.setShowHelp).toHaveBeenCalled();
  });

  test('Escape 关闭面板', () => {
    const props = { ...defaultProps, showHelp: true };
    renderHook(() => useReaderKeyboard(props));
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    });
    expect(defaultProps.setShowHelp).toHaveBeenCalledWith(false);
  });

  test('输入框内按键不触发', () => {
    renderHook(() => useReaderKeyboard(defaultProps));
    const input = document.createElement('input');
    document.body.appendChild(input);
    input.focus();
    act(() => {
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
    });
    expect(defaultProps.goPrev).not.toHaveBeenCalled();
    document.body.removeChild(input);
  });
});
