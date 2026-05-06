import { createEnv } from "@t3-oss/env-nextjs";
import { z } from "zod";

/**
 * Validated env vars. Schema runs at build time AND module-load on the server,
 * so a missing var fails the deploy/start instead of crashing on first request.
 *
 * - `server` keys are server-only (never bundled to the client).
 * - `client` keys MUST be prefixed `NEXT_PUBLIC_` and ARE shipped to the browser.
 * - `runtimeEnv` wires Next.js's per-request env access (required for Edge/Node
 *   parity since 13.4.4).
 */
export const env = createEnv({
  server: {
    API_URL: z.string().url().default("http://localhost:8080"),
  },
  client: {
    NEXT_PUBLIC_SUPABASE_URL: z.string().url(),
    NEXT_PUBLIC_SUPABASE_ANON_KEY: z.string().min(1),
  },
  runtimeEnv: {
    API_URL: process.env.API_URL,
    NEXT_PUBLIC_SUPABASE_URL: process.env.NEXT_PUBLIC_SUPABASE_URL,
    NEXT_PUBLIC_SUPABASE_ANON_KEY: process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY,
  },
  emptyStringAsUndefined: true,
});
