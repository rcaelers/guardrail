// Root server load:
//   - Gate all routes on session, except /login and public product routes.
//   - Expose current user + their accessible products to the whole app.
//   - Unauthenticated users on / or /p/... get public-only product list.

import type { LayoutServerLoad } from './$types';
import { redirect } from '@sveltejs/kit';
import { createAdapter } from '$lib/adapters';
import { env } from '$env/dynamic/private';

async function fetchSelfServiceUrl(request: Request): Promise<string | null> {
  try {
    const webBase = (env.GUARDRAIL_API_URL ?? '').replace(/\/api\/v1\/?$/, '');
    const resp = await fetch(`${webBase}/auth/config`, {
      headers: { cookie: request.headers.get('cookie') ?? '' }
    });
    if (!resp.ok) return null;
    const data = await resp.json() as { self_service_url?: string | null };
    return data.self_service_url ?? null;
  } catch {
    return null;
  }
}

export const load: LayoutServerLoad = async ({ locals, url, request }) => {
  const path = url.pathname;
  // /login and /auth/* are handled by the Rust server or the login page itself —
  // don't gate them or the OIDC redirect loop never terminates.
  const isLogin =
    path === '/login' ||
    path.startsWith('/login/') ||
    path.startsWith('/auth/') ||
    path.startsWith('/invite/');
  const isPublicAllowed = path === '/' || path.startsWith('/p/');

  const realUser = locals.realUser ?? null;
  const adapter = createAdapter(request.headers.get('cookie') ?? '');
  const selfServiceUrl = await fetchSelfServiceUrl(request);

  if (!locals.user) {
    if (isLogin) return { user: null, products: [], realUser: null, selfServiceUrl };
    if (isPublicAllowed) {
      const products = await adapter.listProducts('public');
      return { user: null, products, realUser: null, selfServiceUrl };
    }
    const next = encodeURIComponent(path + url.search);
    throw redirect(303, `/login?next=${next}`);
  }

  const products = await adapter.listProducts('mine', locals.user.id);
  return {
    user: locals.user,
    products,
    realUser,
    selfServiceUrl
  };
};
