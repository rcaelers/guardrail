<script lang="ts">
  import type { Note } from '$lib/adapters/types';
  import { fmtDate } from '$lib/utils/format';

  interface Props { notes: Note[]; onAdd: (body: string) => void; readOnly?: boolean; }
  let { notes, onAdd, readOnly = false }: Props = $props();
  let draft = $state('');

  function submit() {
    if (draft.trim()) {
      onAdd(draft.trim());
      draft = '';
    }
  }
</script>

<div>
  {#each notes as n}
    <div class="mb-2 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3.5 py-3">
      <div class="mb-1.5 flex gap-2.5 text-[11px] text-ink-muted dark:text-ink-mutedDark">
        <span class="font-medium text-ink dark:text-ink-dark">{n.author}</span>
        <span>·</span>
        <span>{fmtDate(n.at)}</span>
      </div>
      <div class="text-[13px] leading-[1.55] text-ink dark:text-ink-dark">{n.body}</div>
    </div>
  {/each}
  {#if !readOnly}
    <div class="mt-3">
      <textarea
        bind:value={draft}
        placeholder="Add a triage note…"
        class="block w-full resize-y rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark p-3 font-sans text-[13px] text-ink dark:text-ink-dark outline-none"
        style:min-height="80px"
      ></textarea>
      <button
        type="button"
        onclick={submit}
        class="mt-2 cursor-pointer rounded-md border-none bg-accent px-3.5 py-1.5 font-sans text-xs font-medium text-white"
      >Add note</button>
    </div>
  {:else}
    <div class="mt-3 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-2 text-[12px] text-ink-muted dark:text-ink-mutedDark">
      Read-only access — you cannot add notes on this product.
    </div>
  {/if}
</div>
