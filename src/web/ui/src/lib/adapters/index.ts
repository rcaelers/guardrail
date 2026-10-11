// Creates the backend adapter for server loads and actions.
//
//   import { createAdapter } from '#lib/adapters/index.js';
//
// Set GUARDRAIL_API_URL in your env (e.g. .env) to route through the HTTP API.
//
// Pass the request's cookie header so the backend can generate a user-scoped JWT.

import { GUARDRAIL_API_URL } from '$app/env/private';
import { httpAdapter } from './http';
import type { GuardrailAdapter } from './types';

export function createAdapter(cookieHeader: string = ''): GuardrailAdapter {
  const apiUrl = GUARDRAIL_API_URL;
  if (!apiUrl) {
    throw new Error('GUARDRAIL_API_URL must be set for the web UI');
  }

  return httpAdapter(apiUrl, cookieHeader);
}
