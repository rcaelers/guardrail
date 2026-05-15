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

  const base = data.ingestionUrl;
  const minidumpUrl = $derived(
    hasToken && base ? `${base}/api/minidump/${tokenInput}/upload` : null
  );
  const symbolsUrl = $derived(
    hasToken && base ? `${base}/api/symbols/${tokenInput}/upload` : null
  );

  function generateToken() {
    tokenInput = crypto.randomUUID();
  }
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

  <form method="POST" action="?/save" use:enhance class="space-y-4 mb-6">
    <div>
      <label for="product_token" class="block text-[13px] font-medium mb-1">Token</label>
      <p class="text-[12px] text-ink-muted dark:text-ink-mutedDark mb-2">
        Set a custom token value, or use Generate to create a random one. Click Save to apply.
      </p>
      <div class="flex gap-2">
        <input
          id="product_token"
          name="product_token"
          type="text"
          bind:value={tokenInput}
          placeholder="(not set)"
          spellcheck="false"
          autocomplete="off"
          class="flex-1 rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 font-mono text-[12px] outline-none focus:ring-1 focus:ring-accent"
        />
        <button
          type="button"
          onclick={generateToken}
          class="rounded-md border border-line dark:border-line-dark px-3 py-1.5 text-[13px] font-medium hover:bg-surface-panel dark:hover:bg-surface-panelDark"
        >Generate</button>
        <button
          type="submit"
          class="rounded-md bg-accent px-4 py-1.5 text-[13px] font-medium text-white"
        >Save</button>
      </div>
    </div>
  </form>

  {#if hasToken}
    <div class="rounded-md border border-line dark:border-line-dark overflow-hidden">
      <div class="bg-surface-panel dark:bg-surface-panelDark px-4 py-3 border-b border-line dark:border-line-dark">
        <div class="text-[13px] font-medium">Upload URLs</div>
        <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
          Use these endpoints in your application to upload crash reports and debug symbols.
        </div>
      </div>
      <div class="px-4 py-3 bg-surface dark:bg-surface-dark space-y-3">
        <div>
          <div class="text-[12px] font-medium mb-1">Minidump upload</div>
          {#if minidumpUrl}
            <code class="font-mono text-[11px] break-all select-all text-ink-muted dark:text-ink-mutedDark">{minidumpUrl}</code>
          {:else}
            <code class="font-mono text-[11px] break-all text-ink-muted dark:text-ink-mutedDark">/api/minidump/{tokenInput}/upload</code>
            {#if !base}
              <p class="text-[11px] text-ink-muted dark:text-ink-mutedDark mt-1">Set <code class="font-mono">GUARDRAIL_INGESTION_URL</code> to see the full URL.</p>
            {/if}
          {/if}
        </div>
        <div>
          <div class="text-[12px] font-medium mb-1">Symbol upload</div>
          {#if symbolsUrl}
            <code class="font-mono text-[11px] break-all select-all text-ink-muted dark:text-ink-mutedDark">{symbolsUrl}</code>
          {:else}
            <code class="font-mono text-[11px] break-all text-ink-muted dark:text-ink-mutedDark">/api/symbols/{tokenInput}/upload</code>
            {#if !base}
              <p class="text-[11px] text-ink-muted dark:text-ink-mutedDark mt-1">Set <code class="font-mono">GUARDRAIL_INGESTION_URL</code> to see the full URL.</p>
            {/if}
          {/if}
        </div>
      </div>
    </div>
  {:else}
    <div class="rounded-md border border-line dark:border-line-dark px-4 py-3 bg-surface dark:bg-surface-dark">
      <p class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
        No token set — upload URLs will be shown here once a token is saved.
      </p>
    </div>
  {/if}
</div>
