import type { RequestHandler } from './$types';
import { error } from '@sveltejs/kit';
import { adapter } from '$lib/adapters';
import { requireProductAccess } from '$lib/server/product-access';

export const GET: RequestHandler = async ({ params, locals }) => {
  if (!locals.user) throw error(401);
  await requireProductAccess(locals.user, params.product);

  const response = await adapter.downloadAttachment(params.id);
  if (!response) throw error(404, `Attachment ${params.id} not found`);

  const headers = new Headers(response.headers);
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers
  });
};
