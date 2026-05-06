import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail } from '@sveltejs/kit';

function requireAdmin(locals: App.Locals) {
  if (!locals.user) throw error(401);
  if (!locals.user.isAdmin) throw error(403, 'Administrator access required');
}

export const load: PageServerLoad = async ({ request }) => {
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const [tokens, products, users, entitlements] = await Promise.all([
    adapter.listAllApiTokens(),
    adapter.listProducts('all'),
    adapter.listUsers(),
    adapter.listEntitlements()
  ]);
  return { tokens, products, users, entitlements };
};

export const actions: Actions = {
  create: async ({ request, locals }) => {
    requireAdmin(locals);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const form = await request.formData();
    const description = String(form.get('description') ?? '').trim();
    if (!description) return fail(400, { error: 'Description required.' });
    const productId = String(form.get('productId') ?? '').trim() || null;
    const userId = String(form.get('userId') ?? '').trim() || null;
    const entitlements = (form.getAll('entitlement') as string[]).filter(Boolean);
    try {
      const created = await adapter.createAdminApiToken({ description, entitlements, productId, userId });
      return { ok: true, created };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },

  update: async ({ request, locals }) => {
    requireAdmin(locals);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    if (!id) return fail(400, { error: 'missing id' });
    const description = String(form.get('description') ?? '').trim();
    if (!description) return fail(400, { error: 'Description required.' });
    const isActive = form.get('isActive') === 'true';
    const productId = String(form.get('productId') ?? '').trim() || null;
    const userId = String(form.get('userId') ?? '').trim() || null;
    const entitlements = (form.getAll('entitlement') as string[]).filter(Boolean);
    try {
      await adapter.updateAdminApiToken(id, { description, isActive, entitlements, productId, userId });
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
    try {
      await adapter.deleteAdminApiToken(id);
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  }
};
