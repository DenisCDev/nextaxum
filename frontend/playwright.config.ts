import { defineConfig, devices } from "@playwright/test";

/**
 * Run with `npm run test:e2e`.
 *
 * The webServer config builds and serves the app on a fixed port so the
 * tests run against the production bundle — same surface as Vercel.
 *
 * Required env (.env.local or shell):
 *   NEXT_PUBLIC_SUPABASE_URL, NEXT_PUBLIC_SUPABASE_ANON_KEY,
 *   E2E_TEST_USER_EMAIL, E2E_TEST_USER_PASSWORD.
 *
 * In CI we recommend a dedicated Supabase staging project.
 */
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? "github" : "list",

  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL ?? "http://localhost:3001",
    trace: "on-first-retry",
  },

  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  ],

  webServer: process.env.PLAYWRIGHT_BASE_URL
    ? undefined
    : {
        command: "npm run build && npm run start -- -p 3001",
        url: "http://localhost:3001",
        reuseExistingServer: !process.env.CI,
        timeout: 120_000,
      },
});
