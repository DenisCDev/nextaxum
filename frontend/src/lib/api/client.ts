import "server-only";
import { createClient } from "@/lib/supabase/server";
import { env } from "@/lib/env";

type FetchOptions = Omit<RequestInit, "headers"> & {
  headers?: Record<string, string>;
};

/**
 * Server-side API client for the Axum backend.
 * Forwards the Supabase access token via the official SSR client so token
 * refresh + cookie-format changes are handled by @supabase/ssr (not by us).
 */
export async function api<T>(path: string, options: FetchOptions = {}): Promise<T> {
  const supabase = await createClient();
  const { data: { session } } = await supabase.auth.getSession();
  const accessToken = session?.access_token;

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...options.headers,
  };

  if (accessToken) {
    headers["Authorization"] = `Bearer ${accessToken}`;
  }

  const res = await fetch(`${env.API_URL}${path}`, {
    ...options,
    headers,
  });

  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(body.error ?? `API error: ${res.status}`);
  }

  if (res.status === 204) return undefined as T;
  return res.json();
}
