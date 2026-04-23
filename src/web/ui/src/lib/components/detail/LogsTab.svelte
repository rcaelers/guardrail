<script lang="ts">
  import type { LogFile } from '$lib/adapters/types';
  import { fmtInt } from '$lib/utils/format';

  let { logs }: { logs: LogFile[] } = $props();
  let active = $state(0);
  let wrap = $state(false);

  $effect(() => { active = 0; wrap = false; });

  const current = $derived(logs[active]);
  const lines = $derived(current?.body.split('\n') ?? []);
  const levelColor = (l: string) =>
    /\bERROR\b|panic|fatal/i.test(l) ? 'text-signal-danger dark:text-signal-dangerDark' :
    /\bWARN\b/i.test(l) ? 'text-signal-warn dark:text-signal-warnDark' :
    /\bDEBUG\b/i.test(l) ? 'text-ink-muted dark:text-ink-mutedDark' :
    'text-ink dark:text-ink-dark';
</script>

{#if !logs || logs.length === 0}
  <div class="text-xs text-ink-muted dark:text-ink-mutedDark">No log files attached.</div>
{:else}
  <div class="-mx-7 -my-5 flex min-h-[380px]">
    <!-- File sidebar -->
    <div class="w-[180px] shrink-0 overflow-auto border-r border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark py-2">
      <div class="px-3.5 pb-2 pt-1 text-[10px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">
        Attached files
      </div>
      {#each logs as f, i}
        <button
          type="button"
          onclick={() => (active = i)}
          class="block w-full cursor-pointer border-l-2 px-3.5 py-2 text-left"
          class:border-accent={active === i}
          class:bg-accent-soft={active === i}
          class:dark:bg-accent-softDark={active === i}
          class:border-transparent={active !== i}
        >
          <div
            class="mb-[2px] truncate font-mono text-xs"
            class:text-accent={active === i}
            class:text-ink={active !== i}
            class:dark:text-ink-dark={active !== i}
          >{f.name}</div>
          <div class="text-[10.5px] text-ink-muted dark:text-ink-mutedDark">
            {f.size} · {fmtInt(f.lines)} lines
          </div>
        </button>
      {/each}
    </div>

    <!-- Log body -->
    <div class="flex min-w-0 flex-1 flex-col">
      <div class="flex items-center gap-2.5 border-b border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-4 py-2 text-[11px] text-ink-muted dark:text-ink-mutedDark">
        <span class="font-mono text-ink dark:text-ink-dark">{current.name}</span>
        <span>·</span>
        <span>{current.size}</span>
        <span>·</span>
        <span>{fmtInt(current.lines)} lines</span>
        <span class="flex-1"></span>
        <label class="inline-flex cursor-pointer items-center gap-1.5">
          <input type="checkbox" bind:checked={wrap} class="m-0"/>
          <span>wrap</span>
        </label>
        <button type="button" class="cursor-pointer rounded border border-line dark:border-line-dark bg-transparent px-2 py-0.5 text-[11px] text-ink dark:text-ink-dark">
          Download
        </button>
      </div>
      <div class="scroll-clean flex-1 overflow-auto bg-[#fbfbfa] dark:bg-[#101012] py-2.5 font-mono text-[11.5px] leading-[1.55]">
        {#each lines as l, i}
          <div class="grid px-3.5" style:grid-template-columns="44px 1fr">
            <span class="select-none pr-3.5 text-right text-ink-muted dark:text-ink-mutedDark">{i + 1}</span>
            <span
              class={levelColor(l)}
              class:whitespace-pre-wrap={wrap}
              class:break-words={wrap}
              class:whitespace-pre={!wrap}
              class:overflow-hidden={!wrap}
              class:text-ellipsis={!wrap}
            >{l || '\u00a0'}</span>
          </div>
        {/each}
      </div>
    </div>
  </div>
{/if}
