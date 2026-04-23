<script lang="ts">
  import type { Environment } from '$lib/adapters/types';
  let { env }: { env: Environment } = $props();
  const entries = $derived(Object.entries(env));
</script>

<div class="grid gap-x-5 gap-y-2.5 text-[12.5px]" style:grid-template-columns="140px 1fr">
  {#each entries as [k, v]}
    <div class="capitalize text-ink-muted dark:text-ink-mutedDark">{k}</div>
    <div class="font-mono text-xs text-ink dark:text-ink-dark">{v}</div>
  {/each}
</div>
