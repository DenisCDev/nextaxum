-- Test fixture: two synthetic users so RLS / multi-tenant isolation can be
-- exercised. Inserted directly into auth.users because pgcrypto + the auth
-- schema are present on Supabase / supabase-cli databases. For sqlx::test
-- against a vanilla Postgres image, swap auth.users for a stub table.
INSERT INTO auth.users (id, email, encrypted_password, instance_id, aud, role, created_at, updated_at)
VALUES
    ('11111111-1111-1111-1111-111111111111', 'alice@test.local', '', '00000000-0000-0000-0000-000000000000', 'authenticated', 'authenticated', now(), now()),
    ('22222222-2222-2222-2222-222222222222', 'bob@test.local',   '', '00000000-0000-0000-0000-000000000000', 'authenticated', 'authenticated', now(), now())
ON CONFLICT (id) DO NOTHING;
