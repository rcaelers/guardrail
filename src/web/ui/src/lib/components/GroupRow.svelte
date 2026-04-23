<script lang="ts">
  import type { CrashGroupSummary } from '$lib/adapters/types';
  import SignalChip from './SignalChip.svelte';
  import StatusPill from './StatusPill.svelte';
  import Sparkline from './Sparkline.svelte';
  import { fmtDate, fmtInt } from '$lib/utils/format';

  interface Props {
    g: CrashGroupSummary;
    selected: boolean;
    expanded: boolean;
    occurrences?: { id: string; os: string; version: string; at: string; similarity: number }[];
    onSelect: (id: string) => void;
    onToggle: (id: string) => void;
  }
  let { g, selected, expanded, occurrences = [], onSelect, onToggle }: Props = $props();

  const COLS = '28px 1fr 80px 72px 80px 110px 90px';
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
    <div class="mb-[3px] truncate text-[13.5px] font-medium text-ink dark:text-ink-dark">{g.title}</div>
    <div class="truncate font-mono text-[11px] text-ink-muted dark:text-ink-mutedDark">
      {g.topFrame}  ·  {g.file}:{g.line}
    </div>
  </div>
  <SignalChip signal={g.signal} />
  <div class="font-mono text-xs text-ink-muted dark:text-ink-mutedDark">{g.version}</div>
  <div class="text-sm font-medium tabular-nums text-ink dark:text-ink-dark">{fmtInt(g.count)}</div>
  <Sparkline count={g.count} />
  <StatusPill status={g.status} />
</div>

{#if expanded}
  {#each occurrences.slice(0, 6) as occ}
    <div
      class="grid items-center gap-4 border-b border-line dark:border-line-dark bg-[#fbfbfc] dark:bg-[#18181a] py-2 pl-12 pr-5 font-mono text-xs text-ink-muted dark:text-ink-mutedDark"
      style:grid-template-columns={COLS}
    >
      <span></span>
      <span>{occ.id}  ·  {occ.os}</span>
      <span></span>
      <span>{occ.version}</span>
      <span></span>
      <span>{fmtDate(occ.at)}</span>
      <span>{(occ.similarity * 100).toFixed(1)}%</span>
    </div>
  {/each}
  {#if g.count > 6}
    <div class="border-b border-line dark:border-line-dark bg-[#fbfbfc] dark:bg-[#18181a] py-2 pl-12 pr-5 text-[11px] text-ink-muted dark:text-ink-mutedDark">
      + {fmtInt(g.count - 6)} more occurrences
    </div>
  {/if}
{/if}
