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
- Test administrator: `admin@hermes.test` / `Local-test-only-Hermes-123!`

Override the host ports with `TEST_CONSOLE_PORT`, `TEST_API_PORT`, and
`TEST_DB_PORT` if any default is already in use. These credentials are for this
local test stack only; do not use them in another environment.

Stop the services and keep the database for the next run:

```sh
docker compose -f infra/test/docker-compose.yml down
```

Remove the services and reset all database state:

```sh
docker compose -f infra/test/docker-compose.yml down --volumes
```