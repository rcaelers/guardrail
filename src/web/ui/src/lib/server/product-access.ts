// Re-derives product + role for a request. Use in form actions, which don't
// receive `parent()` like load functions do.

import { error } from '@sveltejs/kit';
import type { GuardrailAdapter, User } from '$lib/adapters/types';

export async function requireProductAccess(user: User, productSlug: string, adapter: GuardrailAdapter) {
  const product = await adapter.getProduct(productSlug);
  if (!product) throw error(404, `Product "${productSlug}" not found`);
  const role = await adapter.roleOf(user.id, product.id);
  if (!role && !user.isAdmin)
    throw error(403, `You don't have access to ${product.name}`);
  return { product, role };
}
