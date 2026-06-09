// Double-submit CSRF for the browser -> SvelteKit boundary.
//
// `hooks.server.ts` sets a readable `csrf` cookie and requires mutating requests
// to echo it in the `x-csrf-token` header. Every SvelteKit-routed mutation the
// app makes goes through `window.fetch` (form actions via `use:enhance`, the
// manual `?/action` fetches, and `/logout`), so patching `fetch` once attaches
// the header everywhere without touching individual forms.
//
// (The only native, non-fetch form posts to the Rust `/auth/*` routes, which are
// a separate origin concern guarded by SameSite=Lax, not by this token.)

import { browser } from '$app/environment';

const MUTATING = new Set(['POST', 'PUT', 'PATCH', 'DELETE']);

export function readCsrfToken(): string | null {
  if (!browser) return null;
  const match = document.cookie.match(/(?:^|;\s*)csrf=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

let installed = false;

/** Patch `window.fetch` to add the CSRF header to same-origin mutating requests. */
export function installCsrfFetch(): void {
  if (!browser || installed) return;
  installed = true;

  const original = window.fetch.bind(window);

  window.fetch = (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    try {
      const method = (
        init?.method ?? (input instanceof Request ? input.method : 'GET')
      ).toUpperCase();

      if (MUTATING.has(method)) {
        const url = input instanceof Request ? input.url : String(input);
        const sameOrigin =
          url.startsWith('/') || url.startsWith('?') || url.startsWith(location.origin);
        const token = readCsrfToken();

        if (sameOrigin && token) {
          const headers = new Headers(
            init?.headers ?? (input instanceof Request ? input.headers : undefined)
          );
          if (!headers.has('x-csrf-token')) {
            headers.set('x-csrf-token', token);
          }
          init = { ...(init ?? {}), headers };
        }
      }
    } catch {
      // Never let CSRF wiring break a request; the server still enforces.
    }
    return original(input, init);
  };
}
