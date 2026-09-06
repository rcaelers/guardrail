<script lang="ts">
  import { invalidateAll, goto } from '$app/navigation';
  import { page } from '$app/stores';
  import type { PageData } from './$types';
  import type { Status } from '$lib/adapters/types';
  import DetailPanel from '$lib/components/detail/DetailPanel.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';

  let { data }: { data: PageData } = $props();

  const readOnly = $derived(data.role === 'readonly');
  const canMerge = $derived(data.role === 'maintainer');
  const backHref = $derived(`/p/${$page.params.product}/crashes`);
  let pendingDeleteNote = $state<string | null>(null);

  async function setStatus(s: Status, fixedInVersion?: string | null) {
    const body = new FormData();
    body.set('status', s);
    body.set('fixedInVersion', fixedInVersion ?? '');
    await fetch('?/setStatus', { method: 'POST', body });
    await invalidateAll();
  }
  async function addNote(noteBody: string) {
    const body = new FormData();
    body.set('body', noteBody);
    await fetch('?/addNote', { method: 'POST', body });
    await invalidateAll();
  }
  async function updateNote(noteId: string, noteBody: string) {
    const body = new FormData();
    body.set('noteId', noteId);
    body.set('body', noteBody);
    await fetch('?/updateNote', { method: 'POST', body });
    await invalidateAll();
  }
  async function deleteNote(noteId: string) {
    const body = new FormData();
    body.set('noteId', noteId);
    await fetch('?/deleteNote', { method: 'POST', body });
    await invalidateAll();
  }
  async function merge(mergedId: string) {
    const body = new FormData();
    body.set('mergedId', mergedId);
    await fetch('?/merge', { method: 'POST', body });
    await invalidateAll();
  }
</script>

{#if pendingDeleteNote}
  <ConfirmDialog
    message="Delete this note? The text cannot be recovered."
    confirmLabel="Delete note"
    onconfirm={() => {
      const id = pendingDeleteNote!;
      pendingDeleteNote = null;
      void deleteNote(id);
    }}
    oncancel={() => (pendingDeleteNote = null)}
  />
{/if}

<div class="mx-auto flex h-full min-h-0 w-full max-w-[980px] flex-col">
  <div class="shrink-0 px-5 py-3 text-[12px]">
    <a href={backHref} class="text-ink-muted dark:text-ink-mutedDark hover:text-accent">← Back to all crashes</a>
  </div>
  <div class="min-h-0 flex-1">
    <DetailPanel
      group={data.group}
      crash={data.crash}
      onStatusChange={setStatus}
      onMerge={merge}
      onAddNote={addNote}
      onUpdateNote={updateNote}
      onDeleteNote={(id) => (pendingDeleteNote = id)}
      {readOnly}
      {canMerge}
      onClose={() => goto(backHref)}
    />
  </div>
</div>
