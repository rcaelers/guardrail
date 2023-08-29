curl -X POST http://localhost:8080/api/product  -H 'Content-Type: application/json' -d '{ "name":"Workrave" }' 
curl -X POST http://localhost:8080/api/version  -H 'Content-Type: application/json' -d '{ "name":"1.11", "hash":"1234567890", "tag": "v1.11", "product": "Workrave" }'
curl -X POST "http://localhost:8080/api/symbols/upload?product=Workrave&version=1.11" -Fupload_file_symbols=@dev/workrave.sym
# curl -vv -X POST "http://localhost:8080/api/minidump/upload?product=Workrave&version=1.11" -Fupload_file_minidump=@dev/40e1f375-f6c9-4b18-bab1-5063550bb59b.dmp -Fattach=@dev/init.sh