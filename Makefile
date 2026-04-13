.PHONY: dev dev-backend dev-frontend build lint test migrate docker-up sqlx-prepare lockfiles

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

# Prepare sqlx offline cache (requires running database)
sqlx-prepare:
	cd backend && cargo sqlx prepare

# Generate lock files
lockfiles:
	cd backend && cargo generate-lockfile
	cd frontend && npm install
