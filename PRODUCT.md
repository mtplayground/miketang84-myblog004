# Product Snapshot

## What this project is

`myblog004` is a PostgreSQL-backed, server-rendered blog application written in Rust. It exposes a public blog site plus a password-protected admin interface for managing posts.

## What it does today

- Serves a public homepage with paginated published posts
- Renders individual post pages by slug
- Renders tag listing pages
- Renders an About page from `content/about.md`
- Exposes `robots.txt`, `sitemap.xml`, and `rss.xml`
- Provides admin login/logout with signed cookie sessions
- Lets an authenticated admin create, edit, publish, unpublish, and delete posts
- Stores Markdown and pre-rendered sanitized HTML for posts
- Runs SQLx migrations automatically at startup
- Seeds the first admin from environment variables when the `admins` table is empty

## Core data model

- `admins`
- `posts`
- `tags`
- `post_tags`

Post state is explicit: posts are either `draft` or `published`.

## Architectural decisions

- Plain Axum app, not SPA or API-first
- Askama templates for HTML rendering
- SQLx + PostgreSQL for all persistent state
- `tower-sessions` signed cookie sessions for admin auth
- Markdown is rendered and sanitized on write, then stored as HTML for cheap reads
- App startup order is: load env config, connect DB, ping DB, run migrations, seed admin, start server

## Product conventions

- Public content only shows published posts; drafts remain admin-only
- Admin seeding is idempotent and never overwrites an existing admin
- Canonical URLs, sitemap entries, and RSS links come from `BLOG_BASE_URL`
- Static assets are served from `/static`
- Friendly HTML 404 and generic 500 pages are part of the product contract

## Operational expectations

- Required configuration lives in `BLOG_*` and `ADMIN_*` env vars
- The app listens on `0.0.0.0:8080` unless `BLOG_BIND_ADDR` overrides it
- Backups are expected to be handled with PostgreSQL-native tooling such as `pg_dump`

## Test coverage shape

The merged test suite includes repository tests, public-route integration tests, admin-route integration tests, and a full happy-path end-to-end flow against a real Postgres test database.
