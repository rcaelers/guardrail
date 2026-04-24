<script lang="ts">
  import type { PageData } from './$types';
  let { data }: { data: PageData } = $props();
</script>

<svelte:head><title>Sign in · Guardrail</title></svelte:head>

<div class="flex min-h-screen items-center justify-center bg-surface dark:bg-surface-dark px-6 py-12 text-ink dark:text-ink-dark">
  <div class="w-full max-w-[400px]">
    <div class="mb-8 flex items-center gap-2.5">
      <div class="h-[26px] w-[26px] rounded-md bg-ink dark:bg-ink-dark"></div>
      <div class="font-sans text-[15px] font-semibold tracking-[-0.01em]">
        Guardrail <span class="font-normal text-ink-muted dark:text-ink-mutedDark">/ Crashdumps</span>
      </div>
    </div>

    <h1 class="mb-1 text-[22px] font-semibold tracking-[-0.01em]">Sign in failed</h1>
    <p class="mb-6 text-[13px] text-ink-muted dark:text-ink-mutedDark">
      An error occurred during authentication with Pocket ID.
    </p>

    {#if data.error}
      <div class="mb-6 rounded-md border border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950 px-3 py-2 text-[13px] text-red-700 dark:text-red-400">
        {data.error}
      </div>
    {/if}

    <a
      href="/auth/login/start"
      class="block w-full rounded-md bg-ink dark:bg-ink-dark px-3 py-2 text-center text-[13px] font-medium text-surface dark:text-surface-dark"
    >Try again</a>
  </div>
</div>
