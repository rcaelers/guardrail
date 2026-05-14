<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData, ActionData } from './$types';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  let tokenInput = $state(data.productToken ?? '');

  $effect(() => {
    if (form?.productToken !== undefined) {
      tokenInput = form.productToken ?? '';
    }
  });

  const displayToken = $derived(tokenInput || '(not set)');
  const hasToken = $derived(tokenInput.trim().length > 0);
</script>

<div class="mx-auto max-w-[700px]">
  <div class="mb-6">
    <h1 class="mb-1 text-[20px] font-semibold tracking-[-0.01em]">Product token</h1>
    <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
      This token is embedded in the crash upload URL and identifies which product a minidump or symbol upload belongs to.
    </p>
  </div>

  {#if form?.ok}
    <p class="mb-4 text-[12px] text-green-600 dark:text-green-400">Token saved.</p>
  {/if}
  {#if form?.error}
    <p class="mb-4 text-[12px] text-red-600 dark:text-red-400">{form.error}</p>
  {/if}

  <div class="rounded-md border border-line dark:border-line-dark overflow-hidden mb-6">
    <div class="bg-surface-panel dark:bg-surface-panelDark px-4 py-3 border-b border-line dark:border-line-dark">
      <div class="text-[13px] font-medium">Current token</div>
      <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
        {hasToken ? 'Token is set' : 'No token configured — upload URL will not work'}
      </div>
    </div>
    <div class="px-4 py-3 bg-surface dark:bg-surface-dark">
      <code class="font-mono text-[12px] break-all select-all">{displayToken}</code>
    </div>
  </div>

  <form method="POST" action="?/save" use:enhance class="space-y-4 mb-4">
    <div>
      <label for="product_token" class="block text-[13px] font-medium mb-1">
        Set token manually
      </label>
      <p class="text-[12px] text-ink-muted dark:text-ink-mutedDark mb-2">
        Enter a custom token value, or leave blank and click Save to keep the current token unchanged.
        Use "Generate new token" below to create a random token.
      </p>
      <input
        id="product_token"
        name="product_token"
        type="text"
        bind:value={tokenInput}
        placeholder="Leave empty to keep current token"
        spellcheck="false"
        autocomplete="off"
        class="w-full rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 font-mono text-[12px] outline-none focus:ring-1 focus:ring-accent"
      />
    </div>
    <div class="flex justify-end">
      <button
        type="submit"
        class="rounded-md bg-accent px-4 py-1.5 text-[13px] font-medium text-white"
      >Save</button>
    </div>
  </form>

  <div class="rounded-md border border-line dark:border-line-dark overflow-hidden">
    <div class="bg-surface-panel dark:bg-surface-panelDark px-4 py-3 border-b border-line dark:border-line-dark">
      <div class="text-[13px] font-medium">Generate new token</div>
      <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
        Replaces the current token with a new random value. You will need to update the upload URL in your application.
      </div>
    </div>
    <div class="px-4 py-3 bg-surface dark:bg-surface-dark flex justify-end">
      <form method="POST" action="?/regenerate" use:enhance>
        <button
          type="submit"
          class="rounded-md border border-red-500 px-4 py-1.5 text-[13px] font-medium text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950"
        >Generate new token</button>
      </form>
    </div>
  </div>
</div>
