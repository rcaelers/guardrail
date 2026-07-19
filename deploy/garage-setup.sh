#!/bin/sh
# Bootstraps a fresh Garage cluster: assigns a single-node layout, creates the
# guardrail bucket, and grants the guardrail access key read/write on it.
# Ported from workrave-infra's apps/guardrail-setup-garage job, adapted to
# talk to the garage service on the compose network instead of garage.garage.
set -eu

: "${GARAGE_ADMIN_TOKEN:?GARAGE_ADMIN_TOKEN is required}"
: "${GARAGE_ACCESS_KEY:?GARAGE_ACCESS_KEY is required}"
: "${GARAGE_SECRET_KEY:?GARAGE_SECRET_KEY is required}"

base="http://garage:3903"

get() {
  curl -fsS -H "Authorization: Bearer ${GARAGE_ADMIN_TOKEN}" "$base/v2/$1"
}

post() {
  curl -fsS -X POST \
    -H "Authorization: Bearer ${GARAGE_ADMIN_TOKEN}" \
    -H "Content-Type: application/json" \
    -d "$2" \
    "$base/v2/$1"
}

echo "Waiting for Garage admin API..."
attempt=0
until status="$(get GetClusterStatus)"; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 60 ]; then
    echo "Garage admin API did not become ready in time" >&2
    exit 1
  fi
  sleep 2
done

node_id="$(printf '%s\n' "$status" | sed -n 's/.*"id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
layout_version="$(printf '%s\n' "$status" | sed -n 's/.*"layoutVersion"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' | head -n 1)"
if [ -z "$node_id" ] || [ -z "$layout_version" ]; then
  echo "Unable to determine Garage node id or layout version" >&2
  exit 1
fi

if ! printf '%s\n' "$status" | grep -q '"role"[[:space:]]*:[[:space:]]*{'; then
  echo "Assigning single-node cluster layout..."
  next_version="$((layout_version + 1))"
  post UpdateClusterLayout "{\"roles\":[{\"id\":\"$node_id\",\"zone\":\"dev\",\"capacity\":5368709120,\"tags\":[\"guardrail\"]}],\"parameters\":{\"zoneRedundancy\":{\"atLeast\":1}}}" >/dev/null
  post ApplyClusterLayout "{\"version\":$next_version}" >/dev/null
else
  echo "Cluster layout already assigned."
fi

echo "Ensuring guardrail bucket exists..."
bucket_json="$(get 'GetBucketInfo?globalAlias=guardrail' || true)"
if [ -z "$bucket_json" ]; then
  bucket_json="$(post CreateBucket '{"globalAlias":"guardrail"}')"
fi

bucket_id="$(printf '%s\n' "$bucket_json" | sed -n 's/.*"id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
if [ -z "$bucket_id" ]; then
  echo "Unable to determine guardrail bucket id" >&2
  exit 1
fi

echo "Ensuring guardrail access key exists..."
if ! get "GetKeyInfo?id=${GARAGE_ACCESS_KEY}" >/dev/null 2>&1; then
  post ImportKey "{\"accessKeyId\":\"${GARAGE_ACCESS_KEY}\",\"secretAccessKey\":\"${GARAGE_SECRET_KEY}\",\"name\":\"guardrail\"}" >/dev/null
fi

echo "Granting guardrail key access to the bucket..."
post AllowBucketKey "{\"bucketId\":\"$bucket_id\",\"accessKeyId\":\"${GARAGE_ACCESS_KEY}\",\"permissions\":{\"read\":true,\"write\":true,\"owner\":true}}" >/dev/null

echo "Garage setup completed successfully!"
