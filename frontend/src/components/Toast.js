import { useState, useEffect, useRef, createContext, useContext, useCallback } from 'react';

const ToastContext = createContext();

export function useToast() {
  return useContext(ToastContext);
}

let toastCounter = 0;

function startTimer(id, duration, setToasts, timersRef) {
  timersRef.current[id] = setTimeout(() => {
    setToasts(prev => prev.filter(t => t.id !== id));
    delete timersRef.current[id];
  }, duration);
}

// 带动作的提示（如「撤销」）默认停久一点：3 秒来不及看清更来不及点
const ACTION_DURATION = 8000;
// 动作提示上鼠标悬停时暂停计时用的时长（与默认值一致，保证移出后还能再等一会儿）
const ACTION_HOVER_DURATION = 6000;

export function ToastProvider({ children }) {
  const [toasts, setToasts] = useState([]);
  const timersRef = useRef({});

  /**
   * toast(message, type, duration, action)
   *
   * `action` = { label, onClick }（第 4 参，可选）：给提示条加一个可点的动作。
   * 首个用途是「从库中移除」的撤销——破坏性操作不该只有二次确认，还应该在
   * 出错后给一条退路，而这条退路必须活到用户来得及点它。
   * 点击动作后立即收起提示：动作已经执行，再留着只会让人怀疑有没有生效。
   */
  const toast = useCallback((message, type = 'info', duration, action = null) => {
    const id = ++toastCounter;
    const actualDuration = duration || (action
      ? ACTION_DURATION
      : (type === 'error' || type === 'warning' ? 5000 : 3000));
    setToasts(prev => [...prev, { id, message, type, action }]);
    startTimer(id, actualDuration, setToasts, timersRef);
  }, []);

  const dismiss = useCallback((id) => {
    clearTimeout(timersRef.current[id]);
    delete timersRef.current[id];
    setToasts(prev => prev.filter(t => t.id !== id));
  }, []);

  const pause = useCallback((id) => {
    clearTimeout(timersRef.current[id]);
    delete timersRef.current[id];
  }, []);

  const resume = useCallback((id, duration) => {
    startTimer(id, duration, setToasts, timersRef);
  }, []);

  useEffect(() => {
    return () => {
      Object.values(timersRef.current).forEach(clearTimeout);
    };
  }, []);

  return (
    <ToastContext.Provider value={toast}>
      {children}
      <div className="toast-container" role="status" aria-live="polite">
        {toasts.map(t => {
          const duration = t.action
            ? ACTION_HOVER_DURATION
            : (t.type === 'error' || t.type === 'warning' ? 5000 : 3000);
          return (
            <div
              key={t.id}
              className={`toast toast-${t.type}`}
              onMouseEnter={() => pause(t.id)}
              onMouseLeave={() => resume(t.id, duration)}
            >
              <span className="toast-message">{t.message}</span>
              {t.action && (
                <button
                  type="button"
                  className="toast-action"
                  onClick={() => { dismiss(t.id); t.action.onClick(); }}
                >
                  {t.action.label}
                </button>
              )}
              <button className="toast-dismiss" onClick={() => dismiss(t.id)} aria-label="关闭">×</button>
            </div>
          );
        })}
      </div>
    </ToastContext.Provider>
  );
}
