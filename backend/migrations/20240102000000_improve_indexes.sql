-- Replace separate indexes with a composite index optimized for cursor pagination:
-- WHERE user_id = ? ORDER BY created_at DESC, id DESC
DROP INDEX IF EXISTS idx_items_user_id;
DROP INDEX IF EXISTS idx_items_created_at;

CREATE INDEX idx_items_user_id_created_at_id
    ON items (user_id, created_at DESC, id DESC);

-- Foreign key to Supabase auth.users.
-- Uncomment after verifying your Supabase setup grants access to auth schema:
-- ALTER TABLE items ADD CONSTRAINT fk_items_user_id
--     FOREIGN KEY (user_id) REFERENCES auth.users(id) ON DELETE CASCADE;
