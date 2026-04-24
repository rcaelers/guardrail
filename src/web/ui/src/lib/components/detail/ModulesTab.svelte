<script lang="ts">
  import type { Crash } from '$lib/adapters/types';
  import { mainModuleName } from '$lib/utils/crash-report';

  let { crash }: { crash: Crash } = $props();

  const modules = $derived(crash.modules ?? []);
  const mainModule = $derived(mainModuleName(crash));
</script>

<div class="font-mono text-[11.5px]">
  {#if modules.length === 0}
    <div class="rounded border border-dashed border-line px-3 py-3 text-[12px] text-ink-muted dark:border-line-dark dark:text-ink-mutedDark">
      No module list in this report.
    </div>
  {:else}
    {#each modules as module}
      {@const inApp = module.filename === mainModule}
      <div
        class="grid gap-3 border-b border-line dark:border-line-dark px-3 py-2.5"
        style:grid-template-columns="1fr 120px 144px 78px"
        class:text-ink={inApp}
        class:dark:text-ink-dark={inApp}
        class:text-ink-muted={!inApp}
        class:dark:text-ink-mutedDark={!inApp}
      >
        <span>{module.filename}</span>
        <span>{module.version || '—'}</span>
        <span>{module.base_addr}</span>
        <span class="text-right">
          {#if module.loaded_symbols}
            loaded
          {:else if module.missing_symbols}
            missing
          {:else if module.corrupt_symbols}
            corrupt
          {:else}
            partial
          {/if}
        </span>
      </div>
    {/each}
  {/if}
</div>
