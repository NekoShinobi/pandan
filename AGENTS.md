# Pandan agent guide

This file is the source of truth for AI-assisted changes. Keep the public `README.md` concise and user-facing; put implementation rules, invariants, and maintenance guidance here.

## Tooling

- Prefix every shell command with `rtk`.
- Use `rg` or `rg --files` for search.
- Use `apply_patch` for manual file edits.
- Preserve unrelated work in the working tree. Never reset, overwrite, or delete user changes.
- Read a file before editing it and keep changes narrowly scoped.
- `rust-toolchain.toml` supplies `rust-analyzer`, `rustfmt`, and Clippy. `just setup` installs
  the pinned `cargo-nextest`, `cargo-machete`, and `cargo-mutants` releases; use
  `just setup-tools` to refresh only those tools.
- Use `just test-nextest` for a faster local Rust test loop, `just deps-unused` for a read-only
  unused-dependency scan, and `just test-mutants` for deliberate mutation-testing runs. Mutation
  testing is intentionally excluded from `just ci` because it runs the suite once per generated
  mutation.
- Keep the yt-dlp and Deno versions, architecture checksums, and Debian FFmpeg pin in
  `Dockerfile.dev` synchronized with the production `Dockerfile`; both container modes must expose
  the complete Downloads toolchain without installing or updating it at application startup.

## Repository map

- `ui/` — SvelteKit 5 static frontend, TypeScript, Tailwind CSS 4.
- `ui/src/routes/+page.svelte` — application shell, authentication, settings, dashboard, Tasks, and product navigation.
- `ui/src/lib/KanbanPage.svelte` — Kanban boards, card collaboration, workspace membership, and permission settings.
- `ui/src/lib/PodcastsPage.svelte` — podcast catalogue, requests, listening views, and the administrator review queue.
- `ui/src/lib/MusicPage.svelte` — per-account Jellyfin music libraries, collections, search, and queues.
- `ui/src/lib/podcastPlayer.svelte.ts` — module-scoped podcast, Jellyfin, and completed Downloads audio playback state. The shell owns the single `<audio>` element so playback survives section changes; never move it into a page component.
- `ui/src/lib/WallsPage.svelte` — shared wallpaper collection, submission composer, and the administrator review queue.
- `ui/src/lib/` — feature pages and reusable widgets.
- `ui/src/app.css` — shared visual system and component styling.
- `ui/src/app.html` — document shell; owns the title, description, icons, and link-preview tags.
- `assets/og/` — offline design tool that renders the link-preview card and icons into `ui/static/`.
- `ui/src/lib/api.ts` — typed browser API client.
- `DESIGN.md` — persistent interface and interaction rules; read it before frontend changes.
- `crates/server/` — Actix Web API, authentication, remote integrations, and static UI serving.
- `crates/server/src/document.rs` — renders the application document and its absolute link-preview URLs.
- `crates/server/src/walls.rs` — Walls HTTP surface: submission, moderation, image serving, and applying a wall to a wallpaper slot.
- `crates/server/src/podcasts.rs` — podcast HTTP surface: catalogue, requests, subscriptions, playback, and listening state.
- `crates/server/src/podcast_media.rs` — guarded feed and audio fetching, on-disk episode storage, eviction, startup reconciliation, and the refresh and download workers.
- `crates/db/` — SQLx entities, queries, migrations, and SQLite connection lifecycle.
- `SCHEMA.md` — human-readable database contract; update it with schema changes.
- `justfile` — canonical development, build, test, and container commands.

## Product invariants

- Pandan has one dashboard canvas per account. Do not restore the removed multi-workspace UI.
- Pandan is installable as a root-scoped PWA on mobile and desktop. The service worker precaches
  only versioned UI/static assets and the offline connection document; it must never cache `/api`
  responses, authenticated records, uploads, avatars, ntfy data, or podcast audio. A waiting worker
  activates only after the user accepts the Update ready prompt, and `/service-worker.js` is served
  with `Cache-Control: no-cache` plus root scope.
- The dashboard right rail owns one account-scoped Twitch/Kick tracker backed by a `streams` widget
  whose `config_json.placement` is `utility_rail`. It accepts up to 20 accounts across separate
  provider lists, keeps legacy movable stream widgets intact, and replaces the removed
  `task-progress` widget and Task.Progress utility box.
- The dashboard right rail also owns one account-scoped bookmark list capped at 32 links. Bookmark
  destinations open directly in the browser. The server derives the origin `/favicon.ico`, fetches
  it through the `images` network policy with redirect and size guards, stores supported icon bytes
  in SQLite, and serves them only to the owning authenticated account. A favicon failure must never
  prevent the bookmark itself from being saved.
- The Bookmarks product page is separate from the dashboard right-rail bookmark list. It combines
  administrator-managed global categories with account-owned personal categories. Global mutations
  require a server-side administrator check; personal mutations and stored custom icon bytes stay
  account-scoped. Bookmark icons may use the supported Lucide name, a guarded favicon fetch that
  falls back from the conventional origin path to bounded HTML-declared icon discovery, or a guarded
  credential-free HTTPS custom icon fetch. SVG icon sources are rasterized before storage so active
  remote content is never served from Pandan's origin. A remote icon failure saves the bookmark with
  its local fallback. Deleting a category deletes its bookmarks.
- The dashboard right rail's Local.Time list shows every saved Sidebar Monitor timezone in the same
  order. Sidebar Monitor settings use the runtime's standardized IANA timezone list and keep one to
  five selections.
- Calendar month grids default to Sunday-first and share the account's adjustable week-start
  preference. The dashboard right-rail calendar supports month navigation and marks dated tasks,
  contact birthdays, and subscribed calendar events with their source colors.
- All private records and assets must be scoped to the authenticated account in both handlers and queries.
- Administrator checks are enforced by the server, never only by the interface.
- The final administrator cannot be demoted or deleted.
- The initial administrator setup is one-time and claimed atomically.
- Sidebar navigation starts with Announcements, Dashboard, Bookmarks, Tasks, Kanban, Contacts, Calendar, RSS, Journal, Lines, Walls, YouTube, Downloads, Podcasts, Music, Coding, Subscriptions, and Trading. Announcements is numbered `00`, while Dashboard remains the initial page at `01` and Bookmarks remains `02`. Kanban expands to Boards, Workspaces, and Invitations. Administrator-defined global embedded pages follow every built-in page, then the authenticated account's personal embedded pages. An administrator may move a global page into their own personal list or publish one of their own personal pages globally; the server preserves ownership isolation and destination limits. Custom entries rely on their `Global custom` / `Personal custom` group labels and retain `G` / `U` markers in collapsed mode; do not repeat the scope as a right-side badge. Their sidebar hover cards contain only the configured page title and description, without a global/personal static prefix. Expanded rows may use the destination's conventional favicon, a supported Lucide icon, or a credential-free HTTPS custom icon URL; remote image failures fall back to the panel icon, and the choice never replaces the collapsed `G` / `U` marker. Selecting a custom entry loads or reloads its iframe in place, while a trailing box-arrow control incorporated into the same sidebar row opens the destination externally. The row's hover lift moves the listing and external control together, with movement removed for reduced-motion users. The embedded view has no page heading or external action above it and uses the available product canvas. Each embedded page keeps a responsive width and a persisted iframe height from 320–2,400 pixels, defaulting to 720; the visible frame is capped at the product-view height so it never extends past the page, and settings offer 480, 720, and 1,080 pixel presets plus a custom option.
- Announcements is one instance-wide Markdown feed readable by every authenticated account. Only administrators may create, edit, delete, or attach images to announcements, and the server enforces those mutations. JPEG, PNG, WebP, and AVIF images are stored in SQLite at up to 10 MB each and served only through authenticated, no-sniff endpoints. Reactions use the fixed picker catalogue and are account-scoped; deleting an account removes its reactions but retains authored announcements with an unattributed administrator byline.
- Settings is a full product page reached from the existing sidebar control, never an account modal. Its second rail groups General (Preferences and Custom Pages), Security (User Settings, Sessions, and Data Management), and administrator-only Administration (Instance, Network, Logs, and User Management). Main background belongs only in Preferences; Login background belongs only in Instance Settings. Keep the page title, selected-category heading, and category rail anchored while only the selected category body scrolls. Sessions lists every unexpired account session by its last observed user agent and client IP, labels the current session, and lets the owner revoke any row without exposing the private token. Custom Pages and Data Management stay in the page body, while destructive actions still require an explicit second confirmation.
- Persistent application logs are administrator-only newline-delimited JSON under `PANDAN_LOG_DIR`. Keep writes off Actix workers, rotate before the configured file-size ceiling, prune rotated files by age and count, and leave the active file intact. The UI controls file enablement, minimum level, rotation, and retention; the directory remains operator-controlled. Never log credentials, access tokens, authorization or cookie headers, request bodies, query strings, complete source URLs, OIDC state/code/nonce values, Quick Connect secrets, or private session tokens.
- Tasks Active and Archived views share the same page structure: keep New task and Focus Mode visible, and do not collapse the worklist grid when switching views.
- The command palette is a global surface, not a dashboard feature. Keep its entry points page-independent: `Ctrl`/`Cmd` + `K`, the `/` key, and the header search control. Do not add palette triggers inside dashboard widgets or dashboard-only panels, and do not reintroduce the removed `search` web search widget; web search belongs in the palette's fallthrough row.
- Coding provider data is cached in memory per account for one hour. Opening the page uses that cache, the page's Refresh control bypasses it, and project or credential mutations invalidate it. Preserve the generation check so an in-flight request cannot restore stale data after invalidation.
- Notifications are a shell-level ntfy inbox opened from the header Bell, with a command-palette-accessible See all page rather than a new sidebar entry. Each account connects one ntfy.sh or a destination permitted by the server network policy and manually subscribes to up to 32 topics. The access token is encrypted with `PANDAN_SECRET_KEY` and never returned by the API. The Rust server owns one persistent upstream stream per configured account, stores deliveries even when no browser is open, and fans stored events out through the authenticated Pandan SSE endpoint. Browser sessions never connect to ntfy directly. Recovery sync remains a server-owned fallback, with per-topic cursors; changing the server clears the old local message cache and resets those cursors while retaining topic names. The Bell count updates immediately and new deliveries use the fixed-size bottom-right toast. Opening the Bell marks active messages seen. Its preview starts with five messages and loads older local rows in five-message pages as the user reaches the end of the scroll region. Clicking a preview opens that notification on the See all page; swiping left hides it only for the current Bell session, while swiping right sends ntfy's permanent sequence deletion upstream and removes the local row only after success. The See all page keeps cards at one collapsed height and exposes expansion only for larger records through the full-width notification title row and its trailing chevron, never a separate action button. It permanently deletes the selected topic or combined inbox through one Pandan request whose upstream sequence deletions run serially with non-blocking provider-limit pacing. There is no notification archive. `view` actions remain external links, `copy` stays in the browser, and user-triggered `http` actions run through the same server-egress policy.
- The Lines composer is avatar-first: the viewer's avatar sits beside the post entry with no heading above it, and the Private/Instance selector sits beside the Post button. Replies are composed in the centered reply modal, which quotes the parent post above the reply entry; do not restore the inline reply banner.
- Lines has three screens inside its page: the timeline, a thread screen, and an author screen. A post timestamp or reply count opens the thread screen, `Replying to {author}` opens the parent post's thread screen, and an avatar or author name opens the author screen. These are full screens with a Back control, never modals, and the composer belongs to the timeline only. Choosing Lines in the sidebar returns to the timeline even when Lines is already the active section, which the shell drives through the page's `homeToken` prop.
- Walls is the shared wallpaper collection. Any account may submit; only an administrator may approve or reject, and that check is enforced on the server. A `pending` or `rejected` wall is readable only by its submitter and by administrators, and is reported as missing to everyone else so review state never leaks. Only an `approved` wall may be applied to a wallpaper slot, including by its own submitter.
- The Walls search and tag filters are page furniture: they sit with the view tabs, outside the swapping body, and stay put between views. Every scope applies them, `mine` included, so keep `list_walls_by_submitter` in step with `list_walls` rather than short-circuiting the filters away.
- A wall's title, description, and tags stay editable by its submitter and by administrators at any status, including after a decision. Editing is descriptive only: it must never change `status`, `decision_note`, `decided_by`, or `decided_at`, which only approve and reject may move.
- Applying a wall to `login` requires an administrator. `apply` only ever writes a selection for the calling account plus that global slot; an administrator must never be able to change another user's background.
- Deleting an account leaves its walls in place with `user_id` set to NULL, shown as an unattributed contribution, so other people's backgrounds keep working. Walls is deliberately not one of the `UserContentScope` bulk-delete scopes.
- Wall submissions are decoded on the server to generate a thumbnail. Keep the decode inside `web::block` and keep the explicit `image::Limits` guard: the upload size limit bounds the compressed bytes only, and a small file can legitimately decode to gigabytes of pixel buffer.
- Kanban workspaces are collaboration aggregates and are distinct from the removed dashboard `user_workspaces` partition UI. Every board, column, card, comment, checklist, label, and attachment authorization must resolve through active `kanban_workspace_members` membership.
- Kanban roles are `admin`, `member`, and `guest` with the 24 kan.bn-compatible workspace/board/list/card/comment/member permissions. Admin grants are immutable, workspace manage/delete stay admin-only, per-member overrides are allowed for other permissions, and the final workspace admin cannot be demoted or removed.
- Kanban invitations are in-app only and may target existing Pandan users; do not add email delivery or arbitrary addresses.
- Deleting a Kanban column is refused by the server while it still holds active cards, and that message is what the board shows. Keep the check on the server rather than hiding the control, and keep the delete behind `list:delete`.
- The podcast catalogue is one administrator-curated set shared by the whole instance. Members request a feed; only an administrator publishes one. Never let a member route create a `podcasts` row.
- Jellyfin uses one administrator-selected server plus one encrypted Jellyfin identity per Pandan account. The browser must never receive a Jellyfin token, Quick Connect secret, authorization header, password, or complete upstream URL.
- Jellyfin playback is music-only. Discover allowed roots from the linked user's current `CollectionFolder` views with `CollectionType=music`; every item detail, image, audio stream, attachment download, playlist track, and playback report must independently require the selected root in the item's current ancestor chain. Audio additionally requires both `Type=Audio` and `MediaType=Audio`. Return `404` for anything outside that scope.
- Jellyfin artwork and audio are live authenticated proxies and must never be cached by the service worker. The shell-owned player keeps podcast behavior intact, exposes one persisted playback-speed control for podcasts, Jellyfin music, and completed Downloads audio, and reports Jellyfin start/progress/stop best effort without interrupting playback.
- A feed already in the catalogue never becomes a request. Compare on the normalized URL and answer with a subscription instead.
- Podcast requests keep their decision history. Rejections retain the administrator's reason for the requester, and only `pending` requests may be decided or withdrawn.
- Every podcast episode read — metadata, audio bytes, progress, queue, saved state — resolves through an active `podcast_subscriptions` row. An unsubscribed caller, administrator included, gets `404` rather than `403`, so responses do not leak which episodes exist.
- Cached episode audio is a shared instance resource. Clearing one account's podcast content, or deleting the account, must never delete a cached file; `podcast_downloads.requested_by` is `ON DELETE SET NULL` for exactly that reason.
- Caching a show's whole back catalogue (`POST /api/podcasts/{podcast_id}/downloads`) is administrator-only, for the same reason publishing a podcast is: it commits shared disk and shared bandwidth. It only queues what is uncached or previously failed, and the download worker still applies the storage budget, so it must never be reachable by a member or used to bypass a limit.
- The player bar is rendered by the shell as a sibling of `.dashboard-app`, not a descendant, so it does not inherit the tactical telemetry palette. It binds the terminal tokens itself in `ui/src/app.css`; without them its controls hover in the light root theme. Completed Downloads audio reuses this player without podcast progress writes or Jellyfin playback reports; downloaded video remains in the Downloads media dialog.
- The shell audio visualizer applies to Podcasts, Jellyfin music, and completed Downloads audio. Its canvas sits above the wallpaper and below `.dashboard-main`, defaults to Off, and exposes the complete catalogue in `ui/src/lib/audioVisualizationCatalog.ts`; keep every catalogue entry wired through the bounded Canvas renderer families rather than adding one-off page effects. Visibility (10–90%), intensity (50–250%), brightness and contrast (50–200%), base hue, response, and the Mono, Pandan, Signal, and Prism palettes persist together as device-local preferences and reset as one unit. Reduced-motion users receive a static rendering. Stereo analysis may branch from the shared source through analysis-only nodes, but the analyser and the 200% volume gain must share the single lazily created media-element audio graph; never create a second `MediaElementAudioSourceNode` for the shell audio element.
- `podcastPlayer` tracks the loaded source separately from the current episode. A play that fails because the file was not cached yet leaves the episode set and the source unusable, so a retry after the download lands must reload rather than resume, and must adopt the reloaded record that finally carries a duration.
- Eviction is least-recently-used on `last_accessed_at` and skips pinned files, in-flight transfers, and anything in a listener's play queue. When it cannot free enough, defer the download rather than exceeding the storage budget.
- An episode description is remote feed HTML stored verbatim. Render it through the sanitizer before it reaches the DOM, and force its links to `target="_blank"`: following one in this tab tears down the page and stops playback.
- Podcast volume runs from 0 to 200% and starts at 80%. `HTMLMediaElement.volume` is capped at 1, so anything above 100% runs through a lazily built `AudioContext` gain node; once that graph exists the element stays at unity and the gain carries the whole level, so the two never multiply. Build the graph only from a user gesture — created at mount it starts suspended and plays silently.
- Podcast discovery is deliberately absent. Requests carry a feed URL that the server previews; do not add a directory search, playlists, or a streaming proxy for remote audio.

### Link previews

- Pandan is a client-rendered application behind authentication, so a crawler only ever sees `ui/src/app.html`. Every URL previews as the same generic card; do not add per-route or per-record preview tags.
- The document title, description, and preview tags live in `app.html`, not in `svelte:head`. Setting a title in both appends a second `<title>` to the head and the browser honours the first.
- Preview tags carry the `__PANDAN_ORIGIN__` placeholder. `crates/server/src/document.rs` substitutes the public origin as it serves the document, so the application document is rendered and never handed to `actix_files`.
- The resolved origin is `PANDAN_BASE_URL` when configured and otherwise rebuilt from request headers. It is attacker-controlled in that second case: keep it restricted to characters that cannot escape an HTML attribute, fall back to root-relative URLs when it is not, and never reuse it for an authorization or redirect decision.
- `ui/static/og-card.png` is 1200x630; keep the declared `og:image:width` and `og:image:height` in step with it. Regenerate the card and icons with `assets/og/render-brand-assets.py`, which needs Pillow and JetBrains Mono and is not part of the build or CI.

### Appearance and uploads

- Wallpaper slots are:
  - `dashboard` — legacy private slot retained for existing data and API compatibility; do not expose it as a separate selector.
  - `welcome` — private, per user, exposed to every account under Settings → Preferences as Main background, used by the authenticated `Welcome:{user}` loading transition and as the persistent background behind authenticated pages.
  - `loading` — legacy private slot retained for existing data and API compatibility; do not expose it as a separate selector.
  - `login` — global, administrator-managed under Settings → Instance Settings, publicly readable before authentication.
- Every wallpaper slot resolves in one order: an applied wall in `user_wallpaper_selections`, then the uploaded image in `user_wallpapers`, then the packaged default. Only a wall that is still `approved` resolves, so a wall rejected or deleted after it was applied falls back on its own with no cleanup pass. Uploading to a slot clears its selection and applying a wall clears its upload, so the two sources can never disagree.
- The `login` slot stays a singleton across every administrator. Any writer — upload or apply — must clear both tables for that slot first, so the served image never depends on an `updated_at` tiebreak.
- Main and Login background processing are separate appearance records with the same bounded blur,
  brightness, contrast, and saturation controls. Main is personal and editable by every account;
  Login is global, publicly readable for signed-out rendering, and administrator-writable only.
- For an existing authenticated session, render the Welcome loading overlay in the initial server response so the dashboard surface never flashes before the boot transition.
- Wallpaper formats are JPEG, PNG, WebP, and AVIF, with a 30 MB limit.
- Avatars are private, per user, use the same image formats, and have a 10 MB limit. An OIDC `picture` claim may initialize a missing avatar through the guarded server-fetch policy, but must never replace an existing avatar or block login when fetching fails.
- Task attachments are private, per user, and limited to 10 MB.
- Lines attachments are limited to 10 MB. Their read access always follows the parent post: owner-only for private posts and authenticated-instance access for public posts.
- Kanban card attachments are limited to 10 MB. Reads and writes always follow the parent card's active workspace membership and effective permissions.
- Uploaded files are stored in SQLite. Keep content type, authorization, and size validation on the server.
- Downloaded podcast audio is the one exception: it is written to the media root on disk (`PANDAN_MEDIA_DIR`, default `data/podcasts`) so playback can be served with `NamedFile` and answer HTTP Range requests without holding an episode in memory. Authorization still happens in the handler; never mount the media root as static files. Podcast artwork stays a SQLite blob.

### Authentication and secrets

- Passwords use Argon2id outside Actix async workers.
- Sessions are HTTP-only and SameSite Strict. Production HTTPS requires `COOKIE_SECURE=true`.
- OIDC uses discovery, authorization code flow with PKCE, nonce validation, a browser-bound state cookie, and single-use persisted state. Its callback URL is derived from `PANDAN_BASE_URL`.
- OIDC is disabled when all required values are absent and rejected when configuration is partial.
- Authentication policy is administrator-managed. Enforce password login, password registration, and OIDC registration switches on the server; never permit password login to be disabled when OIDC is unavailable.
- `PANDAN_SECRET_KEY` is optional. When present it must decode from base64 to exactly 32 bytes.
- Provider credentials, including ntfy access tokens, use XChaCha20-Poly1305 and must never appear in API responses or logs.
- If the secret key is absent, anonymous integrations remain available but credential storage stays disabled.

### Remote content

- Server-side remote destinations default to public HTTPS. Administrator-managed `network_access_rules` may allow or deny an exact scheme, host, and port for all integrations or one of `rss`, `calendar`, `contacts`, `podcasts`, `notifications`, `coding`, `images`, `youtube`, `widgets`, or `jellyfin`. A deny match always wins; only an explicit allow may authorize HTTP or a private/reserved destination. Keep rules administrator-only and capped at 128.
- Embedded-page destinations must be absolute HTTPS URLs without credentials. The server stores configuration only and never fetches or proxies them. Their iframes default to the restricted `allow-forms allow-popups` sandbox and `no-referrer` policy. Persisted, independent opt-ins may add `allow-scripts` and/or `allow-same-origin`; both are disabled by default and may be enabled together when the user accepts the reduced isolation. Never add top-navigation, download, device, or location permissions.
- Network access rules apply only to requests made by Pandan. Embedded pages and ordinary external links are browser destinations and must not consult the server-egress allow/deny table; their separate URL and sandbox rules still apply.
- Preserve DNS/IP validation against loopback, private, link-local, multicast, reserved, IPv4-mapped IPv6, and NAT64-embedded private ranges. Pin each request client to the addresses that passed validation, and re-run policy plus resolution for every redirect so validation and connection cannot observe different DNS answers.
- `INVIDIOUS_ALLOW_PRIVATE_NETWORK` remains an operator-configured exact-host-and-port override for `INVIDIOUS_BASE_URL`; HTTPS and credential-free are still required, and an administrator deny rule takes precedence. Do not widen the environment override to another provider or reuse it for a user-supplied destination.
- Preserve bounded redirects, connection/request timeouts, response-size limits, and per-provider failure isolation.
- RSS subscriptions refresh in a background worker every 30 minutes. Scheduling reads
  `rss_subscriptions.last_attempted_at`, which is stamped when a refresh is claimed, so a failing
  source backs off for a full window. Keep manual refresh available alongside it.
- Every successful RSS refresh advances `rss_subscriptions.refresh_generation` and stamps returned
  entries with that generation. Inbox may retain older generations, Current shows only the latest
  successful generation, and a failed refresh must never advance or blank that snapshot. Current
  entries remain retention-exempt while the source still exposes them. Dashboard RSS widgets select
  account-owned subscriptions and read this cached Current projection; they must never fetch raw feed
  URLs on demand.
- New RSS subscriptions default to auto-deleting read and unread items after seven days; Read Later
  items stay exempt. Generated Reddit subscriptions use the public Atom `.rss` listing, and the
  fetcher normalizes legacy `.json` listing URLs to Atom because Reddit rejects anonymous server-side
  JSON requests. Preserve separate article and comments destinations when a feed exposes both; a
  Reddit thread URL is the comments destination even when it is also the entry's canonical URL. Do
  not restore direct anonymous JSON fetching.
- YouTube channel metadata refreshes every two hours through configured Invidious first and the public YouTube feed second. Shared portrait images are stored in SQLite and refreshed at most every 24 hours; failed portrait responses must never populate the cache.
- YouTube Downloads is account-private even for administrators. The browser submits only a credential-free public YouTube URL plus closed media/format/height enums; it never controls yt-dlp options, paths, filenames, headers, cookies, proxies, plugins, runtimes, or post-processors. Every extractor and media connection must traverse the loopback `youtube` network-policy proxy and use its validated pinned addresses. Keep configs, plugins, self-update, remote EJS components, playlists, live streams, credentials, and external downloaders disabled.
- Download outputs live under `PANDAN_DOWNLOAD_DIR` in `.partial/<job-id>` and `files/<user-id>/<opaque-id>.<ext>`. Only authenticated, account-scoped handlers may serve a completed row: the file route downloads it as an attachment and the preview route serves it inline with Range support for the shell audio player and Downloads video dialog. Never mount the root statically or cache its `/api` responses. Cancellation, timeouts, output limits, account/content deletion, and quota failure stop the entire yt-dlp/FFmpeg process group and remove partial output. Startup reconciliation resets abandoned leases, removes partial/orphan files, and invalidates missing completed files.
- Download storage uses conservative in-flight reservations against administrator-managed instance, account, and output budgets. Existing completed media is never evicted automatically. Administrators may set policy and see tool versions but must never list, read, download, cancel, retry, or delete another account's jobs.
- The YouTube channel directory always shows every subscription in a fixed left column, independent of the active category filter. Adding a channel may assign it to one or more existing categories in the same transaction, and drag-reordered category positions must persist per account.
- Render user Markdown through the existing sanitizer. Custom HTML and iframe widgets remain sandboxed.
- Lines public posts are readable only by authenticated instance users. Private posts remain owner-only, including from administrators; administrators may force-delete public posts but must never gain private-post read access.
- Podcast feeds and enclosures use their own HTTP clients, not the shared widget client: enclosures redirect through tracking prefixes and run to hundreds of megabytes. Follow redirects manually, re-running the network policy and DNS pinning on every hop, cap redirects, and enforce the size ceiling both on `Content-Length` and while streaming.
- Podcast downloads are written to a partial file inside the media root and renamed into place only once complete and flushed. Reconcile the media root against `podcast_downloads` on startup so an interrupted transfer is never mistaken for a playable episode.
- Lines author avatars are served from `/api/lines/authors/{user_id}/avatar` and follow post visibility: an avatar is readable only when that author has at least one post the viewer can already see. Do not widen it into a general user-avatar lookup.
- `/api/lines/posts/{post_id}/thread` and `/api/lines/authors/{user_id}` apply the same visibility rule as the timeline. A thread only exposes a parent or reply the viewer may already read, and an author profile resolves only for authors with at least one post visible to the viewer.

### Bundled data

- `data/english-revised-version.json` is the packaged source for the daily Bible Verse widget. Keep verse selection local and deterministic; do not replace it with a runtime network request.

## Frontend conventions

- Follow `DESIGN.md` for visual and interaction decisions. Its control rules apply across every feature.
- Keep installed-app chrome clear of `safe-area-inset-*` on notched phones and tablets. Fixed
  players, toasts, sidebars, headers, modal action rails, and full-height dialogs must remain inside
  those insets in both portrait and landscape.
- Use Svelte 5 patterns and run the Svelte autofixer after modifying a `.svelte` file.
- Use the official `@dnd-kit/svelte` adapter and `@dnd-kit/helpers` for Kanban card sorting; do not replace it with native HTML drag events or `svelte-dnd-action`.
- Use Lucide Svelte for interface icons; do not introduce emoji controls.
- Build standard actions from `ui-button` plus exactly one role class: `ui-button--primary`, `ui-button--secondary`, `ui-button--ghost`, or `ui-button--danger`. Add `ui-button--icon` only for icon-only controls.
- Use GridStack for dashboard placement and resizing. Layout is editable only in Edit mode.
- Preserve keyboard access, visible focus states, minimum touch targets, reduced-motion behavior, and mobile reflow.
- Keep the terminal visual language: near-black surfaces, restrained green accents, monospaced content, crisp borders, and translucent structure over wallpaper.
- Reuse existing CSS tokens and components before adding new styles. Avoid isolated hardcoded colors.
- A refresh must never blank what is already on screen. Only a first load may show a loading state; a later one keeps the current records up until the new set lands, and a failure reports itself beside them rather than replacing them. Track "has ever loaded" with a plain variable, not `$state`, so the loader does not depend on its own completion.
- Product pages share one terminal header: `page-header` alongside the page's own class, a `$ {page} --{view}` title rendered by `$lib/TypedHeading.svelte`, and a muted mono standfirst ruled off from the body. The heading element belongs to the component, so style it through `.typed-heading` in `ui/src/app.css`; a page-scoped `h2` rule cannot reach it.
- `TypedHeading` keeps the exact visible title in module scope behind a newest-instance ownership token. Page components may be created before the outgoing one is destroyed, so only the newest owner may update the handoff; this lets the incoming title backspace what the reader actually saw without a stale teardown reviving older text. The first hydrated heading starts empty and types once; reduced motion resolves it immediately.
- Scrollbars are a shared surface, not a per-component one. Reuse the custom scrollbar in `ui/src/app.css` and give every layout-level scroll container `scrollbar-gutter: stable`; see `DESIGN.md`.
- Keep modals centered, consistently structured, dismissible by their close control, and animated from the top with reduced-motion fallbacks.
- Native `dialog` elements enter the browser top layer: never leave them on browser-default light colors or corner positioning. Use the shared modal class or explicitly bind the authenticated terminal tokens, plus `position: fixed`, `inset: 0`, and `margin: auto`; verify the open dialog is centered and dark in the rendered application.

## Backend and database conventions

- Rust workspace edition is 2024 with MSRV 1.85.
- Keep reusable SQL in `crates/db/src/queries.rs` and API orchestration in `crates/server/src/lib.rs` or a focused server module.
- Use `thiserror` for typed server error boundaries, `miette` for fatal startup diagnostics,
  and `tracing` for operator-facing logs. Keep internal causes out of HTTP response bodies.
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

Server-side AVIF decoding for Walls links against system libdav1d 1.3.0 or newer, which is why the container images are built on Debian trixie (`libdav1d-dev` in the builder, `libdav1d7` at runtime). A change touching image handling or the base images should be verified with `docker build .`, not only `cargo test`.

Use `just ci` for the complete local gate. Update `README.md`, `SCHEMA.md`, and this guide when user-visible behavior, schema contracts, or contributor invariants change.
