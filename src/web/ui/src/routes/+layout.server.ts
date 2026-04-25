// Root server load:
//   - Gate all routes on session, except /login and public product routes.
//   - Expose current user + their accessible products to the whole app.
//   - Unauthenticated users on / or /p/... get public-only product list.

import type { LayoutServerLoad } from './$types';
import { redirect } from '@sveltejs/kit';
import { createAdapter } from '$lib/adapters';

export const load: LayoutServerLoad = async ({ locals, url, request }) => {
  const path = url.pathname;
  // /login and /auth/* are handled by the Rust server or the login page itself —
  // don't gate them or the OIDC redirect loop never terminates.
  const isLogin = path === '/login' || path.startsWith('/login/') || path.startsWith('/auth/');
  const isPublicAllowed = path === '/' || path.startsWith('/p/');

  const realUser = locals.realUser ?? null;
  const adapter = createAdapter(request.headers.get('cookie') ?? '');

  if (!locals.user) {
    if (isLogin) return { user: null, products: [], realUser: null };
    if (isPublicAllowed) {
      const products = await adapter.listProducts('public');
      return { user: null, products, realUser: null };
    }
    const next = encodeURIComponent(path + url.search);
    throw redirect(303, `/login?next=${next}`);
  }

  const products = await adapter.listProducts('mine', locals.user.id);
  return {
    user: locals.user,
    products,
    realUser
  };
};
