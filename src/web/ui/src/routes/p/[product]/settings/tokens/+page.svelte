<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData, ActionData } from './$types';
  import type { CreatedApiToken } from '$lib/adapters/types';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  const canManage = $derived(data.role === 'maintainer' || data.user?.isAdmin === true);

  let description = $state('');
  let justCreated = $state<CreatedApiToken | null>(null);
  let copied = $state(false);

  $effect(() => {
    if (form?.created) {
      justCreated = form.created as CreatedApiToken;
      description = '';
    }
  });

  async function copyToken() {
    if (!justCreated) return;
    await navigator.clipboard.writeText(justCreated.token);
    copied = true;
    setTimeout(() => (copied = false), 2000);
  }
</script>

<div class="mx-auto max-w-[720px]">
  <div class="mb-6">
    <h1 class="mb-1 text-[20px] font-semibold tracking-[-0.01em]">API tokens</h1>
    <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
      Tokens let automated systems submit crash reports and upload symbols for
      <span class="font-medium text-ink dark:text-ink-dark">{data.product.name}</span>.
    </p>
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
        >
          {copied ? 'Copied!' : 'Copy'}
        </button>
      </div>
      <button
        type="button"
        onclick={() => (justCreated = null)}
        class="mt-3 text-[12px] text-ink-muted dark:text-ink-mutedDark underline"
      >
        Dismiss
      </button>
    </div>
  {/if}

  <!-- Create token form -->
  {#if canManage}
    <form
      method="POST"
      action="?/create"
      use:enhance={() => async ({ update }) => { await update({ reset: false }); }}
      class="mb-6 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-4 py-3"
    >
      <p class="mb-3 text-[12px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">
        New token
      </p>
      <div class="flex items-end gap-3">
        <label class="flex min-w-0 flex-1 flex-col">
          <span class="mb-1 text-[11px] uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Description</span>
          <input
            name="description"
            bind:value={description}
            placeholder="e.g. CI pipeline"
            required
            class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-2 py-1.5 text-[13px] outline-none"
          />
        </label>
        <button
          type="submit"
          disabled={!description.trim()}
          class="shrink-0 rounded-md bg-accent px-3 py-1.5 text-[13px] font-medium text-white disabled:opacity-50"
        >
          Create
        </button>
      </div>
      {#if form?.error && !form?.created}
        <p class="mt-2 text-[12px] text-red-600 dark:text-red-400">{form.error}</p>
      {/if}
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
            <div class="font-medium text-ink dark:text-ink-dark">{token.description}</div>
            <div class="mt-0.5 flex flex-wrap gap-x-3 text-[11.5px] text-ink-muted dark:text-ink-mutedDark">
              <span>Created {new Date(token.createdAt).toLocaleDateString()}</span>
              {#if token.lastUsedAt}
                <span>Last used {new Date(token.lastUsedAt).toLocaleDateString()}</span>
              {:else}
                <span>Never used</span>
              {/if}
              {#if !token.isActive}
                <span class="text-red-500 dark:text-red-400">Revoked</span>
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

          {#if canManage}
            <form method="POST" action="?/delete" use:enhance>
              <input type="hidden" name="id" value={token.id} />
              <button
                type="submit"
                class="rounded-md border border-line dark:border-line-dark px-2.5 py-1 text-[12px] text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30"
              >
                Delete
              </button>
            </form>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
