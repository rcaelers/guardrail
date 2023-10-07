BASEDIR=$(dirname "$0")
JWTTOKEN=`$BASEDIR/token.sh 230306291224936471 $BASEDIR/230306311793803287-sa-guardrail.json`

curl -X POST http://localhost:8080/api/product -H 'Content-Type: application/json' -H "Authorization: Bearer $JWTTOKEN" -d '{ "name":"Workrave" }' 
curl -X POST http://localhost:8080/api/version -H 'Content-Type: application/json' -H "Authorization: Bearer $JWTTOKEN" -d '{ "name":"1.11", "hash":"1234567890", "tag": "v1.11", "product": "Workrave" }'
curl -vv -X POST "http://localhost:8080/api/symbols/upload?product=Workrave&version=1.11" -H "Authorization: Bearer $JWTTOKEN" -Fupload_file_symbols=@dev/crash.sym
curl -vv -X POST "http://localhost:8080/api/minidump/upload?product=Workrave&version=1.11" -H "Authorization: Bearer $JWTTOKEN" -Fupload_file_minidump=@dev/6fda4029-be94-43ea-90b6-32fe2a78074a.dmp -Fattach=@dev/init.sh
