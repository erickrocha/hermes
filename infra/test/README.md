# Test Stack

This Compose stack starts a disposable MariaDB, applies the backend migrations,
starts the API, and serves the backoffice on the same origin with `/api` proxied
to the API container.

From `04-codebase/hermes`:

```sh
docker compose -f infra/test/docker-compose.yml up --build --wait
```

- Backoffice: http://localhost:5174
- API and Swagger UI: http://localhost:8082
- MariaDB: `127.0.0.1:3308`
- Tracking mock control API: http://localhost:8083
- Test administrator: `admin@hermes.test` / `Local-test-only-Hermes-123!`

Override the host ports with `TEST_CONSOLE_PORT`, `TEST_API_PORT`, and
`TEST_DB_PORT` if any default is already in use. These credentials are for this
local test stack only; do not use them in another environment.

The garage day-boundary acceptance scenario requires the rollover worker. Start
the stack with `TEST_GARAGE_ROLLOVER_INTERVAL_SECONDS=3` for that run; the default
is `0` so unrelated tests do not run a background rollover.

The automatic garage departure acceptance scenario requires its worker. Start
the stack with `TEST_GARAGE_AUTO_DEPARTURE_INTERVAL_SECONDS=1`; it defaults to
`0` so unrelated tests do not run automatic departures.

The garage geofence acceptance scenario uses the local tracking mock and a
1-second poll: start with `TEST_GARAGE_PRESENCE_POLL_INTERVAL_SECONDS=1`. The
mock accepts position updates at `/control/positions`; it never contacts PinME.

Stop the services and keep the database for the next run:

```sh
docker compose -f infra/test/docker-compose.yml down
```

Remove the services and reset all database state:

```sh
docker compose -f infra/test/docker-compose.yml down --volumes
```
## Running the acceptance suites against this stack

The suites in `acceptance/` default to the development stack. Pointing them at this one takes these variables
(the values are this stack's own, local-only):

```sh
export HERMES_API=http://127.0.0.1:8082 HERMES_DB_CONTAINER=hermes-test-mariadb-1 \
       HERMES_DB_USER=hermes_test HERMES_DB_PASSWORD=local-test-db HERMES_DB_NAME=hermes_test \
       HERMES_ADMIN_EMAIL=admin@hermes.test HERMES_ADMIN_PASSWORD='Local-test-only-Hermes-123!' \
       HERMES_ACCESS_TOKEN_SECRET=local-test-access-secret-0123456789abcdef0123456789abcdef
export ACCESS_TOKEN_SECRET=$HERMES_ACCESS_TOKEN_SECRET     # billing_acceptance.py reads this name
python3 acceptance/tenant_hierarchy_acceptance.py
```

Users are created by profile (`acceptance/profile_compat.py`), not by role. A few scenarios need more than the
default stack: the garage workers and the tracking mock (above), the fuel provider stand-in on the host
(`fuel_acceptance.py`), the billing job's interval (`billing_acceptance.py`) and a locally built `hermes` binary
with a scratch database (the "scratch"/"lifecycle" scenarios of `foundation`, `identity_access`,
`fleet_operations`); they report BLOCKED, not FAIL, without them.
