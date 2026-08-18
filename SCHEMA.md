# Database schema

Pandan uses SQLite through SQLx. Connections use WAL mode, a five-second busy timeout, and an
eight-connection pool. Numbered SQL migrations are embedded into the `db` crate and applied on
startup.

## `_migrations`

Internal migration ledger created by `crates/db/src/lib.rs`.

Migration `028_youtube_channel_thumbnail_repair` safely completes the channel portrait columns for
databases that recorded an earlier partial development build of migration `027`. Migration
`029_yearless_contact_birthdays` restores Monica birthdays whose year is unknown from the legacy
generated note marker into the standard `--MM-DD` representation.

| Column       | Type | Constraints                  |
| ------------ | ---- | ---------------------------- |
| `name`       | TEXT | Primary key                  |
| `applied_at` | TEXT | Required, RFC 3339 timestamp |

## `app_metadata`

Small infrastructure table for application-level key/value metadata. Domain tables should be
introduced in later numbered migrations.

| Column       | Type | Constraints                  |
| ------------ | ---- | ---------------------------- |
| `key`        | TEXT | Primary key                  |
| `value`      | TEXT | Required                     |
| `updated_at` | TEXT | Required, RFC 3339 timestamp |

## `authentication_settings`

Singleton administrator-controlled authentication policy. All switches default to enabled for
compatibility with existing installations. Password login remains available as a runtime safety
fallback when OIDC is not configured, and new OIDC identities are rejected when OIDC registration
is disabled while known identities and verified-email links to existing users remain eligible.

| Column                          | Type    | Constraints                          |
| ------------------------------- | ------- | ------------------------------------ |
| `id`                            | INTEGER | Primary key; singleton value `1`     |
| `password_login_enabled`        | INTEGER | Boolean `0` or `1`; defaults to `1`  |
| `password_registration_enabled` | INTEGER | Boolean `0` or `1`; defaults to `1`  |
| `oidc_registration_enabled`     | INTEGER | Boolean `0` or `1`; defaults to `1`  |
| `updated_at`                    | TEXT    | Required, RFC 3339 timestamp         |

## `users`

Private dashboard accounts. Passwords are stored only as Argon2id PHC strings.

| Column          | Type | Constraints                                       |
| --------------- | ---- | ------------------------------------------------- |
| `id`            | TEXT | Primary key                                       |
| `email`         | TEXT | Required, case-insensitive unique value           |
| `password_hash` | TEXT | Required Argon2id hash                            |
| `role`          | TEXT | `administrator` or `member`; defaults to `member` |
| `created_at`    | TEXT | Required, RFC 3339 timestamp                      |

The first-run setup transaction creates the initial user with the `administrator` role and writes
`app_metadata.onboarding_complete`. Setup may use a password or a verified OIDC identity; for OIDC,
the matching `oidc_identities` row is committed in the same transaction. The metadata key is a
one-time database claim: setup can succeed only when both the claim and all users are absent.
Existing installations promote their earliest account to administrator when migration
`005_onboarding` is first applied.

Administrators can list all accounts, promote or demote other users, and remove other accounts.
These operations are authorized on the server. A user cannot mutate their own administrator role
or delete their own active account, and conditional writes ensure at least one administrator
always remains. Removing an account cascades to its settings, sessions, tasks, and OIDC identities.

## `user_settings`

One preference record per user.

| Column                    | Type | Constraints                                         |
| ------------------------- | ---- | --------------------------------------------------- |
| `user_id`                 | TEXT | Primary key, references `users` with cascade delete |
| `display_name`            | TEXT | Required, trimmed length 1–60                       |
| `location`                | TEXT | Required, trimmed length 1–80                       |
| `timezone`                | TEXT | Required, trimmed length 1–80                       |
| `sidebar_timezones_json`  | TEXT | Valid JSON array containing 1–5 timezone names      |
| `temperature_unit`        | TEXT | `celsius` or `fahrenheit`                           |
| `updated_at`              | TEXT | Required, RFC 3339 timestamp                        |

## `user_backgrounds`

Legacy workspace background images retained for migration and data preservation. Workspace `0`
images are copied into the `dashboard` slot in `user_wallpapers`; the current API no longer writes
this table, and nonzero rows from earlier releases remain untouched.

| Column       | Type    | Constraints                                                   |
| ------------ | ------- | ------------------------------------------------------------- |
| `user_id`    | TEXT    | Composite primary key, references `users` with cascade delete |
| `workspace`  | INTEGER | Composite primary key, stable workspace identifier 0–31       |
| `mime_type`  | TEXT    | JPEG, PNG, WebP, or AVIF                                      |
| `image_data` | BLOB    | Required, 1 byte to 30 MB                                     |
| `updated_at` | TEXT    | Required, RFC 3339 timestamp                                  |

## `user_wallpapers`

Named wallpaper surfaces for the authenticated application, session welcome, authenticated
loading screen, and login screen. The `welcome` image is used both by `welcome:{user}` and as
the persistent background behind authenticated pages. The `dashboard` slot is retained for
legacy data compatibility but is no longer exposed as a separate selector. Dashboard, welcome,
and loading images are user-owned and require the owner's authenticated session. The `login`
slot is a single global image: only an administrator may replace or remove it, while the image
itself is publicly retrievable from the application origin for the pre-authentication screen.

| Column       | Type | Constraints                                                          |
| ------------ | ---- | -------------------------------------------------------------------- |
| `user_id`    | TEXT | Composite primary key, references `users` with cascade delete        |
| `slot`       | TEXT | Composite primary key; `dashboard`, `welcome`, `loading`, or `login` |
| `mime_type`  | TEXT | JPEG, PNG, WebP, or AVIF                                             |
| `image_data` | BLOB | Required, 1 byte to 30 MB                                            |
| `updated_at` | TEXT | Required, RFC 3339 timestamp                                         |

## `user_avatars`

Optional private profile images. Avatar bytes are available only through the authenticated user's
settings endpoint and are deleted automatically with the owning account.

| Column       | Type | Constraints                                         |
| ------------ | ---- | --------------------------------------------------- |
| `user_id`    | TEXT | Primary key, references `users` with cascade delete |
| `mime_type`  | TEXT | JPEG, PNG, WebP, or AVIF                            |
| `image_data` | BLOB | Required, 1 byte to 10 MB                           |
| `updated_at` | TEXT | Required, RFC 3339 timestamp                        |

## `user_workspaces`

Legacy dashboard partition metadata retained so existing widget and background foreign keys remain
valid. New accounts receive only workspace `0`, named `Dashboard`. Workspace management is not
exposed by the current API or interface, and existing nonzero partitions are preserved rather than
deleted during the upgrade.

| Column       | Type    | Constraints                                                   |
| ------------ | ------- | ------------------------------------------------------------- |
| `user_id`    | TEXT    | Composite primary key, references `users` with cascade delete |
| `workspace`  | INTEGER | Composite primary key, stable identifier 0–31                 |
| `name`       | TEXT    | Required, trimmed length 1–40                                 |
| `position`   | INTEGER | Per-user unique navigation order, 0–31                        |
| `created_at` | TEXT    | Required, RFC 3339 timestamp                                  |
| `updated_at` | TEXT    | Required, RFC 3339 timestamp                                  |

## `user_appearance`

One dashboard appearance record per user. A trigger creates the default row with each account.
Values are intentionally bounded so the wallpaper remains usable behind terminal surfaces.

| Column                  | Type    | Constraints                                         |
| ----------------------- | ------- | --------------------------------------------------- |
| `user_id`               | TEXT    | Primary key, references `users` with cascade delete |
| `background_blur`       | INTEGER | Blur radius from 0–24 pixels                        |
| `background_brightness` | INTEGER | Brightness percentage from 40–140                   |
| `background_contrast`   | INTEGER | Contrast percentage from 50–160                     |
| `background_saturation` | INTEGER | Saturation percentage from 0–180                    |
| `updated_at`            | TEXT    | Required, RFC 3339 timestamp                        |

## `sessions`

Opaque, revocable browser sessions. The cookie stores only the token; ownership and expiry live
server-side.

| Column       | Type | Constraints                                      |
| ------------ | ---- | ------------------------------------------------ |
| `token`      | TEXT | Primary key                                      |
| `user_id`    | TEXT | Required, references `users` with cascade delete |
| `expires_at` | TEXT | Required, RFC 3339 timestamp                     |
| `created_at` | TEXT | Required, RFC 3339 timestamp                     |

## `oidc_identities`

Stable provider identities linked to personal dashboard accounts. Verified email is used only for
the initial link; later sign-ins resolve by the provider's issuer and subject pair.

| Column       | Type | Constraints                                      |
| ------------ | ---- | ------------------------------------------------ |
| `issuer`     | TEXT | Composite primary key with `subject`             |
| `subject`    | TEXT | Composite primary key with `issuer`              |
| `user_id`    | TEXT | Required, references `users` with cascade delete |
| `created_at` | TEXT | Required, RFC 3339 timestamp                     |

## `oidc_authorizations`

Short-lived, single-use authorization state consumed atomically by the callback.

| Column          | Type | Constraints                                      |
| --------------- | ---- | ------------------------------------------------ |
| `state`         | TEXT | Primary key, cryptographically random CSRF value |
| `pkce_verifier` | TEXT | Required, secret verifier for code exchange      |
| `nonce`         | TEXT | Required, secret ID-token replay defense         |
| `expires_at`    | TEXT | Required, RFC 3339 timestamp                     |
| `created_at`    | TEXT | Required, RFC 3339 timestamp                     |

## `tasks`

User-owned tasks. The active list excludes rows with `archived_at` set, then orders incomplete
tasks before completed tasks and by creation time. Completing a recurring task keeps it active and
advances its due date according to its recurrence and rescheduling preference. Archived tasks are
listed separately newest first; restoring one clears `archived_at` without changing its children.

| Column            | Type    | Constraints                                                   |
| ----------------- | ------- | ------------------------------------------------------------- |
| `id`              | TEXT    | Primary key                                                   |
| `user_id`         | TEXT    | Required, references `users` with cascade delete              |
| `title`           | TEXT    | Required, trimmed length 1–180                                |
| `description`     | TEXT    | Required, up to 4,000 characters                              |
| `completed`       | INTEGER | Required boolean, defaults to `0`                             |
| `priority`        | TEXT    | `p1`, `p2`, `p3`, `p4`, or `none`                             |
| `due_date`        | TEXT    | Optional ISO calendar date                                    |
| `repeat_rule`     | TEXT    | `none`, `daily`, `weekly`, `monthly`, `yearly`, or `custom`   |
| `repeat_interval` | INTEGER | Required, 1–365                                               |
| `repeat_unit`     | TEXT    | `days`, `weeks`, `months`, or `years`                         |
| `reschedule_from` | TEXT    | `due_date` or `completion_date`                               |
| `completed_at`    | TEXT    | Optional RFC 3339 timestamp                                   |
| `archived_at`     | TEXT    | Optional RFC 3339 timestamp; hides the task from active lists |
| `created_at`      | TEXT    | Required, RFC 3339 timestamp                                  |
| `updated_at`      | TEXT    | Required, RFC 3339 timestamp                                  |

## `task_labels`

Ordered, deduplicated labels for a task. The `(task_id, label)` pair is the primary key and task
deletion cascades to these rows.

## `task_subtasks`

Ordered checklist entries owned through their parent task. Each subtask has its own generated
identifier, completion state, position, and creation/update timestamps.

## `task_attachments`

Private task files stored in SQLite. Metadata is returned with task responses; bytes are available
only from the authenticated attachment endpoint after parent-task ownership is verified.

| Column       | Type    | Constraints                                      |
| ------------ | ------- | ------------------------------------------------ |
| `id`         | TEXT    | Primary key                                      |
| `task_id`    | TEXT    | Required, references `tasks` with cascade delete |
| `file_name`  | TEXT    | Required, trimmed length 1–255                   |
| `mime_type`  | TEXT    | Required                                         |
| `byte_size`  | INTEGER | Positive integer; API limit is 10 MB             |
| `file_data`  | BLOB    | Required                                         |
| `created_at` | TEXT    | Required, RFC 3339 timestamp                     |

## `feed_items`

Curated feed entries shown in the dashboard's Feeds workspace.

| Column            | Type    | Constraints                          |
| ----------------- | ------- | ------------------------------------ |
| `id`              | TEXT    | Primary key                          |
| `category`        | TEXT    | `Design`, `Technology`, or `Culture` |
| `source`          | TEXT    | Required                             |
| `title`           | TEXT    | Required                             |
| `summary`         | TEXT    | Required                             |
| `reading_minutes` | INTEGER | Positive integer                     |
| `published_at`    | TEXT    | Required, RFC 3339 timestamp         |

## `rss_subscriptions`

User-owned RSS or Atom sources for the dedicated reader. URLs are unique per user. Fetching is
restricted to public HTTPS destinations by the server; a normalized origin is stored separately
to make base-URL filtering predictable.

| Column             | Type    | Constraints                            |
| ------------------ | ------- | -------------------------------------- |
| `id`               | TEXT    | Primary key                            |
| `user_id`          | TEXT    | References `users` with cascade delete |
| `url`              | TEXT    | Required source URL, unique per user   |
| `base_url`         | TEXT    | Required normalized HTTPS origin       |
| `title`            | TEXT    | Required, fetched feed title           |
| `category`         | TEXT    | Required, trimmed length 1–40          |
| `auto_delete_days` | INTEGER | Optional age from 1–3,650 days         |
| `auto_delete_mode` | TEXT    | `read` or `all`                        |
| `last_fetched_at`  | TEXT    | Optional RFC 3339 timestamp            |
| `last_error`       | TEXT    | Optional safe provider error           |
| `created_at`       | TEXT    | Required, RFC 3339 timestamp           |
| `updated_at`       | TEXT    | Required, RFC 3339 timestamp           |

## `rss_items`

Fetched reader entries owned through their subscription. Refresh upserts by the source's stable
identifier and preserves `read_at`. Automatic retention runs when the reader loads or a source
refreshes; manual pruning can remove old read-only or all entries across one user's subscriptions.

| Column            | Type | Constraints                                         |
| ----------------- | ---- | --------------------------------------------------- |
| `id`              | TEXT | Primary key                                         |
| `subscription_id` | TEXT | References `rss_subscriptions` with cascade delete  |
| `external_id`     | TEXT | Required, unique within the subscription            |
| `url`             | TEXT | Entry destination, empty when omitted by the feed   |
| `title`           | TEXT | Required, trimmed length 1–500                      |
| `summary`         | TEXT | Required, defaults to an empty string               |
| `published_at`    | TEXT | RFC 3339; fetch time is used when the feed omits it |
| `fetched_at`      | TEXT | Required, RFC 3339 timestamp                        |
| `read_at`         | TEXT | Optional RFC 3339 timestamp                         |

## `youtube_channels`

Globally shared YouTube channel cache keyed by the public 24-character Channel ID. Every upstream
attempt updates `last_fetched_at`, including failures, so all users share the same two-hour fetch
window. `refresh_started_at` is a short atomic lease that prevents concurrent requests from
fetching the same channel twice. A configured Invidious instance is attempted first, with the
YouTube uploads feed as fallback. Channel portraits are stored in SQLite and refreshed at most once per 24 hours; missing or failed portraits remain eligible for retry and never populate the cache.

| Column                   | Type | Constraints                                              |
| ------------------------ | ---- | -------------------------------------------------------- |
| `channel_id`             | TEXT | Primary key, 24 characters beginning with `UC`           |
| `title`                  | TEXT | Required; Channel ID until the first fetch               |
| `channel_url`            | TEXT | Required YouTube channel destination                     |
| `thumbnail_url`          | TEXT | Required portrait source URL; empty when unavailable     |
| `thumbnail_fetched_at`   | TEXT | Optional RFC 3339 timestamp for the 24-hour portrait TTL |
| `thumbnail_content_type` | TEXT | Required cached portrait MIME type; empty when absent    |
| `thumbnail_data`         | BLOB | Optional cached portrait bytes                           |
| `last_fetched_at`        | TEXT | Optional RFC 3339 timestamp of the latest attempt        |
| `refresh_started_at`     | TEXT | Optional RFC 3339 refresh lease timestamp                |
| `last_error`             | TEXT | Optional safe provider error                             |
| `created_at`             | TEXT | Required, RFC 3339 timestamp                             |
| `updated_at`             | TEXT | Required, RFC 3339 timestamp                             |

## `youtube_videos`

Fetched uploads stored once for the entire installation. YouTube video IDs are globally unique,
so repeated feeds and overlapping user subscriptions update the existing row instead of copying it.

| Column          | Type | Constraints                                              |
| --------------- | ---- | -------------------------------------------------------- |
| `id`            | TEXT | Primary key                                              |
| `external_id`   | TEXT | Required, globally unique YouTube video ID               |
| `channel_id`    | TEXT | References `youtube_channels` with cascade delete        |
| `url`           | TEXT | Required video destination                               |
| `thumbnail_url` | TEXT | Required; may be empty when YouTube omits media metadata |
| `title`         | TEXT | Required                                                 |
| `published_at`  | TEXT | Required, RFC 3339 timestamp                             |
| `fetched_at`    | TEXT | Required, RFC 3339 timestamp                             |

## `youtube_subscriptions`

User-to-channel join table. Removing a subscription does not delete the shared channel or its
stored videos, because another user may still subscribe to it.

| Column       | Type | Constraints                                                   |
| ------------ | ---- | ------------------------------------------------------------- |
| `user_id`    | TEXT | Composite primary key, references `users` with cascade delete |
| `channel_id` | TEXT | Composite primary key, references `youtube_channels`          |
| `created_at` | TEXT | Required, RFC 3339 timestamp                                  |

## `youtube_groups`

Ordered, user-owned channel collections such as Gaming or Japan. Group names are unique per user
without regard to case.

| Column       | Type    | Constraints                                      |
| ------------ | ------- | ------------------------------------------------ |
| `id`         | TEXT    | Primary key                                      |
| `user_id`    | TEXT    | References `users` with cascade delete           |
| `name`       | TEXT    | Required, case-insensitive unique value per user |
| `position`   | INTEGER | Unique per-user order, 0–127                     |
| `created_at` | TEXT    | Required, RFC 3339 timestamp                     |
| `updated_at` | TEXT    | Required, RFC 3339 timestamp                     |

## `youtube_group_channels`

Many-to-many ordered membership table. The same channel can appear in any number of one user's
groups.

| Column       | Type    | Constraints                                          |
| ------------ | ------- | ---------------------------------------------------- |
| `group_id`   | TEXT    | Composite primary key, references `youtube_groups`   |
| `channel_id` | TEXT    | Composite primary key, references `youtube_channels` |
| `position`   | INTEGER | Unique order within a group, 0–127                   |

## `youtube_settings`

One optional preference row per user. Missing rows default to the thumbnail display.

| Column         | Type | Constraints                                         |
| -------------- | ---- | --------------------------------------------------- |
| `user_id`      | TEXT | Primary key, references `users` with cascade delete |
| `display_mode` | TEXT | `thumbnails` or `compact`                           |
| `updated_at`   | TEXT | Required, RFC 3339 timestamp                        |

## `journal_nodes`

User-owned recursive file tree for the Journal explorer. Every node stores extended Markdown and
may also be the parent of any number of subfiles. Root names and sibling names are unique per user
without regard to case. Deleting a file cascades through every descendant; the API rejects moves
that would create a cycle.

| Column       | Type    | Constraints                                     |
| ------------ | ------- | ----------------------------------------------- |
| `id`         | TEXT    | Primary key                                     |
| `user_id`    | TEXT    | References `users` with cascade delete          |
| `parent_id`  | TEXT    | Optional self-reference with cascade delete     |
| `name`       | TEXT    | Required, trimmed length 1–120                  |
| `content`    | TEXT    | Extended Markdown up to 1,000,000 characters    |
| `position`   | INTEGER | Zero-based order among sibling files, 0–100,000 |
| `created_at` | TEXT    | Required, RFC 3339 timestamp                    |
| `updated_at` | TEXT    | Required, RFC 3339 timestamp                    |

## `calendar_subscriptions`

User-owned public HTTPS iCalendar sources. URLs are unique within an account. Refresh errors are
stored without discarding the most recent successful event snapshot.

| Column            | Type | Constraints                                            |
| ----------------- | ---- | ------------------------------------------------------ |
| `id`              | TEXT | Primary key                                            |
| `user_id`         | TEXT | References `users` with cascade delete                 |
| `url`             | TEXT | Required source URL, unique per user                   |
| `name`            | TEXT | Required fetched calendar name, length 1–120           |
| `color`           | TEXT | Legacy preset key retained for migration compatibility |
| `color_value`     | TEXT | Required six-digit sRGB hex color                      |
| `last_fetched_at` | TEXT | Optional RFC 3339 timestamp                            |
| `last_error`      | TEXT | Optional safe provider error                           |
| `created_at`      | TEXT | Required, RFC 3339 timestamp                           |
| `updated_at`      | TEXT | Required, RFC 3339 timestamp                           |

## `calendar_events`

Bounded event occurrences owned through a calendar subscription. Refresh replaces one source's
snapshot atomically, which removes cancelled upstream occurrences without touching other sources.

| Column            | Type    | Constraints                                             |
| ----------------- | ------- | ------------------------------------------------------- |
| `id`              | TEXT    | Primary key                                             |
| `subscription_id` | TEXT    | References `calendar_subscriptions` with cascade delete |
| `external_id`     | TEXT    | Required source UID                                     |
| `title`           | TEXT    | Required, trimmed length 1–500                          |
| `description`     | TEXT    | Required, defaults to an empty string                   |
| `location`        | TEXT    | Required, defaults to an empty string                   |
| `url`             | TEXT    | Event destination, empty when omitted                   |
| `start_at`        | TEXT    | ISO date for all-day events or RFC 3339 date-time       |
| `end_at`          | TEXT    | Optional matching date or date-time                     |
| `all_day`         | INTEGER | Required boolean                                        |
| `fetched_at`      | TEXT    | Required, RFC 3339 timestamp                            |

The `(subscription_id, external_id, start_at)` tuple is unique so recurring occurrences sharing
one RFC 5545 UID remain distinct.

## `contact_dav_sources`

Account-owned CardDAV address-book resources. The URL is a direct public HTTPS address-book
collection and is validated against private and reserved destinations before storage and sync.
Passwords are optional and use the same XChaCha20-Poly1305 secret key as provider credentials;
API responses expose only `has_password`. Sync is an explicit pull operation and stores only a
user-safe error string when the upstream resource fails.

| Column                | Type | Constraints                                            |
| --------------------- | ---- | ------------------------------------------------------ |
| `id`                  | TEXT | Primary key                                            |
| `user_id`             | TEXT | References `users` with cascade delete                 |
| `name`                | TEXT | Required, trimmed length 1–80                          |
| `url`                 | TEXT | Required, unique per user, length 8–2,048              |
| `username`            | TEXT | Optional DAV username, up to 320 characters            |
| `password_ciphertext` | TEXT | Optional encrypted password, never returned by the API |
| `last_synced_at`      | TEXT | Optional RFC 3339 timestamp                            |
| `last_error`          | TEXT | Optional safe provider error                           |
| `created_at`          | TEXT | Required, RFC 3339 timestamp                           |
| `updated_at`          | TEXT | Required, RFC 3339 timestamp                           |

## `contacts`

Private personal relationship records. A contact belongs to exactly one account and can originate
in Pandan, a Monica JSON import, or a CardDAV resource. Imported records use the
`(user_id, source_kind, source_reference)` identity to update a prior import rather than creating a
duplicate. Deleting a DAV source detaches its contacts but preserves the local records.

| Column                 | Type    | Constraints                                                  |
| ---------------------- | ------- | ------------------------------------------------------------ |
| `id`                   | TEXT    | Primary key                                                  |
| `user_id`              | TEXT    | References `users` with cascade delete                       |
| `dav_source_id`        | TEXT    | Optional `contact_dav_sources` reference with null-on-delete |
| `source_kind`          | TEXT    | `manual`, `monica`, or `carddav`                             |
| `source_reference`     | TEXT    | Optional stable upstream identifier                          |
| `first_name`           | TEXT    | Up to 120 characters                                         |
| `middle_name`          | TEXT    | Up to 120 characters                                         |
| `last_name`            | TEXT    | Up to 120 characters                                         |
| `nickname`             | TEXT    | Up to 120 characters; one name field must be non-empty       |
| `pronouns`             | TEXT    | Up to 80 characters                                          |
| `company`              | TEXT    | Up to 160 characters                                         |
| `job_title`            | TEXT    | Up to 160 characters                                         |
| `birthday`             | TEXT    | Optional `YYYY-MM-DD` date or `--MM-DD` when year is unknown |
| `emails_json`          | TEXT    | Valid JSON array of labeled contact methods                  |
| `phones_json`          | TEXT    | Valid JSON array of labeled contact methods                  |
| `addresses_json`       | TEXT    | Valid JSON array of labeled structured addresses             |
| `important_dates_json` | TEXT    | Valid JSON array of labeled dates and recurrence flags       |
| `tags_json`            | TEXT    | Valid JSON array of display tags                             |
| `relationship_context` | TEXT    | Private relationship context, up to 4,000 characters         |
| `notes`                | TEXT    | Private contact notes, up to 20,000 characters               |
| `favorite`             | INTEGER | Required boolean                                             |
| `archived`             | INTEGER | Required boolean                                             |
| `created_at`           | TEXT    | Required, RFC 3339 timestamp                                 |
| `updated_at`           | TEXT    | Required, RFC 3339 timestamp                                 |

## `contact_photos`

Private binary portraits imported from Monica JSON, CardDAV vCards, or Pandan exports. The API
serves a photo only when both the contact and photo belong to the authenticated account. Re-importing
an upstream contact replaces its photo; deleting the contact cascades to the photo.

| Column       | Type | Constraints                                                             |
| ------------ | ---- | ----------------------------------------------------------------------- |
| `contact_id` | TEXT | Primary key, references contacts with cascade delete                    |
| `user_id`    | TEXT | References users with cascade delete; used for explicit account scoping |
| `mime_type`  | TEXT | image/jpeg, image/png, image/webp, or image/avif                        |
| `image_data` | BLOB | Required validated image bytes, 1 byte through 10 MiB                   |
| `updated_at` | TEXT | Required, RFC 3339 timestamp                                            |

## `payment_subscriptions`

User-owned regular payment records shown in Subscriptions mode.

| Column          | Type    | Constraints                                                                             |
| --------------- | ------- | --------------------------------------------------------------------------------------- |
| `id`            | TEXT    | Primary key                                                                             |
| `user_id`       | TEXT    | References `users` with cascade delete                                                  |
| `service`       | TEXT    | Required, trimmed length 1–120                                                          |
| `description`   | TEXT    | Required, up to 2,000 characters                                                        |
| `frequency`     | TEXT    | Required, trimmed length 1–40                                                           |
| `amount_micros` | INTEGER | Exact charge per billing period in millionths of one currency unit; 0–1,000,000,000,000 |
| `currency`      | TEXT    | Required uppercase three-letter currency code; defaults to `USD` for migrated records   |
| `first_paid_on` | TEXT    | Required ISO calendar date                                                              |
| `created_at`    | TEXT    | Required, RFC 3339 timestamp                                                            |
| `updated_at`    | TEXT    | Required, RFC 3339 timestamp                                                            |

## `coding_projects`

User-owned software release subscriptions. Provider, host, and repository form a unique tuple per
account. Public projects can be refreshed anonymously; `has_credential` is computed at query time
without joining encrypted token material into the API model.

| Column       | Type | Constraints                                           |
| ------------ | ---- | ----------------------------------------------------- |
| `id`         | TEXT | Primary key                                           |
| `user_id`    | TEXT | References `users` with cascade delete                |
| `provider`   | TEXT | `github`, `gitlab`, `codeberg`, `gitea`, or `forgejo` |
| `host`       | TEXT | Required normalized provider host, length 1–253       |
| `repository` | TEXT | Required `owner/name` path, length 3–240              |
| `created_at` | TEXT | Required, RFC 3339 timestamp                          |
| `updated_at` | TEXT | Required, RFC 3339 timestamp                          |

## `coding_credentials`

One encrypted access token per user and code host. Tokens are loaded only by the server-side
provider client. GitLab credentials also authorize the signed-in profile's open merge-request
query and the latest-pipeline query for that user's subscribed GitLab projects.

| Column       | Type | Constraints                                                   |
| ------------ | ---- | ------------------------------------------------------------- |
| `user_id`    | TEXT | Composite primary key, references `users` with cascade delete |
| `provider`   | TEXT | Composite primary key, one of the supported code providers    |
| `host`       | TEXT | Composite primary key, normalized provider host               |
| `ciphertext` | TEXT | XChaCha20-Poly1305 nonce and ciphertext, base64 encoded       |
| `updated_at` | TEXT | Required, RFC 3339 timestamp                                  |

## `dashboard_widgets`

Per-user widget instances and their persisted GridStack layout. The interface normalizes reading
order from the twelve-column coordinates after a move or resize. Layout updates are written
atomically so a resize or reorder cannot be only partially saved.

| Column        | Type    | Constraints                                                         |
| ------------- | ------- | ------------------------------------------------------------------- |
| `id`          | TEXT    | Primary key                                                         |
| `user_id`     | TEXT    | Required, references `users` with cascade delete                    |
| `kind`        | TEXT    | Supported widget type, including the local `bible-verse` widget     |
| `workspace`   | INTEGER | Legacy partition identifier; new widgets use dashboard `0`          |
| `position`    | INTEGER | Zero-based dashboard reading order, bounded to 0–127                |
| `size`        | TEXT    | `compact`, `standard`, `wide`, or `full`                            |
| `grid_x`      | INTEGER | GridStack column offset, 0–11                                       |
| `grid_y`      | INTEGER | GridStack row offset, 0–255                                         |
| `grid_w`      | INTEGER | GridStack width, 1–12 columns                                       |
| `grid_h`      | INTEGER | GridStack height, 1–12 rows                                         |
| `config_json` | TEXT    | Valid JSON object containing non-secret per-instance configuration  |
| `created_at`  | TEXT    | Required, RFC 3339 timestamp                                        |
| `updated_at`  | TEXT    | Required, RFC 3339 timestamp                                        |

## `widget_secrets`

Encrypted credentials for provider-backed widgets. This table is intentionally queried separately
from `dashboard_widgets` so credentials cannot be serialized into dashboard responses.

| Column       | Type | Constraints                                                     |
| ------------ | ---- | --------------------------------------------------------------- |
| `widget_id`  | TEXT | Primary key, references `dashboard_widgets` with cascade delete |
| `user_id`    | TEXT | Required, references `users` with cascade delete                |
| `ciphertext` | TEXT | XChaCha20-Poly1305 nonce and ciphertext, base64 encoded         |
| `updated_at` | TEXT | Required, RFC 3339 timestamp                                    |
