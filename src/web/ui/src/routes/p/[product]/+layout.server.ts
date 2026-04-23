// Product-scoped layout: resolves the product from the URL param, checks
// membership, and exposes role to children. 404 if product doesn't exist,
// 403 if the user has no membership (unless they're an admin — admins can
// view any product for support purposes, but only if the product exists).

import type { LayoutServerLoad } from './$types';
import { error, redirect } from '@sveltejs/kit';
import { adapter } from '$lib/adapters';

export const load: LayoutServerLoad = async ({ params, locals }) => {
  if (!locals.user) throw redirect(303, '/login');

  const product = await adapter.getProduct(params.product);
  if (!product) throw error(404, `Product "${params.product}" not found`);

  const role = await adapter.roleOf(locals.user.id, product.id);
  if (!role && !locals.user.isAdmin)
    throw error(403, `You don't have access to ${product.name}`);

  return { product, role };
};
