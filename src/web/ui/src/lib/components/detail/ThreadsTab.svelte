<script lang="ts">
  import type { Crash } from '$lib/adapters/types';
  import { getCrashingThread, threadDisplayName } from '$lib/utils/crash-report';

  let { crash }: { crash: Crash } = $props();

  const threads = $derived(crash.threads ?? []);
  const crashingThread = $derived(getCrashingThread(crash));
</script>

<div class="font-mono">
  {#if threads.length === 0}
    <div class="rounded border border-dashed border-line px-3 py-3 text-[12px] text-ink-muted dark:border-line-dark dark:text-ink-mutedDark">
      No thread list in this report.
    </div>
  {:else}
    {#each threads as thread}
      <div class="flex items-center gap-3 border-b border-line dark:border-line-dark px-3 py-2.5">
        <span class="w-14 text-[11px] text-ink-muted dark:text-ink-mutedDark">#{thread.thread_id}</span>
        <span class="flex-1 text-ink dark:text-ink-dark">{threadDisplayName(crash, thread)}</span>
        <span class="text-[11px] text-ink-muted dark:text-ink-mutedDark">{thread.frame_count} frames</span>
        {#if thread.last_error_value}
          <span class="text-[11px] text-ink-muted dark:text-ink-mutedDark">{thread.last_error_value}</span>
        {/if}
        {#if crashingThread?.thread_id === thread.thread_id}
          <span class="rounded bg-[#fce8e8] px-1.5 py-0.5 font-sans text-[10px] font-medium text-signal-danger dark:bg-[rgba(208,72,72,0.18)] dark:text-signal-dangerDark">crashed</span>
        {/if}
      </div>
    {/each}
  {/if}
</div>
