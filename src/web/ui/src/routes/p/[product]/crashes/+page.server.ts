// Product-scoped crashes list. Reads productId from params, scopes list +
// selected group to that product, and enforces role gating on mutations.

import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail, redirect } from '@sveltejs/kit';
import type { Status } from '$lib/adapters/types';
import { requireProductAccess } from '$lib/server/product-access';

function canWrite(role: string | null | undefined): boolean {
  return role === 'readwrite' || role === 'maintainer';
}

export const load: PageServerLoad = async ({ url, parent, request }) => {
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const { product } = await parent();

  const version = url.searchParams.get('version') ?? 'all';
  const status = (url.searchParams.get('status') ?? 'all') as Status | 'all';
  const search = url.searchParams.get('q') ?? '';
  const sort = (url.searchParams.get('sort') ?? 'count') as 'count' | 'recent' | 'similarity' | 'version';
  const userText = url.searchParams.get('userText') === 'yes';
  const page = Math.max(1, parseInt(url.searchParams.get('page') ?? '1', 10) || 1);
  const limitRaw = parseInt(url.searchParams.get('limit') ?? '25', 10);
  const limit = [10, 25, 50, 100].includes(limitRaw) ? limitRaw : 25;
  const offset = (page - 1) * limit;

  // Selection model: `crash` is the source of truth for what's shown in the
  // detail pane. `id` (group) is supported for back-compat / convenience —
  // resolves to that group's first crash. Defaults to the first group's
  // first crash so something is always visible.
  const crashId = url.searchParams.get('crash');
  const groupId = url.searchParams.get('id');

  const listPromise = adapter.listGroups({
    productId: product.id,
    version: version === 'all' ? undefined : version,
    status: status === 'all' ? undefined : (status as Status),
    search,
    sort,
    hasUserText: userText || undefined,
    limit,
    offset
  });

  // Resolve the selected crash. If the URL gives us a crash id, fetch
  // the list and the crash in parallel. If only a group id (or nothing)
  // is provided, we need the list first to pick a default group, then
  // getCrash for the detail. Going through getCrash means the list can
  // carry lightweight crash summaries (no full minidump blob per member).
  let selectedGroup = null;
  let selectedCrash = null;

  let list;
  if (crashId) {
    const [l, bundle] = await Promise.all([listPromise, adapter.getCrash(crashId)]);
    list = l;
    if (bundle) { selectedGroup = bundle.group; selectedCrash = bundle.crash; }
  } else {
    list = await listPromise;
    const targetGroupId = groupId ?? list.groups[0]?.id ?? null;
    if (targetGroupId) {
      // The list ships a preview of each group's newest crashes, so the group
      // is usually already resolvable without a getGroup round trip; only a
      // group from another page of the list needs the extra fetch.
      const preview = list.groups.find((g) => g.id === targetGroupId)?.crashes?.[0]?.id ?? null;
      const targetCrashId = preview ?? (await adapter.getGroup(targetGroupId))?.crashes[0]?.id ?? null;
      if (targetCrashId) {
        const bundle = await adapter.getCrash(targetCrashId);
        if (bundle) { selectedGroup = bundle.group; selectedCrash = bundle.crash; }
      }
    }
  }

  // Guard: if caller passed an id from a different product, drop it.
  if (selectedGroup && selectedGroup.productId !== product.id) {
    selectedGroup = null;
    selectedCrash = null;
  }

  return {
    list,
    selectedGroup,
    selectedCrash,
    filters: { version, status, search, sort, userText, page, limit }
  };
};

export const actions: Actions = {
  setStatus: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role } = await requireProductAccess(locals.user, params.product, adapter);
    if (!canWrite(role) && !locals.user.isAdmin) throw error(403, 'You are read-only on this product');
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    const status = String(form.get('status') ?? '') as Status;
    const fixedRaw = String(form.get('fixedInVersion') ?? '').trim();
    const fixedInVersion = fixedRaw.length > 0 ? fixedRaw : null;
    if (!id || !status) return fail(400, { error: 'missing id/status' });
    await adapter.setStatus(id, status, fixedInVersion);
    return { ok: true };
  },
  addNote: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role } = await requireProductAccess(locals.user, params.product, adapter);
    if (!canWrite(role) && !locals.user.isAdmin) throw error(403, 'You are read-only on this product');
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    const body = String(form.get('body') ?? '');
    if (!id || !body.trim()) return fail(400, { error: 'missing id/body' });
    await adapter.addNote(id, body.trim(), locals.user.name);
    return { ok: true };
  },
  updateNote: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role } = await requireProductAccess(locals.user, params.product, adapter);
    if (!canWrite(role) && !locals.user.isAdmin) throw error(403, 'You are read-only on this product');
    const form = await request.formData();
    const noteId = String(form.get('noteId') ?? '');
    const body = String(form.get('body') ?? '');
    if (!noteId || !body.trim()) return fail(400, { error: 'missing noteId/body' });
    await adapter.updateNote(noteId, body.trim());
    return { ok: true };
  },
  deleteNote: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role } = await requireProductAccess(locals.user, params.product, adapter);
    if (!canWrite(role) && !locals.user.isAdmin) throw error(403, 'You are read-only on this product');
    const form = await request.formData();
    const noteId = String(form.get('noteId') ?? '');
    if (!noteId) return fail(400, { error: 'missing noteId' });
    await adapter.deleteNote(noteId);
    return { ok: true };
  },
  merge: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role } = await requireProductAccess(locals.user, params.product, adapter);
    if (role !== 'maintainer' && !locals.user.isAdmin) throw error(403, 'Only maintainers can merge groups');
    const form = await request.formData();
    const primaryId = String(form.get('primaryId') ?? '');
    const mergedId = String(form.get('mergedId') ?? '');
    if (!primaryId || !mergedId) return fail(400, { error: 'missing ids' });
    try {
      await adapter.mergeGroups(primaryId, mergedId);
    } catch (e) {
      // The server refuses pairs that disagree on a decision (fixed version,
      // wontfix vs resolved, assignee); the Related tab already says why.
      return fail(409, { error: e instanceof Error ? e.message : 'merge refused' });
    }
    throw redirect(303, `/p/${params.product}/crashes?id=${encodeURIComponent(primaryId)}`);
  },
  deleteCrash: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { product, role } = await requireProductAccess(locals.user, params.product, adapter);
    if (!canWrite(role) && !locals.user.isAdmin) throw error(403, 'You are read-only on this product');
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    if (!id) return fail(400, { error: 'missing id' });
    const bundle = await adapter.getCrash(id);
    if (!bundle || bundle.crash.productId !== product.id) throw error(404, 'Crash not found');
    await adapter.deleteCrash(id);
    return { ok: true };
  },
  deleteGroup: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { product, role } = await requireProductAccess(locals.user, params.product, adapter);
    if (!canWrite(role) && !locals.user.isAdmin) throw error(403, 'You are read-only on this product');
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    if (!id) return fail(400, { error: 'missing id' });
    const group = await adapter.getGroup(id);
    if (!group || group.productId !== product.id) throw error(404, 'Crash group not found');
    await adapter.deleteGroup(id);
    return { ok: true };
  }
};
