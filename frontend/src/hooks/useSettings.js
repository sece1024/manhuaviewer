import { useState, useEffect, useCallback, useContext, useMemo, useRef, createContext } from 'react';
import api, { localStorageGet, localStorageSet } from '../utils/api';

const SettingsContext = createContext(null);

// localStorage fallback 仅用于首次加载时的 optimistic 初始化
// （阅读器双页/长图/过宽降级也预置，避免应用启动后首次打开漫画时
//  "单页先显示、服务端设置到达后切成双页"的布局闪烁）
const LS_FALLBACKS = {
  reader_fit: 'readerFit',
  page_direction: 'pageDirection',
  reader_double: 'readerDouble',
  reader_long: 'readerLong',
  reader_auto_single: 'readerAutoSingle',
  view_mode: 'viewMode',
  card_density: 'cardDensity',
  type_filter: 'typeFilter',
  sort_by: 'sortBy',
  sort_order: 'sortOrder',
};

function getFallback(key) {
  const lsKey = LS_FALLBACKS[key];
  return lsKey ? localStorageGet(lsKey) || null : null;
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

  useEffect(() => {
    api.getSettings().then(data => {
      setSettings(prev => ({ ...prev, ...data }));
      // 同步回 localStorage 作为缓存
      for (const [serverKey, lsKey] of Object.entries(LS_FALLBACKS)) {
        if (data[serverKey]) localStorageSet(lsKey, data[serverKey]);
      }
    }).catch(() => {});
  }, []);

  // 设置项的乐观更新 + 失败回滚。
  //
  // 默认**不再抛出**（很多调用方是 effect/事件里的 fire-and-forget，抛出去只会变成
  // unhandled rejection）；需要感知失败的调用方传 `{ throwOnError: true }`，或检查
  // 返回的 boolean。无论哪种，失败都会回滚 state 与 localStorage，不会出现"界面
  // 显示已启用、服务端其实没保存"的假象。
  const settingsRef = useRef(settings);
  useEffect(() => { settingsRef.current = settings; }, [settings]);

  const updateSetting = useCallback(async (key, value, { throwOnError = false } = {}) => {
    const prev = settingsRef.current[key];
    const next = { ...settingsRef.current, [key]: value };
    settingsRef.current = next;
    setSettings(next);
    const lsKey = LS_FALLBACKS[key];
    if (lsKey) localStorageSet(lsKey, value);
    try {
      await api.updateSettings({ [key]: value });
      return true;
    } catch (e) {
      // 只回滚这一项，保留期间其它设置的成功更新
      const rolled = { ...settingsRef.current };
      if (prev === undefined) delete rolled[key];
      else rolled[key] = prev;
      settingsRef.current = rolled;
      setSettings(rolled);
      if (lsKey) localStorageSet(lsKey, prev === undefined ? '' : prev);
      console.error('Failed to save setting:', e);
      if (throwOnError) throw e;
      return false;
    }
  }, []);

  // 稳定 value 引用：仅 settings/updateSetting 变化时消费者才重渲染，
  // 避免 App 因无关原因重渲染时把整棵 Library/Reader/Settings 树都带崩。
  const value = useMemo(() => ({ settings, updateSetting }), [settings, updateSetting]);

  return (
    <SettingsContext.Provider value={value}>
      {children}
    </SettingsContext.Provider>
  );
}

export default function useSettings() {
  const ctx = useContext(SettingsContext);
  if (!ctx) throw new Error('useSettings must be used within SettingsProvider');
  return ctx;
}
