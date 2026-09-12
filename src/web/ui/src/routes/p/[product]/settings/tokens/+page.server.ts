import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail } from '@sveltejs/kit';
import { requireProductAccess } from '$lib/server/product-access';

export const load: PageServerLoad = async ({ parent, request, locals }) => {
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const { product, role } = await parent();
  const canManage = role === 'maintainer' || (locals.user?.isAdmin ?? false);
  const [tokens, allEntitlements] = canManage
    ? await Promise.all([adapter.listApiTokens(product.id), adapter.listEntitlements()])
    : [[], []];
  // A token created here is bound to this product, so only product-scoped
  // capabilities can apply to it.
  const entitlements = allEntitlements.filter((e) => e.scope === 'product');
  return { tokens, entitlements };
};

export const actions: Actions = {
  create: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role, product } = await requireProductAccess(locals.user, params.product!, adapter);
    if (role !== 'maintainer' && !locals.user.isAdmin)
      throw error(403, 'Maintainer required to manage API tokens');

    const form = await request.formData();
    const description = String(form.get('description') ?? '').trim();
    if (!description) return fail(400, { error: 'Description required.' });

    const entitlements = (form.getAll('entitlement') as string[]).filter(Boolean);
    if (entitlements.length === 0) {
      entitlements.push('symbol-upload');
    }

    try {
      const created = await adapter.createApiToken(product.id, { description, entitlements });
      return { ok: true, created };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },

  update: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role, product } = await requireProductAccess(locals.user, params.product!, adapter);
    if (role !== 'maintainer' && !locals.user.isAdmin)
      throw error(403, 'Maintainer required to manage API tokens');

    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    const description = String(form.get('description') ?? '').trim();
    const isActive = String(form.get('isActive') ?? 'true') === 'true';
    const entitlements = (form.getAll('entitlement') as string[]).filter(Boolean);
    if (!id) return fail(400, { error: 'missing id' });
    if (!description) return fail(400, { error: 'description required' });

    try {
      await adapter.updateApiToken(product.id, id, { description, isActive, entitlements });
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },

  delete: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role, product } = await requireProductAccess(locals.user, params.product!, adapter);
    if (role !== 'maintainer' && !locals.user.isAdmin)
      throw error(403, 'Maintainer required to manage API tokens');

    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    if (!id) return fail(400, { error: 'missing id' });

    try {
      await adapter.deleteApiToken(product.id, id);
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  }
};
