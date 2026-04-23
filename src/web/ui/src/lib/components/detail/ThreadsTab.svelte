<script lang="ts">
  import type { Thread } from '$lib/adapters/types';
  let { threads }: { threads: Thread[] } = $props();
</script>

<div class="font-mono">
  {#each threads as th}
    <div class="flex items-center gap-3 border-b border-line dark:border-line-dark px-3 py-2.5">
      <span class="w-7 text-[11px] text-ink-muted dark:text-ink-mutedDark">#{th.id}</span>
      <span class="flex-1 text-ink dark:text-ink-dark">{th.name}</span>
      <span class="text-[11px] text-ink-muted dark:text-ink-mutedDark">{th.frames} frames</span>
      {#if th.crashed}
        <span class="rounded bg-[#fce8e8] dark:bg-[rgba(208,72,72,0.18)] px-1.5 py-0.5 font-sans text-[10px] font-medium text-signal-danger dark:text-signal-dangerDark">crashed</span>
      {/if}
    </div>
  {/each}
</div>
