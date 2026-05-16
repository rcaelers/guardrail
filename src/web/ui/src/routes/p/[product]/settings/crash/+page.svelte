<script lang="ts">
  import { enhance } from '$app/forms';
  import { invalidateAll } from '$app/navigation';
  import type { PageData, ActionData } from './$types';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  let annotations = $state('');

  $effect(() => {
    annotations = data.minidump.mandatory_annotations.join('\n');
  });
  let scriptName = $state('');
  let scriptFile: FileList | undefined = $state(undefined);

  $effect(() => {
    if (form?.ok && form?.action === 'uploadScript') {
      scriptName = '';
      scriptFile = undefined;
      invalidateAll();
    }
    if (form?.ok && form?.action === 'deleteScript') {
      invalidateAll();
    }
  });
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
            <form method="POST" action="?/deleteScript" use:enhance>
              <input type="hidden" name="script_id" value={script.id} />
              <button
                type="submit"
                class="text-[12px] text-ink-muted hover:text-red-600 dark:hover:text-red-400"
              >Delete</button>
            </form>
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
      <div class="bg-surface dark:bg-surface-dark px-4 py-4 space-y-3">
        <div>
          <label class="block text-[12px] text-ink-muted dark:text-ink-mutedDark mb-1" for="script_name">
            Name
          </label>
          <input
            id="script_name"
            type="text"
            name="script_name"
            bind:value={scriptName}
            placeholder="my_validation.rhai"
            spellcheck="false"
            autocomplete="off"
            class="w-full rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px] outline-none focus:ring-1 focus:ring-accent"
          />
        </div>
        <div>
          <label class="block text-[12px] text-ink-muted dark:text-ink-mutedDark mb-1" for="script_file">
            Script file (.rhai)
          </label>
          <input
            id="script_file"
            type="file"
            name="script_file"
            accept=".rhai,text/plain"
            class="text-[13px] text-ink dark:text-ink-dark"
          />
        </div>
        <div class="flex justify-end pt-1">
          <button
            type="submit"
            class="rounded-md bg-accent px-4 py-1.5 text-[13px] font-medium text-white"
          >Upload</button>
        </div>
      </div>
    </form>
  </section>
</div>
