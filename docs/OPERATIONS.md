# Operating hermes in production

> **EPIC-XF-10-S08 · `HRMS-044` · `D-18`**
>
> Where the logs go, what is watched, and who is called. The requirement is
> blunt about why this exists: *"so that a failure after cutover is observed
> rather than reported by a user."*
>
> Deployment procedure is [`RUNBOOK-CUTOVER.md`](RUNBOOK-CUTOVER.md). This file
> is about the days after.

---

## 1. What runs

Per `D-18`: **a single VPS running Docker Compose behind Traefik**, defined by
[`infra/prod/docker-compose.yml`](../infra/prod/docker-compose.yml).

| Container | What it is | Exposed |
|---|---|---|
| `traefik` | TLS termination, Let's Encrypt, routing | **:80, :443 — the only public ports** |
| `api` | `hermes_server`, the Rust API | via Traefik, under `/api` |
| `console` | the React backoffice on nginx | via Traefik, everything else |
| `mariadb` | the database | **not published** — compose network only |
| `migrate` | one-shot deploy step, `profiles: [deploy]` | never running |

Both the console and the API answer on one hostname. `/api/*` goes to the API
with the prefix stripped; everything else is the console.

---

## 2. Where the logs go

Every container uses the `json-file` driver with **10 MB × 5 files** per
container. That is the honest, current answer, and it has two consequences
worth stating plainly:

- logs are **on the VPS only**. If the host is lost, they are lost with it;
- they **rotate**. Under load, "last week" may not exist.

```bash
cd $PRODUCTION_PATH/infra/prod

docker compose logs -f --tail=200 api        # follow the API
docker compose logs --since 1h api           # the last hour
docker compose logs --since 30m traefik      # HTTP-level view: status codes, TLS
docker compose logs mariadb | tail -100
```

`RUST_LOG` controls API verbosity and is `info` in production. **Do not run
`debug` in production** beyond a short, deliberate window: it is noisy enough
to rotate the interesting entries away, and it logs request detail belonging to
a tenant.

> **Not built, and a real gap.** There is no log *shipping* — no central
> collector, no retention beyond rotation, no search. For one tenant on one
> host that is a defensible starting point, but it should not survive the second
> tenant. Whoever picks that up owns raising it as a new decision; do not treat
> §2 as a finished answer.

---

## 3. What is monitored, and who is called

`HRMS-044` requires *"a stated set of monitored conditions and a named
recipient for each"*. Fill the names in before cutover — a table of roles with
nobody's name in it is the failure mode this requirement exists to prevent.

| # | Condition | How it is detected | Severity | Recipient |
|---|---|---|---|---|
| 1 | **The site is down** — `GET /` is not 200 | External uptime check every 60s | **Page immediately** | _cutover lead_ |
| 2 | **The API is unhealthy** — `GET /api/health` is not 200, or reports `"database":"down"` | External uptime check every 60s | **Page immediately** | _cutover lead_ |
| 3 | **TLS certificate expiring** within 14 days | Let's Encrypt mail to `ACME_EMAIL`, plus an external check | High | _tech lead_ |
| 4 | **Disk above 80%** on the VPS | Host check | High — MariaDB corrupts on a full disk | _tech lead_ |
| 5 | **A container is restarting repeatedly** | `docker compose ps` shows restarts climbing | High | _tech lead_ |
| 6 | **The API refused to start** — `Refusing to start:` or `Database schema is N migration(s) behind` in the log | Log inspection after every deploy | High | _cutover lead_ |
| 7 | **Login failures spike** | Traefik access log, 401 rate | Medium — credential stuffing | _tech lead_ |
| 8 | **PinME ingestion failing** (`EPIC-FT-01`) | API log: tracking gateway errors | Medium — telemetry stale, not an outage | _tech lead_ |
| 9 | **Invitation email failing** (`EPIC-IA-07`) | API log: the loud SMTP warning | Low — accounts still work, links need hand-delivery | _product owner_ |

**Conditions 1 and 2 need an external checker** — something not on this VPS,
because a host that is down cannot report that it is down. Any hosted uptime
service pointed at `https://$HERMES_DOMAIN/api/health` satisfies both; the
endpoint already distinguishes "process up" from "process up but database
unreachable" (`HRMS-042`), which is the distinction that matters.

> **Nothing above is automated yet.** The endpoints and log lines exist; the
> checker and the alert routing are configuration on a monitoring service that
> `D-18` did not decide. Until they are configured, conditions 1–9 are a manual
> checklist, and §4 is how they are checked.

---

## 4. Daily check

Two minutes, once a day, until monitoring is automated.

```bash
cd $PRODUCTION_PATH/infra/prod

docker compose ps                                  # all Up, restarts not climbing
curl -sS https://$HERMES_DOMAIN/api/health          # {"status":"ok","database":"up"}
df -h /                                            # under 80%
docker compose logs --since 24h api | grep -iE 'error|refusing|panic' | tail -20
```

---

## 5. Routine operations

### Deploy or roll back

Both are the same action — see [`RUNBOOK-CUTOVER.md`](RUNBOOK-CUTOVER.md) §3
and §6. Rolling back is redeploying the previous `IMAGE_TAG`.

### Restart the API

```bash
docker compose restart api
```

Safe: the API applies no migrations at boot (`PD-033`), and refuses to start if
the schema is behind rather than changing it.

### Change a secret

Edit `infra/prod/.env`, then `docker compose up -d api`. Note the two that
behave differently:

- **`SYSADMIN_PASSWORD`** is re-applied at every boot (`HRMS-024`). Per `D-17`
  there is no rotation mechanism and none will be built — editing `.env` and
  restarting *is* the procedure.
- **`ACCESS_TOKEN_SECRET` / `REFRESH_TOKEN_SECRET`** invalidate every issued
  token when changed. All users are logged out. Do it deliberately.

### Back up the database

`deploy.sh` dumps before every deploy, into the `mariadb-backups` volume. **That
is a rollback position, not a backup.** It is on the same host as the data it
protects, so it survives a bad deploy and does not survive losing the VPS.

A scheduled **off-host** backup is **not built**. Until it is, this is the
manual version, and it should be run on a schedule someone owns:

```bash
docker compose exec -T mariadb sh -c \
  'mariadb-dump --user=root --password="$MARIADB_ROOT_PASSWORD" \
   --single-transaction --routines --triggers "$MARIADB_DATABASE"' \
  | gzip > hermes-$(date -u +%Y%m%dT%H%M%SZ).sql.gz
# then copy it OFF this host.
```

> Restoring has never been rehearsed against production-shaped data. A backup
> nobody has restored is a hypothesis.

---

## 6. Known gaps

Listed so they are decided rather than discovered:

| Gap | Consequence | Where it belongs |
|---|---|---|
| No off-host backup schedule | Losing the VPS loses Transmega's data | Next decision after `D-18` |
| No log shipping or retention | No forensics beyond rotation | Same |
| No automated alerting | §3 depends on someone running §4 | Same |
| Single host, single API container | Every deploy is a short outage; no failover | Accepted for one tenant |
| Restore never rehearsed | The rollback position is unproven | Rehearse before cutover |
