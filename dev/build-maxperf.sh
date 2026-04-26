#!/usr/bin/env sh
set -eu

# Local-only helper for strongest optimization on the current machine.
# Set GUARDRAIL_MAXPERF_NATIVE=0 to produce a less CPU-specific binary.
if [ "${GUARDRAIL_MAXPERF_NATIVE:-1}" = "1" ]; then
  case " ${RUSTFLAGS:-} " in
    *" -C target-cpu="*) ;;
    *) export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C target-cpu=native" ;;
  esac
fi

exec cargo build --profile maxperf "$@"
