import type { RequestHandler } from './$types';
import { env } from '$env/dynamic/private';

export const POST: RequestHandler = async ({ request, cookies }) => {
  const webBase = (env.GUARDRAIL_API_URL ?? '').replace(/\/api\/v1\/?$/, '');
  const cookieHeader = request.headers.get('cookie') ?? '';

  // Use redirect:'manual' so Node.js doesn't follow the 302 server-side.
  // Guardrail's logout handler redirects to PocketID's end_session endpoint;
  // that redirect MUST be followed by the browser so PocketID can clear its
  // access_token cookie in the browser's cookie jar.
  let idpLogoutUrl: string | null = null;
  try {
    const resp = await fetch(`${webBase}/auth/logout`, {
      method: 'POST',
      headers: { cookie: cookieHeader },
      redirect: 'manual'
    });
    idpLogoutUrl = resp.headers.get('location');
  } catch {
    // best-effort: proceed with local cleanup even if the call fails
  }

  // Clear all session-related cookies so the browser is fully signed out.
  // 'guardrail' is the tower-sessions session cookie; gr_uid/gr_real_uid are
  // set by the Rust auth handlers.
  for (const name of ['guardrail', 'gr_uid', 'gr_real_uid']) {
    cookies.delete(name, { path: '/' });
  }

  // Return the IdP logout URL so the client can navigate there via
  // window.location.href. A top-level navigation is required so that the
  // browser sends SameSite=Lax cookies to the IdP and processes the
  // resulting Set-Cookie that clears the session.
  return new Response(
    JSON.stringify({ logoutUrl: idpLogoutUrl ?? '/login' }),
    { status: 200, headers: { 'content-type': 'application/json' } }
  );
};
