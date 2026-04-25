<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData, ActionData } from './$types';
  import { fmtDate } from '$lib/utils/format';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  let newEmail = $state('');
  let newName = $state('');
  let showCreate = $state(false);
  let editingUserId = $state<string | null>(null);
</script>

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
          showCreate = false;
        }
      }}
      class="mb-5 flex items-end gap-3 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-4 py-3"
    >
      <label class="flex flex-1 flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Email</span>
        <input name="email" type="email" required bind:value={newEmail} class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px]" />
      </label>
      <label class="flex flex-1 flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Name</span>
        <input name="name" bind:value={newName} class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px]" />
      </label>
      <button type="submit" class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white">Create</button>
      <button type="button" onclick={() => (showCreate = false)} class="rounded-md border border-line dark:border-line-dark bg-transparent px-3 py-1.5 text-[13px]">Cancel</button>
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
            onclick={() => (editingUserId = editingUserId === u.id ? null : u.id)}
          >{editingUserId === u.id ? 'Close' : 'Edit'}</button>
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
                type="submit"
                class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink-muted dark:text-ink-mutedDark hover:text-red-600"
                onclick={(e) => { if (!confirm(`Delete ${u.name}?`)) e.preventDefault(); }}
              >Delete</button>
            </form>
          {/if}
        </div>
      </div>

      {#if editingUserId === u.id}
        {@const availableProducts = data.products.filter((product) => !u.permissions.some((permission) => permission.productId === product.id))}
        <div class="border-t border-line bg-surface-panel/55 px-4 py-4 dark:border-line-dark dark:bg-surface-panelDark/55">
          <div class="grid gap-4 lg:grid-cols-[1.1fr,1fr]">
            <form
              method="POST"
              action="?/update"
              use:enhance={() => async ({ update, result }) => {
                await update();
                if (result.type === 'success') editingUserId = null;
              }}
              class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-4 py-3"
            >
              <input type="hidden" name="id" value={u.id} />
              <div class="mb-3 flex items-center justify-between">
                <h2 class="text-[13px] font-medium">Account</h2>
                <span class="text-[11px] text-ink-muted dark:text-ink-mutedDark">User id: {u.id}</span>
              </div>
              <div class="grid gap-3">
                <label class="flex flex-col">
                  <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Name</span>
                  <input name="name" value={u.name} required class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-1.5 text-[13px]" />
                </label>
                <label class="flex flex-col">
                  <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Email</span>
                  <input name="email" type="email" value={u.email} required class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-1.5 text-[13px]" />
                </label>
                <div class="flex justify-end">
                  <button type="submit" class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white">Save user</button>
                </div>
              </div>
            </form>

            <div class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-4 py-3">
              <div class="mb-3 flex items-center justify-between">
                <h2 class="text-[13px] font-medium">Global access</h2>
                <span class="text-[11px] text-ink-muted dark:text-ink-mutedDark">Admin applies across all products</span>
              </div>
              {#if data.user && u.id === data.user.id}
                <p class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
                  You can edit your own name and email, but self-demotion is blocked to avoid locking yourself out of the admin console.
                </p>
              {:else}
                <form method="POST" action="?/toggleAdmin" use:enhance class="flex items-end gap-3">
                  <input type="hidden" name="id" value={u.id} />
                  <label class="flex flex-col">
                    <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Admin access</span>
                    <select
                      name="isAdmin"
                      value={u.isAdmin ? 'true' : 'false'}
                      onchange={(e) => (e.currentTarget.form as HTMLFormElement).requestSubmit()}
                      class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]"
                    >
                      <option value="false">Member</option>
                      <option value="true">Administrator</option>
                    </select>
                  </label>
                </form>
              {/if}
            </div>
          </div>

          <div class="mt-4 rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-4 py-3">
            <div class="mb-3 flex items-center justify-between">
              <h2 class="text-[13px] font-medium">Product permissions</h2>
              <span class="text-[11px] text-ink-muted dark:text-ink-mutedDark">{u.permissions.length} assigned</span>
            </div>

            <div class="space-y-2">
              {#if u.permissions.length === 0}
                <p class="text-[12px] text-ink-muted dark:text-ink-mutedDark">No product access yet.</p>
              {:else}
                {#each u.permissions as permission (permission.productId)}
                  <div class="grid items-center gap-3 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-2 lg:grid-cols-[1.2fr,160px,100px]">
                    <div class="flex min-w-0 items-center gap-2">
                      <span class="inline-block h-[10px] w-[10px] shrink-0 rounded-[3px]" style:background={permission.product.color}></span>
                      <span class="truncate text-[12.5px] font-medium">{permission.product.name}</span>
                    </div>
                    {#if data.user && u.id === data.user.id}
                      <span class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
                        {permission.role === 'maintainer' ? 'Maintainer' : permission.role === 'readwrite' ? 'Read · write' : 'Read-only'}
                      </span>
                    {:else}
                      <form method="POST" action="?/setPermission" use:enhance class="flex">
                        <input type="hidden" name="userId" value={u.id} />
                        <input type="hidden" name="productId" value={permission.productId} />
                        <select
                          name="role"
                          value={permission.role}
                          onchange={(e) => (e.currentTarget.form as HTMLFormElement).requestSubmit()}
                          class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1 text-[12px]"
                        >
                          <option value="readonly">Read-only</option>
                          <option value="readwrite">Read · write</option>
                          <option value="maintainer">Maintainer</option>
                        </select>
                      </form>
                    {/if}
                    <div class="flex justify-end">
                      {#if !(data.user && u.id === data.user.id)}
                        <form method="POST" action="?/revokePermission" use:enhance>
                          <input type="hidden" name="userId" value={u.id} />
                          <input type="hidden" name="productId" value={permission.productId} />
                          <button
                            type="submit"
                            class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink-muted dark:text-ink-mutedDark hover:text-red-600"
                            onclick={(e) => { if (!confirm(`Revoke ${permission.product.name} access for ${u.name}?`)) e.preventDefault(); }}
                          >Revoke</button>
                        </form>
                      {/if}
                    </div>
                  </div>
                {/each}
              {/if}
            </div>

            {#if !(data.user && u.id === data.user.id) && availableProducts.length > 0}
              <form method="POST" action="?/setPermission" use:enhance class="mt-4 grid gap-3 rounded-md border border-dashed border-line dark:border-line-dark px-3 py-3 lg:grid-cols-[1.4fr,160px,120px]">
                <input type="hidden" name="userId" value={u.id} />
                <label class="flex flex-col">
                  <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Add product</span>
                  <select name="productId" required class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]">
                    <option value="" disabled selected>Pick a product…</option>
                    {#each availableProducts as product}
                      <option value={product.id}>{product.name}</option>
                    {/each}
                  </select>
                </label>
                <label class="flex flex-col">
                  <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Role</span>
                  <select name="role" class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]">
                    <option value="readonly">Read-only</option>
                    <option value="readwrite">Read · write</option>
                    <option value="maintainer">Maintainer</option>
                  </select>
                </label>
                <div class="flex items-end justify-end">
                  <button type="submit" class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white">Grant access</button>
                </div>
              </form>
            {/if}
          </div>
        </div>
      {/if}
    {/each}
  </div>
</div>
