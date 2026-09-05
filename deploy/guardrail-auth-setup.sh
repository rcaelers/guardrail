#!/bin/sh
set -eu

log() {
  printf '%s\n' "$*"
}

request_json() {
  method=$1
  url=$2
  body=${3-}
  response_file=$(mktemp)

  if [ -n "$body" ]; then
    status=$(curl -sS \
      -o "$response_file" \
      -w '%{http_code}' \
      -X "$method" \
      -H 'Content-Type: application/json' \
      -H "X-API-KEY: $POCKET_ID_API_KEY" \
      "$url" \
      -d "$body")
  else
    status=$(curl -sS \
      -o "$response_file" \
      -w '%{http_code}' \
      -X "$method" \
      -H "X-API-KEY: $POCKET_ID_API_KEY" \
      "$url")
  fi

  case "$status" in
    2*)
      cat "$response_file"
      rm -f "$response_file"
      ;;
    *)
      log "Pocket ID request failed: $method $url -> HTTP $status"
      cat "$response_file" >&2
      rm -f "$response_file"
      return 1
      ;;
  esac
}

request_status() {
  method=$1
  url=$2
  output_file=$3
  body=${4-}

  if [ -n "$body" ]; then
    curl -sS \
      -o "$output_file" \
      -w '%{http_code}' \
      -X "$method" \
      -H 'Content-Type: application/json' \
      -H "X-API-KEY: $POCKET_ID_API_KEY" \
      "$url" \
      -d "$body"
    return
  fi

  curl -sS \
    -o "$output_file" \
    -w '%{http_code}' \
    -X "$method" \
    -H "X-API-KEY: $POCKET_ID_API_KEY" \
    "$url"
}

request_noauth_json() {
  method=$1
  url=$2
  body=${3-}
  response_file=$(mktemp)

  if [ -n "$body" ]; then
    status=$(curl -sS \
      -o "$response_file" \
      -w '%{http_code}' \
      -X "$method" \
      -H 'Content-Type: application/json' \
      "$url" \
      -d "$body")
  else
    status=$(curl -sS \
      -o "$response_file" \
      -w '%{http_code}' \
      -X "$method" \
      "$url")
  fi

  case "$status" in
    2*)
      cat "$response_file"
      rm -f "$response_file"
      ;;
    *)
      log "Pocket ID request failed: $method $url -> HTTP $status"
      cat "$response_file" >&2
      rm -f "$response_file"
      return 1
      ;;
  esac
}

: "${POCKET_ID_URL:?POCKET_ID_URL is required}"
: "${POCKET_ID_PUBLIC_URL:?POCKET_ID_PUBLIC_URL is required}"
: "${POCKET_ID_API_KEY:?POCKET_ID_API_KEY is required}"
: "${POCKET_ID_ADMIN_USERNAME:?POCKET_ID_ADMIN_USERNAME is required}"
: "${POCKET_ID_ADMIN_EMAIL:?POCKET_ID_ADMIN_EMAIL is required}"
: "${POCKET_ID_ADMIN_FIRST_NAME:?POCKET_ID_ADMIN_FIRST_NAME is required}"
: "${POCKET_ID_ADMIN_LAST_NAME:?POCKET_ID_ADMIN_LAST_NAME is required}"
: "${POCKET_ID_OUTPUT_DIR:?POCKET_ID_OUTPUT_DIR is required}"
: "${GUARDRAIL_AUTH_OIDC_CLIENT_ID:?GUARDRAIL_AUTH_OIDC_CLIENT_ID is required}"
: "${GUARDRAIL_AUTH_OIDC_CLIENT_NAME:?GUARDRAIL_AUTH_OIDC_CLIENT_NAME is required}"
: "${GUARDRAIL_AUTH_OIDC_CALLBACK_URL:?GUARDRAIL_AUTH_OIDC_CALLBACK_URL is required}"
: "${GUARDRAIL_AUTH_OIDC_LOGOUT_CALLBACK_URL:?GUARDRAIL_AUTH_OIDC_LOGOUT_CALLBACK_URL is required}"

POCKET_ID_URL=${POCKET_ID_URL%/}
POCKET_ID_PUBLIC_URL=${POCKET_ID_PUBLIC_URL%/}
GUARDRAIL_AUTH_OIDC_CALLBACK_URL=${GUARDRAIL_AUTH_OIDC_CALLBACK_URL%/}
GUARDRAIL_AUTH_OIDC_LOGOUT_CALLBACK_URL=${GUARDRAIL_AUTH_OIDC_LOGOUT_CALLBACK_URL%/}
GUARDRAIL_AUTH_OIDC_LAUNCH_URL=${GUARDRAIL_AUTH_OIDC_LAUNCH_URL:-}
GUARDRAIL_AUTH_OIDC_LAUNCH_URL=${GUARDRAIL_AUTH_OIDC_LAUNCH_URL%/}

mkdir -p "$POCKET_ID_OUTPUT_DIR"
umask 077

OIDC_ENV_FILE="$POCKET_ID_OUTPUT_DIR/guardrail-oidc.env"
ADMIN_ENV_FILE="$POCKET_ID_OUTPUT_DIR/admin-login.env"

log "Waiting for Pocket ID API..."
attempt=0
until request_json GET "$POCKET_ID_URL/api/users" >/dev/null 2>&1; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 90 ]; then
    log "Pocket ID API did not become ready in time"
    exit 1
  fi
  sleep 2
done

# The user backing STATIC_API_KEY is an admin as well, and Pocket ID v2 names it
# "static-api-user-<random>" rather than the "Static API User" this script used to
# exclude. Matching it makes the bootstrap believe the admin already exists: no
# real admin is created, and the one-time login code is minted for the API user,
# whose OIDC `sub` maps to no Guardrail account ("Your account has not been
# granted access"). It carries a sentinel all-zero id, which is the sturdiest
# way to tell it apart; the name checks are a backstop.
STATIC_API_USER_ID="00000000-0000-0000-0000-000000000000"

# Sets admin_id/admin_username from $users_json, preferring the configured admin
# username and otherwise falling back to any other admin, so an existing Pocket
# ID with a differently named admin still bootstraps.
find_admin() {
  admin_row=$(printf '%s' "$users_json" | jq -r \
    --arg username "$POCKET_ID_ADMIN_USERNAME" \
    --arg static_id "$STATIC_API_USER_ID" '
      [ .data[]
        | select(.id != $static_id)
        | select((.username | startswith("static-api-user-")) | not)
        | select(.username != "Static API User")
      ]
      | ( map(select(.username == $username)) + map(select(.isAdmin == true)) )
      | first
      | if . then [.id, .username] | @tsv else empty end
    ')
  admin_id=$(printf '%s' "$admin_row" | cut -f1)
  admin_username=$(printf '%s' "$admin_row" | cut -f2)
}

log "Ensuring initial admin user exists..."
users_json=$(request_json GET "$POCKET_ID_URL/api/users")
find_admin

if [ -z "$admin_id" ]; then
  setup_payload=$(jq -nc \
    --arg username "$POCKET_ID_ADMIN_USERNAME" \
    --arg email "$POCKET_ID_ADMIN_EMAIL" \
    --arg first_name "$POCKET_ID_ADMIN_FIRST_NAME" \
    --arg last_name "$POCKET_ID_ADMIN_LAST_NAME" \
    '{
      username: $username,
      email: $email,
      firstName: $first_name,
      lastName: $last_name
    }')
  request_noauth_json POST "$POCKET_ID_URL/api/signup/setup" "$setup_payload" >/dev/null

  users_json=$(request_json GET "$POCKET_ID_URL/api/users")
  find_admin
fi

if [ -z "$admin_id" ]; then
  log "Pocket ID admin user was not found after setup"
  exit 1
fi

if [ -z "$admin_username" ]; then
  admin_username=$POCKET_ID_ADMIN_USERNAME
fi

log "Generating Pocket ID admin login code..."
login_token_json=$(request_json POST "$POCKET_ID_URL/api/users/$admin_id/one-time-access-token" '{"ttl":"168h"}')
admin_login_token=$(printf '%s' "$login_token_json" | jq -r '.token // empty')

if [ -z "$admin_login_token" ]; then
  log "Pocket ID did not return an admin login token"
  exit 1
fi

log "Ensuring Guardrail OIDC client exists..."
client_tmp=$(mktemp)
client_status=$(request_status GET "$POCKET_ID_URL/api/oidc/clients/$GUARDRAIL_AUTH_OIDC_CLIENT_ID" "$client_tmp")

client_payload=$(jq -nc \
  --arg id "$GUARDRAIL_AUTH_OIDC_CLIENT_ID" \
  --arg name "$GUARDRAIL_AUTH_OIDC_CLIENT_NAME" \
  --arg callback "$GUARDRAIL_AUTH_OIDC_CALLBACK_URL" \
  --arg logout "$GUARDRAIL_AUTH_OIDC_LOGOUT_CALLBACK_URL" \
  --arg launch "$GUARDRAIL_AUTH_OIDC_LAUNCH_URL" \
  '{
    id: $id,
    name: $name,
    callbackURLs: [$callback],
    logoutCallbackURLs: [$logout],
    isPublic: false,
    pkceEnabled: true,
    requiresReauthentication: false,
    credentials: {
      federatedIdentities: []
    },
    launchURL: (if $launch != "" then $launch else null end),
    isGroupRestricted: false
  }')

case "$client_status" in
  200)
    request_json PUT "$POCKET_ID_URL/api/oidc/clients/$GUARDRAIL_AUTH_OIDC_CLIENT_ID" "$client_payload" >/dev/null
    log "Updated Guardrail OIDC client: $GUARDRAIL_AUTH_OIDC_CLIENT_ID"
    ;;
  404)
    request_json POST "$POCKET_ID_URL/api/oidc/clients" "$client_payload" >/dev/null
    log "Created Guardrail OIDC client: $GUARDRAIL_AUTH_OIDC_CLIENT_ID"
    ;;
  *)
    log "Unexpected response while checking OIDC client: HTTP $client_status"
    cat "$client_tmp"
    rm -f "$client_tmp"
    exit 1
    ;;
esac
rm -f "$client_tmp"

# Pocket ID v2 keeps a *list* of client secrets under /secrets (v1 had a single
# rotating one under /secret): POST adds a secret and is the only response that
# ever discloses its value, GET lists the existing ones without values.
#
# The pocket-id volume and this bind mount have independent lifetimes, so either
# side can come back empty. Rather than minting a new secret whenever they
# disagree — which silently invalidates the client_secret guardrail-web already
# has — converge on the one we hold on file, importing it into Pocket ID when it
# is no longer registered there.
secrets_url="$POCKET_ID_URL/api/oidc/clients/$GUARDRAIL_AUTH_OIDC_CLIENT_ID/secrets"

existing_secret=""
if [ -f "$OIDC_ENV_FILE" ]; then
  existing_secret=$(grep '^GUARDRAIL_AUTH_OIDC_CLIENT_SECRET=' "$OIDC_ENV_FILE" | cut -d= -f2- || true)
fi

client_secret=""
if [ -n "$existing_secret" ]; then
  # `prefix` holds each stored secret's leading characters in clear text, which
  # is enough to tell whether the one we hold is still registered. It is empty
  # for secrets migrated from v1's single-secret column, so skip those instead
  # of treating an empty prefix as a match for everything.
  secrets_json=$(request_json GET "$secrets_url" 2>/dev/null || true)
  known_prefixes=$(printf '%s' "$secrets_json" \
    | jq -r '.[]? | select(.isActive == true) | select(.prefix != null and .prefix != "") | .prefix' \
      2>/dev/null || true)
  for prefix in $known_prefixes; do
    case "$existing_secret" in
      "$prefix"*)
        client_secret="$existing_secret"
        break
        ;;
    esac
  done

  if [ -n "$client_secret" ]; then
    log "Reusing the OIDC client secret already registered with Pocket ID"
  else
    log "Re-importing the OIDC client secret from $OIDC_ENV_FILE..."
    import_payload=$(jq -nc --arg secret "$existing_secret" '{secret: $secret}')
    if secret_json=$(request_json POST "$secrets_url" "$import_payload"); then
      client_secret=$(printf '%s' "$secret_json" | jq -r '.secret // empty')
    else
      # Pocket ID enforces min=16 printable-ASCII on caller-supplied secrets.
      log "Pocket ID rejected the stored secret; generating a fresh one instead"
    fi
  fi
fi

if [ -z "$client_secret" ]; then
  log "Generating Guardrail OIDC client secret..."
  secret_json=$(request_json POST "$secrets_url")
  client_secret=$(printf '%s' "$secret_json" | jq -r '.secret // empty')
fi

if [ -z "$client_secret" ]; then
  log "Pocket ID did not return a client secret for $GUARDRAIL_AUTH_OIDC_CLIENT_ID"
  exit 1
fi

oidc_env_tmp=$(mktemp)
{
  printf '# Generated by dev/guardrail-auth-setup.sh\n'
  printf 'GUARDRAIL_AUTH_OIDC_ISSUER_URL=%s\n' "$POCKET_ID_PUBLIC_URL"
  printf 'GUARDRAIL_AUTH_OIDC_CLIENT_ID=%s\n' "$GUARDRAIL_AUTH_OIDC_CLIENT_ID"
  printf 'GUARDRAIL_AUTH_OIDC_CLIENT_SECRET=%s\n' "$client_secret"
  printf 'GUARDRAIL_AUTH_OIDC_CALLBACK_URL=%s\n' "$GUARDRAIL_AUTH_OIDC_CALLBACK_URL"
  printf 'GUARDRAIL_AUTH_OIDC_LOGOUT_CALLBACK_URL=%s\n' "$GUARDRAIL_AUTH_OIDC_LOGOUT_CALLBACK_URL"
  if [ -n "$GUARDRAIL_AUTH_OIDC_LAUNCH_URL" ]; then
    printf 'GUARDRAIL_AUTH_OIDC_LAUNCH_URL=%s\n' "$GUARDRAIL_AUTH_OIDC_LAUNCH_URL"
  fi
} > "$oidc_env_tmp"
mv "$oidc_env_tmp" "$OIDC_ENV_FILE"

if [ -n "${GUARDRAIL_OIDC_SECRET_FILE:-}" ]; then
  secret_yaml_tmp=$(mktemp)
  {
    printf 'oidc:\n'
    printf '  client_secret: "%s"\n' "$client_secret"
  } > "$secret_yaml_tmp"
  mv "$secret_yaml_tmp" "$GUARDRAIL_OIDC_SECRET_FILE"
  log "Guardrail oidc.client_secret written to $GUARDRAIL_OIDC_SECRET_FILE"
fi

admin_env_tmp=$(mktemp)
{
  printf '# Generated by dev/guardrail-auth-setup.sh\n'
  printf 'POCKET_ID_ADMIN_ID=%s\n' "$admin_id"
  printf 'POCKET_ID_ADMIN_USERNAME=%s\n' "$admin_username"
  printf 'POCKET_ID_ADMIN_LOGIN_CODE=%s\n' "$admin_login_token"
  printf 'POCKET_ID_ADMIN_LOGIN_URL=%s/lc/%s\n' "$POCKET_ID_PUBLIC_URL" "$admin_login_token"
} > "$admin_env_tmp"
mv "$admin_env_tmp" "$ADMIN_ENV_FILE"

log "Guardrail auth bootstrap completed"
log "OIDC settings written to $OIDC_ENV_FILE"
log "Admin login settings written to $ADMIN_ENV_FILE"
