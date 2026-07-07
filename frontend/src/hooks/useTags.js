import React, { useState, useEffect, useCallback, useContext, createContext, useRef } from 'react';
import api from '../utils/api';

const TagsContext = createContext(null);

// 模块级缓存：TagsProvider 重新挂载时复用上次数据
let _cachedTags = null;

export function TagsProvider({ children }) {
  const [tags, setTags] = useState(() => _cachedTags || []);
  const [loaded, setLoaded] = useState(() => _cachedTags !== null);

  const reload = useCallback(() => {
    return api.getTags().then(data => {
      _cachedTags = data;
      setTags(data);
      setLoaded(true);
      return data;
    }).catch(() => {
      setLoaded(true);
      return _cachedTags || [];
    });
  }, []);

  useEffect(() => {
    // 如果模块级缓存已有数据，跳过首次 fetch（已通过 useState 初始化）
    if (!_cachedTags) {
      reload();
    }
  }, [reload]);

  return (
    <TagsContext.Provider value={{ tags, loaded, reload }}>
      {children}
    </TagsContext.Provider>
  );
}

export default function useTags() {
  const ctx = useContext(TagsContext);
  if (!ctx) throw new Error('useTags must be used within TagsProvider');
  return ctx;
}
