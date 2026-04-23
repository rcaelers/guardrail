<script lang="ts">
  import { page } from '$app/stores';
  import { browser } from '$app/environment';
  import UserMenu from '$lib/components/UserMenu.svelte';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';
  import type { LayoutData } from './$types';

  let { data, children }: { data: LayoutData; children: any } = $props();

  const current = $derived.by(() => {
    const segs = $page.url.pathname.split('/');
    return segs[2] ?? 'users';
  });

  const NAV: Array<[string, string]> = [
    ['users', 'Users'],
    ['products', 'Products']
  ];

  // Dark-mode toggle
  let dark = $state(false);
  $effect(() => {
    if (browser) dark = localStorage.getItem('gr-dark') === '1';
  });
  $effect(() => {
    if (!browser) return;
    document.documentElement.classList.toggle('dark', dark);
    localStorage.setItem('gr-dark', dark ? '1' : '0');
  });
</script>

<div class="flex h-screen w-screen flex-col overflow-hidden">
  <!-- Top bar (admin-flavored) -->
  <header class="flex h-[52px] shrink-0 items-center gap-4 border-b border-line dark:border-line-dark px-4">
    <a href="/" class="flex items-center gap-2.5">
      <div class="h-[22px] w-[22px] rounded-md bg-ink dark:bg-ink-dark"></div>
      <div class="font-sans text-[14px] font-semibold tracking-[-0.01em]">
        Guardrail <span class="font-normal text-ink-muted dark:text-ink-mutedDark">/ Admin console</span>
      </div>
    </a>
    <span class="h-5 w-px bg-line dark:bg-line-dark"></span>
    <nav class="flex gap-0.5 text-[13px]">
      {#each NAV as [slug, label]}
        {@const active = current === slug}
        <a
          href={`/admin/${slug}`}
          class="rounded-md px-2.5 py-1.5"
          class:bg-accent-soft={active}
          class:dark:bg-accent-softDark={active}
          class:text-accent={active}
          class:text-ink-muted={!active}
          class:dark:text-ink-mutedDark={!active}
        >{label}</a>
      {/each}
    </nav>
    <span class="flex-1"></span>
    <a href="/" class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[12px] text-ink-muted dark:text-ink-mutedDark">← Back to app</a>
    <ThemeToggle {dark} setDark={(v) => (dark = v)} />
    {#if data.user}<UserMenu user={data.user} isAdmin={data.user.isAdmin} />{/if}
  </header>

  <div class="min-h-0 flex-1 overflow-auto px-8 py-6">
    {@render children?.()}
  </div>
</div>
