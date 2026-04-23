<script lang="ts">
  import type { Status } from '$lib/adapters/types';
  let { status }: { status: Status } = $props();

  const color: Record<Status, string> = {
    new: 'bg-signal-danger dark:bg-signal-dangerDark',
    triaged: 'bg-signal-warn dark:bg-signal-warnDark',
    resolved: 'bg-signal-ok dark:bg-signal-okDark'
  };
</script>

<span class="inline-flex items-center gap-1.5 text-[11px] font-medium text-ink-muted dark:text-ink-mutedDark">
  <span class="h-[7px] w-[7px] shrink-0 rounded-full {color[status]}"></span>
  {status}
</span>
