// Product-scoped layout: resolves the product from the URL param, checks
// membership, and exposes role to children. 404 if product doesn't exist,
// 403 if the user has no membership (unless they're an admin — admins can
// view any product for support purposes, but only if the product exists).
// Unauthenticated users may view public products (role is null).

import type { LayoutServerLoad } from './$types';
import { error, redirect } from '@sveltejs/kit';
import { createAdapter } from '$lib/adapters';

export const load: LayoutServerLoad = async ({ params, locals, request }) => {
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const product = await adapter.getProduct(params.product);
  if (!product) throw error(404, `Product "${params.product}" not found`);

  if (!locals.user) {
    if (!product.public) {
      const next = encodeURIComponent(`/p/${params.product}/crashes`);
      throw redirect(303, `/login?next=${next}`);
    }
    return { product, role: null };
  }

  const role = await adapter.roleOf(locals.user.id, product.id);
  if (!role && !locals.user.isAdmin)
    throw error(403, `You don't have access to ${product.name}`);

  return { product, role };
};
