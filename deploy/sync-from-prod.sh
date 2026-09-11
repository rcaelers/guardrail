#!/usr/bin/env bash
# Mirror production Guardrail data into the local docker compose stack, so the
# compose setup in this directory holds the same crashes, symbols, products and
# users as the k8s deployment.
#
#   docker compose -f deploy/docker-compose.yml up -d
#   deploy/sync-from-prod.sh
#
# Production is only ever READ: the script port-forwards to the cluster, pulls a
# SurrealDB export and the Garage bucket contents, and writes both into the
# local stack. The LOCAL database and bucket are wiped first — everything in the
# compose stack is replaced by what production has.
#
# Logging in afterwards: production users are bound to the production identity
# provider's OIDC `sub`, which means nothing to the local pocket-id. The script
# therefore clears `sub` on every imported user, which lets the app's existing
# first-login linking claim an account by email/username, and makes sure an
# account matching the local pocket-id admin exists and is an admin.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR

# --- production (read-only) -------------------------------------------------
PROD_DB_NAMESPACE="surrealdb"
PROD_DB_SERVICE="svc/surrealdb"
PROD_DB_PORT=8000
PROD_DB_SECRET="surrealdb-secrets"
PROD_GARAGE_NAMESPACE="garage"
PROD_GARAGE_SERVICE="svc/garage"
PROD_GARAGE_PORT=3900
PROD_GARAGE_SECRET="garage-secrets"
PROD_POCKET_ID_NAMESPACE="guardrail"
PROD_POCKET_ID_SERVICE="svc/pocket-id"
PROD_POCKET_ID_PORT=80
PROD_POCKET_ID_SECRET="pocket-id-secrets"

# Namespace/database/bucket names are identical on both sides
# (workrave-infra apps/guardrail-config/base/01-{database,object-storage}.yaml).
SURREAL_NS="guardrail"
SURREAL_DB="guardrail"
BUCKET="guardrail"

# --- local ------------------------------------------------------------------
LOCAL_DB_URL="http://127.0.0.1:8000"
LOCAL_S3_URL="http://127.0.0.1:3900"
# Pocket ID publishes no host port of its own; Caddy fronts it on 1443 with the
# dev self-signed certificate, hence the --insecure in pid_api below.
LOCAL_POCKET_ID_URL="https://guardrail.home.krandor.org:1443"
# Pocket ID paginates /api/users; ask for more than any dev deployment will have.
PID_LIST_PATH="/api/users?pagination.limit=500"
LOCAL_DB_USER="root"
COMPOSE_PROJECT="guardrail-dev"
COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yml"
ENV_FILE="$SCRIPT_DIR/.env"

# --- options ----------------------------------------------------------------
DO_DB=1
DO_STORAGE=1
DO_FIXUPS=1
DO_USERS=1
FLUSH_QUEUES=1
LOGIN_TTL="168h"
KEEP_DUMP=0
DUMP_FILE=""
KUBE_CONTEXT=""
ADMIN_USERNAME="admin"
ADMIN_EMAIL="admin@guardrail.home.krandor.org"
ASSUME_YES=0
DRY_RUN=0

usage() {
  sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
  cat <<'EOF'

Options:
  --db-only            Sync only the SurrealDB contents
  --storage-only       Sync only the Garage bucket contents
  --dump FILE          Import FILE instead of exporting from production
  --keep-dump          Keep the exported .surql instead of deleting it
  --no-fixups          Skip the login fix-ups (imported users stay bound to
                       the production identity provider and cannot log in)
  --no-users           Do not mirror production's Pocket ID accounts locally
  --login-ttl TTL      Lifetime of the one-time login codes (default: 168h)
  --keep-queues        Do not flush valkey (leaves production job queue state)
  --admin-username U   Local pocket-id admin username (default: admin)
  --admin-email E      Local pocket-id admin email
                       (default: admin@guardrail.home.krandor.org)
  --context NAME       kubectl context to read production from
  -y, --yes            Do not ask for confirmation
  -n, --dry-run        Show what would happen, change nothing
  -h, --help           Show this help
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --db-only) DO_STORAGE=0 ;;
    --storage-only) DO_DB=0; DO_FIXUPS=0; DO_USERS=0 ;;
    --dump) DUMP_FILE="${2:?--dump needs a file}"; shift ;;
    --keep-dump) KEEP_DUMP=1 ;;
    --no-fixups) DO_FIXUPS=0 ;;
    --no-users) DO_USERS=0 ;;
    --login-ttl) LOGIN_TTL="${2:?--login-ttl needs a value}"; shift ;;
    --keep-queues) FLUSH_QUEUES=0 ;;
    --admin-username) ADMIN_USERNAME="${2:?--admin-username needs a value}"; shift ;;
    --admin-email) ADMIN_EMAIL="${2:?--admin-email needs a value}"; shift ;;
    --context) KUBE_CONTEXT="${2:?--context needs a value}"; shift ;;
    -y|--yes) ASSUME_YES=1 ;;
    -n|--dry-run) DRY_RUN=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

log()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }
# Runs a command quietly, or describes it under --dry-run. Callers must not
# redirect its output, or the dry-run description goes to /dev/null with it.
run()  { if [ "$DRY_RUN" = 1 ]; then printf '   would run: %s\n' "$*"; else "$@" >/dev/null; fi; }
# Announces a step, phrased for whichever mode we are in.
step() { if [ "$DRY_RUN" = 1 ]; then log "would $1"; else log "$2"; fi; }

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------

need() { command -v "$1" >/dev/null 2>&1 || die "$1 is required but not installed"; }
need kubectl
need curl
need jq
need docker
[ "$DO_STORAGE" = 1 ] && need rclone

kubectl_() {
  if [ -n "$KUBE_CONTEXT" ]; then
    kubectl --context "$KUBE_CONTEXT" "$@"
  else
    kubectl "$@"
  fi
}

[ -f "$ENV_FILE" ] || die "$ENV_FILE not found — run deploy/generate-secrets.sh first"
set -a
# shellcheck source=/dev/null
. "$ENV_FILE"
set +a
: "${SURREALDB_ROOT_PASSWORD:?SURREALDB_ROOT_PASSWORD missing from $ENV_FILE}"
: "${GARAGE_ACCESS_KEY:?GARAGE_ACCESS_KEY missing from $ENV_FILE}"
: "${GARAGE_SECRET_KEY:?GARAGE_SECRET_KEY missing from $ENV_FILE}"

# Guard: only ever write into the compose stack from this directory. Without
# this a stray port mapping could point 127.0.0.1:8000 at something else.
compose() { docker compose -p "$COMPOSE_PROJECT" -f "$COMPOSE_FILE" "$@"; }
running() { [ "$(docker inspect -f '{{.State.Running}}' "$1" 2>/dev/null)" = "true" ]; }
for c in surrealdb garage; do
  running "$c" || die "local container '$c' is not running — start the stack with:
    docker compose -f $COMPOSE_FILE up -d"
  [ "$(docker inspect -f '{{index .Config.Labels "com.docker.compose.project"}}' "$c")" = "$COMPOSE_PROJECT" ] \
    || die "container '$c' does not belong to the $COMPOSE_PROJECT compose project — refusing to write to it"
done

context_name="${KUBE_CONTEXT:-$(kubectl config current-context)}"
log "production (read-only): kube context '$context_name'"
log "local target: $LOCAL_DB_URL (ns=$SURREAL_NS db=$SURREAL_DB), $LOCAL_S3_URL (bucket=$BUCKET)"

if [ "$ASSUME_YES" != 1 ] && [ "$DRY_RUN" != 1 ]; then
  echo
  echo "This REPLACES the local compose stack's data:"
  [ "$DO_DB" = 1 ]      && echo "  - SurrealDB database '$SURREAL_DB' is dropped and reimported from production"
  [ "$DO_STORAGE" = 1 ] && echo "  - Garage bucket '$BUCKET' is mirrored from production (local-only objects deleted)"
  [ "$DO_USERS" = 1 ] && echo "  - local Pocket ID accounts are replaced by production's (local-only ones are deleted)"
  [ "$FLUSH_QUEUES" = 1 ] && echo "  - valkey is flushed"
  echo "Production itself is only read."
  echo
  # Without a terminal there is nobody to answer, and read would block forever
  # (or take EOF as "no"). Fail loudly rather than hang or silently abort.
  if [ ! -t 0 ]; then
    die "stdin is not a terminal — re-run with --yes to confirm non-interactively"
  fi
  printf 'Continue? [y/N] '
  read -r reply
  case "$reply" in [yY]*) ;; *) echo "Aborted."; exit 1 ;; esac
fi

# ---------------------------------------------------------------------------
# Port-forwards
# ---------------------------------------------------------------------------

PF_PIDS=()
TMPDIR_SYNC="$(mktemp -d)"
cleanup() {
  for pid in "${PF_PIDS[@]:-}"; do
    [ -n "$pid" ] && kill "$pid" 2>/dev/null || true
  done
  if [ "$KEEP_DUMP" = 1 ] && [ -f "$TMPDIR_SYNC/prod.surql" ]; then
    kept="./guardrail-prod-$(date +%Y%m%d-%H%M%S).surql"
    mv "$TMPDIR_SYNC/prod.surql" "$kept"
    log "kept dump: $kept"
  fi
  rm -rf "$TMPDIR_SYNC"
}
trap cleanup EXIT

free_port() {
  python3 - <<'PY'
import socket
s = socket.socket()
s.bind(('127.0.0.1', 0))
print(s.getsockname()[1])
s.close()
PY
}

port_forward() { # namespace target remote_port -> echoes local port
  local ns="$1" target="$2" remote="$3" local_port
  local_port="$(free_port)"
  kubectl_ port-forward -n "$ns" "$target" "${local_port}:${remote}" >"$TMPDIR_SYNC/pf-$ns.log" 2>&1 &
  PF_PIDS+=("$!")
  for _ in $(seq 1 50); do
    if grep -q "Forwarding from" "$TMPDIR_SYNC/pf-$ns.log" 2>/dev/null; then
      echo "$local_port"
      return 0
    fi
    sleep 0.2
  done
  die "port-forward to $target in namespace $ns did not become ready:
$(cat "$TMPDIR_SYNC/pf-$ns.log")"
}

secret() { # namespace name key
  kubectl_ get secret -n "$1" "$2" -o "jsonpath={.data.$3}" | base64 -d
}

# ---------------------------------------------------------------------------
# SurrealDB
# ---------------------------------------------------------------------------

surreal_sql() { # url user pass [db] sql
  local url="$1" user="$2" pass="$3" db="$4" sql="$5"
  local -a headers=(-H "surreal-ns: $SURREAL_NS" -H "Accept: application/json")
  [ -n "$db" ] && headers+=(-H "surreal-db: $db")
  curl -sS --fail-with-body -u "$user:$pass" "${headers[@]}" \
    --data-binary "$sql" "$url/sql"
}

assert_ok() { # json_response context
  local bad
  bad="$(printf '%s' "$1" | jq -r '[.[] | select(.status != "OK")] | length')"
  [ "$bad" = "0" ] || die "$2 failed: $(printf '%s' "$1" | jq -c '[.[] | select(.status != "OK")]')"
}

if [ "$DO_DB" = 1 ]; then
  dump="$TMPDIR_SYNC/prod.surql"

  if [ -n "$DUMP_FILE" ]; then
    [ -f "$DUMP_FILE" ] || die "dump file not found: $DUMP_FILE"
    log "using existing dump $DUMP_FILE"
    cp "$DUMP_FILE" "$dump"
  elif [ "$DRY_RUN" = 1 ]; then
    log "would export ns=$SURREAL_NS db=$SURREAL_DB from $PROD_DB_SERVICE in namespace $PROD_DB_NAMESPACE"
  else
    log "exporting production database…"
    db_port="$(port_forward "$PROD_DB_NAMESPACE" "$PROD_DB_SERVICE" "$PROD_DB_PORT")"
    prod_db_user="$(secret "$PROD_DB_NAMESPACE" "$PROD_DB_SECRET" rootUser)"
    prod_db_pass="$(secret "$PROD_DB_NAMESPACE" "$PROD_DB_SECRET" rootPassword)"
    [ -n "$prod_db_user" ] && [ -n "$prod_db_pass" ] \
      || die "could not read $PROD_DB_SECRET from namespace $PROD_DB_NAMESPACE"
    curl -sS --fail-with-body -u "$prod_db_user:$prod_db_pass" \
      -H "surreal-ns: $SURREAL_NS" -H "surreal-db: $SURREAL_DB" \
      -H "Accept: application/octet-stream" \
      "http://127.0.0.1:${db_port}/export" -o "$dump" \
      || die "export from production failed"
    unset prod_db_pass
    [ -s "$dump" ] || die "production export is empty — wrong namespace/database?"
    log "exported $(wc -c <"$dump" | tr -d ' ') bytes"
  fi

  if [ "$DRY_RUN" = 1 ]; then
    log "would drop and reimport local database '$SURREAL_DB'"
  else
    log "replacing local database '$SURREAL_DB'…"
    # The export defines tables without OVERWRITE, so it must land in an empty
    # database; dropping it also clears rows production no longer has.
    out="$(surreal_sql "$LOCAL_DB_URL" "$LOCAL_DB_USER" "$SURREALDB_ROOT_PASSWORD" "" \
      "DEFINE NAMESPACE IF NOT EXISTS $SURREAL_NS; USE NS $SURREAL_NS; REMOVE DATABASE IF EXISTS $SURREAL_DB; DEFINE DATABASE $SURREAL_DB;")"
    assert_ok "$out" "local database reset"

    curl -sS --fail-with-body -u "$LOCAL_DB_USER:$SURREALDB_ROOT_PASSWORD" \
      -H "surreal-ns: $SURREAL_NS" -H "surreal-db: $SURREAL_DB" -H "Accept: application/json" \
      --data-binary "@$dump" "$LOCAL_DB_URL/import" >/dev/null \
      || die "import into the local database failed"

    counts="$(surreal_sql "$LOCAL_DB_URL" "$LOCAL_DB_USER" "$SURREALDB_ROOT_PASSWORD" "$SURREAL_DB" \
      "SELECT count() FROM users GROUP ALL; SELECT count() FROM products GROUP ALL; SELECT count() FROM crash_groups GROUP ALL; SELECT count() FROM crashes GROUP ALL;")"
    log "imported: $(printf '%s' "$counts" | jq -r '
      def n: (.result[0].count // 0);
      "users=\(.[0]|n) products=\(.[1]|n) crash_groups=\(.[2]|n) crashes=\(.[3]|n)"')"
  fi

  if [ "$DO_FIXUPS" = 1 ]; then
    if [ "$DRY_RUN" = 1 ]; then
      log "would clear OIDC subs and ensure local admin '$ADMIN_USERNAME' <$ADMIN_EMAIL>"
    else
      log "rebinding users to the local identity provider…"
      # Production `sub` values belong to the production pocket-id. Clearing
      # them puts every account back in the "unlinked" state the app claims by
      # email, then username, on first login (src/web/server/src/oidc.rs); the
      # Pocket ID step below then binds each one explicitly.
      # Only worth clearing when nothing else will fix the bindings: the Pocket
      # ID mirror below recreates accounts under production's own ids, which
      # keeps the imported `sub` values correct, and re-points the rest.
      if [ "$DO_USERS" = 1 ]; then
        log "leaving OIDC subs to the Pocket ID mirror"
      else
        cleared="$(surreal_sql "$LOCAL_DB_URL" "$LOCAL_DB_USER" "$SURREALDB_ROOT_PASSWORD" "$SURREAL_DB" \
          "UPDATE users SET sub = NONE RETURN NONE;")"
        assert_ok "$cleared" "clearing OIDC subs"
      fi

      # The break-glass admin is only worth creating when the import carries no
      # admin of its own. Creating it unconditionally left a local-only account
      # with no counterpart in production, reappearing after every sync.
      probe="$(surreal_sql "$LOCAL_DB_URL" "$LOCAL_DB_USER" "$SURREALDB_ROOT_PASSWORD" "$SURREAL_DB" "
        SELECT VALUE id FROM users WHERE email = '$ADMIN_EMAIL' OR username = '$ADMIN_USERNAME';
        SELECT VALUE id FROM users WHERE is_admin = true;")"
      assert_ok "$probe" "looking for an existing admin"
      matching="$(printf '%s' "$probe" | jq -r '.[0].result | length')"
      admins="$(printf '%s' "$probe" | jq -r '.[1].result | length')"

      if [ "$matching" -gt 0 ]; then
        promote="$(surreal_sql "$LOCAL_DB_URL" "$LOCAL_DB_USER" "$SURREALDB_ROOT_PASSWORD" "$SURREAL_DB" "
          UPDATE users SET is_admin = true
          WHERE email = '$ADMIN_EMAIL' OR username = '$ADMIN_USERNAME' RETURN NONE;")"
        assert_ok "$promote" "promoting $ADMIN_USERNAME"
        log "'$ADMIN_USERNAME' is present in the imported data — kept as an admin"
      elif [ "$admins" -gt 0 ]; then
        log "imported data already has $admins admin(s) — no fallback account created"
      else
        created="$(surreal_sql "$LOCAL_DB_URL" "$LOCAL_DB_USER" "$SURREALDB_ROOT_PASSWORD" "$SURREAL_DB" "
          CREATE users SET username = '$ADMIN_USERNAME', email = '$ADMIN_EMAIL',
                           name = 'Guardrail Admin', is_admin = true RETURN NONE;")"
        assert_ok "$created" "creating the fallback admin"
        log "no admin in the imported data — created '$ADMIN_USERNAME' <$ADMIN_EMAIL>"
      fi
    fi
  fi
fi

# ---------------------------------------------------------------------------
# Pocket ID accounts
# ---------------------------------------------------------------------------
# Production's Pocket ID is the source of truth for identities, not Guardrail's
# users table: the two disagree (Guardrail stores "robc@krandor.org" where
# Pocket ID calls the same person "admin"), and it is Pocket ID's username that
# a person actually signs in with. So mirror production's Pocket ID directly
# rather than reconstructing accounts from the Guardrail rows.
#
# Accounts are recreated under production's own ids where possible, which keeps
# the `sub` values carried in the imported Guardrail rows valid as they stand.

pid_api() { # base_url api_key method path [body]
  pid_base="$1"; pid_key="$2"; pid_method="$3"; pid_path="$4"; pid_body="${5-}"
  if [ -n "$pid_body" ]; then
    curl -sS --fail-with-body -k -X "$pid_method" \
      -H "X-API-KEY: $pid_key" -H 'Content-Type: application/json' \
      -d "$pid_body" "$pid_base$pid_path"
  else
    curl -sS --fail-with-body -k -X "$pid_method" \
      -H "X-API-KEY: $pid_key" "$pid_base$pid_path"
  fi
}

# Pocket ID's own account for STATIC_API_KEY, present on both sides under
# different generated names; never mirrored and never pruned.
STATIC_API_USER_ID="00000000-0000-0000-0000-000000000000"

if [ "$DO_USERS" = 1 ] && [ "$DO_DB" = 1 ]; then
  if [ "$DRY_RUN" = 1 ]; then
    log "would mirror production's Pocket ID accounts into $LOCAL_POCKET_ID_URL"
  elif [ -z "${POCKET_ID_STATIC_API_KEY:-}" ]; then
    warn "POCKET_ID_STATIC_API_KEY missing from $ENV_FILE — skipping Pocket ID accounts"
  else
    log "mirroring Pocket ID accounts from production..."
    pid_port="$(port_forward "$PROD_POCKET_ID_NAMESPACE" "$PROD_POCKET_ID_SERVICE" "$PROD_POCKET_ID_PORT")"
    prod_pid_key="$(secret "$PROD_POCKET_ID_NAMESPACE" "$PROD_POCKET_ID_SECRET" STATIC_API_KEY)"
    [ -n "$prod_pid_key" ] \
      || die "could not read $PROD_POCKET_ID_SECRET from namespace $PROD_POCKET_ID_NAMESPACE"

    prod_users="$(pid_api "http://127.0.0.1:${pid_port}" "$prod_pid_key" GET "$PID_LIST_PATH")" \
      || die "could not list production Pocket ID users"
    local_users="$(pid_api "$LOCAL_POCKET_ID_URL" "$POCKET_ID_STATIC_API_KEY" GET "$PID_LIST_PATH")" \
      || die "could not list local Pocket ID users"

    login_file="$SCRIPT_DIR/_private/pocket-id/user-logins.env"
    : >"$login_file.tmp"
    mirrored=0
    skipped=0
    kept_ids="$STATIC_API_USER_ID"

    # One JSON object per line. Not TSV: tab is IFS whitespace, so `read`
    # collapses adjacent tabs and an account with an empty field (no last
    # name, no email) shifts every column after it.
    while IFS= read -r row; do
      [ -n "$row" ] || continue
      pid="$(printf '%s' "$row" | jq -r '.id')"
      [ "$pid" = "$STATIC_API_USER_ID" ] && continue
      username="$(printf '%s' "$row" | jq -r '.username')"
      email="$(printf '%s' "$row" | jq -r '.email // ""')"
      is_admin="$(printf '%s' "$row" | jq -r '.isAdmin // false')"

      payload="$(printf '%s' "$row" | jq -c '
        {
          id,
          username,
          email: (if (.email // "") == "" then null else .email end),
          emailVerified: true,
          firstName: (.firstName // ""),
          lastName: (.lastName // ""),
          displayName: (if (.displayName // "") == "" then .username else .displayName end),
          isAdmin: (.isAdmin // false),
          disabled: (.disabled // false)
        }')"

      # Same id, else same username or email: updating in place keeps any
      # passkey already registered against that local account.
      target="$(printf '%s' "$local_users" | jq -r \
        --arg id "$pid" --arg username "$username" --arg email "$email" '
          [ .data[]?
            | select(.id == $id or .username == $username
                     or (($email != "") and .email == $email)) ]
          | first | .id // empty')"

      if [ -n "$target" ]; then
        if response="$(pid_api "$LOCAL_POCKET_ID_URL" "$POCKET_ID_STATIC_API_KEY" \
                        PUT "/api/users/$target" "$payload" 2>&1)"; then
          local_id="$target"
          action="updated"
        else
          warn "could not update local Pocket ID account '$username': $response"
          skipped=$((skipped + 1)); continue
        fi
      else
        if response="$(pid_api "$LOCAL_POCKET_ID_URL" "$POCKET_ID_STATIC_API_KEY" \
                        POST /api/users "$payload" 2>&1)"; then
          local_id="$(printf '%s' "$response" | jq -r '.id // empty')"
          action="created"
        else
          warn "could not create local Pocket ID account '$username': $response"
          skipped=$((skipped + 1)); continue
        fi
      fi

      if [ -z "$local_id" ]; then
        warn "Pocket ID returned no id for '$username'"
        skipped=$((skipped + 1)); continue
      fi
      kept_ids="$kept_ids $local_id"

      token="$(pid_api "$LOCAL_POCKET_ID_URL" "$POCKET_ID_STATIC_API_KEY" \
        POST "/api/users/$local_id/one-time-access-token" \
        "$(jq -nc --arg ttl "$LOGIN_TTL" '{ttl: $ttl}')" | jq -r '.token // empty')"
      if [ -n "$token" ]; then
        printf '%s=%s/lc/%s\n' "$username" "$LOCAL_POCKET_ID_URL" "$token" >>"$login_file.tmp"
      fi

      log "  $action: $username${email:+ <$email>}$([ "$is_admin" = "true" ] && echo ' (admin)')"
      mirrored=$((mirrored + 1))
    done <<EOF
$(printf '%s' "$prod_users" | jq -c '.data[]')
EOF

    # Whatever is left is local-only -- a leftover from an earlier sync or a
    # hand-made account -- and production has no counterpart, so drop it.
    while IFS="$(printf '\t')" read -r stale_id stale_name; do
      [ -n "$stale_id" ] || continue
      case " $kept_ids " in *" $stale_id "*) continue ;; esac
      if pid_api "$LOCAL_POCKET_ID_URL" "$POCKET_ID_STATIC_API_KEY" \
           DELETE "/api/users/$stale_id" >/dev/null 2>&1; then
        log "  removed local-only account: $stale_name"
      else
        warn "could not remove local-only Pocket ID account '$stale_name'"
      fi
    done <<EOF
$(printf '%s' "$local_users" | jq -r '.data[] | [.id, .username] | @tsv')
EOF

    mv "$login_file.tmp" "$login_file"
    chmod 600 "$login_file"
    log "$mirrored Pocket ID account(s) mirrored${skipped:+, $skipped skipped}"

    # Point every Guardrail row at its local Pocket ID id. Imported `sub` values
    # usually already match, since ids are preserved above, but an account that
    # had to reuse a differently-id'd local one needs correcting.
    log "binding Guardrail users to their Pocket ID accounts..."
    refreshed="$(pid_api "$LOCAL_POCKET_ID_URL" "$POCKET_ID_STATIC_API_KEY" GET "$PID_LIST_PATH")"
    guardrail_users="$(surreal_sql "$LOCAL_DB_URL" "$LOCAL_DB_USER" "$SURREALDB_ROOT_PASSWORD" "$SURREAL_DB" \
      "SELECT meta::id(id) AS id, username, email FROM users
       WHERE username != 'anonymous' ORDER BY username;")"
    assert_ok "$guardrail_users" "reading imported users"

    unmatched=0
    while IFS="$(printf '\t')" read -r gid gusername gemail; do
      [ -n "$gid" ] || continue
      # Email first: Guardrail and Pocket ID routinely disagree on username.
      match="$(printf '%s' "$refreshed" | jq -r \
        --arg username "$gusername" --arg email "$gemail" '
          [ (.data[]? | select(($email != "") and .email == $email)),
            (.data[]? | select(.username == $username)) ]
          | first | .id // empty')"
      if [ -z "$match" ]; then
        warn "no Pocket ID account matches Guardrail user '$gusername' <$gemail> - it cannot log in"
        unmatched=$((unmatched + 1))
        continue
      fi
      case "$gid$match" in
        *[!A-Za-z0-9-]*)
          warn "unexpected characters in ids for '$gusername' - not binding sub"
          continue
          ;;
      esac
      bind="$(surreal_sql "$LOCAL_DB_URL" "$LOCAL_DB_USER" "$SURREALDB_ROOT_PASSWORD" "$SURREAL_DB" \
        "UPDATE type::record('users', '$gid') SET sub = '$match' RETURN NONE;")"
      assert_ok "$bind" "binding sub for $gusername"
    done <<EOF
$(printf '%s' "$guardrail_users" | jq -r '
  .[0].result[] | [.id, .username, (.email // "")] | @tsv')
EOF
    if [ "$unmatched" -gt 0 ]; then
      warn "$unmatched Guardrail user(s) have no Pocket ID account"
    fi

    log "one-time login links (valid $LOGIN_TTL) written to $login_file:"
    while IFS='=' read -r who link; do
      [ -n "$who" ] && log "  $who  ->  $link"
    done <"$login_file"
    log "Each link signs that user in once; Pocket ID then prompts them to add a"
    log "passkey, which is what they use from then on."
  fi
fi

# ---------------------------------------------------------------------------
# Garage / S3
# ---------------------------------------------------------------------------

if [ "$DO_STORAGE" = 1 ]; then
  if [ "$DRY_RUN" = 1 ]; then
    log "would mirror bucket '$BUCKET' from $PROD_GARAGE_SERVICE in namespace $PROD_GARAGE_NAMESPACE"
  else
    log "mirroring object storage…"
    s3_port="$(port_forward "$PROD_GARAGE_NAMESPACE" "$PROD_GARAGE_SERVICE" "$PROD_GARAGE_PORT")"

    prod_s3_key="$(secret "$PROD_GARAGE_NAMESPACE" "$PROD_GARAGE_SECRET" accessKey)"
    prod_s3_secret="$(secret "$PROD_GARAGE_NAMESPACE" "$PROD_GARAGE_SECRET" secretKey)"
    [ -n "$prod_s3_key" ] && [ -n "$prod_s3_secret" ] \
      || die "could not read $PROD_GARAGE_SECRET from namespace $PROD_GARAGE_NAMESPACE"
    export RCLONE_CONFIG_PROD_TYPE=s3 RCLONE_CONFIG_PROD_PROVIDER=Other \
      RCLONE_CONFIG_PROD_ENDPOINT="http://127.0.0.1:${s3_port}" \
      RCLONE_CONFIG_PROD_REGION=us-east-1 \
      RCLONE_CONFIG_PROD_FORCE_PATH_STYLE=true \
      RCLONE_CONFIG_PROD_ACCESS_KEY_ID="$prod_s3_key" \
      RCLONE_CONFIG_PROD_SECRET_ACCESS_KEY="$prod_s3_secret"
    export RCLONE_CONFIG_LOCAL_TYPE=s3 RCLONE_CONFIG_LOCAL_PROVIDER=Other \
      RCLONE_CONFIG_LOCAL_ENDPOINT="$LOCAL_S3_URL" \
      RCLONE_CONFIG_LOCAL_REGION=us-east-1 \
      RCLONE_CONFIG_LOCAL_FORCE_PATH_STYLE=true \
      RCLONE_CONFIG_LOCAL_ACCESS_KEY_ID="$GARAGE_ACCESS_KEY" \
      RCLONE_CONFIG_LOCAL_SECRET_ACCESS_KEY="$GARAGE_SECRET_KEY"

    size="$(rclone size "prod:$BUCKET" --json 2>/dev/null || true)"
    [ -n "$size" ] && log "production bucket: $(printf '%s' "$size" | jq -r '"\(.count) objects, \(.bytes) bytes"')"

    # sync (not copy): objects that exist only locally are removed, so the
    # bucket ends up an exact mirror of production.
    rclone sync "prod:$BUCKET" "local:$BUCKET" --progress --transfers 8 --checksum \
      || die "object storage sync failed"
    log "local bucket: $(rclone size "local:$BUCKET" --json | jq -r '"\(.count) objects, \(.bytes) bytes"')"
  fi
fi

# ---------------------------------------------------------------------------
# Restart the app so it picks up the imported state
# ---------------------------------------------------------------------------

if [ "$FLUSH_QUEUES" = 1 ] && [ "$DO_DB" = 1 ]; then
  # Production queue state refers to jobs this stack never ran, and stale apalis
  # worker registrations stop the processor from picking work up after a
  # restart. A clean valkey avoids both.
  # Not routed through run(): the command line carries the valkey password, and
  # run() would echo it verbatim under --dry-run.
  if [ "$DRY_RUN" = 1 ]; then
    log "would flush valkey"
  else
    log "flushing valkey…"
    docker exec valkey valkey-cli -a "${VALKEY_PASSWORD:-}" --no-auth-warning FLUSHALL >/dev/null
  fi
fi

# guardrail-web/api re-register the SurrealDB JWT access method with the LOCAL
# public key on startup (src/web/server/src/app.rs); the imported definition
# carries production's key, so restarting is what makes local logins work.
restartable=()
for c in guardrail-api guardrail-web guardrail-ingestion guardrail-curator guardrail-processor; do
  running "$c" && restartable+=("$c")
done
if [ ${#restartable[@]} -gt 0 ]; then
  step "restart ${restartable[*]}" "restarting ${restartable[*]}…"
  run compose restart "${restartable[@]}"
else
  warn "no guardrail-* containers running; start them so the local JWT access method is re-registered"
fi

if [ "$DRY_RUN" = 1 ]; then
  log "dry run only — nothing was changed"
else
  log "done — open https://guardrail.home.krandor.org:4433/"
fi
