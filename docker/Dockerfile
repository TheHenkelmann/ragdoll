# syntax=docker/dockerfile:1.7

FROM rust:1.97-trixie AS rust-builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock rustfmt.toml clippy.toml ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release

FROM node:24-bookworm AS frontend-builder
WORKDIR /build/frontend
COPY frontend/package.json frontend/package-lock.json* ./
RUN npm install
COPY frontend ./
RUN npm run build

FROM debian:trixie-slim AS python-builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 python3-venv python3-dev build-essential cmake pkg-config curl ca-certificates \
    && curl -LsSf https://astral.sh/uv/install.sh | sh \
    && rm -rf /var/lib/apt/lists/*
ENV PATH="/root/.local/bin:${PATH}"
WORKDIR /build/python
COPY python/pyproject.toml python/README.md ./
COPY python/src ./src
RUN uv venv /opt/venv --python python3 \
    && uv pip install --python /opt/venv/bin/python .

FROM debian:trixie-slim AS runtime
ARG S6_OVERLAY_VERSION=3.2.0.0
ARG TARGETARCH
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl python3 tesseract-ocr tesseract-ocr-deu tesseract-ocr-eng poppler-utils xz-utils \
    && rm -rf /var/lib/apt/lists/* \
    && case "${TARGETARCH}" in \
         amd64) S6_ARCH=x86_64 ;; \
         arm64) S6_ARCH=aarch64 ;; \
         *) echo "unsupported TARGETARCH: ${TARGETARCH}" >&2; exit 1 ;; \
       esac \
    && curl -fsSL "https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-noarch.tar.xz" \
         -o /tmp/s6-overlay-noarch.tar.xz \
    && curl -fsSL "https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-${S6_ARCH}.tar.xz" \
         -o /tmp/s6-overlay-${S6_ARCH}.tar.xz \
    && tar -C / -Jxpf /tmp/s6-overlay-noarch.tar.xz \
    && tar -C / -Jxpf /tmp/s6-overlay-${S6_ARCH}.tar.xz \
    && rm -f /tmp/s6-overlay-*.tar.xz
ENV S6_KEEP_ENV=1

WORKDIR /app
COPY --from=rust-builder /build/target/release/ragdoll /usr/local/bin/ragdoll
COPY --from=python-builder /opt/venv /opt/venv
RUN ln -sf /usr/bin/python3 /opt/venv/bin/python \
    && ln -sf python /opt/venv/bin/python3
COPY --from=frontend-builder /build/frontend/dist /app/frontend/dist
COPY migrations /app/migrations
COPY docker/cont-init.d /etc/cont-init.d
COPY docker/services.d/ragdoll-gateway /etc/services.d/ragdoll-gateway
COPY docker/services.d/ragdoll-worker /etc/services.d/ragdoll-worker
COPY .env.example /app/.env.example

RUN chmod +x /etc/cont-init.d/* /etc/services.d/ragdoll-gateway/run /etc/services.d/ragdoll-worker/run

ENV PATH="/opt/venv/bin:${PATH}"
ENV RAGDOLL_DATA_DIR=/data
ENV RAGDOLL_MIGRATIONS_DIR=/app/migrations
ENV RAGDOLL_STATIC_DIR=/app/frontend/dist
ENV RAGDOLL_PORT=8080
ENV RAGDOLL_SECRET=change-me-in-production

LABEL org.opencontainers.image.licenses=AGPL-3.0-only
LABEL org.opencontainers.image.title=Ragdoll
LABEL org.opencontainers.image.description="One-stop local RAG pipeline"

VOLUME ["/data"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=120s --retries=5 \
  CMD curl -fsS http://127.0.0.1:8080/health | grep -q '"ready":true'

ENTRYPOINT ["/init"]
