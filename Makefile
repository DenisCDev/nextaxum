.PHONY: dev dev-backend dev-frontend build lint test migrate docker-up sqlx-prepare lockfiles install-hooks supabase-up supabase-down supabase-reset supabase-status

# Run backend and frontend (run in separate terminals)
dev-backend:
	cd backend && cargo run

dev-frontend:
	cd frontend && npm run dev

# Build both
build:
	cd backend && cargo build --release
	cd frontend && npm run build

# Lint and typecheck
lint:
	cd frontend && npm run lint && npm run typecheck
	cd backend && cargo clippy -- -D warnings

# Run tests
test:
	cd backend && cargo test

# Run database migrations (uses DATABASE_URL from .env)
migrate:
	cd backend && sqlx migrate run

# Docker compose
docker-up:
	docker compose up --build

# Prepare sqlx offline cache (requires running database).
# REQUIRED before first Docker build / first CI run on a fresh clone:
# the .sqlx/ directory must be committed for SQLX_OFFLINE=true to work.
sqlx-prepare:
	cd backend && cargo sqlx prepare

# Generate lock files
lockfiles:
	cd backend && cargo generate-lockfile
	cd frontend && npm install

# Install pre-commit / pre-push git hooks (Lefthook). Runs once per clone.
# Requires lefthook on PATH — `npm i -g lefthook` or download from
# https://github.com/evilmartians/lefthook/releases.
install-hooks:
	lefthook install

# --- Local Supabase stack -----------------------------------------------------
# Requires the Supabase CLI: https://supabase.com/docs/guides/cli/getting-started
# `supabase start` boots Postgres+Auth+Storage+Realtime+Studio in Docker, applying
# every backend/migrations/*.sql. Use the printed DATABASE_URL for `cargo run`.

supabase-up:
	supabase start

supabase-down:
	supabase stop

supabase-reset:
	supabase db reset

supabase-status:
	supabase status
