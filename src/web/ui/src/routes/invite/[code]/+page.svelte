<script lang="ts">
  import { enhance } from '$app/forms';
  import { browser } from '$app/environment';
  import type { PageData, ActionData } from './$types';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  // setup_url from a successful action result (stored locally so we don't need
  // to call update() and risk resetting the form state).
  let actionSetupUrl: string | null = $state(null);

  let popup: Window | null = $state(null);
  let popupBlocked = $state(false);

  // The active setup URL — from the action, the server load, or null.
  let setupUrl = $derived(
    actionSetupUrl ?? (form as { setup_url?: string } | null)?.setup_url ?? data.setup_url ?? null
  );

  function startPolling(p: Window) {
    const timer = setInterval(() => {
      if (p.closed) {
        clearInterval(timer);
        if (popup === p) popup = null;
        window.location.href = '/auth/login/start';
      }
    }, 500);
  }

  function buildPopupUrl(url: string): string {
    // Pass our origin so the popup can target the postMessage correctly,
    // even when the popup is served from a different domain (e.g. auth.workrave.org).
    try {
      const u = new URL(url);
      u.searchParams.set('origin', window.location.origin);
      return u.toString();
    } catch {
      return url;
    }
  }

  function openPopup(url: string) {
    if (popup && !popup.closed) { popup.focus(); return; }
    const p = window.open(buildPopupUrl(url), 'guardrail-setup', 'popup,width=520,height=640,left=200,top=100');
    if (!p) {
      popupBlocked = true;
      return;
    }
    popup = p;
    popupBlocked = false;
    startPolling(p);
  }

  // postMessage from the popup (auto-login page or popup-done page).
  // We check e.source === popup so we accept it from any domain the popup is served at.
  $effect(() => {
    if (!browser) return;
    function handleMessage(e: MessageEvent) {
      if (e.source !== popup) return;
      if ((e.data as { type?: string })?.type === 'setup-complete') {
        popup?.close();
        popup = null;
        window.location.href = '/auth/login/start';
      }
    }
    window.addEventListener('message', handleMessage);
    return () => window.removeEventListener('message', handleMessage);
  });

  // Enhance callback shared by both forms that need to open a popup.
  function popupEnhance() {
    // Open a blank popup synchronously within the user gesture so the browser
    // allows it. We navigate it once the server responds with the URL.
    let p: Window | null = null;
    if (browser) {
      p = window.open('about:blank', 'guardrail-setup', 'popup,width=520,height=640,left=200,top=100');
      if (!p) popupBlocked = true;
    }

    return async ({ result, update }: { result: import('@sveltejs/kit').ActionResult; update: () => Promise<void> }) => {
      if (result.type === 'success') {
        const url = (result.data as Record<string, unknown> | undefined)?.setup_url as string | undefined;
        if (url) {
          actionSetupUrl = url;
          if (p && !p.closed) {
            p.location.href = buildPopupUrl(url);
            popup = p;
            startPolling(p);
            return; // don't call update(); we handle state ourselves
          } else {
            popupBlocked = true;
            return;
          }
        }
      }
      p?.close();
      popup = null;
      await update();
    };
  }
</script>

<svelte:head><title>Create account · Guardrail</title></svelte:head>

<div class="flex min-h-screen items-center justify-center bg-surface dark:bg-surface-dark px-6 py-12 text-ink dark:text-ink-dark">
  <div class="w-full max-w-[400px]">
    <div class="mb-8 flex items-center gap-2.5">
      <div class="h-[26px] w-[26px] rounded-md bg-ink dark:bg-ink-dark"></div>
      <div class="font-sans text-[15px] font-semibold tracking-[-0.01em]">Guardrail</div>
    </div>

    {#if setupUrl}
      <!-- ── Popup sign-in phase ──────────────────────────────────────────── -->
      <h1 class="mb-1 text-[22px] font-semibold tracking-[-0.01em]">Signing you in…</h1>
      <p class="mb-6 text-[13px] text-ink-muted dark:text-ink-mutedDark">
        {#if popup && !popup.closed}
          Completing sign-in in the popup window.
        {:else}
          Click the button below to open the sign-in window.
        {/if}
      </p>

      {#if popupBlocked || !popup || popup.closed}
        <button
          type="button"
          onclick={() => openPopup(setupUrl!)}
          class="w-full rounded-md bg-ink dark:bg-ink-dark px-3 py-2 text-[13px] font-medium text-surface dark:text-surface-dark"
        >
          Open sign-in window
        </button>
        {#if popupBlocked}
          <p class="mt-3 text-[12px] text-ink-muted dark:text-ink-mutedDark">
            Your browser blocked the popup. Click the button above to open it manually.
          </p>
        {/if}
      {:else}
        <div class="flex items-center gap-2 text-[13px] text-ink-muted dark:text-ink-mutedDark">
          <svg class="h-4 w-4 animate-spin" viewBox="0 0 24 24" fill="none">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/>
          </svg>
          Signing in…
        </div>
        <button
          type="button"
          onclick={() => openPopup(setupUrl!)}
          class="mt-4 text-[13px] text-ink-muted dark:text-ink-mutedDark underline"
        >
          Reopen window
        </button>
      {/if}

    {:else if data.needs_refresh}
      <!-- ── Returning user: account exists, open popup to sign in ──────── -->
      <h1 class="mb-1 text-[22px] font-semibold tracking-[-0.01em]">Continue sign-in</h1>
      <p class="mb-6 text-[13px] text-ink-muted dark:text-ink-mutedDark">
        Your account is ready. Click the button below to sign in.
      </p>

      {#if (form as { error?: string } | null)?.error}
        <div class="mb-4 rounded-md border border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950 px-3 py-2 text-[13px] text-red-700 dark:text-red-400">
          {(form as { error: string }).error}
        </div>
      {/if}

      <form method="POST" action="?/refresh" use:enhance={popupEnhance}>
        <button
          type="submit"
          class="w-full rounded-md bg-ink dark:bg-ink-dark px-3 py-2 text-[13px] font-medium text-surface dark:text-surface-dark"
        >
          Sign in
        </button>
      </form>

    {:else}
      <!-- ── Registration form ─────────────────────────────────────────── -->
      <h1 class="mb-1 text-[22px] font-semibold tracking-[-0.01em]">Create your account</h1>
      <p class="mb-6 text-[13px] text-ink-muted dark:text-ink-mutedDark">
        Fill in your details to accept this invitation.
      </p>

      {#if (form as { error?: string } | null)?.error}
        <div class="mb-4 rounded-md border border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950 px-3 py-2 text-[13px] text-red-700 dark:text-red-400">
          {(form as { error: string }).error}
        </div>
      {/if}

      <form method="POST" action="?/submit" use:enhance={popupEnhance} class="flex flex-col gap-4">
        <div class="flex flex-col gap-1">
          <label for="username" class="text-[13px] font-medium">Username</label>
          <input
            id="username"
            name="username"
            type="text"
            required
            autofocus
            autocomplete="username"
            value={(form as { username?: string } | null)?.username ?? ''}
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
            value={(form as { email?: string } | null)?.email ?? ''}
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
            value={(form as { first_name?: string } | null)?.first_name ?? ''}
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
            value={(form as { last_name?: string } | null)?.last_name ?? ''}
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
    {/if}
  </div>
</div>
