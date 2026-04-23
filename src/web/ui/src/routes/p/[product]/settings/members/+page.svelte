<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData } from './$types';
  import { fmtDate } from '$lib/utils/format';

  let { data }: { data: PageData } = $props();

  const canManage = $derived(data.role === 'maintainer' || data.user?.isAdmin === true);

  let grantUserId = $state('');
  let grantRole = $state<'readonly' | 'readwrite' | 'maintainer'>('readonly');
</script>

<div class="mx-auto max-w-[920px]">
  <div class="mb-6">
    <h1 class="mb-1 text-[20px] font-semibold tracking-[-0.01em]">Members</h1>
    <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
      Who has access to <span class="font-medium text-ink dark:text-ink-dark">{data.product.name}</span> and what they can do.
    </p>
  </div>

  <!-- Grant new access -->
  {#if canManage}
    <form
      method="POST"
      action="?/grant"
      use:enhance={() => async ({ update }) => { await update(); grantUserId = ''; }}
      class="mb-6 flex items-end gap-3 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-4 py-3"
    >
      <label class="flex min-w-0 flex-1 flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Add user</span>
        <select
          name="userId"
          bind:value={grantUserId}
          required
          class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px]"
        >
          <option value="" disabled>Pick a user…</option>
          {#each data.nonMembers as u}
            <option value={u.id}>{u.name} · {u.email}</option>
          {/each}
        </select>
      </label>
      <label class="flex flex-col">
        <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Role</span>
        <select
          name="role"
          bind:value={grantRole}
          class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px]"
        >
          <option value="readonly">Read-only</option>
          <option value="readwrite">Read · write</option>
          <option value="maintainer">Maintainer</option>
        </select>
      </label>
      <button type="submit" class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white" disabled={!grantUserId}>
        Grant access
      </button>
    </form>
  {/if}

  <!-- Member list -->
  <div class="overflow-hidden rounded-md border border-line dark:border-line-dark">
    <div
      class="grid items-center gap-4 bg-surface-panel dark:bg-surface-panelDark px-4 py-2 text-[10.5px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark"
      style:grid-template-columns="1.5fr 1.5fr 160px 160px 100px"
    >
      <span>User</span>
      <span>Email</span>
      <span>Role</span>
      <span>Joined</span>
      <span></span>
    </div>
    {#each data.members as m (m.userId)}
      <div
        class="grid items-center gap-4 border-t border-line dark:border-line-dark px-4 py-2.5 text-[13px]"
        style:grid-template-columns="1.5fr 1.5fr 160px 160px 100px"
      >
        <div class="flex min-w-0 items-center gap-2.5 truncate">
          <span class="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-accent-soft dark:bg-accent-softDark text-[10.5px] font-semibold text-accent">{m.user.avatar}</span>
          <span class="truncate">
            {m.user.name}
            {#if m.user.isAdmin}<span class="ml-1.5 rounded bg-ink/10 dark:bg-ink-dark/20 px-1.5 py-0.5 text-[10px] font-medium">admin</span>{/if}
            {#if data.user && m.userId === data.user.id}<span class="ml-1 text-[11px] text-ink-muted dark:text-ink-mutedDark">(you)</span>{/if}
          </span>
        </div>
        <div class="truncate text-ink-muted dark:text-ink-mutedDark">{m.user.email}</div>
        <div>
          {#if canManage && !(data.user && m.userId === data.user.id)}
            <form method="POST" action="?/changeRole" use:enhance class="flex">
              <input type="hidden" name="userId" value={m.userId} />
              <select
                name="role"
                value={m.role}
                onchange={(e) => (e.currentTarget.form as HTMLFormElement).requestSubmit()}
                class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1 text-[12px]"
              >
                <option value="readonly">Read-only</option>
                <option value="readwrite">Read · write</option>
                <option value="maintainer">Maintainer</option>
              </select>
            </form>
          {:else}
            <span class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
              {m.role === 'maintainer' ? 'Maintainer' : m.role === 'readwrite' ? 'Read · write' : 'Read-only'}
            </span>
          {/if}
        </div>
        <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">{fmtDate(m.user.joinedAt)}</div>
        <div class="flex justify-end">
          {#if canManage && !(data.user && m.userId === data.user.id)}
            <form method="POST" action="?/revoke" use:enhance>
              <input type="hidden" name="userId" value={m.userId} />
              <button
                type="submit"
                class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink-muted dark:text-ink-mutedDark hover:text-red-600"
                onclick={(e) => { if (!confirm(`Revoke access for ${m.user.name}?`)) e.preventDefault(); }}
              >Revoke</button>
            </form>
          {/if}
        </div>
      </div>
    {/each}
  </div>

  {#if !canManage}
    <p class="mt-4 text-[12px] text-ink-muted dark:text-ink-mutedDark">
      Only maintainers can grant or revoke access.
    </p>
  {/if}
</div>
