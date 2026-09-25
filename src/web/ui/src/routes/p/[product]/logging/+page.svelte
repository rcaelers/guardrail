<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageProps } from './$types';
  import { fmtDate } from '$lib/utils/format';

  let { data, form }: PageProps = $props();
  let selected = $state<string[]>([]);
  let retrying = $state(false);

  const retryable = $derived(data.importLogs.filter((entry) => entry.retryable));
  const allSelected = $derived(retryable.length > 0 && retryable.every((entry) => selected.includes(key(entry))));

  function key(entry: { kind: string; id: string }): string {
    return `${entry.kind}:${entry.id}`;
  }

  function toggle(value: string, checked: boolean) {
    selected = checked
      ? [...new Set([...selected, value])]
      : selected.filter((item) => item !== value);
  }

  function toggleAll(checked: boolean) {
    selected = checked ? retryable.map(key) : [];
  }

  function statusClass(status: string): string {
    if (status === 'retrying') return 'bg-accent-soft text-accent dark:bg-accent-softDark';
    return 'bg-amber-400/20 text-amber-800 dark:text-amber-400';
  }
</script>

<div class="flex h-full min-h-0 flex-col">
  <div class="flex shrink-0 flex-wrap items-center gap-3 border-b border-line px-5 py-3 dark:border-line-dark">
    <div>
      <div class="text-[14px] font-semibold">Import logging</div>
      <div class="text-[11.5px] text-ink-muted dark:text-ink-mutedDark">
        Processed crash and symbol imports that need attention
      </div>
    </div>
    <span class="flex-1"></span>
    <span class="text-xs text-ink-muted dark:text-ink-mutedDark">
      {data.importLogs.length} issue{data.importLogs.length === 1 ? '' : 's'}
    </span>
    <button
      type="submit"
      form="retry-imports"
      disabled={selected.length === 0 || retrying}
      class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white disabled:cursor-not-allowed disabled:opacity-40"
    >{retrying ? 'Queueing…' : `Retry selected (${selected.length})`}</button>
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
      Queued {form.queued} import{form.queued === 1 ? '' : 's'} for retry.
    </div>
  {/if}

  <form
    id="retry-imports"
    method="POST"
    action="?/retry"
    use:enhance={() => {
      retrying = true;
      return async ({ update }) => {
        await update();
        selected = [];
        retrying = false;
      };
    }}
    class="flex min-h-0 flex-1 flex-col"
  >
    <div
      class="grid shrink-0 items-center gap-4 border-b border-line bg-surface-panel px-5 py-2 text-[10.5px] font-medium uppercase tracking-wider text-ink-muted dark:border-line-dark dark:bg-surface-panelDark dark:text-ink-mutedDark"
      style:grid-template-columns="28px 82px 100px minmax(160px,0.8fr) minmax(260px,1.6fr) 80px 140px"
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
      <span>Error</span>
      <span>Attempts</span>
      <span>Last failure</span>
    </div>

    <div class="scroll-clean min-h-0 flex-1 overflow-auto">
      {#each data.importLogs as entry (key(entry))}
        {@const value = key(entry)}
        <div
          class="grid items-center gap-4 border-b border-line px-5 py-3 text-[13px] hover:bg-surface-panel dark:border-line-dark dark:hover:bg-surface-panelDark"
          style:grid-template-columns="28px 82px 100px minmax(160px,0.8fr) minmax(260px,1.6fr) 80px 140px"
        >
          <input
            type="checkbox"
            name="import"
            {value}
            aria-label={`Select ${entry.kind} ${entry.subject}`}
            checked={selected.includes(value)}
            disabled={!entry.retryable}
            onchange={(event) => toggle(value, event.currentTarget.checked)}
          />
          <span class={`w-fit rounded-full px-2 py-0.5 text-[10.5px] font-semibold uppercase tracking-wide ${statusClass(entry.status)}`}>
            {entry.status}
          </span>
          <span class="capitalize">{entry.kind}</span>
          <div class="min-w-0">
            <div class="truncate font-mono text-[12px]" title={entry.subject}>{entry.subject}</div>
            <div class="truncate font-mono text-[10px] text-ink-muted dark:text-ink-mutedDark" title={entry.id}>{entry.id}</div>
          </div>
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
