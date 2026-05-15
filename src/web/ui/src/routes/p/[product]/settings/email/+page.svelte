<script lang="ts">
  import { enhance } from '$app/forms';
  import type { PageData, ActionData } from './$types';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  let subjectTemplate = $state(data.settings.invite_subject);
  let htmlTemplate = $state(data.settings.invite_html_template);
  let textTemplate = $state(data.settings.invite_text_template);

  const hasCustomSubject = $derived(subjectTemplate.trim().length > 0);
  const hasCustomHtml = $derived(htmlTemplate.trim().length > 0);
  const hasCustomText = $derived(textTemplate.trim().length > 0);

  let showDefaultHtml = $state(false);
  let showDefaultText = $state(false);
</script>

<div class="mx-auto max-w-[800px]">
  <div class="mb-6">
    <h1 class="mb-1 text-[20px] font-semibold tracking-[-0.01em]">Email templates</h1>
    <p class="text-[13px] text-ink-muted dark:text-ink-mutedDark">
      Customize the invitation email sent to new users. Leave a field empty to use the default template.
    </p>
  </div>

  {#if form?.ok}
    <p class="mb-4 text-[12px] text-green-600 dark:text-green-400">Settings saved.</p>
  {/if}
  {#if form?.error}
    <p class="mb-4 text-[12px] text-red-600 dark:text-red-400">{form.error}</p>
  {/if}

  <form
    method="POST"
    action="?/save"
    use:enhance
    class="space-y-6"
  >
    <div class="rounded-md border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark px-5 py-4">
      <div class="mb-1 text-[13px] font-medium">Available placeholders</div>
      <p class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
        <code class="rounded bg-surface dark:bg-surface-dark px-1 py-0.5 font-mono text-[11px]">&#123;&#123;app_name&#125;&#125;</code>
        — the application name, and
        <code class="rounded bg-surface dark:bg-surface-dark px-1 py-0.5 font-mono text-[11px]">&#123;&#123;invite_url&#125;&#125;</code>
        — the unique invitation link. Both work in the subject and body fields.
      </p>
    </div>

    <div class="rounded-md border border-line dark:border-line-dark overflow-hidden">
      <div class="flex items-center justify-between bg-surface-panel dark:bg-surface-panelDark px-4 py-3 border-b border-line dark:border-line-dark">
        <div>
          <div class="text-[13px] font-medium">Subject</div>
          <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
            {hasCustomSubject ? 'Using custom subject' : 'Using default subject'}
          </div>
        </div>
        {#if hasCustomSubject}
          <button
            type="button"
            onclick={() => (subjectTemplate = '')}
            class="text-[12px] text-ink-muted hover:text-red-600 dark:hover:text-red-400"
          >Clear</button>
        {/if}
      </div>
      <div class="px-4 py-3 bg-surface dark:bg-surface-dark">
        <input
          type="text"
          name="invite_subject"
          bind:value={subjectTemplate}
          placeholder="You've been invited to &#123;&#123;app_name&#125;&#125;"
          spellcheck="false"
          autocomplete="off"
          class="w-full rounded-md border border-line dark:border-line-dark bg-surface dark:bg-surface-dark px-3 py-1.5 text-[13px] outline-none focus:ring-1 focus:ring-accent"
        />
      </div>
    </div>

    <div class="rounded-md border border-line dark:border-line-dark overflow-hidden">
      <div class="flex items-center justify-between bg-surface-panel dark:bg-surface-panelDark px-4 py-3 border-b border-line dark:border-line-dark">
        <div>
          <div class="text-[13px] font-medium">HTML template</div>
          <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
            {hasCustomHtml ? 'Using custom template' : 'Using default template'}
          </div>
        </div>
        <div class="flex items-center gap-3">
          <button
            type="button"
            onclick={() => (showDefaultHtml = !showDefaultHtml)}
            class="text-[12px] text-ink-muted hover:text-ink dark:hover:text-ink-dark"
          >{showDefaultHtml ? 'Hide default' : 'View default'}</button>
          {#if hasCustomHtml}
            <button
              type="button"
              onclick={() => (htmlTemplate = '')}
              class="text-[12px] text-ink-muted hover:text-red-600 dark:hover:text-red-400"
            >Clear</button>
          {/if}
        </div>
      </div>
      {#if showDefaultHtml}
        <div class="border-b border-line dark:border-line-dark bg-surface-panel/50 dark:bg-surface-panelDark/50 px-4 py-2">
          <div class="text-[11px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark mb-2">Default template</div>
          <pre class="font-mono text-[12px] text-ink-muted dark:text-ink-mutedDark whitespace-pre-wrap break-all">{data.settings.default_invite_html_template}</pre>
        </div>
      {/if}
      <textarea
        name="invite_html_template"
        bind:value={htmlTemplate}
        rows="14"
        spellcheck="false"
        placeholder="Leave empty to use the default HTML template…"
        class="w-full resize-y bg-surface dark:bg-surface-dark px-4 py-3 font-mono text-[12px] outline-none placeholder:text-ink-muted/60"
      ></textarea>
    </div>

    <div class="rounded-md border border-line dark:border-line-dark overflow-hidden">
      <div class="flex items-center justify-between bg-surface-panel dark:bg-surface-panelDark px-4 py-3 border-b border-line dark:border-line-dark">
        <div>
          <div class="text-[13px] font-medium">Plain-text template</div>
          <div class="text-[12px] text-ink-muted dark:text-ink-mutedDark">
            {hasCustomText ? 'Using custom template' : 'Using default template'}
          </div>
        </div>
        <div class="flex items-center gap-3">
          <button
            type="button"
            onclick={() => (showDefaultText = !showDefaultText)}
            class="text-[12px] text-ink-muted hover:text-ink dark:hover:text-ink-dark"
          >{showDefaultText ? 'Hide default' : 'View default'}</button>
          {#if hasCustomText}
            <button
              type="button"
              onclick={() => (textTemplate = '')}
              class="text-[12px] text-ink-muted hover:text-red-600 dark:hover:text-red-400"
            >Clear</button>
          {/if}
        </div>
      </div>
      {#if showDefaultText}
        <div class="border-b border-line dark:border-line-dark bg-surface-panel/50 dark:bg-surface-panelDark/50 px-4 py-2">
          <div class="text-[11px] font-medium uppercase tracking-wider text-ink-muted dark:text-ink-mutedDark mb-2">Default template</div>
          <pre class="font-mono text-[12px] text-ink-muted dark:text-ink-mutedDark whitespace-pre-wrap">{data.settings.default_invite_text_template}</pre>
        </div>
      {/if}
      <textarea
        name="invite_text_template"
        bind:value={textTemplate}
        rows="8"
        spellcheck="false"
        placeholder="Leave empty to use the default plain-text template…"
        class="w-full resize-y bg-surface dark:bg-surface-dark px-4 py-3 font-mono text-[12px] outline-none placeholder:text-ink-muted/60"
      ></textarea>
    </div>

    <div class="flex items-center justify-between">
      <button
        type="submit"
        formaction="?/reset"
        onclick={() => { subjectTemplate = ''; htmlTemplate = ''; textTemplate = ''; }}
        class="text-[12.5px] text-ink-muted hover:text-ink dark:hover:text-ink-dark"
      >Reset all to defaults</button>
      <button
        type="submit"
        class="rounded-md bg-accent px-4 py-1.5 text-[13px] font-medium text-white"
      >Save</button>
    </div>
  </form>
</div>
