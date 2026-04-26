# GUARDRAIL

Gathering Unanticipated Anomaly Reports and Diagnostics for Retrospective Analysis, Insight, and Learnings.

_Every failure tells a story. Guardrail collects the chapters._

## SurrealKit

Guardrail now includes a SurrealKit project scaffold under `database/`.

- Desired-state schema lives in `database/schema/guardrail.surql`
- SurrealKit metadata lives in `database/setup.surql`
- Seed data lives in `database/seed.surql`
- Schema smoke tests live in `database/tests/`

For local development with the Docker SurrealDB service, set these environment variables:

```sh
DATABASE_HOST=ws://localhost:8000
DATABASE_NAMESPACE=guardrail
DATABASE_NAME=guardrail
DATABASE_USER=root
DATABASE_PASSWORD=root
```

Common commands:

```sh
surrealkit sync
surrealkit test
surrealkit rollout status
```

Shared and Kubernetes-managed databases should use SurrealKit rollouts instead of direct sync.
Guardrail now commits rollout manifests under `database/rollouts/`, and ArgoCD runs them as:

- `PreSync`: `surrealkit rollout start <latest-manifest>`
- `PostSync`: `surrealkit rollout complete <latest-manifest>`

To create a new rollout manifest after schema changes:

```sh
surrealkit rollout plan --name describe_the_change
```

The rollout plan updates `database/rollouts/` and `database/snapshots/`. Commit both.

## Web UI + mock data

The SvelteKit UI under `src/web/ui/` can run against three different backends.
Pick the one you need:

### 1. In-memory TypeScript mock (no backend at all)

Fastest path when you just want to see the UI. The mock adapter lives in
`src/web/ui/src/lib/adapters/mock.ts` and seeds itself deterministically.

```sh
cd src/web/ui
bun install
bun run dev
```

The adapter is selected at boot in `src/lib/adapters/index.ts`: if
`GUARDRAIL_API_URL` is unset, the TS mock is used.

### 2. Standalone Rust mock server (`mock_server`)

Serves the same JSON from an in-memory copy of `src/web/server/mock/seed.json`.
Useful for exercising the HTTP adapter without SurrealDB.

```sh
# terminal 1
cargo run -p web --bin mock_server
# listens on http://127.0.0.1:4500/api/v1

# terminal 2
cd src/web/ui
GUARDRAIL_API_URL=http://127.0.0.1:4500/api/v1 bun run dev
```

Routes and tests live in `src/web/server/src/mock_api.rs` and
`src/web/server/tests/mock_api.rs`.

### 3. Real SurrealDB (`db_server` + `import_mock`)

End-to-end path: SurrealDB holds the data, the Rust server queries it, and
the UI talks to the server.

One-time setup (schema + seed):

```sh
# apply schema — first setup.surql, then the app schema
surrealkit --host ws://localhost:8000 --user root --pass root \
           --ns guardrail --db guardrail apply database/setup.surql
surrealkit --host ws://localhost:8000 --user root --pass root \
           --ns guardrail --db guardrail apply database/schema/guardrail.surql

# import the mock data set (idempotent; clears the data tables first)
cargo run -p web --bin import_mock
#   flags: --host --user --pass --ns --db --seed <path>
```

Then run the stack:

```sh
# terminal 1
cargo run -p web --bin db_server
# listens on http://127.0.0.1:4500/api/v1
# env overrides: GUARDRAIL_DB_HOST / USER / PASS / NS / NAME, GUARDRAIL_API_ADDR

# terminal 2
cd src/web/ui
GUARDRAIL_API_URL=http://127.0.0.1:4500/api/v1 bun run dev
```

Key files:

- `src/web/server/src/db_api.rs` — REST handlers, SurrealDB queries
- `src/web/server/src/bin/import_mock.rs` — seed importer
- `src/web/server/src/bin/db_server.rs` — standalone wrapper
- `src/web/server/src/bin/mock_server.rs` — in-memory alternative
- `src/web/server/mock/seed.json` — source of truth for mock data

Test Workrave API tokens:

- Minidump upload: `7aIbCC27SR-HthcWDoIrLLDYziZ1UGQrX5Je2KgXq2ur5BOPVe3Idm4MdnYOAbHfchtm-O9qO7BSqtmyV9oLpQ==`
- Symbols upload: `iLKwi56cQuGBp30Z90L2BJgl8avchNqIgm9ZJajwVb_90Kr0cxIMjEfLa1pzqTMrkNofWZN0A8Uv3pSyFwYSHA==`

These test tokens are documented here for manual dev/testing only. The mock
seed stores only `tokenId` and `tokenHash`, not the raw token values.

The full `web` binary (`cargo run -p web`) also mounts `/api/v1` against
the configured SurrealDB, but it expects the rest of the production
stack (settings, TLS, OIDC, WebAuthn, sessions). `db_server` is the
lightweight dev path.

Schema notes: crash detail lives on `crashes.report` (FLEXIBLE object);
`crash_groups` carries only fingerprint + signal + count + status +
aggregations. Groups and crashes use stable string record ids
(`crash_groups:⟨GR-####⟩`, `crashes:⟨CR-####-###⟩`); the API strips the
table prefix before returning to the UI. User notes and system
annotations share the `annotations` table, discriminated by `source`
(`submission` | `script` | `user`).

## Pocket ID Local Dev

`dev/docker-compose.yml` now includes a local Pocket ID service, a small Caddy reverse proxy for HTTPS, and a one-shot bootstrap container.
The Pocket ID service is configured directly from compose environment variables for local development.

Bring the local stack up with:

```sh
docker compose --parallel 1 -f dev/docker-compose.yml up -d
```

On first start, Pocket ID boots with a static admin API key and token-based user signups enabled.
The `pocket-id-setup` container then uses that API key to:

- create the initial `admin` account if it does not exist
- generate a one-time admin login code
- create or update a `Guardrail` confidential OIDC client
- generate the Guardrail client secret
- write local connection details for both Guardrail and the Pocket ID admin login

The generated Guardrail OIDC client uses:

- callback URL: `https://guardrail.home.krandor.org:4433/auth/oidc/callback`
- logout URL: `https://guardrail.home.krandor.org:4433/`
- launch URL: `https://guardrail.home.krandor.org:4433/`

No Pocket ID admin UI setup is required for the local test stack.
The browser-facing URL is `https://guardrail.home.krandor.org:1443`, proxied to Pocket ID by Caddy.

Generated artifacts are written under `dev/_private/pocket-id/`:

- `guardrail-oidc.env`: generic `GUARDRAIL_AUTH_OIDC_*` issuer, client id, and client secret values for Guardrail
- `admin-login.env`: admin id, username, and a one-time login URL

Export the generated OIDC settings before starting the Rust web server:

```sh
set -a
source dev/_private/pocket-id/guardrail-oidc.env
set +a
```

The local Pocket ID config also sets `ALLOW_USER_SIGNUPS=withToken`, so invite-style signup links are available without any extra UI configuration.
Because Caddy uses a local development certificate, your browser or local OIDC client may require you to trust Caddy's local CA before `https://guardrail.home.krandor.org:1443` is accepted cleanly.

## Command-Line Tool

`/app/guardrailctl` connects directly to SurrealDB using the configured database
credentials. It does not call the HTTP API. The command shape is:

```sh
guardrailctl <invite|token|product> <list|create|remove|...>
```

Create an initial admin invitation from the running compose stack:

```sh
docker compose -f dev/docker-compose.yml exec web \
  /app/guardrailctl -C /config invite create --admin
```

For a product-scoped invitation:

```sh
docker compose -f dev/docker-compose.yml exec web \
  /app/guardrailctl -C /config invite create --grant '<product-id>:maintainer'
```

To create an API token with the `invitation-create` entitlement:

```sh
docker compose -f dev/docker-compose.yml exec web \
  /app/guardrailctl -C /config token create --entitlement invitation-create
```

The invite command can also create that token alongside the invite:

```sh
docker compose -f dev/docker-compose.yml exec web \
  /app/guardrailctl -C /config invite create --admin --create-api-key
```

The same executable can be used through Kubernetes by executing it in a pod
that has the Guardrail config mounted. The examples below assume the web
deployment is named `guardrail-web` and config is mounted at `/config`.

```sh
kubectl exec deploy/guardrail-web -- \
  /app/guardrailctl -C /config invite create --admin
```

Use `-n <namespace>` when Guardrail is not running in your current namespace:

```sh
kubectl exec -n guardrail deploy/guardrail-web -- \
  /app/guardrailctl -C /config invite create --admin
```

For a multi-container pod, add `-c <container-name>`:

```sh
kubectl exec -n guardrail deploy/guardrail-web -c web -- \
  /app/guardrailctl -C /config invite list
```

Common Kubernetes commands:

```sh
kubectl exec -n guardrail deploy/guardrail-web -- \
  /app/guardrailctl -C /config token create --entitlement invitation-create

kubectl exec -n guardrail deploy/guardrail-web -- \
  /app/guardrailctl -C /config product list

kubectl exec -n guardrail deploy/guardrail-web -- \
  /app/guardrailctl -C /config invite remove '<invite-id>'
```

For scripting, pass `--json` before the resource command:

```sh
kubectl exec -n guardrail deploy/guardrail-web -- \
  /app/guardrailctl -C /config --json invite list
```

Useful inspection and cleanup commands:

```sh
docker compose -f dev/docker-compose.yml exec web \
  /app/guardrailctl -C /config invite list

docker compose -f dev/docker-compose.yml exec web \
  /app/guardrailctl -C /config token list

docker compose -f dev/docker-compose.yml exec web \
  /app/guardrailctl -C /config product list
```

Create a product directly in the database:

```sh
docker compose -f dev/docker-compose.yml exec web \
  /app/guardrailctl -C /config product create --name Workrave --public
```

Remove or revoke records by id:

```sh
docker compose -f dev/docker-compose.yml exec web \
  /app/guardrailctl -C /config invite remove '<invite-id>'

docker compose -f dev/docker-compose.yml exec web \
  /app/guardrailctl -C /config token revoke '<token-id>'

docker compose -f dev/docker-compose.yml exec web \
  /app/guardrailctl -C /config product remove '<product-id>'
```

Add `--json` to any command for machine-readable output.

## Manual Uploads

Upload a test minidump to the local ingestion service:

```sh
curl -vv -X POST "localhost:8081/api/minidump/upload?api_key=7aIbCC27SR-HthcWDoIrLLDYziZ1UGQrX5Je2KgXq2ur5BOPVe3Idm4MdnYOAbHfchtm-O9qO7BSqtmyV9oLpQ==" \
  --insecure \
  -F"upload_file_minidump=@dev/6fda4029-be94-43ea-90b6-32fe2a78074a.dmp;type=application/octet-stream" \
  -F"product=workrave;type=text/plain" \
  -F"version=1.11;type=text/plain" \
  -F"channel=rc;type=text/plain" \
  -F"commit=x;type=text/plain" \
  -F"build_id=x;type=text/plain"
```

Upload test symbols to the local API service:

```sh
curl -X POST "localhost:8080/api/symbols/upload?product=workrave" \
  --insecure \
  -H "Authorization: Bearer iLKwi56cQuGBp30Z90L2BJgl8avchNqIgm9ZJajwVb_90Kr0cxIMjEfLa1pzqTMrkNofWZN0A8Uv3pSyFwYSHA==" \
  -Fupload_file_symbols=@dev/crash.sym \
  -Fproduct=Workrave \
  -Fversion=1.11.0.rc.4 \
  -Fchannel=rc \
  -Fcommit=0059ba745f1648d54848e75075cccbe954a8d8f6 \
  -Fbuild_id=1
```

## Todo

- [ ] API
  - [ ] Swagger documentation
  - [ ] Tests
    - [ ] Token generation
- [ ] Job execution
  - [ ] Remove minidump after processing
  - [ ] Periodically clean up left over minidumps
  - [ ] Tests
- [ ] Web UI
  - [ ] Authentication
    - [ ] Invitations
    - [ ] User roles
 [ ] Infra
  - [ ] K8S deployment
