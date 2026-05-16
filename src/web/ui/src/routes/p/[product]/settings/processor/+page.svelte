<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData, ActionData } from './$types';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  const s = $derived(data.settings);

  let skipPatterns = $state('');
  let endPatterns = $state('');
  let delimiter = $state('');
  let maximumFrameCount = $state('');

  $effect(() => {
    skipPatterns = (s.skip_patterns ?? []).join('\n');
    endPatterns = (s.end_patterns ?? []).join('\n');
    delimiter = s.delimiter ?? '';
    maximumFrameCount = s.maximum_frame_count?.toString() ?? '';
  });
</script>

<div class="mx-auto max-w-[800px]">
  <div class="mb-6">
    <h1 class="mb-1 text-[20px] font-semibold tracking-[-0.01em]">Processor settings</h1>
    <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
      Configure per-product signature generation. Leave a field empty to use the global default.
    </p>
  </div>

  {#if form?.ok}
    <p class="mb-4 text-[12px] text-green-600 dark:text-green-400">Settings saved.</p>
  {/if}
  {#if form?.error}
    <p class="mb-4 text-[12px] text-red-600 dark:text-red-400">{form.error}</p>
  {/if}

  <form method="POST" action="?/save" use:enhance class="space-y-6">

    <!-- Delimiter -->
    <div class="rounded-md border border-line dark:border-line-dark overflow-hidden">
      <div class="bg-surface-panel dark:bg-surface-panelDark px-4 py-3 border-b border-line dark:border-line-dark">
        <div class="text-[13px] font-medium">Delimiter</div>
        <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
          Separator used when joining signature tokens.
        </div>
      </div>
      <div class="px-4 py-3 bg-surface dark:bg-surface-dark">
        <input
          type="text"
          name="delimiter"
          bind:value={delimiter}
          placeholder={s.default_delimiter}
          spellcheck="false"
          autocomplete="off"
          class="w-full rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px] outline-none focus:ring-1 focus:ring-accent"
        />
      </div>
    </div>

    <!-- Maximum frame count -->
    <div class="rounded-md border border-line dark:border-line-dark overflow-hidden">
      <div class="bg-surface-panel dark:bg-surface-panelDark px-4 py-3 border-b border-line dark:border-line-dark">
        <div class="text-[13px] font-medium">Maximum frame count</div>
        <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
          Maximum number of stack frames considered when generating a signature.
        </div>
      </div>
      <div class="px-4 py-3 bg-surface dark:bg-surface-dark">
        <input
          type="number"
          name="maximum_frame_count"
          bind:value={maximumFrameCount}
          placeholder={String(s.default_maximum_frame_count)}
          min="1"
          class="w-40 rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px] outline-none focus:ring-1 focus:ring-accent"
        />
      </div>
    </div>

    <!-- Skip patterns -->
    <div class="rounded-md border border-line dark:border-line-dark overflow-hidden">
      <div class="flex items-center justify-between bg-surface-panel dark:bg-surface-panelDark px-4 py-3 border-b border-line dark:border-line-dark">
        <div>
          <div class="text-[13px] font-medium">Skip patterns</div>
          <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
            Regex patterns for frames to skip during signature generation. One per line.
          </div>
        </div>
        {#if skipPatterns.trim()}
          <button
            type="button"
            onclick={() => (skipPatterns = '')}
            class="text-[12px] text-ink-muted hover:text-red-600 dark:hover:text-red-400"
          >Clear</button>
        {/if}
      </div>
      {#if !skipPatterns.trim() && s.default_skip_patterns.length > 0}
        <div class="border-b border-line dark:border-line-dark bg-surface-panel/50 dark:bg-surface-panelDark/50 px-4 py-2">
          <div class="text-[11px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark mb-1">Global defaults</div>
          <pre class="font-mono text-[12px] text-ink-muted dark:text-ink-mutedDark whitespace-pre-wrap">{s.default_skip_patterns.join('\n')}</pre>
        </div>
      {/if}
      <textarea
        name="skip_patterns"
        bind:value={skipPatterns}
        rows="8"
        spellcheck="false"
        placeholder="Leave empty to use global defaults…"
        class="w-full resize-y bg-surface dark:bg-surface-dark px-4 py-3 font-mono text-[12px] outline-none placeholder:text-ink-muted/60"
      ></textarea>
    </div>

    <!-- End patterns -->
    <div class="rounded-md border border-line dark:border-line-dark overflow-hidden">
      <div class="flex items-center justify-between bg-surface-panel dark:bg-surface-panelDark px-4 py-3 border-b border-line dark:border-line-dark">
        <div>
          <div class="text-[13px] font-medium">End patterns</div>
          <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
            Regex patterns that stop frame scanning for signature generation. One per line.
          </div>
        </div>
        {#if endPatterns.trim()}
          <button
            type="button"
            onclick={() => (endPatterns = '')}
            class="text-[12px] text-ink-muted hover:text-red-600 dark:hover:text-red-400"
          >Clear</button>
        {/if}
      </div>
      {#if !endPatterns.trim() && s.default_end_patterns.length > 0}
        <div class="border-b border-line dark:border-line-dark bg-surface-panel/50 dark:bg-surface-panelDark/50 px-4 py-2">
          <div class="text-[11px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark mb-1">Global defaults</div>
          <pre class="font-mono text-[12px] text-ink-muted dark:text-ink-mutedDark whitespace-pre-wrap">{s.default_end_patterns.join('\n')}</pre>
        </div>
      {/if}
      <textarea
        name="end_patterns"
        bind:value={endPatterns}
        rows="6"
        spellcheck="false"
        placeholder="Leave empty to use global defaults…"
        class="w-full resize-y bg-surface dark:bg-surface-dark px-4 py-3 font-mono text-[12px] outline-none placeholder:text-ink-muted/60"
      ></textarea>
    </div>

    <div class="flex justify-end">
      <button
        type="submit"
        class="rounded-md bg-accent px-4 py-1.5 text-[13px] font-medium text-white"
      >Save</button>
    </div>
  </form>
</div>
