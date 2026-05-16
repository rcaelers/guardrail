import type { PageServerLoad, Actions } from './$types';
import { createAdapter } from '$lib/adapters';
import { error, fail, redirect } from '@sveltejs/kit';
import { requireProductAccess } from '$lib/server/product-access';

export const load: PageServerLoad = async ({ locals, url, request, params }) => {
  if (!locals.user) throw redirect(303, `/login?next=${encodeURIComponent(url.pathname)}`);

  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const settings = await adapter.getProcessorSettings(params.product);

  return { settings, productId: params.product };
};

export const actions: Actions = {
  save: async ({ request, locals, params }) => {
    if (!locals.user) throw error(401);
    const adapter = createAdapter(request.headers.get('cookie') ?? '');
    const { role } = await requireProductAccess(locals.user, params.product!, adapter);
    if (role !== 'maintainer' && !locals.user.isAdmin)
      throw error(403, 'Maintainer required');

    const form = await request.formData();

    const parsePatterns = (raw: string) =>
      raw
        .split('\n')
        .map((s) => s.trim())
        .filter(Boolean);

    const skipRaw = (form.get('skip_patterns') as string) ?? '';
    const endRaw = (form.get('end_patterns') as string) ?? '';
    const delimiterRaw = ((form.get('delimiter') as string) ?? '').trim();
    const frameCountRaw = ((form.get('maximum_frame_count') as string) ?? '').trim();

    const skip_patterns = parsePatterns(skipRaw);
    const end_patterns = parsePatterns(endRaw);
    const delimiter = delimiterRaw.length > 0 ? delimiterRaw : null;
    const maximum_frame_count =
      frameCountRaw.length > 0 ? parseInt(frameCountRaw, 10) : null;

    if (maximum_frame_count !== null && isNaN(maximum_frame_count)) {
      return fail(400, { error: 'Maximum frame count must be a number' });
    }

    try {
      await adapter.updateProcessorSettings(params.product!, {
        skip_patterns: skip_patterns.length > 0 ? skip_patterns : null,
        end_patterns: end_patterns.length > 0 ? end_patterns : null,
        delimiter,
        maximum_frame_count,
      });
      return { ok: true };
    } catch (e) {
      return fail(400, { error: (e as Error).message });
    }
  },
};
