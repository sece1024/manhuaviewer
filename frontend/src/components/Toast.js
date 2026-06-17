import React, { useState, useEffect, useRef, createContext, useContext, useCallback } from 'react';

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

export function ToastProvider({ children }) {
  const [toasts, setToasts] = useState([]);
  const timersRef = useRef({});

  const toast = useCallback((message, type = 'info', duration) => {
    const id = ++toastCounter;
    const actualDuration = duration || (type === 'error' || type === 'warning' ? 5000 : 3000);
    setToasts(prev => [...prev, { id, message, type }]);
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
          const duration = t.type === 'error' || t.type === 'warning' ? 5000 : 3000;
          return (
            <div
              key={t.id}
              className={`toast toast-${t.type}`}
              onMouseEnter={() => pause(t.id)}
              onMouseLeave={() => resume(t.id, duration)}
            >
              <span className="toast-message">{t.message}</span>
              <button className="toast-dismiss" onClick={() => dismiss(t.id)} aria-label="关闭">×</button>
            </div>
          );
        })}
      </div>
    </ToastContext.Provider>
  );
}
