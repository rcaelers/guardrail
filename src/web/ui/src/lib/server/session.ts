// Cookie-backed "session" for the mock app.
// Holds just a user id; the adapter resolves it to the full User.

import type { Cookies } from '@sveltejs/kit';

const COOKIE = 'gr_uid';
const MAX_AGE = 60 * 60 * 24 * 30; // 30 days

export function readSessionId(cookies: Cookies): string | null {
  return cookies.get(COOKIE) ?? null;
}

export function writeSession(cookies: Cookies, userId: string) {
  cookies.set(COOKIE, userId, {
    path: '/',
    httpOnly: true,
    sameSite: 'lax',
    maxAge: MAX_AGE
  });
}

export function clearSession(cookies: Cookies) {
  cookies.delete(COOKIE, { path: '/' });
}
