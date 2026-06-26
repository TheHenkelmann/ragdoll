# Concepts: releases, stages, and auth

This is the mental model that makes every API path in Ragdoll obvious. Read it
once and the rest of the docs fall into place.

## Content vs. deployment

Ragdoll deliberately separates **what** you serve from **where** you serve it:

- **Release** — a tagged content bundle: its `sources`, `chunks`, and
  `settings`. Think of it as an immutable-ish content snapshot you can version.
  API path prefix: `/api/v1/releases/{tag}/...`
- **Stage** — a short tag (≤12 chars) that *points at* a release, e.g.
  `prod → first-release`. Stages are the production-facing entry point.
  API path prefix: `/api/v1/stages/{tag}/...`

Bootstrap seeds release `first-release`, stage `prod`, and default settings, so
a fresh install is immediately usable.

A typical workflow: build and test content in a release, point a stage at it for
production traffic, then later fork a new release, validate it, and re-point the
stage — without disturbing live clients.

## Planes and write rules

Both prefixes share the same handlers (`ReleaseCtx` resolves the stage to its
underlying release), but they have different write permissions:

| Plane | Path prefix | Reads | Writes |
|---|---|---|---|
| Release | `/api/v1/releases/{tag}/...` | Session or API key | Session token + superadmin |
| Stage | `/api/v1/stages/{tag}/...` | Session or API key | **API key only** |

> **The stage plane rejects session-token writes.** Production writes (and
> queries you want attributed to a stage) must use an **API key** JWT, not your
> UI login token. This is the single most common "why does my request 403"
> surprise. See [pitfalls.md](pitfalls.md).

Event tables (`queries`, `ingest_jobs`) record both `release_id` and the
optional `stage_id`, so analytics can be viewed through either lens.

## Release lifecycle

```bash
# List releases
curl -sS http://localhost:8080/api/v1/releases -H "Authorization: Bearer $TOKEN"

# Create an empty release
curl -sS -X POST http://localhost:8080/api/v1/releases \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"tag":"v2","message":"Q2 content","init":{"type":"new"}}'

# Fork an existing release (copies sources, chunks, settings — not queries)
curl -sS -X POST http://localhost:8080/api/v1/releases \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"tag":"v2-fork","message":"","init":{"type":"fork","source":"first-release"}}'
```

`init.type` is one of `new`, `fork`, or `template`.

Stages are managed via `GET/POST /api/v1/stages` and
`PATCH/DELETE /api/v1/stages/{tag}`.

## Authentication

Ragdoll uses **Bearer JWT** exclusively (no cookies). There are two token types,
distinguished by the `typ` claim:

| Type | Claim `typ` | Use case |
|---|---|---|
| Session | `session` | UI login, superadmin management, release-plane writes |
| API key | `apikey` | Production integrations, **stage-plane writes** |

```bash
# Log in for a session token
curl -sS -X POST http://localhost:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@ragdoll.ai","password":"admin"}'

export TOKEN="<access_token>"

# Public bootstrap info (no auth) — e.g. the default admin email
curl -sS http://localhost:8080/api/v1/auth/info

# Authenticated request
curl -sS http://localhost:8080/api/v1/releases/first-release/sources \
  -H "Authorization: Bearer $TOKEN"
```

Only `/api/v1/health`, `/api/v1/auth/login`, and `/api/v1/auth/info` are public;
everything else requires a Bearer token.

### Users and API keys (superadmin)

Superadmins manage users and API keys:

| Resource | Endpoints |
|---|---|
| Users | `POST/GET/DELETE /api/v1/users` |
| API keys | `POST/GET/DELETE /api/v1/api_keys` |

> **API key JWTs are shown exactly once**, at creation time. Copy and store the
> token immediately — it cannot be retrieved later.

## Related

- [architecture.md](architecture.md) — how planes map to components and data flow
- [querying.md](querying.md) — how queries are attributed to a release/stage
- [operations.md](operations.md) — managing releases and stages from the UI
