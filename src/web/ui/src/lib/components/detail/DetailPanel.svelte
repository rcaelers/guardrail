<script lang="ts">
  import type { Crash, CrashGroup, Status } from '$lib/adapters/types';
  import StatusPill from '../StatusPill.svelte';
  import SignalChip from '../SignalChip.svelte';
  import StackTab from './StackTab.svelte';
  import ThreadsTab from './ThreadsTab.svelte';
  import ModulesTab from './ModulesTab.svelte';
  import HandlesTab from './HandlesTab.svelte';
  import SystemTab from './SystemTab.svelte';
  import AttachmentsTab from './AttachmentsTab.svelte';
  import AnnotationsTab from './AnnotationsTab.svelte';
  import RelatedTab from './RelatedTab.svelte';
  import NotesTab from './NotesTab.svelte';
  import { fmtDate, fmtInt } from '$lib/utils/format';
  import {
    crashAddress,
    crashOs,
    crashTitle,
    exceptionType,
    exceptionTypeShort,
    topFrameFile,
    topFrameLabel,
    topFrameLine
  } from '$lib/utils/crash-report';

  interface Props {
    group: CrashGroup;
    crash: Crash;
    onStatusChange: (s: Status) => void;
    onMerge: (mergedId: string) => void;
    onAddNote: (body: string) => void;
    onClose?: () => void;
    /** When true, hide mutating affordances (status chips, merge, add-note). */
    readOnly?: boolean;
    /** When true (maintainer), expose destructive actions like merge. */
    canMerge?: boolean;
  }
  let {
    group,
    crash,
    onStatusChange,
    onMerge,
    onAddNote,
    onClose,
    readOnly = false,
    canMerge = true
  }: Props = $props();

  type TabKey =
    | 'stack'
    | 'threads'
    | 'modules'
    | 'handles'
    | 'system'
    | 'annotations'
    | 'attachments'
    | 'related'
    | 'notes';
  let tab = $state<TabKey>('stack');

  const TABS: [TabKey, string][] = [
    ['stack', 'Stack'],
    ['threads', 'Threads'],
    ['modules', 'Modules'],
    ['handles', 'Handles'],
    ['system', 'System'],
    ['annotations', 'Annotations'],
    ['attachments', 'Attachments'],
    ['related', 'Related'],
    ['notes', 'Notes']
  ];
</script>

<div class="flex h-full min-w-0 flex-col border-l border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark">
  <!-- Header -->
  <div class="shrink-0 border-b border-line dark:border-line-dark px-7 pb-4 pt-5">
    <div class="mb-3 flex items-center gap-3">
      <span class="font-mono text-[11px] text-ink-muted dark:text-ink-mutedDark">{crash.id}</span>
      <span class="font-mono text-[10.5px] text-ink-muted dark:text-ink-mutedDark">in {group.id}</span>
      <SignalChip signal={crash.signal || exceptionTypeShort(crash)} />
      <StatusPill status={group.status} />
      <span class="flex-1"></span>
      {#if onClose}
        <button
          type="button"
          onclick={onClose}
          class="cursor-pointer rounded-md border border-line dark:border-line-dark bg-transparent px-2 py-0.5 text-[18px] leading-none text-ink-muted dark:text-ink-mutedDark"
          title="Close detail pane"
          aria-label="Close detail pane"
        >×</button>
      {/if}
    </div>
    <h2 class="mb-2 text-[19px] font-semibold leading-[1.3] text-ink dark:text-ink-dark">{crashTitle(crash)}</h2>
    <div class="font-mono text-xs text-ink-muted dark:text-ink-mutedDark">
      {topFrameLabel(crash)}  ·  {topFrameFile(crash)}:{topFrameLine(crash)}
    </div>
    <div class="mt-3.5 grid gap-4 text-[11px] text-ink-muted dark:text-ink-mutedDark" style:grid-template-columns="repeat(4, 1fr)">
      {#each [
        ['Occurred', fmtDate(crash.at)],
        ['Version', crash.version],
        ['OS', crashOs(crash)],
        ['Exception', exceptionType(crash)],
        ['Address', crashAddress(crash)],
        ['Group events', `${fmtInt(group.count)} (last ${fmtDate(group.lastSeen)})`]
      ] as [label, value]}
        <div>
          <div class="mb-0.5 uppercase tracking-wider text-[10px]">{label}</div>
          <div class="text-[13px] text-ink dark:text-ink-dark">{value}</div>
        </div>
      {/each}
    </div>

    <!-- Actions -->
    <div class="mt-4 flex flex-wrap gap-1.5">
      {#each [['new', 'Mark new'], ['triaged', 'Triage'], ['resolved', 'Resolve']] as [s, label]}
        {@const active = group.status === s}
        <button
          type="button"
          disabled={readOnly}
          onclick={() => onStatusChange(s as Status)}
          class="rounded-md border px-2.5 py-1 font-sans text-[11.5px]"
          class:cursor-pointer={!readOnly}
          class:cursor-not-allowed={readOnly}
          class:opacity-50={readOnly}
          class:border-accent={active}
          class:bg-accent={active}
          class:text-white={active}
          class:border-line={!active}
          class:dark:border-line-dark={!active}
          class:bg-transparent={!active}
          class:text-ink={!active}
          class:dark:text-ink-dark={!active}
          title={readOnly ? 'Read-only access' : ''}
        >{label}</button>
      {/each}
      <button type="button" disabled={readOnly} class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink dark:text-ink-dark" class:cursor-pointer={!readOnly} class:cursor-not-allowed={readOnly} class:opacity-50={readOnly}>Assign…</button>
      <button type="button" class="cursor-pointer rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink dark:text-ink-dark">Open in issue tracker</button>
      {#if readOnly}
        <span class="ml-1 self-center text-[11px] text-ink-muted dark:text-ink-mutedDark">Read-only access</span>
      {/if}
    </div>

    {#if crash.userText?.body}
      <div class="mt-4 rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3.5 py-3">
        <div class="mb-1.5 text-[10px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">User Info</div>
        <div class="mb-2 text-[11px] text-ink-muted dark:text-ink-mutedDark">
          Submitted {fmtDate(crash.userText.createdAt)}
        </div>
        <pre class="max-h-40 overflow-auto whitespace-pre-wrap break-words font-sans text-[13px] leading-[1.55] text-ink dark:text-ink-dark">{crash.userText.body}</pre>
      </div>
    {/if}
  </div>

  <!-- Tabs -->
  <div class="flex shrink-0 gap-0.5 overflow-x-auto border-b border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-7">
    {#each TABS as [k, label]}
      {@const active = tab === k}
      <button
        type="button"
        onclick={() => (tab = k)}
        class="cursor-pointer whitespace-nowrap border-b-2 bg-transparent px-2 py-2.5 font-sans text-[12.5px]"
        class:border-accent={active}
        class:text-ink={active}
        class:dark:text-ink-dark={active}
        class:border-transparent={!active}
        class:text-ink-muted={!active}
        class:dark:text-ink-mutedDark={!active}
      >{label}</button>
    {/each}
  </div>

  <!-- Body -->
  <div class="scroll-clean flex-1 overflow-auto px-7 py-5">
    {#if tab === 'stack'}<StackTab {crash} />{/if}
    {#if tab === 'threads'}<ThreadsTab {crash} />{/if}
    {#if tab === 'modules'}<ModulesTab {crash} />{/if}
    {#if tab === 'handles'}<HandlesTab {crash} />{/if}
    {#if tab === 'system'}<SystemTab {crash} />{/if}
    {#if tab === 'annotations'}<AnnotationsTab annotations={crash.annotations} />{/if}
    {#if tab === 'attachments'}<AttachmentsTab attachments={crash.attachments ?? []} productId={crash.productId} />{/if}
    {#if tab === 'related'}<RelatedTab related={group.related} {onMerge} canMerge={canMerge && !readOnly} />{/if}
    {#if tab === 'notes'}<NotesTab notes={group.notes} onAdd={onAddNote} {readOnly} />{/if}
  </div>
</div>
