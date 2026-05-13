import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail } from '@sveltejs/kit';

export const load: PageServerLoad = async ({ parent }) => {
  const { product } = await parent();
  return { ingestionToken: product.ingestionToken ?? null };
};

export const actions: Actions = {
  save: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');

    const form = await request.formData();
    const token = (form.get('ingestion_token') as string | null) ?? '';

    try {
      const updated = await adapter.updateProductIngestionToken(params.product, token || undefined);
      return { ok: true, ingestionToken: updated.ingestionToken ?? null };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },

  regenerate: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');

    try {
      const updated = await adapter.updateProductIngestionToken(params.product, undefined);
      return { ok: true, ingestionToken: updated.ingestionToken ?? null };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  }
};
