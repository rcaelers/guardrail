// Sign out: clear the gr_uid session cookie and return to the landing page.

import type { Actions } from './$types';
import { redirect } from '@sveltejs/kit';
import { clearSession } from '$lib/server/session';

export const actions: Actions = {
  default: async ({ cookies }) => {
    clearSession(cookies);
    throw redirect(303, '/');
  }
};
