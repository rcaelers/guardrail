import type { RequestHandler } from './$types';
import { error } from '@sveltejs/kit';
import { createAdapter } from '$lib/adapters';
import { requireProductAccess } from '$lib/server/product-access';

export const GET: RequestHandler = async ({ params, locals, request }) => {
  if (!locals.user) throw error(401);
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  await requireProductAccess(locals.user, params.product, adapter);

  let response: Response | null;
  try {
    response = await adapter.downloadAttachment(params.id);
  } catch (e) {
    console.error('downloadAttachment failed:', e instanceof Error ? e.message : e);
    throw error(502, 'Failed to reach storage backend');
  }
  if (!response) throw error(404, `Attachment ${params.id} not found`);

  const headers = new Headers(response.headers);
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers
  });
};
