// Product deletion. Maintainer or admin only. Requires typing the product name.

import type { Actions } from './$types';
import { adapter } from '$lib/adapters';
import { error, fail, redirect } from '@sveltejs/kit';
import { requireProductAccess } from '$lib/server/product-access';

export const actions: Actions = {
  delete: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const { role, product } = await requireProductAccess(locals.user, params.product!);
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
