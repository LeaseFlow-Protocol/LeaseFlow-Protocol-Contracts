#!/usr/bin/env bash
# check_wasm_size.sh – Issue #130: post-build wasm size assertion
#
# Usage:
#   ./scripts/check_wasm_size.sh [MAX_BYTES]
#
# Defaults to 100 000 bytes (100 kB) if MAX_BYTES is not supplied.
# Exits non-zero if the compiled .wasm exceeds the limit.

set -euo pipefail

MAX_BYTES="${1:-100000}"
WASM_PATH="target/wasm32-unknown-unknown/release/leaseflow_contracts.wasm"

if [[ ! -f "$WASM_PATH" ]]; then
  echo "ERROR: wasm not found at $WASM_PATH – run 'cargo build --release --target wasm32-unknown-unknown' first." >&2
  exit 1
fi

ACTUAL=$(wc -c < "$WASM_PATH")
echo "wasm size: ${ACTUAL} bytes  (limit: ${MAX_BYTES} bytes)"

if (( ACTUAL > MAX_BYTES )); then
  echo "FAIL: wasm exceeds size limit by $((ACTUAL - MAX_BYTES)) bytes." >&2
  exit 1
fi

echo "PASS: wasm is within the size limit."
