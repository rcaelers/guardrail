// One crash plus its parent group, for selecting a crash in the list view.
//
// Selecting a crash used to go through a full page navigation, which re-ran the
// group list load — a scan over every crash row in the product — even though
// the list is unchanged by the selection. The list view fetches this instead
// and swaps the detail pane on its own.

import type { RequestHandler } from './$types';
import { error, json } from '@sveltejs/kit';
import { createAdapter } from '$lib/adapters';
import { requireProductAccess } from '$lib/server/product-access';

export const GET: RequestHandler = async ({ params, locals, request }) => {
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const product = locals.user
    ? (await requireProductAccess(locals.user, params.product, adapter)).product
    : await adapter.getProduct(params.product);
  if (!product) throw error(404, `Product "${params.product}" not found`);
  if (!locals.user && !product.public) throw error(401);

  const bundle = await adapter.getCrash(params.crashId);
  if (!bundle) throw error(404, `Crash ${params.crashId} not found`);
  if (bundle.crash.productId !== product.id)
    throw error(404, `Crash ${params.crashId} is not in ${product.name}`);

  return json(bundle);
};
