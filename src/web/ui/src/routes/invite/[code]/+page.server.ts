import type { PageServerLoad, Actions } from './$types';
import { error, fail, redirect } from '@sveltejs/kit';
import { env } from '$env/dynamic/private';

const apiBase = env.GUARDRAIL_API_URL ?? 'http://web:80/api/v1';

export const load: PageServerLoad = async ({ params }) => {
  const r = await fetch(`${apiBase}/invitations/redeem/${encodeURIComponent(params.code)}`);
  if (r.status === 404) {
    throw error(404, 'Invitation not found or has expired.');
  }
  if (!r.ok) {
    throw error(500, 'Failed to load invitation.');
  }
  const data = await r.json();
  if (data.redirect_url) {
    throw redirect(303, data.redirect_url);
  }
  return { code: params.code };
};

export const actions: Actions = {
  default: async ({ params, request }) => {
    const form = await request.formData();
    const username = String(form.get('username') ?? '').trim();
    const email = String(form.get('email') ?? '').trim();
    const first_name = (form.get('first_name') as string | null)?.trim() || null;
    const last_name = (form.get('last_name') as string | null)?.trim() || null;

    if (!username || !email) {
      return fail(400, { error: 'Username and email are required.', username, email, first_name, last_name });
    }

    const r = await fetch(`${apiBase}/invitations/redeem/${encodeURIComponent(params.code)}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ username, email, first_name, last_name })
    });

    if (!r.ok) {
      const text = await r.text();
      const msg = text || 'Failed to create account.';
      return fail(r.status >= 500 ? 500 : 400, { error: msg, username, email, first_name, last_name });
    }

    const { redirect_url } = await r.json();
    throw redirect(303, redirect_url);
  }
};
