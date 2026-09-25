import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { createAdapter } from '$lib/adapters';
import type { SymbolQuery } from '$lib/adapters/types';

export const GET: RequestHandler = async ({ params, request, url }) => {
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const query: SymbolQuery = {
    search: url.searchParams.get('q') ?? '',
    arch: (url.searchParams.get('arch') ?? 'all') as SymbolQuery['arch'],
    format: (url.searchParams.get('format') ?? 'all') as SymbolQuery['format'],
    sort: (url.searchParams.get('sort') ?? 'recent') as SymbolQuery['sort'],
    references: true
  };
  const symbols = await adapter.listSymbols(params.product!, query);
  return json(Object.fromEntries(symbols.map((symbol) => [symbol.id, symbol.referencedBy])));
};
