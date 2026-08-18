# Pandan agent guide

This file is the source of truth for AI-assisted changes. Keep the public `README.md` concise and user-facing; put implementation rules, invariants, and maintenance guidance here.

## Tooling

- Prefix every shell command with `rtk`.
- Use `rg` or `rg --files` for search.
- Use `apply_patch` for manual file edits.
- Preserve unrelated work in the working tree. Never reset, overwrite, or delete user changes.
- Read a file before editing it and keep changes narrowly scoped.

## Repository map

- `ui/` — SvelteKit 5 static frontend, TypeScript, Tailwind CSS 4.
- `ui/src/routes/+page.svelte` — application shell, authentication, settings, dashboard, and Tasks.
- `ui/src/lib/` — feature pages and reusable widgets.
- `ui/src/app.css` — shared visual system and component styling.
- `ui/src/lib/api.ts` — typed browser API client.
- `crates/server/` — Actix Web API, authentication, remote integrations, and static UI serving.
- `crates/db/` — SQLx entities, queries, migrations, and SQLite connection lifecycle.
- `SCHEMA.md` — human-readable database contract; update it with schema changes.
- `justfile` — canonical development, build, test, and container commands.

## Product invariants

- Pandan has one dashboard canvas per account. Do not restore the removed multi-workspace UI.
- All private records and assets must be scoped to the authenticated account in both handlers and queries.
- Administrator checks are enforced by the server, never only by the interface.
- The final administrator cannot be demoted or deleted.
- The initial administrator setup is one-time and claimed atomically.
- Sidebar navigation order is Dashboard, Tasks, Contacts, Calendar, RSS, Journal, YouTube, Coding, Subscriptions, Trading.

### Appearance and uploads

- Wallpaper slots are:
  - `dashboard` — legacy private slot retained for existing data and API compatibility; do not expose it as a separate selector.
  - `welcome` — private, per user, used by `welcome:{user}` and as the persistent background behind authenticated pages.
  - `loading` — private, per user, used by the authenticated loading transition.
  - `login` — global, administrator-managed, publicly readable before authentication.
- Wallpaper formats are JPEG, PNG, WebP, and AVIF, with a 30 MB limit.
- Avatars are private, per user, use the same image formats, and have a 10 MB limit.
- Task attachments are private, per user, and limited to 10 MB.
- Uploaded files are stored in SQLite. Keep content type, authorization, and size validation on the server.

### Authentication and secrets

- Passwords use Argon2id outside Actix async workers.
- Sessions are HTTP-only and SameSite Strict. Production HTTPS requires `COOKIE_SECURE=true`.
- OIDC uses discovery, authorization code flow with PKCE, nonce validation, a browser-bound state cookie, and single-use persisted state. Its callback URL is derived from `PANDAN_BASE_URL`.
- OIDC is disabled when all required values are absent and rejected when configuration is partial.
- Authentication policy is administrator-managed. Enforce password login, password registration, and OIDC registration switches on the server; never permit password login to be disabled when OIDC is unavailable.
- `PANDAN_SECRET_KEY` is optional. When present it must decode from base64 to exactly 32 bytes.
- Provider credentials use XChaCha20-Poly1305 and must never appear in API responses or logs.
- If the secret key is absent, anonymous integrations remain available but credential storage stays disabled.

### Remote content

- RSS, ICS, Invidious, Gitea, and Forgejo URLs must use public HTTPS destinations.
- Preserve DNS/IP validation against loopback, private, link-local, multicast, and reserved ranges.
- Preserve bounded redirects, connection/request timeouts, response-size limits, and per-provider failure isolation.
- YouTube channel metadata refreshes every two hours through configured Invidious first and the public YouTube feed second. Shared portrait images are stored in SQLite and refreshed at most every 24 hours; failed portrait responses must never populate the cache.
- Render user Markdown through the existing sanitizer. Custom HTML and iframe widgets remain sandboxed.

### Bundled data

- `data/english-revised-version.json` is the packaged source for the daily Bible Verse widget. Keep verse selection local and deterministic; do not replace it with a runtime network request.

## Frontend conventions

- Use Svelte 5 patterns and run the Svelte autofixer after modifying a `.svelte` file.
- Use Lucide Svelte for interface icons; do not introduce emoji controls.
- Build standard actions from `ui-button` plus exactly one role class: `ui-button--primary`, `ui-button--secondary`, `ui-button--ghost`, or `ui-button--danger`. Add `ui-button--icon` only for icon-only controls.
- Use GridStack for dashboard placement and resizing. Layout is editable only in Edit mode.
- Preserve keyboard access, visible focus states, minimum touch targets, reduced-motion behavior, and mobile reflow.
- Keep the terminal visual language: near-black surfaces, restrained green accents, monospaced content, crisp borders, and translucent structure over wallpaper.
- Reuse existing CSS tokens and components before adding new styles. Avoid isolated hardcoded colors.
- Keep modals centered, consistently structured, dismissible by their close control, and animated from the top with reduced-motion fallbacks.

## Backend and database conventions

- Rust workspace edition is 2024 with MSRV 1.85.
- Keep reusable SQL in `crates/db/src/queries.rs` and API orchestration in `crates/server/src/lib.rs` or a focused server module.
- Never edit an existing migration after it may have shipped.
- For a schema change:
  1. Add the next numbered file in `crates/db/migrations/`.
  2. Register it in `MIGRATIONS` in `crates/db/src/lib.rs`.
  3. Update entities and queries.
  4. Update migration tests and `SCHEMA.md`.
- Maintain compatibility with existing SQLite databases and preserve account data during table rebuilds.
- Return structured, user-safe errors; do not expose secrets or upstream response bodies.

## Verification

Run checks proportional to the change. Before handing off a cross-stack change, prefer:

```sh
rtk cargo fmt --all -- --check
rtk cargo test --workspace
rtk bun run check        # from ui/
rtk bun run build        # from ui/
rtk git diff --check
```

Use `just ci` for the complete local gate. Update `README.md`, `SCHEMA.md`, and this guide when user-visible behavior, schema contracts, or contributor invariants change.
