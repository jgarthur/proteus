#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKEND_PID=""
FRONTEND_PID=""
FRONTEND_DIR="${ROOT_DIR}/frontend"
FRONTEND_VITE_BIN="${FRONTEND_DIR}/node_modules/.bin/vite"

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

if [[ ! -x "${FRONTEND_VITE_BIN}" ]]; then
  cat <<EOF
Frontend dependencies are missing.

Run:
  cd "${FRONTEND_DIR}"
  npm install

Then rerun:
  ./start-dev.sh
EOF
  exit 1
fi

echo "Starting backend from ${ROOT_DIR}/rust"
(
  cd "${ROOT_DIR}/rust"
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
