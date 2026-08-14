import React, { useState, useEffect, useRef } from 'react';

// 共享 IntersectionObserver：所有 LazyImage 实例共用一个 observer
const _callbacks = new WeakMap();  // el -> callback
let _sharedObserver = null;

function getSharedObserver() {
  if (_sharedObserver) return _sharedObserver;
  _sharedObserver = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          const cb = _callbacks.get(entry.target);
          if (cb) cb();
          _sharedObserver.unobserve(entry.target);
          _callbacks.delete(entry.target);
        }
      }
    },
    { rootMargin: '200px' }
  );
  return _sharedObserver;
}

/**
 * LazyImage — 使用共享 IntersectionObserver 的懒加载图片组件
 * 仅当图片进入视口时才开始加载，配合骨架屏占位
 */
const LazyImage = React.memo(function LazyImage({ src, alt, className, style, onClick }) {
  const [loaded, setLoaded] = useState(false);
  const [inView, setInView] = useState(false);
  const [error, setError] = useState(false);
  const ref = useRef(null);

  // src 变化时重置加载状态，避免卡片复用后永久停留在错误/骨架态
  useEffect(() => {
    setLoaded(false);
    setError(false);
  }, [src]);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const cb = () => setInView(true);
    _callbacks.set(el, cb);
    getSharedObserver().observe(el);

    return () => {
      getSharedObserver().unobserve(el);
      _callbacks.delete(el);
    };
  }, []);

  // 外层 div 始终挂载并携带 ref，错误/无图时仅切换内部内容，
  // 避免占位元素脱离观察器后泄漏到共享 observer。
  const showPlaceholder = error || !src;
  return (
    <div
      ref={ref}
      className={showPlaceholder ? 'no-cover' : className}
      style={{ position: 'relative', width: '100%', height: '100%', ...style }}
      onClick={onClick}
    >
      {showPlaceholder ? (
        <span style={{ fontSize: '48px', color: 'var(--text-tertiary)' }}>📖</span>
      ) : (
        <>
          {/* 骨架屏 */}
          {!loaded && (
            <div style={{
              position: 'absolute',
              inset: 0,
              background: 'var(--bg-tertiary)',
              animation: 'skeleton-pulse 1.5s ease-in-out infinite',
            }} />
          )}
          {inView && (
            <img
              src={src}
              alt={alt}
              loading="lazy"
              onLoad={() => setLoaded(true)}
              onError={() => setError(true)}
              style={{
                width: '100%',
                height: '100%',
                objectFit: 'cover',
                opacity: loaded ? 1 : 0,
                transition: 'opacity 0.3s',
              }}
            />
          )}
        </>
      )}
    </div>
  );
});

export default LazyImage;
