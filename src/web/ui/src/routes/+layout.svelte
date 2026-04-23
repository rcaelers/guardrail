<script lang="ts">
  import '../app.css';
  import { browser } from '$app/environment';
  let { children } = $props();

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

<div class="min-h-screen bg-surface dark:bg-surface-dark text-ink dark:text-ink-dark">
  {@render children?.()}
</div>
