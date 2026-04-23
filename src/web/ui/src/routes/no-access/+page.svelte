<script lang="ts">
  import UserMenu from '$lib/components/UserMenu.svelte';
  import type { PageData } from './$types';
  let { data }: { data: PageData } = $props();
</script>

<svelte:head><title>No access · Guardrail</title></svelte:head>

<div class="flex min-h-screen flex-col">
  <header class="flex h-[52px] shrink-0 items-center gap-5 border-b border-line dark:border-line-dark px-5">
    <div class="flex items-center gap-2.5">
      <div class="h-[22px] w-[22px] rounded-md bg-ink dark:bg-ink-dark"></div>
      <div class="font-sans text-[14px] font-semibold tracking-[-0.01em]">
        Guardrail <span class="font-normal text-ink-muted dark:text-ink-mutedDark">/ Crashdumps</span>
      </div>
    </div>
    <span class="flex-1"></span>
    {#if data.user}<UserMenu user={data.user} isAdmin={data.user.isAdmin} />{/if}
  </header>

  <main class="flex flex-1 items-center justify-center px-6 py-16">
    <div class="max-w-[420px] text-center">
      <div class="mb-3 text-[22px] font-semibold tracking-[-0.01em]">No products yet</div>
      <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
        You're signed in, but no product has been shared with your account.
        Ask an administrator to grant you access.
      </p>
    </div>
  </main>
</div>
