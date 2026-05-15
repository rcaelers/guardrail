import type { RequestHandler } from './$types';
import { redirect } from '@sveltejs/kit';
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
    // best-effort: session will expire naturally if the call fails
  }

  cookies.delete('gr_uid', { path: '/' });
  cookies.delete('gr_real_uid', { path: '/' });

  throw redirect(303, '/login');
};
