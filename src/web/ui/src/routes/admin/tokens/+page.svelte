<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData, ActionData } from './$types';
  import type { CreatedApiToken, EntitlementDef } from '$lib/adapters/types';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  // ── create form ──────────────────────────────────────────────────────
  let showCreate = $state(false);
  let createDescription = $state('');
  let createProductId = $state('');
  let createUserId = $state('');
  let createEntitlements = $state<string[]>([]);
  let justCreated = $state<CreatedApiToken | null>(null);
  let copied = $state(false);

  $effect(() => {
    if (form?.created) {
      justCreated = form.created as CreatedApiToken;
      showCreate = false;
      createDescription = '';
      createProductId = '';
      createUserId = '';
      createEntitlements = [];
    }
  });

  function resetCreate() {
    showCreate = false;
    createDescription = '';
    createProductId = '';
    createUserId = '';
    createEntitlements = [];
  }

  // ── edit form ────────────────────────────────────────────────────────
  let editingId = $state<string | null>(null);
  let editDescription = $state('');
  let editIsActive = $state(true);
  let editProductId = $state('');
  let editUserId = $state('');
  let editEntitlements = $state<string[]>([]);

  function startEdit(token: PageData['tokens'][number]) {
    if (editingId === token.id) { editingId = null; return; }
    editingId = token.id;
    editDescription = token.description;
    editIsActive = token.isActive;
    editProductId = token.productId ?? '';
    editUserId = token.userId ?? '';
    editEntitlements = [...token.entitlements];
  }

  // ── entitlement helpers ──────────────────────────────────────────────
  function entAvailable(ent: EntitlementDef, pid: string, uid: string): boolean {
    if (ent.scope === 'product') return !!pid;
    if (ent.scope === 'user') return !!uid;
    return true;
  }

  function toggleEnt(list: string[], name: string, on: boolean): string[] {
    return on ? [...list, name] : list.filter(n => n !== name);
  }

  async function copyToken() {
    if (!justCreated) return;
    await navigator.clipboard.writeText(justCreated.token);
    copied = true;
    setTimeout(() => (copied = false), 2000);
  }
</script>

<div class="mx-auto max-w-[860px]">
  <div class="mb-6 flex items-end justify-between">
    <div>
      <h1 class="mb-1 text-[20px] font-semibold tracking-[-0.01em]">API tokens</h1>
      <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
        Tokens authenticate API calls. Scope a token to a product and/or user, then select the entitlements it should carry.
      </p>
    </div>
    <button
      type="button"
      onclick={() => (showCreate = !showCreate)}
      class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white"
    >New token</button>
  </div>

  <!-- One-time token display -->
  {#if justCreated}
    <div class="mb-6 rounded-md border border-green-300/70 dark:border-green-800/60 bg-green-50/60 dark:bg-green-950/30 px-5 py-4">
      <p class="mb-2 text-[13px] font-semibold text-green-800 dark:text-green-300">
        Token created — copy it now. It won't be shown again.
      </p>
      <div class="flex items-center gap-2">
        <code class="flex-1 overflow-x-auto rounded bg-surface dark:bg-surface-dark border border-line dark:border-line-dark px-3 py-2 text-[12px] font-mono text-ink dark:text-ink-dark">
          {justCreated.token}
        </code>
        <button
          type="button"
          onclick={copyToken}
          class="shrink-0 rounded-md border border-line dark:border-line-dark px-3 py-2 text-[12px] font-medium"
        >{copied ? 'Copied!' : 'Copy'}</button>
      </div>
      <button
        type="button"
        onclick={() => (justCreated = null)}
        class="mt-3 text-[12px] text-ink-muted dark:text-ink-mutedDark underline"
      >Dismiss</button>
    </div>
  {/if}

  <!-- Create token form -->
  {#if showCreate}
    <form
      method="POST"
      action="?/create"
      use:enhance={() => async ({ update }) => { await update({ reset: false }); }}
      class="mb-6 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-4 py-3"
    >
      <div class="grid gap-3 sm:grid-cols-3">
        <label class="flex flex-col">
          <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Description</span>
          <input
            name="description"
            bind:value={createDescription}
            placeholder="e.g. CI pipeline"
            required
            class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px] outline-none"
          />
        </label>
        <label class="flex flex-col">
          <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Product (optional)</span>
          <select
            name="productId"
            bind:value={createProductId}
            class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px] outline-none"
          >
            <option value="">— Any product —</option>
            {#each data.products as p}
              <option value={p.id}>{p.name}</option>
            {/each}
          </select>
        </label>
        <label class="flex flex-col">
          <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">User (optional)</span>
          <select
            name="userId"
            bind:value={createUserId}
            class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px] outline-none"
          >
            <option value="">— No user —</option>
            {#each data.users as u}
              <option value={u.id}>{u.name || u.email}</option>
            {/each}
          </select>
        </label>
      </div>

      <div class="mt-3">
        <div class="mb-1.5 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Entitlements</div>
        <div class="flex flex-wrap gap-2">
          {#each data.entitlements as ent}
            {@const avail = entAvailable(ent, createProductId, createUserId)}
            {@const checked = createEntitlements.includes(ent.name) && avail}
            <label
              class="flex items-center gap-1.5 rounded border border-line dark:border-line-dark px-2.5 py-1.5 text-[12px] select-none"
              class:opacity-40={!avail}
              class:cursor-not-allowed={!avail}
              class:cursor-pointer={avail}
            >
              <input
                type="checkbox"
                name="entitlement"
                value={ent.name}
                disabled={!avail}
                {checked}
                onchange={(e) => (createEntitlements = toggleEnt(createEntitlements, ent.name, e.currentTarget.checked))}
              />
              <span class="font-mono text-[11px]">{ent.name}</span>
              <span class="text-ink-muted dark:text-ink-mutedDark">— {ent.description}</span>
            </label>
          {/each}
        </div>
      </div>

      <div class="mt-3 flex items-center justify-between">
        {#if form?.error && !form?.created}
          <p class="text-[12px] text-red-600 dark:text-red-400">{form.error}</p>
        {:else}
          <span></span>
        {/if}
        <div class="flex gap-2">
          <button type="button" onclick={resetCreate} class="rounded-md border border-line dark:border-line-dark bg-transparent px-3 py-1.5 text-[13px]">Cancel</button>
          <button
            type="submit"
            disabled={!createDescription.trim()}
            class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white disabled:opacity-50"
          >Create</button>
        </div>
      </div>
    </form>
  {/if}

  <!-- Token list -->
  {#if data.tokens.length === 0}
    <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">No API tokens yet.</p>
  {:else}
    <div class="overflow-hidden rounded-md border border-line dark:border-line-dark">
      {#each data.tokens as token, i}
        <div
          class="flex items-center gap-3 px-4 py-3 text-[13px]"
          class:border-t={i > 0}
          class:border-line={i > 0}
          class:dark:border-line-dark={i > 0}
        >
          <div class="min-w-0 flex-1">
            <div class="flex flex-wrap items-center gap-1.5">
              <span class="font-medium text-ink dark:text-ink-dark">{token.description}</span>
              {#if token.productName}
                <span class="rounded bg-surface dark:bg-surface-dark border border-line dark:border-line-dark px-1.5 py-0.5 text-[11px] text-ink-muted dark:text-ink-mutedDark">
                  {token.productName}
                </span>
              {/if}
              {#if token.userName}
                <span class="rounded bg-surface dark:bg-surface-dark border border-line dark:border-line-dark px-1.5 py-0.5 text-[11px] text-ink-muted dark:text-ink-mutedDark">
                  @{token.userName}
                </span>
              {/if}
              {#if !token.productName && !token.userName}
                <span class="rounded bg-surface dark:bg-surface-dark border border-line dark:border-line-dark px-1.5 py-0.5 text-[11px] text-ink-muted dark:text-ink-mutedDark">
                  global
                </span>
              {/if}
            </div>
            <div class="mt-0.5 flex flex-wrap gap-x-3 text-[11.5px] text-ink-muted dark:text-ink-mutedDark">
              <span>Created {new Date(token.createdAt).toLocaleDateString()}</span>
              {#if token.lastUsedAt}
                <span>Last used {new Date(token.lastUsedAt).toLocaleDateString()}</span>
              {:else}
                <span>Never used</span>
              {/if}
              {#if !token.isActive}
                <span class="text-red-500 dark:text-red-400">Inactive</span>
              {/if}
            </div>
            <div class="mt-1 flex flex-wrap gap-1">
              {#each token.entitlements as ent}
                <span class="rounded bg-surface dark:bg-surface-dark border border-line dark:border-line-dark px-1.5 py-0.5 text-[11px] font-mono">
                  {ent}
                </span>
              {/each}
            </div>
          </div>

          <div class="flex shrink-0 gap-1.5">
            <button
              type="button"
              onclick={() => startEdit(token)}
              class="rounded-md border border-line dark:border-line-dark px-2.5 py-1 text-[12px]"
            >{editingId === token.id ? 'Close' : 'Edit'}</button>
            <form method="POST" action="?/delete" use:enhance>
              <input type="hidden" name="id" value={token.id} />
              <button
                type="submit"
                onclick={(e) => { if (!confirm('Delete this token?')) e.preventDefault(); }}
                class="rounded-md border border-line dark:border-line-dark px-2.5 py-1 text-[12px] text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30"
              >Delete</button>
            </form>
          </div>
        </div>

        {#if editingId === token.id}
          <div class="border-t border-line dark:border-line-dark bg-surface-panel/55 dark:bg-surface-panelDark/55 px-4 py-3">
            <form
              method="POST"
              action="?/update"
              use:enhance={() => async ({ update, result }) => {
                await update();
                if (result.type === 'success') editingId = null;
              }}
            >
              <input type="hidden" name="id" value={token.id} />

              <div class="grid gap-3 sm:grid-cols-4">
                <label class="flex flex-col sm:col-span-2">
                  <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Description</span>
                  <input
                    name="description"
                    bind:value={editDescription}
                    required
                    class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px] outline-none"
                  />
                </label>
                <label class="flex flex-col">
                  <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Status</span>
                  <select
                    name="isActive"
                    bind:value={editIsActive}
                    class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px] outline-none"
                  >
                    <option value={true}>Active</option>
                    <option value={false}>Inactive</option>
                  </select>
                </label>
              </div>

              <div class="mt-3 grid gap-3 sm:grid-cols-2">
                <label class="flex flex-col">
                  <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Product (optional)</span>
                  <select
                    name="productId"
                    bind:value={editProductId}
                    class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px] outline-none"
                  >
                    <option value="">— Any product —</option>
                    {#each data.products as p}
                      <option value={p.id}>{p.name}</option>
                    {/each}
                  </select>
                </label>
                <label class="flex flex-col">
                  <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">User (optional)</span>
                  <select
                    name="userId"
                    bind:value={editUserId}
                    class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px] outline-none"
                  >
                    <option value="">— No user —</option>
                    {#each data.users as u}
                      <option value={u.id}>{u.name || u.email}</option>
                    {/each}
                  </select>
                </label>
              </div>

              <div class="mt-3">
                <div class="mb-1.5 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Entitlements</div>
                <div class="flex flex-wrap gap-2">
                  {#each data.entitlements as ent}
                    {@const avail = entAvailable(ent, editProductId, editUserId)}
                    {@const checked = editEntitlements.includes(ent.name) && avail}
                    <label
                      class="flex items-center gap-1.5 rounded border border-line dark:border-line-dark px-2.5 py-1.5 text-[12px] select-none"
                      class:opacity-40={!avail}
                      class:cursor-not-allowed={!avail}
                      class:cursor-pointer={avail}
                    >
                      <input
                        type="checkbox"
                        name="entitlement"
                        value={ent.name}
                        disabled={!avail}
                        {checked}
                        onchange={(e) => (editEntitlements = toggleEnt(editEntitlements, ent.name, e.currentTarget.checked))}
                      />
                      <span class="font-mono text-[11px]">{ent.name}</span>
                      <span class="text-ink-muted dark:text-ink-mutedDark">— {ent.description}</span>
                    </label>
                  {/each}
                </div>
              </div>

              <div class="mt-3 flex justify-end gap-2">
                <button type="button" onclick={() => (editingId = null)} class="rounded-md border border-line dark:border-line-dark bg-transparent px-3 py-1.5 text-[13px]">Cancel</button>
                <button type="submit" class="rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white">Save</button>
              </div>
            </form>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>
