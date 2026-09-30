-- Platform objects needed by production migrations on a plain PostgreSQL server.
-- The authenticated role is provisioned once per server before the test suite.
CREATE SCHEMA auth;
CREATE SCHEMA extensions;
CREATE SCHEMA storage;

CREATE TABLE auth.users (
    id UUID PRIMARY KEY,
    email TEXT,
    encrypted_password TEXT,
    instance_id UUID,
    aud TEXT,
    role TEXT,
    raw_user_meta_data JSONB,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE FUNCTION auth.uid() RETURNS UUID LANGUAGE SQL STABLE AS $$
    SELECT NULLIF(current_setting('request.jwt.claim.sub', true), '')::UUID;
$$;

CREATE TABLE storage.buckets (
    id TEXT PRIMARY KEY,
    name TEXT,
    public BOOLEAN,
    file_size_limit BIGINT,
    allowed_mime_types TEXT[]
);
CREATE TABLE storage.objects (
    id UUID PRIMARY KEY,
    bucket_id TEXT REFERENCES storage.buckets(id),
    owner UUID
);
CREATE PUBLICATION supabase_realtime;
