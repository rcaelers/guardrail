<script lang="ts">
  import type { Crash } from '$lib/adapters/types';
  import { getCrashingThread, mainModuleName, shortenSourcePath } from '$lib/utils/crash-report';

  let { crash }: { crash: Crash } = $props();

  const crashingThread = $derived(getCrashingThread(crash));
  const frames = $derived(crashingThread?.frames ?? []);
  const mainModule = $derived(mainModuleName(crash));
</script>

<div class="font-mono">
  {#if frames.length === 0}
    <div class="rounded border border-dashed border-line px-3 py-3 text-[12px] text-ink-muted dark:border-line-dark dark:text-ink-mutedDark">
      No crashing thread frames in this report.
    </div>
  {:else}
    {#each frames as frame, i}
      {@const inApp = frame.module === mainModule}
      <div
        class="mb-1 rounded px-3 py-2.5"
        class:bg-[#f6f8fc]={inApp}
        class:dark:bg-[#1d1d20]={inApp}
        class:border={inApp}
        class:border-line={inApp}
        class:dark:border-line-dark={inApp}
        class:opacity-65={!inApp}
      >
        <div class="text-[12.5px] text-ink dark:text-ink-dark">
          <span class="mr-2.5 text-ink-muted dark:text-ink-mutedDark">#{i}</span>
          {frame.function || `<${frame.module ?? 'unknown'}+${frame.module_offset ?? '??'}>`}
        </div>
        <div class="pl-[22px] text-[11px] text-ink-muted dark:text-ink-mutedDark">
          {shortenSourcePath(frame.file)}{frame.line ? `:${frame.line}` : ''}  ·  {frame.offset}
          {#if frame.trust}  ·  {frame.trust}{/if}
        </div>
      </div>
    {/each}
  {/if}
</div>
