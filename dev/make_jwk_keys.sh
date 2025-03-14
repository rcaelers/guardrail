#!/bin/bash

BASEDIR=$(dirname "$0")

openssl genpkey -algorithm ed25519 -out ${BASEDIR}/ed25519-private.pem
openssl pkey -in ${BASEDIR}/ed25519-private.pem -pubout -out ${BASEDIR}/ed25519-public.pem
step crypto jwk create ${BASEDIR}/ed25519-jwk.json ${BASEDIR}/ed25519-jwk-private.json --from-pem ${BASEDIR}/ed25519-public.pem
