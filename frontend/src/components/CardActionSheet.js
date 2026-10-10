import useModalKeyboard from '../hooks/useModalKeyboard';

/**
 * CardActionSheet — 触屏上的卡片操作面板（底部弹出）。
 *
 * 为什么需要它：卡片上的标签/分类/重命名/移除四个按钮此前只靠 `:hover` 显形
 * （`.archive-card:hover .archive-tag-btn { opacity: 1 }`），而整个样式表里没有一条
 * `@media (hover: none)` 兜底——在 iPad 上这四个操作**看不见也点不到**，
 * 其中「重命名」完全没有别的入口（标签/分类还能走多选模式批量做）。
 *
 * 面板把四个动作变成整行的大点击区（一行 > 44px），比在 100px 宽的封面上摊开
 * 四个 24px 图标可靠得多。进入方式有两个：长按卡片（触屏直觉），或卡片右上角
 * 那个在粗指针设备上常显的「⋯」按钮（看得见的入口，避免长按变成隐藏功能）。
 */
export default function CardActionSheet({ title, subtitle, items, onClose }) {
  const panelRef = useModalKeyboard(onClose);

  return (
    <div className="modal-overlay card-sheet-overlay" onClick={onClose}>
      <div
        ref={panelRef}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-label="漫画操作"
        className="card-sheet"
        style={{ outline: 'none' }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="card-sheet-head">
          <div className="card-sheet-title" title={title}>{title}</div>
          {subtitle && <div className="card-sheet-sub">{subtitle}</div>}
        </div>
        <div className="card-sheet-items">
          {items.map((item) => (
            <button
              key={item.key}
              type="button"
              className={`card-sheet-item ${item.danger ? 'danger' : ''}`}
              // 先关面板再执行动作：否则面板会盖在随后打开的弹窗上
              onClick={() => { onClose(); item.onSelect(); }}
            >
              <span className="card-sheet-icon" aria-hidden="true">{item.icon}</span>
              <span>{item.label}</span>
            </button>
          ))}
        </div>
        <button type="button" className="btn btn-secondary card-sheet-cancel" onClick={onClose}>
          取消
        </button>
      </div>
    </div>
  );
}
