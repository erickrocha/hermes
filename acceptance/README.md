# Acceptance tests (Gate 3)

Executable acceptance suites, one per slice. Python 3 standard library only — no packages to
install. Scenarios and their traceability live in
`02-system_requirements/hermes/<slice>_acceptance_tests.md` in the parent SDD repository;
results are logged in `<slice>_test_execution_log.md` next to it.

| Suite | Slice |
|---|---|
| `foundation_acceptance.py` | Foundation (`EPIC-XF-01…09`, scenarios `XF-0xx`) |
| `reference_data_acceptance.py` (+ `reference_data_ui.mjs`) | Reference data (`EPIC-RD-01…03` + QA-added `PD-027-S01…S06`, scenarios `RD-0xx`) |
| `tenancy_plans_acceptance.py` | Tenancy & Plans (`EPIC-TP-01…05`, scenarios `TP-0xx`) |

## Prerequisites

1. Dev database up: `docker compose -f infra/dev/docker-compose.yml up -d` (container
   `dev-mariadb-1`, host port 3307).
2. Schema migrated and API running from `backend/` (reads `backend/.env`):
   `cargo run -- migrate`, then `cargo run` (API on `http://127.0.0.1:8081`).
3. `backend/target/debug/hermes_server` built (the step above builds it) — lifecycle scenarios
   start it themselves.
4. `docker` and `cargo` (with `cargo-llvm-cov` for `--coverage`) on `PATH`.

## Run

From the repository root:

```sh
python3 acceptance/foundation_acceptance.py              # api + docs + lifecycle + cargo
python3 acceptance/foundation_acceptance.py --coverage   # also measure business coverage (slow)
python3 acceptance/foundation_acceptance.py --skip-lifecycle --skip-cargo   # fast, API only
```

Output: one line per scenario (`PASS`/`FAIL`/`BLOCKED`) with evidence, then one line per story.
Exit code is 1 if any story fails. `--json FILE` writes the results for tooling.

## What it touches

- **Dev database (`hermes`)**: fixture tenants/users/plans prefixed `qa-acc-`; all deleted on
  exit, the SysAdmin row's audit columns restored, auto-increment counters reset.
- **Scratch database `hermes_acc`**: created (as MariaDB root) for the lifecycle scenarios and
  dropped on exit.
- **Scratch API instance on port 8091** (`HERMES_SCRATCH_PORT`), started from a temporary
  directory with an explicit environment — `backend/.env` is read, never modified. The instance
  is stopped by its own PID.

## Fixture notes

- Tenants are inserted with SQL because `POST /tenant` fails on the migrated schema (logged
  defect). Switch `seed_fixtures()` to the API once that is fixed.
- There is no SMTP in dev, so the suite builds the invitation token that `POST /user` would
  have e-mailed (it needs `ACCESS_TOKEN_SECRET` from `backend/.env`) and activates each
  account through the real `POST /accept-invite`.

## Configuration

| Variable | Default |
|---|---|
| `HERMES_API` | `http://127.0.0.1:8081` |
| `HERMES_DB_CONTAINER` | `dev-mariadb-1` |
| `HERMES_DB_PASSWORD` / `HERMES_DB_ROOT_PASSWORD` | `brutal` |
| `HERMES_ADMIN_EMAIL` / `HERMES_ADMIN_PASSWORD` | `admin@hermes.dev` / `LocalDevOnly123!` |
| `HERMES_SCRATCH_PORT` | `8091` |
| `HERMES_BINARY` | `backend/target/debug/hermes_server` |

## Tenancy & Plans (`tenancy_plans_acceptance.py`, scenarios `TP-0xx`)

API-only (no lifecycle or cargo steps). It needs the same dev database and running API as above,
plus `backend/.env`, which it only reads. It uses `ACCESS_TOKEN_SECRET` to stand in for the
invitation e-mail.

```sh
python3 acceptance/tenancy_plans_acceptance.py                  # all TP scenarios
python3 acceptance/tenancy_plans_acceptance.py --json out.json  # also write results
```

- Every row it creates has the prefix `qa-tp-`: tenant business names, tax IDs `QATP…`, user e-mails
  and plan names. BR tenants use CNPJs/CPFs generated at run time, after the suite checks that no
  tenant holds them. On exit it deletes only its own rows and never resets `AUTO_INCREMENT`, so it
  can run at the same time as other slices' suites. It prints the table counts before and after.
- Scenarios `TP-080` (billing, D-03) and the tier/quota/grace-days stories are reported as
  `SUPERSEDED`. Where a replacement behaviour exists, it is still checked.
- Exit code is 1 if any story fails.

## Identity & Access suite (`identity_access_acceptance.py`)

Slice IA (`EPIC-IA-01…08`, scenarios `IA-0xx`). Scenarios:
`02-system_requirements/hermes/identity-access_acceptance_tests.md`. Log:
`identity-access_test_execution_log.md`.

```sh
python3 acceptance/identity_access_acceptance.py                  # api + scratch instance + cargo
python3 acceptance/identity_access_acceptance.py --skip-scratch --skip-cargo   # API only (~2 min)
python3 acceptance/identity_access_acceptance.py --json out.json  # also write results
```

- **Dev API/DB.** Fixtures are created entirely through the API (tenants `qa-ia-tenant-A/B`, country
  `US`, tax IDs `QAIA…`, plus owners and users). Invitations are minted as in the foundation suite.
  Only rows prefixed `qa-ia-` are deleted (at start and on exit). The shared SysAdmin row is never written and
  auto-increment counters are left alone, so the suite can run alongside other slices' suites.
- **Scratch instance.** Runs on port `8094` (`HERMES_IA_SCRATCH_PORT`) against the database `hermes_acc_ia`, which is created
  as root and dropped on exit. It starts with `RUST_LOG=debug`, default token lifetimes and a deliberately short
  `SYSADMIN_PASSWORD`, and is stopped by its PID.
- **API log.** IA-066 reads the dev API's log (`HERMES_API_LOG`, default `/tmp/claude-1000/hermes-api.log`)
  for the invitation mail attempt. If the log isn't readable, the scenario is BLOCKED.
- Exit code is 1 if any story fails.

---

## Reference data (`reference_data_acceptance.py`)

Extra prerequisites:

- The migration CLI is built: `(cd backend && cargo build -p migration)` → `backend/target/debug/migration`.
  It is used for the stepwise up/down seeding scenarios (RD-020…023).
- For `--ui` only: the console runs at `http://localhost:5180`, and Playwright (≥ 1.62 with its Chromium) is
  installed somewhere. It is not a hermes dependency. Point `PLAYWRIGHT_MODULE` at that `node_modules/playwright`.

```sh
python3 acceptance/reference_data_acceptance.py                    # api + lifecycle
python3 acceptance/reference_data_acceptance.py --skip-lifecycle   # api only (fast)
PLAYWRIGHT_MODULE=/path/to/node_modules/playwright \
  python3 acceptance/reference_data_acceptance.py --ui             # + console scenarios RD-031, RD-052…054
```

Output: one line per scenario, then one line per story (the plan's stories plus `PD-027-*`). The exit code is 1
if any story fails. `--json FILE` writes the results. `QA_SHOTS=<dir>` keeps the UI screenshots.

What it touches:

- **Dev database (`hermes`)**, shared with other suites:
  - provinces with country code **`QR`** (not an assigned ISO code);
  - cities named `qa-rd-*` under those provinces;
  - one tenant `qa-rd-tenant` with users `qa-rd-owner@` / `qa-rd-user@hermes.test`.

  All of these are deleted on exit. The suite refuses to start if any are left over. The real BR/US rows are
  only read, and a content checksum of them is compared before and after. Auto-increment counters are
  not reset, because other suites may be running.
- **Scratch database `hermes_acc_rd`**: created as root, migrated stepwise, and dropped on exit.
- **Scratch API on port 8096** (`HERMES_RD_SCRATCH_PORT`): used for RD-008 (tables renamed away to force a
  database failure). It runs from a temporary directory with an explicit environment and is stopped by its PID.

| Variable | Default |
|---|---|
| `HERMES_RD_SCRATCH_PORT` | `8096` |
| `HERMES_MIGRATOR` | `backend/target/debug/migration` |
| `HERMES_CONSOLE` (UI) | `http://localhost:5180` |
| `PLAYWRIGHT_MODULE` (UI) | `playwright` (resolved from the script's location) |

## Backoffice suite (`backoffice_acceptance.py`, scenarios `BO-0xx`)

Covers `EPIC-BO-01…06` plus the PD-027/PD-028 console behaviours.
- **API and static checks** (Python stdlib, reusing this folder's helpers):
  - the server half of each console affordance
  - `npm test` and `npx tsc -b --noEmit` in `backoffice/`
  - inspection of the stylesheet and the archived design doc
- **Browser walkthrough** (`backoffice_walkthrough.cjs`, run with `--ui`): headless Chrome against the running console.
  - Needs `playwright-core` somewhere outside the repo. It is not a repo dependency.
  - Screenshots and `ui_results.json` go to `evidence/backoffice/`.

Prerequisites: the API is up on `:8081` (as above), and the console is running on `:5180` (`npm run dev -- --port 5180` in `backoffice/`).

```sh
python3 acceptance/backoffice_acceptance.py                  # API + static checks only
# with the browser walkthrough (one-time: npm install --prefix /tmp/pw playwright-core@1.63.0)
PLAYWRIGHT_NODE_PATH=/tmp/pw/node_modules python3 acceptance/backoffice_acceptance.py --ui
python3 acceptance/backoffice_acceptance.py --ui --keep      # leave fixtures for a manual look
python3 acceptance/backoffice_acceptance.py --teardown       # remove qa-bo-/QB rows only
```

| Variable | Default |
|---|---|
| `HERMES_CONSOLE` | `http://localhost:5180` |
| `PLAYWRIGHT_NODE_PATH` | unset, so the UI part is skipped |
| `CHROME_PATH` | `/usr/bin/google-chrome` |

**What it touches.** It creates only rows prefixed `qa-bo-` (tenants with US tax IDs `QABO…`, users,
plans) and provinces with country `QB`. All of them are deleted with SQL on exit, because the API has no
DELETE for tenants or users. It never resets auto-increment counters, so it is safe alongside other
suites on a shared database. `evidence/backoffice/fixtures.json` holds the last run's fixture IDs and
test-account passwords.

**Deliberate race checks.** BO-064 and BO-065 delay one API response inside the browser, so that an
out-of-order answer happens every time instead of only on a slow network.
