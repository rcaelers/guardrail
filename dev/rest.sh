#!/bin/bash
BASEDIR=$(dirname "$0")
URI=http://localhost:3000

PAYLOAD="{
  \"sub\": \"Guardrail\",
  \"aud\": \"Guardrail\",
  \"name\": \"Guardrail\",
  \"exp\": $(($(date +%s) + 3600)),
  \"iat\": $(date +%s),
  \"jti\": \"$(uuidgen)\",
  \"role\": \"guardrail_webuser\",
  \"user_id\": \"Guardrail\"
}"

TOKEN=$(jwt encode --secret @${BASEDIR}/ed25519-private.pem -A EDDSA "$PAYLOAD")

echo -n "JWT:"
echo "$TOKEN"

curl ${URI}/users --insecure -H 'Content-Type: application/json' -H "Authorization: Bearer $TOKEN"
curl ${URI}/product --insecure -H 'Content-Type: application/json' -H "Authorization: Bearer $TOKEN"
curl -X POST ${URI}/product --insecure -H 'Content-Type: application/json' -H "Authorization: Bearer $TOKEN" -d '{ "name":"Workrave" }'
#curl -X POST ${URI}/product --insecure -H 'Content-Type: application/json' -H "Authorization: Bearer $TOKEN" -d '{ "name":"Guardrail" }'
