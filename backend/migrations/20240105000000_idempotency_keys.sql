-- Idempotency keys table (Stripe pattern). Lets clients send the same POST
-- twice (e.g. after a network blip) and get the original response back
-- instead of creating a second resource.
--
-- Unique per (user_id, key); the key is a free-form opaque string supplied
-- by the client. The cron in src/jobs/mod.rs prunes rows older than 24h.

CREATE TABLE idempotency_keys (
    user_id         UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    key             TEXT NOT NULL,
    request_method  TEXT NOT NULL,
    request_path    TEXT NOT NULL,
    response_status SMALLINT NOT NULL,
    response_body   JSONB NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, key)
);

CREATE INDEX idx_idempotency_keys_created_at ON idempotency_keys (created_at);
