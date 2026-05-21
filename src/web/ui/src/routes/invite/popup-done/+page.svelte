<script lang="ts">
  import { browser } from '$app/environment';

  if (browser) {
    if (window.opener) {
      window.opener.postMessage({ type: 'setup-complete' }, window.location.origin);
    }
    window.close();
  }
</script>

<svelte:head><title>Setup complete · Guardrail</title></svelte:head>

<div class="flex min-h-screen items-center justify-center bg-surface dark:bg-surface-dark px-6 py-12 text-ink dark:text-ink-dark">
  <div class="w-full max-w-[400px] text-center">
    <div class="mb-6 flex justify-center">
      <div class="h-[26px] w-[26px] rounded-md bg-ink dark:bg-ink-dark"></div>
    </div>
    <h1 class="mb-2 text-[22px] font-semibold tracking-[-0.01em]">Passkey registered</h1>
    <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
      You can close this window and return to Guardrail.
    </p>
  </div>
</div>
