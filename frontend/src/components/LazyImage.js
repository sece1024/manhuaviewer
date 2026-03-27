import React, { useState, useEffect, useRef } from 'react';

/**
 * LazyImage — 使用 IntersectionObserver 的懒加载图片组件
 * 仅当图片进入视口时才开始加载，配合骨架屏占位
 */
export default function LazyImage({ src, alt, className, style, onClick }) {
  const [loaded, setLoaded] = useState(false);
  const [inView, setInView] = useState(false);
  const [error, setError] = useState(false);
  const ref = useRef(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setInView(true);
          observer.disconnect();
        }
      },
      { rootMargin: '200px' } // 提前 200px 开始加载
    );

    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  if (error || !src) {
    return (
      <div className="no-cover" style={style}>📖</div>
    );
  }

  return (
    <div ref={ref} className={className} style={{ position: 'relative', ...style }} onClick={onClick}>
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
    </div>
  );
}
