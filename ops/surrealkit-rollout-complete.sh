#!/bin/sh

set -eu

ROLLOUT_DIR="${SURREALKIT_ROLLOUT_DIR:-/app/database/rollouts}"
DATABASE_WAIT_TIMEOUT_SECONDS="${DATABASE_WAIT_TIMEOUT_SECONDS:-120}"
DATABASE_WAIT_INTERVAL_SECONDS="${DATABASE_WAIT_INTERVAL_SECONDS:-5}"

latest_rollout() {
    find "$ROLLOUT_DIR" -maxdepth 1 -type f -name '*.toml' | sort | tail -n 1
}

wait_for_database() {
    remaining_timeout="$DATABASE_WAIT_TIMEOUT_SECONDS"

    while true; do
        if STATUS_OUTPUT="$(surrealkit rollout status "$ROLLOUT_ID" 2>&1)"; then
            return 0
        fi

        case "$STATUS_OUTPUT" in
            *"Failed connecting to"*|*"Connection refused"*|*"IO error:"*|*"dns error:"*)
                if [ "$remaining_timeout" -le 0 ]; then
                    printf '%s\n' "$STATUS_OUTPUT"
                    echo "Timed out waiting for database connectivity"
                    exit 1
                fi

                echo "Waiting for database connectivity..."
                sleep "$DATABASE_WAIT_INTERVAL_SECONDS"
                remaining_timeout=$((remaining_timeout-DATABASE_WAIT_INTERVAL_SECONDS))
                ;;
            *)
                return 0
                ;;
        esac
    done
}

ROLLOUT_PATH="$(latest_rollout)"

if [ -z "$ROLLOUT_PATH" ]; then
    echo "No rollout manifests found in $ROLLOUT_DIR; skipping rollout complete."
    exit 0
fi

ROLLOUT_ID="$(basename "$ROLLOUT_PATH" .toml)"

echo "Selected rollout manifest: $ROLLOUT_ID"
surrealkit rollout lint "$ROLLOUT_ID"

wait_for_database
STATUS_LINE="$(printf '%s\n' "$STATUS_OUTPUT" | grep "$ROLLOUT_ID " | head -n 1 || true)"

if [ -z "$STATUS_LINE" ]; then
    if printf '%s\n' "$STATUS_OUTPUT" | grep -q "No rollout records found."; then
        echo "Rollout $ROLLOUT_ID has not been started; skipping rollout complete."
        exit 0
    fi

    printf '%s\n' "$STATUS_OUTPUT"
    echo "Unable to determine rollout status for $ROLLOUT_ID"
    exit 1
fi

printf '%s\n' "$STATUS_LINE"

case "$STATUS_LINE" in
    *"[ready_to_complete]"*)
        echo "Completing rollout $ROLLOUT_ID"
        exec surrealkit rollout complete "$ROLLOUT_ID"
        ;;
    *"[completed]"*|*"[running_complete]"*)
        echo "Rollout $ROLLOUT_ID is already completing or completed; skipping rollout complete."
        exit 0
        ;;
    *"[failed]"*|*"[rolled_back]"*|*"[running_rollback]"*)
        echo "Rollout $ROLLOUT_ID is in a non-completable state."
        exit 1
        ;;
    *)
        echo "Rollout $ROLLOUT_ID is not ready to complete yet."
        exit 0
        ;;
esac
