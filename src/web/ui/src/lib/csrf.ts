// Double-submit CSRF for the browser -> SvelteKit boundary.
//
// `hooks.server.ts` sets a readable `csrf` cookie and requires mutating requests
// to echo it in the `x-csrf-token` header. Every SvelteKit-routed mutation the
// app makes goes through `window.fetch` (form actions via `use:enhance`, the
// manual `?/action` fetches, and `/logout`), so patching `fetch` once attaches
// the header everywhere without touching individual forms.
//
// (The only native, non-fetch form posts go to the Rust `/auth/*` routes, which
// bypass the SvelteKit server entirely; those are guarded by SameSite=Lax plus
// a server-side Origin check, not by this token.)

import { browser } from '$app/environment';
import { beginBusy } from '$lib/system-busy';

export const MUTATING_METHODS = new Set(['POST', 'PUT', 'PATCH', 'DELETE']);
export const CSRF_COOKIE = 'csrf';
export const CSRF_HEADER = 'x-csrf-token';
export const CSRF_FIELD = '__csrf';

export function readCsrfToken(): string | null {
  if (!browser) return null;
  const match = document.cookie.match(new RegExp(`(?:^|;\\s*)${CSRF_COOKIE}=([^;]+)`));
  return match ? decodeURIComponent(match[1]) : null;
}

let installed = false;

/** Patch `window.fetch` to add the CSRF header to same-origin mutating requests. */
export function installCsrfFetch(): void {
  if (!browser || installed) return;
  installed = true;

  const original = window.fetch.bind(window);

  window.fetch = (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    let sameOrigin = false;
    try {
      const method = (
        init?.method ?? (input instanceof Request ? input.method : 'GET')
      ).toUpperCase();
      const rawUrl = input instanceof Request ? input.url : String(input);
      sameOrigin = new URL(rawUrl, location.href).origin === location.origin;

      if (sameOrigin && MUTATING_METHODS.has(method)) {
        const token = readCsrfToken();

        if (token) {
          const headers = new Headers(
            init?.headers ?? (input instanceof Request ? input.headers : undefined)
          );
          if (!headers.has(CSRF_HEADER)) {
            headers.set(CSRF_HEADER, token);
          }
          init = { ...(init ?? {}), headers };
        }
      }
    } catch {
      // Never let CSRF wiring break a request; the server still enforces.
    }

    const finishBusy = sameOrigin ? beginBusy() : undefined;
    try {
      const response = original(input, init);
      return finishBusy ? response.finally(finishBusy) : response;
    } catch (cause) {
      finishBusy?.();
      throw cause;
    }
  };
}
