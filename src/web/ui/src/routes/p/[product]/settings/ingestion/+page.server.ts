import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '#lib/adapters/index.js';
import { error, fail } from '@sveltejs/kit';
import { GUARDRAIL_INGESTION_URL } from '$app/env/private';
import { requireProductAccess } from '#lib/server/product-access.js';

export const load: PageServerLoad = async ({ parent, url }) => {
  const { product } = await parent();
  return {
    productToken: product.productToken ?? null,
    ingestionUrl: GUARDRAIL_INGESTION_URL || url.origin
  };
};

export const actions: Actions = {
  save: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role } = await requireProductAccess(locals.user, params.product!, adapter);
    if (role !== 'maintainer' && !locals.user.isAdmin)
      throw error(403, 'Maintainer required');

    const form = await request.formData();
    const token = form.get('product_token') as string | null ?? '';

    try {
      const updated = await adapter.updateProductToken(params.product, token || undefined);
      return { ok: true, productToken: updated.productToken ?? null };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  }
};
