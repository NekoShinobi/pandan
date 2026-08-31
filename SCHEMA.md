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

| Column                          | Type    | Constraints                         |
| ------------------------------- | ------- | ----------------------------------- |
| `id`                            | INTEGER | Primary key; singleton value `1`    |
| `password_login_enabled`        | INTEGER | Boolean `0` or `1`; defaults to `1` |
| `password_registration_enabled` | INTEGER | Boolean `0` or `1`; defaults to `1` |
| `oidc_registration_enabled`     | INTEGER | Boolean `0` or `1`; defaults to `1` |
| `updated_at`                    | TEXT    | Required, RFC 3339 timestamp        |

## `logging_settings`

Singleton administrator-controlled policy for persistent structured application logs. The log
directory itself remains an operator-controlled environment path; this table controls whether file
events are written, the minimum level, size-based rotation, and both age- and count-based retention.
The active file is never removed by retention. Console tracing continues to use `RUST_LOG`.

| Column             | Type    | Constraints                                                      |
| ------------------ | ------- | ---------------------------------------------------------------- |
| `id`               | INTEGER | Primary key; singleton value `1`                                 |
| `file_enabled`     | INTEGER | Boolean `0` or `1`; defaults to `1`                              |
| `log_level`        | TEXT    | `error`, `warn`, `info`, `debug`, or `trace`; defaults to `info` |
| `retention_days`   | INTEGER | 1–365; defaults to `14`                                          |
| `max_file_size_mb` | INTEGER | 1–256 MiB per active file; defaults to `10`                      |
| `max_files`        | INTEGER | 1–100 retained rotated files; defaults to `20`                   |
| `updated_at`       | TEXT    | Required, RFC 3339 timestamp                                     |

## `network_access_rules`

Administrator-managed exact-origin policy for requests made by the Pandan server. Public HTTPS is
the implicit default when no rule matches. An `allow` rule may authorize a private or HTTP origin;
a matching `deny` rule takes precedence. Rules are scoped either to all policy-controlled
integration fetches or to one integration. Browser-loaded embedded pages and external links do not
consult this table.

| Column               | Type    | Constraints                                                                                                                |
| -------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------- |
| `id`                 | TEXT    | Primary key                                                                                                                |
| `action`             | TEXT    | Required, `allow` or `deny`                                                                                                |
| `scheme`             | TEXT    | Required, `http` or `https`                                                                                                |
| `host`               | TEXT    | Required normalized hostname or IP, 1–253 characters                                                                       |
| `port`               | INTEGER | Required, 1–65,535                                                                                                         |
| `integration`        | TEXT    | `all`, `rss`, `calendar`, `contacts`, `podcasts`, `notifications`, `coding`, `images`, `youtube`, `widgets`, or `jellyfin` |
| `created_by_user_id` | TEXT    | Optional administrator audit reference, set null on account delete                                                         |
| `created_at`         | TEXT    | Required, RFC 3339 timestamp                                                                                               |
| `updated_at`         | TEXT    | Required, RFC 3339 timestamp                                                                                               |

The action, scheme, host, port, and integration tuple is unique. Instances are limited to 128 rows
by the API.

## `jellyfin_server_settings`

Administrator-selected singleton Jellyfin server. Saving a different server deletes the old
singleton first, so the foreign key on every account connection invalidates old tokens atomically.
The base URL is non-secret but is returned only by administrator APIs.

| Column                  | Type    | Constraints                                              |
| ----------------------- | ------- | -------------------------------------------------------- |
| `id`                    | INTEGER | Primary key; singleton value `1`                         |
| `base_url`              | TEXT    | Required normalized HTTP(S) base URL, 8–2,000 characters |
| `server_id`             | TEXT    | Required Jellyfin server identity, 1–128 characters      |
| `server_name`           | TEXT    | Required display name, 1–120 characters                  |
| `server_version`        | TEXT    | Required upstream version, 1–64 characters               |
| `configured_by_user_id` | TEXT    | Optional administrator reference, set null on delete     |
| `created_at`            | TEXT    | Required, RFC 3339 timestamp                             |
| `updated_at`            | TEXT    | Required, RFC 3339 timestamp                             |

## `jellyfin_user_connections`

Private one-to-one mapping from a Pandan account to its own Jellyfin identity. The access token is
encrypted with the same XChaCha20-Poly1305 credential cipher as other provider secrets and is never
returned through the browser API. Deleting an account or replacing/removing the singleton server
cascades the row.

| Column              | Type    | Constraints                                              |
| ------------------- | ------- | -------------------------------------------------------- |
| `user_id`           | TEXT    | Primary key, references `users` with cascade delete      |
| `server_setting_id` | INTEGER | Required singleton value `1`, cascades on server delete  |
| `jellyfin_user_id`  | TEXT    | Required upstream user identity, 1–128 characters        |
| `jellyfin_username` | TEXT    | Required upstream display name, 1–120 characters         |
| `token_ciphertext`  | TEXT    | Required encrypted token, 1–8,192 characters             |
| `device_id`         | TEXT    | Required stable Pandan device identity, 1–128 characters |
| `last_verified_at`  | TEXT    | Optional RFC 3339 timestamp                              |
| `last_error`        | TEXT    | Optional bounded user-safe error, at most 500 characters |
| `created_at`        | TEXT    | Required, RFC 3339 timestamp                             |
| `updated_at`        | TEXT    | Required, RFC 3339 timestamp                             |

## `users`

Private dashboard accounts. Passwords are stored only as Argon2id PHC strings.

| Column          | Type | Constraints                                       |
| --------------- | ---- | ------------------------------------------------- |
| `id`            | TEXT | Primary key                                       |
| `email`         | TEXT | Required, case-insensitive unique value           |
| `password_hash` | TEXT | Required Argon2id hash                            |
| `role`          | TEXT | `administrator` or `member`; defaults to `member` |
| `created_at`    | TEXT | Required, RFC 3339 timestamp                      |
| `last_login_at` | TEXT | Nullable RFC 3339 timestamp                       |

The first-run setup transaction creates the initial user with the `administrator` role and writes
`app_metadata.onboarding_complete`. Setup may use a password or a verified OIDC identity; for OIDC,
the matching `oidc_identities` row is committed in the same transaction. The metadata key is a
one-time database claim: setup can succeed only when both the claim and all users are absent.
Existing installations promote their earliest account to administrator when migration
`005_onboarding` is first applied.

Creating a browser session records `last_login_at`; the value remains after logout or session
expiry so the administrator directory can report the account's latest successful sign-in.

Administrators can list all accounts, promote or demote other users, and remove other accounts.
These operations are authorized on the server. A user cannot mutate their own administrator role
or delete their own active account, and conditional writes ensure at least one administrator
always remains. Removing an account cascades to its settings, sessions, tasks, and OIDC identities.

## `user_settings`

One preference record per user.

| Column                     | Type | Constraints                                         |
| -------------------------- | ---- | --------------------------------------------------- |
| `user_id`                  | TEXT | Primary key, references `users` with cascade delete |
| `display_name`             | TEXT | Required, trimmed length 1–60                       |
| `location`                 | TEXT | Required, trimmed length 1–80                       |
| `timezone`                 | TEXT | Required, trimmed length 1–80                       |
| `sidebar_timezones_json`   | TEXT | Valid JSON array containing 1–5 timezone names      |
| `calendar_week_start`      | TEXT | `sunday` or `monday`; defaults to `sunday`          |
| `temperature_unit`         | TEXT | `celsius` or `fahrenheit`                           |
| `lines_default_visibility` | TEXT | `private` or `public`; defaults to `private`        |
| `podcast_playback_rate`    | REAL | 0.5–3.0; defaults to 1.0                            |
| `updated_at`               | TEXT | Required, RFC 3339 timestamp                        |

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
This table holds only _uploaded_ images. A slot may instead point at a shared wall through
`user_wallpaper_selections`, which takes precedence; see that table for the full resolution order.

| Column       | Type | Constraints                                                          |
| ------------ | ---- | -------------------------------------------------------------------- |
| `user_id`    | TEXT | Composite primary key, references `users` with cascade delete        |
| `slot`       | TEXT | Composite primary key; `dashboard`, `welcome`, `loading`, or `login` |
| `mime_type`  | TEXT | JPEG, PNG, WebP, or AVIF                                             |
| `image_data` | BLOB | Required, 1 byte to 30 MB                                            |
| `updated_at` | TEXT | Required, RFC 3339 timestamp                                         |

## `walls`

The shared wallpaper collection. Any account may submit an image; only an administrator may
approve or reject it, and the decision is retained so a rejection keeps its reason visible to the
submitter. A `pending` or `rejected` wall is readable only by its submitter and by administrators.
Only an `approved` wall may be applied to a wallpaper slot. `user_id` is nulled when the submitting
account is deleted, so the collection survives and is shown as an unattributed contribution.
`width` and `height` are recorded at submission time so the gallery can reserve each tile's aspect
ratio, and the thumbnail is generated on the server so it can never disagree with the image an
administrator approved.

| Column           | Type    | Constraints                                                    |
| ---------------- | ------- | -------------------------------------------------------------- |
| `id`             | TEXT    | Primary key                                                    |
| `user_id`        | TEXT    | Optional reference to `users`, null on account delete          |
| `title`          | TEXT    | Required, trimmed length 1–120                                 |
| `description`    | TEXT    | Up to 500 characters, defaults to empty                        |
| `status`         | TEXT    | One of `pending`, `approved`, `rejected`                       |
| `decision_note`  | TEXT    | Administrator's reason, up to 500 characters                   |
| `decided_by`     | TEXT    | Optional reference to `users`, null on account delete          |
| `decided_at`     | TEXT    | Optional RFC 3339 timestamp                                    |
| `mime_type`      | TEXT    | JPEG, PNG, WebP, or AVIF                                       |
| `byte_size`      | INTEGER | Required, 1 byte to 30 MB                                      |
| `width`          | INTEGER | Required, decoded pixel width                                  |
| `height`         | INTEGER | Required, decoded pixel height                                 |
| `image_data`     | BLOB    | Required, the submitted image                                  |
| `thumbnail_mime` | TEXT    | Always `image/jpeg`                                            |
| `thumbnail_data` | BLOB    | Required, server-generated gallery thumbnail, 640 px long edge |
| `created_at`     | TEXT    | Required, RFC 3339 timestamp                                   |
| `updated_at`     | TEXT    | Required, RFC 3339 timestamp                                   |

## `wall_tags`

Free-form tags used to filter the collection. Compared case-insensitively.

| Column    | Type | Constraints                                            |
| --------- | ---- | ------------------------------------------------------ |
| `wall_id` | TEXT | Composite primary key, references `walls` with cascade |
| `tag`     | TEXT | Composite primary key, `NOCASE`, trimmed length 1–32   |

## `user_wallpaper_selections`

Points one wallpaper slot at a wall instead of an uploaded blob, so a wall applied by many people
is stored once. A slot resolves an approved selection first, then `user_wallpapers`, then the
packaged default. Deleting a wall cascades the selection away and a wall leaving `approved` stops
resolving, so both cases fall back without a cleanup pass. Uploading to a slot deletes its
selection and applying a wall deletes the slot's uploaded image, so the two sources never disagree.
The `login` slot remains a global singleton: every writer clears both tables for that slot across
all administrators before inserting.

| Column       | Type | Constraints                                                          |
| ------------ | ---- | -------------------------------------------------------------------- |
| `user_id`    | TEXT | Composite primary key, references `users` with cascade delete        |
| `slot`       | TEXT | Composite primary key; `dashboard`, `welcome`, `loading`, or `login` |
| `wall_id`    | TEXT | References `walls` with cascade delete                               |
| `updated_at` | TEXT | Required, RFC 3339 timestamp                                         |

## `user_avatars`

Optional private profile images. Avatar bytes are available only through the authenticated user's
settings endpoint and are deleted automatically with the owning account. A supported image from an
OIDC `picture` claim may initialize this record after the server network policy approves its origin,
but never replaces an existing avatar.

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

## `embedded_pages`

Configured HTTPS destinations rendered as product pages inside the authenticated shell. Global
rows are visible to every signed-in account and are managed only by administrators. User rows are
private to `owner_user_id`; administrators do not gain access to another account's personal rows.
Deleting an owner cascades their personal pages, while deleting the administrator who created a
global page only clears `created_by_user_id` so the shared entry remains available.
Administrators may transactionally move a global row into their own personal list or move one of
their own personal rows into the global list. The move closes the source ordering gap, appends to
the destination, and preserves the page identifier, content, permissions, icon, and audit creator.

The sidebar renders built-in pages first, then global rows, then the current account's user rows.
`position` is zero-based within that scope and is replaced transactionally during reordering.
`allow_scripts` and `allow_same_origin` are independent and disabled by default. The first permits
JavaScript execution inside the iframe; the second preserves the embedded site's origin for
storage, cookies, and origin checks. Either permission may be enabled alone or both may be enabled
together, with each opt-in reducing sandbox isolation.
`iframe_height` controls the vertical frame size in pixels. Existing and newly created pages default
to 720 pixels, with API and database constraints limiting values to 320–2,400 pixels. The interface
offers 480, 720, and 1,080 pixel presets plus a custom option; width remains responsive to the
application canvas.
`icon_kind` selects `favicon`, `lucide`, or `custom`. Favicon rows store no value; the server tries
the conventional origin favicon and bounded HTML-declared discovery. Lucide rows store one supported
icon name. Custom rows store a credential-free HTTPS image URL no longer than 2,000 characters.
Remote icons are fetched through the `images` network policy, SVG sources are rasterized, and up to
256 KiB of supported image bytes are stored in SQLite. The browser reads only the authenticated
Pandan icon endpoint. Existing rows are hydrated on their first icon request. A failed fetch records
the attempt without bytes so the interface stays on the packaged panel icon until the page is edited.

| Column               | Type    | Constraints                                                        |
| -------------------- | ------- | ------------------------------------------------------------------ |
| `id`                 | TEXT    | Primary key                                                        |
| `scope`              | TEXT    | Required, `global` or `user`                                       |
| `owner_user_id`      | TEXT    | Null for global rows; user owner with cascade delete otherwise     |
| `created_by_user_id` | TEXT    | Optional audit reference to `users`, set null when creator deletes |
| `title`              | TEXT    | Required, trimmed length 1–80                                      |
| `description`        | TEXT    | Required, maximum length 280                                       |
| `url`                | TEXT    | Required absolute HTTPS URL, maximum length 2,000                  |
| `icon_kind`          | TEXT    | Required, `favicon`, `lucide`, or `custom`                         |
| `icon_value`         | TEXT    | Null for favicon; supported Lucide name or HTTPS custom icon URL   |
| `icon_content_type`  | TEXT    | Optional supported cached image media type                         |
| `icon_data`          | BLOB    | Optional cached image bytes, maximum 256 KiB                       |
| `icon_fetched_at`    | TEXT    | Optional RFC 3339 timestamp for the latest fetch attempt           |
| `allow_scripts`      | INTEGER | Required boolean, defaults to `0`                                  |
| `allow_same_origin`  | INTEGER | Required boolean, defaults to `0`                                  |
| `iframe_height`      | INTEGER | Required, 320–2,400 pixels, defaults to `720`                      |
| `position`           | INTEGER | Required non-negative order within the page's scope                |
| `created_at`         | TEXT    | Required, RFC 3339 timestamp                                       |
| `updated_at`         | TEXT    | Required, RFC 3339 timestamp                                       |

## `user_appearance`

One Main background appearance record per user. A trigger creates the default row with each
account. Values are intentionally bounded so the wallpaper remains usable behind terminal surfaces.

| Column                  | Type    | Constraints                                         |
| ----------------------- | ------- | --------------------------------------------------- |
| `user_id`               | TEXT    | Primary key, references `users` with cascade delete |
| `background_blur`       | INTEGER | Blur radius from 0–24 pixels                        |
| `background_brightness` | INTEGER | Brightness percentage from 40–140                   |
| `background_contrast`   | INTEGER | Contrast percentage from 50–160                     |
| `background_saturation` | INTEGER | Saturation percentage from 0–180                    |
| `updated_at`            | TEXT    | Required, RFC 3339 timestamp                        |

## `login_appearance`

Singleton processing controls for the global Login background. The values are publicly readable
with the authentication bootstrap so the signed-out page can render them, but only an administrator
may update them. They are independent from every account's Main background processing.

| Column                  | Type    | Constraints                       |
| ----------------------- | ------- | --------------------------------- |
| `id`                    | INTEGER | Primary key; singleton value `1`  |
| `background_blur`       | INTEGER | Blur radius from 0–24 pixels      |
| `background_brightness` | INTEGER | Brightness percentage from 40–140 |
| `background_contrast`   | INTEGER | Contrast percentage from 50–160   |
| `background_saturation` | INTEGER | Saturation percentage from 0–180  |
| `updated_at`            | TEXT    | Required, RFC 3339 timestamp      |

## `sessions`

Opaque, revocable browser sessions. The cookie stores only the private token. The public ID is used
for account-scoped session management, while the last observed user agent and client address help the
owner identify each active browser without exposing its credential.

| Column         | Type | Constraints                                      |
| -------------- | ---- | ------------------------------------------------ |
| `token`        | TEXT | Primary key; private cookie credential           |
| `id`           | TEXT | Required, unique public session identifier       |
| `user_id`      | TEXT | Required, references `users` with cascade delete |
| `user_agent`   | TEXT | Required, at most 512 characters                 |
| `ip_address`   | TEXT | Required, at most 64 characters                  |
| `expires_at`   | TEXT | Required, RFC 3339 timestamp                     |
| `created_at`   | TEXT | Required, RFC 3339 timestamp                     |
| `last_seen_at` | TEXT | Required, RFC 3339 timestamp                     |

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

## `announcements`

Instance-wide homeserver notes readable by every authenticated account. Only administrators may
create, edit, or delete them. Deleting an author keeps the announcement and clears its author
reference so operational history remains available.

| Column       | Type | Constraints                                               |
| ------------ | ---- | --------------------------------------------------------- |
| `id`         | TEXT | Primary key                                               |
| `author_id`  | TEXT | Optional `users` reference; set to null on account delete |
| `title`      | TEXT | Required, trimmed length 1–160                            |
| `content`    | TEXT | Required Markdown source, trimmed length 1–50,000         |
| `created_at` | TEXT | Required, RFC 3339 timestamp                              |
| `updated_at` | TEXT | Required, RFC 3339 timestamp                              |

## `announcement_images`

Administrator-uploaded announcement images stored in SQLite. Bytes are served only after
authentication with content sniffing disabled.

| Column            | Type    | Constraints                                    |
| ----------------- | ------- | ---------------------------------------------- |
| `id`              | TEXT    | Primary key                                    |
| `announcement_id` | TEXT    | References `announcements` with cascade delete |
| `file_name`       | TEXT    | Required, trimmed length 1–255                 |
| `mime_type`       | TEXT    | JPEG, PNG, WebP, or AVIF                       |
| `byte_size`       | INTEGER | Required, 1 byte to 10 MB                      |
| `image_data`      | BLOB    | Required validated image bytes                 |
| `created_at`      | TEXT    | Required, RFC 3339 timestamp                   |

## `announcement_reactions`

One reaction of a given emoji per account and announcement. The composite primary key is
`(announcement_id, user_id, emoji)`, and account deletion cascades through that account's
reactions without removing the announcement.

| Column            | Type | Constraints                                    |
| ----------------- | ---- | ---------------------------------------------- |
| `announcement_id` | TEXT | References `announcements` with cascade delete |
| `user_id`         | TEXT | References `users` with cascade delete         |
| `emoji`           | TEXT | Required, 1–32 characters                      |
| `created_at`      | TEXT | Required, RFC 3339 timestamp                   |

## `line_posts`

Markdown source posts for the Lines timeline. A public post is readable by authenticated users on
the instance; a private post is readable only by its owner, including when the viewer is an
administrator. Replies are posts linked through `reply_to_post_id`. Replies to private posts are
forced private by the API. Deleting an account cascades through its posts, while replies owned by
other accounts retain their content and clear a deleted parent reference.

| Column             | Type | Constraints                                                     |
| ------------------ | ---- | --------------------------------------------------------------- |
| `id`               | TEXT | Primary key                                                     |
| `user_id`          | TEXT | Required, references `users` with cascade delete                |
| `content`          | TEXT | Required, trimmed length 1–2,000                                |
| `visibility`       | TEXT | `private` or `public`; defaults to `private`                    |
| `reply_to_post_id` | TEXT | Optional self-reference; set to null when the parent is deleted |
| `created_at`       | TEXT | Required, RFC 3339 timestamp                                    |
| `updated_at`       | TEXT | Required, RFC 3339 timestamp                                    |

## `line_post_tags`

Case-insensitive, normalized hashtags extracted by the server when a post is created. The
`(post_id, tag)` pair is the primary key. Tag filters and counts are evaluated only across posts
visible to the authenticated viewer.

| Column    | Type | Constraints                                 |
| --------- | ---- | ------------------------------------------- |
| `post_id` | TEXT | References `line_posts` with cascade delete |
| `tag`     | TEXT | Required, case-insensitive length 1–64      |

## `line_post_reactions`

One reaction of a given emoji per user and post. The composite primary key is
`(post_id, user_id, emoji)`. Reactions can be created only when the parent post is readable.

| Column       | Type | Constraints                                 |
| ------------ | ---- | ------------------------------------------- |
| `post_id`    | TEXT | References `line_posts` with cascade delete |
| `user_id`    | TEXT | References `users` with cascade delete      |
| `emoji`      | TEXT | Required, 1–32 characters                   |
| `created_at` | TEXT | Required, RFC 3339 timestamp                |

## `line_post_attachments`

Lines files stored in SQLite. Owners may upload or delete files. Reads verify the parent post on
every request: private attachment bytes are owner-only, while public attachment bytes require any
authenticated account. Only JPEG, PNG, WebP, and AVIF files are displayed inline; other types are
served as downloads with content sniffing disabled.

| Column       | Type    | Constraints                                 |
| ------------ | ------- | ------------------------------------------- |
| `id`         | TEXT    | Primary key                                 |
| `post_id`    | TEXT    | References `line_posts` with cascade delete |
| `file_name`  | TEXT    | Required, trimmed length 1–255              |
| `mime_type`  | TEXT    | Required, trimmed length 1–120              |
| `byte_size`  | INTEGER | Required, 1 byte to 10 MB                   |
| `file_data`  | BLOB    | Required                                    |
| `created_at` | TEXT    | Required, RFC 3339 timestamp                |

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

User-owned RSS, Atom, or generated Reddit listing sources for the dedicated reader. URLs are unique
per user. New subscriptions default to deleting all unsaved items after seven days. Reddit helpers
store a public `reddit.com/r/{subreddit}/{sort}.rss` Atom URL; older saved `.json` listing URLs are
normalized to the same Atom endpoint when fetched. Fetching uses the server network policy; public
HTTPS is implicit, while private or HTTP origins require an administrator allow rule. A normalized
origin is stored separately to make base-URL filtering predictable.
An optional account-owned custom name controls how a source is labelled without replacing the
title fetched from the feed, so later refreshes cannot overwrite the user's choice.

The background refresh worker schedules on `last_attempted_at`, which is stamped when a refresh is
claimed and by every manual refresh, so a failing source backs off for a full window instead of
being retried on every sweep. `last_fetched_at` still records the last successful fetch. Every
successful refresh advances `refresh_generation`; failures leave it unchanged so Current views keep
showing the last known-good snapshot. Each subscription exposes only its configured number of latest
snapshot entries in Current; the limit defaults to 25 and may be set from 1 through 200.

| Column                | Type    | Constraints                               |
| --------------------- | ------- | ----------------------------------------- |
| `id`                  | TEXT    | Primary key                               |
| `user_id`             | TEXT    | References `users` with cascade delete    |
| `url`                 | TEXT    | Required source URL, unique per user      |
| `base_url`            | TEXT    | Required normalized HTTP(S) origin        |
| `title`               | TEXT    | Required, fetched feed title              |
| `custom_name`         | TEXT    | Optional, trimmed length 1–80             |
| `category`            | TEXT    | Required, trimmed length 1–40             |
| `auto_delete_days`    | INTEGER | Optional age from 1–3,650 days            |
| `auto_delete_mode`    | TEXT    | `read` or `all`                           |
| `current_entry_limit` | INTEGER | Latest Current entries, 1–200; default 25 |
| `last_fetched_at`     | TEXT    | Optional RFC 3339 timestamp               |
| `last_attempted_at`   | TEXT    | Optional RFC 3339 refresh attempt         |
| `last_error`          | TEXT    | Optional safe provider error              |
| `refresh_generation`  | INTEGER | Latest successful snapshot, initially 0   |
| `created_at`          | TEXT    | Required, RFC 3339 timestamp              |
| `updated_at`          | TEXT    | Required, RFC 3339 timestamp              |

## `rss_items`

Fetched reader entries owned through their subscription. Refresh upserts by the source's stable
identifier and preserves `read_at`. Automatic retention runs when the reader loads or a source
refreshes; manual pruning can remove old read-only or all entries across one user's subscriptions.
Entries may retain separate article and discussion destinations. Entries saved in `rss_read_later`
are excluded from both automatic retention and manual pruning. Entries stamped with the
subscription's latest successful generation are ranked newest-first. The configured latest entries
form its Current projection and are also protected from retention while the source still exposes them.

| Column                 | Type    | Constraints                                         |
| ---------------------- | ------- | --------------------------------------------------- |
| `id`                   | TEXT    | Primary key                                         |
| `subscription_id`      | TEXT    | References `rss_subscriptions` with cascade delete  |
| `external_id`          | TEXT    | Required, unique within the subscription            |
| `url`                  | TEXT    | Entry destination, empty when omitted by the feed   |
| `comments_url`         | TEXT    | Discussion destination, empty when omitted          |
| `title`                | TEXT    | Required, trimmed length 1–500                      |
| `summary`              | TEXT    | Required, defaults to an empty string               |
| `published_at`         | TEXT    | RFC 3339; fetch time is used when the feed omits it |
| `fetched_at`           | TEXT    | Required, RFC 3339 timestamp                        |
| `read_at`              | TEXT    | Optional RFC 3339 timestamp                         |
| `last_seen_generation` | INTEGER | Successful refresh that last exposed the item       |

## `rss_read_later`

Account-owned RSS Read Later membership. The API verifies that the referenced item belongs to the
same account before creating a row. Deleting the source subscription cascades through the item and
removes its saved membership.

| Column     | Type | Constraints                                                       |
| ---------- | ---- | ----------------------------------------------------------------- |
| `user_id`  | TEXT | Composite primary key, references `users` with cascade delete     |
| `item_id`  | TEXT | Composite primary key, references `rss_items` with cascade delete |
| `saved_at` | TEXT | Required, RFC 3339 timestamp                                      |

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

## `youtube_watch_later`

Private account-to-video Watch Later membership. A video may be saved only while its channel is
subscribed by that account. The saved row remains available after unsubscribing because video
metadata is shared at the instance level; it is removed when the account, video, or YouTube content
area is deleted.

| Column     | Type | Constraints                                                            |
| ---------- | ---- | ---------------------------------------------------------------------- |
| `user_id`  | TEXT | Composite primary key, references `users` with cascade delete          |
| `video_id` | TEXT | Composite primary key, references `youtube_videos` with cascade delete |
| `saved_at` | TEXT | Required, RFC 3339 timestamp                                           |

## `youtube_download_jobs`

Account-owned YouTube output jobs. Each row represents one individual video and one server-defined
profile; playlists and arbitrary yt-dlp expressions are never stored. The partial and completed
media bytes live beneath `PANDAN_DOWNLOAD_DIR`, while the database remains the authoritative
lifecycle and ownership record. A partial unique index prevents duplicate active jobs for the same
account, video, kind, format, and selected height. Administrators do not gain access to another
account's rows or files.

| Column                   | Type    | Constraints                                                                                   |
| ------------------------ | ------- | --------------------------------------------------------------------------------------------- |
| `id`                     | TEXT    | UUID primary key                                                                              |
| `user_id`                | TEXT    | Required reference to `users` with cascade delete                                             |
| `source_url`             | TEXT    | Normalized credential-free YouTube HTTPS URL, length 1–2048                                   |
| `youtube_video_id`       | TEXT    | Required validated video ID, length 1–32                                                      |
| `title`                  | TEXT    | Required bounded source title, length at most 500                                             |
| `channel_name`           | TEXT    | Required bounded channel label, length at most 300                                            |
| `duration_seconds`       | INTEGER | Optional non-negative inspected duration                                                      |
| `media_kind`             | TEXT    | `video` or `audio`                                                                            |
| `output_format`          | TEXT    | `mp4`, `mkv`, `webm`, `m4a`, `mp3`, or `opus`                                                 |
| `max_height`             | INTEGER | Optional positive video height; null for best or audio                                        |
| `status`                 | TEXT    | `queued`, `inspecting`, `downloading`, `postprocessing`, `complete`, `failed`, or `cancelled` |
| `progress_percent`       | REAL    | Optional real value from 0–100                                                                |
| `downloaded_bytes`       | INTEGER | Non-negative observed transfer count                                                          |
| `total_bytes`            | INTEGER | Optional non-negative source total or estimate                                                |
| `speed_bytes_per_second` | REAL    | Optional non-negative progress hint                                                           |
| `eta_seconds`            | INTEGER | Optional non-negative progress hint                                                           |
| `storage_file_name`      | TEXT    | Server-generated filename only; empty before completion                                       |
| `display_file_name`      | TEXT    | Sanitized attachment filename; never used for path resolution                                 |
| `mime_type`              | TEXT    | Validated final MIME type; empty before completion                                            |
| `byte_size`              | INTEGER | Non-negative final file size                                                                  |
| `attempts`               | INTEGER | Claim count from 0–3                                                                          |
| `error_code`             | TEXT    | Optional bounded internal failure category                                                    |
| `last_error`             | TEXT    | Optional bounded user-safe message; no subprocess output                                      |
| `lease_started_at`       | TEXT    | Optional RFC 3339 worker lease                                                                |
| `created_at`             | TEXT    | Required RFC 3339 timestamp                                                                   |
| `started_at`             | TEXT    | Optional RFC 3339 first-start timestamp                                                       |
| `completed_at`           | TEXT    | Optional RFC 3339 completion timestamp                                                        |
| `updated_at`             | TEXT    | Required RFC 3339 timestamp                                                                   |

## `youtube_download_settings`

Administrator-managed singleton (`id = 1`) controlling member access, storage reservations, queue
depth, batches, and fair worker concurrency. Defaults are 20 GiB instance storage, 10 GiB per
account, 2 GiB per output, two global workers, one worker per account, 10 URLs per batch, and 50
unsettled rows per account. Policy validation requires the worst-case concurrent reservation to fit
both storage budgets.

| Column                     | Type    | Constraints                                              |
| -------------------------- | ------- | -------------------------------------------------------- |
| `id`                       | INTEGER | Primary key constrained to `1`                           |
| `member_downloads_enabled` | INTEGER | Boolean, default enabled                                 |
| `storage_budget_bytes`     | INTEGER | Positive instance ceiling                                |
| `per_user_budget_bytes`    | INTEGER | Positive account ceiling no larger than instance storage |
| `max_output_bytes`         | INTEGER | Positive per-job ceiling no larger than account storage  |
| `global_concurrency`       | INTEGER | 1–8                                                      |
| `per_user_concurrency`     | INTEGER | 1–4 and no larger than global concurrency                |
| `max_batch_urls`           | INTEGER | 1–50                                                     |
| `max_queued_per_user`      | INTEGER | 1–200                                                    |
| `updated_at`               | TEXT    | Required RFC 3339 timestamp                              |

## `podcasts`

The instance's administrator-curated podcast catalogue. A row exists only once an administrator has
approved a feed or added one directly; a member request never creates one. Feed and enclosure origins
are evaluated by the server network policy. `normalized_url` is the
uniqueness key, so the same show cannot be catalogued twice under cosmetically different addresses.
Artwork is cached here as a blob and refreshed at most once a day; a failed fetch never overwrites
it. Episode audio is **not** stored in SQLite — see `podcast_downloads`.

| Column                  | Type    | Constraints                                           |
| ----------------------- | ------- | ----------------------------------------------------- |
| `id`                    | TEXT    | Primary key                                           |
| `feed_url`              | TEXT    | Required, trimmed length 1–2048, as submitted         |
| `normalized_url`        | TEXT    | Required, unique, trimmed length 1–2048               |
| `title`                 | TEXT    | Required, trimmed length 1–300                        |
| `description`           | TEXT    | Required, defaults to empty                           |
| `author`                | TEXT    | Required, defaults to empty                           |
| `site_url`              | TEXT    | Required, defaults to empty                           |
| `language`              | TEXT    | Required, defaults to empty                           |
| `artwork_url`           | TEXT    | Required, defaults to empty                           |
| `artwork_content_type`  | TEXT    | Required, defaults to empty                           |
| `artwork_data`          | BLOB    | Optional cached image bytes                           |
| `artwork_fetched_at`    | TEXT    | Optional RFC 3339 timestamp                           |
| `auto_download_count`   | INTEGER | Newest episodes cached automatically, 0–25, default 3 |
| `max_retained_episodes` | INTEGER | Retention window, 1–1000, default 50                  |
| `added_by`              | TEXT    | Optional reference to `users`, null on account delete |
| `last_fetched_at`       | TEXT    | Optional RFC 3339 timestamp                           |
| `refresh_started_at`    | TEXT    | Optional refresh lease, RFC 3339 timestamp            |
| `last_error`            | TEXT    | Optional isolated refresh failure                     |
| `created_at`            | TEXT    | Required, RFC 3339 timestamp                          |
| `updated_at`            | TEXT    | Required, RFC 3339 timestamp                          |

## `podcast_requests`

Member requests awaiting an administrator decision, retained afterwards as history so a rejection
keeps its reason visible to the requester. A partial unique index over
`(user_id, normalized_url) WHERE status = 'pending'` allows one open request per user per feed while
letting decided rows accumulate. Only `pending` rows may be approved, rejected, or withdrawn.

| Column                 | Type | Constraints                                                            |
| ---------------------- | ---- | ---------------------------------------------------------------------- |
| `id`                   | TEXT | Primary key                                                            |
| `user_id`              | TEXT | References `users` with cascade delete                                 |
| `feed_url`             | TEXT | Required, trimmed length 1–2048                                        |
| `normalized_url`       | TEXT | Required, trimmed length 1–2048                                        |
| `resolved_title`       | TEXT | Preview resolved from the feed, defaults to empty                      |
| `resolved_author`      | TEXT | Preview resolved from the feed, defaults to empty                      |
| `resolved_artwork_url` | TEXT | Preview resolved from the feed, defaults to empty                      |
| `note`                 | TEXT | Requester's reason, up to 500 characters                               |
| `status`               | TEXT | One of `pending`, `approved`, `rejected`, `withdrawn`                  |
| `decision_note`        | TEXT | Administrator's reason, up to 500 characters                           |
| `decided_by`           | TEXT | Optional reference to `users`, null on account delete                  |
| `decided_at`           | TEXT | Optional RFC 3339 timestamp                                            |
| `podcast_id`           | TEXT | Optional reference to `podcasts`, set on approval, null on show delete |
| `created_at`           | TEXT | Required, RFC 3339 timestamp                                           |
| `updated_at`           | TEXT | Required, RFC 3339 timestamp                                           |

## `podcast_episodes`

Feed items indexed at the instance level, shared by every subscriber. Unique per `(podcast_id, guid)`;
feeds that omit a guid fall back to the enclosure URL so re-indexing stays idempotent. Rows are
refreshed in place, so a retitled episode keeps everyone's listening position.

| Column             | Type    | Constraints                                      |
| ------------------ | ------- | ------------------------------------------------ |
| `id`               | TEXT    | Primary key                                      |
| `podcast_id`       | TEXT    | References `podcasts` with cascade delete        |
| `guid`             | TEXT    | Required, trimmed length 1–2048, unique per show |
| `title`            | TEXT    | Required, trimmed length 1–500                   |
| `description`      | TEXT    | Required, defaults to empty                      |
| `episode_url`      | TEXT    | Required, defaults to empty                      |
| `enclosure_url`    | TEXT    | Required, trimmed length 1–2048                  |
| `enclosure_type`   | TEXT    | Required, defaults to empty                      |
| `enclosure_bytes`  | INTEGER | Optional, non-negative                           |
| `duration_seconds` | INTEGER | Optional, non-negative                           |
| `published_at`     | TEXT    | Required, RFC 3339 timestamp                     |
| `fetched_at`       | TEXT    | Required, RFC 3339 timestamp                     |

## `podcast_downloads`

The index over cached episode files, doubling as the download work queue. The audio itself lives on
disk under `PANDAN_MEDIA_DIR` (default `data/podcasts`) as `<episode_id>.<ext>`, where the extension
comes from a server-side media-type allowlist — nothing from a remote URL or header reaches the
filesystem. `requested_by` is `ON DELETE SET NULL` on purpose: cached audio is a shared instance
resource and must outlive the account that first asked for it. Startup reconciles this table against
the media root in both directions.

| Column             | Type    | Constraints                                                           |
| ------------------ | ------- | --------------------------------------------------------------------- |
| `episode_id`       | TEXT    | Primary key, references `podcast_episodes` with cascade delete        |
| `status`           | TEXT    | One of `queued`, `downloading`, `ready`, `failed`                     |
| `requested_by`     | TEXT    | Optional reference to `users`, null on account delete                 |
| `file_name`        | TEXT    | Plain file name inside the media root, defaults to empty              |
| `content_type`     | TEXT    | Required, defaults to empty                                           |
| `byte_size`        | INTEGER | Required, non-negative, defaults to 0                                 |
| `downloaded_bytes` | INTEGER | Required, non-negative, defaults to 0                                 |
| `pinned`           | INTEGER | 0 or 1; pinned files are never evicted                                |
| `attempts`         | INTEGER | Required, non-negative, defaults to 0                                 |
| `last_error`       | TEXT    | Required, defaults to empty                                           |
| `lease_started_at` | TEXT    | Optional worker lease, RFC 3339 timestamp                             |
| `last_accessed_at` | TEXT    | Optional RFC 3339 timestamp; least-recently-used eviction ranks on it |
| `created_at`       | TEXT    | Required, RFC 3339 timestamp                                          |
| `updated_at`       | TEXT    | Required, RFC 3339 timestamp                                          |

## `podcast_subscriptions`

Private account-to-podcast membership. Every episode read — metadata, audio bytes, and listening
state — resolves through this table. New-episode notification routing is optional and may reference
only an account-owned ntfy topic; deleting that topic disables the route.

| Column                       | Type    | Constraints                                                      |
| ---------------------------- | ------- | ---------------------------------------------------------------- |
| `user_id`                    | TEXT    | Composite primary key, references `users` with cascade delete    |
| `podcast_id`                 | TEXT    | Composite primary key, references `podcasts` with cascade delete |
| `ntfy_notifications_enabled` | INTEGER | 0 or 1, defaults to 0                                            |
| `ntfy_topic_id`              | TEXT    | Optional reference to `ntfy_topics`, null on topic delete        |
| `created_at`                 | TEXT    | Required, RFC 3339 timestamp                                     |

## `podcast_notification_deliveries`

Durable account-scoped work queue for newly indexed episodes. Rows are inserted in the same
transaction as their episode, claimed with a short lease, retried with backoff, and resolved against
the subscription's current topic and ntfy connection before publishing. Disabling a show's route
removes only undelivered work; delivered rows prevent old episodes from replaying after re-enabling.

| Column             | Type    | Constraints                                                              |
| ------------------ | ------- | ------------------------------------------------------------------------ |
| `user_id`          | TEXT    | Composite primary key, references `users` with cascade delete            |
| `episode_id`       | TEXT    | Composite primary key, references `podcast_episodes` with cascade delete |
| `attempts`         | INTEGER | Required, non-negative, defaults to 0                                    |
| `last_error`       | TEXT    | Required, defaults to empty                                              |
| `next_attempt_at`  | TEXT    | Required RFC 3339 retry time                                             |
| `lease_started_at` | TEXT    | Optional RFC 3339 worker lease                                           |
| `delivered_at`     | TEXT    | Optional RFC 3339 successful publish time                                |
| `created_at`       | TEXT    | Required, RFC 3339 timestamp                                             |
| `updated_at`       | TEXT    | Required, RFC 3339 timestamp                                             |

## `podcast_episode_progress`

Private per-account resume position and completion state, written on an interval while playing and
on pause, seek, and page hide.

| Column             | Type    | Constraints                                                              |
| ------------------ | ------- | ------------------------------------------------------------------------ |
| `user_id`          | TEXT    | Composite primary key, references `users` with cascade delete            |
| `episode_id`       | TEXT    | Composite primary key, references `podcast_episodes` with cascade delete |
| `position_seconds` | INTEGER | Required, non-negative, defaults to 0                                    |
| `completed_at`     | TEXT    | Optional RFC 3339 timestamp                                              |
| `updated_at`       | TEXT    | Required, RFC 3339 timestamp                                             |

## `podcast_queue`

Private per-account play order. This is play order, not a saved collection. `UNIQUE(user_id, position)`
is checked per statement, so reordering parks every affected row in the 256–511 band before writing
final 0–255 positions; the two ranges cannot overlap, which is what makes a full reversal safe.

| Column       | Type    | Constraints                                                              |
| ------------ | ------- | ------------------------------------------------------------------------ |
| `user_id`    | TEXT    | Composite primary key, references `users` with cascade delete            |
| `episode_id` | TEXT    | Composite primary key, references `podcast_episodes` with cascade delete |
| `position`   | INTEGER | 0–511; 0–255 are final positions, 256–511 the reorder parking band       |
| `added_at`   | TEXT    | Required, RFC 3339 timestamp                                             |

## `podcast_saved_episodes`

Private account-to-episode saved list, mirroring `rss_read_later` and `youtube_watch_later`. A saved
episode is exempt from retention trimming.

| Column       | Type | Constraints                                                              |
| ------------ | ---- | ------------------------------------------------------------------------ |
| `user_id`    | TEXT | Composite primary key, references `users` with cascade delete            |
| `episode_id` | TEXT | Composite primary key, references `podcast_episodes` with cascade delete |
| `saved_at`   | TEXT | Required, RFC 3339 timestamp                                             |

## `podcast_settings`

Singleton administrator-controlled podcast policy, seeded by migration `038_podcasts` exactly as
`authentication_settings` is. Closing requests makes the catalogue administrator-only; the storage
budget bounds how much disk cached audio may occupy before least-recently-used eviction runs.

| Column                          | Type    | Constraints                      |
| ------------------------------- | ------- | -------------------------------- |
| `id`                            | INTEGER | Primary key, always 1            |
| `requests_enabled`              | INTEGER | 0 or 1, defaults to 1            |
| `member_downloads_enabled`      | INTEGER | 0 or 1, defaults to 1            |
| `max_pending_requests_per_user` | INTEGER | 0–100, defaults to 5             |
| `storage_budget_bytes`          | INTEGER | 0–1 TiB, defaults to 20 GiB      |
| `max_episode_bytes`             | INTEGER | 1 MiB–5 GiB, defaults to 500 MiB |
| `default_auto_download_count`   | INTEGER | 0–25, defaults to 3              |
| `updated_at`                    | TEXT    | Required, RFC 3339 timestamp     |

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

User-owned iCalendar sources approved by the server network policy. URLs are unique within an
account. A custom listing name overrides the fetched source name without being replaced by refresh.
Refresh errors are stored without discarding the most recent successful event snapshot.

| Column            | Type | Constraints                                              |
| ----------------- | ---- | -------------------------------------------------------- |
| `id`              | TEXT | Primary key                                              |
| `user_id`         | TEXT | References `users` with cascade delete                   |
| `url`             | TEXT | Required source URL, unique per user                     |
| `name`            | TEXT | Required fetched calendar name, length 1–120             |
| `custom_name`     | TEXT | Optional owner-defined listing name, length 1–120        |
| `color`           | TEXT | Legacy preset key retained for migration compatibility   |
| `color_value`     | TEXT | Required six-digit sRGB hex color                        |
| `display_mode`    | TEXT | Month view presentation: `full` or `dot`; default `full` |
| `last_fetched_at` | TEXT | Optional RFC 3339 timestamp                              |
| `last_error`      | TEXT | Optional safe provider error                             |
| `created_at`      | TEXT | Required, RFC 3339 timestamp                             |
| `updated_at`      | TEXT | Required, RFC 3339 timestamp                             |

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

## `ntfy_connections`

One private ntfy server configuration per account. The server may be ntfy.sh or another
credential-free origin approved by the server network policy. The optional access token is encrypted
with XChaCha20-Poly1305 and API responses expose only whether a token exists. The Rust server keeps
the upstream subscription open independently of browser sessions. A recovery-sync or stream failure
records a user-safe error without discarding previously retrieved notifications.

| Column             | Type | Constraints                                                |
| ------------------ | ---- | ---------------------------------------------------------- |
| `user_id`          | TEXT | Primary key, references `users` with cascade delete        |
| `base_url`         | TEXT | Required credential-free HTTP(S) ntfy server URL           |
| `token_ciphertext` | TEXT | Optional encrypted access token, never returned by the API |
| `last_synced_at`   | TEXT | Optional RFC 3339 timestamp                                |
| `last_error`       | TEXT | Optional safe provider error                               |
| `created_at`       | TEXT | Required, RFC 3339 timestamp                               |
| `updated_at`       | TEXT | Required, RFC 3339 timestamp                               |

## `ntfy_topics`

Manual topic subscriptions for the account's connected server. A topic uses ntfy's portable
letters, numbers, dot, underscore, and hyphen form and is unique within the account. The cursor is
stored per topic so adding a new topic can import its available cache without rewinding existing
subscriptions.
Changing the connected server clears locally cached ntfy messages and resets every topic cursor,
while retaining the manually entered topic names and labels so they can be polled on the new server.

| Column            | Type | Constraints                                                  |
| ----------------- | ---- | ------------------------------------------------------------ |
| `id`              | TEXT | Primary key                                                  |
| `user_id`         | TEXT | References `ntfy_connections` with cascade delete            |
| `topic`           | TEXT | Required, 1–64 portable topic characters, unique per user    |
| `label`           | TEXT | Required display label, up to 80 characters                  |
| `last_message_id` | TEXT | Optional ntfy message ID used as the incremental sync cursor |
| `created_at`      | TEXT | Required, RFC 3339 timestamp                                 |
| `updated_at`      | TEXT | Required, RFC 3339 timestamp                                 |

## `ntfy_notifications`

Private local copies of ntfy messages. `(topic_id, remote_id)` is unique, making recovery polling
and realtime replay idempotent. `seen_at` drives the header count. Deleting sends ntfy's sequence
deletion request upstream first and permanently removes the local row only after that succeeds.
Deleting the selected topic, or the combined inbox, submits one account-scoped Pandan request; the
server processes the matching ntfy sequence deletions serially with non-blocking pacing so bulk
actions stay below the provider's default request burst rate.
Migration 047 purges rows archived by earlier builds; `archived_at` remains as a compatibility
column but the application no longer writes or lists archived notifications. Tags and actions
retain the bounded upstream JSON; view links and copy actions run in the browser, while HTTP actions
require an authenticated user gesture and the server network policy.

| Column         | Type    | Constraints                                           |
| -------------- | ------- | ----------------------------------------------------- |
| `id`           | TEXT    | Primary key                                           |
| `user_id`      | TEXT    | References `users` with cascade delete                |
| `topic_id`     | TEXT    | References `ntfy_topics` with cascade delete          |
| `remote_id`    | TEXT    | Required upstream message ID, unique within one topic |
| `occurred_at`  | INTEGER | Required Unix timestamp supplied by ntfy              |
| `title`        | TEXT    | Required, normalized display title                    |
| `message`      | TEXT    | Required notification body                            |
| `priority`     | INTEGER | Required ntfy priority, normalized to 1–5             |
| `tags_json`    | TEXT    | Required valid JSON array                             |
| `click_url`    | TEXT    | Optional absolute HTTP(S) destination                 |
| `actions_json` | TEXT    | Required valid JSON array of bounded ntfy actions     |
| `seen_at`      | TEXT    | Optional RFC 3339 timestamp                           |
| `archived_at`  | TEXT    | Legacy compatibility column; new rows remain NULL     |
| `received_at`  | TEXT    | Required RFC 3339 timestamp                           |

## `contact_dav_sources`

Account-owned CardDAV address-book resources. The URL is a direct HTTP(S) address-book collection
and is evaluated by the server network policy before storage and sync.
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

New and edited records store a validated interval and unit. `frequency` remains as the
server-formatted display label for compatibility. Legacy rows with an unrecognized free-form label
may leave the structured fields null until the record is edited.

| Column               | Type    | Constraints                                                                             |
| -------------------- | ------- | --------------------------------------------------------------------------------------- |
| `id`                 | TEXT    | Primary key                                                                             |
| `user_id`            | TEXT    | References `users` with cascade delete                                                  |
| `service`            | TEXT    | Required, trimmed length 1–120                                                          |
| `description`        | TEXT    | Required, up to 2,000 characters                                                        |
| `frequency`          | TEXT    | Required server-formatted display label, trimmed length 1–40                            |
| `frequency_interval` | INTEGER | 1–999 for structured schedules; null only for unrecognized legacy labels                |
| `frequency_unit`     | TEXT    | `day`, `week`, `month`, or `year`; null only for unrecognized legacy labels             |
| `amount_micros`      | INTEGER | Exact charge per billing period in millionths of one currency unit; 0–1,000,000,000,000 |
| `currency`           | TEXT    | Required uppercase three-letter currency code; defaults to `USD` for migrated records   |
| `first_paid_on`      | TEXT    | Required ISO calendar date                                                              |
| `created_at`         | TEXT    | Required, RFC 3339 timestamp                                                            |
| `updated_at`         | TEXT    | Required, RFC 3339 timestamp                                                            |

## `trading_settings`

One private Trading provider record per account. The Finnhub key is optional and is encrypted by
the server before storage. API responses expose only whether a key exists. Refresh status is safe
for display and retains the timestamp of the last successful provider update when a later attempt
fails.

| Column                        | Type | Constraints                                                     |
| ----------------------------- | ---- | --------------------------------------------------------------- |
| `user_id`                     | TEXT | Primary key, references `users` with cascade delete              |
| `finnhub_api_key_ciphertext`  | TEXT | Nullable XChaCha20-Poly1305 nonce and ciphertext, base64 encoded |
| `last_refresh_at`             | TEXT | Nullable RFC 3339 timestamp of the last successful refresh       |
| `last_refresh_error`          | TEXT | Nullable provider-safe status message                            |
| `created_at`                  | TEXT | Required, RFC 3339 timestamp                                     |
| `updated_at`                  | TEXT | Required, RFC 3339 timestamp                                     |

## `trading_watchlist`

Account-owned market symbols in a stable display order. Each account may store up to ten rows.
Symbols compare case-insensitively, and deleting one cascades to its cached quote.

| Column       | Type    | Constraints                                      |
| ------------ | ------- | ------------------------------------------------ |
| `id`         | TEXT    | Primary key                                      |
| `user_id`    | TEXT    | References `users` with cascade delete           |
| `symbol`     | TEXT    | Required, 1–16 characters; unique per account    |
| `position`   | INTEGER | Required 0–9; unique per account                 |
| `created_at` | TEXT    | Required, RFC 3339 timestamp                     |
| `updated_at` | TEXT    | Required, RFC 3339 timestamp                     |

## `trading_quotes`

The last successful quote for each watched symbol. Price fields remain provider decimal strings
instead of binary floating-point values. Partial refreshes upsert only successful symbols, leaving
older cached rows available so the Trading page never blanks while a provider is degraded.

| Column           | Type | Constraints                                                             |
| ---------------- | ---- | ----------------------------------------------------------------------- |
| `user_id`        | TEXT | Composite primary key and watchlist foreign key                         |
| `symbol`         | TEXT | Composite primary key and watchlist foreign key                         |
| `name`           | TEXT | Required provider name or the normalized symbol                         |
| `price`          | TEXT | Required provider decimal string                                        |
| `previous_close` | TEXT | Nullable provider decimal string                                        |
| `day_open`       | TEXT | Nullable provider decimal string                                        |
| `day_high`       | TEXT | Nullable provider decimal string                                        |
| `day_low`        | TEXT | Nullable provider decimal string                                        |
| `change_percent` | TEXT | Nullable provider decimal string                                        |
| `currency`       | TEXT | Required provider currency; may be empty until metadata has been cached |
| `market_state`   | TEXT | Nullable provider market-state label                                    |
| `source`         | TEXT | `yahoo` or `finnhub`                                                   |
| `quoted_at`      | TEXT | Required RFC 3339 provider timestamp                                    |
| `refreshed_at`   | TEXT | Required RFC 3339 timestamp when Pandan stored this snapshot            |

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

## `coding_categories`

Account-owned labels used to organize subscribed Coding repositories. Names compare
case-insensitively and are unique within one account.

| Column       | Type | Constraints                                       |
| ------------ | ---- | ------------------------------------------------- |
| `id`         | TEXT | Primary key                                       |
| `user_id`    | TEXT | References `users` with cascade delete            |
| `name`       | TEXT | Required, trimmed length 1–48; unique per account |
| `created_at` | TEXT | Required, RFC 3339 timestamp                      |
| `updated_at` | TEXT | Required, RFC 3339 timestamp                      |

## `coding_project_categories`

Many-to-many category assignments for subscribed repositories. Handlers and queries verify that
both sides belong to the authenticated account before replacing assignments.

| Column        | Type | Constraints                                                               |
| ------------- | ---- | ------------------------------------------------------------------------- |
| `project_id`  | TEXT | Composite primary key; references `coding_projects` with cascade delete   |
| `category_id` | TEXT | Composite primary key; references `coding_categories` with cascade delete |

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

## `bookmarks`

Private quick links shown in the dashboard utility rail. Each account may store at most 32 rows.
The destination remains a browser link; only the derived origin `/favicon.ico` is fetched by the
server. Supported favicon bytes are cached in the same row and are returned only after the request
resolves through that row's `user_id`.

| Column                 | Type | Constraints                                                                                       |
| ---------------------- | ---- | ------------------------------------------------------------------------------------------------- |
| `id`                   | TEXT | Primary key                                                                                       |
| `user_id`              | TEXT | Required, references `users` with cascade delete                                                  |
| `title`                | TEXT | Required, trimmed length 1–120                                                                    |
| `url`                  | TEXT | Required credential-free HTTP or HTTPS URL, up to 2,048 bytes; unique per account                 |
| `favicon_content_type` | TEXT | Nullable; AVIF, JPEG, PNG, WebP, ICO, or Microsoft icon media type                                |
| `favicon_data`         | BLOB | Nullable cached icon bytes, 1 byte through 256 KiB                                                |
| `favicon_fetched_at`   | TEXT | Nullable RFC 3339 timestamp; present exactly when cached favicon media type and bytes are present |
| `created_at`           | TEXT | Required, RFC 3339 timestamp                                                                      |
| `updated_at`           | TEXT | Required, RFC 3339 timestamp                                                                      |

## `bookmark_library_categories`

Categories for the full Bookmarks product page. A global category is visible to every
authenticated account and may be mutated only through administrator routes. A personal category
belongs to one account.

| Column               | Type | Constraints                                                               |
| -------------------- | ---- | ------------------------------------------------------------------------- |
| `id`                 | TEXT | Primary key                                                               |
| `scope`              | TEXT | `global` or `personal`                                                    |
| `user_id`            | TEXT | Null for global; required user reference with cascade delete for personal |
| `created_by_user_id` | TEXT | Optional creator reference with null-on-delete                            |
| `name`               | TEXT | Required, trimmed length 1–80; case-insensitive unique within its scope   |
| `created_at`         | TEXT | Required, RFC 3339 timestamp                                              |
| `updated_at`         | TEXT | Required, RFC 3339 timestamp                                              |

Deleting a category cascades all of its bookmark-library items.

## `bookmark_library_items`

Square bookmark tiles grouped by `bookmark_library_categories`. Destinations are credential-free
HTTP or HTTPS URLs. Favicon and custom icon failures leave the item saved with a Lucide fallback.

| Column              | Type | Constraints                                                                        |
| ------------------- | ---- | ---------------------------------------------------------------------------------- |
| `id`                | TEXT | Primary key                                                                        |
| `category_id`       | TEXT | Required category reference with cascade delete                                    |
| `title`             | TEXT | Required, trimmed length 1–120                                                     |
| `url`               | TEXT | Required destination, up to 2,048 bytes; unique within one category                |
| `icon_kind`         | TEXT | `favicon`, `lucide`, or `custom`                                                   |
| `icon_value`        | TEXT | Null for favicon; supported Lucide name or credential-free HTTPS custom icon URL   |
| `icon_content_type` | TEXT | Nullable; AVIF, JPEG, PNG, WebP, ICO, or Microsoft icon media type                 |
| `icon_data`         | BLOB | Nullable cached remote icon bytes, 1 byte through 256 KiB                          |
| `icon_fetched_at`   | TEXT | Nullable RFC 3339 timestamp; present exactly with cached icon media type and bytes |
| `created_at`        | TEXT | Required, RFC 3339 timestamp                                                       |
| `updated_at`        | TEXT | Required, RFC 3339 timestamp                                                       |

## `dashboard_widgets`

Per-user widget instances and their persisted GridStack layout. The interface normalizes reading
order from the twelve-column coordinates after a move or resize. Layout updates are written
atomically so a resize or reorder cannot be only partially saved.

The `search` web search widget was removed in migration `036`; web search now lives in the global
command palette. The migration deletes placed instances, closes the reading-order gap they leave
behind, and drops `search` from the `kind` check.

Migration `049` removes the legacy `task-progress` kind and seeds one account-owned `streams`
widget with `config_json.placement = "utility_rail"`. That system widget stores separate Twitch and
Kick account lists for the dashboard's fixed right rail; legacy movable `streams` widgets remain
valid and retain their encrypted credentials.

| Column        | Type    | Constraints                                                        |
| ------------- | ------- | ------------------------------------------------------------------ |
| `id`          | TEXT    | Primary key                                                        |
| `user_id`     | TEXT    | Required, references `users` with cascade delete                   |
| `kind`        | TEXT    | Supported widget type, including the local `bible-verse` widget    |
| `workspace`   | INTEGER | Legacy partition identifier; new widgets use dashboard `0`         |
| `position`    | INTEGER | Zero-based dashboard reading order, bounded to 0–127               |
| `size`        | TEXT    | `compact`, `standard`, `wide`, or `full`                           |
| `grid_x`      | INTEGER | GridStack column offset, 0–11                                      |
| `grid_y`      | INTEGER | GridStack row offset, 0–255                                        |
| `grid_w`      | INTEGER | GridStack width, 1–12 columns                                      |
| `grid_h`      | INTEGER | GridStack height, 1–12 rows                                        |
| `config_json` | TEXT    | Valid JSON object containing non-secret per-instance configuration |
| `created_at`  | TEXT    | Required, RFC 3339 timestamp                                       |
| `updated_at`  | TEXT    | Required, RFC 3339 timestamp                                       |

## `widget_secrets`

Encrypted credentials for provider-backed widgets. This table is intentionally queried separately
from `dashboard_widgets` so credentials cannot be serialized into dashboard responses.

| Column       | Type | Constraints                                                     |
| ------------ | ---- | --------------------------------------------------------------- |
| `widget_id`  | TEXT | Primary key, references `dashboard_widgets` with cascade delete |
| `user_id`    | TEXT | Required, references `users` with cascade delete                |
| `ciphertext` | TEXT | XChaCha20-Poly1305 nonce and ciphertext, base64 encoded         |
| `updated_at` | TEXT | Required, RFC 3339 timestamp                                    |

## Kanban collaboration

Kanban uses its own `kanban_*` collaboration aggregate and does not reuse the legacy
`user_workspaces` dashboard partition. Access begins with an active workspace membership, then the
server resolves role grants plus per-member overrides. The permission vocabulary is the kan.bn
split: `view`, `create`, `edit`, and `delete` for boards, lists, cards, and comments; `view`,
`invite`, `edit`, and `remove` for members; and `view`, `edit`, `delete`, and `manage` for the
workspace. Admin grants are immutable. Member and Guest grants can be customized except that
workspace `manage` and `delete` remain admin-only. The final active workspace admin cannot be
removed or demoted.

### `kanban_workspaces`

| Column               | Type | Constraints                                 |
| -------------------- | ---- | ------------------------------------------- |
| `id`                 | TEXT | Primary key                                 |
| `name`               | TEXT | Required, trimmed length 1–80               |
| `description`        | TEXT | Required, up to 1,000 characters            |
| `created_by_user_id` | TEXT | Optional user reference with null-on-delete |
| `created_at`         | TEXT | Required, RFC 3339 timestamp                |
| `updated_at`         | TEXT | Required, RFC 3339 timestamp                |

### `kanban_workspace_members`

In-app invitations are rows with `status = invited`; targets must already exist in `users`.

| Column               | Type | Constraints                                                    |
| -------------------- | ---- | -------------------------------------------------------------- |
| `workspace_id`       | TEXT | Composite primary key, workspace reference with cascade delete |
| `user_id`            | TEXT | Composite primary key, user reference with cascade delete      |
| `role`               | TEXT | `admin`, `member`, or `guest`                                  |
| `status`             | TEXT | `invited` or `active`                                          |
| `invited_by_user_id` | TEXT | Optional inviter reference with null-on-delete                 |
| `created_at`         | TEXT | Required, RFC 3339 timestamp                                   |
| `updated_at`         | TEXT | Required, RFC 3339 timestamp                                   |

### `kanban_role_permissions` and `kanban_member_permissions`

| Table                       | Key                         | Purpose                                                     |
| --------------------------- | --------------------------- | ----------------------------------------------------------- |
| `kanban_role_permissions`   | workspace, role, permission | Required boolean grant for each role and all 24 permissions |
| `kanban_member_permissions` | workspace, user, permission | Optional boolean override, cascades with membership         |

### `kanban_boards`

New boards receive three ordered columns: `Todo`, `In Progress`, and `Finished`.

| Column               | Type    | Constraints                                    |
| -------------------- | ------- | ---------------------------------------------- |
| `id`                 | TEXT    | Primary key                                    |
| `workspace_id`       | TEXT    | Workspace reference with cascade delete        |
| `name`               | TEXT    | Required, trimmed length 1–120                 |
| `description`        | TEXT    | Required, up to 2,000 characters               |
| `visibility`         | TEXT    | `private` or `public`; both require membership |
| `archived`           | INTEGER | Required boolean                               |
| `position`           | INTEGER | Non-negative workspace order                   |
| `created_by_user_id` | TEXT    | Optional user reference with null-on-delete    |
| `created_at`         | TEXT    | Required, RFC 3339 timestamp                   |
| `updated_at`         | TEXT    | Required, RFC 3339 timestamp                   |

`kanban_board_favorites` has a composite `(board_id, user_id)` primary key and stores each user's
favorite boards independently.

### `kanban_columns` and `kanban_cards`

| Table            | Important columns                              | Constraints and behavior                                                                                             |
| ---------------- | ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `kanban_columns` | board, name, position                          | Name length 1–80; ordered within one board; deletion requires no active cards                                        |
| `kanban_cards`   | column, title, description, due date, position | Title length 1–240; Markdown source up to 100,000 characters; optional ISO date; archive timestamp preserves history |

Card moves rewrite source and destination positions in one transaction and may not cross boards.
`kanban_card_assignees` links cards to active workspace members. `kanban_labels` stores unique
board-scoped names with a six-digit `#RRGGBB` color. The legacy `accent`, `blue`, `amber`, `red`,
`violet`, and `gray` values remain readable for existing rows;
`kanban_card_labels` is the card-to-label join table.

### Card collaboration tables

| Table                    | Purpose and constraints                                                                 |
| ------------------------ | --------------------------------------------------------------------------------------- |
| `kanban_comments`        | User-attributed card comments, trimmed length 1–10,000; user deletion preserves content |
| `kanban_checklists`      | Ordered named checklists, name length 1–120                                             |
| `kanban_checklist_items` | Ordered checklist rows with a required title and completion boolean                     |
| `kanban_attachments`     | SQLite file bytes, safe name and MIME metadata, 1 byte through 10 MiB                   |
| `kanban_card_activity`   | Append-only actor, action, detail, and timestamp history for significant card changes   |

All collaboration rows cascade from their parent card. Attachment reads and mutations resolve the
parent card's workspace and effective permission before bytes are returned or changed.
