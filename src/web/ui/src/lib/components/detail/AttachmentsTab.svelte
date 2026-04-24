<script lang="ts">
  import type { CrashAttachment } from '$lib/adapters/types';
  import { fmtBytes, fmtDate } from '$lib/utils/format';

  let { attachments, productId }: { attachments: CrashAttachment[]; productId: string } = $props();

  function hrefFor(id: string): string {
    return `/p/${productId}/crashes/attachments/${encodeURIComponent(id)}`;
  }
</script>

<div>
  {#if attachments.length === 0}
    <div class="rounded border border-dashed border-line px-3 py-3 text-[12px] text-ink-muted dark:border-line-dark dark:text-ink-mutedDark">
      No attachments for this crash.
    </div>
  {:else}
    {#each attachments as attachment}
      <div class="mb-2 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3.5 py-3">
        <div class="mb-2 flex items-start justify-between gap-3">
          <div class="min-w-0">
            <div class="truncate text-[13px] font-medium text-ink dark:text-ink-dark">{attachment.name}</div>
            <div class="truncate font-mono text-[11px] text-ink-muted dark:text-ink-mutedDark">{attachment.filename}</div>
          </div>
          <a
            href={hrefFor(attachment.id)}
            download={attachment.filename}
            class="shrink-0 rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink dark:text-ink-dark"
          >Download</a>
        </div>
        <div class="grid gap-x-4 gap-y-1 text-[11px] text-ink-muted dark:text-ink-mutedDark" style:grid-template-columns="88px 1fr">
          <div>Size</div>
          <div class="font-mono">{fmtBytes(attachment.size)}</div>
          <div>Type</div>
          <div class="font-mono">{attachment.mimeType || 'application/octet-stream'}</div>
          <div>Created</div>
          <div class="font-mono">{fmtDate(attachment.createdAt)}</div>
        </div>
      </div>
    {/each}
  {/if}
</div>
