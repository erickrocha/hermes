# EPIC-XF-10-S01 (HRMS-038) — the console's production image.
#
# **The one real decision in this file.** Vite inlines `VITE_*` at *build*
# time, so `VITE_API_URL=https://hermes.transmega.example` would bake a
# customer's hostname into the image and make the artefact environment-specific
# — a different image per environment, and a rebuild required to move one.
# `HRMS-038` asks for a reproducible image, and `HRMS-041` asks for a pipeline
# that promotes *the same* artefact.
#
# So the console is built with a **relative** base URL, `/api`, and served on
# the same origin as the API, with Traefik routing `/api/*` to the backend
# (see `infra/prod/docker-compose.yml`). Consequences, all of them wanted:
#
#   * one image runs in any environment, unchanged;
#   * the browser makes same-origin requests, so no CORS preflight is involved
#     in normal operation at all;
#   * there is no build argument anyone can forget to set correctly.
#
# `HRMS-039`'s environment definition still sets `CORS_ALLOWED_ORIGINS` on the
# API, because `HRMS-025` makes the API refuse to start in production without
# it — it is the API's own guard, not something this arrangement removes.

# ---------- builder ----------
# Node 20 to match `frontend-ci.yml`, so the bundle that ships is produced by
# the same major that CI type-checks and tests. Pinned to a patch rather than
# the floating `20` CI uses, because an image should be reproducible.
#
# The floor is real, not cosmetic: Vite 7 requires >= 20.19, and an earlier
# 22.11.0 here built but warned it was below the supported range. Do not lower
# this without checking Vite's engines field.
FROM node:20.19.0-bookworm-slim AS builder

WORKDIR /build

# `npm ci` installs exactly `package-lock.json`, which is what makes this
# reproducible; `npm install` would be free to resolve newer versions.
COPY package.json package-lock.json ./
RUN npm ci

COPY . .

# Same-origin: see the note at the top of this file.
ENV VITE_API_URL=/api
RUN npm run build

# ---------- runtime ----------
# `nginx-unprivileged` rather than `nginx`: the stock image starts as root to
# bind :80 and only then drops to `nginx`. This one runs as uid 101 end to end
# and listens on :8080, which is what HRMS-038's "non-privileged user" means
# for a static file server.
FROM nginxinc/nginx-unprivileged:1.27-alpine AS runtime

COPY --from=builder /build/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

USER 101

EXPOSE 8080

HEALTHCHECK --interval=15s --timeout=5s --start-period=5s --retries=3 \
    CMD wget --quiet --tries=1 --spider http://127.0.0.1:8080/ || exit 1
