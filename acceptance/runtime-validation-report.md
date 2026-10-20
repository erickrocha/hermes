# Runtime Validation Report — Billing

**Generated:** 2026-10-16  
**Target:** Hermes backend and backoffice, C-015 / EPIC-TP-05

## Environment

| Capability | Result |
|---|---|
| Docker Engine | Available, 29.3.0 |
| Database | MariaDB 11.4, QA database `hermes` with migrations applied |
| Node.js | Available, 20.19.5 |
| Browser E2E | Playwright 1.64.1, Chromium 1248 |
| API readiness | PASS — `/api-docs/openapi.json` returned HTTP 200 on port 8081 |
| Backoffice readiness | PASS — Vite served on port 5173 |

## Results

| Tier | Result | Exit code | Evidence |
|---|---|---:|---|
| Migration/schema | PASS | 0 | All migrations applied to an empty MariaDB 11.4 database; `schema_matches_entities` passed 1/1 |
| API acceptance | PASS | 0 | TP-082…TP-092, 11/11, live API/MariaDB and the one-second billing job |
| Browser E2E | PASS | 0 | `confirmation.spec.ts`, `owner-history.spec.ts`, `suspension.spec.ts`: 3/3 |
| Backoffice regression | PASS | 0 | Vitest: 176/176 tests via `npm test` |
| Production build | PASS | 0 | `tsc -b && vite build`; Vite reports the existing 500 kB chunk-size warning |
| Cleanup | PASS | 0 | Playwright teardown removed QA fixtures; post-run queries found zero matching tenants, users or plans |

The API scenarios covered plan assignment and pending changes, cycle closure, BRL snapshots and overage calculation, overdue status both inside and outside grace, tenant isolation, job idempotence, manual confirmation, authorization, suspension/restoration, and the absence of invoice/fiscal fields. The timestamp idempotence assertion compares at the database column's `DATETIME` precision (whole seconds).

The repeatable browser command is `npm run test:e2e`. Its global setup runs the API acceptance runner with isolated fixtures; global teardown removes them by the generated `qa-bill-*` prefix.

To repeat it, start the migrated QA database, Hermes API on port 8081 and Vite on port 5173; provide `ACCESS_TOKEN_SECRET`, `HERMES_ADMIN_EMAIL`, `HERMES_ADMIN_PASSWORD` and `HERMES_DB_PASSWORD`. Optional overrides are `HERMES_API`, `HERMES_DB_CONTAINER` and `BILLING_BACKOFFICE_URL`.

## Verdict

**PASS — C-015 functional evidence.** The Gate 3 scenarios passed. The epic remains **not accepted** pending formal owner/QA sign-off; implementation effort also remains unmeasured.

## Notes

- Invitation email delivery was not exercised; test accounts were activated using the existing acceptance helper.
- `npm install` reported 4 dependency audit findings (1 moderate, 1 high, 2 critical); dependency remediation was outside this billing validation.
