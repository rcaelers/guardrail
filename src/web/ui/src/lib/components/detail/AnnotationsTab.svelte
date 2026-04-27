<script lang="ts">
  let { annotations }: { annotations: Record<string, string> | undefined } = $props();

  const entries = $derived(
    Object.entries(annotations ?? {}).sort(([a], [b]) => a.localeCompare(b))
  );
</script>

{#if entries.length === 0}
  <div class="rounded border border-dashed border-line px-3 py-3 text-[12px] text-ink-muted dark:border-line-dark dark:text-ink-mutedDark">
    No annotations for this crash.
  </div>
{:else}
  <div class="grid gap-x-5 gap-y-2 text-[12.5px]" style:grid-template-columns="180px 1fr">
    {#each entries as [key, value]}
      <div class="truncate text-ink-muted dark:text-ink-mutedDark" title={key}>{key}</div>
      <div class="break-all font-mono text-xs text-ink dark:text-ink-dark">{value}</div>
    {/each}
  </div>
{/if}
