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

    let login_url: string | null = null;
    try {
      const r = await fetch('/auth/recovery', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ email })
      });
      const data = await r.json().catch(() => ({}));
      login_url = data.login_url ?? null;
    } catch {
      // Swallow errors — the backend always responds 200 to prevent enumeration.
    }

    return { ok: true, login_url };
  }
};
