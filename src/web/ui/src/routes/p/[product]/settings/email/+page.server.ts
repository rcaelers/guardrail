import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail, redirect } from '@sveltejs/kit';

export const load: PageServerLoad = async ({ locals, url, request, params }) => {
  if (!locals.user) throw redirect(303, `/login?next=${encodeURIComponent(url.pathname)}`);

  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const settings = await adapter.getProductEmailSettings(params.product);

  return { settings, productId: params.product };
};

export const actions: Actions = {
  save: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');

    const form = await request.formData();
    const invite_html_template = (form.get('invite_html_template') as string) ?? '';
    const invite_text_template = (form.get('invite_text_template') as string) ?? '';

    try {
      await adapter.updateProductEmailSettings(params.product, {
        invite_html_template,
        invite_text_template,
      });
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },

  reset: async ({ locals, params, request }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');

    try {
      await adapter.updateProductEmailSettings(params.product, {
        invite_html_template: '',
        invite_text_template: '',
      });
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  }
};
