import { render, screen } from '@testing-library/react';
import ThumbnailPanel from '../components/ThumbnailPanel';

// jsdom 无布局：clientWidth/offsetHeight/getBoundingClientRect 全为 0，
// 组件会回退到「宽 800→7 列、行高 150」的估算，仍能测出"只渲染窗口"这个核心断言。
function makePages(n) {
  return Array.from({ length: n }, (_, i) => ({
    id: i + 1,
    url: `/api/archives/1/pages/${i}`,
    thumb_url: `/api/archives/1/pages/${i}/thumb`,
    filename: `page-${i + 1}.jpg`,
  }));
}

describe('ThumbnailPanel 虚拟化', () => {
  test('大漫画只渲染窗口内格子，而非全部页面', () => {
    const { container } = render(
      <ThumbnailPanel
        pages={makePages(2000)}
        currentIndex={0}
        bookmarks={new Set()}
        onSelect={() => {}}
        onClose={() => {}}
      />
    );

    // 标题显示总页数，但 DOM 里只挂窗口内的缩略图格子
    expect(screen.getByText(/2000 页/)).toBeInTheDocument();
    const items = container.querySelectorAll('.thumbnail-item');
    expect(items.length).toBeLessThan(200); // 远小于 2000
    expect(items.length).toBeGreaterThan(0);

    // spacer 撑出总高度（DOM 里存在至少一个 spacer）
    const spacers = container.querySelectorAll('.thumbnail-spacer');
    expect(spacers.length).toBeGreaterThanOrEqual(1);
  });

  test('页数少时基本全量渲染（窗口 >= 总量）', () => {
    const { container } = render(
      <ThumbnailPanel
        pages={makePages(20)}
        currentIndex={0}
        bookmarks={new Set()}
        onSelect={() => {}}
        onClose={() => {}}
      />
    );
    expect(container.querySelectorAll('.thumbnail-item').length).toBe(20);
  });

  test('当前页加 active 高亮，点击回调携带真实页码', () => {
    const onSelect = jest.fn();
    render(
      <ThumbnailPanel
        pages={makePages(30)}
        currentIndex={5}
        bookmarks={new Set([7])}
        onSelect={onSelect}
        onClose={() => {}}
      />
    );
    expect(document.querySelector('.thumbnail-item.active .page-num').textContent).toContain('6'); // 页码 6 = index 5
    // 书签角标在 index 7（页码 8）
    screen.getByText('8 ⭐').click();
    expect(onSelect).toHaveBeenCalledWith(7);
  });
});
