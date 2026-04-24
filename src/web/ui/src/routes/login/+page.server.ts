// Login via Pocket ID OIDC. The Rust server owns the OIDC dance at
// /auth/login/start; we just redirect there, passing `next` through.
// On OIDC error the Rust redirects back here with ?error=...

import type { PageServerLoad } from './$types';
import { redirect } from '@sveltejs/kit';

export const load: PageServerLoad = async ({ locals, url }) => {
  if (locals.user) {
    const next = url.searchParams.get('next') ?? '/';
    throw redirect(303, next);
  }
  const next = url.searchParams.get('next') ?? '/';
  const error = url.searchParams.get('error') ?? null;

  // If there's no error to display, go straight to OIDC — no login page shown.
  if (!error) {
    const oidcStart = `/auth/login/start?next=${encodeURIComponent(next)}`;
    throw redirect(303, oidcStart);
  }

  return { error };
};
