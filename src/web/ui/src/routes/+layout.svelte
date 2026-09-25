<script lang="ts">
  import '../app.css';
  import { browser } from '$app/environment';
  import { onNavigate } from '$app/navigation';
  import { installCsrfFetch } from '$lib/csrf';
  import { beginBusy, systemBusy } from '$lib/system-busy';
  import type { LayoutData } from './$types';

  // Attach the CSRF header to same-origin mutating fetches app-wide.
  installCsrfFetch();

  // Keep the indicator active for the complete navigation lifecycle. The
  // fetch wrapper covers enhanced form actions and direct API requests.
  onNavigate(() => beginBusy());

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

{#if $systemBusy}
  <div
    role="status"
    aria-live="polite"
    class="fixed bottom-4 right-4 z-[100] flex items-center gap-2 rounded-lg border border-line bg-surface/95 px-3 py-2 text-[12.5px] font-medium shadow-xl backdrop-blur dark:border-line-dark dark:bg-surface-dark/95"
  >
    <span class="h-4 w-4 animate-spin rounded-full border-2 border-accent/25 border-t-accent"></span>
    Working…
  </div>
{/if}

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
