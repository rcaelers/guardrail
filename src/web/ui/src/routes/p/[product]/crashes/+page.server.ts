// Product-scoped crashes list. Reads productId from params, scopes list +
// selected group to that product, and enforces role gating on mutations.

import type { PageServerLoad, Actions } from './$types';
import { adapter } from '$lib/adapters';
import { error, fail, redirect } from '@sveltejs/kit';
import type { Status } from '$lib/adapters/types';
import { requireProductAccess } from '$lib/server/product-access';

function canWrite(role: string | null | undefined): boolean {
  return role === 'readwrite' || role === 'maintainer';
}

export const load: PageServerLoad = async ({ url, parent }) => {
  const { product } = await parent();

  const version = url.searchParams.get('version') ?? 'all';
  const status = (url.searchParams.get('status') ?? 'all') as Status | 'all';
  const search = url.searchParams.get('q') ?? '';
  const sort = (url.searchParams.get('sort') ?? 'count') as 'count' | 'recent' | 'similarity' | 'version';
  const selectedId = url.searchParams.get('id');

  const list = await adapter.listGroups({
    productId: product.id,
    version: version === 'all' ? undefined : version,
    status: status === 'all' ? undefined : (status as Status),
    search,
    sort
  });

  const selected = selectedId
    ? await adapter.getGroup(selectedId)
    : list.groups[0]
      ? await adapter.getGroup(list.groups[0].id)
      : null;

  // Guard: if caller passed an id from a different product, drop it.
  const safeSelected = selected && selected.productId === product.id ? selected : null;

  return {
    list,
    selected: safeSelected,
    filters: { version, status, search, sort }
  };
};

export const actions: Actions = {
  setStatus: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const { role } = await requireProductAccess(locals.user, params.product);
    if (!canWrite(role)) throw error(403, 'You are read-only on this product');
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    const status = String(form.get('status') ?? '') as Status;
    if (!id || !status) return fail(400, { error: 'missing id/status' });
    await adapter.setStatus(id, status);
    return { ok: true };
  },
  addNote: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const { role } = await requireProductAccess(locals.user, params.product);
    if (!canWrite(role)) throw error(403, 'You are read-only on this product');
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    const body = String(form.get('body') ?? '');
    if (!id || !body.trim()) return fail(400, { error: 'missing id/body' });
    await adapter.addNote(id, body.trim(), locals.user.name);
    return { ok: true };
  },
  merge: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const { role } = await requireProductAccess(locals.user, params.product);
    if (role !== 'maintainer') throw error(403, 'Only maintainers can merge groups');
    const form = await request.formData();
    const primaryId = String(form.get('primaryId') ?? '');
    const mergedId = String(form.get('mergedId') ?? '');
    if (!primaryId || !mergedId) return fail(400, { error: 'missing ids' });
    await adapter.mergeGroups(primaryId, mergedId);
    throw redirect(303, `/p/${params.product}/crashes?id=${encodeURIComponent(primaryId)}`);
  }
};
