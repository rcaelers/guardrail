// Product-scoped symbols. Read lists everything for the product (filtered by
// URL params); mutations are gated on role.

import type { PageServerLoad, Actions } from './$types';
import { adapter } from '$lib/adapters';
import { error, fail } from '@sveltejs/kit';
import type { SymbolQuery } from '$lib/adapters/types';
import { requireProductAccess } from '$lib/server/product-access';

function canWrite(role: string | null | undefined): boolean {
  return role === 'readwrite' || role === 'maintainer';
}

export const load: PageServerLoad = async ({ url, parent }) => {
  const { product } = await parent();

  const q: SymbolQuery = {
    search: url.searchParams.get('q') ?? '',
    arch: (url.searchParams.get('arch') ?? 'all') as SymbolQuery['arch'],
    format: (url.searchParams.get('format') ?? 'all') as SymbolQuery['format'],
    sort: (url.searchParams.get('sort') ?? 'recent') as SymbolQuery['sort']
  };

  const symbols = await adapter.listSymbols(product.id, q);
  const uploaders = await adapter.listUsers();

  return { symbols, uploaders, filters: q };
};

export const actions: Actions = {
  upload: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const { role, product } = await requireProductAccess(locals.user, params.product!);
    if (!canWrite(role)) throw error(403, 'Read-only on this product');
    const form = await request.formData();
    const name = String(form.get('name') ?? '').trim();
    if (!name) return fail(400, { error: 'Name required.' });
    await adapter.uploadSymbol(product.id, {
      name,
      version: String(form.get('version') ?? '') || '0.0.0',
      arch: String(form.get('arch') ?? 'x86_64'),
      format: String(form.get('format') ?? 'PDB'),
      size: String(form.get('size') ?? '1.0 MB'),
      uploadedBy: locals.user.id
    });
    return { ok: true };
  },
  delete: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const { role } = await requireProductAccess(locals.user, params.product!);
    if (role !== 'maintainer') throw error(403, 'Only maintainers can delete symbols');
    const form = await request.formData();
    const id = String(form.get('id') ?? '');
    if (!id) return fail(400, { error: 'missing id' });
    await adapter.deleteSymbol(id);
    return { ok: true };
  }
};
