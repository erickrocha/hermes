# syntax=docker/dockerfile:1

# EPIC-XF-10-S01 (HRMS-038) — the API's production image.
#
# Three properties HRMS-038 asks for, and where each one is enforced:
#
#   * **Reproducible** — the builder is pinned to the exact toolchain the
#     workspace is developed against (`rustc 1.98.1`, edition 2024), not
#     `rust:latest`. `Cargo.lock` is committed and `--locked` refuses to
#     silently resolve a different dependency graph than the one that was
#     tested. A build of a given commit produces the same binary next month.
#
#   * **No build toolchain in the final layer** — the runtime stage starts from
#     `debian:bookworm-slim` and receives exactly one artefact: the binary.
#     `cargo`, `rustc`, the ~2 GB of `target/` and the crate sources are all
#     left behind in the builder.
#
#   * **Unprivileged** — a dedicated `hermes` user, created with no login shell
#     and no home, owns nothing it does not need. The process never runs as
#     root, so a remote-code-execution bug in a dependency does not start life
#     with write access to the image.

# ---------- builder ----------
FROM rust:1.98.1-bookworm AS builder

# `business` links native-tls (reqwest for PinME per D-15, lettre for the
# invitation mail), so OpenSSL headers are a build-time requirement.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY . .

# Cache mounts rather than the usual "copy the manifests, build placeholder
# sources, then copy the real ones" trick. That trick needs a placeholder file
# for *every* target a manifest declares, and `business/Cargo.toml` declares
# three `[[test]]` targets — so it failed the moment it was first run, and it
# would fail again, silently and confusingly, the next time anyone adds a test
# target. Cache mounts get the same incremental rebuilds without encoding a
# copy of the manifest's target list in this file.
#
# The binary must be copied out of the cache mount inside the same RUN: a cache
# mount is not part of the resulting layer, so anything left in `target/` is
# gone by the time the runtime stage tries to COPY it.
RUN --mount=type=cache,target=/build/target,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    cargo build --release --locked --bin hermes \
    && strip target/release/hermes \
    && cp target/release/hermes /build/hermes

# ---------- runtime ----------
FROM debian:bookworm-slim AS runtime

# `ca-certificates` is not optional: without it every outbound TLS call fails
# at certificate verification — PinME ingestion (EPIC-FT-01) and SMTP invites
# (EPIC-IA-07) both go out over native-tls. `libssl3` is the runtime half of
# the `libssl-dev` used to build.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --no-create-home --shell /usr/sbin/nologin --uid 10001 hermes

WORKDIR /app
COPY --from=builder /build/hermes /usr/local/bin/hermes

USER hermes

# Configuration arrives from the environment (HRMS-021, HRMS-040). No `.env`
# file is copied in, and `load_dotenv` tolerates its absence by design, so the
# image carries no environment-specific value and no secret.
ENV HOST=0.0.0.0 \
    PORT=8080 \
    APP_ENV=production \
    RUST_LOG=info

EXPOSE 8080

# HRMS-042's endpoint. `--fail` makes a 503 — the "listening but cannot reach
# MariaDB" state — an unhealthy container rather than a healthy one.
HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=3 \
    CMD curl --fail --silent http://127.0.0.1:8080/health || exit 1

# No argument starts the server; `migrate` runs the deploy step (PD-033,
# HRMS-045). The same image does both, so the schema is never applied by a
# binary built from a different commit than the one about to serve it.
ENTRYPOINT ["/usr/local/bin/hermes"]
