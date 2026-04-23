<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData, ActionData } from './$types';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  const canDelete = $derived(data.role === 'maintainer' || data.user?.isAdmin === true);

  let confirmText = $state('');
</script>

<div class="mx-auto max-w-[720px]">
  <div class="mb-6">
    <h1 class="mb-1 text-[20px] font-semibold tracking-[-0.01em]">Danger zone</h1>
    <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
      Destructive actions that cannot be undone.
    </p>
  </div>

  <div class="overflow-hidden rounded-md border border-red-300/70 dark:border-red-900/60">
    <div class="flex items-start gap-4 px-5 py-4">
      <div class="flex-1">
        <div class="mb-1 text-[14px] font-semibold">Delete {data.product.name}</div>
        <p class="text-[12.5px] text-ink-muted dark:text-ink-mutedDark">
          Permanently removes the product, its crash groups, symbol store, and all memberships.
          This cannot be undone.
        </p>
      </div>
    </div>
    {#if canDelete}
      <form
        method="POST"
        action="?/delete"
        use:enhance
        class="border-t border-red-300/70 dark:border-red-900/60 bg-red-50/60 dark:bg-red-950/30 px-5 py-4"
      >
        <label class="block">
          <span class="mb-1.5 block text-[12px] text-ink-muted dark:text-ink-mutedDark">
            Type <span class="font-mono font-medium text-ink dark:text-ink-dark">{data.product.name}</span> to confirm.
          </span>
          <input
            name="confirm"
            bind:value={confirmText}
            class="w-full rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px] outline-none"
          />
        </label>
        {#if form?.error}
          <p class="mt-2 text-[12px] text-red-600 dark:text-red-400">{form.error}</p>
        {/if}
        <button
          type="submit"
          disabled={confirmText !== data.product.name}
          class="mt-3 rounded-md bg-red-600 px-3 py-1.5 text-[13px] font-medium text-white disabled:opacity-50"
        >
          Delete product
        </button>
      </form>
    {:else}
      <div class="border-t border-red-300/70 dark:border-red-900/60 bg-red-50/60 dark:bg-red-950/30 px-5 py-3 text-[12.5px] text-ink-muted dark:text-ink-mutedDark">
        Only maintainers or administrators can delete this product.
      </div>
    {/if}
  </div>
</div>
