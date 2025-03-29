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

# curl -X POST ${DB_URI}products --insecure \
#   -H 'Content-Type: application/json' \
#   -H "Authorization: Bearer $JWT" \
#   -H "Prefer: return=representation" \
#   -d '{ "name":"Workrave", "description": "Workrave" }'

RESP=$(curl -s -X POST ${DB_URI}users --insecure \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $JWT" \
  -H "Prefer: return=representation" \
  -d '{ "username":"rob", "is_admin":"true" }')
USER_ID=$(echo $RESP | jq -r ".[0].id")

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

#curl -vv -X POST "${URI}api/symbols/upload?product=Workrave&version=1.11" --insecure -H "Authorization: Bearer $TOKEN" -Fupload_file_symbols=@dev/crash.sym
#curl -vv -X POST "${URI}api/minidump/upload?product=Workrave&version=1.11" --insecure -H "Authorization: Bearer $TOKEN" -Fupload_file_minidump=@dev/6fda4029-be94-43ea-90b6-32fe2a78074a.dmp -Fattach=@dev/init.sh
