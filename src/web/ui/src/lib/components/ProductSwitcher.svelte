<script lang="ts">
  import type { Product } from '$lib/adapters/types';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';

  let {
    products,
    currentId,
    tab = 'crashes'
  }: {
    products: Product[];
    currentId: string | null;
    tab?: string;
  } = $props();

  let open = $state(false);
  let btn: HTMLButtonElement | undefined;

  function close() { open = false; }
  function toggle() { open = !open; }

  const current = $derived(products.find((p) => p.id === currentId) ?? null);

  function pick(p: Product) {
    close();
    // Preserve the current tab segment when switching products
    goto(`/p/${p.id}/${tab}`);
  }
</script>

<svelte:window
  onclick={(e) => {
    if (!open) return;
    if (btn && btn.contains(e.target as Node)) return;
    close();
  }}
  onkeydown={(e) => { if (e.key === 'Escape') close(); }}
/>

<div class="relative">
  <button
    bind:this={btn}
    type="button"
    onclick={toggle}
    class="flex items-center gap-2 rounded-md px-2 py-1.5 hover:bg-surface-panel dark:hover:bg-surface-panelDark"
  >
    {#if current}
      <span
        class="inline-block h-[14px] w-[14px] shrink-0 rounded-[3px]"
        style:background={current.color}
      ></span>
      <span class="text-[13px] font-medium">{current.name}</span>
    {:else}
      <span class="text-[13px] text-ink-muted dark:text-ink-mutedDark">No product</span>
    {/if}
    <svg width="10" height="10" viewBox="0 0 10 10" class="opacity-60"><path d="M2 3.5l3 3 3-3" stroke="currentColor" fill="none" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
  </button>

  {#if open}
    <div class="absolute left-0 top-full z-40 mt-1 w-[240px] rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark shadow-lg">
      <div class="border-b border-line dark:border-line-dark px-3 py-2 text-[11px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">
        Your products
      </div>
      <div class="py-1">
        {#each products as p}
          <button
            type="button"
            onclick={() => pick(p)}
            class="flex w-full items-center gap-2 px-3 py-1.5 text-left text-[13px] hover:bg-surface-panel dark:hover:bg-surface-panelDark"
            class:bg-accent-soft={p.id === currentId}
            class:dark:bg-accent-softDark={p.id === currentId}
          >
            <span class="inline-block h-[12px] w-[12px] rounded-[3px]" style:background={p.color}></span>
            <span class="flex-1">{p.name}</span>
            {#if p.id === currentId}
              <span class="text-accent">✓</span>
            {/if}
          </button>
        {:else}
          <div class="px-3 py-3 text-[12px] text-ink-muted dark:text-ink-mutedDark">
            You don't have access to any products yet.
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>
