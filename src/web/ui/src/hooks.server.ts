// Resolve the current user from the session cookie on every request,
// so routes can read `event.locals.user` instead of re-parsing cookies.

import type { Handle } from '@sveltejs/kit';
import { adapter } from '$lib/adapters';
import { readSessionId } from '$lib/server/session';

export const handle: Handle = async ({ event, resolve }) => {
  const uid = readSessionId(event.cookies);
  event.locals.user = uid ? await adapter.getUser(uid) : null;
  return resolve(event);
};
