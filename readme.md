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
2. Copy `backend/.env.example` to `backend/.env` and fill in real values — every variable
   listed is required; the service refuses to start if one is missing (see "Configuration"
   below).
3. Run the API: `cargo run --manifest-path backend/Cargo.toml`. Migrations run automatically
   at start-up. Swagger UI is served at `/swagger-ui` once it's up.
4. Run the console: see [`backoffice/README.md`](backoffice/README.md).

## Configuration

Every setting is an environment variable, and every one of them is required — a missing
value aborts start-up rather than silently defaulting (`DATABASE_URL`, `HOST`, `PORT`,
`ACCESS_TOKEN_SECRET`, `REFRESH_TOKEN_SECRET`, `SYSADMIN_EMAIL`, `SYSADMIN_PASSWORD`). See
`backend/.env.example` for the full list and local-development values matching
`infra/dev/docker-compose.yml`.

The platform administrator account (`SYSADMIN_EMAIL`/`SYSADMIN_PASSWORD`) is provisioned, or
re-pointed if the configured email changes, on every boot.

## Verification

From `backend/`:

- `cargo test --workspace` — unit tests across all four crates.
- `cargo test -p business --features mock --test mock` — the database-free integration tests
  proving tenant isolation actually reaches the gateway layer, not only the query builder.
- `cargo clippy --workspace --all-targets`

From `backoffice/`, see [`backoffice/README.md`](backoffice/README.md).

## Further reading

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — why Rust, why this crate split, why a
  separately deployed console.
