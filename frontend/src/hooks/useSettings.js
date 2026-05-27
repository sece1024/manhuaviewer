import React, { useState, useEffect, useCallback, useContext, createContext } from 'react';
import api from '../utils/api';

const SettingsContext = createContext(null);

// localStorage fallback 仅用于首次加载时的 optimistic 初始化
const LS_FALLBACKS = {
  reader_fit: 'readerFit',
  page_direction: 'pageDirection',
  view_mode: 'viewMode',
  sort_by: 'sortBy',
  sort_order: 'sortOrder',
};

function getFallback(key) {
  const lsKey = LS_FALLBACKS[key];
  return lsKey ? localStorage.getItem(lsKey) : null;
}

export function SettingsProvider({ children }) {
  const [settings, setSettings] = useState(() => {
    // 用 localStorage 做 optimistic 初始化，避免首屏闪烁
    const initial = {};
    for (const [serverKey] of Object.entries(LS_FALLBACKS)) {
      const val = getFallback(serverKey);
      if (val) initial[serverKey] = val;
    }
    return initial;
  });
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    api.getSettings().then(data => {
      setSettings(prev => ({ ...prev, ...data }));
      // 同步回 localStorage 作为缓存
      for (const [serverKey, lsKey] of Object.entries(LS_FALLBACKS)) {
        if (data[serverKey]) localStorage.setItem(lsKey, data[serverKey]);
      }
      setLoaded(true);
    }).catch(() => setLoaded(true));
  }, []);

  const updateSetting = useCallback(async (key, value) => {
    // 乐观更新
    setSettings(prev => ({ ...prev, [key]: value }));
    const lsKey = LS_FALLBACKS[key];
    if (lsKey) localStorage.setItem(lsKey, value);
    try {
      await api.updateSettings({ [key]: value });
    } catch (e) {
      // 回滚可以在这里做，但对设置来说不常见，暂不做
      console.error('Failed to save setting:', e);
    }
  }, []);

  const getSetting = useCallback((key, fallback = '') => {
    return settings[key] || getFallback(key) || fallback;
  }, [settings]);

  return (
    <SettingsContext.Provider value={{ settings, loaded, updateSetting, getSetting }}>
      {children}
    </SettingsContext.Provider>
  );
}

export default function useSettings() {
  const ctx = useContext(SettingsContext);
  if (!ctx) throw new Error('useSettings must be used within SettingsProvider');
  return ctx;
}
