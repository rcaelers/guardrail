import type { RequestHandler } from './$types';
import { env } from '$env/dynamic/private';

export const POST: RequestHandler = async ({ request, cookies }) => {
  const webBase = (env.GUARDRAIL_API_URL ?? '').replace(/\/api\/v1\/?$/, '');
  const cookieHeader = request.headers.get('cookie') ?? '';

  try {
    await fetch(`${webBase}/auth/logout`, {
      method: 'POST',
      headers: { cookie: cookieHeader }
    });
  } catch {
    // best-effort: server-side session is flushed even if the call fails
  }

  // Clear all session-related cookies so the browser is fully signed out.
  // 'guardrail' is the tower-sessions session cookie; gr_uid/gr_real_uid are
  // set by the Rust auth handlers.
  for (const name of ['guardrail', 'gr_uid', 'gr_real_uid']) {
    cookies.delete(name, { path: '/' });
  }

  return new Response(null, { status: 204 });
};
