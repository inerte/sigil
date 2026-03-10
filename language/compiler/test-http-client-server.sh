#!/bin/bash
set -euo pipefail

echo "========================================"
echo "HTTP Client/Server Integration Tests"
echo "========================================"
echo ""

PROJECT_DIR="http-roundtrip-project"
PORT="45110"
SERVER_LOG="server.log"
SERVER_URL="http://127.0.0.1:${PORT}/health"

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
    wait "${SERVER_PID}" >/dev/null 2>&1 || true
  fi
  rm -rf "${PROJECT_DIR}"
}

trap cleanup EXIT

rm -rf "${PROJECT_DIR}"
mkdir -p "${PROJECT_DIR}/src"

cat > "${PROJECT_DIR}/sigil.json" << 'EOF'
{
  "layout": {
    "src": "src",
    "tests": "tests",
    "out": ".local"
  }
}
EOF

cat > "${PROJECT_DIR}/src/httpRoundtripServer.sigil" << EOF
i stdlib⋅httpServer

λhandleRequest(req:stdlib⋅httpServer.Request)→!IO stdlib⋅httpServer.Response match req.path{
  "/echo"→{
    body:req.body,
    headers:{"content-type"↦"text/plain; charset=utf-8"},
    status:201
  }|
  "/headers"→{
    body:"ok",
    headers:{
      "content-type"↦"text/plain; charset=utf-8",
      "x-request-id"↦"abc-123"
    },
    status:202
  }|
  "/json"→stdlib⋅httpServer.json("{\\"ok\\":true}",200)|
  "/health"→stdlib⋅httpServer.ok("healthy")|
  _→stdlib⋅httpServer.notFound()
}

λmain()→!IO Unit=stdlib⋅httpServer.serve(handleRequest,${PORT})
EOF

cat > "${PROJECT_DIR}/src/getClient.sigil" << EOF
i stdlib⋅httpClient
i stdlib⋅string

λmain()→!IO String match stdlib⋅httpClient.get(stdlib⋅httpClient.emptyHeaders(),"http://127.0.0.1:${PORT}/health"){
  Ok(response)→stdlib⋅string.intToString(response.status)++":"++response.body|
  Err(error)→"ERR:"++error.message
}
EOF

cat > "${PROJECT_DIR}/src/postClient.sigil" << EOF
i stdlib⋅httpClient
i stdlib⋅string

λmain()→!IO String match stdlib⋅httpClient.post("echoed",stdlib⋅httpClient.emptyHeaders(),"http://127.0.0.1:${PORT}/echo"){
  Ok(response)→stdlib⋅string.intToString(response.status)++":"++response.body|
  Err(error)→"ERR:"++error.message
}
EOF

cat > "${PROJECT_DIR}/src/jsonClient.sigil" << EOF
i stdlib⋅httpClient
i stdlib⋅json

λmain()→!IO String match stdlib⋅httpClient.getJson(stdlib⋅httpClient.emptyHeaders(),"http://127.0.0.1:${PORT}/json"){
  Ok(value)→stdlib⋅json.stringify(value)|
  Err(error)→"ERR:"++error.message
}
EOF

cat > "${PROJECT_DIR}/src/headersClient.sigil" << EOF
i core⋅map
i stdlib⋅httpClient
i stdlib⋅string

λmain()→!IO String match stdlib⋅httpClient.get(stdlib⋅httpClient.emptyHeaders(),"http://127.0.0.1:${PORT}/headers"){
  Ok(response)→match core⋅map.get("x-request-id",response.headers){
    Some(value)→stdlib⋅string.intToString(response.status)++":"++value|
    None()→"ERR:missing-header"
  }|
  Err(error)→"ERR:"++error.message
}
EOF

cat > "${PROJECT_DIR}/src/missingClient.sigil" << EOF
i stdlib⋅httpClient
i stdlib⋅string

λmain()→!IO String match stdlib⋅httpClient.get(stdlib⋅httpClient.emptyHeaders(),"http://127.0.0.1:${PORT}/missing"){
  Ok(response)→stdlib⋅string.intToString(response.status)|
  Err(error)→"ERR:"++error.message
}
EOF

cd "${PROJECT_DIR}"
../target/debug/sigil run src/httpRoundtripServer.sigil > server.log 2>&1 &
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
  output=$(../target/debug/sigil run "${file}" --human)
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

cd ..

echo ""
echo "========================================"
echo "HTTP integration tests complete!"
echo "========================================"
