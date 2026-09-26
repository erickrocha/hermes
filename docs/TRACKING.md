# Vehicle tracking integration

How Hermes obtains vehicle position and ignition state, and — just as important — what it
deliberately does **not** do with it yet.

Decisions behind this document: `PD-025` (consume a provider, do not build a tracker), `D-15` (which
provider, and its contract), `D-05`/`D-06` (tenant scope outside a request).
Requirements: `HRMS-900`…`HRMS-905`.

## The shape: consumed, not rebuilt

Hermes **consumes an existing third-party GPS provider's API**. It does not implement a
vehicle-polling tracker of its own (`HRMS-900`).

This is a deliberate reversal of how the system Hermes replaces works. `operacao-trm` runs its own
`rastreador-cron`, which polls devices and writes production state. `PD-025` rules that out: the
platform was created to retire that job, and reimplementing it here would inherit the maintenance
along with the behaviour.

## The provider and its contract

`HRMS-901` requires the provider's contract to be recorded **before** the ingestion component is
designed, so the component is designed once rather than rewritten when the shape turns out
differently. It is recorded here.

| Property | Value |
|---|---|
| Provider | **PinME** — `https://api.pinme.io/api` |
| Family | **Traccar-compatible** |
| Authentication | **HTTP Basic on every call** — `Authorization: Basic base64(email:password)`. There is no `/session`, no token exchange and no bearer token |
| Direction | **Pull.** Hermes asks; the provider never calls Hermes |
| Update frequency | Device-driven. Positions are read on demand; a fix older than **5 minutes** is not treated as current |

This contract did not have to be obtained speculatively. `operacao-trm` integrates this exact
provider in production today, and its `rastreador-status` edge function is a working reference
implementation. The narrative is in
`01-project_truth/operacao-trm/01-ai-native/rastreador-status-edge-doc.md`.

**Why pull matters beyond the wire format.** A push provider would have made ingestion an inbound
authenticated endpoint, landing it in the `web` crate's public surface and under `EPIC-XF-03`'s
whitelist discipline. Pull makes it scheduled, out-of-request code — which lands it squarely on the
tenant-scope question below. The two shapes are different components, not one component with a
different constant, which is why `HRMS-901` insisted on knowing first.

### Endpoints used

| Endpoint | Method | Purpose |
|---|---|---|
| `/positions` | GET | Last known position for **every** device, in one call. Not per-device — the client indexes by `deviceId` |
| `/reports/events` | GET | Fallback. `?deviceId=…&from=…&to=…&type=ignitionOn&type=ignitionOff`, with repeated `deviceId` and `type` parameters and an ISO-8601 window |

### Payload fields depended upon

`latitude`, `longitude`, `valid`, `serverTime` / `fixTime` / `deviceTime` (first present wins),
`attributes.ignition` (boolean), and `attributes.totalDistance` / `odometer` / `distance`
(accumulated metres).

## Freshness and degradation

These rules are **inherited from a production integration, not invented here**, and two of them were
paid for with real incidents recorded in `operacao-trm`'s own notes. They are written down with the
reasoning attached so that a later reader does not "simplify" a rule whose cost has been forgotten.

- A position older than **5 minutes**, or flagged `valid: false`, or carrying no `ignition`
  attribute, is not trusted as current. Ingestion falls back to the ignition-event report over a
  **24-hour** window.
- A timestamp in the *future* is clock skew, not freshness. A mis-set device may not claim to be
  permanently current.
- No event either ⇒ the status is **unknown**, explicitly flagged untrustworthy rather than guessed.
- A failing events report **degrades only the devices that needed it**. It never fails the batch:
  the vehicles that answered from `/positions` are still good readings. A failing `/positions` call
  *does* fail the batch, because there is no partial answer to fall back to.
- **A stale reading must never leave a vehicle tagged as being at base.** An ignition *event* is
  reliable evidence about ignition — it is a recorded fact — so such a reading is trustworthy. What
  it is not is evidence of current *position*. `VehicleTrackingStatus::supports_presence_claim`
  holds that distinction in one place rather than at every call site that might forget it.
- The last known coordinate is attached **even when stale**: a parked vehicle stops emitting but has
  not moved, and a garage view still needs somewhere to draw it.

## Where the boundary is

`HRMS-902` requires the provider's contract to be confined to a single ingestion boundary, so that
changing provider is a change to one module rather than to the domain.

| Layer | File | Knows about PinME? |
|---|---|---|
| Boundary | `backend/business/src/gateway/tracking_provider.rs` | **Yes — and nothing else does** |
| Vocabulary | `backend/business/src/domain/vehicle_tracking.rs` | No |
| Rules | `backend/business/src/use_cases/vehicle_tracking_use_case.rs` | No |

The split is not cosmetic. The freshness rule, the fallback decision and the presence rule are
*Hermes'* policy; the JSON field names are the provider's. If they lived together, swapping provider
would mean re-deciding the policy, and the acceptance suite asserts the separation
(`FT-003`) precisely because boundaries erode quietly.

Configuration is named for the role rather than the vendor — `TRACKING_API_BASE_URL`,
`TRACKING_API_EMAIL`, `TRACKING_API_PASSWORD` — so a change of provider is not a change of
deployment configuration. Credentials are Base64-encoded once in the constructor and never appear in
an error message or a log line.

## Tenant scope

Ingestion runs **outside any authenticated request**, which is exactly the hazard `HRMS-008` and
`U-016` were written about.

`D-05` made the out-of-request default `Denied`. Before this integration, the only way out of that
default was `run_as_platform`, which grants the entire platform — so anything acting for a single
tenant had to take platform-wide reach it did not need. That is scope acquired by omission, the
accident `U-016` names.

`entity::audit::run_for_tenant(tenant_id, actor, …)` was added for this: an **explicit, named**
tenant scope (`HRMS-903`). Ingestion uses it and does not use `run_as_platform`; `FT-004` and
`FT-005` assert both halves.

## What is deliberately not built

**Nothing is persisted.** There is no tracking table, no entity and no migration.

`HRMS-905` requires the system to state what consumes ingested tracking data *before* that data is
persisted. The candidate consumers — the garage-service queue and the fuel cycle — are
`operacao-trm`'s decision **`D-01`**, which is open, and which is **not Hermes' decision to take**.
Building a telemetry store now would be inventing a schema for a reader nobody has specified.

This is the requirement being satisfied, not a gap in the work.

What the slice *does* settle is the shape a store would hold, because `VehicleTrackingStatus`
carries the four properties `PD-025`'s note argues `D-01` needs: a **value**, its **provenance**
(which source answered), its **quality** (trustworthy or not), and an **as-of instant**. When `D-01`
names a consumer, the input is already explicit and provenanced rather than something to retrofit.

Hermes also holds no vehicle, garage, trip or fuel entity, and this slice introduces none. Tracking
arrives as an *integration*, not as a domain model; the fleet-operations domain arrives from
`01-project_truth/operacao-trm/`, not from here.
