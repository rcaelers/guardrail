// Resolve the current user from the session cookie on every request,
// so routes can read `event.locals.user` instead of re-parsing cookies.

import type { Handle } from '@sveltejs/kit';
import { adapter } from '$lib/adapters';
import { clearSession, readSessionId } from '$lib/server/session';

export const handle: Handle = async ({ event, resolve }) => {
  const uid = readSessionId(event.cookies);
  if (!uid) {
    event.locals.user = null;
    return resolve(event);
  }

  try {
    event.locals.user = await adapter.getUser(uid);
    if (!event.locals.user) clearSession(event.cookies);
  } catch (error) {
    console.warn(`Failed to resolve session user ${uid}:`, error);
    clearSession(event.cookies);
    event.locals.user = null;
  }

  return resolve(event);
};
