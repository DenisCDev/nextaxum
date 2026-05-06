import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tsconfigPaths from "vite-tsconfig-paths";

export default defineConfig({
  plugins: [tsconfigPaths(), react()],
  test: {
    environment: "jsdom",
    setupFiles: ["./vitest.setup.ts"],
    // Playwright e2e tests live under e2e/ — keep vitest off them.
    exclude: ["**/node_modules/**", "**/.next/**", "**/e2e/**", "**/dist/**"],
    globals: true,
  },
});
