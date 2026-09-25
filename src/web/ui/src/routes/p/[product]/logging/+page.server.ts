import type { Actions, PageServerLoad } from './$types';
import type { ImportKind, RetryImportRef } from '$lib/adapters/types';
import { createAdapter } from '$lib/adapters';
import { error, fail } from '@sveltejs/kit';
import { requireProductAccess } from '$lib/server/product-access';

export const load: PageServerLoad = async ({ parent, request }) => {
  const { product } = await parent();
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  return { importLogs: await adapter.listImportLogs(product.id) };
};

function parseSelection(values: FormDataEntryValue[]): RetryImportRef[] | null {
  const imports: RetryImportRef[] = [];
  const seen = new Set<string>();
  for (const value of values) {
    const [kind, id] = String(value).split(':', 2);
    if ((kind !== 'crash' && kind !== 'symbol') || !id) return null;
    const key = `${kind}:${id}`;
    if (seen.has(key)) continue;
    seen.add(key);
    imports.push({ kind: kind as ImportKind, id });
  }
  return imports;
}

const IMPORT_BATCH_SIZE = 200;

export const actions: Actions = {
  retry: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role, product } = await requireProductAccess(locals.user, params.product!, adapter);
    if (role !== 'maintainer' && !locals.user.isAdmin) {
      throw error(403, 'Only maintainers can retry imports');
    }

    const form = await request.formData();
    const imports = parseSelection(form.getAll('import'));
    if (!imports || imports.length === 0) return fail(400, { error: 'Select at least one import.' });

    let queued = 0;
    try {
      for (let offset = 0; offset < imports.length; offset += IMPORT_BATCH_SIZE) {
        const result = await adapter.retryImports(product.id, imports.slice(offset, offset + IMPORT_BATCH_SIZE));
        queued += result.queued;
      }
      return { ok: true, queued };
    } catch (cause) {
      console.error('Unable to queue import retry', cause);
      return fail(502, {
        error: queued > 0
          ? `Queued ${queued} imports before the request failed. The remaining processed data is still available.`
          : 'The retry could not be queued. The processed data remains available; try again later.'
      });
    }
  },

  delete: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role, product } = await requireProductAccess(locals.user, params.product!, adapter);
    if (role !== 'maintainer' && !locals.user.isAdmin) {
      throw error(403, 'Only maintainers can delete retained imports');
    }

    const form = await request.formData();
    const imports = parseSelection(form.getAll('import'));
    if (!imports || imports.length === 0) return fail(400, { error: 'Select at least one import.' });

    let deleted = 0;
    try {
      for (let offset = 0; offset < imports.length; offset += IMPORT_BATCH_SIZE) {
        const result = await adapter.deleteImports(product.id, imports.slice(offset, offset + IMPORT_BATCH_SIZE));
        deleted += result.deleted;
      }
      return { ok: true, deleted };
    } catch (cause) {
      console.error('Unable to delete retained imports', cause);
      return fail(502, {
        error: deleted > 0
          ? `Deleted ${deleted} imports before the request failed. Refresh before trying the remaining selection again.`
          : 'The retained imports could not be deleted; try again later.'
      });
    }
  }
};
