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
  for (const value of values) {
    const [kind, id] = String(value).split(':', 2);
    if ((kind !== 'crash' && kind !== 'symbol') || !id) return null;
    imports.push({ kind: kind as ImportKind, id });
  }
  return imports;
}

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
    if (imports.length > 200) return fail(400, { error: 'Select no more than 200 imports.' });

    try {
      const result = await adapter.retryImports(product.id, imports);
      return { ok: true, queued: result.queued };
    } catch (cause) {
      console.error('Unable to queue import retry', cause);
      return fail(502, {
        error: 'The retry could not be queued. The processed data remains available; try again later.'
      });
    }
  }
};
