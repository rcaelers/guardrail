<script lang="ts">
  interface Props { dark: boolean; setDark: (v: boolean) => void; }
  let { dark, setDark }: Props = $props();
</script>

<div
  class="inline-flex rounded-md border p-0.5"
  class:bg-white={!dark}
  class:bg-[#1b1b1e]={dark}
  class:border-[#e2e2e2]={!dark}
  class:border-[#2a2a2e]={dark}
>
  {#each [['light', '☀'], ['dark', '☾']] as [k, glyph]}
    {@const active = (k === 'dark') === dark}
    <button
      type="button"
      onclick={() => setDark(k === 'dark')}
      class="cursor-pointer rounded px-2.5 py-1 text-xs font-sans transition-colors"
      class:bg-[#f0f0f0]={active && !dark}
      class:bg-[#2a2a2e]={active && dark}
      class:text-ink={!dark}
      class:text-ink-dark={dark}
    >{glyph} {k}</button>
  {/each}
</div>
