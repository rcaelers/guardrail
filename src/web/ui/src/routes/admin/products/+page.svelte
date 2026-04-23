<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData, ActionData } from './$types';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  let showCreate = $state(false);
  let newName = $state('');
  let newSlug = $state('');
  let newDesc = $state('');
</script>

<div class="mx-auto max-w-[960px]">
  <div class="mb-6 flex items-end justify-between">
    <div>
      <h1 class="mb-1 text-[20px] font-semibold tracking-[-0.01em]">Products</h1>
      <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
        {data.products.length} product{data.products.length === 1 ? '' : 's'}.
        Creating a product starts it empty — grant members from the product's Settings.
      </p>
    </div>
    <button
      type="button"
      onclick={() => (showCreate = !showCreate)}
      class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white"
    >New product</button>
  </div>

  {#if showCreate}
    <form
      method="POST"
      action="?/create"
      use:enhance={() => async ({ update, result }) => {
        await update();
        if (result.type === 'success') {
          newName = ''; newSlug = ''; newDesc = ''; showCreate = false;
        }
      }}
      class="mb-5 grid gap-3 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-4 py-3"
      style:grid-template-columns="1fr 1fr"
    >
      <label class="flex flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Name</span>
        <input name="name" required bind:value={newName} class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px]" />
      </label>
      <label class="flex flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Slug <span class="normal-case opacity-60">(optional)</span></span>
        <input name="slug" bind:value={newSlug} placeholder="auto-generated" class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 font-mono text-[12.5px]" />
      </label>
      <label class="col-span-2 flex flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Description</span>
        <input name="description" bind:value={newDesc} class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px]" />
      </label>
      <div class="col-span-2 flex justify-end gap-2">
        <button type="button" onclick={() => (showCreate = false)} class="rounded-md border border-line dark:border-line-dark bg-transparent px-3 py-1.5 text-[13px]">Cancel</button>
        <button type="submit" class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white">Create product</button>
      </div>
    </form>
    {#if form?.error}
      <p class="mb-4 text-[12px] text-red-600 dark:text-red-400">{form.error}</p>
    {/if}
  {/if}

  <div class="overflow-hidden rounded-md border border-line dark:border-line-dark">
    <div
      class="grid items-center gap-4 bg-surface-panel dark:bg-surface-panelDark px-4 py-2 text-[10.5px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark"
      style:grid-template-columns="1.4fr 1fr 1.6fr 100px 140px"
    >
      <span>Product</span>
      <span>Slug</span>
      <span>Description</span>
      <span>Members</span>
      <span></span>
    </div>
    {#each data.products as p (p.id)}
      <div
        class="grid items-center gap-4 border-t border-line dark:border-line-dark px-4 py-2.5 text-[13px]"
        style:grid-template-columns="1.4fr 1fr 1.6fr 100px 140px"
      >
        <div class="flex min-w-0 items-center gap-2 truncate">
          <span class="inline-block h-[12px] w-[12px] shrink-0 rounded-[3px]" style:background={p.color}></span>
          <a href={`/p/${p.id}/crashes`} class="truncate font-medium hover:text-accent">{p.name}</a>
        </div>
        <div class="truncate font-mono text-[12px] text-ink-muted dark:text-ink-mutedDark">{p.slug}</div>
        <div class="truncate text-[12.5px] text-ink-muted dark:text-ink-mutedDark">{p.description}</div>
        <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">{p.memberCount}</div>
        <div class="flex justify-end">
          <form method="POST" action="?/delete" use:enhance>
            <input type="hidden" name="id" value={p.id} />
            <button
              type="submit"
              class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink-muted dark:text-ink-mutedDark hover:text-red-600"
              onclick={(e) => { if (!confirm(`Delete ${p.name}? This also deletes all crashes, symbols and memberships.`)) e.preventDefault(); }}
            >Delete</button>
          </form>
        </div>
      </div>
    {/each}
  </div>
</div>
