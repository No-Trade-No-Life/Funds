#!/usr/bin/env bash
set -euo pipefail

HTTP_PORT="${HTTP_PORT:-$((30000 + $$ % 10000))}"
HTTP_ADDR="${HTTP_ADDR:-127.0.0.1:${HTTP_PORT}}"
BASE_URL="${BASE_URL:-http://${HTTP_ADDR}}"
DATABASE_PATH="${DATABASE_PATH:-$(mktemp -t funds-http-api.XXXXXX.sqlite)}"
SERVER_LOG="$(mktemp -t funds-http-api.XXXXXX.log)"
SERVER_PID=""

cleanup() {
  if [[ -n "${SERVER_PID}" ]]; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
    wait "${SERVER_PID}" >/dev/null 2>&1 || true
  fi

  rm -f "${DATABASE_PATH}" "${SERVER_LOG}"
}

trap cleanup EXIT

start_server() {
  DATABASE_PATH="${DATABASE_PATH}" HTTP_ADDR="${HTTP_ADDR}" cargo run >"${SERVER_LOG}" 2>&1 &
  SERVER_PID="$!"

  for _ in {1..60}; do
    if curl --silent --fail "${BASE_URL}/health" >/dev/null; then
      return
    fi

    sleep 0.25
  done

  printf 'HTTP service did not become ready. Server log:\n' >&2
  sed 's/^/  /' "${SERVER_LOG}" >&2
  exit 1
}

request() {
  local method="$1"
  local path="$2"
  local body="${3:-}"
  local response_file
  local status

  response_file="$(mktemp -t funds-http-api-response.XXXXXX.json)"

  if [[ -n "${body}" ]]; then
    status="$(curl --silent --show-error --output "${response_file}" --write-out '%{http_code}' \
      --request "${method}" \
      --header 'Content-Type: application/json' \
      --data "${body}" \
      "${BASE_URL}${path}")"
  else
    status="$(curl --silent --show-error --output "${response_file}" --write-out '%{http_code}' \
      --request "${method}" \
      "${BASE_URL}${path}")"
  fi

  printf '%s\t%s\n' "${status}" "${response_file}"
}

assert_status() {
  local actual="$1"
  local expected="$2"
  local response_file="$3"

  if [[ "${actual}" != "${expected}" ]]; then
    printf 'Expected HTTP %s, got %s. Body:\n' "${expected}" "${actual}" >&2
    sed 's/^/  /' "${response_file}" >&2
    exit 1
  fi
}

assert_json() {
  local response_file="$1"
  local expression="$2"

  node -e "const fs = require('fs'); const body = JSON.parse(fs.readFileSync(process.argv[1], 'utf8')); if (!(${expression})) process.exit(1);" "${response_file}" || {
    printf 'JSON assertion failed: %s\nBody:\n' "${expression}" >&2
    sed 's/^/  /' "${response_file}" >&2
    exit 1
  }
}

call_api() {
  local method="$1"
  local path="$2"
  local expected_status="$3"
  local body="${4:-}"
  local result
  local status
  local response_file

  result="$(request "${method}" "${path}" "${body}")"
  status="${result%%$'\t'*}"
  response_file="${result#*$'\t'}"
  assert_status "${status}" "${expected_status}" "${response_file}"
  printf '%s' "${response_file}"
}

start_server

health_response="$(call_api GET /health 200)"
grep -q '^ok$' "${health_response}" || {
  printf 'Health response was not ok. Body:\n' >&2
  sed 's/^/  /' "${health_response}" >&2
  exit 1
}

fund_response="$(call_api POST /funds 201 '{"account_id":"fund/main","description":"主基金"}')"
assert_json "${fund_response}" "body.account_id === 'fund/main' && body.state.summary.unit_price === 1"

duplicate_fund_response="$(call_api POST /funds 409 '{"account_id":"fund/main","description":"主基金"}')"
assert_json "${duplicate_fund_response}" "body.message.includes('already exists')"

deposit_response="$(call_api POST /funds/fund%2Fmain/events 200 '{"updated_at":"2026-06-09T00:00:00Z","comment":"张三入金","fund_equity":null,"order":{"name":"张三","deposit":1000},"investor":null,"taxation":null}')"
assert_json "${deposit_response}" "body.state.total_assets === 1000 && body.state.investors['张三'].share === 1000"

equity_response="$(call_api POST /funds/fund%2Fmain/events 200 '{"updated_at":"2026-06-10T00:00:00Z","comment":"更新权益","fund_equity":{"equity":1200},"order":null,"investor":null,"taxation":null}')"
assert_json "${equity_response}" "body.state.summary.unit_price === 1.2"

fund_list_response="$(call_api GET /funds 200)"
assert_json "${fund_list_response}" "Array.isArray(body) && body.length === 1 && body[0].events.length === 2"

missing_fund_response="$(call_api GET /funds/missing 404)"
assert_json "${missing_fund_response}" "body.message.includes('was not found')"

credential_response="$(call_api POST /credentials 201 '{"label":"OKX 主账户","exchange":"okx","payload":{"access_key":"access","secret_key":"secret","passphrase":"pass"}}')"
assert_json "${credential_response}" "body.id === 'credential-1' && body.label === 'OKX 主账户' && body.payload === undefined"

credential_list_response="$(call_api GET /credentials 200)"
assert_json "${credential_list_response}" "Array.isArray(body) && body.length === 1 && body[0].id === 'credential-1' && body[0].payload === undefined"

credential_summary_response="$(call_api GET /credentials/credential-1/capital-summary 200)"
assert_json "${credential_summary_response}" "body.credential_id === 'credential-1' && ['ok', 'failed'].includes(body.status)"

capital_summary_response="$(call_api GET /capital-summary 200)"
assert_json "${capital_summary_response}" "Array.isArray(body.accounts) && body.accounts.length === 1"

delete_credential_response="$(call_api DELETE /credentials/credential-1 204)"
[[ ! -s "${delete_credential_response}" ]] || {
  printf 'DELETE response should be empty. Body:\n' >&2
  sed 's/^/  /' "${delete_credential_response}" >&2
  exit 1
}

empty_credential_list_response="$(call_api GET /credentials 200)"
assert_json "${empty_credential_list_response}" "Array.isArray(body) && body.length === 0"

printf 'HTTP API smoke test passed.\n'
