// Settings layout: side-nav between Members and Danger zone.
// Maintainer-only gate applies to children; we still let readwrite/readonly
// see member list (readonly), but individual actions are gated per form.

import type { LayoutServerLoad } from './$types';
import { error } from '@sveltejs/kit';

export const load: LayoutServerLoad = async ({ parent }) => {
  const { user, role, product } = await parent();
  if (!user) throw error(401);
  // Settings visible to anyone with any role (or admin). Individual
  // destructive actions enforce stricter requirements.
  if (!role && !user.isAdmin)
    throw error(403, `No access to ${product.name} settings`);
  return {};
};
