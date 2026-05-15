<script lang="ts">
  import type { User } from '$lib/adapters/types';

  let { user, isAdmin = false }: { user: User; isAdmin?: boolean } = $props();

  let open = $state(false);
  let btn: HTMLButtonElement | undefined;
  function close() { open = false; }
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
    onclick={() => (open = !open)}
    class="flex h-8 w-8 items-center justify-center rounded-full bg-accent-soft dark:bg-accent-softDark text-[11px] font-semibold text-accent hover:ring-2 hover:ring-line dark:hover:ring-line-dark"
    aria-label="Account"
  >
    {user.avatar}
  </button>

  {#if open}
    <div class="absolute right-0 top-full z-40 mt-1 w-[220px] rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark shadow-lg">
      <div class="border-b border-line dark:border-line-dark px-3 py-2">
        <div class="text-[13px] font-medium">{user.name}</div>
        <div class="text-[11px] text-ink-muted dark:text-ink-mutedDark">{user.email}</div>
      </div>
      <div class="py-1 text-[13px]">
        {#if isAdmin}
          <a href="/admin" class="block px-3 py-1.5 hover:bg-surface-panel dark:hover:bg-surface-panelDark">Admin console</a>
        {/if}
        <form method="POST" action="/logout">
          <button
            type="submit"
            class="block w-full cursor-pointer px-3 py-1.5 text-left hover:bg-surface-panel dark:hover:bg-surface-panelDark"
          >Sign out</button>
        </form>
      </div>
    </div>
  {/if}
</div>
