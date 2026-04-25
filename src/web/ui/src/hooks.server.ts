// Resolve the current user from the session cookie on every request,
// so routes can read `event.locals.user` instead of re-parsing cookies.

import type { Handle } from '@sveltejs/kit';
import { env } from '$env/dynamic/private';
import { createAdapter } from '$lib/adapters';
import { clearSession, readSessionId } from '$lib/server/session';

export const handle: Handle = async ({ event, resolve }) => {
  event.locals.realUser = null;

  const uid = readSessionId(event.cookies);
  if (!uid) {
    event.locals.user = null;
    return resolve(event);
  }

  const cookieHeader = event.request.headers.get('cookie') ?? '';
  const adapter = createAdapter(cookieHeader);

  try {
    event.locals.user = await adapter.getUser(uid);
    if (!event.locals.user) {
      clearSession(event.cookies);
    } else {
      const realUid = event.cookies.get('gr_real_uid') ?? null;
      if (realUid) {
        // /auth/real-user reads from the trusted tower session and queries root DB
        // so it works even when the effective user (gr_uid) is not an admin.
        const webBase = (env.GUARDRAIL_API_URL ?? '').replace(/\/api\/v1\/?$/, '');
        try {
          const r = await fetch(`${webBase}/auth/real-user`, {
            headers: { cookie: cookieHeader }
          });
          if (r.ok) {
            event.locals.realUser = await r.json();
          }
        } catch (e) {
          console.warn('Failed to fetch real user:', e);
        }
      }
    }
  } catch (error) {
    console.warn(`Failed to resolve session user ${uid}:`, error);
    clearSession(event.cookies);
    event.locals.user = null;
  }

  return resolve(event);
};
