<script lang="ts">
  import type { Crash } from '$lib/adapters/types';
  import { crashAddress, crashOs, crashPlatform, exceptionType, mainModuleName } from '$lib/utils/crash-report';

  let { crash }: { crash: Crash } = $props();

  const rows = $derived([
    ['Report status', crash.status || '—'],
    ['Exception', exceptionType(crash)],
    ['Address', crashAddress(crash)],
    ['OS', crashOs(crash)],
    ['Platform', crashPlatform(crash) || '—'],
    ['CPU', crash.system_info?.cpu_info || '—'],
    ['CPU arch', crash.system_info?.cpu_arch || '—'],
    ['CPU count', String(crash.system_info?.cpu_count ?? '—')],
    ['PID', String(crash.pid ?? '—')],
    ['Threads', String(crash.thread_count ?? crash.threads?.length ?? '—')],
    ['Main module', mainModuleName(crash)]
  ]);
</script>

<div class="grid gap-x-5 gap-y-2.5 text-[12.5px]" style:grid-template-columns="140px 1fr">
  {#each rows as [key, value]}
    <div class="text-ink-muted dark:text-ink-mutedDark">{key}</div>
    <div class="font-mono text-xs text-ink dark:text-ink-dark">{value}</div>
  {/each}
</div>
