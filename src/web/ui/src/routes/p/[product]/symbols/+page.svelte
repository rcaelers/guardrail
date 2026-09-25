<script lang="ts">
  import { enhance } from '$app/forms';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import type { PageProps } from './$types';
  import Select from '$lib/components/Select.svelte';
  import { fmtDate } from '$lib/utils/format';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import RowSelectionMenu from '$lib/components/RowSelectionMenu.svelte';

  let { data, form }: PageProps = $props();

  const canUpload = $derived(data.role === 'readwrite' || data.role === 'maintainer');
  const canDelete = $derived(data.role === 'maintainer');
  let selected = $state<string[]>([]);
  let lastSelectedIndex = $state<number | null>(null);
  let dragging = $state(false);
  let dragChecked = $state(false);
  let selectionMenu = $state<{ x: number; y: number; index: number } | null>(null);
  let deleteIds = $state<string[]>([]);
  let deleteForm = $state<HTMLFormElement>();
  let deleting = $state(false);
  let pendingConfirm = $state<{ message: string; confirmLabel: string } | null>(null);

  const selectedSet = $derived(new Set(selected));
  const allSelected = $derived(data.symbols.length > 0 && data.symbols.every((symbol) => selected.includes(symbol.id)));

  async function updateParam(key: string, value: string) {
    selected = [];
    lastSelectedIndex = null;
    selectionMenu = null;
    const url = new URL($page.url);
    if (!value || value === 'all' || value === '') url.searchParams.delete(key);
    else url.searchParams.set(key, value);
    await goto(url, { keepFocus: true, noScroll: true, replaceState: true });
  }

  let showUpload = $state(false);
  let upName = $state('');
  let upVersion = $state('');
  let upArch = $state('x86_64');
  let upFormat = $state('PDB');

  function uploaderName(id: string) {
    return data.uploaders.find((u) => u.id === id)?.name ?? id;
  }

  function updateSelection(values: string[], checked: boolean) {
    const next = new Set(selected);
    for (const value of values) {
      if (checked) next.add(value);
      else next.delete(value);
    }
    selected = [...next];
  }

  function toggleAt(index: number, checked: boolean, range: boolean) {
    const symbol = data.symbols[index];
    if (!symbol || !canDelete) return;

    if (range && lastSelectedIndex !== null) {
      const start = Math.min(lastSelectedIndex, index);
      const end = Math.max(lastSelectedIndex, index);
      updateSelection(data.symbols.slice(start, end + 1).map((item) => item.id), checked);
    } else {
      updateSelection([symbol.id], checked);
    }
    lastSelectedIndex = index;
  }

  function beginPointerSelection(event: PointerEvent, index: number) {
    const symbol = data.symbols[index];
    if (event.button !== 0 || !symbol || !canDelete) return;
    event.preventDefault();
    event.currentTarget instanceof HTMLElement && event.currentTarget.focus();

    const checked = !selectedSet.has(symbol.id);
    toggleAt(index, checked, event.shiftKey);
    if (!event.shiftKey) {
      dragging = true;
      dragChecked = checked;
    }
  }

  function extendPointerSelection(index: number) {
    if (dragging) toggleAt(index, dragChecked, false);
  }

  function endPointerSelection() {
    dragging = false;
  }

  function handleCheckboxChange(event: Event, index: number) {
    const input = event.currentTarget as HTMLInputElement;
    const symbol = data.symbols[index];
    if (symbol && input.checked !== selectedSet.has(symbol.id)) {
      toggleAt(index, input.checked, false);
    }
  }

  function toggleAll(checked: boolean) {
    selected = checked ? data.symbols.map((symbol) => symbol.id) : [];
    lastSelectedIndex = null;
  }

  function openSelectionMenu(event: MouseEvent, index: number) {
    if (!canDelete || !data.symbols[index]) return;
    event.preventDefault();
    const padding = 8;
    selectionMenu = {
      x: Math.max(padding, Math.min(event.clientX, window.innerWidth - 210 - padding)),
      y: Math.max(padding, Math.min(event.clientY, window.innerHeight - 88 - padding)),
      index
    };
  }

  function selectNearby(index: number) {
    const anchor = data.symbols[index];
    if (!anchor || !canDelete) return;
    const anchorTime = Date.parse(anchor.uploadedAt);
    if (!Number.isFinite(anchorTime)) return;
    const fiveMinutes = 5 * 60 * 1000;
    updateSelection(
      data.symbols
        .filter((symbol) => {
          const time = Date.parse(symbol.uploadedAt);
          return Number.isFinite(time) && Math.abs(time - anchorTime) <= fiveMinutes;
        })
        .map((symbol) => symbol.id),
      true
    );
    lastSelectedIndex = index;
  }

  function selectSameVersion(index: number) {
    const anchor = data.symbols[index];
    if (!anchor?.version || !canDelete) return;
    updateSelection(
      data.symbols.filter((symbol) => symbol.version === anchor.version).map((symbol) => symbol.id),
      true
    );
    lastSelectedIndex = index;
  }

  function requestDelete(ids: string[], message: string) {
    deleteIds = ids;
    pendingConfirm = { message, confirmLabel: ids.length === 1 ? 'Delete' : `Delete ${ids.length}` };
  }

  function requestBulkDelete() {
    const ids = [...selected];
    if (ids.length === 0) return;
    const selectedSymbols = data.symbols.filter((symbol) => selectedSet.has(symbol.id));
    const referencedBy = selectedSymbols.reduce((total, symbol) => total + symbol.referencedBy, 0);
    const references = referencedBy > 0
      ? ` They are referenced by ${referencedBy} crash group${referencedBy === 1 ? '' : 's'}.`
      : '';
    requestDelete(
      ids,
      `Delete ${ids.length} selected symbol${ids.length === 1 ? '' : 's'}?${references} This cannot be undone.`
    );
  }
</script>

<svelte:window onpointerup={endPointerSelection} onpointercancel={endPointerSelection} />

{#if selectionMenu}
  <RowSelectionMenu
    x={selectionMenu.x}
    y={selectionMenu.y}
    version={data.symbols[selectionMenu.index]?.version ?? ''}
    onselectnearby={() => selectNearby(selectionMenu!.index)}
    onselectsameversion={() => selectSameVersion(selectionMenu!.index)}
    onclose={() => (selectionMenu = null)}
  />
{/if}

{#if pendingConfirm}
  <ConfirmDialog
    message={pendingConfirm.message}
    confirmLabel={pendingConfirm.confirmLabel}
    onconfirm={() => {
      pendingConfirm = null;
      if (deleteForm) {
        deleting = true;
        deleteForm.requestSubmit();
      }
    }}
    oncancel={() => (pendingConfirm = null)}
  />
{/if}

<div class="flex h-full min-h-0 flex-col">
  <!-- Toolbar -->
  <div class="flex shrink-0 flex-wrap items-center gap-3 border-b border-line dark:border-line-dark px-5 py-3">
    <input
      type="search"
      placeholder="Search name or debug ID…"
      value={data.filters.search ?? ''}
      onchange={(e) => updateParam('q', (e.currentTarget as HTMLInputElement).value)}
      class="w-[280px] rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-1.5 text-[13px] outline-none"
    />
    <Select
      label="Arch"
      value={String(data.filters.arch ?? 'all')}
      options={[['all', 'All'], ['x86_64', 'x86_64'], ['x86', 'x86'], ['arm64', 'arm64']]}
      onChange={(v) => updateParam('arch', v)}
    />
    <Select
      label="Format"
      value={String(data.filters.format ?? 'all')}
      options={[['all', 'All'], ['PDB', 'PDB'], ['dSYM', 'dSYM'], ['Breakpad', 'Breakpad'], ['ELF', 'ELF']]}
      onChange={(v) => updateParam('format', v)}
    />
    <Select
      label="Sort"
      value={String(data.filters.sort ?? 'recent')}
      options={[['recent', 'Recently uploaded'], ['name', 'Name'], ['size', 'Size']]}
      onChange={(v) => updateParam('sort', v)}
    />
    <span class="flex-1"></span>
    {#if canDelete}
      <span class="text-[11.5px] text-ink-muted dark:text-ink-mutedDark">
        Shift-click or drag to select; right-click a row for grouping options
      </span>
    {/if}
    <span class="text-xs text-ink-muted dark:text-ink-mutedDark">
      {data.symbols.length} symbol{data.symbols.length === 1 ? '' : 's'}
    </span>
    {#if canDelete}
      <button
        type="button"
        disabled={selected.length === 0 || deleting}
        class="rounded-md border border-red-500/50 px-3 py-1.5 text-[13px] font-medium text-red-700 disabled:cursor-not-allowed disabled:opacity-40 dark:text-red-400"
        onclick={requestBulkDelete}
      >{deleting ? 'Deleting…' : `Delete selected (${selected.length})`}</button>
    {/if}
    {#if canUpload}
      <button
        type="button"
        onclick={() => (showUpload = !showUpload)}
        class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white"
      >Upload</button>
    {/if}
  </div>

  <!-- Upload panel (inline) -->
  {#if showUpload && canUpload}
    <form
      method="POST"
      action="?/upload"
      use:enhance={() => async ({ update }) => {
        await update();
        showUpload = false;
        upName = ''; upVersion = '';
      }}
      class="flex shrink-0 flex-wrap items-end gap-3 border-b border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-5 py-3"
    >
      <label class="flex flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Name</span>
        <input name="name" required bind:value={upName} placeholder="mylib.dll" class="w-[220px] rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px] outline-none" />
      </label>
      <label class="flex flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Version</span>
        <input name="version" bind:value={upVersion} placeholder="1.0.0" class="w-[120px] rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px] outline-none" />
      </label>
      <label class="flex flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Arch</span>
        <select name="arch" bind:value={upArch} class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px]">
          {#each ['x86_64', 'x86', 'arm64'] as a}<option value={a}>{a}</option>{/each}
        </select>
      </label>
      <label class="flex flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Format</span>
        <select name="format" bind:value={upFormat} class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px]">
          {#each ['PDB', 'dSYM', 'Breakpad', 'ELF'] as f}<option value={f}>{f}</option>{/each}
        </select>
      </label>
      <div class="ml-auto flex gap-2">
        <button type="button" onclick={() => (showUpload = false)} class="rounded-md border border-line dark:border-line-dark bg-transparent px-3 py-1.5 text-[13px]">Cancel</button>
        <button type="submit" class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white">Upload symbol</button>
      </div>
    </form>
  {/if}

  {#if form?.error}
    <div class="shrink-0 border-b border-red-500/30 bg-red-500/10 px-5 py-2.5 text-[12.5px] text-red-700 dark:text-red-400">
      {form.error}
    </div>
  {:else if form?.deleted !== undefined}
    <div class="shrink-0 border-b border-accent/30 bg-accent-soft px-5 py-2.5 text-[12.5px] text-accent dark:bg-accent-softDark">
      Deleted {form.deleted} symbol{form.deleted === 1 ? '' : 's'}.
    </div>
  {/if}

  {#if canDelete}
    <form
      method="POST"
      action="?/delete"
      bind:this={deleteForm}
      use:enhance={() => {
        const submittedIds = new Set(deleteIds);
        return async ({ result, update }) => {
          try {
            await update();
            if (result.type === 'success') {
              selected = selected.filter((id) => !submittedIds.has(id));
              lastSelectedIndex = null;
              deleteIds = [];
            }
          } finally {
            deleting = false;
          }
        };
      }}
      class="hidden"
    >
      {#each deleteIds as id}<input type="hidden" name="symbol" value={id} />{/each}
    </form>
  {/if}

  <!-- Header -->
  <div
    class="grid shrink-0 items-center gap-4 border-b border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-5 py-2 text-[10.5px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark"
    style:grid-template-columns="28px 1.3fr 80px 100px 1fr 90px 80px 1.2fr 180px 1fr 100px"
  >
    {#if canDelete}
      <input
        type="checkbox"
        aria-label="Select all symbols"
        checked={allSelected}
        disabled={data.symbols.length === 0 || deleting}
        onchange={(event) => toggleAll(event.currentTarget.checked)}
      />
    {:else}<span></span>{/if}
    <span>Module</span>
    <span>Version</span>
    <span>Channel</span>
    <span>Build</span>
    <span>Arch</span>
    <span>Format</span>
    <span>Debug ID</span>
    <span>Uploaded</span>
    <span>Refs</span>
    <span></span>
  </div>

  <!-- Rows -->
  <div class="scroll-clean min-h-0 flex-1 overflow-auto">
    {#each data.symbols as s, index (s.id)}
      <div
        role="row"
        tabindex="-1"
        class="grid items-center gap-4 border-b border-line dark:border-line-dark px-5 py-2.5 text-[13px] hover:bg-surface-panel dark:hover:bg-surface-panelDark"
        style:grid-template-columns="28px 1.3fr 80px 100px 1fr 90px 80px 1.2fr 180px 1fr 100px"
        oncontextmenu={(event) => openSelectionMenu(event, index)}
      >
        {#if canDelete}
          <input
            type="checkbox"
            value={s.id}
            aria-label={`Select ${s.name} ${s.version}`}
            checked={selected.includes(s.id)}
            disabled={deleting}
            class="cursor-pointer disabled:cursor-not-allowed"
            title="Shift-click to select a range, or hold and drag across checkboxes"
            onpointerdown={(event) => beginPointerSelection(event, index)}
            onpointerenter={() => extendPointerSelection(index)}
            onclick={(event) => event.detail > 0 && event.preventDefault()}
            onchange={(event) => handleCheckboxChange(event, index)}
          />
        {:else}<span></span>{/if}
        <div class="min-w-0 truncate">
          <div class="truncate font-mono text-[12.5px] text-ink dark:text-ink-dark">{s.name}</div>
          <div class="truncate text-[10.5px] text-ink-muted dark:text-ink-mutedDark">{s.size}</div>
        </div>
        <div class="truncate">{s.version}</div>
        <div class="truncate text-[12px]">{s.channel}</div>
        <div class="min-w-0 truncate text-[12px]">
          <div class="truncate font-mono text-[11px]" title={s.commit}>{s.buildTag || '—'}</div>
          {#if s.commit}<div class="truncate font-mono text-[10px] text-ink-muted dark:text-ink-mutedDark" title={s.commit}>{s.commit.slice(0, 8)}</div>{/if}
        </div>
        <div class="truncate font-mono text-[12px]">{s.arch}</div>
        <div class="truncate">{s.format}</div>
        <div class="truncate font-mono text-[11px] text-ink-muted dark:text-ink-mutedDark">{s.debugId}</div>
        <div class="truncate text-[12px] text-ink-muted dark:text-ink-mutedDark">
          {fmtDate(s.uploadedAt)} · {uploaderName(s.uploadedBy)}
        </div>
        <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">{s.referencedBy} crash group{s.referencedBy === 1 ? '' : 's'}</div>
        <div class="flex justify-end">
          {#if canDelete}
            <button
              type="button"
              disabled={deleting}
              class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink-muted dark:text-ink-mutedDark hover:text-red-600 disabled:cursor-not-allowed disabled:opacity-40"
              onclick={() => requestDelete([s.id], `Delete ${s.name} (${s.version})?`)}
            >Delete</button>
          {/if}
        </div>
      </div>
    {:else}
      <div class="px-5 py-10 text-center text-[13px] text-ink-muted dark:text-ink-mutedDark">
        No symbols match these filters.
      </div>
    {/each}
  </div>
</div>
