<script lang="ts">
  import { page } from '$app/stores';
  import type { RelatedRef } from '$lib/adapters/types';
  import { fmtInt } from '$lib/utils/format';
  import StatusPill from '$lib/components/StatusPill.svelte';

  interface Props { related: RelatedRef[]; onMerge: (mergedId: string) => void; canMerge?: boolean; }
  let { related, onMerge, canMerge = true }: Props = $props();

  // Link back to the same product-scoped crashes list.
  const productId = $derived($page.params.product);

  const pct = (s?: number) => (s === undefined ? '' : `${Math.round(s * 100)}%`);
</script>

{#if related.length === 0}
  <div class="text-xs text-ink-muted dark:text-ink-mutedDark">
    No group shares a crash site with this one.
  </div>
{:else}
  <div class="mb-3 text-[11px] text-ink-muted dark:text-ink-mutedDark">
    Ranked by how much of the stack matches, innermost frame first. 100% is the
    same stack; module name case is ignored.
  </div>

  {#each related as r (r.id)}
    <div class="mb-1.5 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3.5 py-3">
      <div class="mb-1.5 flex items-center gap-3">
        <span
          class="shrink-0 rounded bg-surface px-1.5 py-0.5 font-mono text-[11px] tabular-nums text-ink dark:bg-surface-dark dark:text-ink-dark"
          title="Share of the stack that matches"
        >{pct(r.similarity)}</span>
        <a
          href={productId ? `/p/${productId}/crashes?id=${r.id}` : `/crashes?id=${r.id}`}
          class="shrink-0 font-mono text-[11px] text-ink-muted hover:text-accent dark:text-ink-mutedDark"
        >{r.id}</a>
        {#if r.status}
          <StatusPill status={r.status} />
        {/if}
        <span class="flex-1"></span>
        <span class="shrink-0 text-[11px] text-ink-muted dark:text-ink-mutedDark">{fmtInt(r.count)} events</span>
        {#if canMerge}
          <button
            type="button"
            onclick={() => onMerge(r.id)}
            class="shrink-0 cursor-pointer rounded border border-line dark:border-line-dark bg-transparent px-2.5 py-1 font-sans text-[11px] text-ink dark:text-ink-dark"
          >Merge</button>
        {:else}
          <span class="shrink-0 rounded border border-line dark:border-line-dark px-2.5 py-1 text-[11px] text-ink-muted dark:text-ink-mutedDark" title="Only maintainers can merge">Merge</span>
        {/if}
      </div>

      <!-- The fingerprint decides whether this is the same bug, so it is shown
           in full rather than truncated to one line. -->
      <div class="max-h-[6.5rem] overflow-y-auto whitespace-pre-wrap break-all font-mono text-[11px] leading-[1.5] text-ink-muted dark:text-ink-mutedDark" title={r.title}>{(r.title ?? '').split('|').join('\n')}</div>
    </div>
  {/each}
{/if}
