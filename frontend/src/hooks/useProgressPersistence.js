import { useCallback, useEffect, useMemo, useRef } from 'react';
import api from '../utils/api';

/**
 * 阅读进度持久化：防抖保存 + 换档/卸载立即 flush。
 *
 * 进度保存的完整生命周期都在这一个 hook 里：
 * - `commitSave` 按 (archiveId, pageIndex, totalPages) 指纹去重，避免同一值重复 POST；
 * - 当前档的阅读参数镜像到 `saveParamsRef`（只有「archive 属于当前 archiveId」才写入，
 *   防止换档渲染周期把「新 id + 旧页码」混进 flush）；
 * - 状态变化触发 1s 防抖保存；卸载时立即保存一次。
 *
 * `flushPending` 供调用方在**换档前**手动落盘：防抖定时器即将被清掉，且组件复用不卸载
 * （组内章节跳转/末页续章不会触发卸载 flush），否则旧档案最后几秒的阅读位置会丢失。
 */
export default function useProgressPersistence({ archive, archiveId, pages, currentIndex }) {
  const lastSavedRef = useRef(null);
  const saveTimerRef = useRef(null);
  const saveParamsRef = useRef({ archiveId: null, currentIndex: 0, pagesLength: 0 });

  const commitSave = useCallback((aid, index, len) => {
    if (!Number.isFinite(aid) || aid <= 0 || !Number.isFinite(len) || len <= 0) return;
    const fingerprint = `${aid}:${index}:${len}`;
    if (lastSavedRef.current === fingerprint) return;
    lastSavedRef.current = fingerprint;
    api.saveHistory(aid, index, len).catch(() => {});
  }, []);

  // 保存进度参数镜像（供卸载/换档 flush 读取最新值）。
  // 只有「已加载的 archive 属于当前 archiveId」时才更新——切换渲染周期里 archive 还是旧对象，
  // 此时覆写会把「新 id + 旧页码」混进 flush，导致连跳两话时把旧进度写进新档案。
  useEffect(() => {
    if (archive && archive.id === parseInt(archiveId) && pages.length > 0) {
      saveParamsRef.current = { archiveId, currentIndex, pagesLength: pages.length };
    }
  }, [archive, archiveId, currentIndex, pages.length]);

  // 进度保存：仅在状态变化时调度防抖保存；卸载/换档时单独 flush。
  // clearTimeout 必须在守卫之前（换档周期也要清掉旧定时器）；archive.id 归属校验保证
  // 切换渲染周期（archive 还是旧对象）不会用「新 archiveId + 旧页码」排定有害定时器。
  useEffect(() => {
    clearTimeout(saveTimerRef.current);
    if (!archive || archive.id !== parseInt(archiveId) || pages.length === 0) return;
    saveTimerRef.current = setTimeout(() => {
      commitSave(parseInt(archiveId), currentIndex, pages.length);
    }, 1000);
  }, [currentIndex, archive, pages.length, archiveId, commitSave]);

  // 仅在组件卸载时立即保存一次（指纹去重，避免与刚完成的防抖重复提交）
  useEffect(() => {
    return () => {
      clearTimeout(saveTimerRef.current);
      const { archiveId: aid, currentIndex: ci, pagesLength: pl } = saveParamsRef.current;
      commitSave(parseInt(aid), ci, pl);
    };
  }, [commitSave]);

  // 换档前先落盘旧档案未保存的进度（防抖定时器将被清掉，且组件复用不卸载）
  const flushPending = useCallback(() => {
    clearTimeout(saveTimerRef.current);
    const { archiveId: aid, currentIndex: ci, pagesLength: pl } = saveParamsRef.current;
    commitSave(parseInt(aid), ci, pl);
  }, [commitSave]);

  return { flushPending };
}