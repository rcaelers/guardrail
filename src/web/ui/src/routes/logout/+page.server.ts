// Sign-out endpoint. POST /logout clears the cookie and kicks to /login.

import type { Actions } from './$types';
import { redirect } from '@sveltejs/kit';
import { clearSession } from '$lib/server/session';

export const actions: Actions = {
  default: async ({ cookies }) => {
    clearSession(cookies);
    throw redirect(303, '/login');
  }
};
