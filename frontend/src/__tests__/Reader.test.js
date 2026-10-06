import { render, screen, waitFor, act, fireEvent } from '@testing-library/react';
import { MemoryRouter, Routes, Route, useNavigate } from 'react-router-dom';
import Reader from '../pages/Reader';
import { ToastProvider } from '../components/Toast';
import { SettingsProvider } from '../hooks/useSettings';

jest.mock('../utils/api');
const api = require('../utils/api').default;

// jsdom 没有 ResizeObserver，Reader 挂载即 new ResizeObserver → 空实现顶替
class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}
global.ResizeObserver = ResizeObserverMock;

function makePages(n) {
  return Array.from({ length: n }, (_, i) => ({
    id: i + 1,
    url: `/api/archives/1/pages/${i}`,
    thumb_url: `/api/archives/1/pages/${i}/thumb`,
    filename: `page-${i + 1}.jpg`,
  }));
}

function renderReader(readPage = 0, pageCount = 6) {
  api.getPages.mockResolvedValue({
    archive: { id: 1, title: '测试漫画', archive_type: 'folder', group_id: null },
    pages: makePages(pageCount),
    read_page: readPage,
  });
  return render(
    <SettingsProvider>
      <ToastProvider>
        <MemoryRouter initialEntries={['/reader/1']}>
          <Routes>
            <Route path="/reader/:archiveId" element={<Reader />} />
          </Routes>
        </MemoryRouter>
      </ToastProvider>
    </SettingsProvider>
  );
}

const pressKey = (key) => {
  act(() => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key }));
  });
};

// —— 触摸手势辅助 ——
// 原生手势监听挂在阅读区外层容器上（ref 绑定），事件从 role="region" 的内层冒泡过去。
const touchStart = (el, x, y = 0) => fireEvent.touchStart(el, {
  touches: [{ clientX: x, clientY: y, identifier: 0 }],
  changedTouches: [{ clientX: x, clientY: y, identifier: 0 }],
});
const touchEnd = (el, x, y = 0) => fireEvent.touchEnd(el, {
  touches: [],
  changedTouches: [{ clientX: x, clientY: y, identifier: 0 }],
});
// 完整滑动：start → 若干次 move → end（贴近真实手指轨迹）
const swipe = (el, fromX, toX, { y = 0, steps = 4 } = {}) => {
  touchStart(el, fromX, y);
  for (let i = 1; i <= steps; i++) {
    const x = fromX + ((toX - fromX) * i) / steps;
    fireEvent.touchMove(el, { touches: [{ clientX: x, clientY: y, identifier: 0 }] });
  }
  touchEnd(el, toX, y);
};
// jsdom 里 clientWidth 恒为 0 → 点击区/滑动阈值都按 0 宽计算，手动给出可视宽度
const withWidth = (el, width) => {
  Object.defineProperty(el, 'clientWidth', { value: width, configurable: true });
};

describe('Reader 双页模式', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getBookmarks.mockResolvedValue({ pages: [] });
    api.saveHistory.mockResolvedValue({});
    api.updateSettings.mockResolvedValue({});
    api.getGroupChapters.mockResolvedValue([]);
  });

  test('开启双页后一次显示两张跨页图（不再回退到单页布局）', async () => {
    const { container } = renderReader();
    await waitFor(() => {
      expect(screen.getByRole('region', { name: /页面阅读区/ })).toBeInTheDocument();
    });

    await act(async () => {
      fireEvent.click(screen.getByLabelText('启用双页模式'));
    });

    // RTL：右页=当前页(page-1)，左页=下一页(page-2)
    expect(screen.getByAltText('page-1.jpg')).toBeInTheDocument();
    expect(screen.getByAltText('page-2.jpg')).toBeInTheDocument();
    expect(container.querySelector('.reader-page-wrapper')).toBeNull(); // 不是单页布局
  });

  test('双页跨页两张图各占一个固定侧栏盒：就绪后 .ready（不透明度 1、无空白帧）原子呈现', async () => {
    const { container } = renderReader();
    await waitFor(() => {
      expect(screen.getByRole('region', { name: /页面阅读区/ })).toBeInTheDocument();
    });
    await act(async () => {
      fireEvent.click(screen.getByLabelText('启用双页模式'));
    });

    // 加载完成前：两张图分别位于独立的等宽侧栏盒内（跨页布局不随解码尺寸跳动），
    // 且不透明度为 0（藏在加载占位之后，不露背景）
    const spread = container.querySelector('.reader-spread');
    expect(spread).not.toBeNull();
    expect(spread.querySelectorAll('.reader-spread-side').length).toBe(2);
    const imgs = spread.querySelectorAll('.reader-spread-side img.reader-spread-img');
    expect(imgs.length).toBe(2);
    for (const img of imgs) {
      expect(img.closest('.reader-spread-side')).not.toBeNull();
      expect(img.classList.contains('ready')).toBe(false);
    }

    // 两张图加载完成 → 跨页就绪：两张图同时带 .ready 原子呈现（无淡入过渡/空白帧）
    await act(async () => {
      fireEvent.load(imgs[0]);
      fireEvent.load(imgs[1]);
    });
    expect(spread.querySelectorAll('.reader-spread-img.ready').length).toBe(2);
  });

  test('双页模式翻页仍保持双页布局，步进 2 页', async () => {
    const { container } = renderReader();
    await waitFor(() => {
      expect(screen.getByRole('region', { name: /页面阅读区/ })).toBeInTheDocument();
    });
    await act(async () => {
      fireEvent.click(screen.getByLabelText('启用双页模式'));
    });

    pressKey('ArrowRight'); // 双页 RTL → currentIndex += 2 → 跨页 {page-3, page-4}

    expect(screen.getByAltText('page-3.jpg')).toBeInTheDocument();
    expect(screen.getByAltText('page-4.jpg')).toBeInTheDocument();
    expect(container.querySelector('.reader-page-wrapper')).toBeNull();
  });

  test('末页缺一张时保留双页布局（空位占位），不掉回单页造成布局切换闪烁', async () => {
    const { container } = renderReader();
    await waitFor(() => {
      expect(screen.getByRole('region', { name: /页面阅读区/ })).toBeInTheDocument();
    });
    await act(async () => {
      fireEvent.click(screen.getByLabelText('启用双页模式'));
    });

    pressKey('End'); // 跳到最后一页（index 5），RTL 下仅右页存在

    const lone = screen.getByAltText('page-6.jpg');
    const spread = lone.closest('.reader-spread');
    // 双页布局：两个等宽侧栏盒，末页缺一张时空位侧栏盒占位（aria-hidden）
    expect(spread.querySelectorAll('.reader-spread-side').length).toBe(2);
    expect(spread.querySelectorAll('img').length).toBe(1);
    expect(container.querySelector('.reader-spread-side[aria-hidden="true"]')).not.toBeNull();
    expect(container.querySelector('.reader-page-wrapper')).toBeNull(); // 未回退到单页
  });

  test('单页模式：末页继续翻环回第一页，首页往回翻环回末页', async () => {
    renderReader();
    await waitFor(() => {
      expect(screen.getByRole('region', { name: /页面阅读区/ })).toBeInTheDocument();
    });

    pressKey('End'); // index 5
    expect(screen.getByAltText('page-6.jpg')).toBeInTheDocument();

    pressKey('ArrowRight'); // 末页继续 → 环回本册第一页
    expect(screen.getByAltText('page-1.jpg')).toBeInTheDocument();

    pressKey('ArrowLeft'); // 第一页往回 → 环回本册末页
    expect(screen.getByAltText('page-6.jpg')).toBeInTheDocument();
  });

  test('双页模式：末页继续翻环回第一跨页，首页往回翻环回末跨页', async () => {
    renderReader();
    await waitFor(() => {
      expect(screen.getByRole('region', { name: /页面阅读区/ })).toBeInTheDocument();
    });
    await act(async () => {
      fireEvent.click(screen.getByLabelText('启用双页模式'));
    });

    pressKey('End'); // index 5（末页单张，RTL 右=page-6）
    expect(screen.getByAltText('page-6.jpg')).toBeInTheDocument();

    pressKey('ArrowRight'); // 末页继续 → 环回第一跨页 (0,1)
    expect(screen.getByAltText('page-1.jpg')).toBeInTheDocument();
    expect(screen.getByAltText('page-2.jpg')).toBeInTheDocument();

    pressKey('ArrowLeft'); // 第一跨页往回 → 环回末跨页（index 4 → 右=page-5，左=page-6）
    expect(screen.getByAltText('page-5.jpg')).toBeInTheDocument();
    expect(screen.getByAltText('page-6.jpg')).toBeInTheDocument();
  });

  test('切换档案：旧档案进度先落盘，且不把旧页码写进新档案（回归：同路由换档损坏进度）', async () => {
    jest.useFakeTimers();
    // 每个档案独立返回（id 随路由变化）
    api.getPages.mockImplementation((id) => Promise.resolve({
      archive: { id: Number(id), title: `档案${id}`, archive_type: 'folder', group_id: null },
      pages: makePages(6),
      read_page: 0,
    }));

    function GoButton({ to, label }) {
      const navigate = useNavigate();
      return <button onClick={() => navigate(to)}>{label}</button>;
    }

    render(
      <SettingsProvider>
        <ToastProvider>
          <MemoryRouter initialEntries={['/reader/1']}>
            <Routes>
              <Route path="/reader/:archiveId" element={(
                <>
                  <GoButton to="/reader/2" label="切到档案2" />
                  <Reader />
                </>
              )} />
            </Routes>
          </MemoryRouter>
        </ToastProvider>
      </SettingsProvider>
    );

    await waitFor(() => {
      expect(screen.getByRole('region', { name: /页面阅读区/ })).toBeInTheDocument();
    });

    // 档案 1 翻 3 页到 index 3（第 4 页），防抖尚未触发
    pressKey('ArrowRight');
    pressKey('ArrowRight');
    pressKey('ArrowRight');

    // 立刻切到档案 2：旧档案进度应立即落盘（flush），而不是 1s 后才被防抖覆盖
    await act(async () => {
      fireEvent.click(screen.getByText('切到档案2'));
    });
    await act(async () => {
      jest.advanceTimersByTime(1000); // 让新档案加载 + 防抖落定
    });

    const calls = api.saveHistory.mock.calls.map(c => c.slice(0, 3));
    expect(calls).toContainEqual([1, 3, 6]);     // 旧档案 1 的最后位置已保存
    expect(calls).not.toContainEqual([2, 3, 6]); // 旧页码绝不能写进新档案 2
    expect(calls).toContainEqual([2, 0, 6]);     // 新档案 2 正常保存自己的（首页）进度

    jest.useRealTimers();
  });

  test('组主档案（group_id===id）显示章节列表而非阅读器', async () => {
    // 与后端 /pages 响应一致：archive 包含 group_id（此前 mock 带而真实响应缺，
    // 掩盖了“组主档案章节列表永不触发”的缺陷；后端已修复，这里做回归保护）
    api.getPages.mockResolvedValue({
      archive: { id: 5, title: '组测试', archive_type: 'folder', group_id: 5 },
      pages: makePages(3),
      read_page: 0,
    });
    api.getGroupChapters.mockResolvedValue([
      { id: 5, title: '组测试', page_count: 3, read_page: 1, archive_type: 'folder' },
      { id: 6, title: '第2话', page_count: 4, read_page: 0, archive_type: 'folder' },
    ]);

    render(
      <SettingsProvider>
        <ToastProvider>
          <MemoryRouter initialEntries={['/reader/5']}>
            <Routes>
              <Route path="/reader/:archiveId" element={<Reader />} />
            </Routes>
          </MemoryRouter>
        </ToastProvider>
      </SettingsProvider>
    );
    await waitFor(() => {
      expect(screen.getByText(/2 话/)).toBeInTheDocument(); // 章节列表头部
    });
    expect(screen.getByText('第2话')).toBeInTheDocument();
    expect(screen.queryByRole('region', { name: /页面阅读区/ })).toBeNull(); // 不是阅读器
  });
});

describe('Reader 触摸手势（iPad / 网页端）', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getBookmarks.mockResolvedValue({ pages: [] });
    api.saveHistory.mockResolvedValue({});
    api.updateSettings.mockResolvedValue({});
    api.getGroupChapters.mockResolvedValue([]);
  });

  const openReader = async () => {
    renderReader();
    return screen.findByRole('region', { name: /页面阅读区/ });
  };

  test('rtl（默认）：右滑 = 下一页，左滑 = 上一页', async () => {
    const region = await openReader();
    expect(screen.getByAltText('page-1.jpg')).toBeInTheDocument();

    swipe(region, 100, 260); // 右滑
    expect(screen.getByAltText('page-2.jpg')).toBeInTheDocument();

    swipe(region, 260, 100); // 左滑回上一页
    expect(screen.getByAltText('page-1.jpg')).toBeInTheDocument();
  });

  test('ltr：左滑 = 下一页（方向跟随「翻页方向」设置）', async () => {
    api.getSettings.mockResolvedValue({ page_direction: 'ltr' });
    // aria-label 由 pageDirection 生成：等待 ltr 设置到达
    renderReader();
    const region = await screen.findByRole('region', { name: /点左侧翻到上一页/ });

    swipe(region, 260, 100); // 左滑
    expect(screen.getByAltText('page-2.jpg')).toBeInTheDocument();

    swipe(region, 100, 260); // 右滑回上一页
    expect(screen.getByAltText('page-1.jpg')).toBeInTheDocument();
  });

  test('竖向滑动不翻页（交还浏览器/不误翻）', async () => {
    const region = await openReader();
    swipe(region, 200, 205, { y: 300 });
    expect(screen.getByAltText('page-1.jpg')).toBeInTheDocument();
  });

  test('位移过小不算滑动，交给点击区', async () => {
    const region = await openReader();
    withWidth(region, 900);
    swipe(region, 100, 106);
    expect(screen.getByAltText('page-1.jpg')).toBeInTheDocument(); // 没有翻页

    fireEvent.click(region, { clientX: 100, clientY: 0 }); // rtl：点左侧 = 下一页
    expect(screen.getByAltText('page-2.jpg')).toBeInTheDocument();
  });

  test('慢速滑动（>500ms）仍然翻页（回归：原 500ms 硬上限让慢滑无效）', async () => {
    const region = await openReader();
    const nowSpy = jest.spyOn(Date, 'now').mockReturnValue(100000);
    try {
      touchStart(region, 100, 0);
      fireEvent.touchMove(region, { touches: [{ clientX: 200, clientY: 0, identifier: 0 }] });
      nowSpy.mockReturnValue(100700); // 700ms 后仍在 1s 窗口内
      touchEnd(region, 260, 0);
    } finally {
      nowSpy.mockRestore();
    }
    expect(screen.getByAltText('page-2.jpg')).toBeInTheDocument();
  });

  test('快速轻扫：位移不足 40px 但速度够快也翻页', async () => {
    const region = await openReader();
    withWidth(region, 900); // 大屏阈值 = 72px，25px 只能靠轻扫判定
    const nowSpy = jest.spyOn(Date, 'now').mockReturnValue(100000);
    try {
      touchStart(region, 400, 0);
      nowSpy.mockReturnValue(100016); // 16ms
      touchEnd(region, 425, 2);
    } finally {
      nowSpy.mockRestore();
    }
    expect(screen.getByAltText('page-2.jpg')).toBeInTheDocument();
  });

  test('放大后单指拖动是平移（不翻页），拖动期间挂 .dragging（去掉过渡）', async () => {
    const region = await openReader();
    withWidth(region, 900);
    // 双击放大到 2.5x
    touchStart(region, 400, 200);
    touchEnd(region, 400, 200);
    touchStart(region, 400, 200);
    touchEnd(region, 400, 200);
    expect(screen.getByText(/250%/)).toBeInTheDocument();

    touchStart(region, 500, 200);
    fireEvent.touchMove(region, { touches: [{ clientX: 300, clientY: 220, identifier: 0 }] });
    expect(document.querySelector('.reader-container.dragging')).not.toBeNull();

    touchEnd(region, 300, 220);
    expect(screen.getByAltText('page-1.jpg')).toBeInTheDocument(); // 没有翻页
    expect(screen.getByText(/250%/)).toBeInTheDocument();
  });

  test('滑动后浏览器补发的 click 被忽略：一次滑动只翻一页', async () => {
    const region = await openReader();
    withWidth(region, 900);

    swipe(region, 100, 300); // 右滑 → 第 2 页
    expect(screen.getByAltText('page-2.jpg')).toBeInTheDocument();

    // iPad Safari 在 touchend 后补发的合成 click（rtl 下点在左侧 = 下一页）
    fireEvent.click(region, { clientX: 100, clientY: 200 });
    expect(screen.getByAltText('page-2.jpg')).toBeInTheDocument(); // 没有被多翻一页
  });

  test('单击左右区域仍然翻页（点击区不受手势抑制影响）', async () => {
    const region = await openReader();
    withWidth(region, 900);

    fireEvent.click(region, { clientX: 100, clientY: 200 }); // rtl：左侧 = 下一页
    expect(screen.getByAltText('page-2.jpg')).toBeInTheDocument();

    fireEvent.click(region, { clientX: 800, clientY: 200 }); // rtl：右侧 = 上一页
    expect(screen.getByAltText('page-1.jpg')).toBeInTheDocument();
  });

  test('双指捏合缩放图片，且阻止浏览器把整个网页一起缩放', async () => {
    const region = await openReader();
    fireEvent.touchStart(region, {
      touches: [
        { clientX: 200, clientY: 300, identifier: 0 },
        { clientX: 300, clientY: 300, identifier: 1 },
      ],
    });
    // 非被动监听里的 preventDefault 生效 → fireEvent 返回 false
    const notPrevented = fireEvent.touchMove(region, {
      touches: [
        { clientX: 100, clientY: 300, identifier: 0 },
        { clientX: 400, clientY: 300, identifier: 1 },
      ],
    });
    expect(notPrevented).toBe(false);
    expect(screen.getByText(/300%/)).toBeInTheDocument(); // 100px → 300px，缩放 3 倍

    fireEvent.touchEnd(region, { touches: [], changedTouches: [{ clientX: 400, clientY: 300, identifier: 1 }] });
  });

  test('双击缩放：触摸双击生效，且忽略浏览器补发的 dblclick（不再自我抵消）', async () => {
    const region = await openReader();

    touchStart(region, 400, 200);
    touchEnd(region, 400, 200);
    touchStart(region, 400, 200);
    touchEnd(region, 400, 200);
    expect(screen.getByText(/250%/)).toBeInTheDocument();

    fireEvent.doubleClick(region); // 浏览器补发
    expect(screen.getByText(/250%/)).toBeInTheDocument(); // 未被切回 100%
  });

  test('水平滑动中的 touchmove 被阻止（浏览器不接管手势），竖向不拦截', async () => {
    const region = await openReader();
    touchStart(region, 100, 300);
    const horizontal = fireEvent.touchMove(region, { touches: [{ clientX: 200, clientY: 300, identifier: 0 }] });
    expect(horizontal).toBe(false); // preventDefault 生效

    touchStart(region, 100, 300);
    const vertical = fireEvent.touchMove(region, { touches: [{ clientX: 104, clientY: 400, identifier: 0 }] });
    expect(vertical).toBe(true); // 竖向交给浏览器，不拦截
  });

  test('阅读区挂载后尺寸监听才生效：窄容器禁用双页（回归：ResizeObserver 此前从未 attach）', async () => {
    const observed = [];
    const OriginalRO = global.ResizeObserver;
    global.ResizeObserver = class {
      constructor(cb) { this.cb = cb; }
      observe(el) { observed.push(el); this.cb([{ contentRect: { width: 500, height: 400 } }]); }
      unobserve() {}
      disconnect() {}
    };
    try {
      await openReader();
      await waitFor(() => expect(observed.length).toBe(1));
      // 500px < DOUBLE_PAGE_MIN_WIDTH(600) → 双页开关被禁用
      await waitFor(() => expect(screen.getByLabelText('启用双页模式')).toBeDisabled());
    } finally {
      global.ResizeObserver = OriginalRO;
    }
  });
});