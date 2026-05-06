-- Add the items table to Supabase's Realtime publication so any INSERT/
-- UPDATE/DELETE is broadcast over the websocket. RLS on items still applies
-- — clients only receive events for rows their JWT can SELECT.
ALTER PUBLICATION supabase_realtime ADD TABLE items;
