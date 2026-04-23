// Admin: users management.

import type { PageServerLoad, Actions } from './$types';
import { adapter } from '$lib/adapters';
import { error, fail } from '@sveltejs/kit';

export const load: PageServerLoad = async () => {
  const users = await adapter.listUsers();
  // Sort: admins first, then alphabetical
  users.sort((a, b) => {
    if (a.isAdmin !== b.isAdmin) return a.isAdmin ? -1 : 1;
    return a.name.localeCompare(b.name);
  });
  return { users };
};

export const actions: Actions = {
  create: async ({ request }) => {
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
  delete: async ({ request, locals }) => {
    if (!locals.user) throw error(401);
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    if (!id) return fail(400, { error: 'missing id' });
    if (id === locals.user.id) return fail(400, { error: "You can't delete your own account." });
    await adapter.deleteUser(id);
    return { ok: true };
  },
  toggleAdmin: async ({ request, locals }) => {
    if (!locals.user) throw error(401);
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    const isAdmin = form.get('isAdmin') === 'true';
    if (!id) return fail(400, { error: 'missing id' });
    if (id === locals.user.id && !isAdmin)
      return fail(400, { error: "You can't remove your own admin status." });
    await adapter.setAdmin(id, isAdmin);
    return { ok: true };
  }
};
