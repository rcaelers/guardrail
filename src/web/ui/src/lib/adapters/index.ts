// Picks the adapter at boot. Swap the export when your real backend is ready.
//
//   import { createAdapter } from '$lib/adapters';
//
// Set GUARDRAIL_API_URL in your env (e.g. .env) to route through the HTTP
// adapter; otherwise the in-memory mock is used.
//
// Pass the request's cookie header so the backend can generate a user-scoped JWT.

import { env } from '$env/dynamic/private';
import { mockAdapter } from './mock';
import { httpAdapter } from './http';
import type { GuardrailAdapter } from './types';

export function createAdapter(cookieHeader: string = ''): GuardrailAdapter {
  return env.GUARDRAIL_API_URL
    ? httpAdapter(env.GUARDRAIL_API_URL, cookieHeader)
    : mockAdapter;
}
