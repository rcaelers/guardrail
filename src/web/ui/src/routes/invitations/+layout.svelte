<script lang="ts">
  import { browser } from '$app/environment';
  import { page } from '$app/stores';
  import UserMenu from '$lib/components/UserMenu.svelte';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';
  import type { LayoutData } from './$types';

  let { data, children }: { data: LayoutData; children: any } = $props();

  const isAdmin = $derived(data.user?.isAdmin ?? false);

  let dark = $state(false);
  $effect(() => { if (browser) dark = localStorage.getItem('gr-dark') === '1'; });
  $effect(() => {
    if (!browser) return;
    document.documentElement.classList.toggle('dark', dark);
    localStorage.setItem('gr-dark', dark ? '1' : '0');
  });

  const NAV: Array<[string, string, string]> = [
    ['users', 'Users', '/admin/users'],
    ['products', 'Products', '/admin/products'],
    ['invitations', 'Invitations', '/invitations'],
    ['tokens', 'API tokens', '/admin/tokens'],
    ['email', 'Email', '/admin/email'],
  ];

  const current = $derived.by(() => {
    const segs = $page.url.pathname.split('/');
    if (segs[1] === 'invitations') return 'invitations';
    return segs[2] ?? '';
  });
</script>

<div class="flex h-screen w-screen flex-col overflow-hidden">
  <header class="flex h-[52px] shrink-0 items-center gap-4 border-b border-line dark:border-line-dark px-4">
    <a href="/" class="flex items-center gap-2.5">
      <div class="h-[22px] w-[22px] rounded-md bg-ink dark:bg-ink-dark"></div>
      <div class="font-sans text-[14px] font-semibold tracking-[-0.01em]">
        Guardrail
        {#if isAdmin}
          <span class="font-normal text-ink-muted dark:text-ink-mutedDark">/ Admin console</span>
        {:else}
          <span class="font-normal text-ink-muted dark:text-ink-mutedDark">/ Invitations</span>
        {/if}
      </div>
    </a>
    <span class="flex-1"></span>
    <a href="/" class="rounded-md border border-line dark:border-line-dark bg-transparent px-2.5 py-1 text-[12px] text-ink-muted dark:text-ink-mutedDark">← Back to app</a>
    <ThemeToggle {dark} setDark={(v) => (dark = v)} />
    {#if data.user}<UserMenu user={data.user} isAdmin={data.user.isAdmin} />{/if}
  </header>

  <div class="flex min-h-0 flex-1">
    {#if isAdmin}
      <aside class="w-[200px] shrink-0 border-r border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-4">
        <div class="mb-2 px-2 text-[11px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">
          Admin
        </div>
        <nav class="space-y-0.5 text-[13px]">
          {#each NAV as [slug, label, href]}
            {@const active = current === slug}
            <a
              {href}
              class="block rounded-md px-2.5 py-1.5"
              class:bg-accent-soft={active}
              class:dark:bg-accent-softDark={active}
              class:text-accent={active}
              class:text-ink={!active}
              class:dark:text-ink-dark={!active}
            >{label}</a>
          {/each}
        </nav>
      </aside>
    {/if}

    <div class="min-h-0 flex-1 overflow-auto px-8 py-6">
      {@render children?.()}
    </div>
  </div>
</div>
