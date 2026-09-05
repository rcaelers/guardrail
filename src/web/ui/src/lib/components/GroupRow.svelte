<script lang="ts">
  import type { CrashSummary, CrashGroupSummary } from '$lib/adapters/types';
  import SignalChip from './SignalChip.svelte';
  import StatusPill from './StatusPill.svelte';
  import Sparkline from './Sparkline.svelte';
  import { fmtDate, fmtInt } from '$lib/utils/format';

  interface Props {
    g: CrashGroupSummary;
    selected: boolean;
    expanded: boolean;
    /** Member crashes shown under the group row when expanded. */
    crashes?: CrashSummary[];
    /** Total member crashes in the group, of which `crashes` may be a prefix. */
    total?: number;
    /** True while the remaining member crashes are being fetched. */
    loadingMore?: boolean;
    /** The crash id currently shown in the detail pane, if any. */
    selectedCrashId?: string | null;
    canDelete?: boolean;
    onSelect: (id: string) => void;
    onToggle: (id: string) => void;
    onLoadMore: (id: string) => void;
    onSelectCrash: (crashId: string, groupId: string) => void;
    onDeleteGroup?: (id: string) => void;
    onDeleteCrash?: (crashId: string, groupId: string) => void;
  }
  let {
    g,
    selected,
    expanded,
    crashes = [],
    total,
    loadingMore = false,
    selectedCrashId = null,
    canDelete = false,
    onSelect,
    onToggle,
    onLoadMore,
    onSelectCrash,
    onDeleteGroup,
    onDeleteCrash
  }: Props = $props();

  const crashTotal = $derived(total ?? g.count);
  const remaining = $derived(Math.max(0, crashTotal - crashes.length));

  // No version column: a group's crashes can span several versions, so a single
  // value on the group row would be arbitrary. Individual crashes still show
  // theirs, in the column the group uses for its exception.
  const COLS = '28px 1fr 260px 80px 110px 90px 76px';

  const groupName = $derived(g.fingerprint || g.title || g.id);
  // Only UUIDs are abbreviated; readable ids are shown in full.
  const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
  const shortId = $derived(UUID.test(g.id) ? g.id.slice(0, 8) : g.id);
  const exception = $derived(g.exceptionType || g.exceptionTypeShort || g.signal);
  const subtitle = $derived.by(() => {
    const parts: string[] = [];
    if (g.topFrame) parts.push(g.topFrame);
    if (g.file) parts.push(g.line ? `${g.file}:${g.line}` : g.file);
    return parts.join('  ·  ');
  });
</script>

<div
  role="button"
  tabindex="0"
  onclick={() => onSelect(g.id)}
  onkeydown={(e) => { if (e.key === 'Enter') onSelect(g.id); }}
  class="grid cursor-pointer items-center gap-4 border-b border-line dark:border-line-dark px-5 py-3.5 transition-colors"
  class:bg-accent-soft={selected}
  class:dark:bg-accent-softDark={selected}
  class:hover:bg-[#f6f6f7]={!selected}
  class:dark:hover:bg-[#1f1f22]={!selected}
  style:grid-template-columns={COLS}
>
  <button
    type="button"
    onclick={(e) => { e.stopPropagation(); onToggle(g.id); }}
    class="flex h-[22px] w-[22px] items-center justify-center rounded text-ink-muted dark:text-ink-mutedDark"
    aria-label={expanded ? 'Collapse' : 'Expand'}
  >
    <svg width="10" height="10" viewBox="0 0 10 10" style:transform={expanded ? 'rotate(90deg)' : ''} class="transition-transform">
      <path d="M3 2 L7 5 L3 8" stroke="currentColor" stroke-width="1.4" fill="none" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>
  </button>
  <div class="min-w-0">
    <div class="mb-[3px] truncate text-[13.5px] font-medium text-ink dark:text-ink-dark">{groupName}</div>
    <div class="flex min-w-0 items-center gap-2 font-mono text-[11px] text-ink-muted dark:text-ink-mutedDark">
      <span class="shrink-0 text-ink-muted dark:text-ink-mutedDark" title="Crash group {g.id}">{shortId}</span>
      {#if subtitle}
        <span class="truncate">{subtitle}</span>
      {/if}
    </div>
  </div>
  <SignalChip signal={exception} />
  <div class="text-sm font-medium tabular-nums text-ink dark:text-ink-dark">{fmtInt(g.count)}</div>
  <Sparkline trend={g.trend} count={g.count} />
  <StatusPill status={g.status} />
  <div class="flex justify-end">
    {#if canDelete && onDeleteGroup}
      <button
        type="button"
        onclick={(e) => { e.stopPropagation(); onDeleteGroup(g.id); }}
        class="rounded-md border border-line dark:border-line-dark bg-transparent px-2 py-1 text-[11px] text-ink-muted hover:text-red-600 dark:text-ink-mutedDark"
      >Delete</button>
    {/if}
  </div>
</div>

{#if expanded}
  {#each crashes as c (c.id)}
    {@const isActive = selectedCrashId === c.id}
    <div
      role="button"
      tabindex="0"
      onclick={(e) => { e.stopPropagation(); onSelectCrash(c.id, g.id); }}
      onkeydown={(e) => { if (e.key === 'Enter') { e.stopPropagation(); onSelectCrash(c.id, g.id); } }}
      class="grid cursor-pointer items-center gap-4 border-b border-line dark:border-line-dark py-2 pl-12 pr-5 font-mono text-xs text-ink-muted dark:text-ink-mutedDark transition-colors"
      class:bg-accent-soft={isActive}
      class:dark:bg-accent-softDark={isActive}
      class:bg-[#fbfbfc]={!isActive}
      class:dark:bg-[#18181a]={!isActive}
      class:hover:bg-[#f2f2f4]={!isActive}
      class:dark:hover:bg-[#212124]={!isActive}
      style:grid-template-columns={COLS}
    >
      <span></span>
      <span class:text-ink={isActive} class:dark:text-ink-dark={isActive}>
        {c.id}{#if c.os}  ·  {c.os}{/if}
        {#if c.hasUserText}
          {#if c.userTextAvailable === false}
            <span
              class="ml-1.5 rounded-sm border border-dashed border-amber-600/50 px-1 py-px font-sans text-[10px] text-amber-700 line-through dark:text-amber-500"
              title="The reporter described this crash, but the stored text is no longer in storage"
            >text</span>
          {:else}
            <span
              class="ml-1.5 rounded-sm bg-accent-soft dark:bg-accent-softDark px-1 py-px font-sans text-[10px] text-accent"
              title="The reporter described this crash"
            >text</span>
          {/if}
        {/if}
      </span>
      <span>{c.version}</span>
      <span></span>
      <span>{fmtDate(c.at)}</span>
      <span>{(c.similarity * 100).toFixed(1)}%</span>
      <div class="flex justify-end">
        {#if canDelete && onDeleteCrash}
          <button
            type="button"
            onclick={(e) => { e.stopPropagation(); onDeleteCrash(c.id, g.id); }}
            class="rounded-md border border-line dark:border-line-dark bg-transparent px-2 py-1 font-sans text-[11px] text-ink-muted hover:text-red-600 dark:text-ink-mutedDark"
          >Delete</button>
        {/if}
      </div>
    </div>
  {/each}
  {#if loadingMore}
    <div
      class="border-b border-line dark:border-line-dark bg-[#fbfbfc] dark:bg-[#18181a] py-2 pl-12 pr-5 text-[11px] text-ink-muted dark:text-ink-mutedDark"
    >
      Loading…
    </div>
  {:else if remaining > 0}
    <div
      role="button"
      tabindex="0"
      onclick={(e) => { e.stopPropagation(); onLoadMore(g.id); }}
      onkeydown={(e) => { if (e.key === 'Enter') { e.stopPropagation(); onLoadMore(g.id); } }}
      class="border-b border-line dark:border-line-dark bg-[#fbfbfc] dark:bg-[#18181a] py-2 pl-12 pr-5 text-[11px] text-ink-muted dark:text-ink-mutedDark cursor-pointer hover:bg-[#f2f2f4] dark:hover:bg-[#212124] transition-colors"
    >
      + {fmtInt(remaining)} more · <span class="text-accent underline">Load more</span>
    </div>
  {/if}
{/if}
