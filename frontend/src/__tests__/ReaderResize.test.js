import { render, screen, waitFor, act, fireEvent, within } from '@testing-library/react';
import { MemoryRouter, Routes, Route } from 'react-router-dom';
import Reader from '../pages/Reader';
import { ToastProvider } from '../components/Toast';
import { SettingsProvider } from '../hooks/useSettings';

jest.mock('../utils/api');
const api = require('../utils/api').default;

/**
 * 阅读器「调整图片大小」的行为探针：适应模式（高/宽/原始）、滚轮缩放、1:1 重置、
 * 放大后拖动平移、旋转，以及"翻页是否重置缩放"。
 * 捏合与双击已由 Reader.test.js 覆盖，这里只补没被覆盖的路径。
 */

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

function renderReader() {
  api.getPages.mockResolvedValue({
    archive: { id: 1, title: '测试漫画', archive_type: 'folder', group_id: null },
    pages: makePages(6),
    read_page: 0,
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

async function openReader() {
  renderReader();
  await waitFor(() => expect(screen.getByRole('region', { name: /页面阅读区/ })).toBeInTheDocument());
  return screen.getByRole('region', { name: /页面阅读区/ });
}

const img = () => screen.getByAltText('page-1.jpg');
// jsdom 里 clientWidth 恒为 0 → 点击区按 0 宽计算，手动给出可视宽度
const withWidth = (el, width) => {
  Object.defineProperty(el, 'clientWidth', { value: width, configurable: true });
};
const touchStart = (el, x, y = 0) => fireEvent.touchStart(el, {
  touches: [{ clientX: x, clientY: y, identifier: 0 }],
  changedTouches: [{ clientX: x, clientY: y, identifier: 0 }],
});
const touchEnd = (el, x, y = 0) => fireEvent.touchEnd(el, {
  touches: [],
  changedTouches: [{ clientX: x, clientY: y, identifier: 0 }],
});
const pressKey = (key) => act(() => { window.dispatchEvent(new KeyboardEvent('keydown', { key })); });

describe('阅读器调整图片大小', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    api.getSettings.mockResolvedValue({});
    api.getBookmarks.mockResolvedValue({ pages: [] });
    api.getArchiveSiblings.mockResolvedValue([]);
    api.saveHistory.mockResolvedValue({});
  });

  test('滚轮缩放改变 transform 与百分比读数', async () => {
    const region = await openReader();
    expect(img().style.transform).toContain('scale(1)');

    fireEvent.wheel(region, { deltaY: -100 }); // 向上滚 = 放大
    expect(screen.getByText(/115%/)).toBeInTheDocument();
    expect(img().style.transform).toContain('scale(1.15)');

    fireEvent.wheel(region, { deltaY: 100 }); // 向下滚 = 缩小
    expect(screen.getByText(/100%/)).toBeInTheDocument();
  });

  test('适应模式选择落到图片尺寸约束上（高/宽/原始）', async () => {
    await openReader();
    const select = screen.getByLabelText('适应模式');
    expect(select.value).toBe('height');
    expect(img().style.maxHeight).toBe('100%');

    fireEvent.change(select, { target: { value: 'width' } });
    expect(img().style.maxWidth).toBe('100%');
    expect(img().style.height).toBe('auto');

    fireEvent.change(select, { target: { value: 'original' } });
    expect(img().style.maxHeight).toBe('none');
    expect(img().style.maxWidth).toBe('none');

    // 持久化到设置，下次打开沿用
    expect(api.updateSettings).toHaveBeenCalledWith({ reader_fit: 'original' });
  });

  test('W 键循环切换适应模式并写回设置', async () => {
    await openReader();
    expect(screen.getByLabelText('适应模式').value).toBe('height');

    pressKey('w');
    await waitFor(() => expect(screen.getByLabelText('适应模式').value).toBe('width'));
    expect(api.updateSettings).toHaveBeenCalledWith({ reader_fit: 'width' });

    pressKey('w');
    await waitFor(() => expect(screen.getByLabelText('适应模式').value).toBe('original'));

    pressKey('w');
    await waitFor(() => expect(screen.getByLabelText('适应模式').value).toBe('height'));
  });

  test('缩放后未旋转时，拖动平移按屏幕方向 1:1 移动', async () => {
    const region = await openReader();
    fireEvent.wheel(region, { deltaY: -100 }); // scale = 1.15

    fireEvent.mouseDown(region, { button: 0, clientX: 100, clientY: 100 });
    fireEvent.mouseMove(region, { clientX: 160, clientY: 130 });

    // translate 会被除以 scale 写进 transform（60/1.15、30/1.15），屏幕位移仍是 (60, 30)
    expect(img().style.transform).toMatch(/translate\(52\.17\d+px, 26\.08\d+px\)/);
  });

  test('未放大时拖动不平移（拖动只在放大后生效，避免与翻页抢手势）', async () => {
    const region = await openReader();
    fireEvent.mouseDown(region, { button: 0, clientX: 100, clientY: 100 });
    fireEvent.mouseMove(region, { clientX: 200, clientY: 150 });
    expect(img().style.transform).toContain('translate(0px, 0px)');
  });

  test('旋转 90° 后允许拖动平移（此前 scale=1 时拖动被禁用，被裁掉的部分看不到）', async () => {
    const region = await openReader();
    const before = { maxHeight: img().style.maxHeight, maxWidth: img().style.maxWidth };

    pressKey('r');
    await waitFor(() => expect(img().style.transform).toContain('rotate(90deg)'));

    // 现状：旋转不重算尺寸约束（布局盒没变），所以超出 overflow:hidden 的部分只能靠拖动看
    expect(img().style.maxHeight).toBe(before.maxHeight);
    expect(img().style.maxWidth).toBe(before.maxWidth);

    fireEvent.mouseDown(region, { button: 0, clientX: 100, clientY: 100 });
    fireEvent.mouseMove(region, { clientX: 160, clientY: 100 });

    // 拖动生效了（不再是 translate(0,0)）
    expect(img().style.transform).not.toContain('translate(0px, 0px)');
  });

  test('旋转 90° 后拖动方向与屏幕一致（向右拖，图片在屏幕上就向右走）', async () => {
    const region = await openReader();
    pressKey('r');
    await waitFor(() => expect(img().style.transform).toContain('rotate(90deg)'));

    fireEvent.mouseDown(region, { button: 0, clientX: 100, clientY: 100 });
    fireEvent.mouseMove(region, { clientX: 160, clientY: 100 }); // 只向右拖 60px

    // transform 是 scale(1) rotate(90deg) translate(0px, -60px)：
    // 本地位移 (0,-60) 代回 R(90°) 就是屏幕上的 (60,0) —— 图片跟着光标向右走
    expect(img().style.transform).toMatch(/translate\(0px, -60px\)/);
    expect(img().style.transform).toContain('rotate(90deg)');
  });

  test('触摸拖动同样按旋转换算（旋转后单指拖动 = 平移，不是滑动翻页）', async () => {
    const region = await openReader();
    pressKey('r');
    await waitFor(() => expect(img().style.transform).toContain('rotate(90deg)'));

    touchStart(region, 100, 100);
    fireEvent.touchMove(region, { touches: [{ clientX: 160, clientY: 100, identifier: 0 }] });
    touchEnd(region, 160, 100);

    expect(img().style.transform).toMatch(/translate\(0px, -60px\)/);
    expect(img().style.transform).toContain('rotate(90deg)');
  });

  test('拖动之后不会顺手翻页（吞掉浏览器补发的 click）', async () => {
    const region = await openReader();
    fireEvent.wheel(region, { deltaY: -100 });

    fireEvent.mouseDown(region, { button: 0, clientX: 100, clientY: 100 });
    fireEvent.mouseMove(region, { clientX: 160, clientY: 100 });
    fireEvent.mouseUp(region);
    // 浏览器在 mousedown/mouseup 之后会补一个 click
    fireEvent.click(region, { clientX: 100, clientY: 100 });

    expect(screen.getByAltText('page-1.jpg')).toBeInTheDocument(); // 没有被翻页
  });

  test('只是手抖（位移在阈值内）仍然算点击，翻页不受影响', async () => {
    const region = await openReader();
    withWidth(region, 900);

    fireEvent.mouseDown(region, { button: 0, clientX: 100, clientY: 100 });
    fireEvent.mouseMove(region, { clientX: 102, clientY: 100 }); // 2px，未达拖动阈值
    fireEvent.mouseUp(region);
    fireEvent.click(region, { clientX: 100, clientY: 200 }); // rtl 左侧 = 下一页

    await waitFor(() => expect(screen.getByAltText('page-2.jpg')).toBeInTheDocument());
  });

  test('翻页会重置缩放（当前行为，记录以便确认是否有意为之）', async () => {
    const region = await openReader();
    fireEvent.wheel(region, { deltaY: -100 });
    expect(screen.getByText(/115%/)).toBeInTheDocument();

    pressKey('ArrowRight');
    await waitFor(() => expect(screen.getByAltText('page-2.jpg')).toBeInTheDocument());
    expect(screen.getByText(/100%/)).toBeInTheDocument();
  });

  test('⋯ 菜单的「缩放还原到 1:1」清掉缩放与位移', async () => {
    const region = await openReader();
    fireEvent.wheel(region, { deltaY: -100 });
    fireEvent.mouseDown(region, { button: 0, clientX: 100, clientY: 100 });
    fireEvent.mouseMove(region, { clientX: 140, clientY: 120 });
    expect(img().style.transform).not.toContain('translate(0px, 0px)');

    fireEvent.click(screen.getByRole('button', { name: '打开更多菜单' }));
    fireEvent.click(within(screen.getByRole('dialog', { name: '更多操作' })).getByText('缩放还原到 1:1'));

    await waitFor(() => expect(img().style.transform).toContain('scale(1)'));
    expect(img().style.transform).toContain('translate(0px, 0px)');
  });

  test('切换适应模式/双页不会在 setState updater 里更新别的组件（React 会告警）', async () => {
    // updater 必须是纯函数：在里面调 updateSetting/setLongImage 会触发
    // "Cannot update a component while rendering a different component"，
    // 且 React 重复调用 updater 时副作用会执行两次
    const errors = [];
    const spy = jest.spyOn(console, 'error').mockImplementation((...args) => {
      errors.push(String(args[0]));
    });
    try {
      await openReader();
      pressKey('w'); // 适应模式循环（会写设置）
      pressKey('w');
      pressKey('d'); // 双页开关（会关长图、重置加载态）
      pressKey('d');
      pressKey('l'); // 长图开关
      pressKey('l');
    } finally {
      spy.mockRestore();
    }
    expect(errors.filter(e => e.includes('Cannot update a component'))).toHaveLength(0);
  });

  test('长图模式下缩放与适应模式都不生效（图宽固定 100%，手势也关掉了）', async () => {
    api.getSettings.mockResolvedValue({ reader_long: '1' });
    const region = await openReader();

    expect(img().style.width).toBe('100%');
    fireEvent.wheel(region, { deltaY: -100 }); // 手势 enabled=false，滚轮监听根本没挂
    expect(img().style.transform).toBe('');
  });
});
