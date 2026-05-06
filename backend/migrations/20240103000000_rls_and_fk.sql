-- Defense-in-depth: even though the Axum backend connects with a Postgres role
-- that bypasses RLS, enabling RLS on the table protects data when accessed via
-- the Supabase REST/Realtime endpoints (anon/authenticated roles), and isolates
-- users in case a frontend ever queries the table directly.

-- Foreign key to Supabase auth.users.
-- Cascading delete keeps items consistent when a user is removed.
ALTER TABLE items
    ADD CONSTRAINT fk_items_user_id
        FOREIGN KEY (user_id) REFERENCES auth.users(id) ON DELETE CASCADE;

ALTER TABLE items ENABLE ROW LEVEL SECURITY;
ALTER TABLE items FORCE ROW LEVEL SECURITY;

-- Authenticated users can read their own rows.
CREATE POLICY items_select_own ON items
    FOR SELECT
    TO authenticated
    USING ((SELECT auth.uid()) = user_id);

CREATE POLICY items_insert_own ON items
    FOR INSERT
    TO authenticated
    WITH CHECK ((SELECT auth.uid()) = user_id);

CREATE POLICY items_update_own ON items
    FOR UPDATE
    TO authenticated
    USING ((SELECT auth.uid()) = user_id)
    WITH CHECK ((SELECT auth.uid()) = user_id);

CREATE POLICY items_delete_own ON items
    FOR DELETE
    TO authenticated
    USING ((SELECT auth.uid()) = user_id);
