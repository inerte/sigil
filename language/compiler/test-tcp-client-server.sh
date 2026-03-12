#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "========================================"
echo "TCP Client/Server Integration Tests"
echo "========================================"
echo ""

PROJECT_DIR="${SCRIPT_DIR}/../../projects/topology-tcp"
PORT="45120"
SERVER_LOG="server.log"

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
    wait "${SERVER_PID}" >/dev/null 2>&1 || true
  fi
  rm -f "${PROJECT_DIR}/${SERVER_LOG}"
  rm -f "${PROJECT_DIR}/src/rawTcpClient.sigil"
}

trap cleanup EXIT

cd "${PROJECT_DIR}"
../../language/compiler/target/debug/sigil validate . --env test --human
../../language/compiler/target/debug/sigil run src/tcpRoundtripServer.sigil --env test > "${SERVER_LOG}" 2>&1 &
SERVER_PID=$!

node - <<EOF
const net = require('node:net');
const port = ${PORT};
let tries = 0;
function attempt() {
  tries += 1;
  const socket = net.createConnection({ host: '127.0.0.1', port }, () => {
    socket.end();
    process.exit(0);
  });
  socket.once('error', () => {
    socket.destroy();
    if (tries >= 50) process.exit(1);
    setTimeout(attempt, 200);
  });
}
attempt();
EOF

if [[ $? -ne 0 ]]; then
  echo "TCP server did not start"
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

run_and_assert src/pingClient.sigil "pong"
run_and_assert src/echoClient.sigil "echoed"
run_and_assert src/upperClient.sigil "HELLO"

cat > src/rawTcpClient.sigil << EOF
i stdlib⋅tcpClient

λmain()→!IO String match stdlib⋅tcpClient.send("ping","127.0.0.1"){
  Ok(response)→response.message|
  Err(error)→error.message
}
EOF

if ../../language/compiler/target/debug/sigil compile src/rawTcpClient.sigil >/tmp/sigil-topology-tcp-raw.out 2>&1; then
  echo "Expected raw TCP endpoint compile failure"
  exit 1
fi

if ! grep -q "SIGIL-TOPO-RAW-ENDPOINT-FORBIDDEN" /tmp/sigil-topology-tcp-raw.out; then
  echo "Expected SIGIL-TOPO-RAW-ENDPOINT-FORBIDDEN"
  cat /tmp/sigil-topology-tcp-raw.out
  exit 1
fi

cd ..

echo ""
echo "========================================"
echo "TCP integration tests complete!"
echo "========================================"
