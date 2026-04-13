import "server-only";
import { cache } from "react";
import { redirect } from "next/navigation";
import { createClient } from "@/lib/supabase/server";

/**
 * Data Access Layer — the real security boundary.
 *
 * The proxy (proxy.ts) only does optimistic redirects and token refresh.
 * This function is where actual auth verification happens.
 * Uses React.cache() so multiple calls in one render are deduplicated.
 *
 * Call this in every Server Component, Server Action, or Route Handler
 * that needs authenticated user data.
 *
 * @see https://nextjs.org/docs/app/guides/authentication
 */
export const verifySession = cache(async () => {
  const supabase = await createClient();
  const {
    data: { user },
  } = await supabase.auth.getUser();

  if (!user) {
    redirect("/login");
  }

  return { user };
});
