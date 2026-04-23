<script lang="ts">
  import type { Role } from '$lib/adapters/types';

  let { role }: { role: Role | null } = $props();

  const label = $derived(
    role === 'maintainer' ? 'Maintainer'
    : role === 'readwrite' ? 'Read · write'
    : role === 'readonly' ? 'Read-only'
    : 'No access'
  );
</script>

{#if role}
  <span
    class="rounded-full border border-line dark:border-line-dark px-2 py-0.5 text-[10.5px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark"
    title="Your role for this product"
  >{label}</span>
{/if}
