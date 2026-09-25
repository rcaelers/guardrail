<script lang="ts">
  let {
    x,
    y,
    version,
    onselectnearby,
    onselectsameversion,
    onclose
  }: {
    x: number;
    y: number;
    version: string;
    onselectnearby: () => void;
    onselectsameversion: () => void;
    onclose: () => void;
  } = $props();

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') onclose();
  }
</script>

<svelte:window onpointerdown={onclose} onkeydown={handleKeydown} onscroll={onclose} />

<div
  role="menu"
  tabindex="-1"
  aria-label="Row selection actions"
  class="fixed z-50 w-[210px] rounded-md border border-line bg-surface py-1 shadow-xl dark:border-line-dark dark:bg-surface-dark"
  style:left={`${x}px`}
  style:top={`${y}px`}
  onpointerdown={(event) => event.stopPropagation()}
  oncontextmenu={(event) => event.preventDefault()}
>
  <button
    type="button"
    role="menuitem"
    class="block w-full px-3 py-2 text-left text-[13px] hover:bg-surface-panel dark:hover:bg-surface-panelDark"
    onclick={() => {
      onselectnearby();
      onclose();
    }}
  >Select nearby</button>
  <button
    type="button"
    role="menuitem"
    disabled={!version}
    title={version ? `Select all rows with version ${version}` : 'This row has no version'}
    class="block w-full px-3 py-2 text-left text-[13px] hover:bg-surface-panel disabled:cursor-not-allowed disabled:opacity-40 dark:hover:bg-surface-panelDark"
    onclick={() => {
      onselectsameversion();
      onclose();
    }}
  >Select same version</button>
</div>
