-- DB-level updated_at trigger via the moddatetime extension (postgres-contrib,
-- pre-installed on Supabase). Replaces app-side `SET updated_at = now()` so any
-- UPDATE — including ones from psql, the Studio, or another service — keeps
-- the column accurate.

CREATE EXTENSION IF NOT EXISTS moddatetime SCHEMA extensions;

CREATE TRIGGER items_updated_at
    BEFORE UPDATE ON items
    FOR EACH ROW
    EXECUTE FUNCTION extensions.moddatetime(updated_at);
