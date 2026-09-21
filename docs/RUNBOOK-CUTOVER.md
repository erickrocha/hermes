# Cutover runbook — onboarding Transmega onto hermes

> **EPIC-XF-10-S07 · `HRMS-043` · `PD-018` · `D-02`**
>
> This is the procedure for putting hermes into production and onboarding its
> first tenant. It exists because `D-02` committed to *"everything running in
> production by 2026-10-17"* and, until 2026-09-21, no document said how.
>
> **Read the whole thing before starting.** The point of a runbook is that
> nobody is deciding anything at 22:00 on cutover night.

---

## 0. Who

| Role | Responsibility on the night |
|---|---|
| **Cutover lead** | Runs every command in §3. The only person who types. |
| **Product owner** | Owns the go/no-go at each gate and the §6 rollback trigger. Confirms Transmega's data looks right. |
| **Transmega contact** | Available by phone. Confirms they can log in and that the branding is theirs. |

One person runs the commands. Everyone else watches. If the lead is unsure at
any gate, the answer is **stop and call the owner**, never improvise.

---

## 1. Prerequisites — days before, not on the night

Nothing in §3 starts until every line here is true.

| # | Item | How it is confirmed | Owner |
|---|---|---|---|
| 1 | **`D-18` is settled**: a VPS exists, sized and paid for, and someone is named as its operator | The operator can SSH in | Owner + tech lead |
| 2 | DNS for `$HERMES_DOMAIN` points at the VPS, TTL lowered to 300s at least 24h ahead | `dig +short $HERMES_DOMAIN` | Cutover lead |
| 3 | Docker Engine + Compose v2 installed on the VPS | `docker compose version` | Cutover lead |
| 4 | Repository checked out on the host at `$PRODUCTION_PATH` | `ls infra/prod/docker-compose.yml` | Cutover lead |
| 5 | `infra/prod/.env` created from `.env.example`, **every** value filled | `./deploy.sh` refuses otherwise | Cutover lead |
| 6 | Token secrets generated **on the host** with `openssl rand -hex 32`, and the two differ | `deploy.sh` checks both | Cutover lead |
| 7 | A release tag exists and its images are published | `docker manifest inspect $API_IMAGE:$IMAGE_TAG` | Tech lead |
| 8 | **Transmega's palette is signed off** (`EPIC-XF-10-S09`, `HRMS-409`, `D-12`) | Written confirmation from Transmega | Product owner |
| 9 | Ports 80 and 443 open to the world; **3306 is not** | `ss -tlnp` on the host | Cutover lead |
| 10 | Rehearsed: §3 run end to end against a throwaway host or domain | The rehearsal's smoke check passed | Cutover lead |

> **Item 8 is the one that will slip.** It is the only item here that depends on
> another company's marketing department rather than on engineering, and
> `EPIC-BO-01` is already accepted with a *draft* palette (commit `8acb161`),
> which makes it easy to believe it is done. It is not done. Start it first.

> **Item 10 is the one that will be skipped.** Don't. A rehearsal is the only
> thing that turns this document from a plan into a procedure.

---

## 2. Freeze

From the start of §3 until §5 passes:

- no merges to `develop`,
- no new release tags,
- no schema changes.

---

## 3. The cutover

Every step below is run from `infra/prod` on the production host.

### 3.1 Deploy

```bash
cd $PRODUCTION_PATH/infra/prod
./deploy.sh
```

`deploy.sh` does this, in this order, and stops at the first failure:

| Step | What it does | Can it damage a running system? |
|---|---|---|
| 1 | Refuses to run on an incomplete `.env` | No |
| 2 | Pulls the exact `IMAGE_TAG` | No |
| 3 | **Dumps the database** into the `mariadb-backups` volume | No |
| 4 | Runs migrations as a deploy step (`PD-033`, `HRMS-045`) | **Yes — first destructive step** |
| 5 | Starts Traefik, MariaDB, the API and the console | Yes |
| 6 | Runs `smoke.sh` (`HRMS-042`) | No |

Steps 1–3 are all reversible by doing nothing. The first step that changes
anything is 4, and it does not run until a restorable dump exists.

> **GATE A — the deploy script exited 0.** If it did not, stop. Go to §6.
> Do not re-run it "to see if it works this time".

### 3.2 TLS

The first `docker compose up` triggers a Let's Encrypt TLS-ALPN challenge.
It fails if DNS is wrong or port 443 is blocked — which is why those are §1
items.

```bash
curl -sSI https://$HERMES_DOMAIN/ | head -1        # expect HTTP/2 200
curl -sS  https://$HERMES_DOMAIN/api/health        # expect {"status":"ok","database":"up"}
```

> **GATE B — TLS is valid and `/api/health` reports the database up.**
> Let's Encrypt rate-limits failures. If the certificate did not issue, fix DNS
> or the firewall and wait; do not loop `deploy.sh`.

### 3.3 First login

The SysAdmin is seeded from `.env` at every boot (`HRMS-024`).

```bash
./smoke.sh
```

Then log in through the browser at `https://$HERMES_DOMAIN` as
`$SYSADMIN_EMAIL`.

> **GATE C — a human logged into the console in a browser.** `smoke.sh` proves
> the API answers; only a browser proves the console, the session storage and
> the same-origin `/api` routing all work together.

> **`D-17` (resolved 2026-09-18): there is no password-rotation mechanism and
> none will be built.** Changing the SysAdmin password means editing `.env` and
> restarting the API. Whoever can read `.env` is the platform administrator —
> treat host access accordingly.

### 3.4 Create Transmega

In the console, as SysAdmin:

1. Create the tenant **Transmega** with its real tax identifier (`EPIC-TP-01`).
2. Put it on its plan (`EPIC-TP-02`, exactly one active plan).
3. Apply Transmega's **signed-off** palette (`HRMS-409`, §1 item 8).
4. Create Transmega's first tenant owner.

> **GATE D — the Transmega contact logs in, in their own colours, and confirms
> the branding is theirs.** This is `PD-018`'s acceptance target, and it is the
> owner's call, not the cutover lead's.

### 3.5 Invitations

If `SMTP_*` is configured, invite Transmega's users and confirm one email
arrives. If it is not configured, accounts are still created and invite tokens
still issued — the link has to be delivered another way, and someone must be
told that is the plan (`EPIC-IA-07`).

---

## 4. Post-cutover watch

For **24 hours** after Gate D:

```bash
docker compose -f infra/prod/docker-compose.yml logs -f --tail=200 api
```

Watch for the conditions in [`OPERATIONS.md`](OPERATIONS.md). Raise the DNS
TTL back up only after the watch ends.

---

## 5. Done

The cutover is complete when all four gates passed, the 24-hour watch found
nothing, and the owner says so. Record the deployed `IMAGE_TAG` and the date in
the change register.

---

## 6. Rollback

**Trigger — any one of these, no debate required:**

- `deploy.sh` exited non-zero and the cause is not understood within **15 minutes**;
- `smoke.sh` fails after a successful deploy;
- Gate C or Gate D fails;
- tenant data looks wrong in any way;
- the owner says roll back.

**Losing an evening is cheap. Transmega's first day on a broken platform is not.**

### 6.1 Application-only rollback (no schema change in this release)

Redeploy the previous tag. This is why `latest` is never published.

```bash
sed -i "s|^IMAGE_TAG=.*|IMAGE_TAG=<previous-tag>|" .env
./deploy.sh
```

### 6.2 Rollback including a schema change

Migrations here are **forward-only**: there is no `down` step, deliberately.
An untested down-migration run against live data is not a safer position than a
restore from ten minutes ago. The rollback position is the dump `deploy.sh`
took at step 3.

```bash
# 1. Stop serving. Do this first: nothing should write during a restore.
docker compose -f docker-compose.yml stop api console

# 2. Find the dump taken by this deploy.
docker compose -f docker-compose.yml exec mariadb ls -lt /backups

# 3. Restore it.
docker compose -f docker-compose.yml exec -T mariadb sh -c \
  'mariadb --user=root --password="$MARIADB_ROOT_PASSWORD" "$MARIADB_DATABASE" \
   < /backups/<the-dump>.sql'

# 4. Bring the PREVIOUS image back.
sed -i "s|^IMAGE_TAG=.*|IMAGE_TAG=<previous-tag>|" .env
./deploy.sh

# 5. Verify.
./smoke.sh
```

> **Why step 4 works.** The API refuses to start against a schema that is
> *behind* the binary. After a restore the schema matches the *previous* tag, so
> the previous image starts and the new one would not — the guard that makes a
> bad deploy fail loudly is the same one that makes this rollback safe.

### 6.3 If the restore also fails

Stop. Leave the system down and call the owner and the tech lead. A partially
restored database serving live traffic is worse than an outage, and the dump in
`/backups` is still there.

---

## 7. What this runbook does not cover

Stated so nobody discovers it at 02:00:

- **Backups after cutover.** `deploy.sh` dumps *before each deploy*. That is a
  rollback position, **not** a backup schedule. A scheduled off-host backup is
  an `OPERATIONS.md` item and is **not built** — the first tenant's data is
  protected only against a bad deploy, not against losing the VPS.
- **Zero-downtime deploys.** Single host, single API container: a deploy is a
  short outage. Accepted for one tenant; revisit before the second.
- **Horizontal scale.** `HRMS-022`'s advisory lock means multiple instances
  *can* be migrated safely, but nothing here runs more than one.
- **Disaster recovery of the host itself.** Rebuilding from this repository
  restores the *environment* (`HRMS-039`); it does not restore *data* without
  an off-host backup.
