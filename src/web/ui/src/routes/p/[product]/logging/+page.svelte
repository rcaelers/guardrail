<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageProps } from './$types';
  import RowSelectionMenu from '$lib/components/RowSelectionMenu.svelte';
  import { fmtDate } from '$lib/utils/format';

  let { data, form }: PageProps = $props();
  let selected = $state<string[]>([]);
  let busy = $state<'retry' | 'delete' | null>(null);
  let lastSelectedIndex = $state<number | null>(null);
  let dragging = $state(false);
  let dragChecked = $state(false);
  let selectionMenu = $state<{ x: number; y: number; index: number } | null>(null);

  const retryable = $derived(data.importLogs.filter((entry) => entry.retryable));
  const selectedSet = $derived(new Set(selected));
  const allSelected = $derived(retryable.length > 0 && retryable.every((entry) => selected.includes(key(entry))));

  function key(entry: { kind: string; id: string }): string {
    return `${entry.kind}:${entry.id}`;
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
    const entry = data.importLogs[index];
    if (!entry?.retryable) return;

    if (range && lastSelectedIndex !== null) {
      const start = Math.min(lastSelectedIndex, index);
      const end = Math.max(lastSelectedIndex, index);
      updateSelection(
        data.importLogs.slice(start, end + 1).filter((item) => item.retryable).map(key),
        checked
      );
    } else {
      updateSelection([key(entry)], checked);
    }
    lastSelectedIndex = index;
  }

  function beginPointerSelection(event: PointerEvent, index: number) {
    const entry = data.importLogs[index];
    if (event.button !== 0 || !entry?.retryable) return;
    event.preventDefault();
    event.currentTarget instanceof HTMLElement && event.currentTarget.focus();

    const checked = !selectedSet.has(key(entry));
    toggleAt(index, checked, event.shiftKey);
    if (!event.shiftKey) {
      dragging = true;
      dragChecked = checked;
    }
  }

  function extendPointerSelection(index: number) {
    if (!dragging) return;
    toggleAt(index, dragChecked, false);
  }

  function endPointerSelection() {
    dragging = false;
  }

  function handleCheckboxChange(event: Event, index: number) {
    const input = event.currentTarget as HTMLInputElement;
    const entry = data.importLogs[index];
    if (entry && input.checked !== selectedSet.has(key(entry))) {
      toggleAt(index, input.checked, false);
    }
  }

  function toggleAll(checked: boolean) {
    selected = checked ? retryable.map(key) : [];
    lastSelectedIndex = null;
  }

  function openSelectionMenu(event: MouseEvent, index: number) {
    if (!data.importLogs[index]?.retryable) return;
    event.preventDefault();
    const padding = 8;
    selectionMenu = {
      x: Math.max(padding, Math.min(event.clientX, window.innerWidth - 210 - padding)),
      y: Math.max(padding, Math.min(event.clientY, window.innerHeight - 88 - padding)),
      index
    };
  }

  function selectNearby(index: number) {
    const nearbyAnchor = data.importLogs[index];
    if (!nearbyAnchor?.retryable) return;
    const anchorTime = Date.parse(nearbyAnchor.lastFailedAt);
    if (!Number.isFinite(anchorTime)) return;
    const fiveMinutes = 5 * 60 * 1000;
    const values = data.importLogs
      .filter((entry) => {
        if (!entry.retryable || entry.kind !== nearbyAnchor.kind) return false;
        const time = Date.parse(entry.lastFailedAt);
        return Number.isFinite(time) && Math.abs(time - anchorTime) <= fiveMinutes;
      })
      .map(key);
    updateSelection(values, true);
    lastSelectedIndex = index;
  }

  function selectSameVersion(index: number) {
    const anchor = data.importLogs[index];
    if (!anchor?.retryable || !anchor.version) return;
    updateSelection(
      data.importLogs
        .filter((entry) => entry.retryable && entry.kind === anchor.kind && entry.version === anchor.version)
        .map(key),
      true
    );
    lastSelectedIndex = index;
  }

  function statusClass(status: string): string {
    if (status === 'retrying') return 'bg-accent-soft text-accent dark:bg-accent-softDark';
    return 'bg-amber-400/20 text-amber-800 dark:text-amber-400';
  }
</script>

<svelte:window onpointerup={endPointerSelection} onpointercancel={endPointerSelection} />

{#if selectionMenu}
  <RowSelectionMenu
    x={selectionMenu.x}
    y={selectionMenu.y}
    version={data.importLogs[selectionMenu.index]?.version ?? ''}
    onselectnearby={() => selectNearby(selectionMenu!.index)}
    onselectsameversion={() => selectSameVersion(selectionMenu!.index)}
    onclose={() => (selectionMenu = null)}
  />
{/if}

<div class="flex h-full min-h-0 flex-col">
  <div class="flex shrink-0 flex-wrap items-center gap-3 border-b border-line px-5 py-3 dark:border-line-dark">
    <div>
      <div class="text-[14px] font-semibold">Import logging</div>
      <div class="text-[11.5px] text-ink-muted dark:text-ink-mutedDark">
        Shift-click or drag to select; right-click a row for grouping options
      </div>
    </div>
    <span class="flex-1"></span>
    <span class="text-xs text-ink-muted dark:text-ink-mutedDark">
      {data.importLogs.length} issue{data.importLogs.length === 1 ? '' : 's'}
    </span>
    <button
      type="submit"
      form="retry-imports"
      formaction="?/delete"
      disabled={selected.length === 0 || busy !== null}
      class="rounded-md border border-red-500/50 px-3 py-1.5 text-[13px] font-medium text-red-700 disabled:cursor-not-allowed disabled:opacity-40 dark:text-red-400"
    >{busy === 'delete' ? 'Deleting…' : `Delete selected (${selected.length})`}</button>
    <button
      type="submit"
      form="retry-imports"
      disabled={selected.length === 0 || busy !== null}
      class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white disabled:cursor-not-allowed disabled:opacity-40"
    >{busy === 'retry' ? 'Queueing…' : `Retry selected (${selected.length})`}</button>
  </div>

  {#if data.importLogs.length > 0}
    <div class="shrink-0 border-b border-amber-400/40 bg-amber-400/10 px-5 py-2.5 text-[12.5px] text-amber-900 dark:text-amber-300">
      These imports are still retained in object storage. Retrying requeues the original processed data; successful imports disappear from this list.
    </div>
  {/if}

  {#if form?.error}
    <div class="shrink-0 border-b border-red-500/30 bg-red-500/10 px-5 py-2.5 text-[12.5px] text-red-700 dark:text-red-400">
      {form.error}
    </div>
  {:else if form?.ok}
    <div class="shrink-0 border-b border-accent/30 bg-accent-soft px-5 py-2.5 text-[12.5px] text-accent dark:bg-accent-softDark">
      {#if form.queued !== undefined}
        Queued {form.queued} import{form.queued === 1 ? '' : 's'} for retry.
      {:else}
        Deleted {form.deleted} retained import{form.deleted === 1 ? '' : 's'}.
      {/if}
    </div>
  {/if}

  <form
    id="retry-imports"
    method="POST"
    action="?/retry"
    use:enhance={({ submitter, cancel }) => {
      const deleting = submitter?.getAttribute('formaction') === '?/delete';
      if (deleting && !window.confirm(`Permanently delete ${selected.length} retained import${selected.length === 1 ? '' : 's'}? This cannot be undone.`)) {
        cancel();
        return;
      }
      busy = deleting ? 'delete' : 'retry';
      return async ({ result, update }) => {
        try {
          await update();
          if (result.type === 'success') {
            selected = [];
            lastSelectedIndex = null;
          }
        } finally {
          busy = null;
        }
      };
    }}
    class="flex min-h-0 flex-1 flex-col"
  >
    <div
      class="grid shrink-0 items-center gap-4 border-b border-line bg-surface-panel px-5 py-2 text-[10.5px] font-medium uppercase tracking-wider text-ink-muted dark:border-line-dark dark:bg-surface-panelDark dark:text-ink-mutedDark"
      style:grid-template-columns="28px 82px 72px minmax(160px,0.8fr) 110px minmax(240px,1.5fr) 70px 140px"
    >
      <input
        type="checkbox"
        aria-label="Select all retryable imports"
        checked={allSelected}
        disabled={retryable.length === 0}
        onchange={(event) => toggleAll(event.currentTarget.checked)}
      />
      <span>Status</span>
      <span>Type</span>
      <span>Item</span>
      <span>Version</span>
      <span>Error</span>
      <span>Attempts</span>
      <span>Last failure</span>
    </div>

    <div class="scroll-clean min-h-0 flex-1 overflow-auto">
      {#each data.importLogs as entry, index (key(entry))}
        {@const value = key(entry)}
        <div
          role="row"
          tabindex="-1"
          class="grid items-center gap-4 border-b border-line px-5 py-3 text-[13px] hover:bg-surface-panel dark:border-line-dark dark:hover:bg-surface-panelDark"
          style:grid-template-columns="28px 82px 72px minmax(160px,0.8fr) 110px minmax(240px,1.5fr) 70px 140px"
          oncontextmenu={(event) => openSelectionMenu(event, index)}
        >
          <input
            type="checkbox"
            name="import"
            {value}
            aria-label={`Select ${entry.kind} ${entry.subject}`}
            checked={selected.includes(value)}
            disabled={!entry.retryable}
            class="cursor-pointer disabled:cursor-not-allowed"
            title={entry.retryable ? 'Shift-click to select a range, or hold and drag across checkboxes' : 'This import is currently retrying'}
            onpointerdown={(event) => beginPointerSelection(event, index)}
            onpointerenter={() => extendPointerSelection(index)}
            onclick={(event) => event.detail > 0 && event.preventDefault()}
            onchange={(event) => handleCheckboxChange(event, index)}
          />
          <span class={`w-fit rounded-full px-2 py-0.5 text-[10.5px] font-semibold uppercase tracking-wide ${statusClass(entry.status)}`}>
            {entry.status}
          </span>
          <span class="capitalize">{entry.kind}</span>
          <div class="min-w-0">
            <div class="truncate font-mono text-[12px]" title={entry.subject}>{entry.subject}</div>
            <div class="truncate font-mono text-[10px] text-ink-muted dark:text-ink-mutedDark" title={entry.id}>{entry.id}</div>
          </div>
          <span class="truncate font-mono text-[12px]" title={entry.version}>{entry.version || '—'}</span>
          <div class="min-w-0 truncate text-[12px] text-ink-muted dark:text-ink-mutedDark" title={entry.error}>
            {entry.error}
          </div>
          <span>{entry.attempts || '—'}</span>
          <span class="text-[12px] text-ink-muted dark:text-ink-mutedDark" title={entry.lastFailedAt}>
            {fmtDate(entry.lastFailedAt)}
          </span>
        </div>
      {:else}
        <div class="px-5 py-12 text-center">
          <div class="text-[14px] font-medium">No failed imports</div>
          <div class="mt-1 text-[12.5px] text-ink-muted dark:text-ink-mutedDark">
            Failed or stalled crash and symbol imports will appear here.
          </div>
        </div>
      {/each}
    </div>
  </form>
</div>
