#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKEND_PID=""
FRONTEND_PID=""
FRONTEND_DIR="${ROOT_DIR}/frontend"
FRONTEND_VITE_BIN="${FRONTEND_DIR}/node_modules/.bin/vite"
RAYON_THREADS="${RAYON_NUM_THREADS:-}"

usage() {
  cat <<EOF
Usage: ./start-dev.sh [--threads N]

Options:
  --threads N  Set RAYON_NUM_THREADS for the backend process.
  -h, --help   Show this help text.

Environment:
  RAYON_NUM_THREADS  Default backend Rayon thread count when --threads is not provided.
EOF
}

cleanup() {
  local exit_code="${1:-$?}"

  trap - EXIT INT TERM

  if [[ -n "${FRONTEND_PID}" ]]; then
    kill "${FRONTEND_PID}" 2>/dev/null || true
    wait "${FRONTEND_PID}" 2>/dev/null || true
  fi

  if [[ -n "${BACKEND_PID}" ]]; then
    kill "${BACKEND_PID}" 2>/dev/null || true
    wait "${BACKEND_PID}" 2>/dev/null || true
  fi

  exit "${exit_code}"
}

wait_for_first_exit() {
  while true; do
    if ! kill -0 "${BACKEND_PID}" 2>/dev/null; then
      if wait "${BACKEND_PID}"; then
        return 0
      fi
      return $?
    fi

    if ! kill -0 "${FRONTEND_PID}" 2>/dev/null; then
      if wait "${FRONTEND_PID}"; then
        return 0
      fi
      return $?
    fi

    sleep 1
  done
}

trap 'cleanup $?' EXIT
trap 'cleanup 130' INT TERM

while [[ $# -gt 0 ]]; do
  case "$1" in
    --threads)
      if [[ $# -lt 2 ]]; then
        echo "--threads requires a positive integer argument" >&2
        usage >&2
        exit 1
      fi
      RAYON_THREADS="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -n "${RAYON_THREADS}" && ! "${RAYON_THREADS}" =~ ^[1-9][0-9]*$ ]]; then
  echo "Rayon thread count must be a positive integer, got: ${RAYON_THREADS}" >&2
  exit 1
fi

if [[ ! -x "${FRONTEND_VITE_BIN}" ]]; then
  cat <<EOF
Frontend dependencies are missing.

Run:
  cd "${FRONTEND_DIR}"
  npm install

Then rerun:
  ./start-dev.sh [--threads N]
EOF
  exit 1
fi

if [[ -n "${RAYON_THREADS}" ]]; then
  echo "Starting backend from ${ROOT_DIR}/rust with RAYON_NUM_THREADS=${RAYON_THREADS}"
else
  echo "Starting backend from ${ROOT_DIR}/rust"
fi
(
  cd "${ROOT_DIR}/rust"
  if [[ -n "${RAYON_THREADS}" ]]; then
    export RAYON_NUM_THREADS="${RAYON_THREADS}"
  fi
  exec cargo run --features web --bin proteus-server
) &
BACKEND_PID=$!

echo "Starting frontend from ${ROOT_DIR}/frontend"
(
  cd "${FRONTEND_DIR}"
  exec "${FRONTEND_VITE_BIN}"
) &
FRONTEND_PID=$!
wait_for_first_exit
