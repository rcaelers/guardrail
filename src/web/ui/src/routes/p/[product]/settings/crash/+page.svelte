<script lang="ts">
  import { enhance } from '$app/forms';
  import { invalidateAll } from '$app/navigation';
  import { createAdapter } from '$lib/adapters';
  import type { PageData, ActionData } from './$types';
  import type { ValidationScript } from '$lib/adapters/types';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  let annotations = $state('');

  $effect(() => {
    annotations = data.minidump.mandatory_annotations.join('\n');
  });

  let scriptFiles: FileList | undefined = $state(undefined);

  $effect(() => {
    if (form?.ok && form?.action === 'uploadScript') {
      scriptFiles = undefined;
      invalidateAll();
    }
    if (form?.ok && form?.action === 'deleteScript') {
      invalidateAll();
    }
  });

  // ── Script viewer ──────────────────────────────────────────────────────────

  let viewingScript: ValidationScript | null = $state(null);
  let viewLoading = $state(false);
  let viewError = $state('');

  async function openScript(script: ValidationScript) {
    viewingScript = { ...script };
    viewLoading = true;
    viewError = '';
    try {
      const adapter = createAdapter('');
      const full = await adapter.getValidationScript(data.productId, script.id);
      viewingScript = full;
    } catch (e) {
      viewError = (e as Error).message || 'Failed to load script content.';
    } finally {
      viewLoading = false;
    }
  }

  function closeViewer() {
    viewingScript = null;
    viewError = '';
  }

  // ── Rhai syntax highlighting ────────────────────────────────────────────────

  type TokenType = 'comment' | 'string' | 'keyword' | 'number' | 'builtin' | 'ident' | 'other';

  interface Token { type: TokenType; text: string }

  const KEYWORDS = new Set([
    'let', 'fn', 'if', 'else', 'for', 'while', 'in', 'return', 'true', 'false',
    'import', 'export', 'as', 'break', 'continue', 'loop', 'match', 'switch',
    'do', 'until', 'throw', 'try', 'catch', 'type', 'const', 'global', 'is',
    'not', 'and', 'or', 'private', 'static',
  ]);

  const BUILTINS = new Set([
    'print', 'debug', 'len', 'push', 'pop', 'insert', 'remove', 'clear',
    'contains', 'keys', 'values', 'type_of', 'to_string', 'to_int', 'to_float',
    'to_bool', 'range', 'timestamp', 'sleep', 'exit',
  ]);

  function tokenize(code: string): Token[] {
    const tokens: Token[] = [];
    let i = 0;
    while (i < code.length) {
      // Single-line comment
      if (code[i] === '/' && code[i + 1] === '/') {
        let j = i;
        while (j < code.length && code[j] !== '\n') j++;
        tokens.push({ type: 'comment', text: code.slice(i, j) });
        i = j;
        continue;
      }
      // Block comment
      if (code[i] === '/' && code[i + 1] === '*') {
        let j = i + 2;
        while (j < code.length - 1 && !(code[j] === '*' && code[j + 1] === '/')) j++;
        j += 2;
        tokens.push({ type: 'comment', text: code.slice(i, j) });
        i = j;
        continue;
      }
      // String literals (double or single quote)
      if (code[i] === '"' || code[i] === "'") {
        const q = code[i];
        let j = i + 1;
        while (j < code.length && code[j] !== q) {
          if (code[j] === '\\') j++;
          j++;
        }
        j++;
        tokens.push({ type: 'string', text: code.slice(i, j) });
        i = j;
        continue;
      }
      // Numbers
      if (/[0-9]/.test(code[i])) {
        let j = i;
        while (j < code.length && /[0-9._xXa-fA-F]/.test(code[j])) j++;
        tokens.push({ type: 'number', text: code.slice(i, j) });
        i = j;
        continue;
      }
      // Identifiers / keywords / builtins
      if (/[a-zA-Z_]/.test(code[i])) {
        let j = i;
        while (j < code.length && /[a-zA-Z0-9_]/.test(code[j])) j++;
        const text = code.slice(i, j);
        const type: TokenType = KEYWORDS.has(text) ? 'keyword' : BUILTINS.has(text) ? 'builtin' : 'ident';
        tokens.push({ type, text });
        i = j;
        continue;
      }
      tokens.push({ type: 'other', text: code[i] });
      i++;
    }
    return tokens;
  }

  const TOKEN_CLASS: Record<TokenType, string> = {
    comment: 'color:#6a9955',
    string:  'color:#ce9178',
    keyword: 'color:#569cd6',
    builtin: 'color:#dcdcaa',
    number:  'color:#b5cea8',
    ident:   'color:#9cdcfe',
    other:   'color:#d4d4d4',
  };
</script>

<div class="mx-auto max-w-[800px] space-y-10">

  <div>
    <h1 class="mb-1 text-[20px] font-semibold tracking-[-0.01em]">Crash ingestion</h1>
    <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
      Configure mandatory annotations and validation scripts for this product's crash submissions.
    </p>
  </div>

  {#if form?.error}
    <p class="text-[12px] text-red-600 dark:text-red-400">{form.error}</p>
  {/if}

  <!-- Mandatory annotations -->
  <section>
    <div class="mb-3">
      <h2 class="text-[15px] font-semibold">Mandatory annotations</h2>
      <p class="text-[12px] text-ink-muted dark:text-ink-mutedDark mt-0.5">
        Crash submissions must include all listed annotation keys. One per line.
      </p>
    </div>

    {#if form?.ok && form?.action === 'saveAnnotations'}
      <p class="mb-3 text-[12px] text-green-600 dark:text-green-400">Annotations saved.</p>
    {/if}

    <form method="POST" action="?/saveAnnotations" use:enhance class="space-y-3">
      <div class="rounded-md border border-line dark:border-line-dark overflow-hidden">
        <textarea
          name="mandatory_annotations"
          bind:value={annotations}
          rows="5"
          spellcheck="false"
          placeholder="product&#10;version"
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
  </section>

  <!-- Validation scripts -->
  <section>
    <div class="mb-3">
      <h2 class="text-[15px] font-semibold">Validation scripts</h2>
      <p class="text-[12px] text-ink-muted dark:text-ink-mutedDark mt-0.5">
        Rhai scripts run against each crash submission. Scripts can inspect and modify annotations or reject the crash.
      </p>
    </div>

    <!-- Script list -->
    {#if data.scripts.length > 0}
      <div class="mb-4 rounded-md border border-line dark:border-line-dark overflow-hidden">
        {#each data.scripts as script, i}
          <div
            class="flex items-center justify-between px-4 py-2.5 bg-surface dark:bg-surface-dark"
            class:border-t={i > 0}
            class:border-line={i > 0}
            class:dark:border-line-dark={i > 0}
          >
            <div>
              <div class="text-[13px] font-medium font-mono">{script.name}</div>
              <div class="text-[11px] text-ink-muted dark:text-ink-mutedDark">
                Uploaded {new Date(script.created_at).toLocaleDateString()}
              </div>
            </div>
            <div class="flex items-center gap-4">
              <button
                type="button"
                onclick={() => openScript(script)}
                class="text-[12px] text-ink-muted hover:text-accent dark:hover:text-accent"
              >View</button>
              <form method="POST" action="?/deleteScript" use:enhance>
                <input type="hidden" name="script_id" value={script.id} />
                <button
                  type="submit"
                  class="text-[12px] text-ink-muted hover:text-red-600 dark:hover:text-red-400"
                >Delete</button>
              </form>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <p class="mb-4 text-[12px] text-ink-muted dark:text-ink-mutedDark italic">No validation scripts uploaded yet.</p>
    {/if}

    {#if form?.ok && form?.action === 'uploadScript'}
      <p class="mb-3 text-[12px] text-green-600 dark:text-green-400">Script uploaded.</p>
    {/if}

    <!-- Upload form -->
    <form
      method="POST"
      action="?/uploadScript"
      enctype="multipart/form-data"
      use:enhance
      class="rounded-md border border-line dark:border-line-dark overflow-hidden"
    >
      <div class="bg-surface-panel dark:bg-surface-panelDark px-4 py-3 border-b border-line dark:border-line-dark">
        <div class="text-[13px] font-medium">Upload script</div>
      </div>
      <div class="bg-surface dark:bg-surface-dark px-4 py-4">
        <label class="block text-[12px] text-ink-muted dark:text-ink-mutedDark mb-2">
          Script file (.rhai)
        </label>
        <div class="flex items-center gap-3">
          <label
            class="inline-flex cursor-pointer items-center gap-1.5 rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-3 py-1.5 text-[13px] font-medium hover:bg-surface dark:hover:bg-surface-dark transition-colors"
          >
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor" class="size-3.5 text-ink-muted dark:text-ink-mutedDark">
              <path d="M7.25 10.25a.75.75 0 0 0 1.5 0V4.56l1.97 1.97a.75.75 0 1 0 1.06-1.06L8.53 2.22a.75.75 0 0 0-1.06 0L4.22 5.47a.75.75 0 0 0 1.06 1.06l1.97-1.97v5.69Z" />
              <path d="M3.5 9.75a.75.75 0 0 0-1.5 0v1.5A2.75 2.75 0 0 0 4.75 14h6.5A2.75 2.75 0 0 0 14 11.25v-1.5a.75.75 0 0 0-1.5 0v1.5c0 .69-.56 1.25-1.25 1.25h-6.5c-.69 0-1.25-.56-1.25-1.25v-1.5Z" />
            </svg>
            Browse…
            <input
              type="file"
              name="script_file"
              accept=".rhai,text/plain"
              class="sr-only"
              onchange={(e) => { scriptFiles = (e.currentTarget as HTMLInputElement).files ?? undefined; }}
            />
          </label>
          <span class="text-[13px] font-mono text-ink-muted dark:text-ink-mutedDark">
            {scriptFiles?.[0]?.name ?? 'No file chosen'}
          </span>
        </div>
        <div class="flex justify-end pt-4">
          <button
            type="submit"
            class="rounded-md bg-accent px-4 py-1.5 text-[13px] font-medium text-white"
          >Upload</button>
        </div>
      </div>
    </form>
  </section>
</div>

<!-- Script viewer slide-over -->
{#if viewingScript}
  <!-- Backdrop -->
  <button
    type="button"
    class="fixed inset-0 z-40 bg-black/40"
    aria-label="Close viewer"
    onclick={closeViewer}
  ></button>

  <!-- Panel -->
  <div class="fixed inset-y-0 right-0 z-50 flex w-full max-w-[680px] flex-col bg-[#1e1e1e] shadow-2xl">
    <!-- Header -->
    <div class="flex items-center justify-between border-b border-white/10 px-5 py-3">
      <div>
        <div class="font-mono text-[13px] font-medium text-white">{viewingScript.name}</div>
        <div class="text-[11px] text-white/50">
          Uploaded {new Date(viewingScript.created_at).toLocaleDateString()}
        </div>
      </div>
      <button
        type="button"
        onclick={closeViewer}
        class="rounded p-1 text-white/60 hover:text-white hover:bg-white/10"
        aria-label="Close"
      >
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor" class="size-4">
          <path d="M5.28 4.22a.75.75 0 0 0-1.06 1.06L6.94 8l-2.72 2.72a.75.75 0 1 0 1.06 1.06L8 9.06l2.72 2.72a.75.75 0 1 0 1.06-1.06L9.06 8l2.72-2.72a.75.75 0 0 0-1.06-1.06L8 6.94 5.28 4.22Z" />
        </svg>
      </button>
    </div>

    <!-- Content -->
    <div class="flex-1 overflow-auto">
      {#if viewLoading}
        <div class="flex h-32 items-center justify-center text-[13px] text-white/40">Loading…</div>
      {:else if viewError}
        <div class="px-5 py-4 text-[13px] text-red-400">{viewError}</div>
      {:else if viewingScript.content != null}
        <pre
          class="min-h-full px-5 py-4 font-mono text-[13px] leading-relaxed"
          style="background:#1e1e1e; color:#d4d4d4; tab-size:4"
        >{#each tokenize(viewingScript.content) as token}<span style={TOKEN_CLASS[token.type]}>{token.text}</span>{/each}</pre>
      {/if}
    </div>
  </div>
{/if}
