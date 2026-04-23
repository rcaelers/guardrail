// Picks the adapter at boot. Swap the export when your real backend is ready.
//
//   import { adapter } from '$lib/adapters';
//
// Set GUARDRAIL_API_URL in your env (e.g. .env) to route through the HTTP
// adapter; otherwise the in-memory mock is used.

import { env } from '$env/dynamic/private';
import { mockAdapter } from './mock';
import { httpAdapter } from './http';
import type { GuardrailAdapter } from './types';

export const adapter: GuardrailAdapter = env.GUARDRAIL_API_URL
  ? httpAdapter(env.GUARDRAIL_API_URL)
  : mockAdapter;
