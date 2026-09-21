#!/usr/bin/env bash
# EPIC-XF-10-S05 (HRMS-042) — post-deployment verification.
#
# "Report a failed deployment as failed rather than as silently running."
#
# Two checks, and the second is the one that matters. A health endpoint proves
# the process is up and can reach MariaDB. It does NOT prove the deployment
# works: a wrong CORS origin, a token secret that changed without the database
# knowing, a missing SysAdmin seed or a console served from the wrong origin
# all leave /health perfectly green. So this also performs a real authenticated
# request end to end, through the public hostname, through Traefik, through the
# prefix strip, into the API, and back.
#
# Exit code is the contract: 0 means the deployment is verified, anything else
# means it is not. deploy.sh treats a non-zero exit as a failed deploy.

set -euo pipefail

cd "$(dirname "$0")"

if [ -f .env ]; then
    # shellcheck disable=SC1091
    set -a; . ./.env; set +a
fi

BASE_URL="${SMOKE_BASE_URL:-https://${HERMES_DOMAIN:?HERMES_DOMAIN is required}}"
EMAIL="${SYSADMIN_EMAIL:?SYSADMIN_EMAIL is required}"
PASSWORD="${SYSADMIN_PASSWORD:?SYSADMIN_PASSWORD is required}"

pass() { printf '  \033[32mPASS\033[0m  %s\n' "$*"; }
fail() { printf '  \033[31mFAIL\033[0m  %s\n' "$*" >&2; exit 1; }

printf '\nSmoke verification against %s\n\n' "$BASE_URL"

# --- 1. health (is it up, and can it reach the database?) -------------------
health_body="$(mktemp)"
# Assigned up front because the trap below is armed before the login check
# runs: under `set -u` an early fail() would otherwise make the trap itself
# error on an unbound $login_body and skip the cleanup it exists to do.
login_body=""
trap 'rm -f "$health_body" "$login_body" 2>/dev/null || true' EXIT

# `|| true`, never `|| echo "000"`: --write-out has already printed a
# http_code of 000 by the time curl fails, so echoing another one produced
# the misleading six-digit "000000" seen in the 2026-09-21 rehearsal.
health_code="$(curl --silent --show-error --location --max-time 15 \
    --output "$health_body" --write-out '%{http_code}' \
    "${BASE_URL}/api/health" || true)"

[ "$health_code" = "200" ] \
    || fail "GET /api/health returned ${health_code} (expected 200). Body: $(cat "$health_body")"
grep -q '"database":"up"' "$health_body" \
    || fail "/api/health did not report the database as up: $(cat "$health_body")"
pass "GET /api/health -> 200, database up"

# --- 2. an authenticated request (does the deployment actually work?) -------
# POST /login takes a form body, not JSON (endpoints/auth_endpoint.rs).
login_body="$(mktemp)"
login_code="$(curl --silent --show-error --location --max-time 15 \
    --output "$login_body" --write-out '%{http_code}' \
    --request POST "${BASE_URL}/api/login" \
    --data-urlencode "email=${EMAIL}" \
    --data-urlencode "password=${PASSWORD}" || true)"

[ "$login_code" = "200" ] \
    || fail "POST /api/login returned ${login_code} (expected 200). The SysAdmin seed, the token secrets or the routing are wrong."

grep -q 'accessToken\|access_token' "$login_body" \
    || fail "POST /api/login returned 200 without a token: $(cat "$login_body")"
pass "POST /api/login -> 200 with an access token"

# --- 3. the console is actually served --------------------------------------
console_code="$(curl --silent --show-error --location --max-time 15 \
    --output /dev/null --write-out '%{http_code}' "${BASE_URL}/" || true)"
[ "$console_code" = "200" ] \
    || fail "GET / returned ${console_code} (expected 200) — the console is not being served"
pass "GET / -> 200, console served"

printf '\n\033[32mDeployment verified.\033[0m\n\n'
