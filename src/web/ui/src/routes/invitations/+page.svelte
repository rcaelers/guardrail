<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData, ActionData } from './$types';
  import type { InvitationGrant, Role } from '$lib/adapters/types';
  import { fmtDate } from '$lib/utils/format';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  // --- Create form state ---
  let showCreate = $state(false);
  let createGrants = $state<InvitationGrant[]>([]);
  let createIsAdmin = $state(false);
  let createExpiresAt = $state('');
  let createMaxUses = $state('');
  let createProductPick = $state('');
  let createRolePick = $state<Role>('readonly');

  function addCreateGrant() {
    if (!createProductPick) return;
    if (createGrants.some((g) => g.product_id === createProductPick)) return;
    createGrants = [...createGrants, { product_id: createProductPick, role: createRolePick }];
    createProductPick = '';
    createRolePick = 'readonly';
  }

  function removeCreateGrant(pid: string) {
    createGrants = createGrants.filter((g) => g.product_id !== pid);
  }

  function resetCreate() {
    showCreate = false;
    createGrants = [];
    createIsAdmin = false;
    createExpiresAt = '';
    createMaxUses = '';
  }

  // --- Edit state ---
  let editingId = $state<string | null>(null);
  let editGrants = $state<InvitationGrant[]>([]);
  let editIsAdmin = $state(false);
  let editExpiresAt = $state('');
  let editMaxUses = $state('');
  let editProductPick = $state('');
  let editRolePick = $state<Role>('readonly');

  function startEdit(inv: PageData['invitations'][number]) {
    if (editingId === inv.id) { editingId = null; return; }
    editingId = inv.id;
    editGrants = inv.grants.map((g) => ({ ...g }));
    editIsAdmin = inv.is_admin;
    editExpiresAt = inv.expires_at ? inv.expires_at.slice(0, 16) : '';
    editMaxUses = inv.max_uses != null ? String(inv.max_uses) : '';
    editProductPick = '';
    editRolePick = 'readonly';
  }

  function addEditGrant() {
    if (!editProductPick) return;
    if (editGrants.some((g) => g.product_id === editProductPick)) return;
    editGrants = [...editGrants, { product_id: editProductPick, role: editRolePick }];
    editProductPick = '';
    editRolePick = 'readonly';
  }

  function removeEditGrant(pid: string) {
    editGrants = editGrants.filter((g) => g.product_id !== pid);
  }

  // Derived: product name lookup
  const productName = (id: string) =>
    data.assignableProducts.find((p) => p.id === id)?.name ?? id;

  // Products not yet in grants list for add-row picker
  const createAvailable = $derived(
    data.assignableProducts.filter((p) => !createGrants.some((g) => g.product_id === p.id))
  );
  const editAvailable = $derived(
    data.assignableProducts.filter((p) => !editGrants.some((g) => g.product_id === p.id))
  );

  function copyLink(code: string) {
    navigator.clipboard.writeText(`${data.origin}/invite/${code}`);
  }

  function statusClass(s: string) {
    if (s === 'Active') return 'text-green-600 dark:text-green-400';
    if (s === 'Exhausted') return 'text-amber-600 dark:text-amber-400';
    return 'text-ink-muted dark:text-ink-mutedDark';
  }
</script>

<div class="mx-auto max-w-[1100px]">
  <div class="mb-6 flex items-end justify-between">
    <div>
      <h1 class="mb-1 text-[20px] font-semibold tracking-[-0.01em]">Invitations</h1>
      <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
        Invite people by sending them a link. Access rights are granted on first login.
      </p>
    </div>
    <button
      type="button"
      onclick={() => (showCreate = !showCreate)}
      class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white"
    >New invitation</button>
  </div>

  {#if form?.error}
    <p class="mb-4 text-[12px] text-red-600 dark:text-red-400">{form.error}</p>
  {/if}

  <!-- Create form -->
  {#if showCreate}
    <form
      method="POST"
      action="?/create"
      use:enhance={() => async ({ update, result }) => {
        await update();
        if (result.type === 'success') resetCreate();
      }}
      class="mb-6 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-5 py-4"
    >
      <h2 class="mb-4 text-[13px] font-medium">New invitation</h2>

      <div class="grid gap-4 lg:grid-cols-3">
        <label class="flex flex-col">
          <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Expires at</span>
          <input
            name="expires_at"
            type="datetime-local"
            bind:value={createExpiresAt}
            class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px]"
          />
        </label>
        <label class="flex flex-col">
          <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Max uses</span>
          <input
            name="max_uses"
            type="number"
            min="1"
            placeholder="Unlimited"
            bind:value={createMaxUses}
            class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px]"
          />
        </label>
        {#if data.isAdmin}
          <label class="flex flex-col">
            <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Admin invitation</span>
            <select
              name="is_admin"
              bind:value={createIsAdmin}
              class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px]"
            >
              <option value={false}>No</option>
              <option value={true}>Yes — grants full admin</option>
            </select>
          </label>
        {:else}
          <input type="hidden" name="is_admin" value="false" />
        {/if}
      </div>

      <!-- Grants editor -->
      <div class="mt-4">
        <div class="mb-2 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Product access</div>

        {#each createGrants as g (g.product_id)}
          <input type="hidden" name="grant_product" value={g.product_id} />
          <input type="hidden" name="grant_role" value={g.role} />
          <div class="mb-1.5 flex items-center gap-2 rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-2 text-[13px]">
            <span class="flex-1 font-medium">{productName(g.product_id)}</span>
            <select
              value={g.role}
              onchange={(e) => { g.role = e.currentTarget.value as Role; createGrants = [...createGrants]; }}
              class="rounded border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-0.5 text-[12px]"
            >
              <option value="readonly">Read-only</option>
              <option value="readwrite">Read · write</option>
              <option value="maintainer">Maintainer</option>
            </select>
            <button type="button" onclick={() => removeCreateGrant(g.product_id)} class="text-[11px] text-ink-muted hover:text-red-600">×</button>
          </div>
        {/each}

        {#if createAvailable.length > 0}
          <div class="mt-2 flex items-center gap-2">
            <select
              bind:value={createProductPick}
              class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]"
            >
              <option value="" disabled>Add product…</option>
              {#each createAvailable as p}
                <option value={p.id}>{p.name}</option>
              {/each}
            </select>
            <select
              bind:value={createRolePick}
              class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]"
            >
              <option value="readonly">Read-only</option>
              <option value="readwrite">Read · write</option>
              <option value="maintainer">Maintainer</option>
            </select>
            <button
              type="button"
              onclick={addCreateGrant}
              disabled={!createProductPick}
              class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1.5 text-[12px] disabled:opacity-40"
            >Add</button>
          </div>
        {/if}
      </div>

      <div class="mt-4 flex justify-end gap-2">
        <button type="button" onclick={resetCreate} class="rounded-md border border-line dark:border-line-dark bg-transparent px-3 py-1.5 text-[13px]">Cancel</button>
        <button type="submit" class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white">Create</button>
      </div>
    </form>
  {/if}

  <!-- Invitations table -->
  <div class="overflow-hidden rounded-md border border-line dark:border-line-dark">
    <div
      class="grid items-center gap-4 bg-surface-panel dark:bg-surface-panelDark px-4 py-2 text-[10.5px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark"
      style:grid-template-columns="180px 90px 130px 120px 1fr 130px"
    >
      <span>Code</span>
      <span>Status</span>
      <span>Created</span>
      <span>Uses</span>
      <span>Products</span>
      <span></span>
    </div>

    {#if data.invitations.length === 0}
      <div class="px-4 py-8 text-center text-[13px] text-ink-muted dark:text-ink-mutedDark">
        No invitations yet.
      </div>
    {/if}

    {#each data.invitations as inv (inv.id)}
      {@const isExpired = inv.expires_at && new Date(inv.expires_at) < new Date()}
      {@const displayStatus = isExpired && inv.status === 'Active' ? 'Expired' : inv.status}
      {@const isMine = inv.created_by === data.currentUserId}
      {@const creatorLabel = data.isAdmin ? (data.userMap[inv.created_by] ?? inv.created_by) : isMine ? 'you' : '—'}

      <div
        class="grid items-center gap-4 border-t border-line dark:border-line-dark px-4 py-2.5 text-[13px]"
        style:grid-template-columns="180px 90px 130px 120px 1fr 130px"
      >
        <!-- Code + copy -->
        <div class="flex min-w-0 items-center gap-1.5">
          <code class="truncate rounded bg-surface-panel dark:bg-surface-panelDark px-1.5 py-0.5 font-mono text-[11px]">
            {inv.code.slice(0, 12)}…
          </code>
          {#if inv.status === 'Active' && !isExpired}
            <button
              type="button"
              onclick={() => copyLink(inv.code)}
              title="Copy invite link"
              class="shrink-0 rounded border border-line dark:border-line-dark px-1.5 py-0.5 text-[10px] text-ink-muted hover:text-accent"
            >copy</button>
          {/if}
        </div>

        <!-- Status -->
        <span class="text-[12px] font-medium {statusClass(displayStatus)}">{displayStatus}</span>

        <!-- Created -->
        <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
          <div>{fmtDate(inv.created_at)}</div>
          <div class="text-[11px]">by {creatorLabel}</div>
        </div>

        <!-- Uses -->
        <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
          {inv.use_count}{inv.max_uses != null ? ` / ${inv.max_uses}` : ''}
          {#if inv.expires_at}
            <div class="text-[11px]">exp {fmtDate(inv.expires_at)}</div>
          {/if}
        </div>

        <!-- Grants summary -->
        <div class="flex min-w-0 flex-wrap gap-1">
          {#each inv.grants as g}
            {@const name = data.assignableProducts.find((p) => p.id === g.product_id)?.name ?? g.product_id}
            <span class="rounded bg-surface-panel dark:bg-surface-panelDark px-1.5 py-0.5 text-[11px]">
              {name} · {g.role}
            </span>
          {/each}
          {#if inv.is_admin}
            <span class="rounded bg-accent-soft dark:bg-accent-softDark px-1.5 py-0.5 text-[11px] font-medium text-accent">admin</span>
          {/if}
        </div>

        <!-- Actions -->
        <div class="flex justify-end gap-1.5">
          {#if inv.status !== 'Revoked'}
            <button
              type="button"
              onclick={() => startEdit(inv)}
              class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px]"
            >{editingId === inv.id ? 'Close' : 'Edit'}</button>
            <form method="POST" action="?/revoke" use:enhance>
              <input type="hidden" name="id" value={inv.id} />
              <button
                type="submit"
                onclick={(e) => { if (!confirm('Revoke this invitation?')) e.preventDefault(); }}
                class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[11.5px] text-ink-muted hover:text-red-600"
              >Revoke</button>
            </form>
          {/if}
        </div>
      </div>

      <!-- Inline edit panel -->
      {#if editingId === inv.id}
        <div class="border-t border-line dark:border-line-dark bg-surface-panel/55 px-5 py-4 dark:bg-surface-panelDark/55">
          <form
            method="POST"
            action="?/update"
            use:enhance={() => async ({ update, result }) => {
              await update();
              if (result.type === 'success') editingId = null;
            }}
          >
            <input type="hidden" name="id" value={inv.id} />

            <div class="grid gap-4 lg:grid-cols-3">
              <label class="flex flex-col">
                <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Expires at</span>
                <input
                  name="expires_at"
                  type="datetime-local"
                  bind:value={editExpiresAt}
                  class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px]"
                />
              </label>
              <label class="flex flex-col">
                <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Max uses</span>
                <input
                  name="max_uses"
                  type="number"
                  min="1"
                  placeholder="Unlimited"
                  bind:value={editMaxUses}
                  class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px]"
                />
              </label>
              {#if data.isAdmin}
                <label class="flex flex-col">
                  <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Admin invitation</span>
                  <select
                    name="is_admin"
                    bind:value={editIsAdmin}
                    class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px]"
                  >
                    <option value={false}>No</option>
                    <option value={true}>Yes — grants full admin</option>
                  </select>
                </label>
              {:else}
                <input type="hidden" name="is_admin" value={inv.is_admin} />
              {/if}
            </div>

            <!-- Grants editor -->
            <div class="mt-4">
              <div class="mb-2 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Product access</div>

              {#each editGrants as g (g.product_id)}
                {@const editable = data.assignableProducts.some((p) => p.id === g.product_id)}
                <input type="hidden" name="grant_product" value={g.product_id} />
                <input type="hidden" name="grant_role" value={g.role} />
                <div class="mb-1.5 flex items-center gap-2 rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-2 text-[13px]">
                  <span class="flex-1 font-medium {editable ? '' : 'text-ink-muted dark:text-ink-mutedDark'}">{productName(g.product_id)}</span>
                  {#if editable}
                    <select
                      value={g.role}
                      onchange={(e) => { g.role = e.currentTarget.value as Role; editGrants = [...editGrants]; }}
                      class="rounded border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-0.5 text-[12px]"
                    >
                      <option value="readonly">Read-only</option>
                      <option value="readwrite">Read · write</option>
                      <option value="maintainer">Maintainer</option>
                    </select>
                    <button type="button" onclick={() => removeEditGrant(g.product_id)} class="text-[11px] text-ink-muted hover:text-red-600">×</button>
                  {:else}
                    <span class="text-[12px] text-ink-muted dark:text-ink-mutedDark">{g.role} (read-only)</span>
                  {/if}
                </div>
              {/each}

              {#if editAvailable.length > 0}
                <div class="mt-2 flex items-center gap-2">
                  <select
                    bind:value={editProductPick}
                    class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]"
                  >
                    <option value="" disabled>Add product…</option>
                    {#each editAvailable as p}
                      <option value={p.id}>{p.name}</option>
                    {/each}
                  </select>
                  <select
                    bind:value={editRolePick}
                    class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-2 py-1.5 text-[13px]"
                  >
                    <option value="readonly">Read-only</option>
                    <option value="readwrite">Read · write</option>
                    <option value="maintainer">Maintainer</option>
                  </select>
                  <button
                    type="button"
                    onclick={addEditGrant}
                    disabled={!editProductPick}
                    class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1.5 text-[12px] disabled:opacity-40"
                  >Add</button>
                </div>
              {/if}
            </div>

            <div class="mt-4 flex justify-end gap-2">
              <button type="button" onclick={() => (editingId = null)} class="rounded-md border border-line dark:border-line-dark bg-transparent px-3 py-1.5 text-[13px]">Cancel</button>
              <button type="submit" class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white">Save</button>
            </div>
          </form>
        </div>
      {/if}
    {/each}
  </div>
</div>
