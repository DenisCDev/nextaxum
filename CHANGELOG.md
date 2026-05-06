# Changelog

All notable changes to this template are tracked here. The format is loosely
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the
project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
once it cuts an initial release.

## [Unreleased]

### Added
- Supabase Storage avatar upload Server Action (`app/dashboard/avatar-upload.tsx`) +
  private `avatars` bucket with own-row RLS (migration 20240108).
- TOTP MFA enrollment page at `/dashboard/mfa` using `auth.mfa.enroll`/
  `verify`.
- OAuth (Google) callback at `/auth/callback` (PKCE).
- Realtime subscription on the items dashboard (postgres_changes filtered
  by `user_id`); `items` added to `supabase_realtime` publication.
- Profile shadow table (`public.profiles`) with `handle_new_user()` trigger;
  `GET`/`PUT /api/profile`.
- Idempotency-Key support on `POST /api/items` (Stripe pattern); 24h cron
  cleanup.
- HMAC-SHA256 webhook receiver at `POST /webhooks/{provider}` with
  constant-time compare and `(provider, event_id)` dedup.
- OpenAPI spec via utoipa + Swagger UI at `/docs`; `gen:api` script
  regenerates the typed Zod client.
- Vitest + React Testing Library for unit tests; Playwright for E2E.
- sqlx::test integration tests behind a Postgres service in CI.
- Daily `cargo audit` workflow with auto-issue filing.
- OpenTelemetry exporter behind the `otel` feature flag.
- Background cron loop with shared CancellationToken (`jobs/`).
- Lefthook pre-commit + pre-push hooks.
- Dependabot, issue/PR templates, CODEOWNERS, `.editorconfig`,
  `rustfmt.toml`, SECURITY.md, `.well-known/security.txt`.
- Local Supabase stack via `supabase/config.toml` (`make supabase-up`).
- vercel.json pinning the gru1 (São Paulo) region.

### Changed
- Frontend env validation moved to `@t3-oss/env-nextjs` + Zod; missing envs
  fail at build time.
- Backend `Config::from_env` returns `Result` and prints a precise error
  before exiting with code 2.
- `tower::limit::RateLimitLayer` (single shared bucket) replaced with
  `tower-governor` per-IP limit.
- `updated_at` is bumped by a `moddatetime` trigger; the app no longer
  passes `now()` on UPDATE.
- `/health` is now liveness-only; `/ready` probes DB + JWKS.
- Items handler tower stack migrated from `Router` to `OpenApiRouter` so
  the spec is generated compile-time.
- TraceLayer span includes `request_id`, correlating every per-request log.
- Login is a Server Action with `useActionState` + Zod (no client-side
  `signInWithPassword`).
- `api/client.ts` reads the access token via `supabase.auth.getSession()`
  instead of parsing the `sb-*-auth-token` cookie by hand.

### Fixed
- Backend `main.rs` referenced `config.port` after moving `config` into
  `AppState::new` — captured `port` before the move.
- Migrations now enable + force RLS on `items` with own-row policies; FK
  to `auth.users` is uncommented.
- Docker compose waits for `migrate: condition: service_completed_successfully`
  so the API never races an unfinished migration.
- Frontend `next lint` (removed in Next 16) replaced with ESLint flat
  config (`eslint.config.mjs`).
- Cookie callback typings in `supabase/server.ts` and `supabase/proxy.ts`
  no longer trigger `noImplicitAny` under `next build`.

### Security
- Asymmetric JWT verification (RS256/ES256/EdDSA) via cached JWKS, with
  HS256 fallback. Algorithm selected per-token from the JWT header.
- Pooler URLs (`:6543` or `pgbouncer=true`) detected at startup; sqlx's
  prepared-statement cache is disabled to avoid Supavisor transaction-mode
  errors.
- Direct (port 5432) connection recommended for the persistent backend in
  `.env.example`.
