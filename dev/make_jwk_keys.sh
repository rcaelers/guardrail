#!/bin/bash

BASEDIR=$(dirname "$0")
KEYNAME="${1:-ed25519}"

openssl genpkey -algorithm ed25519 -out "${BASEDIR}/${KEYNAME}-private.pem"
openssl pkey -in "${BASEDIR}/${KEYNAME}-private.pem" -pubout -out "${BASEDIR}/${KEYNAME}-public.pem"
step crypto jwk create "${BASEDIR}/${KEYNAME}-jwk.json" "${BASEDIR}/${KEYNAME}-jwk-private.json" --from-pem "${BASEDIR}/${KEYNAME}-private.pem"
