import type { PageServerLoad, Actions } from './$types';
import { error, fail, redirect } from '@sveltejs/kit';
import { env } from '$env/dynamic/private';

const apiBase = env.GUARDRAIL_API_URL ?? 'http://web:80/api/v1';

export const load: PageServerLoad = async ({ params }) => {
  const r = await fetch(`${apiBase}/invitations/redeem/${encodeURIComponent(params.code)}`, {
    signal: AbortSignal.timeout(10_000)
  });
  if (r.status === 404) {
    throw error(404, 'Invitation not found or has expired.');
  }
  if (!r.ok) {
    throw error(500, 'Failed to load invitation.');
  }
  const data = await r.json();

  // Provider returned a direct navigation URL (e.g. no provisioner configured).
  if (data.redirect_url) {
    throw redirect(303, data.redirect_url);
  }

  return { code: params.code, needs_refresh: data.needs_refresh === true, setup_url: null };
};

export const actions: Actions = {
  submit: async ({ params, request }) => {
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
      body: JSON.stringify({ username, email, first_name, last_name }),
      signal: AbortSignal.timeout(10_000)
    });

    if (!r.ok) {
      const text = await r.text();
      const msg = text || 'Failed to create account.';
      return fail(r.status >= 500 ? 500 : 400, { error: msg, username, email, first_name, last_name });
    }

    const data = await r.json();

    // Provider supports a popup setup window.
    if (data.setup_url) {
      return { setup_url: data.setup_url as string };
    }

    // Provider has no popup URL (e.g. Rauthy); go straight to login.
    throw redirect(303, data.redirect_url ?? '/auth/login/start');
  },

  refresh: async ({ params }) => {
    const r = await fetch(
      `${apiBase}/invitations/redeem/${encodeURIComponent(params.code)}/setup-url`,
      { method: 'POST', signal: AbortSignal.timeout(10_000) }
    );
    if (!r.ok) {
      const text = await r.text();
      return fail(r.status >= 500 ? 500 : 400, { error: text || 'Failed to get a new setup link.' });
    }
    const data = await r.json();
    if (data.setup_url) {
      return { setup_url: data.setup_url as string };
    }
    throw redirect(303, data.redirect_url ?? '/auth/login/start');
  }
};
