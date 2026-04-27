<script lang="ts">
  import type { Crash, CrashGroupSummary } from '$lib/adapters/types';
  import SignalChip from './SignalChip.svelte';
  import StatusPill from './StatusPill.svelte';
  import Sparkline from './Sparkline.svelte';
  import { fmtDate, fmtInt } from '$lib/utils/format';

  interface Props {
    g: CrashGroupSummary;
    selected: boolean;
    expanded: boolean;
    /** Member crashes shown under the group row when expanded. */
    crashes?: Crash[];
    /** The crash id currently shown in the detail pane, if any. */
    selectedCrashId?: string | null;
    onSelect: (id: string) => void;
    onToggle: (id: string) => void;
    onSelectCrash: (crashId: string, groupId: string) => void;
  }
  let {
    g,
    selected,
    expanded,
    crashes = [],
    selectedCrashId = null,
    onSelect,
    onToggle,
    onSelectCrash
  }: Props = $props();

  const COLS = '28px 1fr 260px 72px 80px 110px 90px';

  const groupName = $derived(g.fingerprint || g.title || g.id);
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
    {#if subtitle}
      <div class="truncate font-mono text-[11px] text-ink-muted dark:text-ink-mutedDark">{subtitle}</div>
    {/if}
  </div>
  <SignalChip signal={exception} />
  <div class="font-mono text-xs text-ink-muted dark:text-ink-mutedDark">{g.version}</div>
  <div class="text-sm font-medium tabular-nums text-ink dark:text-ink-dark">{fmtInt(g.count)}</div>
  <Sparkline trend={g.trend} count={g.count} />
  <StatusPill status={g.status} />
</div>

{#if expanded}
  {#each crashes.slice(0, 6) as c (c.id)}
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
      <span class:text-ink={isActive} class:dark:text-ink-dark={isActive}>{c.id}  ·  {c.os}</span>
      <span></span>
      <span>{c.version}</span>
      <span></span>
      <span>{fmtDate(c.at)}</span>
      <span>{(c.similarity * 100).toFixed(1)}%</span>
    </div>
  {/each}
  {@const shown = Math.min(crashes.length, 6)}
  {#if g.count > shown}
    <div class="border-b border-line dark:border-line-dark bg-[#fbfbfc] dark:bg-[#18181a] py-2 pl-12 pr-5 text-[11px] text-ink-muted dark:text-ink-mutedDark">
      + {fmtInt(g.count - shown)} more crashes
    </div>
  {/if}
{/if}
