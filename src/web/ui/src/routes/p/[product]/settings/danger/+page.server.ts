// Product deletion. Maintainer or admin only. Requires typing the product name.

import type { Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail, redirect } from '@sveltejs/kit';
import { requireProductAccess } from '$lib/server/product-access';

export const actions: Actions = {
  visibility: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role, product } = await requireProductAccess(locals.user, params.product!, adapter);
    if (role !== 'maintainer' && !locals.user.isAdmin)
      throw error(403, 'Maintainer required');
    const form = await request.formData();
    const isPublic = form.get('public') === 'on';
    try {
      const updated = await adapter.updateProduct(product.id, {
        name: product.name,
        slug: product.slug,
        description: product.description,
        color: product.color,
        public: isPublic
      });
      return { visibilityOk: true, public: updated.public };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },

  delete: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role, product } = await requireProductAccess(locals.user, params.product!, adapter);
    if (role !== 'maintainer' && !locals.user.isAdmin)
      throw error(403, 'Maintainer required');
    const form = await request.formData();
    const confirmation = String(form.get('confirm') ?? '');
    if (confirmation !== product.name)
      return fail(400, { error: `Type "${product.name}" exactly to confirm.` });
    await adapter.deleteProduct(product.id);
    throw redirect(303, '/');
  }
};
