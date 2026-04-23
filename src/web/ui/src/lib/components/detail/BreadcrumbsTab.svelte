<script lang="ts">
  import type { Breadcrumb } from '$lib/adapters/types';
  let { breadcrumbs }: { breadcrumbs: Breadcrumb[] } = $props();
  const levelClass = (l: string) =>
    l === 'error' ? 'text-signal-danger dark:text-signal-dangerDark' :
    l === 'warn' ? 'text-signal-warn dark:text-signal-warnDark' :
    'text-ink-muted dark:text-ink-mutedDark';
</script>

<div class="font-mono">
  {#each breadcrumbs as b}
    <div
      class="grid gap-3 border-b border-line dark:border-line-dark px-3 py-2.5 text-[11.5px]"
      style:grid-template-columns="70px 60px 90px 1fr"
    >
      <span class="text-ink-muted dark:text-ink-mutedDark">{b.t}</span>
      <span class={levelClass(b.level)}>{b.level}</span>
      <span class="text-ink-muted dark:text-ink-mutedDark">{b.category}</span>
      <span class="text-ink dark:text-ink-dark">{b.msg}</span>
    </div>
  {/each}
</div>
