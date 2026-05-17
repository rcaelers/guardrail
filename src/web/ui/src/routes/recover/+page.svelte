<script lang="ts">
  import { enhance } from '$app/forms';
  import type { ActionData } from './$types';

  let { form }: { form: ActionData } = $props();
</script>

<svelte:head><title>Recover access · Guardrail</title></svelte:head>

<div class="flex min-h-screen items-center justify-center bg-surface dark:bg-surface-dark px-6 py-12 text-ink dark:text-ink-dark">
  <div class="w-full max-w-[400px]">
    <div class="mb-8 flex items-center gap-2.5">
      <div class="h-[26px] w-[26px] rounded-md bg-ink dark:bg-ink-dark"></div>
      <div class="font-sans text-[15px] font-semibold tracking-[-0.01em]">
        Guardrail <span class="font-normal text-ink-muted dark:text-ink-mutedDark">/ Crashdumps</span>
      </div>
    </div>

    {#if form?.ok}
      {#if form?.login_url}
        <h1 class="mb-1 text-[22px] font-semibold tracking-[-0.01em]">Your login link</h1>
        <p class="mb-4 text-[13px] text-ink-muted dark:text-ink-mutedDark">
          No email sender is configured. Use this link to sign in — it expires in 15 minutes.
        </p>
        <a
          href={form.login_url}
          class="block w-full rounded-md bg-ink dark:bg-ink-dark px-3 py-2 text-center text-[13px] font-medium text-surface dark:text-surface-dark mb-4"
        >Open login link</a>
        <div class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-2 text-[12px] text-ink-muted dark:text-ink-mutedDark break-all font-mono">
          {form.login_url}
        </div>
      {:else}
        <h1 class="mb-1 text-[22px] font-semibold tracking-[-0.01em]">Check your email</h1>
        <p class="mb-6 text-[13px] text-ink-muted dark:text-ink-mutedDark">
          If that address is registered you'll receive a one-time login link shortly.
          The link expires in 15 minutes.
        </p>
      {/if}
      <a
        href="/auth/login/start"
        data-sveltekit-reload
        class="mt-4 block w-full rounded-md border border-line dark:border-line-dark px-3 py-2 text-center text-[13px] font-medium"
      >Back to sign in</a>
    {:else}
      <h1 class="mb-1 text-[22px] font-semibold tracking-[-0.01em]">Recover access</h1>
      <p class="mb-6 text-[13px] text-ink-muted dark:text-ink-mutedDark">
        Enter the email address on your account and we'll send you a one-time login link.
      </p>

      {#if form?.error}
        <div class="mb-4 rounded-md border border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950 px-3 py-2 text-[13px] text-red-700 dark:text-red-400">
          {form.error}
        </div>
      {/if}

      <form method="POST" use:enhance class="space-y-4">
        <label class="block">
          <span class="mb-1 block text-[12px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark">
            Email
          </span>
          <input
            name="email"
            type="email"
            required
            autocomplete="email"
            placeholder="you@example.com"
            class="w-full rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-2 text-[13px] outline-none focus:border-accent"
          />
        </label>

        <button
          type="submit"
          class="w-full rounded-md bg-ink dark:bg-ink-dark px-3 py-2 text-[13px] font-medium text-surface dark:text-surface-dark"
        >Send login link</button>
      </form>

      <p class="mt-6 text-center text-[12px] text-ink-muted dark:text-ink-mutedDark">
        Remember your passkey?
        <a href="/auth/login/start" data-sveltekit-reload class="text-accent hover:underline">Sign in</a>
      </p>
    {/if}
  </div>
</div>
