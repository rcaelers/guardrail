BASEDIR=$(dirname "$0")
API_URI=http://guardrail.home.krandor.org:80/
DB_URI=http://guardrail.home.krandor.org:3000/

TOKEN="Amve0SLuRJOiEVdJIFwkYHHhLHLfS1teEmBdFZPdJC8GGUJ8BUwRD0R3-yQ0RmCyDzt0vNlVooUYi40jcT13bw=="

RESP=$(curl -s -X POST ${API_URI}api/auth/jwt --insecure -H 'Content-Type: text/plain' -H "Authorization: Bearer $TOKEN")

JWT=$(echo $RESP | jq -r '.token')
if [ -z "$JWT" ]; then
  echo "Failed to get JWT token"
  exit 1
fi
echo JWT=$JWT

hash_token() {
  SALT=$(openssl rand -hex 16)
  echo -n "$1" | argon2 "$SALT" -id -v 13 -t 3 -m 16 -p 4 -e
}

create_token() {
  local DESCRIPTION=$1
  local ENTITLEMENTS=$2
  local EXTRA=$3

  local RESP=$(curl -s -X POST ${API_URI}api/auth/token --insecure \
    -H 'Content-Type: application/json' \
    -H "Prefer: return=representation")

  GEN_TOKEN_ID=$(echo $RESP | jq -r '.token_id')
  GEN_TOKEN=$(echo $RESP | jq -r '.token')
  GEN_TOKEN_HASH=$(echo $RESP | jq -r '.token_hash')

  local RESP=$(curl -s -X POST ${DB_URI}api_tokens --insecure \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $JWT" \
    -H "Prefer: return=representation" \
    -d '{
      "description": "'"$DESCRIPTION"'",
      "token_id": "'$GEN_TOKEN_ID'",
      "token_hash": "'"$GEN_TOKEN_HASH"'",
      "entitlements": '"$ENTITLEMENTS"',
      "is_active": true,
      '$EXTRA'
    }')
  local TOKEN_ID=$(echo $RESP | jq -r ".[0].id")
  echo "$GEN_TOKEN"
}

RESP=$(curl -s -X POST ${DB_URI}users --insecure \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $JWT" \
  -H "Prefer: return=representation" \
  -d '{ "username":"rob", "is_admin":"true" }')
USER_ID=$(echo $RESP | jq -r ".[0].id")

RESP=$(curl -s -X POST ${DB_URI}products --insecure \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $JWT" \
  -H "Prefer: return=representation" \
  -d '{ "name":"Workrave", "description": "Workrave Application" }')
echo $RESP
WORKRAVE_ID=$(echo $RESP | jq -r ".[0].id")

MINIDUMP_TOKEN=$(create_token "Minidump upload" '["minidump-upload"]' '"product_id":"'$WORKRAVE_ID'"')
SYMBOLS_TOKEN=$(create_token "Symbols upload" '["symbol-upload"]' '"product_id":"'$WORKRAVE_ID'"')

echo "Minidump token: $MINIDUMP_TOKEN"
echo "Symbols token: $SYMBOLS_TOKEN"

for i in {1..20}; do
  RESP=$(curl -s -X POST ${DB_URI}products --insecure \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $JWT" \
    -H "Prefer: return=representation" \
    -d '{ "name":"App'$i'", "description": "Test Application" }')
  echo $RESP
  APP_ID=$(echo $RESP | jq -r ".[0].id")

  for v in {1..20}; do
    RESP=$(curl -s -X POST ${DB_URI}versions --insecure \
      -H 'Content-Type: application/json' \
      -H "Authorization: Bearer $JWT" \
      -H "Prefer: return=representation" \
      -d '{ "name":"1.'$v'", "hash":"1234567890", "tag": "v1.'$v'", "product_id":"'$APP_ID'" }')
  done

  RESP=$(curl -s -X POST ${DB_URI}user_access --insecure \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $JWT" \
    -H "Prefer: return=representation" \
    -d '{ "user_id":"'$USER_ID'", "product_id":"'$APP_ID'", role:"admin" }')

done

RESP=$(curl -s -X POST ${DB_URI}users --insecure \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $JWT" \
  -H "Prefer: return=representation" \
  -d '{ "username":"rob", "is_admin":"true" }')

curl -X GET ${DB_URI}products --insecure \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $JWT"

# for i in {1..20}; do
#   for v in {1..20}; do
#     for r in {1..5}; do
#       curl -vv -X POST "${URI}api/symbols/upload?product=GuardrailTest${i}&version=1.${v}" --insecure -H "Authorization: Bearer $TOKEN" -Fsymbols_file=@dev/crash.sym
#       curl -vv -X POST "${URI}api/minidump/upload?product=GuardrailTest${i}&version=1.${v}" --insecure -H "Authorization: Bearer $TOKEN" -Fminidump_file=@dev/6fda4029-be94-43ea-90b6-32fe2a78074a.dmp -Fattach=@dev/init.sh
#     done
#   done
# done

curl -vv -X POST "${API_URI}api/symbols/upload?product=App1&version=1.1" --insecure -H "Authorization: Bearer $SYMBOLS_TOKEN" -Fsymbols_file=@dev/crash.sym
curl -vv -X POST "${API_URI}api/minidump/upload" --insecure -H "Authorization: Bearer $MINIDUMP_TOKEN" -Fminidump_file=@dev/6fda4029-be94-43ea-90b6-32fe2a78074a.dmp -Fattach=@dev/init.sh

curl -vv -X POST "{API_URI}api/minidump/upload?api_key=$MINIDUMP_TOKEN" --insecure \
  -F"upload_file_minidump=@dev/6fda4029-be94-43ea-90b6-32fe2a78074a.dmp;type=application/octet-stream" \
  -F"product=Workrave;type=text/plain" \
  -F"version=1.11;type=text/plain" \
  -F"channel=rc;type=text/plain" \
  -F"commit=x;type=text/plain" \
  -F"buildid=x;type=text/plain"
