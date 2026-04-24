<script lang="ts">
  import type { Crash } from '$lib/adapters/types';

  let { crash }: { crash: Crash } = $props();

  const handles = $derived(crash.handles ?? []);
</script>

<div class="font-mono text-[11.5px]">
  {#if handles.length === 0}
    <div class="rounded border border-dashed border-line px-3 py-3 text-[12px] text-ink-muted dark:border-line-dark dark:text-ink-mutedDark">
      No handle table in this report.
    </div>
  {:else}
    {#each handles as handle}
      <div class="grid gap-3 border-b border-line dark:border-line-dark px-3 py-2.5 text-ink dark:text-ink-dark" style:grid-template-columns="96px 160px 1fr">
        <span>{handle.handle}</span>
        <span>{handle.type_name || '—'}</span>
        <span class="truncate text-ink-muted dark:text-ink-mutedDark">{handle.object_name || '—'}</span>
      </div>
    {/each}
  {/if}
</div>
