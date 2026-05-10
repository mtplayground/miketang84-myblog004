# myblog004

`myblog004` is a server-rendered blog built with Rust, Axum, Askama, SQLx, and PostgreSQL. It serves public blog pages, an authenticated admin interface, RSS and sitemap endpoints, and runs database migrations automatically at startup.

## Stack

- Rust 2024 edition
- Axum for routing and HTTP
- Askama for HTML templates
- SQLx for PostgreSQL access and migrations
- `tower-sessions` signed cookie sessions for admin auth
- Argon2 for password hashing

## Features

- Public homepage with published posts and pagination
- Individual post pages and tag listing pages
- Markdown-backed About page
- RSS feed, `sitemap.xml`, and `robots.txt`
- Admin login, dashboard, and post create/edit/publish/delete flows
- Automatic admin seeding from environment variables
- Automatic SQLx migrations on boot

## Requirements

- Rust toolchain with `cargo`
- PostgreSQL 16+ recommended
- A PostgreSQL connection URL available as `BLOG_DATABASE_URL`

This app is PostgreSQL-only. It does not support SQLite, JSON files, or in-memory persistence.

## Build

```bash
cargo build
```

For release builds:

```bash
cargo build --release
```

## Configuration

The application reads configuration from environment variables at startup.

Copy the example file and fill in real values:

```bash
cp .env.example .env
```

Variables:

| Variable | Required | Default | Purpose |
| --- | --- | --- | --- |
| `BLOG_BIND_ADDR` | No | `0.0.0.0:8080` | Address and port the HTTP server binds to |
| `BLOG_LOG` | No | `info,myblog004=info,tower_http=info` | Tracing filter; falls back to `RUST_LOG` if unset |
| `BLOG_SESSION_SECRET` | Yes | None | Session signing secret; must be at least 32 bytes |
| `BLOG_DATABASE_URL` | Yes | None | PostgreSQL connection string used by SQLx |
| `BLOG_BASE_URL` | Yes | None | Absolute public site URL used for canonical links, RSS, and sitemap entries |
| `BLOG_TITLE` | Yes | None | Human-readable site title |
| `BLOG_RSS_LIMIT` | No | `20` | Maximum number of published posts included in the RSS feed |
| `ADMIN_USERNAME` | Yes | None | Initial admin username used for startup seeding |
| `ADMIN_PASSWORD` | Yes | None | Initial admin password used for startup seeding |

Notes:

- If your Postgres proxy rejects TLS negotiation, add `sslmode=disable` to `BLOG_DATABASE_URL`.
- The app prefers `sslmode=prefer` when no explicit `sslmode` is present, then retries with `sslmode=disable` when it detects a proxy that rejects the SSL negotiation request.
- Keep `BLOG_BASE_URL` normalized to the public origin, for example `https://example.com/`.

## Run

Set the required environment variables and start the server:

```bash
export BLOG_DATABASE_URL='postgres://user:password@localhost:5432/myblog004'
export BLOG_BASE_URL='http://localhost:8080/'
export BLOG_TITLE='My Blog'
export BLOG_SESSION_SECRET='replace-with-at-least-32-random-bytes'
export ADMIN_USERNAME='admin'
export ADMIN_PASSWORD='change-me'

cargo run
```

The app listens on `0.0.0.0:8080` unless `BLOG_BIND_ADDR` overrides it.

## Startup behavior

On boot, the binary performs this sequence:

1. Initialize tracing from `BLOG_LOG`, then `RUST_LOG`, then a built-in info-level default.
2. Load and validate all required `BLOG_*` and `ADMIN_*` variables.
3. Connect to PostgreSQL using `BLOG_DATABASE_URL`.
4. Run a health-check query (`SELECT 1`).
5. Run embedded SQLx migrations from `migrations/`.
6. Seed the first admin from `ADMIN_USERNAME` and `ADMIN_PASSWORD` if the `admins` table is empty.
7. Start the Axum HTTP server.

If an admin row already exists, seeding is skipped and startup continues normally.

## Admin seeding

Admin seeding is intentionally idempotent:

- If the `admins` table is empty, startup hashes `ADMIN_PASSWORD` with Argon2 and inserts one admin row.
- If any admin already exists, the app does not overwrite credentials or create another row.

Operationally, this means:

- `ADMIN_USERNAME` and `ADMIN_PASSWORD` are required even after the first boot.
- Changing those values later does not rotate the stored password automatically.
- Password rotation should be handled through an explicit database/admin workflow, not by changing env vars and restarting.

## Migrations

Migrations live in `migrations/` and are embedded with `sqlx::migrate!()`. They run automatically during application startup.

Manual migration command, if you need it during development:

```bash
sqlx migrate run
```

Important behavior:

- Migrations run before the server starts accepting traffic.
- A failed migration fails the process startup.
- Startup is safe to repeat; already-applied migrations are not re-applied.

## Development workflow

Typical local workflow:

```bash
cargo build
cargo test
cargo run
```

Targeted test runs are also supported, for example:

```bash
cargo test --test public_routes
cargo test --test admin_routes
cargo test --test e2e_happy_path
```

## Routes

Public routes:

- `/`
- `/about`
- `/posts/:slug`
- `/tags/:tag`
- `/rss.xml`
- `/sitemap.xml`
- `/robots.txt`

Admin routes:

- `/admin/login`
- `/admin`
- `/admin/posts/new`
- `/admin/posts/:id/edit`
- `/admin/posts/:id/publish`
- `/admin/posts/:id/unpublish`
- `/admin/posts/:id/delete`

## PostgreSQL backup and restore

Backups should be taken with PostgreSQL-native tooling.

Logical backup with `pg_dump`:

```bash
pg_dump "$BLOG_DATABASE_URL" --format=custom --file myblog004.dump
```

Plain SQL backup:

```bash
pg_dump "$BLOG_DATABASE_URL" --format=plain --file myblog004.sql
```

Restore from a custom-format backup:

```bash
createdb myblog004_restore
pg_restore --clean --if-exists --no-owner --dbname myblog004_restore myblog004.dump
```

Restore from a plain SQL backup:

```bash
createdb myblog004_restore
psql myblog004_restore < myblog004.sql
```

Recommended backup policy:

- Take scheduled logical backups with `pg_dump`
- Store backups outside the application host
- Periodically test restore into a separate database
- Capture backups before risky schema or data maintenance

For managed PostgreSQL providers, prefer their snapshot tooling in addition to `pg_dump`, not instead of it.

## Operations checklist

- Set all required env vars before first boot.
- Use a strong random `BLOG_SESSION_SECRET`.
- Confirm `BLOG_BASE_URL` matches the public origin exactly.
- Monitor startup logs for migration or seeding failures.
- Back up Postgres before manual maintenance.
- Treat the admin credentials in the environment as secrets.

## Notes

- Static assets are served from `/static`.
- The About page source lives at `content/about.md`.
- Public error pages return friendly 404 and generic 500 HTML responses.
