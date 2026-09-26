// Expensive crash-list enrichment, loaded after the lightweight group rows
// have rendered. Keeping this separate prevents previews, trends and version
// discovery from delaying the Crashes tab's first paint.

import type { RequestHandler } from './$types';
import { error, json } from '@sveltejs/kit';
import { createAdapter } from '$lib/adapters';
import type { Status } from '$lib/adapters/types';
import { requireProductAccess } from '$lib/server/product-access';

export const GET: RequestHandler = async ({ params, url, locals, request }) => {
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const product = locals.user
    ? (await requireProductAccess(locals.user, params.product, adapter)).product
    : await adapter.getProduct(params.product);
  if (!product) throw error(404, `Product "${params.product}" not found`);
  if (!locals.user && !product.public) throw error(401);

  const version = url.searchParams.get('version') ?? 'all';
  const status = (url.searchParams.get('status') ?? 'all') as Status | 'all';
  const search = url.searchParams.get('q') ?? '';
  const sort = (url.searchParams.get('sort') ?? 'count') as
    | 'count'
    | 'recent'
    | 'similarity'
    | 'version';
  const page = Math.max(1, parseInt(url.searchParams.get('page') ?? '1', 10) || 1);
  const limitRaw = parseInt(url.searchParams.get('limit') ?? '25', 10);
  const limit = [10, 25, 50, 100].includes(limitRaw) ? limitRaw : 25;

  return json(
    await adapter.listGroups({
      productId: product.id,
      version: version === 'all' ? undefined : version,
      status: status === 'all' ? undefined : status,
      search,
      sort,
      hasUserText: url.searchParams.get('userText') === 'yes' || undefined,
      details: true,
      limit,
      offset: (page - 1) * limit
    })
  );
};
