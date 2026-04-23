<script lang="ts">
  let { signal }: { signal: string } = $props();
</script>

<span
  class="rounded bg-[#f1f1f3] dark:bg-[#242428] px-1.5 py-0.5 font-mono text-[10.5px] font-medium tracking-wide text-ink-muted dark:text-ink-mutedDark"
>{signal}</span>
