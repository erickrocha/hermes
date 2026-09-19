# Hermes

A multi-tenant SaaS control plane: a Rust HTTP API plus a React admin console (the
"backoffice") that manage *tenants*, *business plans* (priced subscription products), and
*users* with three roles (platform administrator, tenant owner, tenant user). Hermes is a
platform/billing-administration layer, not a vertical application on its own — it exists to
replace [operacao-trm](https://github.com/amzontasp/operacao-trm) (a legacy single-tenant
fleet-operations system) with a proper multi-tenant architecture. Transmega, operacao-trm's
current operator, is Hermes' first tenant.

## Repository layout

| Path | What it is |
|---|---|
| `backend/` | Rust Cargo workspace — see below |
| `backoffice/` | Vite + React + TypeScript admin console — see [`backoffice/README.md`](backoffice/README.md) |
| `infra/dev/docker-compose.yml` | Local MariaDB for development |

### The backend workspace

Four crates with a strictly one-way dependency graph, `web → business → entity`, `web → migration`:

| Crate | Responsibility |
|---|---|
| `entity` | SeaORM table definitions only. No business logic. |
| `migration` | SeaORM migrations and the seed data under `migration/data/`. |
| `business` | Domain structs, use cases, gateways. No HTTP types — testable with no server and, via the `mock` cargo feature, no database. |
| `web` | The Axum HTTP API: routes, endpoints, JSON contracts, authentication, OpenAPI. |

Every table, column, entity and enum value is named in English, without exception — a
deliberate reaction against operacao-trm's Portuguese schema, and a rule that applies to
every future table, not only ones that will eventually replace part of operacao-trm's domain.

## Running it locally

1. Start the database: `docker compose -f infra/dev/docker-compose.yml up -d`.
2. Copy `backend/.env.example` to `backend/.env` and fill in real values. The two token
   secrets **must** be replaced: generate each with `openssl rand -hex 32` (the placeholders
   are deliberately too short and the server refuses them).
3. Apply the migrations — a separate, deliberate step (PD-033):
   `cargo run --manifest-path backend/Cargo.toml -- migrate`.
   The server **does not** migrate at start-up; it refuses to start while any migration is
   pending and names the ones missing. Run this after every pull that adds a migration.
4. Run the API: `cargo run --manifest-path backend/Cargo.toml`. In development Swagger UI is
   served at `/swagger-ui` and the OpenAPI document at `/api-docs/openapi.json`.
5. Run the console: see [`backoffice/README.md`](backoffice/README.md).

## Deploying

Migrations are a deploy step, never a side effect of starting the service:

1. Run `hermes_server migrate` once against the target database. It takes a database lock,
   so two deploy jobs running it at the same time are safe, and it is a no-op when nothing
   is pending.
2. Start (or roll) the service with `hermes_server` and no arguments.

Never edit a migration that has been applied anywhere; fix forward with a new one (HRMS-026).

## Configuration

Every setting is an environment variable (a local `backend/.env` is read too; a malformed
`.env` aborts start-up rather than being partially loaded). See `backend/.env.example` for
the full list and local-development values matching `infra/dev/docker-compose.yml`.

| Variable | Required | Notes |
|---|---|---|
| `DATABASE_URL`, `HOST`, `PORT` | yes | |
| `ACCESS_TOKEN_SECRET`, `REFRESH_TOKEN_SECRET` | yes | ≥ 32 bytes each and different from each other, or start-up is refused |
| `SYSADMIN_EMAIL`, `SYSADMIN_PASSWORD` | yes | The platform administrator, provisioned on first start |
| `APP_ENV` | no | Unset or `development` = developer machine. Any other value (`production`, `staging`, …) turns Swagger UI **and** the OpenAPI document off, and makes `CORS_ALLOWED_ORIGINS` mandatory |
| `CORS_ALLOWED_ORIGINS` | outside development | Comma-separated origins. In development, if unset, any origin is allowed; elsewhere start-up is refused without it |
| `ACCESS_TOKEN_HOURS`, `REFRESH_TOKEN_DAYS` | no | Token lifetimes; default 3 hours / 7 days. Invalid values fall back to the default |
| `SMTP_*`, `BACKOFFICE_BASE_URL` | no | Invitation email — see below |

The platform administrator account (`SYSADMIN_EMAIL`/`SYSADMIN_PASSWORD`) is provisioned on first
start, and re-pointed if the configured email changes. Passwords are stored as Argon2id; any legacy
bcrypt hash is upgraded on that user's next successful login.

**Account-invite email is the one non-fail-fast exception.** `SMTP_HOST`/`SMTP_PORT`/`SMTP_USER`/
`SMTP_PASSWORD`/`SMTP_FROM`/`BACKOFFICE_BASE_URL` are read lazily by `POST /user`, not at boot.
Without them, creating a user still works — the account and its 7-day invite token are created either
way — the server just can't email the link, and says so loudly in the logs rather than silently
dropping it. See `business/src/commons/email_sender.rs` for the reasoning.

## Verification

From `backend/`:

- `cargo test --workspace` — unit tests across all four crates.
- `cargo test -p business --features mock --test mock` — the database-free integration tests
  proving tenant isolation actually reaches the gateway layer, not only the query builder.
- `cargo clippy --workspace --all-targets -- -D warnings` — CI fails on any warning.
- With the local database migrated:
  `DATABASE_URL=mysql://hermes:brutal@localhost:3307/hermes cargo test -p business --test schema_matches_entities -- --ignored`
  — proves every entity column exists in the migrated schema.

Acceptance tests (Gate 3) live in [`acceptance/`](acceptance/README.md).

From `backoffice/`, see [`backoffice/README.md`](backoffice/README.md).

## Further reading

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — why Rust, why this crate split, why a
  separately deployed console.
