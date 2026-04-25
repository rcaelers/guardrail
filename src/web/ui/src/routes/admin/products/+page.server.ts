// Admin: products management.

import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail } from '@sveltejs/kit';

function requireAdmin(locals: App.Locals) {
  if (!locals.user) throw error(401);
  if (!locals.user.isAdmin) throw error(403, 'Administrator access required');
}

export const load: PageServerLoad = async ({ request }) => {
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const products = await adapter.listProducts('all');
  // Enrich with member counts for the table.
  const withCounts = await Promise.all(
    products.map(async (p) => ({ ...p, memberCount: (await adapter.listMembers(p.id)).length }))
  );
  return { products: withCounts };
};

export const actions: Actions = {
  create: async ({ request, locals }) => {
    requireAdmin(locals);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
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
  update: async ({ request, locals }) => {
    requireAdmin(locals);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    const name = String(form.get('name') ?? '').trim();
    const slug = String(form.get('slug') ?? '').trim();
    const description = String(form.get('description') ?? '').trim();
    const color = String(form.get('color') ?? '').trim();
    if (!id) return fail(400, { error: 'missing id' });
    if (!name) return fail(400, { error: 'Name required.' });
    if (!slug) return fail(400, { error: 'Slug required.' });
    try {
      await adapter.updateProduct(id, { name, slug, description, color });
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },
  delete: async ({ request, locals }) => {
    requireAdmin(locals);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    if (!id) return fail(400, { error: 'missing id' });
    await adapter.deleteProduct(id);
    return { ok: true };
  }
};
