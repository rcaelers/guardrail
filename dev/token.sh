#!/bin/bash

# Check if the required arguments are provided
if [ "$#" -lt 2 ]; then
    echo "Usage: $0 <CLIENT_ID> <PATH_TO_KEY_JSON> [TOKEN_ENDPOINT]"
    exit 1
fi

CLIENT_ID="$1"
KEY_JSON="$2"
TOKEN_ENDPOINT="${3:-https://idp.krandor.org/oauth/v2/token}"  # Default endpoint if not provided
BASE_URL=$(echo "$TOKEN_ENDPOINT" | awk -F/ '{print $1 "//" $3}')

# Extract necessary values from the JSON file
PRIVATE_KEY=$(jq -r .key "$KEY_JSON")
KEY_ID=$(jq -r .keyId "$KEY_JSON")

# JWT Header for RS256
HEADER="{
  \"alg\": \"RS256\",
  \"typ\": \"JWT\",
  \"kid\": \"$KEY_ID\"
}"

# JWT Payload with the necessary claims. Adjust as per Zitadel requirements
PAYLOAD="{
  \"iss\": \"$CLIENT_ID\",
  \"sub\": \"$CLIENT_ID\",
  \"aud\": \"$BASE_URL\",
  \"exp\": $(($(date +%s) + 3600)),
  \"iat\": $(date +%s),
  \"jti\": \"$(uuidgen)\"
}"

BASE64URL_HEADER=$(echo -n "$HEADER" | jq -c -r . | openssl base64 -A -a -e | tr '+/' '-_' | tr -d '=')
BASE64URL_PAYLOAD=$(echo -n "$PAYLOAD" | jq -c -r . | openssl base64 -A -a -e | tr '+/' '-_' | tr -d '=')

UNSIGNED_JWT="$BASE64URL_HEADER.$BASE64URL_PAYLOAD"

# Use a file descriptor to provide the key to openssl
exec 3<<<"$PRIVATE_KEY"
SIGNATURE=$(echo -n "$UNSIGNED_JWT" | openssl dgst -binary -sha256 -sign /dev/fd/3 | openssl base64 -A -a -e | tr '+/' '-_' | tr -d '=')
exec 3<&-

SIGNED_JWT="$UNSIGNED_JWT.$SIGNATURE"

# Request token using the JWT
TOKEN_RESPONSE=$(curl -s -X POST \
  "$TOKEN_ENDPOINT" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  --data-urlencode "client_assertion_type=urn:ietf:params:oauth:client-assertion-type:jwt-bearer" \
  --data-urlencode "client_assertion=$SIGNED_JWT" \
  --data-urlencode "assertion=$SIGNED_JWT" \
  --data-urlencode "grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer" \
  --data-urlencode "scope=openid profile email urn:zitadel:iam:org:project:roles urn:zitadel:iam:user:metadata urn:zitadel:iam:org:project:id:zitadel:aud")

# Extract and print the access token
ACCESS_TOKEN=$(echo "$TOKEN_RESPONSE" | jq -r .access_token)

if [ "$ACCESS_TOKEN" != "null" ]; then
    echo "$ACCESS_TOKEN"
else
    exit 1
fi
