import { useEffect } from 'react';

/**
 * useCommands — 页面把自己这一屏的操作「投稿」给命令面板。
 *
 * 为什么需要这一层：书库一屏上有 20 多个操作，散在顶栏、侧栏、卡片、长按面板和
 * 整理模式里，用户必须记住"这个功能在哪一栏"。命令面板把这些收成一个入口，但面板
 * 挂在应用外壳上，拿不到页面内部的 setState —— 所以反过来让页面把命令登记上来。
 *
 * 用单槽位而不是多层注册表：这个应用同一时刻只挂一个页面（路由级），单槽位语义
 * 最简单也最不容易出错；离场时按 owner 校验后再清空，避免"旧页面卸载把新页面的
 * 命令一起清掉"。
 */

// useSyncExternalStore 要求 getSnapshot 返回稳定引用：空列表不能每次新建数组
const EMPTY = [];
let current = null;
const listeners = new Set();

function emit() {
  listeners.forEach(l => l());
}

/** 登记一屏的命令，返回注销函数。仅登记；不感知面板是否打开。 */
export function registerCommands(commands) {
  current = commands;
  emit();
  return () => {
    if (current === commands) {
      current = null;
      emit();
    }
  };
}

export function getPageCommands() {
  return current || EMPTY;
}

export function subscribeCommands(listener) {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

/**
 * 组件挂载期间把自己的命令登记给命令面板。
 * `commands` 必须是稳定引用（调用方用 useMemo），否则每次渲染都会重登记并触发面板重渲染。
 */
export default function useCommands(commands) {
  useEffect(() => registerCommands(commands), [commands]);
}

// 测试用：清空登记，避免用例之间互相污染（不动订阅者，挂载中的面板仍能收到后续变更）
export function resetCommands() {
  current = null;
}
