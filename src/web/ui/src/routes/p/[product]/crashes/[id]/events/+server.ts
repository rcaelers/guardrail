// Member crashes of one group, for the expanded row in the crash list.
//
// The list load already ships a small preview inline with every group, so
// expanding costs no request; this endpoint backs the "load more" row, which
// pulls the group's remaining crashes without a full page navigation.

import type { RequestHandler } from './$types';
import { error, json } from '@sveltejs/kit';
import { createAdapter } from '$lib/adapters';
import { requireProductAccess } from '$lib/server/product-access';

export const GET: RequestHandler = async ({ params, url, locals, request }) => {
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const product = locals.user
    ? (await requireProductAccess(locals.user, params.product, adapter)).product
    : await adapter.getProduct(params.product);
  if (!product) throw error(404, `Product "${params.product}" not found`);
  if (!locals.user && !product.public) throw error(401);

  const limitRaw = parseInt(url.searchParams.get('limit') ?? '', 10);
  const offsetRaw = parseInt(url.searchParams.get('offset') ?? '', 10);
  const limit = Number.isFinite(limitRaw) && limitRaw > 0 ? Math.min(limitRaw, 500) : undefined;
  const offset = Number.isFinite(offsetRaw) && offsetRaw > 0 ? offsetRaw : undefined;

  const result = await adapter.listGroupCrashes(params.id, {
    limit,
    offset,
    hasUserText: url.searchParams.get('hasUserText') === 'true' || undefined,
    version: url.searchParams.get('version') ?? undefined
  });

  // The API scopes reads by RLS, but a group id from another product would
  // otherwise leak its crash list through this product's URL.
  const foreign = result.crashes.find((c) => c.productId !== product.id);
  if (foreign) throw error(404, `Group ${params.id} is not in ${product.name}`);

  return json(result);
};
