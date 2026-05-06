-- Pre-seeded items for the two test users. Three rows for alice so cursor
-- pagination has something to slice; one row for bob so isolation tests can
-- verify alice never sees bob's data.
INSERT INTO items (id, user_id, title, description, created_at, updated_at) VALUES
    ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1', '11111111-1111-1111-1111-111111111111', 'alice item 1', NULL,           now() - interval '3 minutes', now() - interval '3 minutes'),
    ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2', '11111111-1111-1111-1111-111111111111', 'alice item 2', 'second',       now() - interval '2 minutes', now() - interval '2 minutes'),
    ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa3', '11111111-1111-1111-1111-111111111111', 'alice item 3', 'third',        now() - interval '1 minute',  now() - interval '1 minute'),
    ('bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbb1', '22222222-2222-2222-2222-222222222222', 'bob item 1',   'private',      now(), now());
