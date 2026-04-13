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
- [SQL Query Performance & RLS Security Guide](#sql-query-performance--rls-security-guide)
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

## SQL Query Performance & RLS Security Guide

> **Goal**: World-class PostgreSQL performance and security. Every technique below is sourced from official PostgreSQL docs, Supabase docs, and battle-tested production patterns.

### Table of Contents (this section)

- [EXPLAIN ANALYZE — Reading Query Plans](#explain-analyze--reading-query-plans)
- [Index Strategy](#index-strategy)
- [Query Optimization Patterns](#query-optimization-patterns)
- [Row Level Security (RLS)](#row-level-security-rls)
- [RLS Performance Optimization](#rls-performance-optimization)
- [RLS Security Hardening](#rls-security-hardening)
- [Connection Pooling Deep Dive](#connection-pooling-deep-dive)
- [How This Template Applies These Principles](#how-this-template-applies-these-principles)
- [RLS Decision: Application-Layer vs Database-Layer](#rls-decision-application-layer-vs-database-layer)
- [Checklist: Adding a New Table](#checklist-adding-a-new-table)

---

### EXPLAIN ANALYZE — Reading Query Plans

Always profile queries before and after optimization. Use `EXPLAIN (ANALYZE, BUFFERS)` to see actual execution stats.

```sql
-- Basic: shows estimated plan
EXPLAIN SELECT * FROM items WHERE user_id = 'abc' ORDER BY created_at DESC LIMIT 20;

-- Full: executes query and shows actual time, rows, buffer I/O
EXPLAIN (ANALYZE, BUFFERS) SELECT * FROM items
WHERE user_id = '550e8400-e29b-41d4-a716-446655440000'
ORDER BY created_at DESC, id DESC
LIMIT 21;
```

**How to read the output:**

| Field | Meaning |
|-------|---------|
| `Seq Scan` | Full table scan — bad for large tables, needs an index |
| `Index Scan` | Uses index to find rows — good for selective queries |
| `Index Only Scan` | Answers query entirely from index — best possible |
| `Bitmap Index Scan` | Two-phase: index finds locations, then heap fetches — good for moderate selectivity |
| `cost=startup..total` | Arbitrary units (1.0 = one sequential page read) |
| `rows=N` | Estimated output rows (not scanned rows) |
| `actual time=start..end` | Real milliseconds |
| `loops=N` | Times this node executed — multiply by time for true cost |
| `Buffers: shared hit=N` | Pages found in RAM cache (good) |
| `Buffers: shared read=N` | Pages read from disk (minimize this) |
| `Rows Removed by Filter` | Rows scanned but discarded — high = needs better index |

**This template's pagination query** should show `Index Scan using idx_items_user_id_created_at_id` with zero `Rows Removed by Filter`. If you see `Seq Scan`, the index is missing or the planner chose to ignore it (run `ANALYZE items;` to update statistics).

> **Source**: [PostgreSQL Docs — Using EXPLAIN](https://www.postgresql.org/docs/current/using-explain.html)

---

### Index Strategy

#### Index Types — When to Use Each

| Type | Best For | Operators | Example |
|------|----------|-----------|---------|
| **B-tree** (default) | Equality, range, sorting, LIKE 'prefix%' | `< <= = >= > BETWEEN IN IS NULL` | `CREATE INDEX idx ON t (col);` |
| **Hash** | Equality only, large values | `=` | `CREATE INDEX idx ON t USING hash (col);` |
| **GIN** | Arrays, JSONB, full-text search | `@> <@ && ?` | `CREATE INDEX idx ON t USING gin (jsonb_col);` |
| **GiST** | Geometry, ranges, nearest-neighbor | `@> <@ && <<` | `CREATE INDEX idx ON t USING gist (geo_col);` |
| **BRIN** | Very large tables, naturally ordered data (timestamps) | `< <= = >= >` | `CREATE INDEX idx ON t USING brin (created_at);` |
| **SP-GiST** | Quadtrees, k-d trees, radix trees | varies | `CREATE INDEX idx ON t USING spgist (col);` |

> **Source**: [PostgreSQL Docs — Index Types](https://www.postgresql.org/docs/current/indexes-types.html)

#### Composite Indexes — Column Order Matters

For B-tree composite indexes, **leftmost columns are most selective**. The index `(user_id, created_at DESC, id DESC)` supports:

```sql
-- ✅ Uses index fully (equality on col1, range on col2+col3)
WHERE user_id = $1 ORDER BY created_at DESC, id DESC

-- ✅ Uses index (equality on col1 only)
WHERE user_id = $1

-- ❌ Cannot use index efficiently (missing col1)
WHERE created_at > '2024-01-01'
```

**Rule**: Equality columns first, then range/sort columns, in the same order as your `ORDER BY`.

> **Source**: [PostgreSQL Docs — Multicolumn Indexes](https://www.postgresql.org/docs/current/indexes-multicolumn.html)

#### Covering Indexes (INCLUDE) — Index-Only Scans

Add payload columns with `INCLUDE` so PostgreSQL can answer queries entirely from the index, avoiding heap access:

```sql
-- Covering index: title is payload, not a search key
CREATE INDEX idx_items_covering
    ON items (user_id, created_at DESC, id DESC)
    INCLUDE (title, description);
```

This enables **Index Only Scan** for queries selecting `id, title, description` with the right `WHERE`/`ORDER BY`. Trade-off: larger index size, slower writes.

> Use `INCLUDE` only for columns frequently selected but never filtered/sorted on. Only B-tree, GiST, and SP-GiST support it.

> **Source**: [PostgreSQL Docs — Index-Only Scans and Covering Indexes](https://www.postgresql.org/docs/current/indexes-index-only-scans.html)

#### Partial Indexes — Index Only What You Query

```sql
-- Only index active items — smaller, faster
CREATE INDEX idx_items_active ON items (user_id, created_at DESC)
    WHERE deleted_at IS NULL;

-- Only index unprocessed orders — dramatically smaller
CREATE INDEX idx_orders_pending ON orders (created_at)
    WHERE status = 'pending';
```

**When to use**: When queries consistently filter on a fixed condition and most rows don't match it.

> **Source**: [PostgreSQL Docs — Partial Indexes](https://www.postgresql.org/docs/current/indexes-partial.html)

#### BRIN for Time-Series Data

For append-only or rarely-updated tables with monotonically increasing timestamps, BRIN indexes are **10x+ smaller** than B-tree:

```sql
-- BRIN: stores min/max per 128-page block range (default)
CREATE INDEX idx_items_created_brin ON items USING brin (created_at);
```

Trade-off: less precise than B-tree (may scan extra blocks), but dramatically smaller. Best for audit logs, event tables, and analytics data.

> **Source**: [Supabase Docs — Query Optimization](https://supabase.com/docs/guides/database/query-optimization)

#### Anti-Patterns

| Anti-Pattern | Why It's Bad | Fix |
|-------------|-------------|-----|
| `SELECT *` | Fetches unnecessary columns, prevents index-only scans | Select only needed columns |
| Missing indexes on foreign keys | Slow JOINs and CASCADE deletes | `CREATE INDEX idx ON child (parent_id);` |
| Over-indexing | Slows writes, wastes storage, confuses planner | Only index columns in WHERE/JOIN/ORDER BY |
| Function in WHERE | `WHERE LOWER(email) = $1` can't use B-tree on `email` | Use expression index: `CREATE INDEX idx ON t (LOWER(email));` |
| Implicit type casting | `WHERE int_col = '123'` may prevent index use | Match types exactly |
| Not running ANALYZE | Planner uses stale statistics → bad plans | `ANALYZE table_name;` after bulk changes |

---

### Query Optimization Patterns

#### 1. Cursor Pagination (used in this template)

```sql
-- ✅ O(1) per page via index — stable under concurrent writes
SELECT id, user_id, title, description, created_at, updated_at
FROM items
WHERE user_id = $1
  AND (created_at, id) < ($2, $3)   -- cursor condition
ORDER BY created_at DESC, id DESC
LIMIT $4;

-- ❌ O(n) offset — rescans skipped rows, unstable
SELECT * FROM items ORDER BY created_at DESC OFFSET 1000 LIMIT 50;
```

> **Source**: [Use The Index, Luke — No Offset](https://use-the-index-luke.com/no-offset)

#### 2. LIMIT + 1 Trick (used in this template)

Fetch `limit + 1` rows. If you get more than `limit`, there are more pages — no separate `COUNT(*)` needed:

```sql
-- Fetch 21 rows to know if page 2 exists, return only 20 to client
SELECT ... LIMIT $limit + 1;
```

#### 3. COALESCE for Partial Updates (used in this template)

```sql
-- Only update fields the client sent (non-NULL), keep existing values for the rest
UPDATE items
SET title = COALESCE($3, title),
    description = COALESCE($4, description),
    updated_at = now()
WHERE id = $1 AND user_id = $2
RETURNING *;
```

#### 4. Avoid N+1 Queries

```sql
-- ❌ N+1: one query per item
for item in items:
    SELECT * FROM tags WHERE item_id = item.id;

-- ✅ Batch: one query for all
SELECT * FROM tags WHERE item_id = ANY($1::uuid[]);
```

In sqlx/Rust:

```rust
let ids: Vec<Uuid> = items.iter().map(|i| i.id).collect();
sqlx::query_as!(Tag, "SELECT * FROM tags WHERE item_id = ANY($1)", &ids)
    .fetch_all(pool).await?;
```

#### 5. Materialized Views for Expensive Aggregations

```sql
CREATE MATERIALIZED VIEW user_stats AS
SELECT user_id, COUNT(*) as item_count, MAX(created_at) as last_item
FROM items
GROUP BY user_id;

-- Refresh periodically (not on every write)
REFRESH MATERIALIZED VIEW CONCURRENTLY user_stats;

-- Index the materialized view
CREATE UNIQUE INDEX idx_user_stats_uid ON user_stats (user_id);
```

> **Source**: [PostgreSQL Docs — Materialized Views](https://www.postgresql.org/docs/current/rules-materializedviews.html)

#### 6. Compile-Time Checked Queries (used in this template)

sqlx's `query_as!()` macro verifies SQL against the database schema at compile time — column names, types, and query validity are all checked before the code compiles:

```rust
// Typo in column name? Won't compile.
// Wrong type? Won't compile.
// Missing column? Won't compile.
sqlx::query_as!(Item,
    "SELECT id, user_id, title, description, created_at, updated_at
     FROM items WHERE id = $1 AND user_id = $2",
    id, user_id
).fetch_optional(pool).await?;
```

This eliminates an entire class of runtime SQL errors. Combined with parameterized queries, this provides **zero SQL injection risk** with compile-time guarantees.

> **Source**: [SQLx Docs](https://docs.rs/sqlx/0.8/sqlx/)

---

### Row Level Security (RLS)

RLS adds `WHERE` clauses to every query at the database level. It's PostgreSQL's built-in mechanism for row-level authorization.

#### Fundamentals

```sql
-- 1. Enable RLS (required — without this, policies are ignored)
ALTER TABLE items ENABLE ROW LEVEL SECURITY;

-- 2. Without policies, RLS = default deny (no rows visible)
-- 3. Create policies to grant access
```

#### Policy Types

```sql
-- SELECT: controls which rows are visible (USING clause)
CREATE POLICY "Users see own items" ON items
    FOR SELECT TO authenticated
    USING ( (select auth.uid()) = user_id );

-- INSERT: validates new rows (WITH CHECK clause)
CREATE POLICY "Users create own items" ON items
    FOR INSERT TO authenticated
    WITH CHECK ( (select auth.uid()) = user_id );

-- UPDATE: USING filters existing rows, WITH CHECK validates changes
CREATE POLICY "Users update own items" ON items
    FOR UPDATE TO authenticated
    USING ( (select auth.uid()) = user_id )
    WITH CHECK ( (select auth.uid()) = user_id );

-- DELETE: USING clause only
CREATE POLICY "Users delete own items" ON items
    FOR DELETE TO authenticated
    USING ( (select auth.uid()) = user_id );
```

#### PERMISSIVE vs RESTRICTIVE

```sql
-- PERMISSIVE (default): combined with OR — expands access
CREATE POLICY "Users see own" ON items FOR SELECT
    USING (user_id = (select auth.uid()));

CREATE POLICY "Admins see all" ON items FOR SELECT
    USING ((select auth.jwt()->>'role') = 'admin');
-- Result: user sees own OR is admin

-- RESTRICTIVE: combined with AND — narrows access
CREATE POLICY "MFA required" ON items
    AS RESTRICTIVE FOR UPDATE TO authenticated
    USING ((select auth.jwt()->>'aal') = 'aal2');
-- Result: must pass a PERMISSIVE policy AND this restriction
```

**Combination formula**: `(permissive1 OR permissive2 ...) AND restrictive1 AND restrictive2 ...`

> **Source**: [PostgreSQL Docs — CREATE POLICY](https://www.postgresql.org/docs/current/sql-createpolicy.html), [PostgreSQL Docs — Row Security Policies](https://www.postgresql.org/docs/current/ddl-rowsecurity.html)

#### Auto-Enable RLS on New Tables

Prevent forgetting to enable RLS on future tables:

```sql
CREATE OR REPLACE FUNCTION rls_auto_enable()
RETURNS EVENT_TRIGGER
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog
AS $$
DECLARE cmd record;
BEGIN
  FOR cmd IN SELECT * FROM pg_event_trigger_ddl_commands()
    WHERE command_tag IN ('CREATE TABLE', 'CREATE TABLE AS')
      AND schema_name = 'public'
  LOOP
    EXECUTE format('ALTER TABLE IF EXISTS %s ENABLE ROW LEVEL SECURITY', cmd.object_identity);
    RAISE LOG 'rls_auto_enable: enabled RLS on %', cmd.object_identity;
  END LOOP;
END;
$$;

CREATE EVENT TRIGGER ensure_rls ON ddl_command_end
    WHEN TAG IN ('CREATE TABLE', 'CREATE TABLE AS')
    EXECUTE FUNCTION rls_auto_enable();
```

> **Source**: [Supabase Docs — Row Level Security](https://supabase.com/docs/guides/database/postgres/row-level-security)

---

### RLS Performance Optimization

RLS policies run on every row. Unoptimized policies can turn millisecond queries into seconds. These optimizations come from Supabase's official benchmarks.

#### 1. Wrap `auth.uid()` in SELECT — 94% faster

```sql
-- ❌ SLOW: auth.uid() called per row (179ms on 100k rows)
USING ( auth.uid() = user_id );

-- ✅ FAST: auth.uid() cached via initPlan (9ms on 100k rows)
USING ( (select auth.uid()) = user_id );
```

The `(select ...)` wrapper tells PostgreSQL to evaluate the function **once** and cache the result (initPlan), instead of calling it per-row.

> **Benchmark**: 179ms → 9ms (94.97% improvement). For security definer functions: 178,000ms → 12ms (99.993% improvement).

#### 2. Index Columns Used in Policies — 99% faster

```sql
-- Policy checks user_id → add B-tree index on user_id
CREATE INDEX idx_items_user_id ON items USING btree (user_id);
```

> **Benchmark**: 171ms → <0.1ms (99.94% improvement on 100k rows).

#### 3. Always Specify Roles with TO — 99% faster for anon

```sql
-- ❌ Applies to all roles including anon (runs policy for everyone)
CREATE POLICY "select" ON items FOR SELECT
    USING ( (select auth.uid()) = user_id );

-- ✅ Only runs for authenticated role — anon skips immediately
CREATE POLICY "select" ON items FOR SELECT
    TO authenticated
    USING ( (select auth.uid()) = user_id );
```

> **Benchmark**: When anon accesses: 170ms → <0.1ms (99.78% improvement).

#### 4. Minimize Joins in Policies — 99% faster

```sql
-- ❌ SLOW: joins source table to auth table (9,000ms)
USING (
  (select auth.uid()) IN (
    SELECT user_id FROM team_user WHERE team_user.team_id = team_id
  )
);

-- ✅ FAST: fetches user's teams first, then checks membership (20ms)
USING (
  team_id IN (
    SELECT team_id FROM team_user WHERE user_id = (select auth.uid())
  )
);
```

> **Benchmark**: 9,000ms → 20ms (99.78% improvement).

#### 5. Security Definer Functions for Complex Logic

For policies that check multiple tables, wrap in a `SECURITY DEFINER` function (bypasses RLS on the lookup table):

```sql
-- Function in private schema (never exposed to API)
CREATE FUNCTION private.user_has_role(required_role text)
RETURNS boolean
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  RETURN EXISTS (
    SELECT 1 FROM public.user_roles
    WHERE user_id = (select auth.uid())
      AND role = required_role
  );
END;
$$;

-- Policy uses the cached function result
CREATE POLICY "Editors can update" ON articles
    FOR UPDATE TO authenticated
    USING ( (select private.user_has_role('editor')) );
```

> **Critical**: Always put security definer functions in a **private schema** (not exposed via PostgREST/Supabase API). Always `SET search_path = ''` to prevent search path injection.

#### 6. Add Client-Side Filters Even with RLS

```javascript
// ❌ Relies only on RLS to filter (planner has less info)
const { data } = await supabase.from('items').select()

// ✅ Explicit filter helps planner create better execution plan
const { data } = await supabase.from('items').select().eq('user_id', userId)
```

> **Benchmark**: 171ms → 9ms (94.74% improvement).

#### Supabase Database Advisor Lint: `auth_rls_initplan`

Supabase's built-in **Security Advisor** (Dashboard → Database → Security Advisor) includes lint `0003_auth_rls_initplan` that automatically detects `auth.uid()` and `auth.jwt()` calls not wrapped in `(select ...)`. Enable this check and fix all warnings.

> **Source**: [Supabase — RLS Performance Best Practices](https://supabase.com/docs/guides/database/postgres/row-level-security), [Supabase — Performance Advisors](https://supabase.com/docs/guides/database/database-advisors), [GaryAustin1/RLS-Performance Benchmarks](https://github.com/GaryAustin1/RLS-Performance)

---

### RLS Security Hardening

#### 1. Never Trust `raw_user_meta_data` for Authorization

```sql
-- ❌ VULNERABLE: users can update their own raw_user_meta_data
USING ( auth.jwt()->'user_metadata'->>'role' = 'admin' );

-- ✅ SAFE: raw_app_meta_data is immutable by users
USING ( auth.jwt()->'app_metadata'->>'role' = 'admin' );
```

> `raw_user_meta_data` is editable by the user via `supabase.auth.updateUser()`. Only `raw_app_meta_data` (set via service role or admin API) is safe for authorization decisions.

#### 2. Handle NULL from `auth.uid()`

When no user is authenticated, `auth.uid()` returns `NULL`. Since `NULL = anything` is always `FALSE`, unauthenticated users get no rows — but be explicit:

```sql
-- Explicit NULL check (defense-in-depth)
USING ( auth.uid() IS NOT NULL AND (select auth.uid()) = user_id )
```

#### 3. Policies on Every Table — Default Deny

RLS with no policies = **no access** (default deny). But if you forget to enable RLS, the table is **fully public** via the Supabase API:

```sql
-- Check all tables without RLS enabled
SELECT schemaname, tablename
FROM pg_tables
WHERE schemaname = 'public'
  AND tablename NOT IN (
    SELECT tablename FROM pg_tables t
    JOIN pg_class c ON c.relname = t.tablename
    WHERE c.relrowsecurity = true
  );
```

Use the auto-enable trigger (above) and Supabase's Security Advisor to catch this.

#### 4. Don't Forget Junction Tables

```sql
-- If items has RLS but item_tags doesn't, attackers can read item_tags directly
ALTER TABLE item_tags ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users see own item tags" ON item_tags
    FOR SELECT TO authenticated
    USING (
      item_id IN (
        SELECT id FROM items WHERE user_id = (select auth.uid())
      )
    );
```

#### 5. Views Bypass RLS by Default

```sql
-- ❌ Views run as postgres user (superuser) — bypasses RLS
CREATE VIEW public_items AS SELECT * FROM items;

-- ✅ PostgreSQL 15+: security_invoker makes view respect caller's RLS
CREATE VIEW public_items
    WITH (security_invoker = true)
    AS SELECT * FROM items;
```

#### 6. RBAC Pattern with RLS

```sql
-- Roles table (set by admin, not user-editable)
CREATE TABLE public.user_roles (
    user_id UUID REFERENCES auth.users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('viewer', 'editor', 'admin')),
    PRIMARY KEY (user_id, role)
);
ALTER TABLE user_roles ENABLE ROW LEVEL SECURITY;

-- Hierarchical: admin > editor > viewer
CREATE FUNCTION private.user_has_minimum_role(min_role text)
RETURNS boolean LANGUAGE plpgsql SECURITY DEFINER SET search_path = '' AS $$
DECLARE role_rank int;
BEGIN
  SELECT CASE r.role
    WHEN 'admin' THEN 3
    WHEN 'editor' THEN 2
    WHEN 'viewer' THEN 1
    ELSE 0
  END INTO role_rank
  FROM public.user_roles r
  WHERE r.user_id = (select auth.uid())
  ORDER BY CASE r.role
    WHEN 'admin' THEN 3 WHEN 'editor' THEN 2 WHEN 'viewer' THEN 1 ELSE 0
  END DESC LIMIT 1;

  RETURN COALESCE(role_rank, 0) >= CASE min_role
    WHEN 'admin' THEN 3 WHEN 'editor' THEN 2 WHEN 'viewer' THEN 1 ELSE 0
  END;
END; $$;

-- Apply to tables
CREATE POLICY "Viewers can read" ON articles
    FOR SELECT TO authenticated
    USING ( (select private.user_has_minimum_role('viewer')) );

CREATE POLICY "Editors can modify" ON articles
    FOR UPDATE TO authenticated
    USING ( (select private.user_has_minimum_role('editor')) );
```

#### 7. Multi-Tenant RLS Pattern

```sql
-- org_members table links users to organizations
CREATE POLICY "Tenant isolation" ON items
    FOR ALL TO authenticated
    USING (
      org_id IN (
        SELECT org_id FROM org_members
        WHERE user_id = (select auth.uid())
      )
    )
    WITH CHECK (
      org_id IN (
        SELECT org_id FROM org_members
        WHERE user_id = (select auth.uid())
      )
    );
```

#### 8. Testing RLS Policies

```sql
-- Test as a specific role
SET ROLE authenticated;
SET request.jwt.claims = '{"sub": "user-uuid-here", "role": "authenticated"}';

-- Verify policy works
SELECT * FROM items;  -- Should only see user's items

-- Reset
RESET ROLE;
```

> **Source**: [PostgreSQL Docs — Row Security Policies](https://www.postgresql.org/docs/current/ddl-rowsecurity.html), [Supabase — Securing Your API](https://supabase.com/docs/guides/api/securing-your-api)

---

### Connection Pooling Deep Dive

```
                      Supabase Architecture
┌──────────────────────────────────────────────────────────┐
│  Axum (sqlx PgPool)                                      │
│  ├── min_connections: 2                                   │
│  ├── max_connections: 20                                  │
│  └── idle_timeout: 10min                                  │
│         │                                                 │
│         ▼                                                 │
│  Supavisor / PgBouncer (port 6543, transaction mode)      │
│  ├── Reuses server connections across client sessions     │
│  ├── Transaction mode: conn returned after each txn       │
│  └── ⚠ No prepared statements in transaction mode         │
│         │                                                 │
│         ▼                                                 │
│  PostgreSQL (port 5432)                                   │
│  └── max_connections: ~100 (varies by Supabase plan)      │
└──────────────────────────────────────────────────────────┘
```

**Key rules**:

| Rule | Why |
|------|-----|
| App connections < PgBouncer pool size | Prevents connection starvation |
| Use port 6543 for application queries | PgBouncer transaction pooling |
| Use port 5432 for migrations only | Migrations need prepared statements |
| Set `?pgbouncer=true` in DATABASE_URL | Disables named prepared statements (incompatible with transaction mode) |
| sqlx `PgPoolOptions::max_connections(20)` | Don't exceed Supabase connection limit |
| sqlx `PgPoolOptions::min_connections(2)` | Keep warm connections for low-latency first requests |

> **Source**: [Supabase — Connection Pooler](https://supabase.com/docs/guides/database/connecting-to-postgres#connection-pooler)

---

### How This Template Applies These Principles

| Principle | Implementation | File |
|-----------|---------------|------|
| **Zero SQL injection** | `sqlx::query_as!()` compile-time parameterized queries | `backend/src/db/mod.rs` |
| **Ownership isolation** | Every query includes `WHERE user_id = $1` | `backend/src/db/mod.rs` |
| **Optimal indexing** | Composite index `(user_id, created_at DESC, id DESC)` matches query exactly | `backend/migrations/` |
| **Cursor pagination** | `(created_at, id) < ($2, $3)` — O(1) page fetch | `backend/src/db/mod.rs` |
| **No COUNT(*)** | `LIMIT + 1` trick for `has_more` | `backend/src/routes/items.rs` |
| **Partial updates** | `COALESCE($3, title)` preserves unchanged fields | `backend/src/db/mod.rs` |
| **Connection pooling** | sqlx PgPool → PgBouncer (port 6543) → PostgreSQL | `backend/src/main.rs` |
| **JWT verification** | HS256 + audience + expiration, per-request | `backend/src/middleware/auth.rs` |
| **Input validation** | `validator` crate (Rust) + `zod` (TypeScript) | Extractors + Server Actions |
| **In-memory caching** | Moka async cache per user_id, TTL + write invalidation | `backend/src/routes/items.rs` |
| **ETag support** | `updated_at`-based weak ETag, 304 Not Modified | `backend/src/routes/items.rs` |

---

### RLS Decision: Application-Layer vs Database-Layer

This template uses **application-layer authorization** (Axum enforces `WHERE user_id = $1` in every query) instead of PostgreSQL RLS. Here's why, and when to switch:

| Factor | Application-Layer (this template) | Database-Layer (RLS) |
|--------|----------------------------------|---------------------|
| **Architecture** | Backend is the only DB client | Multiple clients access DB (PostgREST, Realtime, Edge Functions) |
| **Performance** | Zero overhead — no policy evaluation | 1-10ms overhead per query (with optimizations above) |
| **Auditability** | All auth logic in Rust, compile-time checked | Policies in SQL, checked at runtime |
| **Defense-in-depth** | Single enforcement point (Axum) | DB-level fallback even if app has bugs |
| **Complexity** | Simple — auth in one place | Policies per table, must keep in sync |

**When to add RLS to this template**: If you add Supabase client-side queries (real-time subscriptions, direct PostgREST access, Edge Functions), enable RLS as a defense-in-depth layer. The application-layer checks remain — RLS becomes a safety net.

**If adding RLS to the `items` table in this template**:

```sql
-- Enable RLS
ALTER TABLE items ENABLE ROW LEVEL SECURITY;
-- Force even table owner to respect RLS
ALTER TABLE items FORCE ROW LEVEL SECURITY;

-- Index for policy performance (already exists in composite index)
-- The composite index (user_id, created_at DESC, id DESC) covers this.

-- Policies with all optimizations applied:
CREATE POLICY "Users select own items" ON items
    FOR SELECT TO authenticated
    USING ( (select auth.uid()) = user_id );

CREATE POLICY "Users insert own items" ON items
    FOR INSERT TO authenticated
    WITH CHECK ( (select auth.uid()) = user_id );

CREATE POLICY "Users update own items" ON items
    FOR UPDATE TO authenticated
    USING ( (select auth.uid()) = user_id )
    WITH CHECK ( (select auth.uid()) = user_id );

CREATE POLICY "Users delete own items" ON items
    FOR DELETE TO authenticated
    USING ( (select auth.uid()) = user_id );

-- Service role (Axum backend) bypasses RLS via direct connection
-- No policy needed for the backend — it uses DATABASE_URL (postgres role)
```

---

### Checklist: Adding a New Table

When extending this template with new tables, follow this checklist:

```
□ Enable RLS if table is in public schema and accessed via Supabase API
    ALTER TABLE new_table ENABLE ROW LEVEL SECURITY;

□ Create policies with ALL optimizations:
    - Wrap auth.uid() in (select auth.uid())
    - Specify TO authenticated (not public)
    - Minimize joins — use IN (select ...) pattern
    - Complex checks → private.security_definer_function()

□ Add indexes for:
    - Columns in WHERE clauses
    - Columns used in RLS policies (user_id)
    - Foreign keys (for JOIN and CASCADE performance)
    - Composite index matching your most common query pattern

□ Use cursor pagination (not offset) for list endpoints
    - Composite index: (filter_col, sort_col DESC, id DESC)

□ Use sqlx::query_as!() for compile-time SQL verification

□ Include user_id in every query (even with RLS — belt and suspenders)

□ Run EXPLAIN ANALYZE on new queries to verify index usage

□ Run Supabase Security Advisor to check for RLS gaps
```

> **Official Sources for this section**: [PostgreSQL — Performance Tips](https://www.postgresql.org/docs/current/performance-tips.html) | [PostgreSQL — Row Security](https://www.postgresql.org/docs/current/ddl-rowsecurity.html) | [PostgreSQL — CREATE POLICY](https://www.postgresql.org/docs/current/sql-createpolicy.html) | [PostgreSQL — Index Types](https://www.postgresql.org/docs/current/indexes-types.html) | [Supabase — Row Level Security](https://supabase.com/docs/guides/database/postgres/row-level-security) | [Supabase — Query Optimization](https://supabase.com/docs/guides/database/query-optimization) | [Supabase — Securing Your API](https://supabase.com/docs/guides/api/securing-your-api) | [Supabase — Performance Advisors](https://supabase.com/docs/guides/database/database-advisors) | [SQLx Docs](https://docs.rs/sqlx/0.8/sqlx/)

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

### PostgreSQL / SQL Performance & Security

| Topic | Documentation |
|-------|---------------|
| EXPLAIN & Query Plans | https://www.postgresql.org/docs/current/using-explain.html |
| Index Types | https://www.postgresql.org/docs/current/indexes-types.html |
| Multicolumn Indexes | https://www.postgresql.org/docs/current/indexes-multicolumn.html |
| Partial Indexes | https://www.postgresql.org/docs/current/indexes-partial.html |
| Covering Indexes / Index-Only Scans | https://www.postgresql.org/docs/current/indexes-index-only-scans.html |
| Row Level Security (PostgreSQL) | https://www.postgresql.org/docs/current/ddl-rowsecurity.html |
| CREATE POLICY Reference | https://www.postgresql.org/docs/current/sql-createpolicy.html |
| Performance Tips | https://www.postgresql.org/docs/current/performance-tips.html |
| Supabase RLS Guide | https://supabase.com/docs/guides/database/postgres/row-level-security |
| Supabase Query Optimization | https://supabase.com/docs/guides/database/query-optimization |
| Supabase Security Advisors | https://supabase.com/docs/guides/database/database-advisors |
| Supabase API Security | https://supabase.com/docs/guides/api/securing-your-api |
| RLS Performance Benchmarks | https://github.com/GaryAustin1/RLS-Performance |

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
