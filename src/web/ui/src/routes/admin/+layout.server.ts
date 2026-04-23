// Admin-only gate. Also loads lists once so both pages can hit the tab instantly.

import type { LayoutServerLoad } from './$types';
import { error, redirect } from '@sveltejs/kit';

export const load: LayoutServerLoad = async ({ locals }) => {
  if (!locals.user) throw redirect(303, '/login');
  if (!locals.user.isAdmin) throw error(403, 'Administrator access required');
  return {};
};
