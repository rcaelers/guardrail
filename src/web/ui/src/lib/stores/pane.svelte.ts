// Detail-pane width + open state, persisted to localStorage.

import { browser } from '$app/environment';

class PaneStore {
  pct = $state<number>(42);
  open = $state<boolean>(true);

  constructor() {
    if (browser) {
      const p = parseFloat(localStorage.getItem('gr-detail-pct') ?? '');
      if (Number.isFinite(p) && p >= 22 && p <= 68) this.pct = p;
      const o = localStorage.getItem('gr-detail-open');
      if (o !== null) this.open = o === '1';

      $effect.root(() => {
        $effect(() => {
          localStorage.setItem('gr-detail-pct', String(this.pct));
        });
        $effect(() => {
          localStorage.setItem('gr-detail-open', this.open ? '1' : '0');
        });
      });
    }
  }
}

export const pane = new PaneStore();
