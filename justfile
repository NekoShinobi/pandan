set dotenv-load := true
set shell := ["bash", "-euo", "pipefail", "-c"]

export DEV_UID := env_var_or_default("DEV_UID", `id -u`)
export DEV_GID := env_var_or_default("DEV_GID", `id -g`)

DEPS_MIN_AGE_DAYS := "3"
CARGO_NEXTEST_VERSION := "0.9.143"
CARGO_MACHETE_VERSION := "0.9.2"
CARGO_MUTANTS_VERSION := "27.1.0"

# List available recipes.
[private]
default:
    @just --list

# Install Rust and Svelte dependencies exactly as locked.
[group('dev')]
setup: setup-tools
    cargo fetch --locked
    cd ui && bun install --frozen-lockfile

# Install the pinned Rust development tools used by repository recipes.
[group('dev')]
setup-tools:
    rustup component add rust-analyzer
    cargo install --locked --version "={{ CARGO_NEXTEST_VERSION }}" cargo-nextest
    cargo install --locked --version "={{ CARGO_MACHETE_VERSION }}" cargo-machete
    cargo install --locked --version "={{ CARGO_MUTANTS_VERSION }}" cargo-mutants

# Start the API and Vite dev servers with live reload.
[group('dev')]
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    trap 'kill 0' EXIT
    just dev-api &
    just dev-ui &
    wait

# Start the Rust API with bacon live reload.
[group('dev')]
dev-api:
    bacon --headless run

# Start the SvelteKit Vite dev server.
[group('dev')]
dev-ui:
    cd ui && bun run dev

# Build the production UI and release Rust binary.
[group('build')]
build: build-ui build-api

# Build only the Rust binary.
[group('build')]
build-api:
    cargo build --release

# Build only the Svelte UI.
[group('build')]
build-ui:
    cd ui && bun run build

# Fast type-check for Rust and Svelte.
[group('checks')]
check: check-api check-ui

# Type-check the Rust workspace.
[group('checks')]
check-api:
    cargo check --workspace

# Type-check the Svelte UI.
[group('checks')]
check-ui:
    cd ui && bun run check

# Run the Rust test suite.
[group('checks')]
test:
    cargo test --workspace

# Run the Rust test suite with nextest's process-per-test runner.
[group('checks')]
test-nextest:
    cargo nextest run --workspace

# Mutation-test both Rust crates using nextest for the generated test runs.
[group('checks')]
test-mutants:
    cargo mutants --workspace

# Run all backend and frontend linters.
[group('checks')]
lint: lint-api lint-ui

# Run Clippy across the Rust workspace.
[group('checks')]
lint-api:
    cargo clippy --workspace --all-targets

# Run ESLint and Prettier checks over the frontend.
[group('checks')]
lint-ui:
    cd ui && bun run lint

# Format Rust and frontend sources.
[group('checks')]
fmt:
    cargo fmt --all
    cd ui && bun run format

# Verify formatting without changing files.
[group('checks')]
fmt-check:
    cargo fmt --all -- --check
    cd ui && bun run format:check

# Run the complete pre-commit gate.
[group('checks')]
ci: fmt-check check lint test

# Show available dependency updates without changing lockfiles.
[group('deps')]
deps-outdated:
    cargo update --dry-run
    cd ui && bun outdated

# Report likely unused Rust dependencies without changing manifests.
[group('deps')]
deps-unused:
    cargo machete

# Refresh lockfiles within declared semver ranges after a three-day cooldown.
[group('deps')]
deps-update:
    #!/usr/bin/env bash
    set -euo pipefail
    days="{{ DEPS_MIN_AGE_DAYS }}"
    seconds="$(( days * 24 * 60 * 60 ))"
    if ! cargo +nightly -Z help 2>&1 | grep -q 'min-publish-age'; then
      echo "error: nightly cargo cannot enforce the ${days}-day cooldown." >&2
      exit 1
    fi
    cargo +nightly update -Z min-publish-age --config "registry.global-min-publish-age = \"${days} days\""
    cd ui && bun update --minimum-release-age "$seconds"

# Refresh frontend dependencies across major versions deliberately.
[group('deps')]
deps-update-major:
    cd ui && bun update --latest --minimum-release-age "$(( {{ DEPS_MIN_AGE_DAYS }} * 24 * 60 * 60 ))"

# Scan Rust and frontend dependencies for known vulnerabilities.
[group('deps')]
deps-audit:
    cargo audit
    cd ui && bun audit

# Build and start the production stack in the foreground.
[group('docker')]
up:
    docker compose -f compose.yml up --build

# Build and start the production stack in the background.
[group('docker')]
up-detach:
    docker compose -f compose.yml up --build -d

# Start the fully containerized development stack with live reload.
[group('docker')]
up-dev:
    docker compose -f compose.dev.yml up --build

# Stop the production and development stacks.
[group('docker')]
down:
    -docker compose -f compose.dev.yml down
    -docker compose -f compose.yml down

# Follow production container logs.
[group('docker')]
logs:
    docker compose -f compose.yml logs -f

# Follow development container logs.
[group('docker')]
logs-dev:
    docker compose -f compose.dev.yml logs -f

# Validate both Compose configurations without starting containers.
[group('docker')]
compose-check:
    docker compose -f compose.yml config --quiet
    docker compose -f compose.dev.yml config --quiet

# Build the production container image without starting it.
[group('docker')]
docker-build:
    docker build -t pandan .

# Delete host and container-development SQLite files; they are recreated on startup.
[group('db')]
db-reset:
    rm -f data/pandan.db data/pandan.db-shm data/pandan.db-wal
    rm -f .devdata/pandan.db .devdata/pandan.db-shm .devdata/pandan.db-wal

# Copy the example environment file without overwriting an existing one.
[group('misc')]
init:
    cp -n .env.example .env
