<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData, ActionData } from './$types';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  let email = $state<string>('');
  let submitting = $state(false);

  $effect(() => {
    if (form?.email) email = form.email;
  });
</script>

<svelte:head><title>Sign in · Guardrail</title></svelte:head>

<div class="flex min-h-screen items-center justify-center bg-surface dark:bg-surface-dark px-6 py-12 text-ink dark:text-ink-dark">
  <div class="w-full max-w-[400px]">
    <!-- logo + title -->
    <div class="mb-8 flex items-center gap-2.5">
      <div class="h-[26px] w-[26px] rounded-md bg-ink dark:bg-ink-dark"></div>
      <div class="font-sans text-[15px] font-semibold tracking-[-0.01em]">
        Guardrail <span class="font-normal text-ink-muted dark:text-ink-mutedDark">/ Crashdumps</span>
      </div>
    </div>

    <h1 class="mb-1 text-[22px] font-semibold tracking-[-0.01em]">Sign in</h1>
    <p class="mb-6 text-[13px] text-ink-muted dark:text-ink-mutedDark">
      Mock auth — pick any seeded account below.
    </p>

    <form
      method="POST"
      use:enhance={() => {
        submitting = true;
        return async ({ update }) => {
          await update();
          submitting = false;
        };
      }}
      class="space-y-3"
    >
      <label class="block text-[12px] font-medium text-ink-muted dark:text-ink-mutedDark" for="email">Email</label>
      <input
        id="email"
        name="email"
        type="email"
        required
        autocomplete="email"
        bind:value={email}
        class="w-full rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-2 text-[13px] outline-none focus:border-accent"
        placeholder="you@studio.co"
      />

      {#if form?.error}
        <p class="text-[12px] text-red-600 dark:text-red-400">{form.error}</p>
      {/if}

      <button
        type="submit"
        disabled={submitting}
        class="w-full rounded-md bg-ink dark:bg-ink-dark px-3 py-2 text-[13px] font-medium text-surface dark:text-surface-dark disabled:opacity-60"
      >
        {submitting ? 'Signing in…' : 'Sign in'}
      </button>
    </form>

    <div class="mt-6">
      <div class="mb-2 text-[11px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">Suggested</div>
      <div class="divide-y divide-line dark:divide-line-dark rounded-md border border-line dark:border-line-dark">
        {#each data.suggestions as s}
          <button
            type="button"
            onclick={() => (email = s.email)}
            class="flex w-full items-center justify-between px-3 py-2 text-left text-[13px] hover:bg-surface-panel dark:hover:bg-surface-panelDark"
          >
            <span>
              <span class="font-medium">{s.name}</span>
              <span class="ml-2 text-ink-muted dark:text-ink-mutedDark">{s.email}</span>
            </span>
            {#if s.isAdmin}
              <span class="rounded bg-accent-soft dark:bg-accent-softDark px-1.5 py-0.5 text-[10px] font-medium text-accent">admin</span>
            {/if}
          </button>
        {/each}
      </div>
    </div>
  </div>
</div>
