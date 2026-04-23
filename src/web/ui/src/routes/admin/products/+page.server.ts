// Admin: products management.

import type { PageServerLoad, Actions } from './$types';
import { adapter } from '$lib/adapters';
import { fail } from '@sveltejs/kit';

export const load: PageServerLoad = async () => {
  const products = await adapter.listProducts('all');
  // Enrich with member counts for the table.
  const withCounts = await Promise.all(
    products.map(async (p) => ({ ...p, memberCount: (await adapter.listMembers(p.id)).length }))
  );
  return { products: withCounts };
};

export const actions: Actions = {
  create: async ({ request }) => {
    const form = await request.formData();
    const name = String(form.get('name') ?? '').trim();
    const slug = String(form.get('slug') ?? '').trim();
    const description = String(form.get('description') ?? '').trim();
    if (!name) return fail(400, { error: 'Name required.' });
    try {
      await adapter.createProduct({ name, slug: slug || undefined, description });
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },
  delete: async ({ request }) => {
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    if (!id) return fail(400, { error: 'missing id' });
    await adapter.deleteProduct(id);
    return { ok: true };
  }
};
