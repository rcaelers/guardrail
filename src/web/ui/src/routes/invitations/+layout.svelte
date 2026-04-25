<script lang="ts">
  import { browser } from '$app/environment';
  import UserMenu from '$lib/components/UserMenu.svelte';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';
  import type { LayoutData } from './$types';

  let { data, children }: { data: LayoutData; children: any } = $props();

  let dark = $state(false);
  $effect(() => { if (browser) dark = localStorage.getItem('gr-dark') === '1'; });
  $effect(() => {
    if (!browser) return;
    document.documentElement.classList.toggle('dark', dark);
    localStorage.setItem('gr-dark', dark ? '1' : '0');
  });
</script>

<div class="flex h-screen w-screen flex-col overflow-hidden">
  <header class="flex h-[52px] shrink-0 items-center gap-4 border-b border-line dark:border-line-dark px-4">
    <a href="/" class="flex items-center gap-2.5">
      <div class="h-[22px] w-[22px] rounded-md bg-ink dark:bg-ink-dark"></div>
      <div class="font-sans text-[14px] font-semibold tracking-[-0.01em]">
        Guardrail <span class="font-normal text-ink-muted dark:text-ink-mutedDark">/ Invitations</span>
      </div>
    </a>
    <span class="flex-1"></span>
    <a href="/" class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[12px] text-ink-muted dark:text-ink-mutedDark">← Back to app</a>
    <ThemeToggle {dark} setDark={(v) => (dark = v)} />
    {#if data.user}<UserMenu user={data.user} isAdmin={data.user.isAdmin} />{/if}
  </header>
  <div class="min-h-0 flex-1 overflow-auto px-8 py-6">
    {@render children?.()}
  </div>
</div>
