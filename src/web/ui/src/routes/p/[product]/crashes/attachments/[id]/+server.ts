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
    const race = await Promise.race([
      adapter.downloadAttachment(params.id),
      new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error('timeout')), 30_000)
      ),
    ]);
    response = race;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    if (msg === 'timeout') throw error(504, 'Storage backend timed out');
    console.error('downloadAttachment failed:', msg);
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
