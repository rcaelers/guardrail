<script lang="ts">
  import type { UserDescription } from '$lib/adapters/types';
  import { fmtDate } from '$lib/utils/format';
  let { user }: { user: UserDescription | null } = $props();
</script>

{#if !user}
  <div class="px-2 py-8 text-center">
    <div class="mb-1.5 text-[13px] text-ink-muted dark:text-ink-mutedDark">No user context attached</div>
    <div class="text-[11.5px] text-ink-muted/80 dark:text-ink-mutedDark/80">
      The crash reporter didn't collect a description for this event.
    </div>
  </div>
{:else}
  <div class="mb-3.5 rounded-lg border border-line dark:border-line-dark bg-surface-panel dark:bg-surface-panelDark p-4">
    <div class="mb-2.5 flex items-center gap-2.5 text-[11.5px] text-ink-muted dark:text-ink-mutedDark">
      <div class="flex h-[26px] w-[26px] items-center justify-center rounded-full bg-accent-soft dark:bg-accent-softDark font-sans text-xs font-semibold text-accent">
        {(user.author || 'A').slice(0, 1).toUpperCase()}
      </div>
      <div>
        <div class="text-[12.5px] font-medium text-ink dark:text-ink-dark">
          {user.author || 'Anonymous reporter'}
        </div>
        <div class="text-[11px]">Submitted with crash · {fmtDate(user.at)}</div>
      </div>
      <span class="flex-1"></span>
      <span class="rounded bg-[#f1f1f3] dark:bg-[#242428] px-2 py-0.5 text-[10px] uppercase tracking-wider">user-submitted</span>
    </div>
    <div class="whitespace-pre-wrap border-l-2 border-accent pl-3.5 font-sans text-[13.5px] leading-[1.6] text-ink dark:text-ink-dark">
      {user.body}
    </div>
  </div>
  <div class="px-1 font-sans text-[11px] text-ink-muted dark:text-ink-mutedDark">
    This description was attached by the user at the time of the crash. It is shown verbatim — not edited by triage.
  </div>
{/if}
