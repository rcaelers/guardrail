import type { Actions, PageServerLoad } from './$types';
import { redirect } from '@sveltejs/kit';

export const load: PageServerLoad = async ({ locals }) => {
  if (locals.user) throw redirect(303, '/');
  return {};
};

export const actions: Actions = {
  default: async ({ request, fetch }) => {
    const form = await request.formData();
    const email = (form.get('email') as string ?? '').trim();

    if (!email) return { ok: false, error: 'Please enter your email address.' };

    try {
      await fetch('/auth/recovery', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ email })
      });
    } catch {
      // Swallow errors — the backend always responds 200 to prevent enumeration.
    }

    return { ok: true };
  }
};
