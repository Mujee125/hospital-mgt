import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "path";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    css: false,
    include: ["src/**/*.test.{ts,tsx}"],
    exclude: ["node_modules", "dist", "e2e", "src-tauri"],
    // IT-001: Coverage configuration — generates HTML, LCOV, and Cobertura reports.
    coverage: {
      provider: "v8",
      reporter: ["text", "html", "lcov", "cobertura"],
      reportsDirectory: "./coverage",
      include: [
        "src/pages/BloodBank.tsx",
        "src/lib/queries.ts",
        "src/lib/models.ts",
        "src/lib/rbac.ts",
      ],
      exclude: [
        "src/**/*.test.{ts,tsx}",
        "src/test/**",
        "src/components/ui/**",
        "src/main.tsx",
      ],
      thresholds: {
        // Target thresholds — NOT enforced as failures yet (P1 goal).
        // Uncomment when coverage is sufficient to enforce.
        // lines: 70,
        // functions: 70,
        // branches: 60,
      },
    },
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
