// Members settings. View is available to any member; mutations require
// maintainer role (or admin).

import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail } from '@sveltejs/kit';
import type { Role } from '$lib/adapters/types';
import { requireProductAccess } from '$lib/server/product-access';

function canManage(role: string | null | undefined, isAdmin: boolean): boolean {
  return isAdmin || role === 'maintainer';
}

export const load: PageServerLoad = async ({ parent, request }) => {
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const { product } = await parent();
  const members = await adapter.listMembers(product.id);
  const allUsers = await adapter.listUsers();
  const memberIds = new Set(members.map((m) => m.userId));
  const nonMembers = allUsers.filter((u) => !memberIds.has(u.id));
  return { members, nonMembers };
};

export const actions: Actions = {
  grant: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role, product } = await requireProductAccess(locals.user, params.product!, adapter);
    if (!canManage(role, locals.user.isAdmin)) throw error(403, 'Maintainer required');
    const form = await request.formData();
    const userId = String(form.get('userId') ?? '');
    const newRole = String(form.get('role') ?? 'readonly') as Role;
    if (!userId) return fail(400, { error: 'missing userId' });
    await adapter.grantAccess({ userId, productId: product.id, role: newRole });
    return { ok: true };
  },
  changeRole: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role, product } = await requireProductAccess(locals.user, params.product!, adapter);
    if (!canManage(role, locals.user.isAdmin)) throw error(403, 'Maintainer required');
    const form = await request.formData();
    const userId = String(form.get('userId') ?? '');
    const newRole = String(form.get('role') ?? '') as Role;
    if (!userId || !newRole) return fail(400, { error: 'missing fields' });
    await adapter.grantAccess({ userId, productId: product.id, role: newRole });
    return { ok: true };
  },
  revoke: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role, product } = await requireProductAccess(locals.user, params.product!, adapter);
    if (!canManage(role, locals.user.isAdmin)) throw error(403, 'Maintainer required');
    const form = await request.formData();
    const userId = String(form.get('userId') ?? '');
    if (!userId) return fail(400, { error: 'missing userId' });
    await adapter.revokeAccess({ userId, productId: product.id });
    return { ok: true };
  }
};
