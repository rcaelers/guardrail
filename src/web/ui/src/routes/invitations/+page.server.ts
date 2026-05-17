import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail, redirect } from '@sveltejs/kit';
import type { Role } from '$lib/adapters/types';

export const load: PageServerLoad = async ({ locals, url, request }) => {
  if (!locals.user) throw redirect(303, `/login?next=${encodeURIComponent(url.pathname)}`);

  const adapter = createAdapter(request.headers.get('cookie') ?? '');

  // Determine products the user can assign grants for.
  // Admins can assign any product; maintainers only their maintained products.
  let assignableProducts = await adapter.listProducts('all');
  if (!locals.user.isAdmin) {
    const memberships = await adapter.membershipsFor(locals.user.id);
    const maintainedIds = new Set(
      memberships.filter((m) => m.role === 'maintainer').map((m) => m.productId)
    );
    assignableProducts = assignableProducts.filter((p) => maintainedIds.has(p.id));
    if (assignableProducts.length === 0) {
      throw error(403, 'You need maintainer access to at least one product to manage invitations.');
    }
  }

  const [invitations, allUsers] = await Promise.all([
    adapter.listInvitations(),
    locals.user.isAdmin ? adapter.listUsers() : Promise.resolve([])
  ]);

  const userMap = Object.fromEntries(allUsers.map((u) => [u.id, u.name]));

  return {
    invitations,
    assignableProducts,
    userMap,
    currentUserId: locals.user.id,
    isAdmin: locals.user.isAdmin,
    origin: url.origin
  };
};

export const actions: Actions = {
  create: async ({ request, locals }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');

    const form = await request.formData();
    const is_admin = form.get('is_admin') === 'true';
    const expires_at_raw = (form.get('expires_at') as string) || null;
    const expires_at = expires_at_raw ? `${expires_at_raw}T23:59:59Z` : null;
    const send_email = form.get('send_email') === 'on';
    const email_to = send_email ? ((form.get('email_to') as string) || null) : null;

    const product_ids = (form.getAll('grant_product') as string[]).filter(Boolean);
    const roles = form.getAll('grant_role') as string[];
    const grants = product_ids.map((pid, i) => ({
      product_id: pid,
      role: (roles[i] ?? 'readonly') as Role
    }));

    try {
      await adapter.createInvitation({
        is_admin,
        grants,
        expires_at: expires_at || null,
        max_uses: 1,
        to: email_to || undefined
      });
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },

  update: async ({ request, locals }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');

    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    if (!id) return fail(400, { error: 'missing id' });

    const is_admin = form.get('is_admin') === 'true';
    const expires_at_raw = (form.get('expires_at') as string) || null;
    const expires_at = expires_at_raw ? `${expires_at_raw}T23:59:59Z` : null;

    const product_ids = (form.getAll('grant_product') as string[]).filter(Boolean);
    const roles = form.getAll('grant_role') as string[];
    const grants = product_ids.map((pid, i) => ({
      product_id: pid,
      role: (roles[i] ?? 'readonly') as Role
    }));

    try {
      await adapter.updateInvitation(id, { is_admin, grants, expires_at, max_uses: 1 });
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },

  revoke: async ({ request, locals }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');

    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    if (!id) return fail(400, { error: 'missing id' });

    try {
      await adapter.revokeInvitation(id);
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },

  resend: async ({ request, locals }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');

    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    const to = String(form.get('email_to') ?? '');
    if (!id) return fail(400, { error: 'missing id' });
    if (!to) return fail(400, { error: 'missing email address' });

    try {
      await adapter.resendInvitation(id, to);
      return { ok: true, action: 'resend' };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  }
};
