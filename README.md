# nextaxum

Production-ready monorepo template: **Next.js 16** (frontend) + **Axum 0.8** (Rust backend) + **Supabase** (auth & database).

Deploy: Frontend on **Vercel**, Backend on **Railway**, DB on **Supabase**.

---

## Table of Contents

- [Architecture](#architecture)
- [Request Flow](#request-flow)
- [Directory Map](#directory-map)
- [Backend (Rust/Axum)](#backend-rustaxum)
- [Frontend (Next.js 16)](#frontend-nextjs-16)
- [Authentication Flow](#authentication-flow)
- [Database](#database)
- [Security](#security)
- [Performance](#performance)
- [Configuration](#configuration)
- [Local Development](#local-development)
- [Deployment](#deployment)
- [Design Decisions & Rationale](#design-decisions--rationale)
- [Official Documentation Sources](#official-documentation-sources)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Browser                              │
│                                                             │
│  ┌─────────────────────┐   ┌─────────────────────────────┐ │
│  │  Supabase JS Client │   │  Next.js Client Components  │ │
│  │  (auth only)        │   │  (React 19)                 │ │
│  └────────┬────────────┘   └──────────┬──────────────────┘ │
└───────────┼────────────────────────────┼────────────────────┘
            │                            │
            ▼                            ▼
┌───────────────────┐    ┌──────────────────────────────────┐
│   Supabase Auth   │    │   Next.js Server (Vercel)        │
│   (sign in/up)    │    │                                  │
│                   │    │   Server Components               │
│   - OAuth flows   │    │   Server Actions (mutations)      │
│   - JWT issuance  │    │   Proxy (token refresh)            │
│                   │    │   DAL (auth verification)          │
│                   │    │                                  │
└───────────────────┘    │   ┌──────────────────────────┐   │
                         │   │  API Client (server-only) │   │
                         │   │  forwards Supabase JWT    │   │
                         │   └────────────┬─────────────┘   │
                         └────────────────┼─────────────────┘
                                          │
                                          ▼
                         ┌──────────────────────────────────┐
                         │   Axum API (Railway)              │
                         │                                  │
                         │   Middleware stack:                │
                         │   → Request ID                    │
                         │   → Rate Limiting                 │
                         │   → JWT Verification              │
                         │   → Compression                   │
                         │   → Security Headers              │
                         │                                  │
                         │   Handlers → DB layer → sqlx      │
                         │                                  │
                         │   Moka in-memory cache            │
                         └────────────┬─────────────────────┘
                                      │
                                      ▼
                         ┌──────────────────────────────────┐
                         │   Supabase PostgreSQL             │
                         │   (port 6543 via PgBouncer)       │
                         └──────────────────────────────────┘
```

**Why this split?**
- Auth (sign in/up, OAuth) goes **directly to Supabase** from the browser — no backend hop needed.
- Data reads use **Server Components** — they call the Axum API server-to-server. No CORS, no token in browser.
- Data writes use **Server Actions** — same server-to-server path. The browser never talks to Axum.
- Axum handles business logic, validation, authorization (ownership checks), and caching.

---

## Request Flow

### Read (e.g., loading the dashboard)

```
1. Browser requests /dashboard
2. Next.js proxy runs (proxy.ts, Node.js runtime):
   - Creates Supabase SSR client
   - Calls getClaims() to refresh the session token (no DB call)
   - If unauthenticated → optimistic redirect to /login
3. DashboardPage (Server Component) renders:
   - Calls verifySession() from DAL (the real security layer)
   - verifySession() uses getUser() which validates against Supabase
   - Suspense boundary wraps <ItemsLoader>
   - Shell (header + logout) streams immediately to browser
4. ItemsLoader (async Server Component):
   - Calls getItems() from lib/api/items.ts
   - getItems() uses React.cache() for deduplication
   - api() in client.ts extracts Supabase JWT from cookies
   - fetch() calls Axum GET /api/items with Bearer token
5. Axum receives request:
   - SetRequestIdLayer assigns X-Request-Id UUID
   - RateLimitLayer checks global limit
   - require_auth middleware verifies JWT (HS256, audience "authenticated")
   - AuthUser extractor reads claims from extensions
   - list_items handler checks moka cache (first page only)
   - If cache miss → sqlx cursor-paginated query
   - Returns PaginatedResponse JSON
6. Next.js receives response, renders ItemsList, streams to browser
```

### Write (e.g., adding an item)

```
1. User submits form in ItemsList (Client Component)
2. Calls addItem() Server Action
3. Server Action:
   - Validates title with zod (min 1, max 255, trimmed)
   - Calls createItem() which calls api("/api/items", { method: "POST" })
   - api() extracts JWT from cookies, sends to Axum
4. Axum receives POST /api/items:
   - Same auth middleware chain
   - ValidatedJson<CreateItem> extractor deserializes + validates with validator crate
   - db::create_item inserts via sqlx with RETURNING
   - items_cache.invalidate(user.id) clears cached list
   - Returns 201 + Item JSON
5. Server Action calls revalidatePath("/dashboard")
6. Next.js re-renders the dashboard Server Component with fresh data
```

---

## Directory Map

```
nextaxum/
├── .env.example                      # All env vars documented by service
├── .gitignore
├── .github/workflows/ci.yml          # PR/push: frontend lint + backend clippy
├── Makefile                           # dev, build, lint, test, migrate, docker-up
├── docker-compose.yml                 # Local dev: backend + frontend + migrate
│
├── backend/                           # Rust Axum API
│   ├── Cargo.toml                     # Dependencies with pinned features
│   ├── Dockerfile                     # Multi-stage, non-root, healthcheck
│   ├── railway.toml                   # Railway deploy config
│   ├── migrations/
│   │   ├── 20240101000000_create_items.sql       # Schema + initial indexes
│   │   └── 20240102000000_improve_indexes.sql    # Composite index for pagination
│   └── src/
│       ├── main.rs           # Entry: tracing, config, state, serve + graceful shutdown
│       ├── config.rs         # Env vars → typed Config struct with defaults
│       ├── state.rs          # AppState: PgPool + moka Cache + Config (Arc-wrapped)
│       ├── error.rs          # AppError enum → HTTP status + JSON response
│       ├── models/mod.rs     # Item, CreateItem, UpdateItem, PaginationParams, PaginatedResponse
│       ├── db/mod.rs         # sqlx queries: cursor pagination, CRUD, ownership filtering
│       ├── routes/
│       │   ├── mod.rs        # Router: middleware stack (rate limit, CORS, headers, compression)
│       │   ├── health.rs     # GET /health → SELECT 1 → {"status":"ok"}
│       │   └── items.rs      # CRUD /api/items with cache, ETag, pagination
│       ├── middleware/
│       │   └── auth.rs       # JWT verification: HS256, audience, claims → extensions
│       └── extractors/
│           ├── auth_user.rs  # AuthUser: extracts Claims from request extensions
│           └── validated.rs  # ValidatedJson<T>: deserialize + validate → 400 on failure
│
└── frontend/                          # Next.js 16 App Router
    ├── package.json                   # next 16.2, react 19, supabase-ssr, zod
    ├── tsconfig.json                  # Strict, bundler resolution, @/* alias
    ├── next.config.ts                 # standalone output, CSP, HSTS, typed routes
    ├── Dockerfile                     # Multi-stage, non-root nodejs user
    └── src/
        ├── proxy.ts                   # Token refresh + optimistic route protection
        ├── lib/
        │   ├── dal.ts                 # Data Access Layer: verifySession() — real auth boundary
        │   ├── env.ts                 # Server-only env validation (throws if missing)
        │   ├── env.client.ts          # Client-safe env (NEXT_PUBLIC_ only)
        │   ├── api/
        │   │   ├── client.ts          # server-only fetch wrapper: JWT from cookies → Bearer
        │   │   └── items.ts           # Typed API functions with React.cache() dedup
        │   └── supabase/
        │       ├── server.ts          # SSR Supabase client (cookie-based)
        │       ├── browser.ts         # Browser Supabase client
        │       └── proxy.ts           # Token refresh helper for Next.js proxy
        └── app/
            ├── layout.tsx             # Root layout + globals.css import
            ├── globals.css            # CSS reset (box-sizing, margin, system-ui font)
            ├── page.tsx               # Home page with nav links
            ├── error.tsx              # Global error boundary
            ├── not-found.tsx          # 404 page
            ├── login/page.tsx         # Email/password auth via Supabase browser client
            └── dashboard/
                ├── page.tsx           # Server Component: auth check, Suspense streaming
                ├── items-list.tsx     # Client Component: form + list + delete
                ├── actions.ts         # Server Actions: addItem, removeItem, logout (zod)
                ├── loading.tsx        # Suspense fallback skeleton
                └── error.tsx          # Dashboard-scoped error boundary
```

---

## Backend (Rust/Axum)

### Middleware Stack

Middleware executes in this order (outermost first):

| Order | Layer | Purpose |
|-------|-------|---------|
| 1 | `SetRequestIdLayer` | Generates UUID `X-Request-Id` header |
| 2 | `PropagateRequestIdLayer` | Copies request ID to response |
| 3 | `CatchPanicLayer` | Catches panics, returns 500 instead of crashing |
| 4 | `TraceLayer` | Structured JSON logging of request/response |
| 5 | `RateLimitLayer` | Global: 100 requests per 10 seconds |
| 6 | `TimeoutLayer` | Configurable (default 30s) request timeout |
| 7 | `CompressionLayer` | gzip, brotli, deflate, zstd response compression |
| 8 | `RequestBodyLimitLayer` | Configurable (default 2MB) body size limit |
| 9 | `CorsLayer` | Origin = FRONTEND_URL, credentials = true |
| 10 | Security headers | nosniff, DENY frame, HSTS, CSP, referrer-policy |

On protected routes (`/api/*`), `require_auth` runs as a `route_layer` between the router and handlers.

### Error Handling Pattern

All handlers return `AppResult<T>` which is `Result<T, AppError>`. The `AppError` enum implements `IntoResponse`:

| Variant | HTTP Status | When |
|---------|------------|------|
| `NotFound(msg)` | 404 | Item doesn't exist or wrong user_id |
| `Validation(msg)` | 400 | ValidatedJson or manual validation failure |
| `Unauthorized` | 401 | Missing/invalid JWT |
| `Forbidden` | 403 | Valid JWT but insufficient permissions |
| `Conflict(msg)` | 409 | Duplicate or constraint violation |
| `Sqlx(RowNotFound)` | 404 | sqlx returned no rows |
| `Sqlx(other)` | 500 | DB error (logged, not exposed to client) |
| `Internal(anyhow)` | 500 | Any other error (logged, not exposed) |

All errors return `{"error": "message"}` JSON. Internal details are never exposed to the client.

### Custom Extractors

- **`AuthUser`**: Reads `Claims` from request extensions (set by auth middleware). Use `user.id()` to get the Supabase user UUID. Fails with 401 if claims are missing.
- **`ValidatedJson<T>`**: Deserializes JSON body into `T`, then runs `T::validate()` from the `validator` crate. Fails with 400 and validation details.

### Pagination

Cursor-based using `(created_at, id)` tuples:

```
GET /api/items?limit=20
→ { "data": [...], "next_cursor": "2024-01-15T10:30:00Z,550e8400-...", "has_more": true }

GET /api/items?limit=20&cursor=2024-01-15T10:30:00Z,550e8400-...
→ { "data": [...], "next_cursor": null, "has_more": false }
```

**Why cursor over offset?** Offset pagination breaks when data changes between pages (skipping or duplicating items). Cursor pagination is stable and uses the composite index `(user_id, created_at DESC, id DESC)` efficiently.

The `limit + 1` trick: we fetch one extra row to know if there are more pages without a separate COUNT query.

### Caching

`moka` in-memory async cache stores the first page of items per user_id:

- **Cache hit**: First page request with no cursor checks cache first.
- **Invalidation**: Any create/update/delete invalidates that user's cache entry.
- **TTL**: Configurable via `CACHE_TTL_SECS` (default 300s).
- **Capacity**: Configurable via `CACHE_MAX_CAPACITY` (default 10,000 entries).

Subsequent pages (with cursor) bypass the cache entirely.

### ETag Support

`GET /api/items/{id}` returns a weak ETag based on `updated_at`:

```
ETag: W/"1705312200000"
Cache-Control: private, max-age=0, must-revalidate
```

If the client sends `If-None-Match` matching the ETag, the server returns `304 Not Modified` with no body.

---

## Frontend (Next.js 16)

### Rendering Strategy

| Route | Rendering | Why |
|-------|-----------|-----|
| `/` | Static | No dynamic data |
| `/login` | Client Component | Interactive form, browser-side Supabase auth |
| `/dashboard` | Dynamic Server Component | User-specific data, `force-dynamic` |
| `/dashboard` items | Suspense-streamed | `<Suspense>` wraps `<ItemsLoader>` async component |

**Streaming**: The dashboard shell (header, user email, logout button) renders and streams to the browser immediately. The items list loads inside a `<Suspense>` boundary — the browser shows "Loading items..." until the Axum API responds.

### Server Actions

Defined in `dashboard/actions.ts` with `"use server"` directive:

| Action | Validation | Side Effect |
|--------|-----------|-------------|
| `addItem(title)` | zod: trim, min 1, max 255 | `revalidatePath("/dashboard")` |
| `removeItem(id)` | zod: valid UUID | `revalidatePath("/dashboard")` |
| `logout()` | none | `supabase.auth.signOut()` → `redirect("/login")` |

**Why zod in Server Actions?** Server Actions are callable from the client. Client-side `maxLength={255}` is bypassable. Zod validates on the server before the API call.

**Why no `router.refresh()`?** `revalidatePath()` in the Server Action already tells Next.js to re-render the page with fresh data. Adding `router.refresh()` would cause a redundant fetch.

### API Client (`lib/api/client.ts`)

Server-only module that:
1. Reads the Supabase session cookie (pattern: `sb-*-auth-token`)
2. Parses the JSON to extract `access_token`
3. Sends it as `Authorization: Bearer <token>` to the Axum backend
4. The backend URL comes from `API_URL` env var (server-only, never `NEXT_PUBLIC_`)

### Environment Variables

Two modules enforce the server/client boundary:

| Module | Scope | Variables |
|--------|-------|-----------|
| `lib/env.ts` | `server-only` | `NEXT_PUBLIC_SUPABASE_URL`, `NEXT_PUBLIC_SUPABASE_ANON_KEY`, `API_URL` |
| `lib/env.client.ts` | browser-safe | `NEXT_PUBLIC_SUPABASE_URL`, `NEXT_PUBLIC_SUPABASE_ANON_KEY` |

`env.ts` throws at startup if required vars are missing. Import it in server-only files instead of using `process.env!` directly.

**Why is `API_URL` not `NEXT_PUBLIC_API_URL`?** The API client runs only on the server (has `import "server-only"`). Using `NEXT_PUBLIC_` would leak the backend URL into the browser bundle, exposing infrastructure.

---

## Authentication Flow

### Sign In

```
1. User enters email/password on /login (Client Component)
2. Supabase browser client calls supabase.auth.signInWithPassword()
3. Supabase returns JWT tokens, @supabase/ssr stores them in cookies
4. router.push("/dashboard") triggers navigation
5. Next.js proxy refreshes session via getClaims() (no DB call)
6. Dashboard Server Component calls verifySession() (DAL) → getUser() validates
7. Dashboard loads with verified session
```

### Authenticated API Call

```
1. Server Component needs data → calls getItems()
2. getItems() → api("/api/items")
3. api() reads sb-{ref}-auth-token cookie
4. Parses JSON cookie value → extracts access_token
5. Sends to Axum as Authorization: Bearer <token>
6. Axum require_auth middleware:
   - Extracts "Bearer " prefix
   - Decodes JWT with SUPABASE_JWT_SECRET (HS256)
   - Validates exp (not expired) and aud ("authenticated")
   - Injects Claims into request extensions
7. AuthUser extractor reads Claims → handler gets user.id()
8. All DB queries filter by user_id for ownership
```

### Session Refresh (Proxy) + Auth Verification (DAL)

Next.js uses a **defense-in-depth** pattern with two layers:

**Layer 1 — Proxy (`src/proxy.ts`)** runs on every request:
1. Creates Supabase SSR client with request/response cookie handlers
2. Calls `getClaims()` — validates JWT and refreshes expired tokens (no DB call)
3. Updated tokens are written to response cookies
4. If unauthenticated and route is `/dashboard/*` → optimistic redirect to `/login`

**Layer 2 — DAL (`src/lib/dal.ts`)** called in Server Components/Actions:
1. `verifySession()` calls `getUser()` which validates against Supabase (DB call)
2. If invalid → hard redirect to `/login`
3. Uses `React.cache()` — multiple calls in one render are free

**Why two layers?** The proxy is fast (no DB call) but optimistic — it can be bypassed (see CVE-2025-29927). The DAL is the real security boundary. Neither alone is sufficient.

See: [Next.js Authentication Guide](https://nextjs.org/docs/app/guides/authentication)

---

## Database

### Schema

```sql
CREATE TABLE items (
    id          UUID PRIMARY KEY,           -- Generated in Rust via Uuid::new_v4()
    user_id     UUID NOT NULL,              -- From JWT claims.sub
    title       VARCHAR(255) NOT NULL,
    description TEXT,
    created_at  TIMESTAMPTZ DEFAULT now(),
    updated_at  TIMESTAMPTZ DEFAULT now()
);

-- Composite index for cursor pagination:
-- WHERE user_id = $1 ORDER BY created_at DESC, id DESC LIMIT $2
CREATE INDEX idx_items_user_id_created_at_id
    ON items (user_id, created_at DESC, id DESC);
```

### Connection Pooling

Supabase provides **PgBouncer** on port **6543** (transaction mode). The backend's sqlx pool sits in front of it:

```
Axum handlers → sqlx PgPool (max 20 conn) → PgBouncer (port 6543) → PostgreSQL
```

- `DATABASE_URL` should use port 6543 with `?pgbouncer=true` for the application.
- `DATABASE_URL_DIRECT` (port 5432) is for migrations only — PgBouncer doesn't support prepared statements needed by `sqlx migrate run`.

### sqlx Offline Mode

`sqlx::query_as!()` macros verify queries against the database schema **at compile time**. This requires either:
1. A live database connection during `cargo build` (via `DATABASE_URL`)
2. Pre-generated query metadata in `.sqlx/` directory (offline mode)

For CI and Railway builds (no DB access), use offline mode:
```bash
# Against a live dev database:
cargo sqlx prepare

# This creates .sqlx/ directory — commit it to git
# Then set SQLX_OFFLINE=true during builds
```

The Dockerfile sets `ENV SQLX_OFFLINE=true` in the builder stage.

---

## Security

### Headers (applied by both Axum and Next.js)

| Header | Value | Purpose |
|--------|-------|---------|
| `X-Content-Type-Options` | `nosniff` | Prevents MIME-type sniffing |
| `X-Frame-Options` | `DENY` | Prevents clickjacking |
| `X-XSS-Protection` | `0` | Disables buggy browser XSS filter |
| `Referrer-Policy` | `strict-origin-when-cross-origin` | Controls referrer leakage |
| `Permissions-Policy` | `camera=(), microphone=(), geolocation=()` | Disables device APIs |
| `Strict-Transport-Security` | `max-age=63072000; includeSubDomains; preload` | Forces HTTPS for 2 years |
| `Content-Security-Policy` | (see below) | Controls resource loading |

**CSP on Axum (API-only)**: `default-src 'none'; frame-ancestors 'none'` — the API serves JSON only, no resources needed.

**CSP on Next.js**: `default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self' <supabase-url>; frame-ancestors 'none'; base-uri 'self'; form-action 'self'`

### Measures Summary

| Measure | Where | How |
|---------|-------|-----|
| SQL injection | Backend | `sqlx::query_as!()` compile-time checked parameterized queries |
| JWT verification | Backend middleware | HS256 + audience + expiration validation |
| Ownership isolation | Backend DB layer | Every query filters `WHERE user_id = $1` |
| Rate limiting | Backend middleware | `RateLimitLayer` 100 req/10s global |
| Body size limit | Backend middleware | `RequestBodyLimitLayer` default 2MB |
| CORS | Backend middleware | Single origin (FRONTEND_URL), explicit methods/headers |
| Input validation (Rust) | Backend extractors | `validator` crate on all DTOs |
| Input validation (TS) | Frontend actions | `zod` schemas in Server Actions |
| Secret isolation | Frontend env | `API_URL` is server-only, `server-only` imports prevent leaks |
| Env validation | Frontend startup | `env.ts` throws if required vars missing |
| Panic recovery | Backend middleware | `CatchPanicLayer` prevents server crash |
| Error masking | Backend error handler | Internal errors return generic message, logged server-side |
| Request timeout | Backend middleware | Default 30s, configurable |

---

## Performance

| Technique | Where | Impact |
|-----------|-------|--------|
| Cursor pagination | Backend DB | O(1) page fetch vs O(n) offset. Uses composite index. |
| `limit + 1` trick | Backend items | Determines `has_more` without COUNT query |
| Composite index | Database | `(user_id, created_at DESC, id DESC)` matches query exactly |
| Moka cache | Backend state | First-page cache per user, TTL-based, invalidated on writes |
| ETag / 304 | Backend items | Single-item conditional GET saves bandwidth |
| Response compression | Backend middleware | gzip/brotli/zstd via tower-http |
| Connection pooling | Backend state | sqlx pool (2-20 conns) → PgBouncer → PostgreSQL |
| Suspense streaming | Frontend dashboard | Shell renders immediately, items stream later |
| React.cache() | Frontend API | Deduplicates identical fetches within one Server Component render |
| `force-dynamic` | Frontend dashboard | No stale cache for user-specific data |
| Turbopack | Frontend dev | `next dev --turbopack` for fast refresh |
| Release profile | Backend build | LTO fat, codegen-units=1, strip, panic=abort |
| Multi-stage Docker | Both | Small final images, cached dependency layers |

---

## Configuration

All configuration is via environment variables. See `.env.example` for the complete list.

### Backend Environment Variables

| Variable | Required | Default | Purpose |
|----------|----------|---------|---------|
| `DATABASE_URL` | Yes | — | PostgreSQL connection (pooled, port 6543) |
| `SUPABASE_JWT_SECRET` | Yes | — | JWT verification secret |
| `FRONTEND_URL` | No | `http://localhost:3000` | CORS allowed origin |
| `PORT` | No | — | Railway injects this |
| `BACKEND_PORT` | No | `8080` | Fallback if PORT not set |
| `RUST_LOG` | No | `backend=debug,tower_http=debug` | Log level filter |
| `DB_MAX_CONNECTIONS` | No | `20` | sqlx pool max |
| `DB_MIN_CONNECTIONS` | No | `2` | sqlx pool min |
| `REQUEST_TIMEOUT_SECS` | No | `30` | Request timeout |
| `BODY_LIMIT_BYTES` | No | `2097152` | Max request body (2MB) |
| `ITEMS_PAGE_SIZE` | No | `50` | Max items per page |
| `CACHE_TTL_SECS` | No | `300` | Moka cache TTL |
| `CACHE_MAX_CAPACITY` | No | `10000` | Moka cache max entries |

### Frontend Environment Variables

| Variable | Required | Scope | Purpose |
|----------|----------|-------|---------|
| `NEXT_PUBLIC_SUPABASE_URL` | Yes | Client + Server | Supabase project URL |
| `NEXT_PUBLIC_SUPABASE_ANON_KEY` | Yes | Client + Server | Supabase anon key (safe for client) |
| `API_URL` | No | Server only | Axum backend URL (default `http://localhost:8080`) |

---

## Local Development

### Prerequisites

- Rust 1.85+ (edition 2024)
- Node.js 22+
- A Supabase project (free tier works)
- sqlx-cli: `cargo install sqlx-cli --features postgres`

### Setup

```bash
# 1. Clone and configure
git clone https://github.com/DenisCDev/nextaxum.git
cd nextaxum
cp .env.example .env
# Edit .env with your Supabase credentials

# 2. Generate lockfiles
make lockfiles

# 3. Run migrations
make migrate

# 4. Generate sqlx offline cache (for CI builds)
make sqlx-prepare

# 5. Run backend (terminal 1)
make dev-backend

# 6. Run frontend (terminal 2)
make dev-frontend
```

Frontend: http://localhost:3000 | Backend: http://localhost:8080 | Health: http://localhost:8080/health

### With Docker

```bash
docker compose up --build
```

---

## Deployment

### Frontend → Vercel

1. Import the repo in Vercel dashboard
2. Set **Root Directory** to `frontend`
3. Framework Preset: **Next.js** (auto-detected)
4. Add environment variables:
   - `NEXT_PUBLIC_SUPABASE_URL`
   - `NEXT_PUBLIC_SUPABASE_ANON_KEY`
   - `API_URL` = your Railway backend URL (e.g., `https://nextaxum-backend-production.up.railway.app`)
5. Deploy — Vercel handles the rest

### Backend → Railway

1. Create a new project in Railway dashboard
2. Connect the GitHub repo
3. Set **Root Directory** to `backend` (or Railway reads `railway.toml`)
4. Add environment variables:
   - `DATABASE_URL` (Supabase pooled connection)
   - `SUPABASE_JWT_SECRET`
   - `FRONTEND_URL` = your Vercel URL (e.g., `https://nextaxum.vercel.app`)
5. Railway builds via Dockerfile, detects health check at `/health`
6. Railway injects `PORT` automatically

### Supabase

1. Create project at [supabase.com](https://supabase.com)
2. Go to Settings → API to find: Project URL, Anon Key, Service Role Key, JWT Secret
3. Go to Settings → Database for connection strings (pooled and direct)
4. Run migrations against the direct connection:
   ```bash
   DATABASE_URL="postgresql://...@db.xxx.supabase.co:5432/postgres" make migrate
   ```

### Environment Variables by Service

| Variable | Vercel | Railway | Supabase Dashboard |
|----------|--------|---------|-------------------|
| `NEXT_PUBLIC_SUPABASE_URL` | Yes | — | Settings → API |
| `NEXT_PUBLIC_SUPABASE_ANON_KEY` | Yes | — | Settings → API |
| `API_URL` | Yes | — | — |
| `DATABASE_URL` | — | Yes | Settings → Database |
| `SUPABASE_JWT_SECRET` | — | Yes | Settings → API |
| `FRONTEND_URL` | — | Yes | — |

---

## Design Decisions & Rationale

### Why Axum over Actix-web or Rocket?

Axum is built by the Tokio team, uses the Tower middleware ecosystem (composable, reusable), and has the most active community. It's the de facto standard for new Rust web projects.

See: [Tokio blog — Announcing Axum 0.8](https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0)

### Why cursor pagination over offset?

Offset pagination (`LIMIT 50 OFFSET 100`) re-scans skipped rows and breaks when data changes between pages. Cursor pagination (`WHERE (created_at, id) < ($cursor)`) uses the index directly and is stable under concurrent writes.

See: [Use The Index, Luke — Pagination](https://use-the-index-luke.com/no-offset)

### Why moka over Redis for caching?

This is a template — Redis adds operational complexity (another service to deploy/monitor). Moka is an in-memory async cache that works within a single process. For multi-instance deployments, replace with Redis.

See: [moka crate documentation](https://docs.rs/moka/latest/moka/)

### Why `proxy.ts` instead of `middleware.ts`?

Next.js 16 deprecated `middleware.ts` and replaced it with `proxy.ts`. The rename reflects a philosophical shift: the proxy sits at the network boundary and should only do lightweight checks (JWT validation, token refresh, optimistic redirects). It runs on the **Node.js runtime** (not Edge).

The real auth check happens in the **Data Access Layer** (`lib/dal.ts`) — a `verifySession()` function using `React.cache()` called from Server Components and Server Actions. This defense-in-depth pattern was adopted after CVE-2025-29927 showed that middleware-only auth could be bypassed.

See: [Next.js proxy.ts API Reference](https://nextjs.org/docs/app/api-reference/file-conventions/proxy), [Next.js Authentication Guide](https://nextjs.org/docs/app/guides/authentication)

### Why `getClaims()` instead of `getSession()` in the proxy?

Supabase docs explicitly warn: "Always use `getClaims()` to protect pages. Never trust `getSession()` inside server code such as Proxy." `getClaims()` validates the JWT signature every time. `getSession()` may return stale data from cookies without verification. `getUser()` is used in the DAL for full server-side validation.

See: [Supabase Server-Side Auth for Next.js](https://supabase.com/docs/guides/auth/server-side/nextjs)

### Why `server-only` imports?

Next.js can accidentally bundle server code into the client. The `server-only` package causes a build error if a server module is imported from a Client Component. This prevents secret leakage.

See: [Next.js Docs — server-only](https://nextjs.org/docs/app/building-your-application/rendering/composition-patterns#keeping-server-only-code-out-of-the-client-environment)

### Why `force-dynamic` on the dashboard?

The dashboard shows user-specific data. Without `force-dynamic`, Next.js might try to statically generate it or serve a cached version for a different user. `force-dynamic` ensures fresh data on every request.

See: [Next.js Docs — Route Segment Config](https://nextjs.org/docs/app/api-reference/file-conventions/route-segment-config#dynamic)

### Why zod in Server Actions?

Server Actions are POST endpoints callable from the client. Client-side HTML validation (`maxLength`, `required`) is bypassable. Zod provides server-side runtime validation with TypeScript type inference.

See: [Zod documentation](https://zod.dev/)

### Why `API_URL` instead of `NEXT_PUBLIC_API_URL`?

The API client (`lib/api/client.ts`) is marked `server-only`. Using `NEXT_PUBLIC_` prefix would expose the backend URL in the browser JavaScript bundle, leaking infrastructure details. Server-only env vars stay on the server.

See: [Next.js Docs — Environment Variables](https://nextjs.org/docs/app/building-your-application/configuring/environment-variables)

### Why Suspense for the items list?

Wrapping the data-fetching component in `<Suspense>` enables streaming — the dashboard shell (header, logout button) renders and sends to the browser immediately, while items load in the background. This improves Time to First Byte (TTFB) and perceived performance.

See: [Next.js Docs — Streaming](https://nextjs.org/docs/app/building-your-application/routing/loading-ui-and-streaming)

### Why ETag on single items?

For apps that poll or revisit items frequently, ETags avoid re-transferring unchanged data. The weak ETag is computed from `updated_at` timestamp — no extra hashing needed.

See: [MDN — ETag](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/ETag)

### Why edition 2024 in Cargo.toml?

Rust edition 2024 (stabilized in Rust 1.85, Feb 2025) includes ergonomic improvements like `gen` blocks and refined async traits. The Dockerfile pins `rust:1.85-slim` to ensure compatibility.

See: [Rust Edition Guide — 2024](https://doc.rust-lang.org/edition-guide/rust-2024/)

---

## Official Documentation Sources

These are the authoritative references for each technology at the versions used in this template. Consult them when modifying or extending the codebase.

### Rust / Backend

| Technology | Version | Documentation |
|-----------|---------|---------------|
| Axum | 0.8 | https://docs.rs/axum/0.8/axum/ |
| Tower HTTP | 0.6 | https://docs.rs/tower-http/0.6/tower_http/ |
| Tower | 0.5 | https://docs.rs/tower/0.5/tower/ |
| SQLx | 0.8 | https://docs.rs/sqlx/0.8/sqlx/ |
| SQLx CLI | — | https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md |
| Tokio | 1.x | https://docs.rs/tokio/1/tokio/ |
| Serde | 1.x | https://serde.rs/ |
| jsonwebtoken | 9 | https://docs.rs/jsonwebtoken/9/jsonwebtoken/ |
| validator | 0.19 | https://docs.rs/validator/0.19/validator/ |
| moka | 0.12 | https://docs.rs/moka/0.12/moka/ |
| thiserror | 2 | https://docs.rs/thiserror/2/thiserror/ |
| tracing | 0.1 | https://docs.rs/tracing/0.1/tracing/ |
| Rust Edition 2024 | 1.85+ | https://doc.rust-lang.org/edition-guide/rust-2024/ |

### Frontend

| Technology | Version | Documentation |
|-----------|---------|---------------|
| Next.js | 16.2 | https://nextjs.org/docs |
| Next.js App Router | — | https://nextjs.org/docs/app |
| React | 19 | https://react.dev/reference/react |
| TypeScript | 5.7 | https://www.typescriptlang.org/docs/ |
| Zod | 3 | https://zod.dev/ |
| Supabase JS | 2.x | https://supabase.com/docs/reference/javascript/ |
| Supabase SSR | 0.6 | https://supabase.com/docs/guides/auth/server-side/nextjs |
| ESLint | 9 | https://eslint.org/docs/latest/ |

### Infrastructure

| Technology | Documentation |
|-----------|---------------|
| Vercel | https://vercel.com/docs |
| Railway | https://docs.railway.com/ |
| Supabase | https://supabase.com/docs |
| Supabase Auth | https://supabase.com/docs/guides/auth |
| Supabase Database | https://supabase.com/docs/guides/database |
| PgBouncer (Supabase) | https://supabase.com/docs/guides/database/connecting-to-postgres#connection-pooler |
| Docker | https://docs.docker.com/ |
| GitHub Actions | https://docs.github.com/en/actions |

---

## Adding New Features

When extending this template, follow these patterns:

### Adding a new API resource (e.g., "projects")

1. **Model**: Add structs to `backend/src/models/mod.rs` (or create `models/projects.rs`)
2. **Migration**: Create `migrations/YYYYMMDD_create_projects.sql`
3. **DB layer**: Add query functions to `backend/src/db/` (use `query_as!` for compile-time checks)
4. **Route**: Create `backend/src/routes/projects.rs`, define router, add to `routes/mod.rs`
5. **Frontend API**: Add `frontend/src/lib/api/projects.ts` with typed functions
6. **Page**: Create `frontend/src/app/projects/page.tsx` (Server Component)
7. **Actions**: Create `frontend/src/app/projects/actions.ts` with zod validation
8. Run `cargo sqlx prepare` to update offline cache

### Adding a new protected route (frontend)

1. Create the page under `src/app/`
2. Call `verifySession()` from `@/lib/dal` at the top of the Server Component (this is the real auth check)
3. Optionally add the path to the proxy redirect in `src/lib/supabase/proxy.ts` (optimistic, not security)
4. Add `loading.tsx` and `error.tsx` siblings

### Replacing moka with Redis

1. Add `deadpool-redis` to Cargo.toml
2. Add `redis_url` to `Config`
3. Replace `items_cache: Cache<Uuid, Vec<Item>>` with `redis: deadpool_redis::Pool` in `AppStateInner`
4. Update cache reads/writes in `routes/items.rs` to use Redis GET/SET with TTL
