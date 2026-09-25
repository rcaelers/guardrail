import { writable } from 'svelte/store';

const SHOW_AFTER_MS = 100;
const HIDE_AFTER_MS = 50;

const visible = writable(false);
let active = 0;
let showTimer: ReturnType<typeof setTimeout> | undefined;
let hideTimer: ReturnType<typeof setTimeout> | undefined;

export const systemBusy = { subscribe: visible.subscribe };

/**
 * Register an asynchronous operation with the global busy indicator.
 * The returned function is idempotent and must be called when the operation
 * finishes. Short operations never make the indicator visible.
 */
export function beginBusy(): () => void {
  active += 1;
  if (hideTimer) {
    clearTimeout(hideTimer);
    hideTimer = undefined;
  }
  if (active === 1 && !showTimer) {
    showTimer = setTimeout(() => {
      showTimer = undefined;
      if (active > 0) visible.set(true);
    }, SHOW_AFTER_MS);
  }

  let finished = false;
  return () => {
    if (finished) return;
    finished = true;
    active = Math.max(0, active - 1);
    if (active !== 0) return;

    if (showTimer) {
      clearTimeout(showTimer);
      showTimer = undefined;
    }
    hideTimer = setTimeout(() => {
      hideTimer = undefined;
      if (active === 0) visible.set(false);
    }, HIDE_AFTER_MS);
  };
}
