import "server-only";
import { cookies } from "next/headers";
import { env } from "@/lib/env";

type FetchOptions = Omit<RequestInit, "headers"> & {
  headers?: Record<string, string>;
};

/**
 * Server-side API client for the Axum backend.
 * Automatically forwards the Supabase access token.
 */
export async function api<T>(path: string, options: FetchOptions = {}): Promise<T> {
  const cookieStore = await cookies();

  // Supabase SSR stores the access token in a cookie named sb-<ref>-auth-token
  const allCookies = cookieStore.getAll();
  const accessTokenCookie = allCookies.find(
    (c) => c.name.startsWith("sb-") && c.name.endsWith("-auth-token")
  );

  let accessToken: string | undefined;
  if (accessTokenCookie) {
    try {
      const parsed = JSON.parse(accessTokenCookie.value);
      accessToken = parsed.access_token;
    } catch {
      accessToken = accessTokenCookie.value;
    }
  }

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...options.headers,
  };

  if (accessToken) {
    headers["Authorization"] = `Bearer ${accessToken}`;
  }

  const res = await fetch(`${env.apiUrl}${path}`, {
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
