<div align="center">

# Pandan

**A self-hosted, terminal-inspired command center for the things you follow and do.**

[![Version](https://img.shields.io/badge/version-0.1.0-8fd6a3?style=flat-square)](#)
[![Rust](https://img.shields.io/badge/Rust-2024-1f2321?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Svelte](https://img.shields.io/badge/Svelte-5-ff3e00?style=flat-square&logo=svelte&logoColor=white)](https://svelte.dev/)
[![SQLite](https://img.shields.io/badge/SQLite-persistent-21618c?style=flat-square&logo=sqlite&logoColor=white)](https://sqlite.org/)
[![Docker](https://img.shields.io/badge/Docker-ready-2496ed?style=flat-square&logo=docker&logoColor=white)](#docker)

</div>

Pandan brings planning, reading, notes, calendars, subscriptions, and software activity into one private dashboard. Its dense terminal aesthetic stays readable over customizable wallpapers, while a responsive GridStack canvas keeps the home view personal.

## What is inside

| Area              | Highlights                                                                                |
| ----------------- | ----------------------------------------------------------------------------------------- |
| **Dashboard**     | Movable widgets for weather, feeds, clocks, daily verses, releases, embeds, and more      |
| **Tasks**         | Priorities, labels, subtasks, attachments, recurrence, due dates, and an archived view    |
| **Contacts**      | Name-first sorting, editable portraits, yearless birthdays, Monica JSON import, and CardDAV sync |
| **Calendar**      | Multiple public ICS feeds, contact birthdays, source colors, month view, and daily agendas |
| **RSS & YouTube** | Compact reading queue, source filters, item details, read state, and retention controls   |
| **Journal**       | Nested Markdown documents that can contain both content and child documents               |
| **Coding**        | Releases, owned repositories, open PR counts, and GitLab pipeline status across providers |
| **Subscriptions** | Recurring service costs with per-currency daily, weekly, monthly, and yearly totals       |

Each account has isolated data, a private avatar, server-backed sidebar monitor timezones, a Welcome wallpaper shared by the session transition and authenticated pages, and a separate Loading wallpaper. Settings also provide explicitly confirmed, account-scoped cleanup controls for each content area. Administrators manage the public Login wallpaper and user directory.

## Quick start

Local development requires Rust 1.85+, Bun, Just, and Bacon.

```sh
just init
just setup
just dev
```

Open the UI at [localhost:5173](http://localhost:5173). Vite proxies `/api` to the Rust server on port `9651`. A fresh database starts with a one-time administrator setup.

### Useful commands

```sh
just check     # Rust and Svelte type checks
just test      # Rust test suite
just lint      # Clippy, ESLint, and Prettier
just ci        # Complete local quality gate
just build     # Static UI and release server
```

## Docker

```sh
just up-dev    # Containerized development with live reload
just up        # Production build on localhost:9651
```

Production persists SQLite in the `pandan-data` volume. Set `COOKIE_SECURE=true` behind HTTPS.

The `main` branch and `vMAJOR.MINOR.PATCH` tags publish container images to
`ghcr.io/nekoshinobi/pandan`.

## Configuration

Copy `.env.example` with `just init`. The main settings are:

| Variable                   | Use                                                                    |
| -------------------------- | ---------------------------------------------------------------------- |
| `DATABASE_URL`             | SQLite database URL                                                    |
| `PORT`                     | Rust server port; defaults to `9651`                                   |
| `COOKIE_SECURE`            | Require HTTPS session cookies                                          |
| `OIDC_*`                   | Optional standard OpenID Connect configuration                         |
| `PANDAN_WIDGET_SECRET_KEY` | Optional base64-encoded 32-byte key for encrypted provider credentials |
| `INVIDIOUS_BASE_URL`       | Optional public HTTPS instance used before YouTube's uploads feed      |

OIDC is enabled only when all required values are present. Generate the provider-credential key with `openssl rand -base64 32` and keep it stable across restarts.

When `INVIDIOUS_BASE_URL` is configured, YouTube channels are checked through that instance first and fall back to YouTube's public uploads feed. Channel video metadata is refreshed every two hours and shared across users; channel portraits are stored locally and refreshed at most every 24 hours. Failed portrait responses are never cached.

## Security

Pandan uses HTTP-only SameSite Strict sessions, Argon2id password hashing, account-scoped records, and encrypted integration secrets. Remote RSS, calendar, CardDAV, and custom Git-service requests reject private and reserved network destinations and enforce redirect, timeout, and response-size limits.

## Documentation

- [Database schema](SCHEMA.md)
- [AI and contributor guidance](AGENTS.md)

Pandan is under active development. Review the deployment configuration before exposing it publicly.
