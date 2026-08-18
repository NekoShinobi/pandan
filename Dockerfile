# syntax=docker/dockerfile:1.7

# Build the static Svelte application.
FROM oven/bun:1.3.10 AS ui-builder
WORKDIR /app/ui

COPY ui/package.json ui/bun.lock ./
RUN bun install --frozen-lockfile --ignore-scripts

COPY ui/ ./
RUN bun run prepare && bun run build

# Build the Rust server and cache dependencies separately from application code.
FROM rust:1-slim-bookworm AS rust-builder
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates pkg-config \
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
FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 pandan \
    && useradd --system --uid 10001 --gid pandan --home-dir /app pandan \
    && mkdir -p /app/data \
    && chown -R pandan:pandan /app

COPY --from=rust-builder --chown=pandan:pandan /app/target/release/pandan ./pandan
COPY --from=ui-builder --chown=pandan:pandan /app/ui/build ./ui/build

USER pandan:pandan

ENV DATABASE_URL=sqlite:///app/data/pandan.db \
    PORT=9651 \
    RUST_LOG=info

EXPOSE 9651

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["curl", "--fail", "--silent", "http://127.0.0.1:9651/api/health"]

ENTRYPOINT ["./pandan"]
