#!/usr/bin/env bash
# Implantação do Hermes em rede interna (docker-compose.internal.yml): constrói as imagens, faz o dump do
# banco (posição de rollback), roda as migrações e só então sobe a API e o console.
#
# Voltar atrás: reexecute com a IMAGE_TAG anterior (e restaure o dump de antes da migração, se a falha
# foi de esquema) — veja docs/RUNBOOK-CUTOVER.md.

set -euo pipefail

cd "$(dirname "$0")"

COMPOSE="docker compose --env-file .env -f docker-compose.internal.yml"

log() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
die() { printf '\n\033[31mFALHOU: %s\033[0m\n' "$*" >&2; exit 1; }

[ -f .env ] || die ".env não encontrado. Copie .env.internal.example para .env e preencha."

# shellcheck disable=SC1091
set -a; . ./.env; set +a

for required in HERMES_URL IMAGE_TAG DATABASE_URL MARIADB_ROOT_PASSWORD MARIADB_PASSWORD \
                ACCESS_TOKEN_SECRET REFRESH_TOKEN_SECRET SYSADMIN_EMAIL SYSADMIN_PASSWORD; do
    [ -n "${!required:-}" ] || die "$required está vazio no .env"
done
[ "${#ACCESS_TOKEN_SECRET}" -ge 32 ]  || die "ACCESS_TOKEN_SECRET precisa de pelo menos 32 bytes"
[ "${#REFRESH_TOKEN_SECRET}" -ge 32 ] || die "REFRESH_TOKEN_SECRET precisa de pelo menos 32 bytes"
[ "$ACCESS_TOKEN_SECRET" != "$REFRESH_TOKEN_SECRET" ] || die "ACCESS_TOKEN_SECRET e REFRESH_TOKEN_SECRET precisam ser diferentes"
case "$DATABASE_URL" in *TROQUE-PELA*) die "DATABASE_URL ainda tem a senha de exemplo";; esac
case "$HERMES_URL" in */) die "HERMES_URL não pode terminar em /";; esac
if [ -n "${TRACKING_API_BASE_URL:-}" ] && { [ -z "${TRACKING_API_EMAIL:-}" ] || [ -z "${TRACKING_API_PASSWORD:-}" ]; }; then
    printf 'AVISO: TRACKING_API_EMAIL/TRACKING_API_PASSWORD vazios: o rastreamento (PinME) ficará desligado.\n' >&2
fi

log "Construindo as imagens (${IMAGE_TAG})"
$COMPOSE --profile deploy build

log "Dump do banco (posição de rollback)"
$COMPOSE up -d mariadb
$COMPOSE exec -T mariadb sh -c 'until healthcheck.sh --connect --innodb_initialized; do sleep 2; done'
BACKUP="/backups/pre-deploy-$(date -u +%Y%m%dT%H%M%SZ)-${IMAGE_TAG}.sql"
$COMPOSE exec -T mariadb sh -c \
    "mariadb-dump --user=root --password=\"\$MARIADB_ROOT_PASSWORD\" --single-transaction --routines --triggers \"\$MARIADB_DATABASE\" > ${BACKUP}" \
    || die "o dump falhou — não vou migrar sem posição de rollback"
log "Dump gravado em ${BACKUP} (volume hermes_mariadb-backups)"

log "Rodando as migrações"
$COMPOSE --profile deploy run --rm migrate || die "as migrações falharam — nada foi reiniciado"

log "Subindo os serviços"
$COMPOSE up -d --remove-orphans mariadb api console

log "Aguardando a API responder"
for _ in $(seq 1 45); do
    curl --silent --fail --max-time 3 --output /dev/null "${HERMES_URL}/api/health" && break
    sleep 2
done

log "Verificando"
SMOKE_BASE_URL="$HERMES_URL" ./smoke.sh || die "a verificação falhou — veja docs/RUNBOOK-CUTOVER.md"

log "Implantado ${IMAGE_TAG} em ${HERMES_URL}"
