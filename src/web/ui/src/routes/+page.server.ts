// Root index: pick a reasonable landing page based on session state.
//   - not signed in -> +layout.server.ts already redirected to /login
//   - signed in, has products -> /p/<first>/crashes
//   - signed in, no products, admin -> /admin
//   - signed in, no products, not admin -> /no-access

import type { PageServerLoad } from './$types';
import { redirect } from '@sveltejs/kit';

export const load: PageServerLoad = async ({ parent }) => {
  const { user, products } = await parent();
  if (!user) throw redirect(303, '/login'); // safety net
  if (products.length > 0) throw redirect(303, `/p/${products[0].id}/crashes`);
  if (user.isAdmin) throw redirect(303, '/admin');
  throw redirect(303, '/no-access');
};
