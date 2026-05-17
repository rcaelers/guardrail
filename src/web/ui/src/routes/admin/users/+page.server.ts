// Admin: users management.

import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail } from '@sveltejs/kit';
import type { Role } from '$lib/adapters/types';

function requireAdmin(locals: App.Locals) {
  if (!locals.user) throw error(401);
  if (!locals.user.isAdmin) throw error(403, 'Administrator access required');
  return locals.user;
}

export const load: PageServerLoad = async ({ request }) => {
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
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
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const form = await request.formData();
    const email = String(form.get('email') ?? '').trim();
    const name = String(form.get('name') ?? '').trim();
    const isAdmin = form.get('isAdmin') === 'true';
    const permissions: Array<{ productId: string; role: Role }> = JSON.parse(
      String(form.get('permissions') ?? '[]')
    );
    if (!email) return fail(400, { error: 'Email required.' });
    try {
      const user = await adapter.createUser({ email, name, isAdmin });
      for (const p of permissions) {
        await adapter.grantAccess({ userId: user.id, productId: p.productId, role: p.role });
      }
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },

  update: async ({ request, locals }) => {
    const currentUser = requireAdmin(locals);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    const email = String(form.get('email') ?? '').trim();
    const name = String(form.get('name') ?? '').trim();
    const isAdmin = form.get('isAdmin') === 'true';
    const permissions: Array<{ productId: string; role: Role }> = JSON.parse(
      String(form.get('permissions') ?? '[]')
    );
    if (!id) return fail(400, { error: 'missing id' });
    if (!email) return fail(400, { error: 'Email required.' });
    if (!name) return fail(400, { error: 'Name required.' });
    const isSelf = id === currentUser.id;
    try {
      await adapter.updateUser(id, { email, name });
      if (!isSelf) {
        await adapter.setAdmin(id, isAdmin);
      }
      const current = await adapter.membershipsFor(id);
      const newIds = new Set(permissions.map((p) => p.productId));
      for (const p of permissions) {
        await adapter.grantAccess({ userId: id, productId: p.productId, role: p.role });
      }
      for (const p of current) {
        if (!newIds.has(p.productId)) {
          await adapter.revokeAccess({ userId: id, productId: p.productId });
        }
      }
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },

  delete: async ({ request, locals }) => {
    const user = requireAdmin(locals);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    if (!id) return fail(400, { error: 'missing id' });
    if (id === user.id) return fail(400, { error: "You can't delete your own account." });
    await adapter.deleteUser(id);
    return { ok: true };
  },
};
