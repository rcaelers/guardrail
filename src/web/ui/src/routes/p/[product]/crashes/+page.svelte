<script lang="ts">
  import { goto, invalidateAll, replaceState } from '$app/navigation';
  import { page } from '$app/stores';
  import type { PageData } from './$types';
  import type { Crash, CrashGroup, CrashSummary, Status } from '$lib/adapters/types';

  import Select from '$lib/components/Select.svelte';
  import GroupRow from '$lib/components/GroupRow.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import DetailPanel from '$lib/components/detail/DetailPanel.svelte';
  import { pane } from '$lib/stores/pane.svelte';

  let { data }: { data: PageData } = $props();

  const readOnly = $derived(data.role === 'readonly' && !data.user?.isAdmin);
  const canDelete = $derived(!readOnly);
  const canMerge = $derived(data.role === 'maintainer' || !!data.user?.isAdmin);
  let pendingConfirm = $state<{ message: string; confirmLabel: string; action: () => Promise<void> } | null>(null);

  // Expanded-group state (per-row chevron) — local to session. Any number of
  // groups can be open at once; expanding one never closes another.
  let expanded = $state<Set<string>>(new Set());
  function toggleExpanded(id: string) {
    const next = new Set(expanded);
    if (next.has(id)) next.delete(id); else next.add(id);
    expanded = next;
  }

  // Member crashes per group. Every group in the list arrives with a short
  // inline preview, so expanding a row renders immediately with no request;
  // this map only holds the full lists pulled by "load more".
  let loadedCrashes = $state<Record<string, CrashSummary[]>>({});
  let loadingCrashes = $state<Record<string, boolean>>({});

  function crashesFor(g: { id: string; crashes?: CrashSummary[] }): CrashSummary[] {
    return loadedCrashes[g.id] ?? g.crashes ?? [];
  }

  // Drops the full list so the row falls back to the (freshly loaded) preview.
  function forgetLoaded(groupId: string) {
    if (groupId in loadedCrashes) {
      const next = { ...loadedCrashes };
      delete next[groupId];
      loadedCrashes = next;
    }
  }

  async function loadAllCrashes(groupId: string) {
    if (loadingCrashes[groupId]) return;
    loadingCrashes[groupId] = true;
    try {
      const params = new URLSearchParams();
      if (data.filters.userText) params.set('hasUserText', 'true');
      if (data.filters.version && data.filters.version !== 'all')
        params.set('version', data.filters.version);
      const query = params.size ? `?${params}` : '';
      const r = await fetch(
        `/p/${encodeURIComponent($page.params.product!)}/crashes/${encodeURIComponent(groupId)}/events${query}`
      );
      if (!r.ok) return;
      const body = (await r.json()) as { crashes: CrashSummary[] };
      loadedCrashes[groupId] = body.crashes;
    } finally {
      loadingCrashes[groupId] = false;
    }
  }

  // ---- Selection ----
  // Selecting a crash only changes the detail pane, but going through `goto`
  // re-ran the page load, and with it the group-list query — a scan across
  // every crash row in the product. Fetch the crash on its own and move the URL
  // with shallow routing, which leaves the already-correct list untouched.
  // `data` still supplies the selection on first render and after a reload, so
  // deep links and the back button keep working.
  let picked = $state<{ crash: Crash; group: CrashGroup } | null>(null);
  let loadingCrashId = $state<string | null>(null);

  const activeCrash = $derived(picked?.crash ?? data.selectedCrash);
  const activeGroup = $derived(picked?.group ?? data.selectedGroup);

  // A pick belongs to the list it was made from. Changing a filter reloads the
  // list and the server resolves a fresh default, but the picked crash may not
  // even be in the new list — so hand the pane back to `data`. Selecting a
  // crash moves the URL shallowly and leaves the filters alone, so this does
  // not fire on selection.
  const filterKey = $derived(
    [
      data.filters.version,
      data.filters.status,
      data.filters.search,
      data.filters.sort,
      data.filters.userText,
      data.filters.page,
      data.filters.limit
    ].join('\u0000')
  );
  $effect(() => {
    filterKey;
    picked = null;
  });

  async function showCrash(crashId: string, force = false) {
    if (!force && (activeCrash?.id === crashId || loadingCrashId === crashId)) return;
    loadingCrashId = crashId;
    const url = new URL($page.url);
    url.searchParams.delete('id');
    url.searchParams.set('crash', crashId);
    replaceState(url, {});
    try {
      const r = await fetch(
        `/p/${encodeURIComponent($page.params.product!)}/crashes/detail/${encodeURIComponent(crashId)}`
      );
      if (!r.ok) return;
      picked = (await r.json()) as { crash: Crash; group: CrashGroup };
    } finally {
      if (loadingCrashId === crashId) loadingCrashId = null;
    }
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

  // Selecting a group shows its newest crash and expands the row so the other
  // crashes are there to pick from. The preview shipped with the list already
  // names that crash, so no lookup is needed to find it.
  async function selectGroup(id: string) {
    pane.open = true;
    if (!expanded.has(id)) toggleExpanded(id);
    const first = crashesFor({ id, crashes: data.list.groups.find((g) => g.id === id)?.crashes })[0];
    if (first) await showCrash(first.id);
  }

  // Selecting a specific crash within an (expanded) group.
  async function selectCrash(crashId: string, groupId: string) {
    pane.open = true;
    if (!expanded.has(groupId)) toggleExpanded(groupId);
    await showCrash(crashId);
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
  // invalidateAll refreshes the list; the detail pane is client-side state, so
  // drop the pick and re-read it if the server load lands elsewhere.
  async function refreshAfterMutation() {
    const id = activeCrash?.id ?? null;
    picked = null;
    await invalidateAll();
    if (id && data.selectedCrash?.id !== id) await showCrash(id, true);
  }

  async function setStatus(s: Status) {
    if (!activeGroup) return;
    const body = new FormData();
    body.set('id', activeGroup.id);
    body.set('status', s);
    await fetch('?/setStatus', { method: 'POST', body });
    await refreshAfterMutation();
  }
  async function addNote(noteBody: string) {
    if (!activeGroup) return;
    const body = new FormData();
    body.set('id', activeGroup.id);
    body.set('body', noteBody);
    body.set('author', 'you');
    await fetch('?/addNote', { method: 'POST', body });
    await refreshAfterMutation();
  }
  async function merge(mergedId: string) {
    if (!activeGroup) return;
    const body = new FormData();
    body.set('primaryId', activeGroup.id);
    body.set('mergedId', mergedId);
    await fetch('?/merge', { method: 'POST', body });
    await refreshAfterMutation();
  }

  function confirmDeleteCrash(crashId: string, groupId?: string) {
    pendingConfirm = {
      message: 'Permanently delete this crash event?',
      confirmLabel: 'Delete crash',
      action: () => deleteCrash(crashId, groupId)
    };
  }

  function confirmDeleteGroup(groupId: string) {
    pendingConfirm = {
      message: 'Permanently delete this entire crash group and all crash events in it?',
      confirmLabel: 'Delete group',
      action: () => deleteGroup(groupId)
    };
  }

  async function deleteCrash(crashId: string, groupId?: string) {
    const body = new FormData();
    body.set('id', crashId);
    const r = await fetch('?/deleteCrash', { method: 'POST', body });
    if (!r.ok) return;
    forgetLoaded(groupId ?? activeCrash?.groupId ?? '');
    const url = new URL($page.url);
    if (activeCrash?.id === crashId) {
      url.searchParams.delete('crash');
      url.searchParams.delete('id');
      pane.open = false;
      await goto(url, { keepFocus: true, noScroll: true, replaceState: true });
    }
    await invalidateAll();
  }

  async function deleteGroup(groupId: string) {
    const body = new FormData();
    body.set('id', groupId);
    const r = await fetch('?/deleteGroup', { method: 'POST', body });
    if (!r.ok) return;
    const next = new Set(expanded);
    next.delete(groupId);
    expanded = next;
    forgetLoaded(groupId);
    const url = new URL($page.url);
    if (activeGroup?.id === groupId || url.searchParams.get('id') === groupId) {
      url.searchParams.delete('id');
      url.searchParams.delete('crash');
      pane.open = false;
      await goto(url, { keepFocus: true, noScroll: true, replaceState: true });
    }
    await invalidateAll();
  }
</script>

{#if pendingConfirm}
  <ConfirmDialog
    message={pendingConfirm.message}
    confirmLabel={pendingConfirm.confirmLabel}
    onconfirm={() => {
      const action = pendingConfirm!.action;
      pendingConfirm = null;
      void action();
    }}
    oncancel={() => (pendingConfirm = null)}
  />
{/if}

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
      <label
        class="inline-flex cursor-pointer items-center gap-1.5 text-xs text-ink-muted dark:text-ink-mutedDark"
        title="Show only crashes the reporter described"
      >
        <input
          type="checkbox"
          checked={data.filters.userText}
          onchange={(e) => updateParam('userText', (e.currentTarget as HTMLInputElement).checked ? 'yes' : '', true)}
          class="m-0"
        />
        <span>Has user text</span>
      </label>
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
      style:grid-template-columns="28px 1fr 260px 80px 110px 90px 76px"
    >
      <span></span>
      <span>Crash group</span>
      <span>Exception</span>
      <span>Events</span>
      <span>30d trend</span>
      <span>Status</span>
      <span></span>
    </div>

    <!-- Rows -->
    <div class="scroll-clean min-h-0 flex-1 overflow-auto">
      {#each data.list.groups as g (g.id)}
        <GroupRow
          {g}
          selected={activeGroup?.id === g.id}
          expanded={expanded.has(g.id)}
          crashes={crashesFor(g)}
          total={g.matchingCount ?? g.count}
          loadingMore={loadingCrashes[g.id] ?? false}
          selectedCrashId={activeCrash?.id ?? null}
          {canDelete}
          onSelect={selectGroup}
          onToggle={toggleExpanded}
          onLoadMore={loadAllCrashes}
          onSelectCrash={selectCrash}
          onDeleteCrash={confirmDeleteCrash}
          onDeleteGroup={confirmDeleteGroup}
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
  {#if pane.open && activeGroup && activeCrash}
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
    style:flex-basis={pane.open && activeCrash ? `${pane.pct}%` : '0%'}
    style:display={pane.open && activeCrash ? 'block' : 'none'}
  >
    {#if activeGroup && activeCrash}
      <DetailPanel
        group={activeGroup}
        crash={activeCrash}
        onStatusChange={setStatus}
        onMerge={merge}
        onAddNote={addNote}
        onDeleteCrash={confirmDeleteCrash}
        onDeleteGroup={confirmDeleteGroup}
        {readOnly}
        {canMerge}
        onClose={() => (pane.open = false)}
      />
    {/if}
  </div>

  <!-- Collapsed detail rail -->
  {#if !pane.open && activeCrash}
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
