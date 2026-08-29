# syntax=docker/dockerfile:1.7

# Pin and verify the external media toolchain for the two published Linux
# architectures. The official yt-dlp standalone executable includes yt-dlp-ejs;
# Deno supplies its restricted JavaScript challenge runtime.
FROM debian:trixie-slim AS media-tools

ARG TARGETARCH
ARG YT_DLP_VERSION=2026.08.19
ARG DENO_VERSION=2.9.6

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl unzip \
    && rm -rf /var/lib/apt/lists/* \
    && case "${TARGETARCH}" in \
        amd64) \
          YT_DLP_ASSET=yt-dlp_linux; \
          YT_DLP_SHA256=58162f9bfdc27458ea47bfcb311cf47028f17d8154a8bf7d689861d46399230a; \
          DENO_ASSET=deno-x86_64-unknown-linux-gnu.zip; \
          DENO_SHA256=394f07f4da2bebe6ce6f1e7ce0fa16429b29b08c35e3fac3fe25972676dff4b2 ;; \
        arm64) \
          YT_DLP_ASSET=yt-dlp_linux_aarch64; \
          YT_DLP_SHA256=b16e4dab368a816cd05d477d698a605a6ae87ccee1c8ffd38fa21d7254141fcc; \
          DENO_ASSET=deno-aarch64-unknown-linux-gnu.zip; \
          DENO_SHA256=9a46afc6c392c7cd2ff71a31558935545b46408d0e87f7a86908c712721c046e ;; \
        *) echo "unsupported TARGETARCH: ${TARGETARCH}" >&2; exit 1 ;; \
      esac \
    && curl --fail --location --silent --show-error \
      "https://github.com/yt-dlp/yt-dlp/releases/download/${YT_DLP_VERSION}/${YT_DLP_ASSET}" \
      --output /usr/local/bin/yt-dlp \
    && printf '%s  %s\n' "${YT_DLP_SHA256}" /usr/local/bin/yt-dlp | sha256sum --check --strict \
    && chmod 0755 /usr/local/bin/yt-dlp \
    && curl --fail --location --silent --show-error \
      "https://github.com/denoland/deno/releases/download/v${DENO_VERSION}/${DENO_ASSET}" \
      --output /tmp/deno.zip \
    && printf '%s  %s\n' "${DENO_SHA256}" /tmp/deno.zip | sha256sum --check --strict \
    && unzip -q /tmp/deno.zip -d /usr/local/bin \
    && chmod 0755 /usr/local/bin/deno \
    && /usr/local/bin/yt-dlp --version \
    && /usr/local/bin/deno --version

# Build the static Svelte application.
FROM oven/bun:1.3.10 AS ui-builder
WORKDIR /app/ui

COPY ui/package.json ui/bun.lock ./
RUN bun install --frozen-lockfile --ignore-scripts

COPY ui/ ./
RUN bun run prepare && bun run build

# Build the Rust server and cache dependencies separately from application code.
FROM rust:1-slim-trixie AS rust-builder
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates pkg-config libdav1d-dev \
    && rm -rf /var/lib/apt/lists/*

COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY crates/db/Cargo.toml crates/db/Cargo.toml
COPY crates/server/Cargo.toml crates/server/Cargo.toml

RUN mkdir -p crates/db/src crates/server/src \
    && printf '%s\n' 'pub fn placeholder() {}' > crates/db/src/lib.rs \
    && printf '%s\n' 'fn main() {}' > crates/server/src/main.rs \
    && cargo build --release --locked

COPY crates/ crates/
COPY data/ data/
RUN touch crates/db/src/lib.rs crates/server/src/lib.rs crates/server/src/main.rs \
    && cargo build --release --locked --bin pandan

# Keep the runtime image small while retaining curl for its container health check.
FROM debian:trixie-slim AS runtime
WORKDIR /app

ARG PUID=99
ARG PGID=100

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl ffmpeg=7:7.1.5-0+deb13u1 libdav1d7 \
    && rm -rf /var/lib/apt/lists/* \
    && if ! getent group "${PGID}" >/dev/null; then groupadd --system --gid "${PGID}" pandan; fi \
    && useradd --system --non-unique --uid "${PUID}" --gid "${PGID}" --home-dir /app pandan \
    && mkdir -p /app/data \
    && chown -R "${PUID}:${PGID}" /app

COPY --from=rust-builder --chown=${PUID}:${PGID} /app/target/release/pandan ./pandan
COPY --from=ui-builder --chown=${PUID}:${PGID} /app/ui/build ./ui/build
COPY --from=media-tools /usr/local/bin/yt-dlp /usr/local/bin/yt-dlp
COPY --from=media-tools /usr/local/bin/deno /usr/local/bin/deno
COPY --chown=${PUID}:${PGID} THIRD_PARTY_NOTICES.md /usr/share/doc/pandan/THIRD_PARTY_NOTICES.md

USER ${PUID}:${PGID}

ENV DATABASE_URL=sqlite:///app/data/pandan.db \
    PANDAN_MEDIA_DIR=/app/data/podcasts \
    PANDAN_DOWNLOAD_DIR=/app/data/downloads \
    PANDAN_YTDLP_BIN=/usr/local/bin/yt-dlp \
    PANDAN_FFMPEG_BIN=/usr/bin/ffmpeg \
    PANDAN_DENO_BIN=/usr/local/bin/deno \
    PANDAN_DOWNLOADS_ENABLED=true \
    PORT=9651 \
    RUST_LOG=info

EXPOSE 9651

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["curl", "--fail", "--silent", "http://127.0.0.1:9651/api/health"]

ENTRYPOINT ["./pandan"]
