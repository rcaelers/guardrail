<script lang="ts">
  let { count, trend }: { count: number; trend?: number[] } = $props();
  const bars = $derived.by(() => {
    if (trend && trend.length > 0) {
      const max = Math.max(...trend, 1);
      return trend.map(v => v / max * 0.85 + 0.15);
    }
    // fallback: deterministic placeholder when no real data is available
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
