BASEDIR=$(dirname "$0")
API_URI=https://guardrail.home.krandor.org:4433/
DB_URI=http://guardrail.home.krandor.org:3000/

TOKEN="eLRwoDQGlZNSVWQOOyrTV7f8i9C78iGjv9YB"

RESP=$(curl -s -X POST ${API_URI}api/auth/token --insecure -H 'Content-Type: text/plain' -H "Authorization: Bearer $TOKEN")

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

RESP=$(curl -s -X POST ${DB_URI}users --insecure \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $JWT" \
  -H "Prefer: return=representation" \
  -d '{ "username":"rob", "is_admin":"true" }')
USER_ID=$(echo $RESP | jq -r ".[0].id")

SYMBOL_TOKEN="symbol-upload-token"
SYMBOL_TOKEN_HASH=$(hash_token "$SYMBOL_TOKEN")
RESP=$(curl -s -X POST ${DB_URI}api_tokens --insecure \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $JWT" \
  -H "Prefer: return=representation" \
  -d '{
    "description": "Symbols upload token",
    "token_hash": "'$SYMBOL_TOKEN_HASH'",
    "entitlements": ["symbol-upload"],
    "is_active": true
  }')
TOKEN_ID=$(echo $RESP | jq -r ".[0].id")
echo "Created symbol upload token with ID: $TOKEN_ID"

MINIDUMP_TOKEN="minidump-upload-token"
MINIDUMP_TOKEN_HASH=$(hash_token "$MINIDUMP_TOKEN")
RESP=$(curl -s -X POST ${DB_URI}api_tokens --insecure \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $JWT" \
  -H "Prefer: return=representation" \
  -d '{
    "description": "Minidump upload token",
    "token_hash": "'$MINIDUMP_TOKEN_HASH'",
    "entitlements": ["minidump-upload"],
    "is_active": true
  }')
TOKEN_ID=$(echo $RESP | jq -r ".[0].id")
echo "Created minidump upload token with ID: $TOKEN_ID"

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
#       curl -vv -X POST "${URI}api/symbols/upload?product=GuardrailTest${i}&version=1.${v}" --insecure -H "Authorization: Bearer $TOKEN" -Fupload_file_symbols=@dev/crash.sym
#       curl -vv -X POST "${URI}api/minidump/upload?product=GuardrailTest${i}&version=1.${v}" --insecure -H "Authorization: Bearer $TOKEN" -Fupload_file_minidump=@dev/6fda4029-be94-43ea-90b6-32fe2a78074a.dmp -Fattach=@dev/init.sh
#     done
#   done
# done

curl -vv -X POST "${API_URI}api/symbols/upload?product=Workrave&version=1.11" --insecure -H "Authorization: Bearer symbols-upload-token" -Fupload_file_symbols=@dev/crash.sym
#curl -vv -X POST "${URI}api/minidump/upload?product=Workrave&version=1.11" --insecure -H "Authorization: Bearer $TOKEN" -Fupload_file_minidump=@dev/6fda4029-be94-43ea-90b6-32fe2a78074a.dmp -Fattach=@dev/init.sh
