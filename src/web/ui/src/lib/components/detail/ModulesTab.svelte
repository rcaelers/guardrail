<script lang="ts">
  import type { Module } from '$lib/adapters/types';
  let { modules }: { modules: Module[] } = $props();
</script>

<div class="font-mono text-[11.5px]">
  {#each modules as m}
    <div
      class="grid gap-3 border-b border-line dark:border-line-dark px-3 py-2.5"
      style:grid-template-columns="1fr 110px 140px 80px"
      class:text-ink={m.inApp}
      class:dark:text-ink-dark={m.inApp}
      class:text-ink-muted={!m.inApp}
      class:dark:text-ink-mutedDark={!m.inApp}
    >
      <span>{m.name}</span>
      <span>{m.version || '—'}</span>
      <span>{m.addr}</span>
      <span class="text-right">{m.size}</span>
    </div>
  {/each}
</div>
