import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail, redirect } from '@sveltejs/kit';

export const load: PageServerLoad = async ({ locals, url, request }) => {
  if (!locals.user?.isAdmin) throw redirect(303, `/login?next=${encodeURIComponent(url.pathname)}`);

  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const settings = await adapter.getAppEmailSettings();

  return { settings };
};

export const actions: Actions = {
  save: async ({ request, locals }) => {
    if (!locals.user?.isAdmin) throw error(403);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');

    const form = await request.formData();
    const recovery_html_template = (form.get('recovery_html_template') as string) ?? '';
    const recovery_text_template = (form.get('recovery_text_template') as string) ?? '';

    try {
      await adapter.updateAppEmailSettings({ recovery_html_template, recovery_text_template });
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },

  reset: async ({ locals, request }) => {
    if (!locals.user?.isAdmin) throw error(403);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');

    try {
      await adapter.updateAppEmailSettings({ recovery_html_template: '', recovery_text_template: '' });
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  }
};
