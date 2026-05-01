// Resolve the current user from the session cookie on every request,
// so routes can read `event.locals.user` instead of re-parsing cookies.

import type { Handle, HandleServerError } from '@sveltejs/kit';
import { env } from '$env/dynamic/private';
import { createAdapter } from '$lib/adapters';
import { clearSession, readSessionId } from '$lib/server/session';

export const handle: Handle = async ({ event, resolve }) => {
  const start = Date.now();
  const { method } = event.request;
  const path = event.url.pathname + event.url.search;

  console.log(
    JSON.stringify({
      timestamp: new Date().toISOString(),
      level: 'INFO',
      message: 'started processing request',
      method,
      uri: path
    })
  );

  event.locals.realUser = null;

  let getMeMs: number | undefined;
  let realUserMs: number | undefined;
  let resolveMs: number | undefined;

  const uid = readSessionId(event.cookies);
  if (!uid) {
    event.locals.user = null;
  } else {
    const cookieHeader = event.request.headers.get('cookie') ?? '';
    const adapter = createAdapter(cookieHeader);

    try {
      const t0 = Date.now();
      event.locals.user = await adapter.getMe();
      getMeMs = Date.now() - t0;
      if (!event.locals.user) {
        clearSession(event.cookies);
      } else {
        const realUid = event.cookies.get('gr_real_uid') ?? null;
        if (realUid) {
          // /auth/real-user reads from the trusted tower session and queries root DB
          // so it works even when the effective user (gr_uid) is not an admin.
          const webBase = (env.GUARDRAIL_API_URL ?? '').replace(/\/api\/v1\/?$/, '');
          try {
            const t1 = Date.now();
            const r = await fetch(`${webBase}/auth/real-user`, {
              headers: { cookie: cookieHeader }
            });
            realUserMs = Date.now() - t1;
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
  }

  const t2 = Date.now();
  const response = await resolve(event);
  resolveMs = Date.now() - t2;

  console.log(
    JSON.stringify({
      timestamp: new Date().toISOString(),
      level: 'INFO',
      message: 'finished processing request',
      method,
      uri: path,
      status: response.status,
      latency: `${Date.now() - start} ms`,
      ...(getMeMs !== undefined && { 'latency.getMe': `${getMeMs} ms` }),
      ...(realUserMs !== undefined && { 'latency.realUser': `${realUserMs} ms` }),
      ...(resolveMs !== undefined && { 'latency.resolve': `${resolveMs} ms` })
    })
  );

  return response;
};

export const handleError: HandleServerError = ({ error, event, status, message }) => {
  const err = error instanceof Error ? error : new Error(String(error));
  console.error(
    JSON.stringify({
      level: 'error',
      message: 'SvelteKit request failed',
      method: event.request.method,
      path: event.url.pathname,
      query: event.url.search,
      status,
      errorMessage: err.message,
      stack: err.stack
    })
  );

  return { message };
};
