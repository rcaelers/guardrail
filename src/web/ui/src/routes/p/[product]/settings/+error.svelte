<script lang="ts">
  import { page } from '$app/stores';
</script>

<div class="flex h-full flex-col items-center justify-center px-4">
  <div class="w-full max-w-[420px] text-center">
    <div class="mb-2 text-[72px] font-semibold leading-none tracking-tight text-ink-muted/30 dark:text-ink-mutedDark/30">
      {$page.status}
    </div>
    <h1 class="mb-2 text-[18px] font-semibold tracking-[-0.01em]">
      {$page.status === 403 ? 'Access denied' :
       $page.status === 404 ? 'Page not found' :
       $page.status === 401 ? 'Sign in required' :
       'Something went wrong'}
    </h1>
    <p class="mb-8 text-[13px] text-ink-muted dark:text-ink-mutedDark">
      {$page.error?.message ?? 'An unexpected error occurred.'}
    </p>
    <div class="flex justify-center gap-2">
      <a
        href="/"
        class="rounded-md bg-accent px-4 py-2 text-[13px] font-medium text-white"
      >Go to home</a>
      <button
        type="button"
        onclick={() => history.back()}
        class="rounded-md border border-line dark:border-line-dark bg-transparent px-4 py-2 text-[13px] text-ink-muted dark:text-ink-mutedDark"
      >Go back</button>
    </div>
  </div>
</div>
