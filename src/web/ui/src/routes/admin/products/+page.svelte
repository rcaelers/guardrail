<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData, ActionData } from './$types';
  import type { User } from '$lib/adapters/types';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  let showCreate = $state(false);
  let editingProductId = $state<string | null>(null);
  let pendingConfirm = $state<{ message: string; confirmLabel: string; form: HTMLFormElement } | null>(null);
  let newName = $state('');
  let newSlug = $state('');
  let newDesc = $state('');

  // Staged member state for the currently-edited product (nothing persists until Save)
  let editMembers = $state<Array<{ userId: string; role: string; user: User }>>([]);
  let editAddUserId = $state('');
  let editAddMemberRole = $state('readonly');

  const editUnassignedUsers = $derived(
    data.users.filter((u) => !editMembers.some((m) => m.userId === u.id))
  );

  function openEditProduct(p: typeof data.products[0]) {
    editingProductId = p.id;
    editMembers = p.members.map((m) => ({ userId: m.userId, role: m.role, user: m.user }));
    editAddUserId = '';
    editAddMemberRole = 'readonly';
  }

  function addEditMember() {
    if (!editAddUserId) return;
    const user = data.users.find((u) => u.id === editAddUserId);
    if (!user) return;
    editMembers = [...editMembers, { userId: editAddUserId, role: editAddMemberRole, user }];
    editAddUserId = '';
    editAddMemberRole = 'readonly';
  }
</script>

{#if pendingConfirm}
  <ConfirmDialog
    message={pendingConfirm.message}
    confirmLabel={pendingConfirm.confirmLabel}
    onconfirm={() => { pendingConfirm!.form.requestSubmit(); pendingConfirm = null; }}
    oncancel={() => (pendingConfirm = null)}
  />
{/if}

<div class="mx-auto max-w-[1100px]">
  <div class="mb-6 flex items-end justify-between">
    <div>
      <h1 class="mb-1 text-[20px] font-semibold tracking-[-0.01em]">Products</h1>
      <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
        {data.products.length} product{data.products.length === 1 ? '' : 's'}.
        Edit product metadata and manage member access from the edit panel.
      </p>
    </div>
    <button
      type="button"
      onclick={() => (showCreate = !showCreate)}
      class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white"
    >New product</button>
  </div>

  {#if form?.error}
    <p class="mb-4 text-[12px] text-red-600 dark:text-red-400">{form.error}</p>
  {/if}

  {#if showCreate}
    <form
      method="POST"
      action="?/create"
      use:enhance={() => async ({ update, result }) => {
        await update();
        if (result.type === 'success') {
          newName = '';
          newSlug = '';
          newDesc = '';
          showCreate = false;
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
  {/if}

  <div class="overflow-hidden rounded-md border border-line dark:border-line-dark">
    <div
      class="grid items-center gap-4 bg-surface-panel dark:bg-surface-panelDark px-4 py-2 text-[10.5px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark"
      style:grid-template-columns="1.3fr 1fr 1.6fr 100px 160px"
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
        style:grid-template-columns="1.3fr 1fr 1.6fr 100px 160px"
      >
        <div class="flex min-w-0 items-center gap-2 truncate">
          <span class="inline-block h-[12px] w-[12px] shrink-0 rounded-[3px]" style:background={p.color}></span>
          <a href={`/p/${p.id}/crashes`} class="truncate font-medium hover:text-accent">{p.name}</a>
        </div>
        <div class="truncate font-mono text-[12px] text-ink-muted dark:text-ink-mutedDark">{p.slug}</div>
        <div class="truncate text-[12.5px] text-ink-muted dark:text-ink-mutedDark">{p.description}</div>
        <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">{p.members.length}</div>
        <div class="flex justify-end gap-2">
          <button
            type="button"
            class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink dark:text-ink-dark"
            onclick={() => {
              if (editingProductId === p.id) {
                editingProductId = null;
              } else {
                openEditProduct(p);
              }
            }}
          >{editingProductId === p.id ? 'Cancel' : 'Edit'}</button>
          <form method="POST" action="?/delete" use:enhance>
            <input type="hidden" name="id" value={p.id} />
            <button
              type="button"
              class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink-muted dark:text-ink-mutedDark hover:text-red-600"
              onclick={(e) => { pendingConfirm = { message: `Delete ${p.name}? This also deletes all crashes, symbols and memberships.`, confirmLabel: 'Delete', form: (e.currentTarget as HTMLElement).closest('form')! }; }}
            >Delete</button>
          </form>
        </div>
      </div>

      {#if editingProductId === p.id}
        <div class="border-t border-line bg-surface-panel/55 px-4 py-4 dark:border-line-dark dark:bg-surface-panelDark/55 space-y-4">
          <form
            method="POST"
            action="?/update"
            use:enhance={() => async ({ update, result }) => {
              await update();
              if (result.type === 'success') editingProductId = null;
            }}
          >
            <input type="hidden" name="id" value={p.id} />
            <input type="hidden" name="members" value={JSON.stringify(editMembers.map((m) => ({ userId: m.userId, role: m.role })))} />

            <div class="grid gap-3 rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-4 py-3 lg:grid-cols-[1fr,1fr]">
              <div class="col-span-2 flex items-center justify-between">
                <h2 class="text-[13px] font-medium">Edit product</h2>
                <span class="text-[11px] text-ink-muted dark:text-ink-mutedDark">Route id: {p.id}</span>
              </div>
              <label class="flex flex-col">
                <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Name</span>
                <input name="name" value={p.name} required class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-1.5 text-[13px]" />
              </label>
              <label class="flex flex-col">
                <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Slug</span>
                <input name="slug" value={p.slug} required class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-1.5 font-mono text-[12.5px]" />
              </label>
              <label class="col-span-2 flex flex-col">
                <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Description</span>
                <input name="description" value={p.description} class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-1.5 text-[13px]" />
              </label>
              <label class="flex flex-col">
                <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Color</span>
                <div class="flex items-center gap-2 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5">
                  <input name="color" type="color" value={p.color} class="h-7 w-10 rounded border-0 bg-transparent p-0" />
                  <span class="font-mono text-[12px] text-ink-muted dark:text-ink-mutedDark">{p.color}</span>
                </div>
              </label>
            </div>

            <div class="mt-4 rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-4 py-3">
              <div class="mb-3 flex items-center justify-between">
                <h2 class="text-[13px] font-medium">Members</h2>
                <span class="text-[11px] text-ink-muted dark:text-ink-mutedDark">{editMembers.length} assigned</span>
              </div>

              <div class="space-y-2">
                {#if editMembers.length === 0}
                  <p class="text-[12px] text-ink-muted dark:text-ink-mutedDark">No members yet.</p>
                {:else}
                  {#each editMembers as member (member.userId)}
                    <div class="grid items-center gap-3 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-2 lg:grid-cols-[1.4fr,160px,100px]">
                      <div class="flex min-w-0 items-center gap-2">
                        <span class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-accent-soft dark:bg-accent-softDark text-[10px] font-semibold text-accent">{member.user.avatar}</span>
                        <div class="min-w-0">
                          <div class="truncate text-[12.5px] font-medium">{member.user.name}</div>
                          <div class="truncate text-[11px] text-ink-muted dark:text-ink-mutedDark">{member.user.email}</div>
                        </div>
                      </div>
                      <select
                        value={member.role}
                        onchange={(e) => {
                          const newRole = e.currentTarget.value;
                          editMembers = editMembers.map((m) =>
                            m.userId === member.userId ? { ...m, role: newRole } : m
                          );
                        }}
                        class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1 text-[12px]"
                      >
                        <option value="readonly">Read-only</option>
                        <option value="readwrite">Read · write</option>
                        <option value="maintainer">Maintainer</option>
                      </select>
                      <div class="flex justify-end">
                        <button
                          type="button"
                          onclick={() => { editMembers = editMembers.filter((m) => m.userId !== member.userId); }}
                          class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink-muted dark:text-ink-mutedDark hover:text-red-600"
                        >Remove</button>
                      </div>
                    </div>
                  {/each}
                {/if}
              </div>

              {#if editUnassignedUsers.length > 0}
                <div class="mt-4 grid gap-3 rounded-md border border-dashed border-line dark:border-line-dark px-3 py-3 lg:grid-cols-[1.4fr,160px,120px]">
                  <label class="flex flex-col">
                    <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Add user</span>
                    <select bind:value={editAddUserId} class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]">
                      <option value="" disabled>Pick a user…</option>
                      {#each editUnassignedUsers as u}
                        <option value={u.id}>{u.name} ({u.email})</option>
                      {/each}
                    </select>
                  </label>
                  <label class="flex flex-col">
                    <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Role</span>
                    <select bind:value={editAddMemberRole} class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]">
                      <option value="readonly">Read-only</option>
                      <option value="readwrite">Read · write</option>
                      <option value="maintainer">Maintainer</option>
                    </select>
                  </label>
                  <div class="flex items-end justify-end">
                    <button
                      type="button"
                      onclick={addEditMember}
                      class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white"
                    >Add</button>
                  </div>
                </div>
              {/if}
            </div>

            <div class="mt-4 flex justify-end gap-2">
              <button type="button" onclick={() => (editingProductId = null)} class="rounded-md border border-line dark:border-line-dark bg-transparent px-3 py-1.5 text-[13px]">Cancel</button>
              <button type="submit" class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white">Save product</button>
            </div>
          </form>
        </div>
      {/if}
    {/each}
  </div>
</div>
