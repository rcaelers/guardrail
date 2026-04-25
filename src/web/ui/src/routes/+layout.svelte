<script lang="ts">
  import '../app.css';
  import { browser } from '$app/environment';
  import type { LayoutData } from './$types';

  let { data, children }: { data: LayoutData; children: any } = $props();

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

{#if data.realUser}
  <div class="fixed left-0 right-0 top-0 z-50 flex h-8 items-center justify-between bg-amber-400 px-4 text-[12px] font-medium text-amber-950 shadow-sm">
    <span>
      Impersonating <strong>{data.user?.name ?? data.user?.id}</strong>
      <span class="ml-1 font-normal opacity-75">— acting as this user across the entire app</span>
    </span>
    <form method="POST" action="/auth/impersonate/stop">
      <button
        type="submit"
        class="rounded bg-amber-950/15 px-2.5 py-0.5 text-[11px] font-semibold hover:bg-amber-950/25"
      >Stop impersonating</button>
    </form>
  </div>
{/if}

<div
  class="min-h-screen bg-surface dark:bg-surface-dark text-ink dark:text-ink-dark"
  class:pt-8={!!data.realUser}
>
  {@render children?.()}
</div>
