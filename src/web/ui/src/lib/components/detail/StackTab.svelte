<script lang="ts">
  import type { StackFrame } from '$lib/adapters/types';
  let { stack }: { stack: StackFrame[] } = $props();
</script>

<div class="font-mono">
  {#each stack as f, i}
    <div
      class="mb-1 rounded px-3 py-2.5"
      class:bg-[#f6f8fc]={f.inApp}
      class:dark:bg-[#1d1d20]={f.inApp}
      class:border={f.inApp}
      class:border-line={f.inApp}
      class:dark:border-line-dark={f.inApp}
      class:opacity-65={!f.inApp}
    >
      <div class="text-[12.5px] text-ink dark:text-ink-dark">
        <span class="mr-2.5 text-ink-muted dark:text-ink-mutedDark">#{i}</span>{f.fn}
      </div>
      <div class="pl-[22px] text-[11px] text-ink-muted dark:text-ink-mutedDark">
        {f.file}{f.line ? `:${f.line}` : ''}  ·  {f.addr}
      </div>
    </div>
  {/each}
</div>
