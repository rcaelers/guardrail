# Webserver API Access Requirements

This document defines the expected authorization model for the webserver API and records the
test coverage that verifies it. It covers the routes registered by:

- `src/web/server/src/routes/auth.rs`
- `src/web/server/src/routes/impersonation.rs`
- `src/web/server/src/routes/db_api.rs`
- `src/web/server/src/routes/invite.rs`

## Principals

| Principal | Meaning |
| --- | --- |
| Anonymous | No authenticated session and no bearer token. |
| Authenticated user | A browser session backed by an existing local user. |
| Product readonly | Authenticated user with `readonly` membership on a product. |
| Product readwrite | Authenticated user with `readwrite` membership on a product. |
| Product maintainer | Authenticated user with `maintainer` membership on a product. |
| Admin | Authenticated user with global `is_admin = true`. |
| Impersonated user | A session where an admin acts as another user; authorization uses the effective user. |
| API token | Bearer token. Global tokens act like admin for token-enabled admin/product routes; product-scoped tokens are limited to that product. |

## Role Hierarchy

Product roles are ordered as:

`maintainer > readwrite > readonly`

The minimum role for a route is enough; higher roles also satisfy it. Admin bypasses product-role checks.

## Requirements

| ID | Requirement |
| --- | --- |
| A1 | Anonymous users may read only explicitly public product data. |
| A2 | Anonymous users must not read users, memberships, invitations, API tokens, private crashes, private attachments, or private symbols. |
| A3 | Anonymous users must not create, update, delete, grant, revoke, upload, merge, add notes, change state, or impersonate. |
| R1 | Readonly product users may read the product, crash groups, crash detail, annotations, attachments, and symbols for that product. |
| R2 | Readonly product users must not mutate the product, crash state, notes/annotations, symbols, memberships, invitations, or API tokens. |
| W1 | Readwrite product users may do all readonly operations. |
| W2 | Readwrite product users may add user notes/annotations and change crash state for that product. |
| W3 | Readwrite product users must not manage symbols, memberships, product API tokens, invitations, products, users, or crash-group merges. |
| M1 | Product maintainers may do all readwrite operations for maintained products. |
| M2 | Product maintainers may upload/delete symbols, manage product API tokens, grant/revoke product access, create/update/revoke product invitations, and merge crash groups for maintained products. |
| M3 | Product maintainers must not manage products they do not maintain. |
| M4 | Product maintainers must not create or update invitations that grant admin access or grant access to products they do not maintain. |
| ADM1 | Admins may perform every API operation, including global product/user/API-token management. |
| ADM2 | Admins may impersonate other users. |
| IMP1 | Only admins may start impersonation. |
| IMP2 | While impersonating, authorization uses the effective impersonated user, not the real admin, except that stopping impersonation restores the real admin. |
| PUB1 | Invite redemption endpoints are intentionally public for valid invitation codes. |
| TOK1 | API-token access is accepted only on routes that explicitly use token-aware guards. |
| TOK2 | Product-scoped API tokens may act only on their product; global API tokens may act on token-enabled admin/product routes. |

## Route Coverage Matrix

Status: `Covered` means there is an automated webserver test for the route's access decision and at least one success path where applicable.

| API call | Required access | Status | Test coverage |
| --- | --- | --- | --- |
| `GET /` | Public page | Covered | `home::test_home_page` |
| `GET /auth/login` | Public OIDC start | Covered | `oidc::tests::*` helper/unit coverage |
| `GET /auth/login/start` | Public OIDC start | Covered | `oidc::tests::*` helper/unit coverage |
| `GET /auth/oidc/callback` | Public provider callback with state | Covered | `oidc::tests::*` helper/unit coverage |
| `POST /auth/logout` | Public; clears own session | Covered | `auth::test_logout` |
| `GET /auth/real-user` | Session while impersonating | Covered | `auth::test_get_real_user` |
| `POST /auth/impersonate/{user_id}` | Admin only | Covered | `auth::test_start_impersonation` |
| `POST /auth/impersonate/stop` | Impersonating session | Covered | `auth::test_stop_impersonation` |
| `GET /products` | Public products for anonymous; role-scoped/private for authenticated | Covered | `products::test_list_products` |
| `GET /products?scope=public` | Anonymous allowed; public products only | Covered | `products::test_list_products` |
| `GET /products?scope=mine` | Authenticated/RLS-scoped user products | Covered | `products::test_list_products` |
| `POST /products` | Admin only | Covered | `products::test_create_product` |
| `GET /products/{id}` | Public product or product readonly+ | Covered | `products::test_get_product` |
| `POST /products/{id}` | Admin at DB layer; maintainer guard is not enough for update | Covered | `products::test_update_product_all_contexts`, `products::test_update_product_public_flag` |
| `DELETE /products/{id}` | Admin only | Covered | `products::test_delete_product` |
| `GET /products/{pid}/members` | RLS-scoped membership view | Covered | `products::test_list_members` |
| `POST /products/{pid}/members/{uid}` | Product maintainer | Covered | `products::test_grant_access_all_contexts` |
| `DELETE /products/{pid}/members/{uid}` | Product maintainer | Covered | `products::test_revoke_access_all_contexts` |
| `GET /users` | Admin only | Covered | `users::test_list_users` |
| `POST /users` | Admin only through API | Covered | `users::test_create_user`, `users::test_create_user_missing_email` |
| `GET /users/{id}` | Admin only through API | Covered | `users::test_get_user` |
| `POST /users/{id}` | Admin only through API | Covered | `users::test_update_user`, `users::test_update_user_missing_email` |
| `DELETE /users/{id}` | Admin only | Covered | `users::test_delete_user` |
| `POST /users/{id}/admin` | Admin only | Covered | `users::test_set_admin` |
| `GET /users/find?q=...` | Authenticated session | Covered | `users::test_find_user_requires_session` |
| `GET /users/{id}/memberships` | Own user, admin, or global token | Covered | `users::test_memberships_self_or_admin`, `users::test_memberships_with_bearer_tokens` |
| `GET /me` | Authenticated session | Covered | `users::test_get_me` |
| `GET /crashes?productId=...` | Public product or product readonly+ | Covered | `crashes::test_list_groups`, `crashes::test_list_groups_with_crash_data`, edge-case crash list tests |
| `GET /crashes/{group_id}` | Public product or product readonly+ | Covered | `crashes::test_get_group`, `crashes::test_get_group_with_related` |
| `GET /crashes/by-crash/{crash_id}` | Public product or product readonly+ | Covered | `crashes::test_get_crash_handler`, `crashes::test_get_crash_with_annotations_and_user_text` |
| `GET /attachments/{id}/download` | Public product or product readonly+ | Covered | `attachments::test_download_attachment` |
| `POST /crashes/{group_id}/status` | Product readwrite+ | Covered | `crashes::test_set_crash_status_requires_session`, `crashes::test_crash_mutations_by_product_role` |
| `POST /crashes/{group_id}/notes` | Product readwrite+ | Covered | `crashes::test_add_note_requires_session`, `crashes::test_add_note_on_group`, `crashes::test_crash_mutations_by_product_role` |
| `POST /crashes/{group_id}/merge` | Product maintainer | Covered | `crashes::test_merge_groups_requires_session`, `crashes::test_merge_groups_success`, `crashes::test_crash_mutations_by_product_role` |
| `GET /products/{pid}/symbols` | Public product or product readonly+ | Covered | `symbols::test_list_symbols`, `symbols::test_list_symbols_format_sort` |
| `POST /products/{pid}/symbols` | Product maintainer | Covered | `symbols::test_upload_symbol_all_contexts` |
| `DELETE /symbols/{id}` | Product maintainer | Covered | `symbols::test_delete_symbol_requires_session` |
| `GET /api-tokens` | Admin/global token only | Covered | `api_tokens::test_list_all_api_tokens`, `api_tokens::test_admin_api_tokens_with_bearer_tokens` |
| `POST /api-tokens` | Admin/global token only | Covered | `api_tokens::test_create_admin_api_token`, `api_tokens::test_admin_api_tokens_with_bearer_tokens` |
| `PATCH /api-tokens/{id}` | Admin/global token only | Covered | `api_tokens::test_update_admin_api_token`, `api_tokens::test_admin_api_tokens_with_bearer_tokens` |
| `DELETE /api-tokens/{id}` | Admin/global token only | Covered | `api_tokens::test_delete_admin_api_token`, `api_tokens::test_admin_api_tokens_with_bearer_tokens` |
| `GET /api-tokens/entitlements` | Admin/global token only | Covered | `api_tokens::test_list_entitlements` |
| `GET /products/{pid}/api-tokens` | Product maintainer or matching/global token | Covered | `api_tokens::test_list_product_api_tokens_all_contexts`, `api_tokens::test_product_api_tokens_with_bearer_tokens` |
| `POST /products/{pid}/api-tokens` | Product maintainer or matching/global token | Covered | `api_tokens::test_create_product_api_token_all_contexts`, `api_tokens::test_product_api_tokens_with_bearer_tokens` |
| `DELETE /products/{pid}/api-tokens/{id}` | Product maintainer or matching/global token | Covered | `api_tokens::test_delete_product_api_token_all_contexts`, `api_tokens::test_product_api_tokens_with_bearer_tokens` |
| `GET /invitations` | Authenticated session; list scoped by admin/maintained products | Covered | `invitations::test_list_invitations_requires_session` |
| `POST /invitations` | Admin, product maintainer within maintained products, or entitled token | Covered | `invitations::test_create_invitation_admin`, `invitations::test_create_invitation_non_admin_restrictions`, token invitation tests |
| `PUT /invitations/{id}` | Admin or maintainer overlap; grants limited to maintained products | Covered | `invitations::test_update_invitation_requires_session`, `invitations::test_update_invitation_success` |
| `DELETE /invitations/{id}` | Admin, creator, or maintainer overlap | Covered | `invitations::test_revoke_invitation_requires_session`, `invitations::test_revoke_invitation_existing` |
| `GET /invitations/redeem/{code}` | Public valid-code lookup | Covered | `invitations::test_get_invite_info`, pending/setup-url tests |
| `POST /invitations/redeem/{code}` | Public valid-code redemption | Covered | `invitations::test_redeem_invite_json_*` |
| `GET /invite/{code}` | Public valid-code form | Covered | `invitations::test_show_invite_form*` |
| `POST /invite/{code}` | Public valid-code form submission | Covered | `invitations::test_redeem_invite_form*` |

## Notes

- Product data visibility is primarily enforced by SurrealDB RLS. Tests assert both HTTP status and response contents where leakage would otherwise be easy to miss.
- Product mutations now use explicit route guards for operations whose URL does not directly carry a product ID (`/crashes/{group_id}/...`, `/symbols/{id}`).
- `POST /products/{id}` intentionally remains admin-only at the database permission layer even though the route guard accepts maintainers; tests document the resulting `404` for maintainer-only product updates.
- There is no separate generic "add annotation" API. User annotations are represented by `POST /crashes/{group_id}/notes`.
