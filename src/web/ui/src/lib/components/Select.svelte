<script lang="ts">
  interface Props {
    label: string;
    value: string;
    options: [string, string][];
    onChange: (v: string) => void;
  }
  let { label, value, options, onChange }: Props = $props();
</script>

<label class="inline-flex items-center gap-1.5 text-xs text-ink-muted dark:text-ink-mutedDark">
  <span>{label}</span>
  <div class="relative">
    <select
      value={value}
      onchange={(e) => onChange((e.target as HTMLSelectElement).value)}
      class="cursor-pointer appearance-none rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark py-1 pl-2.5 pr-6 text-xs text-ink dark:text-ink-dark outline-none"
    >
      {#each options as [v, l]}
        <option value={v}>{l}</option>
      {/each}
    </select>
    <svg
      width="9" height="9" viewBox="0 0 9 9"
      class="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-ink-muted dark:text-ink-mutedDark"
    >
      <path d="M2 3.5 L4.5 6 L7 3.5" stroke="currentColor" stroke-width="1.3" fill="none" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>
  </div>
</label>
