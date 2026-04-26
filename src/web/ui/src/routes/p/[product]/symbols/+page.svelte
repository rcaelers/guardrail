<script lang="ts">
  import { enhance } from '$app/forms';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import type { PageData } from './$types';
  import Select from '$lib/components/Select.svelte';
  import { fmtDate } from '$lib/utils/format';

  let { data }: { data: PageData } = $props();

  const canUpload = $derived(data.role === 'readwrite' || data.role === 'maintainer');
  const canDelete = $derived(data.role === 'maintainer');

  async function updateParam(key: string, value: string) {
    const url = new URL($page.url);
    if (!value || value === 'all' || value === '') url.searchParams.delete(key);
    else url.searchParams.set(key, value);
    await goto(url, { keepFocus: true, noScroll: true, replaceState: true });
  }

  let showUpload = $state(false);
  let upName = $state('');
  let upVersion = $state('');
  let upArch = $state('x86_64');
  let upFormat = $state('PDB');

  function uploaderName(id: string) {
    return data.uploaders.find((u) => u.id === id)?.name ?? id;
  }
</script>

<div class="flex h-full min-h-0 flex-col">
  <!-- Toolbar -->
  <div class="flex shrink-0 flex-wrap items-center gap-3 border-b border-line dark:border-line-dark px-5 py-3">
    <input
      type="search"
      placeholder="Search name or debug ID…"
      value={data.filters.search ?? ''}
      onchange={(e) => updateParam('q', (e.currentTarget as HTMLInputElement).value)}
      class="w-[280px] rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-1.5 text-[13px] outline-none"
    />
    <Select
      label="Arch"
      value={String(data.filters.arch ?? 'all')}
      options={[['all', 'All'], ['x86_64', 'x86_64'], ['x86', 'x86'], ['arm64', 'arm64']]}
      onChange={(v) => updateParam('arch', v)}
    />
    <Select
      label="Format"
      value={String(data.filters.format ?? 'all')}
      options={[['all', 'All'], ['PDB', 'PDB'], ['dSYM', 'dSYM'], ['Breakpad', 'Breakpad'], ['ELF', 'ELF']]}
      onChange={(v) => updateParam('format', v)}
    />
    <Select
      label="Sort"
      value={String(data.filters.sort ?? 'recent')}
      options={[['recent', 'Recently uploaded'], ['name', 'Name'], ['size', 'Size']]}
      onChange={(v) => updateParam('sort', v)}
    />
    <span class="flex-1"></span>
    <span class="text-xs text-ink-muted dark:text-ink-mutedDark">
      {data.symbols.length} symbol{data.symbols.length === 1 ? '' : 's'}
    </span>
    {#if canUpload}
      <button
        type="button"
        onclick={() => (showUpload = !showUpload)}
        class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white"
      >Upload</button>
    {/if}
  </div>

  <!-- Upload panel (inline) -->
  {#if showUpload && canUpload}
    <form
      method="POST"
      action="?/upload"
      use:enhance={() => async ({ update }) => {
        await update();
        showUpload = false;
        upName = ''; upVersion = '';
      }}
      class="flex shrink-0 flex-wrap items-end gap-3 border-b border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-5 py-3"
    >
      <label class="flex flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Name</span>
        <input name="name" required bind:value={upName} placeholder="mylib.dll" class="w-[220px] rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px] outline-none" />
      </label>
      <label class="flex flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Version</span>
        <input name="version" bind:value={upVersion} placeholder="1.0.0" class="w-[120px] rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px] outline-none" />
      </label>
      <label class="flex flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Arch</span>
        <select name="arch" bind:value={upArch} class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px]">
          {#each ['x86_64', 'x86', 'arm64'] as a}<option value={a}>{a}</option>{/each}
        </select>
      </label>
      <label class="flex flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Format</span>
        <select name="format" bind:value={upFormat} class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px]">
          {#each ['PDB', 'dSYM', 'Breakpad', 'ELF'] as f}<option value={f}>{f}</option>{/each}
        </select>
      </label>
      <div class="ml-auto flex gap-2">
        <button type="button" onclick={() => (showUpload = false)} class="rounded-md border border-line dark:border-line-dark bg-transparent px-3 py-1.5 text-[13px]">Cancel</button>
        <button type="submit" class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white">Upload symbol</button>
      </div>
    </form>
  {/if}

  <!-- Header -->
  <div
    class="grid shrink-0 items-center gap-4 border-b border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-5 py-2 text-[10.5px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark"
    style:grid-template-columns="1.3fr 80px 90px 80px 1.2fr 180px 1fr 100px"
  >
    <span>Module</span>
    <span>Version</span>
    <span>Arch</span>
    <span>Format</span>
    <span>Debug ID</span>
    <span>Uploaded</span>
    <span>Refs</span>
    <span></span>
  </div>

  <!-- Rows -->
  <div class="scroll-clean min-h-0 flex-1 overflow-auto">
    {#each data.symbols as s (s.id)}
      <div
        class="grid items-center gap-4 border-b border-line dark:border-line-dark px-5 py-2.5 text-[13px] hover:bg-surface-panel dark:hover:bg-surface-panelDark"
        style:grid-template-columns="1.3fr 80px 90px 80px 1.2fr 180px 1fr 100px"
      >
        <div class="min-w-0 truncate">
          <div class="truncate font-mono text-[12.5px] text-ink dark:text-ink-dark">{s.name}</div>
          <div class="truncate text-[10.5px] text-ink-muted dark:text-ink-mutedDark">{s.size}</div>
        </div>
        <div class="truncate">{s.version}</div>
        <div class="truncate font-mono text-[12px]">{s.arch}</div>
        <div class="truncate">{s.format}</div>
        <div class="truncate font-mono text-[11px] text-ink-muted dark:text-ink-mutedDark">{s.debugId}</div>
        <div class="truncate text-[12px] text-ink-muted dark:text-ink-mutedDark">
          {fmtDate(s.uploadedAt)} · {uploaderName(s.uploadedBy)}
        </div>
        <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">{s.referencedBy} crash group{s.referencedBy === 1 ? '' : 's'}</div>
        <div class="flex justify-end">
          {#if canDelete}
            <form method="POST" action="?/delete" use:enhance>
              <input type="hidden" name="id" value={s.id} />
              <button
                type="submit"
                class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink-muted dark:text-ink-mutedDark hover:text-red-600"
                onclick={(e) => { if (!confirm(`Delete ${s.name} (${s.version})?`)) e.preventDefault(); }}
              >Delete</button>
            </form>
          {/if}
        </div>
      </div>
    {:else}
      <div class="px-5 py-10 text-center text-[13px] text-ink-muted dark:text-ink-mutedDark">
        No symbols match these filters.
      </div>
    {/each}
  </div>
</div>
