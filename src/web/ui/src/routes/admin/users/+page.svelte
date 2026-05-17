<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData, ActionData } from './$types';
  import type { Product } from '$lib/adapters/types';
  import { fmtDate } from '$lib/utils/format';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  // --- New user form ---
  let newEmail = $state('');
  let newName = $state('');
  let newIsAdmin = $state(false);
  let newPermissions = $state<Array<{ productId: string; role: string }>>([]);
  let addProductId = $state('');
  let addRole = $state('readonly');
  let showCreate = $state(false);

  // --- Edit user staged state (nothing persists until Save) ---
  let editingUserId = $state<string | null>(null);
  let editName = $state('');
  let editEmail = $state('');
  let editIsAdmin = $state(false);
  let editPermissions = $state<Array<{ productId: string; role: string; product: Product }>>([]);
  let editAddProductId = $state('');
  let editAddRole = $state('readonly');

  let pendingConfirm = $state<{ message: string; confirmLabel: string; form: HTMLFormElement } | null>(null);

  const availableForNew = $derived(
    data.products.filter((p) => !newPermissions.some((np) => np.productId === p.id))
  );

  const editAvailableProducts = $derived(
    data.products.filter((p) => !editPermissions.some((ep) => ep.productId === p.id))
  );

  function addNewPermission() {
    if (!addProductId) return;
    newPermissions = [...newPermissions, { productId: addProductId, role: addRole }];
    addProductId = '';
    addRole = 'readonly';
  }

  function openEditUser(u: typeof data.users[0]) {
    editingUserId = u.id;
    editName = u.name;
    editEmail = u.email;
    editIsAdmin = u.isAdmin;
    editPermissions = u.permissions.map((p) => ({
      productId: p.productId,
      role: p.role,
      product: p.product,
    }));
    editAddProductId = '';
    editAddRole = 'readonly';
  }

  function addEditPermission() {
    if (!editAddProductId) return;
    const product = data.products.find((p) => p.id === editAddProductId);
    if (!product) return;
    editPermissions = [...editPermissions, { productId: editAddProductId, role: editAddRole, product }];
    editAddProductId = '';
    editAddRole = 'readonly';
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
      <h1 class="mb-1 text-[20px] font-semibold tracking-[-0.01em]">Users</h1>
      <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
        {data.users.length} accounts. Manage account details, admin access, and per-product permissions.
      </p>
    </div>
    <button
      type="button"
      onclick={() => (showCreate = !showCreate)}
      class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white"
    >New user</button>
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
          newEmail = '';
          newName = '';
          newIsAdmin = false;
          newPermissions = [];
          showCreate = false;
        }
      }}
      class="mb-5 rounded-md border border-line dark:border-line-dark bg-surface-panel/55 dark:bg-surface-panelDark/55 px-4 py-4"
    >
      <input type="hidden" name="isAdmin" value={newIsAdmin} />
      <input type="hidden" name="permissions" value={JSON.stringify(newPermissions)} />
      <div class="grid gap-4 lg:grid-cols-[1.1fr,1fr]">
        <div class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-4 py-3">
          <h2 class="mb-3 text-[13px] font-medium">Account</h2>
          <div class="grid gap-3">
            <label class="flex flex-col">
              <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Name</span>
              <input name="name" bind:value={newName} class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-1.5 text-[13px]" />
            </label>
            <label class="flex flex-col">
              <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Email</span>
              <input name="email" type="email" required bind:value={newEmail} class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-1.5 text-[13px]" />
            </label>
          </div>
        </div>

        <div class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-4 py-3">
          <div class="mb-3 flex items-center justify-between">
            <h2 class="text-[13px] font-medium">Global access</h2>
            <span class="text-[11px] text-ink-muted dark:text-ink-mutedDark">Admin applies across all products</span>
          </div>
          <label class="flex flex-col">
            <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Admin access</span>
            <select
              bind:value={newIsAdmin}
              class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]"
            >
              <option value={false}>Member</option>
              <option value={true}>Administrator</option>
            </select>
          </label>
        </div>
      </div>

      <div class="mt-4 rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-4 py-3">
        <div class="mb-3 flex items-center justify-between">
          <h2 class="text-[13px] font-medium">Product permissions</h2>
          <span class="text-[11px] text-ink-muted dark:text-ink-mutedDark">{newPermissions.length} assigned</span>
        </div>

        <div class="space-y-2">
          {#if newPermissions.length === 0}
            <p class="text-[12px] text-ink-muted dark:text-ink-mutedDark">No product access yet.</p>
          {:else}
            {#each newPermissions as perm (perm.productId)}
              {@const product = data.products.find((p) => p.id === perm.productId)}
              <div class="grid items-center gap-3 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-2 lg:grid-cols-[1.2fr,160px,100px]">
                <div class="flex min-w-0 items-center gap-2">
                  <span class="inline-block h-[10px] w-[10px] shrink-0 rounded-[3px]" style:background={product?.color}></span>
                  <span class="truncate text-[12.5px] font-medium">{product?.name}</span>
                </div>
                <select
                  value={perm.role}
                  onchange={(e) => {
                    newPermissions = newPermissions.map((np) =>
                      np.productId === perm.productId ? { ...np, role: e.currentTarget.value } : np
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
                    onclick={() => { newPermissions = newPermissions.filter((np) => np.productId !== perm.productId); }}
                    class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink-muted dark:text-ink-mutedDark hover:text-red-600"
                  >Remove</button>
                </div>
              </div>
            {/each}
          {/if}
        </div>

        {#if availableForNew.length > 0}
          <div class="mt-4 grid gap-3 rounded-md border border-dashed border-line dark:border-line-dark px-3 py-3 lg:grid-cols-[1.4fr,160px,120px]">
            <label class="flex flex-col">
              <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Add product</span>
              <select bind:value={addProductId} class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]">
                <option value="" disabled>Pick a product…</option>
                {#each availableForNew as product}
                  <option value={product.id}>{product.name}</option>
                {/each}
              </select>
            </label>
            <label class="flex flex-col">
              <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Role</span>
              <select bind:value={addRole} class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]">
                <option value="readonly">Read-only</option>
                <option value="readwrite">Read · write</option>
                <option value="maintainer">Maintainer</option>
              </select>
            </label>
            <div class="flex items-end justify-end">
              <button
                type="button"
                onclick={addNewPermission}
                class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white"
              >Add</button>
            </div>
          </div>
        {/if}
      </div>

      <div class="mt-4 flex justify-end gap-2">
        <button type="button" onclick={() => (showCreate = false)} class="rounded-md border border-line dark:border-line-dark bg-transparent px-3 py-1.5 text-[13px]">Cancel</button>
        <button type="submit" class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white">Create user</button>
      </div>
    </form>
  {/if}

  <div class="overflow-hidden rounded-md border border-line dark:border-line-dark">
    <div
      class="grid items-center gap-4 bg-surface-panel dark:bg-surface-panelDark px-4 py-2 text-[10.5px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark"
      style:grid-template-columns="1.5fr 1.7fr 1.3fr 140px 240px"
    >
      <span>User</span>
      <span>Email</span>
      <span>Permissions</span>
      <span>Joined</span>
      <span></span>
    </div>
    {#each data.users as u (u.id)}
      <div
        class="grid items-center gap-4 border-t border-line dark:border-line-dark px-4 py-2.5 text-[13px]"
        style:grid-template-columns="1.5fr 1.7fr 1.3fr 140px 240px"
      >
        <div class="flex min-w-0 items-center gap-2.5 truncate">
          <span class="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-accent-soft dark:bg-accent-softDark text-[10.5px] font-semibold text-accent">{u.avatar}</span>
          <span class="truncate">
            {u.name}
            {#if u.isAdmin}<span class="ml-1.5 rounded bg-accent-soft px-1.5 py-0.5 text-[10px] font-medium text-accent dark:bg-accent-softDark">admin</span>{/if}
            {#if data.user && u.id === data.user.id}<span class="ml-1 text-[11px] text-ink-muted dark:text-ink-mutedDark">(you)</span>{/if}
          </span>
        </div>
        <div class="truncate text-ink-muted dark:text-ink-mutedDark">{u.email}</div>
        <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
          {u.permissions.length} product{u.permissions.length === 1 ? '' : 's'}
        </div>
        <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">{fmtDate(u.joinedAt)}</div>
        <div class="flex justify-end gap-2">
          <button
            type="button"
            class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink dark:text-ink-dark"
            onclick={() => {
              if (editingUserId === u.id) {
                editingUserId = null;
              } else {
                openEditUser(u);
              }
            }}
          >{editingUserId === u.id ? 'Cancel' : 'Edit'}</button>
          {#if !(data.user && u.id === data.user.id)}
            <form method="POST" action="/auth/impersonate/{u.id}">
              <button
                type="submit"
                class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink-muted dark:text-ink-mutedDark hover:text-accent"
              >Impersonate</button>
            </form>
            <form method="POST" action="?/delete" use:enhance>
              <input type="hidden" name="id" value={u.id} />
              <button
                type="button"
                class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink-muted dark:text-ink-mutedDark hover:text-red-600"
                onclick={(e) => { pendingConfirm = { message: `Delete ${u.name}?`, confirmLabel: 'Delete', form: (e.currentTarget as HTMLElement).closest('form')! }; }}
              >Delete</button>
            </form>
          {/if}
        </div>
      </div>

      {#if editingUserId === u.id}
        {@const isSelf = !!(data.user && u.id === data.user.id)}
        <div class="border-t border-line bg-surface-panel/55 px-4 py-4 dark:border-line-dark dark:bg-surface-panelDark/55">
          <form
            method="POST"
            action="?/update"
            use:enhance={() => async ({ update, result }) => {
              await update();
              if (result.type === 'success') editingUserId = null;
            }}
          >
            <input type="hidden" name="id" value={u.id} />
            <input type="hidden" name="isAdmin" value={String(editIsAdmin)} />
            <input type="hidden" name="permissions" value={JSON.stringify(editPermissions.map((p) => ({ productId: p.productId, role: p.role })))} />

            <div class="grid gap-4 lg:grid-cols-[1.1fr,1fr]">
              <div class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-4 py-3">
                <div class="mb-3 flex items-center justify-between">
                  <h2 class="text-[13px] font-medium">Account</h2>
                  <span class="text-[11px] text-ink-muted dark:text-ink-mutedDark">User id: {u.id}</span>
                </div>
                <div class="grid gap-3">
                  <label class="flex flex-col">
                    <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Name</span>
                    <input name="name" bind:value={editName} required class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-1.5 text-[13px]" />
                  </label>
                  <label class="flex flex-col">
                    <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Email</span>
                    <input name="email" type="email" bind:value={editEmail} required class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-1.5 text-[13px]" />
                  </label>
                </div>
              </div>

              <div class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-4 py-3">
                <div class="mb-3 flex items-center justify-between">
                  <h2 class="text-[13px] font-medium">Global access</h2>
                  <span class="text-[11px] text-ink-muted dark:text-ink-mutedDark">Admin applies across all products</span>
                </div>
                {#if isSelf}
                  <p class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
                    Admin status cannot be changed for your own account to avoid locking yourself out.
                  </p>
                {:else}
                  <label class="flex flex-col">
                    <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Admin access</span>
                    <select
                      bind:value={editIsAdmin}
                      class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]"
                    >
                      <option value={false}>Member</option>
                      <option value={true}>Administrator</option>
                    </select>
                  </label>
                {/if}
              </div>
            </div>

            <div class="mt-4 rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-4 py-3">
              <div class="mb-3 flex items-center justify-between">
                <h2 class="text-[13px] font-medium">Product permissions</h2>
                <span class="text-[11px] text-ink-muted dark:text-ink-mutedDark">{editPermissions.length} assigned</span>
              </div>

              <div class="space-y-2">
                {#if editPermissions.length === 0}
                  <p class="text-[12px] text-ink-muted dark:text-ink-mutedDark">No product access yet.</p>
                {:else}
                  {#each editPermissions as perm (perm.productId)}
                    <div class="grid items-center gap-3 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-2 lg:grid-cols-[1.2fr,160px,100px]">
                      <div class="flex min-w-0 items-center gap-2">
                        <span class="inline-block h-[10px] w-[10px] shrink-0 rounded-[3px]" style:background={perm.product.color}></span>
                        <span class="truncate text-[12.5px] font-medium">{perm.product.name}</span>
                      </div>
                      <select
                        value={perm.role}
                        onchange={(e) => {
                          const newRole = e.currentTarget.value;
                          editPermissions = editPermissions.map((ep) =>
                            ep.productId === perm.productId ? { ...ep, role: newRole } : ep
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
                          onclick={() => { editPermissions = editPermissions.filter((ep) => ep.productId !== perm.productId); }}
                          class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink-muted dark:text-ink-mutedDark hover:text-red-600"
                        >Remove</button>
                      </div>
                    </div>
                  {/each}
                {/if}
              </div>

              {#if editAvailableProducts.length > 0}
                <div class="mt-4 grid gap-3 rounded-md border border-dashed border-line dark:border-line-dark px-3 py-3 lg:grid-cols-[1.4fr,160px,120px]">
                  <label class="flex flex-col">
                    <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Add product</span>
                    <select bind:value={editAddProductId} class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]">
                      <option value="" disabled>Pick a product…</option>
                      {#each editAvailableProducts as product}
                        <option value={product.id}>{product.name}</option>
                      {/each}
                    </select>
                  </label>
                  <label class="flex flex-col">
                    <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Role</span>
                    <select bind:value={editAddRole} class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]">
                      <option value="readonly">Read-only</option>
                      <option value="readwrite">Read · write</option>
                      <option value="maintainer">Maintainer</option>
                    </select>
                  </label>
                  <div class="flex items-end justify-end">
                    <button
                      type="button"
                      onclick={addEditPermission}
                      class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white"
                    >Add</button>
                  </div>
                </div>
              {/if}
            </div>

            <div class="mt-4 flex justify-end gap-2">
              <button type="button" onclick={() => (editingUserId = null)} class="rounded-md border border-line dark:border-line-dark bg-transparent px-3 py-1.5 text-[13px]">Cancel</button>
              <button type="submit" class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white">Save user</button>
            </div>
          </form>
        </div>
      {/if}
    {/each}
  </div>
</div>
