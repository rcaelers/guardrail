<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData, ActionData } from './$types';
  import { fmtDate } from '$lib/utils/format';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  let newEmail = $state('');
  let newName = $state('');
  let showCreate = $state(false);
</script>

<div class="mx-auto max-w-[960px]">
  <div class="mb-6 flex items-end justify-between">
    <div>
      <h1 class="mb-1 text-[20px] font-semibold tracking-[-0.01em]">Users</h1>
      <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
        {data.users.length} accounts. Only administrators can create or delete users.
      </p>
    </div>
    <button
      type="button"
      onclick={() => (showCreate = !showCreate)}
      class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white"
    >New user</button>
  </div>

  {#if showCreate}
    <form
      method="POST"
      action="?/create"
      use:enhance={() => async ({ update, result }) => {
        await update();
        if (result.type === 'success') {
          newEmail = ''; newName = ''; showCreate = false;
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
    {#if form?.error}
      <p class="mb-4 text-[12px] text-red-600 dark:text-red-400">{form.error}</p>
    {/if}
  {/if}

  <div class="overflow-hidden rounded-md border border-line dark:border-line-dark">
    <div
      class="grid items-center gap-4 bg-surface-panel dark:bg-surface-panelDark px-4 py-2 text-[10.5px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark"
      style:grid-template-columns="1.5fr 1.8fr 110px 160px 140px"
    >
      <span>User</span>
      <span>Email</span>
      <span>Admin</span>
      <span>Joined</span>
      <span></span>
    </div>
    {#each data.users as u (u.id)}
      <div
        class="grid items-center gap-4 border-t border-line dark:border-line-dark px-4 py-2.5 text-[13px]"
        style:grid-template-columns="1.5fr 1.8fr 110px 160px 140px"
      >
        <div class="flex min-w-0 items-center gap-2.5 truncate">
          <span class="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-accent-soft dark:bg-accent-softDark text-[10.5px] font-semibold text-accent">{u.avatar}</span>
          <span class="truncate">{u.name}
            {#if data.user && u.id === data.user.id}<span class="ml-1 text-[11px] text-ink-muted dark:text-ink-mutedDark">(you)</span>{/if}
          </span>
        </div>
        <div class="truncate text-ink-muted dark:text-ink-mutedDark">{u.email}</div>
        <div>
          <form method="POST" action="?/toggleAdmin" use:enhance class="flex items-center">
            <input type="hidden" name="id" value={u.id} />
            <input type="hidden" name="isAdmin" value={(!u.isAdmin).toString()} />
            <button
              type="submit"
              class="rounded-full px-2 py-0.5 text-[11px] font-medium"
              class:bg-accent-soft={u.isAdmin}
              class:dark:bg-accent-softDark={u.isAdmin}
              class:text-accent={u.isAdmin}
              class:border={!u.isAdmin}
              class:border-line={!u.isAdmin}
              class:dark:border-line-dark={!u.isAdmin}
              class:text-ink-muted={!u.isAdmin}
              class:dark:text-ink-mutedDark={!u.isAdmin}
              title={u.isAdmin ? 'Click to demote' : 'Click to promote'}
            >{u.isAdmin ? 'admin' : 'member'}</button>
          </form>
        </div>
        <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">{fmtDate(u.joinedAt)}</div>
        <div class="flex justify-end">
          {#if !(data.user && u.id === data.user.id)}
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
    {/each}
  </div>
</div>
