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

log "Ensuring initial admin user exists..."
users_json=$(request_json GET "$POCKET_ID_URL/api/users")
admin_id=$(printf '%s' "$users_json" | jq -r --arg username "$POCKET_ID_ADMIN_USERNAME" '
  .data[]
  | select(.username == $username or (.isAdmin == true and .username != "Static API User"))
  | .id
' | head -n 1)
admin_username=$(printf '%s' "$users_json" | jq -r --arg username "$POCKET_ID_ADMIN_USERNAME" '
  .data[]
  | select(.username == $username or (.isAdmin == true and .username != "Static API User"))
  | .username
' | head -n 1)

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
  admin_id=$(printf '%s' "$users_json" | jq -r --arg username "$POCKET_ID_ADMIN_USERNAME" '
    .data[]
    | select(.username == $username or (.isAdmin == true and .username != "Static API User"))
    | .id
  ' | head -n 1)
  admin_username=$(printf '%s' "$users_json" | jq -r --arg username "$POCKET_ID_ADMIN_USERNAME" '
    .data[]
    | select(.username == $username or (.isAdmin == true and .username != "Static API User"))
    | .username
  ' | head -n 1)
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
    pkceEnabled: false,
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

log "Generating Guardrail OIDC client secret..."
secret_json=$(request_json POST "$POCKET_ID_URL/api/oidc/clients/$GUARDRAIL_AUTH_OIDC_CLIENT_ID/secret")
client_secret=$(printf '%s' "$secret_json" | jq -r '.secret // empty')

if [ -z "$client_secret" ]; then
  log "Pocket ID did not return a client secret for $GUARDRAIL_AUTH_OIDC_CLIENT_ID"
  exit 1
fi

oidc_env_tmp=$(mktemp)
{
  printf '# Generated by dev/pocket-id-init.sh\n'
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

admin_env_tmp=$(mktemp)
{
  printf '# Generated by dev/pocket-id-init.sh\n'
  printf 'POCKET_ID_ADMIN_ID=%s\n' "$admin_id"
  printf 'POCKET_ID_ADMIN_USERNAME=%s\n' "$admin_username"
  printf 'POCKET_ID_ADMIN_LOGIN_CODE=%s\n' "$admin_login_token"
  printf 'POCKET_ID_ADMIN_LOGIN_URL=%s/lc/%s\n' "$POCKET_ID_PUBLIC_URL" "$admin_login_token"
} > "$admin_env_tmp"
mv "$admin_env_tmp" "$ADMIN_ENV_FILE"

log "Pocket ID bootstrap completed"
log "OIDC settings written to $OIDC_ENV_FILE"
log "Admin login settings written to $ADMIN_ENV_FILE"
