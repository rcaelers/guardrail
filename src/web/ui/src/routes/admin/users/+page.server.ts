// Admin: users management.

import type { PageServerLoad, Actions } from './$types';
import { adapter } from '$lib/adapters';
import { error, fail } from '@sveltejs/kit';
import type { Role } from '$lib/adapters/types';

function requireAdmin(locals: App.Locals) {
  if (!locals.user) throw error(401);
  if (!locals.user.isAdmin) throw error(403, 'Administrator access required');
  return locals.user;
}

export const load: PageServerLoad = async () => {
  const [users, products] = await Promise.all([
    adapter.listUsers(),
    adapter.listProducts('all')
  ]);
  // Sort: admins first, then alphabetical
  users.sort((a, b) => {
    if (a.isAdmin !== b.isAdmin) return a.isAdmin ? -1 : 1;
    return a.name.localeCompare(b.name);
  });
  const usersWithPermissions = await Promise.all(
    users.map(async (user) => ({
      ...user,
      permissions: await adapter.membershipsFor(user.id)
    }))
  );
  return { users: usersWithPermissions, products };
};

export const actions: Actions = {
  create: async ({ request, locals }) => {
    requireAdmin(locals);
    const form = await request.formData();
    const email = String(form.get('email') ?? '').trim();
    const name = String(form.get('name') ?? '').trim();
    if (!email) return fail(400, { error: 'Email required.' });
    try {
      await adapter.createUser({ email, name });
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },
  update: async ({ request, locals }) => {
    requireAdmin(locals);
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    const email = String(form.get('email') ?? '').trim();
    const name = String(form.get('name') ?? '').trim();
    if (!id) return fail(400, { error: 'missing id' });
    if (!email) return fail(400, { error: 'Email required.' });
    if (!name) return fail(400, { error: 'Name required.' });
    try {
      await adapter.updateUser(id, { email, name });
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },
  delete: async ({ request, locals }) => {
    const user = requireAdmin(locals);
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    if (!id) return fail(400, { error: 'missing id' });
    if (id === user.id) return fail(400, { error: "You can't delete your own account." });
    await adapter.deleteUser(id);
    return { ok: true };
  },
  toggleAdmin: async ({ request, locals }) => {
    const user = requireAdmin(locals);
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    const isAdmin = form.get('isAdmin') === 'true';
    if (!id) return fail(400, { error: 'missing id' });
    if (id === user.id && !isAdmin)
      return fail(400, { error: "You can't remove your own admin status." });
    await adapter.setAdmin(id, isAdmin);
    return { ok: true };
  },
  setPermission: async ({ request, locals }) => {
    const user = requireAdmin(locals);
    const form = await request.formData();
    const userId = String(form.get('userId') ?? '');
    const productId = String(form.get('productId') ?? '');
    const role = String(form.get('role') ?? '') as Role;
    if (!userId || !productId || !role) return fail(400, { error: 'missing fields' });
    if (userId === user.id) return fail(400, { error: "You can't change your own product access from the admin console." });
    await adapter.grantAccess({ userId, productId, role });
    return { ok: true };
  },
  revokePermission: async ({ request, locals }) => {
    const user = requireAdmin(locals);
    const form = await request.formData();
    const userId = String(form.get('userId') ?? '');
    const productId = String(form.get('productId') ?? '');
    if (!userId || !productId) return fail(400, { error: 'missing fields' });
    if (userId === user.id) return fail(400, { error: "You can't revoke your own product access from the admin console." });
    await adapter.revokeAccess({ userId, productId });
    return { ok: true };
  }
};
