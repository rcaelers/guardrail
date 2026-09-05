<script lang="ts">
  import type { Note } from '$lib/adapters/types';
  import { fmtDate } from '$lib/utils/format';

  interface Props {
    notes: Note[];
    /** Notes belong to the group, not the crash shown in the pane. */
    groupId: string;
    onAdd: (body: string) => void;
    onUpdate: (noteId: string, body: string) => void;
    onDelete: (noteId: string) => void;
    readOnly?: boolean;
  }
  let { notes, groupId, onAdd, onUpdate, onDelete, readOnly = false }: Props = $props();

  let draft = $state('');
  let editingId = $state<string | null>(null);
  let editDraft = $state('');

  function submit() {
    if (draft.trim()) {
      onAdd(draft.trim());
      draft = '';
    }
  }

  function startEdit(note: Note) {
    editingId = note.id;
    editDraft = note.body;
  }

  function saveEdit() {
    const body = editDraft.trim();
    if (editingId && body) {
      onUpdate(editingId, body);
    }
    editingId = null;
  }
</script>

<div>
  <div class="mb-3 text-[11px] text-ink-muted dark:text-ink-mutedDark">
    Notes apply to the whole crash group
    <span class="font-mono text-ink dark:text-ink-dark">{groupId}</span>, not to the
    individual crash shown here.
  </div>

  {#each notes as n (n.id)}
    <div class="mb-2 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3.5 py-3">
      <div class="mb-1.5 flex items-center gap-2.5 text-[11px] text-ink-muted dark:text-ink-mutedDark">
        <span class="font-medium text-ink dark:text-ink-dark">{n.author}</span>
        <span>·</span>
        <span>{fmtDate(n.at)}</span>
        <span class="flex-1"></span>
        {#if !readOnly && editingId !== n.id}
          <button
            type="button"
            onclick={() => startEdit(n)}
            class="cursor-pointer rounded border border-line dark:border-line-dark bg-transparent px-2 py-0.5 text-[11px] text-ink-muted hover:text-ink dark:text-ink-mutedDark dark:hover:text-ink-dark"
          >Edit</button>
          <button
            type="button"
            onclick={() => onDelete(n.id)}
            class="cursor-pointer rounded border border-line dark:border-line-dark bg-transparent px-2 py-0.5 text-[11px] text-ink-muted hover:text-red-600 dark:text-ink-mutedDark dark:hover:text-red-400"
          >Delete</button>
        {/if}
      </div>

      {#if editingId === n.id}
        <textarea
          bind:value={editDraft}
          class="block w-full resize-y rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark p-2.5 font-sans text-[13px] text-ink dark:text-ink-dark outline-none"
          style:min-height="80px"
        ></textarea>
        <div class="mt-2 flex gap-2">
          <button
            type="button"
            onclick={saveEdit}
            disabled={!editDraft.trim()}
            class="cursor-pointer rounded-md border-none bg-accent px-3 py-1 text-xs font-medium text-white disabled:opacity-50"
          >Save</button>
          <button
            type="button"
            onclick={() => (editingId = null)}
            class="cursor-pointer rounded-md border border-line dark:border-line-dark bg-transparent px-3 py-1 text-xs text-ink dark:text-ink-dark"
          >Cancel</button>
        </div>
      {:else}
        <div class="whitespace-pre-wrap text-[13px] leading-[1.55] text-ink dark:text-ink-dark">{n.body}</div>
      {/if}
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
        disabled={!draft.trim()}
        class="mt-2 cursor-pointer rounded-md border-none bg-accent px-3.5 py-1.5 font-sans text-xs font-medium text-white disabled:opacity-50"
      >Add note</button>
    </div>
  {:else}
    <div class="mt-3 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-2 text-[12px] text-ink-muted dark:text-ink-mutedDark">
      Read-only access — you cannot add notes on this product.
    </div>
  {/if}
</div>
