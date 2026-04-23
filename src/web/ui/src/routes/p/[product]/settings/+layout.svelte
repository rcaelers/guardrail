<script lang="ts">
  import { page } from '$app/stores';
  import type { LayoutData } from './$types';
  let { data, children }: { data: LayoutData; children: any } = $props();

  const productId = $derived($page.params.product);
  const current = $derived.by(() => {
    const segs = $page.url.pathname.split('/');
    return segs[5] ?? 'members';
  });

  const NAV: Array<[string, string]> = [
    ['members', 'Members'],
    ['danger', 'Danger zone']
  ];
</script>

<div class="flex h-full min-h-0">
  <!-- Settings side-nav -->
  <aside class="w-[200px] shrink-0 border-r border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-4">
    <div class="mb-2 px-2 text-[11px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">
      Settings
    </div>
    <nav class="space-y-0.5 text-[13px]">
      {#each NAV as [slug, label]}
        {@const active = current === slug}
        <a
          href={`/p/${productId}/settings/${slug}`}
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

  <div class="min-h-0 flex-1 overflow-auto px-8 py-6">
    {@render children?.()}
  </div>
</div>
