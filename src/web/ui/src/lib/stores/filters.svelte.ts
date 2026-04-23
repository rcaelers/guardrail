// Session filter state — a Svelte 5 runed store.
// Import once: `import { filters } from '$lib/stores/filters.svelte';`

import type { Status } from '$lib/adapters/types';

class FilterStore {
  version = $state<string>('all');
  status = $state<Status | 'all'>('all');
  sort = $state<'count' | 'recent' | 'similarity' | 'version'>('count');
  search = $state<string>('');
  view = $state<'grouped' | 'list'>('grouped');

  reset() {
    this.version = 'all';
    this.status = 'all';
    this.sort = 'count';
    this.search = '';
    this.view = 'grouped';
  }
}

export const filters = new FilterStore();
