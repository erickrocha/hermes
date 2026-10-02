# Architecture rationale

EPIC-XF-08-S02 (HRMS-034). The three decisions below — Rust, the crate split, and the
separately deployed console — were made by the project owner before this repository had any
ADR or design note, so nothing in the commit history records them.

The owner's reasoning was captured directly on **2026-09-19, closing decision D-13**. Each
section states that reasoning first, then what the built structure observably buys. The two
are kept apart on purpose: the first is why the choice was made, the second is why it still
holds up. A later reader should be able to tell the difference.

## Why Rust

**Owner's reasoning (D-13).** Existing knowledge of the language, and the fact that it is
safe and fast. The choice was deliberate on those grounds, not the result of a survey of
alternatives.

What it buys, observably: memory safety and compile-time correctness for a service holding
credentials and billing data, a single self-contained binary to deploy, and a release profile
tuned for it (`opt-level = 3`, thin LTO, in `backend/Cargo.toml`).

## Why a four-crate workspace (`entity` / `business` / `migration` / `application`)

**Owner's reasoning (D-13).** Organisation, reuse, and good practice. The split is a
structural default the owner applies deliberately, not a reaction to a specific problem this
codebase hit.

What it buys, observably — the dependency graph is strictly one-way (`application → business →
entity`, `application → migration`) and the `application` crate builds the single `hermes` binary:

- `business` has no `axum` dependency and no HTTP types at all — it compiles and is testable
  as a plain library, independent of whether a server exists.
- The `mock` cargo feature (`business/mock` → `sea-orm/mock`) exists specifically so domain
  logic can be tested with no database at all — see `business/tests/mock.rs`.
- `entity` carries table definitions and nothing else, so a migration or a query never has
  business logic hiding inside a `Model`.

Two owner-ratified rules now sit on top of this layout as standing constraints: English-only
persistence identifiers, and this exact four-crate layering with no bypass (AD-019, AD-020).
Those arrived after the fact — they codify the split rather than explain its origin.

## Why a separately deployed console (`backoffice/`)

**Owner's reasoning (D-13).** The backend is expected to serve **many clients** — a mobile
app, the web console, and third-party integrations. A console welded to the server would make
the first additional client a rewrite, so the API was kept as the product and the console as
one consumer of it.

This settles a question the structure alone could not answer: the separation was for multiple
anticipated clients, not for deployment independence or a team split.

What it buys, observably: `backoffice/` is its own npm package with its own toolchain,
reaching the API only over HTTP at a configurable `VITE_API_URL`. It holds no privileged
access of its own — every authorization decision it displays is enforced again by the API,
never delegated to it (see `backend/business/src/domain/authorization.rs`). The API is
therefore the sole integration seam: the console can be replaced, or joined by a second
client, without the backend changing.

There is no second client in this repository today; the design anticipates one.

## What this document is not

It does not restate the platform, interface and structural constraints already tracked as
`AD-###` entries in this project's SDD project-truth workspace (outside this repository) —
that's the authoritative, versioned record, and duplicating it here would just create a
second copy to drift. This file exists so a new engineer working from a checkout of *this*
repository alone — without access to that workspace — isn't left reconstructing the "why"
from the migrations.
