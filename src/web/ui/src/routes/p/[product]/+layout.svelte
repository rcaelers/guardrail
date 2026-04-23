<script lang="ts">
  import type { LayoutData } from './$types';
  import { page } from '$app/stores';
  import { browser } from '$app/environment';
  import ProductSwitcher from '$lib/components/ProductSwitcher.svelte';
  import UserMenu from '$lib/components/UserMenu.svelte';
  import RoleBadge from '$lib/components/RoleBadge.svelte';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';

  let { data, children }: { data: LayoutData; children: any } = $props();

  // Derive the active tab from the URL so switching products preserves it.
  const tab = $derived.by(() => {
    const segments = $page.url.pathname.split('/'); // ['', 'p', pid, tab, ...]
    return segments[3] ?? 'crashes';
  });

  // Dark-mode toggle — mirrors original layout
  let dark = $state(false);
  $effect(() => {
    if (browser) dark = localStorage.getItem('gr-dark') === '1';
  });
  $effect(() => {
    if (!browser) return;
    document.documentElement.classList.toggle('dark', dark);
    localStorage.setItem('gr-dark', dark ? '1' : '0');
  });

  const TABS: Array<[string, string]> = [
    ['crashes', 'Crashes'],
    ['symbols', 'Symbols'],
    ['settings', 'Settings']
  ];
</script>

<div class="flex h-screen w-screen flex-col overflow-hidden">
  <!-- Top bar -->
  <header class="flex h-[52px] shrink-0 items-center gap-4 border-b border-line dark:border-line-dark px-4">
    <div class="flex items-center gap-2.5">
      <a href="/" class="flex items-center gap-2.5">
        <div class="h-[22px] w-[22px] rounded-md bg-ink dark:bg-ink-dark"></div>
        <div class="font-sans text-[14px] font-semibold tracking-[-0.01em]">
          Guardrail <span class="font-normal text-ink-muted dark:text-ink-mutedDark">/ Crashdumps</span>
        </div>
      </a>
    </div>

    <span class="h-5 w-px bg-line dark:bg-line-dark"></span>

    <ProductSwitcher products={data.products ?? []} currentId={data.product.id} {tab} />
    <RoleBadge role={data.role} />

    <!-- Tabs -->
    <nav class="ml-3 flex gap-0.5 text-[13px]">
      {#each TABS as [slug, label]}
        {@const active = tab === slug || (slug === 'settings' && tab === 'settings')}
        <a
          href={`/p/${data.product.id}/${slug}`}
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
    <ThemeToggle {dark} setDark={(v) => (dark = v)} />
    {#if data.user}<UserMenu user={data.user} isAdmin={data.user.isAdmin} />{/if}
  </header>

  <div class="min-h-0 flex-1">
    {@render children?.()}
  </div>
</div>
