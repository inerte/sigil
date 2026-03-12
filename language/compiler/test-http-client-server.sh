#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "========================================"
echo "HTTP Client/Server Integration Tests"
echo "========================================"
echo ""

PROJECT_DIR="${SCRIPT_DIR}/../../projects/topology-http"
PORT="45110"
SERVER_LOG="server.log"
SERVER_URL="http://127.0.0.1:${PORT}/health"

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
    wait "${SERVER_PID}" >/dev/null 2>&1 || true
  fi
  rm -f "${PROJECT_DIR}/${SERVER_LOG}"
  rm -f "${PROJECT_DIR}/src/rawEndpointClient.sigil"
}

trap cleanup EXIT

cd "${PROJECT_DIR}"
../../language/compiler/target/debug/sigil validate . --env test --human
../../language/compiler/target/debug/sigil run src/httpRoundtripServer.sigil --env test > "${SERVER_LOG}" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 50); do
  if curl --silent --fail "${SERVER_URL}" >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done

if ! curl --silent --fail "${SERVER_URL}" >/dev/null 2>&1; then
  echo "Server did not start"
  cat "${SERVER_LOG}" 2>/dev/null || true
  exit 1
fi

run_and_assert() {
  local file=$1
  local expected=$2
  local output
  output=$(../../language/compiler/target/debug/sigil run "${file}" --env test --human)
  echo "${output}"
  if ! grep -q "${expected}" <<<"${output}"; then
    echo "Expected '${expected}' from ${file}"
    exit 1
  fi
}

run_and_assert src/getClient.sigil "200:healthy"
run_and_assert src/postClient.sigil "201:echoed"
run_and_assert src/jsonClient.sigil '{"ok":true}'
run_and_assert src/headersClient.sigil "202:abc-123"
run_and_assert src/missingClient.sigil "404"

cat > src/rawEndpointClient.sigil << EOF
i stdlib⋅httpClient

λmain()→!IO String match stdlib⋅httpClient.get("http://127.0.0.1:${PORT}",stdlib⋅httpClient.emptyHeaders(),"/health"){
  Ok(response)→response.body|
  Err(error)→error.message
}
EOF

if ../../language/compiler/target/debug/sigil compile src/rawEndpointClient.sigil >/tmp/sigil-topology-http-raw.out 2>&1; then
  echo "Expected raw HTTP endpoint compile failure"
  exit 1
fi

if ! grep -q "SIGIL-TOPO-RAW-ENDPOINT-FORBIDDEN" /tmp/sigil-topology-http-raw.out; then
  echo "Expected SIGIL-TOPO-RAW-ENDPOINT-FORBIDDEN"
  cat /tmp/sigil-topology-http-raw.out
  exit 1
fi

cd ..

echo ""
echo "========================================"
echo "HTTP integration tests complete!"
echo "========================================"
