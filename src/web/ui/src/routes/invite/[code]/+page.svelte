<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData, ActionData } from './$types';

  let { data, form }: { data: PageData; form: ActionData } = $props();
</script>

<svelte:head><title>Create account · Guardrail</title></svelte:head>

<div class="flex min-h-screen items-center justify-center bg-surface dark:bg-surface-dark px-6 py-12 text-ink dark:text-ink-dark">
  <div class="w-full max-w-[400px]">
    <div class="mb-8 flex items-center gap-2.5">
      <div class="h-[26px] w-[26px] rounded-md bg-ink dark:bg-ink-dark"></div>
      <div class="font-sans text-[15px] font-semibold tracking-[-0.01em]">Guardrail</div>
    </div>

    <h1 class="mb-1 text-[22px] font-semibold tracking-[-0.01em]">Create your account</h1>
    <p class="mb-6 text-[13px] text-ink-muted dark:text-ink-mutedDark">
      Fill in your details to accept this invitation.
    </p>

    {#if form?.error}
      <div class="mb-4 rounded-md border border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950 px-3 py-2 text-[13px] text-red-700 dark:text-red-400">
        {form.error}
      </div>
    {/if}

    <form method="POST" use:enhance class="flex flex-col gap-4">
      <div class="flex flex-col gap-1">
        <label for="username" class="text-[13px] font-medium">Username</label>
        <input
          id="username"
          name="username"
          type="text"
          required
          autofocus
          autocomplete="username"
          value={form?.username ?? ''}
          class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-2 text-[13px] outline-none focus:ring-2 focus:ring-ink dark:focus:ring-ink-dark"
        />
      </div>

      <div class="flex flex-col gap-1">
        <label for="email" class="text-[13px] font-medium">Email</label>
        <input
          id="email"
          name="email"
          type="email"
          required
          autocomplete="email"
          value={form?.email ?? ''}
          class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-2 text-[13px] outline-none focus:ring-2 focus:ring-ink dark:focus:ring-ink-dark"
        />
      </div>

      <div class="flex flex-col gap-1">
        <label for="first_name" class="text-[13px] font-medium">
          First name <span class="text-ink-muted dark:text-ink-mutedDark font-normal">(optional)</span>
        </label>
        <input
          id="first_name"
          name="first_name"
          type="text"
          autocomplete="given-name"
          value={form?.first_name ?? ''}
          class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-2 text-[13px] outline-none focus:ring-2 focus:ring-ink dark:focus:ring-ink-dark"
        />
      </div>

      <div class="flex flex-col gap-1">
        <label for="last_name" class="text-[13px] font-medium">
          Last name <span class="text-ink-muted dark:text-ink-mutedDark font-normal">(optional)</span>
        </label>
        <input
          id="last_name"
          name="last_name"
          type="text"
          autocomplete="family-name"
          value={form?.last_name ?? ''}
          class="rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-2 text-[13px] outline-none focus:ring-2 focus:ring-ink dark:focus:ring-ink-dark"
        />
      </div>

      <button
        type="submit"
        class="mt-2 rounded-md bg-ink dark:bg-ink-dark px-3 py-2 text-[13px] font-medium text-surface dark:text-surface-dark"
      >
        Continue
      </button>
    </form>
  </div>
</div>
