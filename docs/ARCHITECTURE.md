# Architecture rationale

EPIC-XF-08-S02 (HRMS-034). No ADR or design note for these three decisions exists anywhere
in this repository's history — not in a commit message, not in a code comment. What follows
is read off the structure that was built, not a record of what the original author actually
reasoned through at the time. Where that matters, it's marked explicitly, so a later reader
can tell "why this holds up" from "why this was chosen."

## Why Rust

No comment or commit records this. What the choice buys, observably: memory safety and
compile-time correctness for a service holding credentials and billing data, a single
self-contained binary to deploy, and a release profile tuned for it (`opt-level = 3`, thin
LTO, in `backend/Cargo.toml`).

## Why a four-crate workspace (`entity` / `business` / `migration` / `web`)

The dependency graph is strictly one-way: `web → business → entity`, `web → migration`. The
root `hermes_server` binary is a three-line shim that only calls `web::main()`. Observably,
this buys:

- `business` has no `axum` dependency and no HTTP types at all — it compiles and is testable
  as a plain library, independent of whether a server exists.
- The `mock` cargo feature (`business/mock` → `sea-orm/mock`) exists specifically so domain
  logic can be tested with no database at all — see `business/tests/mock.rs`.
- `entity` carries table definitions and nothing else, so a migration or a query never has
  business logic hiding inside a `Model`.

"This was done for testability and to keep HTTP concerns out of the domain" is the reading
the structure supports; it is not a recorded decision, and the two owner-ratified rules that
now sit on top of it — English-only persistence identifiers and this exact four-crate
layering with no bypass (AD-019, AD-020) — arrived after the fact, as standing constraints
rather than as the original reasoning.

## Why a separately deployed console (`backoffice/`)

`backoffice/` is its own npm package, its own toolchain, reaching the API only over HTTP at a
configurable `VITE_API_URL`. It holds no privileged access of its own — every authorization
decision it displays is enforced again by the API, not delegated to it (see
`backend/business/src/domain/authorization.rs`).

Observably, this means the API is the sole integration seam: the console could be replaced,
or joined by a second client, without the backend changing. Whether that separation was
originally for deployment independence, a team split, or because a second client was already
anticipated is not recorded, and there is no second client in this repository today.

## What this document is not

It does not restate the platform, interface and structural constraints already tracked as
`AD-###` entries in this project's SDD project-truth workspace (outside this repository) —
that's the authoritative, versioned record, and duplicating it here would just create a
second copy to drift. This file exists so a new engineer working from a checkout of *this*
repository alone — without access to that workspace — isn't left reconstructing the "why"
from the migrations.
