// Product-scoped crash detail. Validates that the group belongs to this
// product; applies role gating to mutations.

import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail, redirect } from '@sveltejs/kit';
import type { Status } from '$lib/adapters/types';
import { requireProductAccess } from '$lib/server/product-access';

function canWrite(role: string | null | undefined): boolean {
  return role === 'readwrite' || role === 'maintainer';
}

export const load: PageServerLoad = async ({ params, parent, request }) => {
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const { product } = await parent();
  // Load the group (with lightweight crash summaries) to find the first
  // crash id, then fetch that crash's full detail.
  const group = await adapter.getGroup(params.id);
  if (!group) throw error(404, `Group ${params.id} not found`);
  if (group.productId !== product.id)
    throw error(404, `Group ${params.id} is not in ${product.name}`);
  const firstId = group.crashes[0]?.id;
  if (!firstId) throw error(404, `Group ${params.id} has no crashes`);
  const bundle = await adapter.getCrash(firstId);
  if (!bundle) throw error(404, `Crash ${firstId} not found`);
  return { group: bundle.group, crash: bundle.crash };
};

export const actions: Actions = {
  setStatus: async ({ request, params, locals }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role } = await requireProductAccess(locals.user, params.product!, adapter);
    if (!canWrite(role)) throw error(403, 'Read-only on this product');
    const form = await request.formData();
    const status = String(form.get('status') ?? '') as Status;
    if (!status) return fail(400, { error: 'missing status' });
    await adapter.setStatus(params.id!, status);
    return { ok: true };
  },
  addNote: async ({ request, params, locals }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role } = await requireProductAccess(locals.user, params.product!, adapter);
    if (!canWrite(role)) throw error(403, 'Read-only on this product');
    const form = await request.formData();
    const body = String(form.get('body') ?? '');
    if (!body.trim()) return fail(400, { error: 'missing body' });
    await adapter.addNote(params.id!, body.trim(), locals.user.name);
    return { ok: true };
  },
  merge: async ({ request, params, locals }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role } = await requireProductAccess(locals.user, params.product!, adapter);
    if (role !== 'maintainer') throw error(403, 'Maintainer required');
    const form = await request.formData();
    const mergedId = String(form.get('mergedId') ?? '');
    if (!mergedId) return fail(400, { error: 'missing mergedId' });
    await adapter.mergeGroups(params.id!, mergedId);
    throw redirect(303, `/p/${params.product}/crashes/${params.id}`);
  }
};
