<script lang="ts">
  import { page } from '$app/stores';
  import type { RelatedRef } from '$lib/adapters/types';
  import { fmtInt } from '$lib/utils/format';

  interface Props { related: RelatedRef[]; onMerge: (mergedId: string) => void; canMerge?: boolean; }
  let { related, onMerge, canMerge = true }: Props = $props();

  // Link back to the same product-scoped crashes list.
  const productId = $derived($page.params.product);
</script>

{#if related.length === 0}
  <div class="text-xs text-ink-muted dark:text-ink-mutedDark">No related groups.</div>
{:else}
  {#each related as r}
    <div class="mb-1.5 flex items-center gap-3.5 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3.5 py-3">
      <span class="font-mono text-[11px] text-ink-muted dark:text-ink-mutedDark">{r.id}</span>
      <a
        href={productId ? `/p/${productId}/crashes?id=${r.id}` : `/crashes?id=${r.id}`}
        class="flex-1 truncate text-[13px] text-ink hover:text-accent dark:text-ink-dark"
      >{r.title}</a>
      <span class="text-[11px] text-ink-muted dark:text-ink-mutedDark">{fmtInt(r.count)} events</span>
      {#if canMerge}
        <button
          type="button"
          onclick={() => onMerge(r.id)}
          class="cursor-pointer rounded border border-line dark:border-line-dark bg-transparent px-2.5 py-1 font-sans text-[11px] text-ink dark:text-ink-dark"
        >Merge</button>
      {:else}
        <span class="rounded border border-line dark:border-line-dark px-2.5 py-1 text-[11px] text-ink-muted dark:text-ink-mutedDark" title="Only maintainers can merge">Merge</span>
      {/if}
    </div>
  {/each}
{/if}
