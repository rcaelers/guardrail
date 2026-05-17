import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail } from '@sveltejs/kit';
import { env } from '$env/dynamic/private';
import { requireProductAccess } from '$lib/server/product-access';

export const load: PageServerLoad = async ({ parent, url }) => {
  const { product } = await parent();
  return {
    productToken: product.productToken ?? null,
    ingestionUrl: env.GUARDRAIL_INGESTION_URL || url.origin
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
    const token = (form.get('product_token') as string | null) ?? '';

    try {
      const updated = await adapter.updateProductToken(params.product, token || undefined);
      return { ok: true, productToken: updated.productToken ?? null };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  }
};
