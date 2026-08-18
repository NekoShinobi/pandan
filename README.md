<div align="center">

# Pandan

**A self-hosted, terminal-inspired command center for the things you follow and do.**

[![Version](https://img.shields.io/badge/version-0.1.0-8fd6a3?style=flat-square)](Cargo.toml)
[![Rust](https://img.shields.io/badge/Rust-2024%20%7C%201.85%2B-1f2321?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Svelte](https://img.shields.io/badge/Svelte-5-ff3e00?style=flat-square&logo=svelte&logoColor=white)](https://svelte.dev/)
[![SQLite](https://img.shields.io/badge/SQLite-persistent-21618c?style=flat-square&logo=sqlite&logoColor=white)](https://sqlite.org/)
[![Docker](https://img.shields.io/badge/Docker-ready-2496ed?style=flat-square&logo=docker&logoColor=white)](#docker-deployment)

</div>

Pandan brings planning, reading, contacts, calendars, notes, lightweight instance posts, subscriptions, and software activity into one private web application. Each account gets an isolated dashboard canvas, personal settings and uploads, and a responsive interface built around a restrained terminal aesthetic.

> [!IMPORTANT]
> Pandan is under active development. Back up the database before upgrading and review the deployment and security settings before exposing an installation to the internet.

## Contents

- [Features](#features)
- [Dashboard widgets](#dashboard-widgets)
- [Docker deployment](#docker-deployment)
- [First run](#first-run)
- [Configuration](#configuration)
- [OpenID Connect](#openid-connect)
- [Data, backups, and upgrades](#data-backups-and-upgrades)
- [Security model](#security-model)
- [Local development](#local-development)
- [Architecture](#architecture)
- [Troubleshooting](#troubleshooting)

## Features

| Area | What it provides |
| --- | --- |
| **Dashboard** | One GridStack canvas per account with movable and resizable widgets; layout changes are enabled only in Edit mode. |
| **Tasks** | Priorities, descriptions, labels, subtasks, attachments, due dates, recurring schedules, completion, archiving, and completed-task cleanup. |
| **Kanban** | Shared workspaces with multiple boards, Todo / In Progress / Finished defaults, drag-and-drop cards, sanitized Markdown descriptions, assignees, live labels, due dates, comments, checklists, attachments, favorites, archives, in-app invitations, and configurable Admin / Member / Guest permissions. |
| **Contacts** | Search, tags, favorites, archives, portraits, yearless birthdays, important dates, Pandan/Monica JSON import, Pandan JSON export, and CardDAV synchronization. |
| **Calendar** | Task due dates, multiple public HTTPS ICS feeds, recurring events, custom source colors, contact birthdays, a month grid, and a selected-day agenda. |
| **RSS** | RSS and Atom subscriptions plus Reddit subreddit helpers, categories, source filters, article details, read state, a pruning-safe Read Later queue, manual refresh, and age-based retention. |
| **Journal** | Nested documents that can contain Markdown content and child documents; rendered Markdown is sanitized before display. |
| **Lines** | A Markdown timeline with public or private posts, replies, hashtag discovery, search, file attachments, reactions, and administrator moderation of public posts. |
| **YouTube** | Channel subscriptions, groups, a private Watch Later queue, thumbnail or compact layouts, manual refresh, and server-side shared metadata caching. |
| **Coding** | Releases from GitHub, GitLab, Codeberg, Gitea, and Forgejo; connected accounts can also show owned repositories, open pull requests, and GitLab pipelines. |
| **Subscriptions** | Recurring service costs, first-payment dates, filtering, and separate daily, weekly, monthly, and yearly totals for each currency. |
| **Trading** | A navigation placeholder for a future market workspace; watchlists and trade planning are not implemented yet. |

Accounts also have a private avatar, one to five sidebar monitor timezones, temperature and location preferences, a default Lines visibility, background adjustments, and a Welcome wallpaper used by the authenticated loading transition and behind authenticated pages. Administrators manage accounts, authentication policy, public Lines moderation, and the public Login wallpaper. Each user can permanently clear one content area at a time from Settings after explicit confirmation.

## Dashboard widgets

Pandan currently includes:

- Weather, a deterministic daily Bible verse from the bundled English Revised Version, task summary, task list, task progress, web search, focus timer, curated feed, feed sources, personal calendar, and world clocks.
- YouTube uploads, RSS/Atom feeds, Reddit, market symbols, code releases, and Twitch or Kick live-channel status.
- Sandboxed custom HTML and HTTPS iframe widgets. Custom HTML cannot run scripts, navigate the parent, or access the parent page; remote sites may still refuse iframe embedding.

Provider credentials are optional. Anonymous integrations remain usable without a server encryption key, while features that persist Reddit, Twitch, CardDAV, or source-control credentials require `PANDAN_SECRET_KEY`.

## Docker deployment

Docker Compose is the recommended way to run Pandan. It builds the UI and Rust server into one read-only production container and stores SQLite in a named volume.

### Build from source

Requirements: Git, Docker with Compose v2, and optionally [Just](https://just.systems/).

```sh
git clone https://github.com/NekoShinobi/pandan.git
cd pandan
just init
```

Review `.env`, then start Pandan:

```sh
just up-detach
```

Without Just, the equivalent commands are:

```sh
cp -n .env.example .env
docker compose -f compose.yml up --build -d
```

Open [http://localhost:9651](http://localhost:9651), or use the host port assigned through `PORT`.

Useful production commands:

```sh
just logs       # Follow application logs
just down       # Stop production and development stacks; keep named volumes
just up         # Rebuild and run production in the foreground
```

The `latest` branch publishes `ghcr.io/nekoshinobi/pandan:latest`. Pull requests build the image but do not publish it. The supplied Compose file intentionally builds the checked-out source instead of pulling the registry image.

### Reverse proxy and HTTPS

When Pandan is served at `https://pandan.example.com`:

```dotenv
COOKIE_SECURE=true
PANDAN_BASE_URL=https://pandan.example.com
```

Terminate TLS at the reverse proxy, proxy requests to port `9651`, and preserve the `/api` path. `COOKIE_SECURE=true` prevents browsers from sending the session cookie over plain HTTP. Ensure the proxy accepts request bodies larger than the configured upload you intend to use; wallpapers can be up to 30 MB.

## First run

A new database opens a one-time setup screen. Create the initial account with an email address, display name, and a password between 10 and 128 characters, or use the configured OIDC provider. A verified OIDC identity can claim setup even when registration of new OIDC accounts is disabled. The setup transaction atomically creates the account, links its OIDC identity when applicable, claims initialization, and makes that account the first administrator.

After setup, administrators can use **Settings → User administration** to:

- enable or disable password login, password registration, and registration of new OIDC identities;
- promote or demote other accounts; and
- remove other accounts.

Pandan prevents an administrator from deleting their active account or removing the final administrator. Password login cannot be disabled unless OIDC is configured.

## Configuration

`just init` copies `.env.example` to `.env` without overwriting an existing file. Docker Compose reads `.env` automatically. When running the binary directly, export the same values in its environment.

### Server settings

| Variable | Default | Description |
| --- | --- | --- |
| `DATABASE_URL` | `sqlite://data/pandan.db` | SQLx SQLite URL. Production Compose overrides this with `sqlite:///app/data/pandan.db`. |
| `PORT` | `9651` | Server port when running directly; in production Compose it selects the published host port while the container continues to listen on `9651`. |
| `RUST_LOG` | `info` | Rust tracing filter, such as `info`, `debug`, or a crate-specific filter. |
| `COOKIE_SECURE` | `false` | Accepts `1`, `true`, or `yes` (case-insensitive) to mark session and OIDC state cookies as HTTPS-only. |
| `PANDAN_BASE_URL` | unset in the server | Public absolute HTTP(S) application URL. Required when OIDC is enabled and used to derive its callback URL. |
| `PANDAN_SECRET_KEY` | unset | Base64 text that decodes to exactly 32 bytes. Enables encrypted provider-credential storage. |
| `INVIDIOUS_BASE_URL` | unset | Optional public HTTPS Invidious base URL used before YouTube's public uploads feed. |

### Production container settings

| Variable | Default | Description |
| --- | --- | --- |
| `PUID` | `99` | Numeric user ID assigned to the production application process at image build time. |
| `PGID` | `100` | Numeric group ID assigned to the production application process at image build time. |

The supplied Compose file passes these values into the production image build. Rebuild the image after changing either value. Development containers continue to use `DEV_UID` and `DEV_GID` so bind-mounted source files match the current host user.

Volumes created by older Pandan images may still be owned by `10001:10001`. Before starting the new image against such a volume, update it once (replace `99:100` if you configured different IDs):

```sh
docker compose -f compose.yml run --rm --no-deps --user root \
  --cap-add CHOWN --cap-add DAC_OVERRIDE --entrypoint chown app \
  -R 99:100 /app/data
```

Generate a credential-encryption key with:

```sh
openssl rand -base64 32
```

Add the result to `.env` as `PANDAN_SECRET_KEY` before saving provider credentials. Keep it secret, include it in secure operational backups, and keep it stable across restarts; stored credentials are encrypted with XChaCha20-Poly1305 and are not returned by the API.

### OIDC settings

| Variable | Required | Description |
| --- | --- | --- |
| `OIDC_ISSUER` | With OIDC | Exact issuer URL advertised by the provider's discovery document; this is not the authorization or login endpoint. |
| `OIDC_CLIENT_ID` | With OIDC | Registered client identifier. |
| `OIDC_CLIENT_SECRET` | With OIDC | Registered client secret. |
| `OIDC_PROVIDER_NAME` | No | Login button label; defaults to `Single sign-on` in the server. |
| `PANDAN_BASE_URL` | With OIDC | Public application URL from which the callback is derived. |

OIDC stays disabled when issuer, client ID, and client secret are all absent. A partial OIDC configuration causes startup to fail instead of silently weakening authentication.

### Development-only settings

| Variable | Default | Description |
| --- | --- | --- |
| `API_URL` | `http://localhost:${PORT}` | API target for the Vite development proxy. Container development sets it to the API service. |
| `UI_PORT` | `5173` | Published Vite port in `compose.dev.yml`. |
| `VITE_USE_POLLING` | `true` in `.env.example` | Uses polling for Vite file watching, which is useful with bind mounts. |
| `DEV_UID` / `DEV_GID` | Current host IDs through Just; otherwise `1000` | Ownership used by the development containers and named caches. |

## OpenID Connect

Pandan uses provider discovery and the authorization-code flow with PKCE, nonce validation, a browser-bound HTTP-only SameSite Lax state cookie, and single-use persisted authorization state. It requests the `email` and `profile` scopes and requires a verified email claim.

Set all required values and register this callback with the identity provider:

```text
<PANDAN_BASE_URL>/api/auth/oidc/callback
```

Trailing slashes are normalized. For example, both `https://pandan.example.com` and `https://pandan.example.com/` produce:

```text
https://pandan.example.com/api/auth/oidc/callback
```

A path prefix is preserved: `https://example.com/pandan/` becomes `https://example.com/pandan/api/auth/oidc/callback`. `PANDAN_BASE_URL` must be an absolute HTTP(S) URL without embedded credentials, a query string, or a fragment.

`OIDC_ISSUER` is not normalized in the same way. Copy the issuer exactly from the provider's OpenID discovery document, including any realm, tenant, or path and its trailing-slash convention.

OIDC can sign in an existing linked identity or link a verified email to an existing account. Whether a previously unseen identity may create an account is controlled separately by the administrator's OIDC registration setting. When the provider supplies a `picture` claim and the account has no avatar, Pandan imports the supported public HTTPS image without replacing an avatar the user already chose.

## Data, backups, and upgrades

All account records, cached remote content, avatars, wallpapers, contact photos, task and Kanban attachments, and encrypted provider credentials live in SQLite. Pending migrations are embedded in the server and applied automatically at startup. SQLite uses WAL mode, a five-second busy timeout, and an eight-connection pool.

| Run mode | Database location |
| --- | --- |
| Production Compose | `/app/data/pandan.db` in the `pandan-data` named volume |
| Container development | `.devdata/pandan.db` in the repository bind mount |
| Host development/default binary | `data/pandan.db` |

### Back up Compose data

Stop writes before copying the SQLite directory:

```sh
docker compose -f compose.yml stop app
mkdir -p backup
docker compose -f compose.yml cp app:/app/data/. ./backup/pandan-data/
docker compose -f compose.yml start app
```

Store the backup together with the matching `PANDAN_SECRET_KEY` if encrypted credentials need to remain usable. For a direct host installation, stop Pandan and copy `data/pandan.db` (and any adjacent `-wal` or `-shm` files if present) to backup storage.

### Upgrade a source deployment

```sh
git pull --ff-only
just up-detach
```

Create a backup first. The second command rebuilds the image, recreates the container, preserves the named data volume, and applies pending database migrations when the new server starts.

The `just db-reset` recipe permanently deletes the host and container-development databases. It does not delete the production named volume, but it should still be used only when a clean development database is intended.

### Upload limits

| Content | Accepted formats | Limit |
| --- | --- | --- |
| Wallpapers | JPEG, PNG, WebP, AVIF | 30 MB |
| Avatars and contact photos | JPEG, PNG, WebP, AVIF | 10 MB |
| Task attachments | Any content type accepted by the task attachment endpoint | 10 MB |
| Kanban card attachments | Any content type accepted by the Kanban attachment endpoint | 10 MB |
| Contact JSON import | Pandan or Monica JSON | 64 MB and at most 10,000 records |

## Remote content behavior

- User-supplied RSS, ICS, CardDAV, Invidious, Gitea, and Forgejo destinations must use public HTTPS URLs. Server-side fetches reject loopback, private, link-local, multicast, and reserved destinations after DNS resolution.
- Remote requests use bounded redirects, connection and request timeouts, response-size limits, and isolated provider errors so one failing provider does not break an entire page.
- YouTube channel metadata refreshes every two hours. Pandan tries configured Invidious first and YouTube's public uploads feed second. Shared channel portraits are stored in SQLite and refreshed at most every 24 hours; invalid or failed portrait responses are not cached.
- The daily Bible Verse widget selects locally and deterministically from the packaged English Revised Version data; it makes no runtime verse request.

## Security model

- Private records and uploads are scoped to the authenticated account by both handlers and database queries.
- Kanban resources are scoped through active workspace membership and the server-enforced Admin, Member, and Guest permission matrix. Invitations can target only existing Pandan accounts, and the final workspace administrator cannot be removed or demoted.
- Passwords are hashed with Argon2id outside Actix async workers.
- Sessions use random server-side tokens in HTTP-only, SameSite Strict cookies and expire after 30 days. Set `COOKIE_SECURE=true` for HTTPS.
- Administrator authorization is enforced by the server, including the invariant that at least one administrator remains.
- Stored provider secrets use XChaCha20-Poly1305 when `PANDAN_SECRET_KEY` is configured. Secrets are neither included in API responses nor written to logs.
- User Markdown is sanitized, while custom HTML and iframe widgets run in browser sandboxes.
- The supplied production Compose configuration runs the container as an unprivileged user with a read-only root filesystem, a temporary `/tmp`, all Linux capabilities dropped, and `no-new-privileges` enabled.

These controls reduce common self-hosting risks, but Pandan is not a substitute for TLS, network access controls, database backups, secret management, and timely dependency updates.

## Health check

The unauthenticated health endpoint checks API and database availability:

```sh
curl --fail http://localhost:9651/api/health
```

A healthy response is:

```json
{"status":"ok","database":"connected"}
```

Both production and development containers use this endpoint for their Docker health checks.

## Local development

Host development requires:

- Rust stable with the project's MSRV of 1.85, plus `rustfmt` and Clippy;
- Bun 1.3.10 and Node.js (Node 24 is used by the development container);
- [Just](https://just.systems/); and
- [Bacon](https://dystroy.org/bacon/) for Rust rebuild/restart behavior.

Install locked dependencies and start both development servers:

```sh
just init
just setup
just dev
```

Open [http://localhost:5173](http://localhost:5173). Vite proxies `/api` to the Rust server on port `9651`; edits to either side reload during development.

For a fully containerized development loop:

```sh
just up-dev
```

The development UI is published on `UI_PORT` (default `5173`) and the API on `PORT` (default `9651`). Source files are bind-mounted, while Rust targets, registries, frontend dependencies, Svelte output, and Bun downloads use named caches.

### Quality and build commands

```sh
just check          # Cargo check plus Svelte/TypeScript checks
just test           # Rust workspace tests
just lint           # Clippy, ESLint, and Prettier checks
just fmt            # Format Rust and frontend sources
just fmt-check      # Verify formatting without changing files
just ci             # Complete local gate: format, check, lint, and test
just build          # Static Svelte UI and release Rust binary
just compose-check  # Validate production and development Compose files
just deps-audit     # Audit Rust and frontend dependencies
```

A host production build places the binary at `target/release/pandan` and the static UI at `ui/build`. Run the binary from the repository root so its relative `./ui/build` path resolves correctly.

## Architecture

```text
Browser
  └─ SvelteKit 5 static application (Tailwind CSS 4, GridStack)
       └─ /api
            └─ Actix Web server
                 ├─ authentication and account-scoped application logic
                 ├─ bounded remote-provider clients
                 └─ SQLx → SQLite
```

The production image builds both applications and serves the static Svelte output and JSON API from the same Actix process and origin. The Rust workspace uses edition 2024 and contains:

- `crates/server/` — HTTP server, authentication, uploads, integrations, and static UI serving;
- `crates/db/` — SQLite entities, queries, migrations, and connection lifecycle;
- `ui/` — SvelteKit 5 static frontend; and
- `data/english-revised-version.json` — packaged daily-verse source.

See [SCHEMA.md](SCHEMA.md) for the database contract and [AGENTS.md](AGENTS.md) for contributor and AI-assisted implementation rules.

## Troubleshooting

### OIDC configuration or discovery error at startup

Set `OIDC_ISSUER`, `OIDC_CLIENT_ID`, and `OIDC_CLIENT_SECRET` together, and set an absolute `PANDAN_BASE_URL` when OIDC is enabled. Remove all three OIDC values to disable it. Confirm that the provider callback exactly matches the derived URL documented above.

If discovery reports an issuer error, check `OIDC_ISSUER` rather than `PANDAN_BASE_URL`: it must be the exact issuer advertised in the provider's `/.well-known/openid-configuration` document, not its login, authorization, token, or discovery-document URL.

`Failed to parse server response` means an OIDC endpoint returned content that did not match the JSON response expected by the client. Startup discovery fetches and validates both the provider metadata and the JSON Web Key Set (JWKS) referenced by its `jwks_uri`. Verify both URLs from the Pandan container; each must return the expected JSON rather than an HTML login, proxy, or error page.

If adding the issuer's advertised trailing slash changes an `unexpected issuer URI` error into `Failed to parse server response`, the slash is correct: issuer validation has succeeded and the failure is likely the subsequent `jwks_uri` response. If parsing instead fails after the provider redirects back to Pandan, inspect the preceding `OIDC code exchange failed` log and verify the provider's token endpoint, client authentication method, client ID, and client secret.

### Secure-cookie login loop

`COOKIE_SECURE=true` requires the browser-facing site to use HTTPS. For local plain-HTTP development, leave it `false`. Behind a reverse proxy, access Pandan through the HTTPS origin rather than the internal HTTP port.

### Provider credentials cannot be saved

Set `PANDAN_SECRET_KEY` to a valid base64-encoded 32-byte value and restart the server. `openssl rand -base64 32` produces the expected format.

### Remote feed, calendar, or self-hosted Git URL is rejected

Pandan intentionally rejects plain HTTP and destinations that resolve to non-public address ranges. Use a public HTTPS endpoint with valid DNS. Redirects to blocked destinations are rejected as well.

### Container development files have the wrong owner

Run container development through `just up-dev`; Just exports the current host UID and GID, and the entrypoint repairs named-cache ownership before dropping privileges. If Compose is invoked directly, set `DEV_UID` and `DEV_GID` explicitly.

### Database health check fails

Inspect `just logs`, verify that the data volume is writable by the container, and confirm `DATABASE_URL`. Migrations run on startup, so the application log also reports migration or database-opening failures.
