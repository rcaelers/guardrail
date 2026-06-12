// Resolve the current user from the session cookie on every request,
// so routes can read `event.locals.user` instead of re-parsing cookies.

import type { Handle, HandleServerError } from '@sveltejs/kit';
import { env } from '$env/dynamic/private';
import { createAdapter } from '$lib/adapters';
import { CSRF_COOKIE, CSRF_FIELD, CSRF_HEADER, MUTATING_METHODS } from '$lib/csrf';

function newCsrfToken(): string {
  return crypto.randomUUID().replace(/-/g, '') + crypto.randomUUID().replace(/-/g, '');
}

export const handle: Handle = async ({ event, resolve }) => {
  const start = Date.now();
  const { method } = event.request;
  const path = event.url.pathname + event.url.search;

  // --- CSRF: double-submit token ----------------------------------------
  // Ensure a readable token cookie exists, then require every mutating request
  // to echo it (x-csrf-token header, or a __csrf form field as a fallback).
  // Cross-site callers can neither read the cookie nor set the header, so they
  // cannot forge a matching pair. The client patches fetch to send the header.
  const secure =
    event.url.protocol === 'https:' ||
    event.request.headers.get('x-forwarded-proto') === 'https';
  let csrfToken = event.cookies.get(CSRF_COOKIE);
  if (!csrfToken) {
    csrfToken = newCsrfToken();
    event.cookies.set(CSRF_COOKIE, csrfToken, {
      path: '/',
      httpOnly: false,
      sameSite: 'lax',
      secure,
      maxAge: 60 * 60 * 24 * 7
    });
  }
  if (MUTATING_METHODS.has(method)) {
    let provided = event.request.headers.get(CSRF_HEADER);
    if (!provided) {
      const contentType = event.request.headers.get('content-type') ?? '';
      if (
        contentType.includes('form-urlencoded') ||
        contentType.includes('multipart/form-data')
      ) {
        try {
          const field = (await event.request.clone().formData()).get(CSRF_FIELD);
          provided = typeof field === 'string' ? field : null;
        } catch {
          provided = null;
        }
      }
    }
    if (!provided || provided !== csrfToken) {
      console.warn(
        JSON.stringify({ level: 'WARN', message: 'CSRF validation failed', method, uri: path })
      );
      return new Response('CSRF validation failed', { status: 403 });
    }
  }

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

  const cookieHeader = event.request.headers.get('cookie') ?? '';
  const adapter = createAdapter(cookieHeader);

  try {
    const t0 = Date.now();
    event.locals.user = await adapter.getMe();
    getMeMs = Date.now() - t0;
    if (event.locals.user) {
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
  } catch (error) {
    console.warn('Failed to resolve session user:', error);
    event.locals.user = null;
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
