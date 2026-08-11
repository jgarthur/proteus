#!/usr/bin/env bash
#
# Verifies that the serial and Rayon execution paths produce identical simulations.
#
# The two paths are mutually exclusive at compile time (`#[cfg(feature = "rayon")]` /
# `#[cfg(not(...))]`), so a single test binary cannot compare them. This script builds
# `examples/parity_digest.rs` twice and diffs the replay digests, then repeats the
# Rayon run across thread counts to catch scheduling-dependent divergence.
#
# Usage: ./scripts/check-rayon-parity.sh [ticks] [thread-counts...]
set -euo pipefail

RUST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${RUST_DIR}"

TICKS="${1:-300}"
shift || true
if [[ $# -gt 0 ]]; then
  THREAD_COUNTS=("$@")
else
  THREAD_COUNTS=(1 2 4 8)
fi

if [[ ! "${TICKS}" =~ ^[1-9][0-9]*$ ]]; then
  echo "tick count must be a positive integer, got: ${TICKS}" >&2
  exit 1
fi

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "${WORK_DIR}"' EXIT

echo "==> Building serial baseline (default features)"
cargo build --quiet --example parity_digest

echo "==> Recording serial digests (${TICKS} ticks)"
cargo run --quiet --example parity_digest -- "${TICKS}" >"${WORK_DIR}/serial.txt"
sed 's/^/    /' "${WORK_DIR}/serial.txt"

# Control run. Every comparison below is between two separate processes, so a
# simulation that is not reproducible across processes (hash-order leakage, address
# dependence, uninitialised reads) would surface as a bogus "Rayon diverged" result.
# Re-running the serial build isolates that: if this fails, the problem is not Rayon.
echo "==> Control: re-running serial build in a second process"
cargo run --quiet --example parity_digest -- "${TICKS}" >"${WORK_DIR}/serial-control.txt"
if ! diff -u "${WORK_DIR}/serial.txt" "${WORK_DIR}/serial-control.txt" \
  --label "serial (run 1)" --label "serial (run 2)"; then
  echo >&2
  echo "Serial path is not reproducible across processes - this is NOT a Rayon bug." >&2
  echo "Investigate cross-process nondeterminism before trusting the parity result." >&2
  exit 1
fi
echo "    OK - serial path is reproducible across processes"

echo "==> Building Rayon path (--features rayon)"
cargo build --quiet --features rayon --example parity_digest

failed=0
for threads in "${THREAD_COUNTS[@]}"; do
  echo "==> Comparing Rayon path at RAYON_NUM_THREADS=${threads}"
  RAYON_NUM_THREADS="${threads}" cargo run --quiet --features rayon \
    --example parity_digest -- "${TICKS}" >"${WORK_DIR}/rayon-${threads}.txt"

  if diff -u "${WORK_DIR}/serial.txt" "${WORK_DIR}/rayon-${threads}.txt" \
    --label "serial" --label "rayon (${threads} threads)"; then
    echo "    OK - identical to serial baseline"
  else
    echo "    FAIL - Rayon path diverged from serial at ${threads} threads" >&2
    failed=1
  fi
done

if [[ "${failed}" -ne 0 ]]; then
  echo
  echo "Rayon parity check FAILED: the serial and Rayon paths disagree." >&2
  exit 1
fi

echo
echo "Rayon parity check passed: serial == rayon across threads ${THREAD_COUNTS[*]}."
