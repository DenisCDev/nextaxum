# nextaxum

Template monorepo production-grade pra construir apps reais em **Next.js 16 + Axum + Supabase**.

[English version](./README.en.md)

O frontend roda na Vercel, o backend Rust na Railway, e o banco é Supabase Postgres. Auth passa pelo Supabase; o frontend conversa direto com o Supabase pra gerenciar sessão e com a API Rust pra tudo que precisa de lógica custom, validação ou acesso ao DB com rate limit.

Não é um "hello world" — cada escolha aqui foi feita pensando em produção (RLS, verificação JWT, migrations RLS-friendly, correlação de requisições, POSTs idempotentes, handlers testados, audit de segurança diário). Você clona, configura 4 env vars, e tem um app real rodando. Ou apaga o recurso `items` de exemplo e usa como scaffold.

---

## Por que cada peça existe

- **Next.js 16** pro app que o usuário vê. App Router, Server Components, Server Actions, Turbopack. Renderiza na borda da rede da Vercel.
- **Axum 0.8** como camada de API. Tudo que precisa de computação pesada, tarefa demorada, webhook assinado, ou SQL complexo entra no serviço Rust — não em function serverless. A pilha de middleware do Tower cuida de rate limit por IP, request IDs, tracing estruturado, headers de segurança, compressão.
- **Supabase** pra Postgres + Auth + Storage + Realtime. O backend Rust conecta direto no mesmo banco via sqlx — RLS mantém tudo seguro mesmo quando as duas camadas escrevem nas mesmas tabelas.

---

## Arquitetura

```
                 Browser
                    │
           cookies (httpOnly)
                    │
              ┌─────▼─────┐
              │  Next.js  │  proxy.ts (auth otimista) +
              │  na       │  DAL (verifySession em cada Server Action /
              │  Vercel   │  Server Component / Route Handler)
              └──┬──────┬─┘
   Server Action │      │ fetch /api/* com bearer JWT
                 │      │
        ┌────────▼─┐  ┌─▼──────────┐
        │ Supabase │  │ Axum API   │  bypass de RLS via role
        │ Auth/    │  │ na Railway │  postgres direto; tower-governor
        │ Storage/ │  │            │  rate limit; OpenAPI em /docs
        │ Realtime │  └──────┬─────┘
        └────┬─────┘         │
             │               │ sqlx (porta 5432, direta)
             │               │
             └────►  Postgres + RLS  ◄────┘
                    (Supabase)
```

Dois caminhos pro banco:

1. **Frontend → Supabase REST/Realtime/Storage**: protegido por RLS em cada tabela que o usuário consegue alcançar.
2. **Frontend → Axum → Postgres**: verifica o JWT do Supabase (HS256 ou assimétrico via JWKS) e roda lógica server-side confiável no mesmo DB.

Você escolhe por endpoint. CRUD simples por linha: vai direto pelo Supabase REST. Qualquer coisa com validação não-trivial, fan-out, chamadas externas ou trabalho longo: passa pelo Axum.

---

## O que tem na caixa

### Frontend (`frontend/`)

- App Router com Server Components, Server Actions, formulários `useActionState`.
- `proxy.ts` (substituto do `middleware.ts` no Next 16) faz checks otimistas de auth via claims JWT — rápido, sem ida ao banco.
- A barreira real de auth é a **DAL** (`lib/dal.ts::verifySession`). Todo Server Component, Server Action e Route Handler que toca dado privado chama ela.
- Login: email/senha (Server Action com validação Zod) + Google OAuth (PKCE, callback em `/auth/callback`).
- Página de enrollment de TOTP MFA em `/dashboard/mfa`.
- Upload de avatar via Server Action pra bucket privado do Supabase Storage.
- Lista de items que atualiza ao vivo via subscription Realtime em `postgres_changes`.
- Env vars validadas no boot via `@t3-oss/env-nextjs` + Zod.
- Vitest pra unit + Playwright pra E2E.
- ESLint flat config, TypeScript strict, pronto pra Tailwind (não bundlado — sua escolha).

### Backend (`backend/`)

- Axum 0.8 com pilha completa de middleware tower-http: request ID, panic catch, tracing, rate limit por IP (tower-governor), timeout, compressão, body limit, CORS, headers de segurança (HSTS, CSP, X-Frame-Options, etc).
- CRUD de items com paginação por cursor + GET condicional via ETag.
- Tabela shadow de profile (`public.profiles`) com trigger `handle_new_user()` — todo signup novo ganha row automaticamente.
- Suporte a Idempotency-Key em `POST /items` (padrão Stripe, cron de limpeza 24h).
- Receiver de webhook assinado em `POST /webhooks/{provider}` (HMAC-SHA256, comparação constant-time, dedup por `(provider, event_id)`).
- Verificação JWT: HS256 (legado) **e** assimétrico (RS256/ES256/EdDSA) via JWKS cacheado. Algoritmo escolhido por token a partir do header.
- Spec OpenAPI gerado pelo utoipa em compile-time. Swagger UI em `/docs`, spec raw em `/openapi.json`. Frontend regenera client tipado Zod via `npm run gen:api`.
- Health endpoints separados: `/health` (liveness, nunca toca deps) vs `/ready` (readiness, sonda DB e JWKS).
- Loop de cron com `CancellationToken` compartilhado pra shutdown gracioso.
- Exporter OpenTelemetry atrás da feature flag `otel`.
- Testes de integração via `sqlx::test` contra Postgres real no CI.

### Banco (`backend/migrations/`)

- Tabela `items` com índice composto pra cursor, RLS habilitado e **forçado**, quatro políticas own-row pro role `authenticated`.
- Tabela shadow `profiles` espelhando `auth.users`.
- Tabelas `idempotency_keys` e `webhook_events` com PKs e índices apropriados.
- Trigger `moddatetime` em toda tabela com `updated_at` (sem chamar `now()` no app).
- Funções/triggers usam `SECURITY DEFINER SET search_path = ''` por orientação Supabase.

### Repo / Ops

- GitHub Actions CI: jobs frontend e backend filtrados por path, job Playwright separado gated em variable `RUN_E2E`.
- Workflow diário `cargo audit` que abre issue automática.
- Dependabot pra cargo + npm + github-actions, semanal, agrupado.
- Lefthook pre-commit (eslint + typecheck + cargo fmt + clippy) e pre-push (vitest + cargo test).
- Docker compose pro stack local backend+frontend; config Supabase CLI (`supabase/config.toml`) pro stack local completo.
- `vercel.json` pinando região São Paulo.
- `.editorconfig`, `rustfmt.toml`, templates de issue/PR, CODEOWNERS, SECURITY.md, `.well-known/security.txt`.

---

## Setup obrigatório (mínimo pra bootar)

Você precisa de um projeto Supabase (free tier basta), Node 22 e Rust stable.

### 1. Clone + install

```bash
git clone <URL do seu fork>
cd nextaxum
cd frontend && npm ci && cd ..
cd backend && cargo build && cd ..   # gera Cargo.lock — comita
```

### 2. Projeto Supabase

Vai em https://supabase.com/dashboard/new e cria um projeto. Anota:

- **Project URL**: `https://<ref>.supabase.co`
- **anon public key** + **JWT secret**: `Settings → API`
- **Senha do DB**: `Settings → Database → Connection string`

Aplica as migrations da forma que preferir:

- **Supabase CLI** (recomendado): `supabase link --project-ref <ref>` depois `supabase db push`. Lê `backend/migrations/*.sql` via `supabase/config.toml`.
- **sqlx CLI**: `cd backend && DATABASE_URL=... sqlx migrate run`.
- **Manual**: cola cada `backend/migrations/*.sql` no SQL editor do Studio em ordem.

### 3. Gerar metadata sqlx offline (necessário antes do primeiro build Docker)

O backend usa macros `query!`/`query_as!` do sqlx que checam SQL em compile-time. CI e build Docker setam `SQLX_OFFLINE=true` pra não precisar de banco — mas precisam da metadata cacheada.

```bash
cd backend
cargo install sqlx-cli --no-default-features --features rustls,postgres
DATABASE_URL=postgres://... cargo sqlx prepare
git add .sqlx/
git commit -m "chore: refresh sqlx offline metadata"
```

Repete sempre que mexer numa macro `query!`/`query_as!`.

### 4. Env vars

Copia `.env.example` pra `.env` (Docker compose) ou `.env.local` (frontend dev) e preenche:

```bash
# Obrigatórias
SUPABASE_URL=https://<ref>.supabase.co
SUPABASE_JWT_SECRET=<de Settings → API>
DATABASE_URL=postgresql://postgres:<senha>@db.<ref>.supabase.co:5432/postgres
NEXT_PUBLIC_SUPABASE_URL=https://<ref>.supabase.co
NEXT_PUBLIC_SUPABASE_ANON_KEY=<anon key>

# Obrigatórias pra ligar frontend → Axum em dev
API_URL=http://localhost:8080
FRONTEND_URL=http://localhost:3000
```

Atenção no `DATABASE_URL`: usa a conexão **direta** (porta `5432`), não o pooler (porta `6543`). O pooler roda em transaction mode que não suporta prepared statements; sqlx depende deles. O backend desabilita o cache de prepared statements automaticamente se vê `:6543` ou `pgbouncer=true` na URL, mas a performance é melhor na conexão direta.

### 5. Rodar

Dois terminais:

```bash
# terminal 1
cd backend && cargo run

# terminal 2
cd frontend && npm run dev
```

Abre `http://localhost:3000`. O Swagger UI do backend fica em `http://localhost:8080/docs`.

Ou comando único via Docker:

```bash
docker compose up --build
```

---

## Setup opcional

Cada seção é independente — escolhe as que precisar.

### Stack Supabase local (dev offline completo)

Pula o projeto cloud durante desenvolvimento.

```bash
make supabase-up        # sobe Postgres + Auth + Storage + Realtime + Studio
make supabase-status    # imprime URLs locais e a anon key temporária
make supabase-down      # para tudo
make supabase-reset     # dropa o DB e reaplica migrations
```

Studio em `http://localhost:54323`. Aponta seu `.env` pras URLs que `make supabase-status` imprime.

### Verificação JWT assimétrico (recomendado pra projetos novos)

A Supabase recomenda chaves assimétricas (RS256/ES256) em vez do secret HS256 legado. O backend suporta os dois — algoritmo escolhido por token a partir do header.

Pra trocar:

1. Dashboard Supabase → `Authentication → Signing Keys` → habilita RS256 ou ES256.
2. Coloca no `.env`:
   ```bash
   SUPABASE_JWKS_URL=https://<ref>.supabase.co/auth/v1/.well-known/jwks.json
   ```
3. Reinicia o backend. Tokens HS256 existentes continuam válidos durante a transição.

### Login Google OAuth

1. Dashboard Supabase → `Authentication → Providers → Google` → habilita, cola o client ID + secret OAuth.
2. Adiciona `https://<seu-domínio-prod>/auth/callback` (e `http://localhost:3000/auth/callback` pra dev) em `Redirect URLs`.
3. O botão "Sign in with Google" em `/login` já tá ligado.

### TOTP MFA

Já ligado em `/dashboard/mfa`. Não precisa de setup além do Supabase ter MFA habilitado por padrão.

### Upload de avatar

A migration cria um bucket `avatars` privado com RLS. Sem setup adicional. Usuários sobem via `/dashboard` (botão renderizado pelo `avatar-upload.tsx`).

### Subscriptions Realtime

A migration `20240107000000` já adiciona a tabela `items` na publication `supabase_realtime`. Abre `/dashboard` em duas abas pra confirmar que eventos INSERT/UPDATE/DELETE chegam sem refetch.

### Webhooks (Stripe / GitHub / genérico)

O receiver em `POST /webhooks/{provider}` fica desabilitado até você setar:

```bash
WEBHOOK_SECRET=<32+ bytes de entropia>
```

O remetente precisa computar `HMAC-SHA256(secret, body_cru)` e mandar em `X-Signature: sha256=<hex>`. Mais `X-Event-Id` (usado pra dedup) e opcionalmente `X-Event-Type`. Olha `backend/src/routes/webhooks.rs` pra onde plugar dispatch por provider.

### Export de tracing OpenTelemetry

```bash
cd backend
cargo build --features otel
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317 cargo run --features otel
```

Manda spans (com correlação por request_id já ligada) pra qualquer collector OTLP — Honeycomb, Tempo, Jaeger, OpenTelemetry Collector. Sem a feature `otel`, nenhuma dessas crates compila junto.

### Pre-commit hooks (Lefthook)

Pega quebra de CI localmente antes do push.

```bash
# Instala lefthook uma vez: npm i -g lefthook  OU  baixa de
# https://github.com/evilmartians/lefthook/releases

make install-hooks
```

Hooks rodam em `git commit` (eslint, typecheck, cargo fmt, clippy) e `git push` (vitest, cargo test).

### CI E2E com Playwright

1 Variable + 4 Secrets em `Settings → Secrets and variables → Actions`:

| Onde | Nome | Valor |
|---|---|---|
| Variable | `RUN_E2E` | `true` |
| Secret | `E2E_SUPABASE_URL` | URL de um projeto Supabase **staging** |
| Secret | `E2E_SUPABASE_ANON_KEY` | anon key desse projeto |
| Secret | `E2E_TEST_USER_EMAIL` | email de usuário de teste pré-criado |
| Secret | `E2E_TEST_USER_PASSWORD` | senha desse usuário |

O job `frontend-e2e` só roda quando `RUN_E2E=true`. Caso contrário, é skipped silenciosamente e você ainda roda Playwright local com as mesmas envs.

### Deploy em produção

**Frontend (Vercel)**:

1. Importa o repo, define **Root Directory** como `frontend`.
2. Adiciona env vars: `NEXT_PUBLIC_SUPABASE_URL`, `NEXT_PUBLIC_SUPABASE_ANON_KEY`, `API_URL` (URL Railway).
3. `vercel.json` já pina região `gru1` (São Paulo). Muda se sua Railway está em outra região.

**Backend (Railway)**:

1. Novo projeto → Deploy from GitHub → escolhe esse repo, define Root Directory pra `/backend`.
2. Env vars: `DATABASE_URL` (direta, 5432), `SUPABASE_JWT_SECRET`, `FRONTEND_URL` (URL Vercel), opcionalmente `SUPABASE_JWKS_URL`, `WEBHOOK_SECRET`.
3. Healthcheck path é `/ready` (definido em `railway.toml`).

**Banco (Supabase)**:

Aplica migrations via `supabase db push` ou cola SQL no Studio. Confirma que RLS está habilitado em `items` e `profiles` (deveria estar, mas verifica).

---

## Comandos comuns

```bash
# Frontend
cd frontend
npm run dev            # localhost:3000 com Turbopack
npm run lint
npm run typecheck
npm run test:run       # vitest, run único
npm run test           # vitest watch
npm run test:e2e       # playwright (precisa das envs E2E_TEST_USER_*)
npm run gen:api        # regenera client tipado a partir de /openapi.json
npm run build

# Backend
cd backend
cargo run
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test                          # precisa de DATABASE_URL apontando pra Postgres
cargo sqlx prepare                  # refresh .sqlx/ depois de mudanças em macros
cargo build --release --locked
cargo build --features otel         # OpenTelemetry opt-in

# Repo
make install-hooks
make docker-up
make supabase-up
```

---

## Mapa de arquivos

```
.
├── frontend/
│   ├── src/
│   │   ├── app/                       Rotas App Router
│   │   │   ├── auth/callback/route.ts Code exchange OAuth
│   │   │   ├── dashboard/             Protegida — items, MFA, avatar
│   │   │   └── login/                 Email/senha + Google
│   │   ├── lib/
│   │   │   ├── api/{client,items}.ts  Wrapper fetch server-side pro Axum
│   │   │   ├── dal.ts                 verifySession (barreira real de auth)
│   │   │   ├── env.ts                 Env validada por Zod (boot falha rápido)
│   │   │   └── supabase/{browser,server,proxy}.ts
│   │   └── proxy.ts                   Middleware do Next 16 (auth otimista)
│   ├── __tests__/                     Vitest
│   ├── e2e/                           Playwright
│   ├── eslint.config.mjs              Flat config (Next 16 removeu `next lint`)
│   ├── vitest.config.mts
│   ├── playwright.config.ts
│   ├── next.config.ts                 Headers de segurança + typedRoutes
│   └── vercel.json
│
├── backend/
│   ├── src/
│   │   ├── main.rs                    Entry point (layout lib + bin)
│   │   ├── lib.rs                     Re-exports pros testes de integração
│   │   ├── config.rs                  Result<Config, ConfigError>
│   │   ├── state.rs                   AppState (pool, JWKS cache)
│   │   ├── error.rs                   AppError → IntoResponse
│   │   ├── telemetry.rs               Init de tracing (consciente de otel)
│   │   ├── jobs/                      Cron de background
│   │   ├── middleware/
│   │   │   ├── auth.rs                Verificação JWT (HS256 + JWKS)
│   │   │   └── jwks.rs                Fetcher JWKS cacheado
│   │   ├── extractors/
│   │   │   ├── auth_user.rs
│   │   │   ├── idempotency.rs
│   │   │   └── validated.rs
│   │   ├── models/                    Derives sqlx + utoipa
│   │   ├── db/                        Helpers de query
│   │   ├── routes/
│   │   │   ├── mod.rs                 Montagem OpenApiRouter + Swagger UI
│   │   │   ├── health.rs              /health + /ready
│   │   │   ├── items.rs               CRUD /api/items
│   │   │   ├── profile.rs             /api/profile
│   │   │   └── webhooks.rs            /webhooks/{provider}
│   │   └── test_support.rs            Helpers pra tests/
│   ├── tests/                         Testes #[sqlx::test]
│   ├── migrations/                    Arquivos SQL numerados
│   ├── Cargo.toml
│   ├── rustfmt.toml
│   ├── Dockerfile
│   └── railway.toml
│
├── supabase/config.toml               Stack CLI local
├── docker-compose.yml
├── lefthook.yml                       Hooks pre-commit / pre-push
├── .github/
│   ├── workflows/{ci.yml,audit.yml}
│   ├── dependabot.yml
│   ├── ISSUE_TEMPLATE/
│   ├── pull_request_template.md
│   ├── CODEOWNERS
│   └── SECURITY.md
├── .env.example
├── .editorconfig
├── Makefile
├── CHANGELOG.md
└── README.md
```

---

## Troubleshooting

**`cargo build` falha com "set DATABASE_URL"**: você não tem `.sqlx/` comitado ainda. Roda `cargo sqlx prepare` contra um banco vivo, depois comita. Ou seta `DATABASE_URL` no shell pra pular o offline mode.

**Backend boota mas toda chamada de API retorna 401**: mismatch de alg JWT. Se o projeto usa chaves assimétricas, seta `SUPABASE_JWKS_URL`. Se usa HS256, garante que `SUPABASE_JWT_SECRET` bate com `Settings → API → JWT Secret`.

**`sqlx::Error::Database … prepared statement … already exists`**: você tá conectando via pooler (porta 6543) em transaction mode. Troca pra direta (5432). Pra uso inevitável do pooler, o backend já desabilita o cache de prepared statements automaticamente quando vê `:6543` ou `pgbouncer=true`.

**Eventos Realtime não chegam**: confirma que a migration que rodou `ALTER PUBLICATION supabase_realtime ADD TABLE items;` foi aplicada. Studio mostra publications em `Database → Publications`.

**Playwright skipa todo teste**: `E2E_TEST_USER_EMAIL` ou `E2E_TEST_USER_PASSWORD` não setados. O spec auto-skipa quando não tem — seta ou roda com `RUN_E2E=true` no CI.

**Build Vercel falha em `next lint`**: você está em Next 16 mas usando o script antigo. Puxa do template — `package.json` já tem `eslint .` no lugar.

**Testes falham com "schema auth does not exist"**: testes rodam contra um Postgres stub no CI que inclui uma `auth.users` falsa; localmente você precisa do Supabase rodando (`make supabase-up`) ou aplicar o mesmo CREATE SCHEMA. O workflow CI (`.github/workflows/ci.yml`) faz isso no step "Provision auth schema for tests".

---

## Versões pinadas neste template

- Next.js `^16.2`, React `^19`, TypeScript `^5.7`, Node `22`
- Rust `stable` (edition 2024 = `>=1.85`), Axum `0.8`, sqlx `0.8`, tower-http `0.6`, tower-governor `0.7`
- Supabase: managed (sem pin de versão — eles fazem continuous-deploy)

A matrix do CI testa contra essas. Bumpar qualquer uma é decisão deliberada.

---

## Licença

MIT — ver [LICENSE](LICENSE) quando adicionado.

## Segurança

Ver [.github/SECURITY.md](.github/SECURITY.md) pra política de disclosure.
