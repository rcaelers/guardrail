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
    if (reset) url.searchParams.delete('id');
    await goto(url, { keepFocus: true, noScroll: true, replaceState: true });
  }

  function selectGroup(id: string) {
    updateParam('id', id);
    pane.open = true;
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
    if (!data.selected) return;
    const body = new FormData();
    body.set('id', data.selected.id);
    body.set('status', s);
    await fetch('?/setStatus', { method: 'POST', body });
    await invalidateAll();
  }
  async function addNote(noteBody: string) {
    if (!data.selected) return;
    const body = new FormData();
    body.set('id', data.selected.id);
    body.set('body', noteBody);
    body.set('author', 'you');
    await fetch('?/addNote', { method: 'POST', body });
    await invalidateAll();
  }
  async function merge(mergedId: string) {
    if (!data.selected) return;
    const body = new FormData();
    body.set('primaryId', data.selected.id);
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
      <span class="flex-1"></span>
      <span class="text-xs text-ink-muted dark:text-ink-mutedDark">
        {data.list.groups.length} of {data.list.total.toLocaleString()} groups
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
          selected={data.selected?.id === g.id}
          expanded={expanded.has(g.id)}
          occurrences={data.selected?.id === g.id ? data.selected.occurrences : []}
          onSelect={selectGroup}
          onToggle={toggleExpanded}
        />
      {/each}
    </div>
  </div>

  <!-- RESIZER -->
  {#if pane.open && data.selected}
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
    style:flex-basis={pane.open && data.selected ? `${pane.pct}%` : '0%'}
    style:display={pane.open && data.selected ? 'block' : 'none'}
  >
    {#if data.selected}
      <DetailPanel
        group={data.selected}
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
  {#if !pane.open && data.selected}
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
