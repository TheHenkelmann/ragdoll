// SPDX-License-Identifier: AGPL-3.0-only

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Ragdoll API",
        version = "2.0.0",
        description = "Local RAG pipeline with releases/stages, JWT auth, and per-release settings.\n\n\
            All HTTP API routes live under `/api/v1`. The web UI is served from `/` and is not part of this spec.\n\n\
            ## Authentication\n\
            All endpoints except `/api/v1/health`, `/api/v1/auth/login`, `/api/v1/auth/info`, and this \
            documentation require `Authorization: Bearer <token>`.\n\
            Obtain a session token via `POST /api/v1/auth/login` or create an API key via \
            `POST /api/v1/api_keys` (superadmin).\n\n\
            ## Planes\n\
            - **Release plane** (`/api/v1/releases/{tag}/...`): session tokens may write (superadmin for content/settings).\n\
            - **Stage plane** (`/api/v1/stages/{tag}/...`): writes require an API key token.\n\n\
            ## Core paths\n\
            - `GET /api/v1/health`\n\
            - `POST /api/v1/auth/login`, `GET /api/v1/auth/status`, `GET /api/v1/auth/info`\n\
            - `GET/POST/DELETE /api/v1/users`, `GET/POST/DELETE /api/v1/api_keys`\n\
            - `GET/POST /api/v1/releases`, `PATCH/DELETE /api/v1/releases/{tag}`\n\
            - `GET/POST /api/v1/stages`, `PATCH/DELETE /api/v1/stages/{tag}`\n\
            - `GET /api/v1/models`, `POST /api/v1/models/{name}/download`\n\
            - `GET /api/v1/analytics?lens=stage|release&tag=...&days=14`\n\
            - `/api/v1/releases/{tag}/sources|queries|chunks|settings|db/{table}`\n\
            - `/api/v1/stages/{tag}/sources|queries|chunks|settings|db/{table}`\n\n\
            ## Query parameters\n\
            - `store_payload` (bool, default false): persist query text and chunk content.\n\
            - `ts_start` (epoch ms): upstream latency anchor.\n\
            - `playground` is reserved for internal UI use and omitted from public docs.\n\n\
            ## Models\n\
            Only dim-1024 models are supported (`BAAI/bge-m3`, `BAAI/bge-reranker-v2-m3`). Changing embedding model requires manual re-ingest."
    ),
    tags(
        (name = "auth", description = "Authentication and principals"),
        (name = "releases", description = "Release-scoped content and settings"),
        (name = "stages", description = "Stage-scoped production plane"),
        (name = "analytics", description = "Aggregated metrics")
    )
)]
pub struct ApiDoc;
