-- Webhook idempotency log. PK on (provider, event_id) means a redelivery of
-- the same event is a no-op INSERT, and the receiver can safely treat any
-- DUPLICATE_KEY error as "already processed".
--
-- Service-role only — never exposed to the client. RLS not required because
-- this table never sits behind PostgREST.

CREATE TABLE webhook_events (
    provider     TEXT NOT NULL,
    event_id     TEXT NOT NULL,
    event_type   TEXT,
    payload      JSONB NOT NULL,
    received_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (provider, event_id)
);

CREATE INDEX idx_webhook_events_received_at ON webhook_events (received_at);
