<script lang="ts">
  let { count }: { count: number } = $props();
  const bars = $derived.by(() => {
    let s = count;
    const out: number[] = [];
    for (let i = 0; i < 14; i++) {
      s = (s * 9301 + 49297) % 233280;
      out.push((s / 233280) * 0.85 + 0.15);
    }
    return out;
  });
</script>

<div class="flex h-[18px] w-[72px] items-end gap-[2px]">
  {#each bars as b}
    <div
      class="w-[3px] shrink-0 rounded-[1px] bg-ink-muted dark:bg-ink-mutedDark"
      style:height="{b * 18}px"
      style:opacity={0.35 + b * 0.45}
    ></div>
  {/each}
</div>
