-- Avatar bucket: private, 1 MB cap, image/* only. Files are stored at
-- `<user_id>/<filename>` so the RLS policies can match `owner = auth.uid()`.

INSERT INTO storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
VALUES (
    'avatars',
    'avatars',
    false,
    1048576,
    ARRAY['image/png', 'image/jpeg', 'image/webp']
)
ON CONFLICT (id) DO NOTHING;

-- Policies on storage.objects: each authenticated user gets full control over
-- their own files in the avatars bucket; nobody else sees them.
CREATE POLICY avatars_select_own ON storage.objects
    FOR SELECT
    TO authenticated
    USING (bucket_id = 'avatars' AND owner = (SELECT auth.uid()));

CREATE POLICY avatars_insert_own ON storage.objects
    FOR INSERT
    TO authenticated
    WITH CHECK (bucket_id = 'avatars' AND owner = (SELECT auth.uid()));

CREATE POLICY avatars_update_own ON storage.objects
    FOR UPDATE
    TO authenticated
    USING (bucket_id = 'avatars' AND owner = (SELECT auth.uid()))
    WITH CHECK (bucket_id = 'avatars' AND owner = (SELECT auth.uid()));

CREATE POLICY avatars_delete_own ON storage.objects
    FOR DELETE
    TO authenticated
    USING (bucket_id = 'avatars' AND owner = (SELECT auth.uid()));
