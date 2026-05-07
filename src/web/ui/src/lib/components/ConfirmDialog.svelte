<script lang="ts">
  let { message, confirmLabel = 'Confirm', onconfirm, oncancel }: {
    message: string;
    confirmLabel?: string;
    onconfirm: () => void;
    oncancel: () => void;
  } = $props();
</script>

<div
  role="dialog"
  aria-modal="true"
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
  onclick={oncancel}
>
  <div
    class="w-[380px] rounded-lg border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-5 py-5 shadow-xl"
    onclick={(e) => e.stopPropagation()}
  >
    <p class="mb-5 text-[13px] leading-relaxed">{message}</p>
    <div class="flex justify-end gap-2">
      <button
        type="button"
        onclick={oncancel}
        class="rounded-md border border-line dark:border-line-dark bg-transparent px-3 py-1.5 text-[13px]"
      >Cancel</button>
      <button
        type="button"
        onclick={onconfirm}
        class="rounded-md bg-red-600 px-3 py-1.5 text-[13px] font-medium text-white hover:bg-red-700"
      >{confirmLabel}</button>
    </div>
  </div>
</div>
