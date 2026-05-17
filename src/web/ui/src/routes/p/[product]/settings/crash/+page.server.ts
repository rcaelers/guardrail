import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail, redirect } from '@sveltejs/kit';
import { requireProductAccess } from '$lib/server/product-access';

export const load: PageServerLoad = async ({ locals, url, request, params }) => {
  if (!locals.user) throw redirect(303, `/login?next=${encodeURIComponent(url.pathname)}`);

  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const [minidump, scripts] = await Promise.all([
    adapter.getMinidumpSettings(params.product),
    adapter.listValidationScripts(params.product),
  ]);

  return { minidump, scripts, productId: params.product };
};

export const actions: Actions = {
  saveAnnotations: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role } = await requireProductAccess(locals.user, params.product!, adapter);
    if (role !== 'maintainer' && !locals.user.isAdmin)
      throw error(403, 'Maintainer required');

    const form = await request.formData();
    const raw = (form.get('mandatory_annotations') as string) ?? '';
    const mandatory_annotations = raw
      .split('\n')
      .map((s) => s.trim())
      .filter(Boolean);

    try {
      await adapter.updateMinidumpSettings(params.product!, { mandatory_annotations });
      return { ok: true, action: 'saveAnnotations' };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },

  uploadScript: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role } = await requireProductAccess(locals.user, params.product!, adapter);
    if (role !== 'maintainer' && !locals.user.isAdmin)
      throw error(403, 'Maintainer required');

    const form = await request.formData();
    const file = form.get('script_file') as File | null;

    if (!file || file.size === 0) return fail(400, { error: 'Script file is required' });
    const name = file.name;

    const content = await file.text();
    if (!content.trim()) return fail(400, { error: 'Script file is empty' });

    try {
      await adapter.uploadValidationScript(params.product!, name, content);
      return { ok: true, action: 'uploadScript' };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },

  deleteScript: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role } = await requireProductAccess(locals.user, params.product!, adapter);
    if (role !== 'maintainer' && !locals.user.isAdmin)
      throw error(403, 'Maintainer required');

    const form = await request.formData();
    const scriptId = (form.get('script_id') as string) ?? '';
    if (!scriptId) return fail(400, { error: 'Script ID is required' });

    try {
      await adapter.deleteValidationScript(params.product!, scriptId);
      return { ok: true, action: 'deleteScript' };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },
};
