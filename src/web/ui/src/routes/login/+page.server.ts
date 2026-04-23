// Fake sign-in. Accepts any email that maps to a seeded user; sets a cookie.
// Unauthenticated GETs redirect here from the root layout.

import type { Actions, PageServerLoad } from './$types';
import { fail, redirect } from '@sveltejs/kit';
import { adapter } from '$lib/adapters';
import { writeSession } from '$lib/server/session';

export const load: PageServerLoad = async ({ locals, url }) => {
  if (locals.user) {
    const next = url.searchParams.get('next') ?? '/';
    throw redirect(303, next);
  }
  // Expose the seeded user list so the login screen can suggest emails.
  const users = await adapter.listUsers();
  return {
    suggestions: users.map((u) => ({ email: u.email, name: u.name, isAdmin: u.isAdmin }))
  };
};

export const actions: Actions = {
  default: async ({ request, cookies, url }) => {
    const form = await request.formData();
    const email = String(form.get('email') ?? '').trim();
    if (!email) return fail(400, { email, error: 'Enter an email.' });
    const user = await adapter.signIn(email);
    if (!user) return fail(401, { email, error: `No user with email "${email}".` });
    writeSession(cookies, user.id);
    const next = url.searchParams.get('next') ?? '/';
    throw redirect(303, next);
  }
};
