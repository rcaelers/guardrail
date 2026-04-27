<script lang="ts">
  import { goto, invalidateAll } from '$app/navigation';
  import { page } from '$app/stores';
  import type { PageData } from './$types';
  import type { Status } from '$lib/adapters/types';

  import Select from '$lib/components/Select.svelte';
  import GroupRow from '$lib/components/GroupRow.svelte';
  import DetailPanel from '$lib/components/detail/DetailPanel.svelte';
  import { pane } from '$lib/stores/pane.svelte';

  let { data }: { data: PageData } = $props();

  const readOnly = $derived(data.role === 'readonly');
  const canMerge = $derived(data.role === 'maintainer');

  // Expanded-group state (per-row chevron) — local to session.
  let expanded = $state<Set<string>>(new Set());
  function toggleExpanded(id: string) {
    const next = new Set(expanded);
    if (next.has(id)) next.delete(id); else next.add(id);
    expanded = next;
  }

  // ---- URL-driven filters ----
  async function updateParam(key: string, value: string, reset = false) {
    const url = new URL($page.url);
    if (!value || value === 'all' || value === '') url.searchParams.delete(key);
    else url.searchParams.set(key, value);
    if (reset) {
      url.searchParams.delete('id');
      url.searchParams.delete('crash');
      url.searchParams.delete('page');
    }
    await goto(url, { keepFocus: true, noScroll: true, replaceState: true });
  }

  async function setPageSize(size: string) {
    const url = new URL($page.url);
    url.searchParams.set('limit', size);
    url.searchParams.delete('page');
    url.searchParams.delete('id');
    url.searchParams.delete('crash');
    await goto(url, { keepFocus: true, noScroll: true, replaceState: true });
  }

  async function goToPage(n: number) {
    const totalPages = Math.max(1, Math.ceil(data.list.total / data.filters.limit));
    const clamped = Math.max(1, Math.min(totalPages, n));
    const url = new URL($page.url);
    if (clamped <= 1) url.searchParams.delete('page');
    else url.searchParams.set('page', String(clamped));
    await goto(url, { keepFocus: true, noScroll: true, replaceState: true });
  }

  // Selecting a group selects its first crash for the detail pane and
  // expands the row so the user can see the other crashes available to pick.
  async function selectGroup(id: string) {
    const url = new URL($page.url);
    url.searchParams.delete('crash');
    url.searchParams.set('id', id);
    pane.open = true;
    if (!expanded.has(id)) toggleExpanded(id);
    await goto(url, { keepFocus: true, noScroll: true, replaceState: true });
  }

  // Selecting a specific crash within an (expanded) group.
  async function selectCrash(crashId: string, groupId: string) {
    const url = new URL($page.url);
    url.searchParams.delete('id');
    url.searchParams.set('crash', crashId);
    pane.open = true;
    if (!expanded.has(groupId)) toggleExpanded(groupId);
    await goto(url, { keepFocus: true, noScroll: true, replaceState: true });
  }

  // ---- Resizable split-pane ----
  let splitEl: HTMLDivElement | undefined;

  function onDragStart(e: MouseEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const startPct = pane.pct;
    const rect = splitEl!.getBoundingClientRect();

    function move(ev: MouseEvent) {
      const dx = ev.clientX - startX;
      const deltaPct = -(dx / rect.width) * 100;
      pane.pct = Math.min(68, Math.max(22, startPct + deltaPct));
    }
    function up() {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', up);
      document.body.style.cursor = '';
    }
    document.body.style.cursor = 'col-resize';
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up);
  }

  // ---- Form actions ----
  async function setStatus(s: Status) {
    if (!data.selectedGroup) return;
    const body = new FormData();
    body.set('id', data.selectedGroup.id);
    body.set('status', s);
    await fetch('?/setStatus', { method: 'POST', body });
    await invalidateAll();
  }
  async function addNote(noteBody: string) {
    if (!data.selectedGroup) return;
    const body = new FormData();
    body.set('id', data.selectedGroup.id);
    body.set('body', noteBody);
    body.set('author', 'you');
    await fetch('?/addNote', { method: 'POST', body });
    await invalidateAll();
  }
  async function merge(mergedId: string) {
    if (!data.selectedGroup) return;
    const body = new FormData();
    body.set('primaryId', data.selectedGroup.id);
    body.set('mergedId', mergedId);
    await fetch('?/merge', { method: 'POST', body });
    await invalidateAll();
  }
</script>

<div class="flex h-full min-h-0">
  <!-- LIST -->
  <div
    class="flex min-w-0 flex-1 flex-col border-r border-line dark:border-line-dark bg-surface dark:bg-surface-dark"
    style:flex-basis={pane.open ? `${100 - pane.pct}%` : '100%'}
  >
    <!-- Filter toolbar -->
    <div class="flex shrink-0 flex-wrap items-center gap-3 border-b border-line dark:border-line-dark px-5 py-3">
      <input
        type="search"
        placeholder="Search title, symbol, signature…"
        value={data.filters.search}
        onchange={(e) => updateParam('q', (e.currentTarget as HTMLInputElement).value, true)}
        class="w-[300px] rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-1.5 text-[13px] text-ink dark:text-ink-dark outline-none"
      />
      <Select
        label="Version"
        value={data.filters.version}
        options={[['all', 'All'], ...data.list.versions.map((v): [string, string] => [v, v])]}
        onChange={(v) => updateParam('version', v, true)}
      />
      <Select
        label="Status"
        value={data.filters.status}
        options={[['all', 'All'], ['new', 'New'], ['triaged', 'Triaged'], ['resolved', 'Resolved']]}
        onChange={(v) => updateParam('status', v, true)}
      />
      <Select
        label="Sort"
        value={data.filters.sort}
        options={[['count', 'Most frequent'], ['recent', 'Recently seen'], ['similarity', 'Similarity'], ['version', 'Version']]}
        onChange={(v) => updateParam('sort', v)}
      />
      <Select
        label="Per page"
        value={String(data.filters.limit)}
        options={[['10', '10'], ['25', '25'], ['50', '50'], ['100', '100']]}
        onChange={setPageSize}
      />
      <span class="flex-1"></span>
      <span class="text-xs text-ink-muted dark:text-ink-mutedDark">
        {#if data.list.total > data.filters.limit}
          {@const start = (data.filters.page - 1) * data.filters.limit + 1}
          {@const end = Math.min(data.filters.page * data.filters.limit, data.list.total)}
          {start}–{end} of {data.list.total.toLocaleString()} groups
        {:else}
          {data.list.total.toLocaleString()} groups
        {/if}
      </span>
    </div>

    <!-- Column header -->
    <div
      class="grid shrink-0 items-center gap-4 border-b border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-5 py-2 text-[10.5px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark"
      style:grid-template-columns="28px 1fr 80px 72px 80px 110px 90px"
    >
      <span></span>
      <span>Crash group</span>
      <span>Signal</span>
      <span>Version</span>
      <span>Events</span>
      <span>30d trend</span>
      <span>Status</span>
    </div>

    <!-- Rows -->
    <div class="scroll-clean min-h-0 flex-1 overflow-auto">
      {#each data.list.groups as g (g.id)}
        <GroupRow
          {g}
          selected={data.selectedGroup?.id === g.id}
          expanded={expanded.has(g.id)}
          crashes={data.selectedGroup?.id === g.id ? data.selectedGroup.crashes : []}
          selectedCrashId={data.selectedCrash?.id ?? null}
          onSelect={selectGroup}
          onToggle={toggleExpanded}
          onSelectCrash={selectCrash}
        />
      {/each}
    </div>

    <!-- Pagination footer -->
    {#if data.list.total > data.filters.limit}
      {@const totalPages = Math.ceil(data.list.total / data.filters.limit)}
      <div class="flex shrink-0 items-center justify-center gap-2 border-t border-line dark:border-line-dark px-5 py-2">
        <button
          type="button"
          disabled={data.filters.page <= 1}
          onclick={() => goToPage(data.filters.page - 1)}
          class="rounded px-2.5 py-1 text-xs text-ink dark:text-ink-dark hover:bg-surface-panel dark:hover:bg-surface-panelDark disabled:cursor-not-allowed disabled:opacity-40"
        >
          ← Previous
        </button>
        <span class="text-xs text-ink-muted dark:text-ink-mutedDark">Page</span>
        <input
          type="number"
          min="1"
          max={totalPages}
          value={data.filters.page}
          onchange={(e) => {
            const n = parseInt((e.currentTarget as HTMLInputElement).value, 10);
            if (!isNaN(n)) goToPage(n);
          }}
          class="w-12 rounded border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-1.5 py-1 text-center text-xs text-ink dark:text-ink-dark outline-none [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
        />
        <span class="text-xs text-ink-muted dark:text-ink-mutedDark">of {totalPages}</span>
        <button
          type="button"
          disabled={data.filters.page >= totalPages}
          onclick={() => goToPage(data.filters.page + 1)}
          class="rounded px-2.5 py-1 text-xs text-ink dark:text-ink-dark hover:bg-surface-panel dark:hover:bg-surface-panelDark disabled:cursor-not-allowed disabled:opacity-40"
        >
          Next →
        </button>
      </div>
    {/if}
  </div>

  <!-- RESIZER -->
  {#if pane.open && data.selectedGroup && data.selectedCrash}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      role="separator"
      aria-orientation="vertical"
      aria-valuenow={Math.round(pane.pct)}
      aria-valuemin={22}
      aria-valuemax={68}
      tabindex="0"
      onmousedown={onDragStart}
      ondblclick={() => (pane.pct = 42)}
      onkeydown={(e) => {
        if (e.key === 'ArrowLeft') { pane.pct = Math.min(68, pane.pct + 2); e.preventDefault(); }
        else if (e.key === 'ArrowRight') { pane.pct = Math.max(22, pane.pct - 2); e.preventDefault(); }
        else if (e.key === 'Home') { pane.pct = 22; e.preventDefault(); }
        else if (e.key === 'End') { pane.pct = 68; e.preventDefault(); }
      }}
      title="Drag to resize · double-click to reset"
      class="group relative z-[2] w-[6px] shrink-0 cursor-col-resize bg-transparent"
    >
      <div class="absolute inset-y-0 left-1/2 -translate-x-1/2 w-px bg-line dark:bg-line-dark group-hover:w-[2px] group-hover:bg-accent group-active:bg-accent"></div>
    </div>
  {/if}

  <!-- DETAIL -->
  <div
    bind:this={splitEl}
    class="min-w-0 shrink-0"
    style:flex-basis={pane.open && data.selectedCrash ? `${pane.pct}%` : '0%'}
    style:display={pane.open && data.selectedCrash ? 'block' : 'none'}
  >
    {#if data.selectedGroup && data.selectedCrash}
      <DetailPanel
        group={data.selectedGroup}
        crash={data.selectedCrash}
        onStatusChange={setStatus}
        onMerge={merge}
        onAddNote={addNote}
        {readOnly}
        {canMerge}
        onClose={() => (pane.open = false)}
      />
    {/if}
  </div>

  <!-- Collapsed detail rail -->
  {#if !pane.open && data.selectedCrash}
    <button
      type="button"
      onclick={() => (pane.open = true)}
      class="flex w-[40px] shrink-0 cursor-pointer items-center justify-center border-l border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark text-[11px] text-ink-muted dark:text-ink-mutedDark"
      title="Show detail"
    >
      <span style:writing-mode="vertical-rl" style:transform="rotate(180deg)">◀ Show detail</span>
    </button>
  {/if}
</div>
