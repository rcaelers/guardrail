<script lang="ts">
  import { page } from '$app/stores';

  const NAV: Array<[string, string]> = [
    ['users', 'Users'],
    ['products', 'Products'],
    ['invitations', 'Invitations'],
    ['tokens', 'API tokens']
  ];
</script>

<span class="h-5 w-px bg-line dark:bg-line-dark"></span>
<nav class="flex gap-0.5 text-[13px]">
  {#each NAV as [slug, label]}
    {@const href = slug === 'invitations' ? '/invitations' : `/admin/${slug}`}
    {@const active = slug === 'invitations' ? $page.url.pathname.startsWith('/invitations') : $page.url.pathname.startsWith(`/admin/${slug}`)}
    <a
      {href}
      class="rounded-md px-2.5 py-1.5"
      class:bg-accent-soft={active}
      class:dark:bg-accent-softDark={active}
      class:text-accent={active}
      class:text-ink-muted={!active}
      class:dark:text-ink-mutedDark={!active}
    >{label}</a>
  {/each}
</nav>
