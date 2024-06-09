BASEDIR=$(dirname "$0")
URI=https://guardrail.home.krandor.org:4433/

PAYLOAD="{
  \"sub\": \"Guardrail\",
  \"aud\": \"Guardrail\",
  \"name\": \"Guardrail\",
  \"exp\": $(($(date +%s) + 3600)),
  \"iat\": $(date +%s),
  \"jti\": \"$(uuidgen)\"
}"

TOKEN=$(jwt encode --secret @${BASEDIR}/ed25519-private.pem -A EDDSA "$PAYLOAD")

echo -n "JWT:"
echo "$TOKEN"

curl -X POST ${URI}api/product --insecure -H 'Content-Type: application/json' -H "Authorization: Bearer $TOKEN" -d '{ "name":"Workrave" }'
curl -X POST ${URI}api/product --insecure -H 'Content-Type: application/json' -H "Authorization: Bearer $TOKEN" -d '{ "name":"Guardrail" }'

curl -X POST ${URI}api/version --insecure -H 'Content-Type: application/json' -H "Authorization: Bearer $TOKEN" -d '{ "name":"1.11", "hash":"1234567890", "tag": "v1.11", "product":"Workrave" }'
curl -X POST ${URI}api/version --insecure -H 'Content-Type: application/json' -H "Authorization: Bearer $TOKEN" -d '{ "name":"1.11", "hash":"1234567890", "tag": "v1.11", "product":"Guardrail" }'

for i in {1..20}; do
  curl -X POST ${URI}api/product --insecure -H 'Content-Type: application/json' -H "Authorization: Bearer $TOKEN" -d '{ "name":"GuardrailTest'$i'" }'
done

for i in {1..20}; do
  for v in {1..20}; do
    curl -X POST ${URI}api/version --insecure -H 'Content-Type: application/json' -H "Authorization: Bearer $TOKEN" -d '{ "name":"1.'$v'", "hash":"1234567890", "tag": "v1.11", "product":"GuardrailTest'$i'" }'
  done
done

curl -vv -X POST "${URI}api/symbols/upload?product=Workrave&version=1.11" --insecure -H "Authorization: Bearer $TOKEN" -Fupload_file_symbols=@dev/crash.sym
curl -vv -X POST "${URI}api/minidump/upload?product=Workrave&version=1.11" --insecure -H "Authorization: Bearer $TOKEN" -Fupload_file_minidump=@dev/6fda4029-be94-43ea-90b6-32fe2a78074a.dmp -Fattach=@dev/init.sh
