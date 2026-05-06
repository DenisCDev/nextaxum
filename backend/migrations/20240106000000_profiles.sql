-- Profile shadow table — canonical Supabase pattern. The auth.users row owns
-- the credentials; the public.profiles row owns app-level fields you can read
-- via the REST/Realtime APIs without exposing auth schema directly.

CREATE TABLE profiles (
    id            UUID PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
    display_name  TEXT,
    avatar_url    TEXT,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER profiles_updated_at
    BEFORE UPDATE ON profiles
    FOR EACH ROW
    EXECUTE FUNCTION extensions.moddatetime(updated_at);

ALTER TABLE profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE profiles FORCE ROW LEVEL SECURITY;

CREATE POLICY profiles_select_own ON profiles
    FOR SELECT
    TO authenticated
    USING ((SELECT auth.uid()) = id);

CREATE POLICY profiles_update_own ON profiles
    FOR UPDATE
    TO authenticated
    USING ((SELECT auth.uid()) = id)
    WITH CHECK ((SELECT auth.uid()) = id);

-- Backfill any users that already exist (no-op on a fresh install).
INSERT INTO profiles (id, display_name)
SELECT id, COALESCE(raw_user_meta_data->>'display_name', email)
FROM auth.users
ON CONFLICT (id) DO NOTHING;

-- Auto-provision a profiles row whenever a new auth user is created. Runs as
-- the function owner (postgres) so it can reach the public schema even though
-- the trigger fires inside auth.
CREATE OR REPLACE FUNCTION public.handle_new_user()
    RETURNS TRIGGER
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = ''
AS $$
BEGIN
    INSERT INTO public.profiles (id, display_name)
    VALUES (NEW.id, COALESCE(NEW.raw_user_meta_data->>'display_name', NEW.email));
    RETURN NEW;
END;
$$;

CREATE TRIGGER on_auth_user_created
    AFTER INSERT ON auth.users
    FOR EACH ROW
    EXECUTE FUNCTION public.handle_new_user();
