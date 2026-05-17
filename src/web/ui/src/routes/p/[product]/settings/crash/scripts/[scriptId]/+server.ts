import type { RequestHandler } from './$types';
import { error, json } from '@sveltejs/kit';
import { createAdapter } from '$lib/adapters';
import { requireProductAccess } from '$lib/server/product-access';

export const GET: RequestHandler = async ({ params, locals, request }) => {
  if (!locals.user) throw error(401);
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  await requireProductAccess(locals.user, params.product, adapter);

  try {
    const script = await adapter.getValidationScript(params.product, params.scriptId);
    return json(script);
  } catch (e) {
    throw error(502, (e as Error).message || 'Failed to load script');
  }
};
