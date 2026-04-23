// Root server load:
//   - Gate all routes on session (except /login).
//   - Expose current user + their accessible products to the whole app.

import type { LayoutServerLoad } from './$types';
import { redirect } from '@sveltejs/kit';
import { adapter } from '$lib/adapters';

export const load: LayoutServerLoad = async ({ locals, url }) => {
  const path = url.pathname;
  const isLogin = path === '/login' || path.startsWith('/login/');

  if (!locals.user) {
    if (isLogin) return { user: null, products: [] };
    const next = encodeURIComponent(path + url.search);
    throw redirect(303, `/login?next=${next}`);
  }

  const products = await adapter.listProducts('mine', locals.user.id);
  return {
    user: locals.user,
    products
  };
};
