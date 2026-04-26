// Stub for a real backend. Fill in fetch() calls to your API.
// Every adapter must satisfy GuardrailAdapter — swap this in via index.ts.
//
// Endpoints assumed here (rename to match your API):
//   POST /auth/signin                     { email } -> User
//   GET  /users/:id                                  -> User | 404
//   GET  /users                                      -> User[]
//   POST /users                           { email, name }
//   POST /users/:id                       { email?, name? }
//   DEL  /users/:id
//   POST /users/:id/admin                 { isAdmin }
//   GET  /products?scope=all|mine&user=…  -> Product[]
//   GET  /products/:id
//   POST /products                        { name, slug?, description? }
//   POST /products/:id                    { name?, slug?, description?, color? }
//   DEL  /products/:id
//   GET  /products/:id/members            -> MembershipWithUser[]
//   GET  /users/:id/memberships           -> MembershipWithProduct[]
//   POST /products/:pid/members/:uid      { role }
//   DEL  /products/:pid/members/:uid
//   GET  /crashes?productId=…&…           -> ListResult
//   GET  /crashes/:id                     -> CrashGroup | 404
//   POST /crashes/:id/status              { status }
//   POST /crashes/:id/notes               { body, author } -> Note
//   POST /crashes/:id/merge               { mergedId }
//   GET  /products/:pid/symbols?…         -> Symbol[]
//   POST /products/:pid/symbols           { name, version, arch, format, size, uploadedBy } -> Symbol
//   DEL  /symbols/:id

import type {
  GuardrailAdapter, Crash, CrashGroup, ListQuery, ListResult, Note, Status,
  User, Product, Role, MembershipWithUser, MembershipWithProduct,
  Symbol as SymbolRow, SymbolQuery,
  Invitation, CreateInvitationSpec, UpdateInvitationSpec,
  ApiToken, CreatedApiToken, CreateApiTokenSpec, CreateAdminApiTokenSpec
} from './types';

export function httpAdapter(baseUrl: string, cookieHeader: string = ''): GuardrailAdapter {
  const qs = (q: Record<string, unknown>) =>
    new URLSearchParams(
      Object.entries(q)
        .filter(([, v]) => v !== undefined && v !== null && v !== '')
        .map(([k, v]) => [k, String(v)])
    ).toString();

  async function req(path: string, init?: RequestInit): Promise<Response> {
    const url = `${baseUrl}${path}`;
    const headers = new Headers(init?.headers);
    if (cookieHeader && !headers.has('cookie')) headers.set('cookie', cookieHeader);
    try {
      return await fetch(url, { ...init, headers });
    } catch (e) {
      const reason = e instanceof Error ? e.message : String(e);
      throw new Error(`${init?.method ?? 'GET'} ${url} failed: ${reason}`);
    }
  }

  async function json<T>(res: Response, what: string): Promise<T> {
    if (!res.ok) throw new Error(`${what} ${res.status}`);
    return res.json();
  }
  const jpost = (path: string, body: unknown) =>
    req(path, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body)
    });
  const jdel = (path: string) => req(path, { method: 'DELETE' });

  return {
    // --- session ---
    async signIn(email) {
      const r = await jpost('/auth/signin', { email });
      if (r.status === 404 || r.status === 401) return null;
      return json<User>(r, 'signIn');
    },
    async getUser(id) {
      const r = await req(`/users/${encodeURIComponent(id)}`);
      if (r.status === 404) return null;
      return json<User>(r, 'getUser');
    },

    // --- products ---
    async listProducts(scope = 'all', userId) {
      const r = await req(`/products?${qs({ scope, user: userId })}`);
      return json<Product[]>(r, 'listProducts');
    },
    async getProduct(id) {
      const r = await req(`/products/${encodeURIComponent(id)}`);
      if (r.status === 404) return null;
      return json<Product>(r, 'getProduct');
    },
    async createProduct(spec) {
      const r = await jpost('/products', spec);
      return json<Product>(r, 'createProduct');
    },
    async updateProduct(id, patch) {
      const r = await jpost(`/products/${encodeURIComponent(id)}`, patch);
      return json<Product>(r, 'updateProduct');
    },
    async deleteProduct(id) {
      const r = await jdel(`/products/${encodeURIComponent(id)}`);
      if (!r.ok) throw new Error(`deleteProduct ${r.status}`);
    },

    // --- users ---
    async listUsers() {
      const r = await req('/users');
      return json<User[]>(r, 'listUsers');
    },
    async createUser(spec) {
      const r = await jpost('/users', spec);
      return json<User>(r, 'createUser');
    },
    async updateUser(id, patch) {
      const r = await jpost(`/users/${encodeURIComponent(id)}`, patch);
      return json<User>(r, 'updateUser');
    },
    async deleteUser(id) {
      const r = await jdel(`/users/${encodeURIComponent(id)}`);
      if (!r.ok) throw new Error(`deleteUser ${r.status}`);
    },
    async setAdmin(id, isAdmin) {
      const r = await jpost(`/users/${encodeURIComponent(id)}/admin`, { isAdmin });
      if (!r.ok) throw new Error(`setAdmin ${r.status}`);
    },

    // --- memberships ---
    async listMembers(productId) {
      const r = await req(`/products/${encodeURIComponent(productId)}/members`);
      return json<MembershipWithUser[]>(r, 'listMembers');
    },
    async membershipsFor(userId) {
      const r = await req(`/users/${encodeURIComponent(userId)}/memberships`);
      return json<MembershipWithProduct[]>(r, 'membershipsFor');
    },
    async roleOf(userId, productId) {
      // derive from memberships endpoint to avoid a second roundtrip
      const ms = await this.membershipsFor(userId);
      return ms.find((m) => m.productId === productId)?.role ?? null;
    },
    async grantAccess({ userId, productId, role }) {
      const r = await jpost(
        `/products/${encodeURIComponent(productId)}/members/${encodeURIComponent(userId)}`,
        { role }
      );
      if (!r.ok) throw new Error(`grantAccess ${r.status}`);
    },
    async revokeAccess({ userId, productId }) {
      const r = await jdel(
        `/products/${encodeURIComponent(productId)}/members/${encodeURIComponent(userId)}`
      );
      if (!r.ok) throw new Error(`revokeAccess ${r.status}`);
    },

    // --- crashes ---
    async listGroups(q: ListQuery): Promise<ListResult> {
      const r = await req(`/crashes?${qs(q as unknown as Record<string, unknown>)}`);
      return json<ListResult>(r, 'listGroups');
    },
    async getGroup(id) {
      const r = await req(`/crashes/${encodeURIComponent(id)}`);
      if (r.status === 404) return null;
      return json<CrashGroup>(r, 'getGroup');
    },
    async getCrash(id) {
      const r = await req(`/crashes/by-crash/${encodeURIComponent(id)}`);
      if (r.status === 404) return null;
      return json<{ crash: Crash; group: CrashGroup }>(r, 'getCrash');
    },
    async downloadAttachment(id) {
      const r = await req(`/attachments/${encodeURIComponent(id)}/download`);
      if (r.status === 404) return null;
      if (!r.ok) throw new Error(`downloadAttachment ${r.status}`);
      return r;
    },
    async setStatus(id, status: Status) {
      const r = await jpost(`/crashes/${encodeURIComponent(id)}/status`, { status });
      if (!r.ok) throw new Error(`setStatus ${r.status}`);
    },
    async addNote(id, body, author): Promise<Note> {
      const r = await jpost(`/crashes/${encodeURIComponent(id)}/notes`, { body, author });
      return json<Note>(r, 'addNote');
    },
    async mergeGroups(primaryId, mergedId) {
      const r = await jpost(`/crashes/${encodeURIComponent(primaryId)}/merge`, { mergedId });
      if (!r.ok) throw new Error(`mergeGroups ${r.status}`);
    },

    // --- invitations ---
    async listInvitations() {
      const r = await req('/invitations');
      return json<Invitation[]>(r, 'listInvitations');
    },
    async createInvitation(spec: CreateInvitationSpec) {
      const r = await jpost('/invitations', spec);
      return json<Invitation>(r, 'createInvitation');
    },
    async updateInvitation(id: string, patch: UpdateInvitationSpec) {
      const r = await req(`/invitations/${encodeURIComponent(id)}`, {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(patch)
      });
      return json<Invitation>(r, 'updateInvitation');
    },
    async revokeInvitation(id: string) {
      const r = await req(`/invitations/${encodeURIComponent(id)}`, { method: 'DELETE' });
      if (!r.ok) throw new Error(`revokeInvitation ${r.status}`);
    },

    // --- symbols ---
    async listSymbols(productId, q: SymbolQuery = {}) {
      const r = await req(
        `/products/${encodeURIComponent(productId)}/symbols?${qs(q as unknown as Record<string, unknown>)}`
      );
      return json<SymbolRow[]>(r, 'listSymbols');
    },
    async uploadSymbol(productId, spec) {
      const r = await jpost(`/products/${encodeURIComponent(productId)}/symbols`, spec);
      return json<SymbolRow>(r, 'uploadSymbol');
    },
    async deleteSymbol(id) {
      const r = await jdel(`/symbols/${encodeURIComponent(id)}`);
      if (!r.ok) throw new Error(`deleteSymbol ${r.status}`);
    },

    // --- api tokens ---
    async listApiTokens(productId) {
      const r = await req(`/products/${encodeURIComponent(productId)}/api-tokens`);
      return json<ApiToken[]>(r, 'listApiTokens');
    },
    async createApiToken(productId, spec: CreateApiTokenSpec) {
      const r = await jpost(`/products/${encodeURIComponent(productId)}/api-tokens`, spec);
      return json<CreatedApiToken>(r, 'createApiToken');
    },
    async deleteApiToken(productId, id) {
      const r = await jdel(`/products/${encodeURIComponent(productId)}/api-tokens/${encodeURIComponent(id)}`);
      if (!r.ok) throw new Error(`deleteApiToken ${r.status}`);
    },

    // --- admin api tokens (product-optional) ---
    async listAllApiTokens() {
      const r = await req('/api-tokens');
      return json<ApiToken[]>(r, 'listAllApiTokens');
    },
    async createAdminApiToken(spec: CreateAdminApiTokenSpec) {
      const r = await jpost('/api-tokens', spec);
      return json<CreatedApiToken>(r, 'createAdminApiToken');
    },
    async deleteAdminApiToken(id) {
      const r = await jdel(`/api-tokens/${encodeURIComponent(id)}`);
      if (!r.ok) throw new Error(`deleteAdminApiToken ${r.status}`);
    }
  };
}
