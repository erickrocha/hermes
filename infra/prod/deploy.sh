#!/usr/bin/env bash
# EPIC-XF-10-S04/S06 (HRMS-041, HRMS-045) — the deploy step.
#
# Deploying hermes is running THIS, on the production host, with IMAGE_TAG set
# to the tag the release pipeline published. It is deliberately a script and
# not a list of commands in a wiki, because the ordering is the part that must
# not be improvised:
#
#   1. refuse to run on an incomplete .env        (fail before touching anything)
#   2. pull the exact tag                          (fail before touching anything)
#   3. dump the database                           (the rollback position)
#   4. run migrations as a deploy step             (PD-033 / HRMS-045)
#   5. start the new containers
#   6. smoke-verify                                (HRMS-042)
#
# Steps 1-3 cannot damage a running system. The first step that can is 4, and
# it does not happen until a restorable dump exists.
#
# ROLLBACK: re-run this script with the previous IMAGE_TAG. If the failure was
# a schema change, restore the dump from step 3 first — see
# docs/RUNBOOK-CUTOVER.md §Rollback.

set -euo pipefail

cd "$(dirname "$0")"

COMPOSE="docker compose --env-file .env -f docker-compose.yml"

log() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
die() { printf '\n\033[31mFAILED: %s\033[0m\n' "$*" >&2; exit 1; }

# --- 1. configuration -------------------------------------------------------
[ -f .env ] || die ".env not found. Copy .env.example to .env and fill it in (HRMS-040)."

# shellcheck disable=SC1091
set -a; . ./.env; set +a

for required in HERMES_DOMAIN ACME_EMAIL API_IMAGE CONSOLE_IMAGE IMAGE_TAG \
                DATABASE_URL MARIADB_ROOT_PASSWORD MARIADB_PASSWORD \
                ACCESS_TOKEN_SECRET REFRESH_TOKEN_SECRET \
                SYSADMIN_EMAIL SYSADMIN_PASSWORD; do
    [ -n "${!required:-}" ] || die "$required is empty in .env"
done

# The API would refuse these at boot anyway (application/src/infrastructure/health.rs); catching them
# here means finding out before the database has been touched, not after.
[ "${#ACCESS_TOKEN_SECRET}" -ge 32 ]  || die "ACCESS_TOKEN_SECRET must be at least 32 bytes"
[ "${#REFRESH_TOKEN_SECRET}" -ge 32 ] || die "REFRESH_TOKEN_SECRET must be at least 32 bytes"
[ "$ACCESS_TOKEN_SECRET" != "$REFRESH_TOKEN_SECRET" ] \
    || die "ACCESS_TOKEN_SECRET and REFRESH_TOKEN_SECRET must differ"
[ "$IMAGE_TAG" != "latest" ] \
    || die "IMAGE_TAG must be an immutable tag, never 'latest' — rollback depends on it"

log "Deploying ${IMAGE_TAG} to ${HERMES_DOMAIN}"

# --- 2. pull ----------------------------------------------------------------
log "Pulling images"
$COMPOSE --profile deploy pull

# --- 3. rollback position ---------------------------------------------------
log "Backing up the database (this is the rollback position)"
$COMPOSE up -d mariadb
$COMPOSE exec -T mariadb sh -c \
    'until healthcheck.sh --connect --innodb_initialized; do sleep 2; done'

BACKUP="/backups/pre-deploy-$(date -u +%Y%m%dT%H%M%SZ)-${IMAGE_TAG}.sql"
$COMPOSE exec -T mariadb sh -c \
    "mariadb-dump --user=root --password=\"\$MARIADB_ROOT_PASSWORD\" \
     --single-transaction --routines --triggers \"\$MARIADB_DATABASE\" > ${BACKUP}" \
    || die "database backup failed — refusing to migrate without a rollback position"
log "Backup written to ${BACKUP} (volume hermes_mariadb-backups)"

# --- 4. migrate (PD-033 / HRMS-045) ----------------------------------------
log "Running migrations as a deploy step"
$COMPOSE --profile deploy run --rm migrate \
    || die "migrations failed — nothing has been restarted; the old containers are still serving"

# --- 5. start ---------------------------------------------------------------
log "Starting services"
$COMPOSE up -d --remove-orphans traefik mariadb api console

# --- 6. verify (HRMS-042) ---------------------------------------------------
log "Verifying the deployment"
./smoke.sh || die "smoke verification failed — see docs/RUNBOOK-CUTOVER.md §Rollback"

log "Deployed ${IMAGE_TAG} successfully"
