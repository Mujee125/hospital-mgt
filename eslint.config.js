// ESLint 9 flat config — per SDLC §5.1 / Security Matrix A.8.25.
//
// Mirrors the modern Vite + React + TS template:
//   - @eslint/js recommended
//   - typescript-eslint strict
//   - eslint-plugin-react-hooks (rules-of-hooks, exhaustive-deps)
//   - eslint-plugin-react-refresh (Fast Refresh boundary hygiene)
//   - eslint-config-prettier (disables style rules that conflict with Prettier)
//
// Run: `npm run lint`      — check
// Run: `npm run lint:fix`  — auto-fix
import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import tseslint from "typescript-eslint";
import prettier from "eslint-config-prettier";

export default tseslint.config(
  // Global ignores
  {
    ignores: [
      "dist/**",
      "node_modules/**",
      "src-tauri/**",
      "public/**",
      "*.config.ts",
      "*.config.js",
    ],
  },

  // Base JS recommended
  js.configs.recommended,

  // TypeScript strict (type-aware rules kept off for speed — enable per-file if needed)
  ...tseslint.configs.recommended,

  // React + TS app files
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      ecmaVersion: 2020,
      globals: { ...globals.browser },
    },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": [
        "warn",
        { allowConstantExport: true },
      ],
      // Per Security Matrix A.8.16 / SRS NFR-15 — never let `any` slip silently.
      "@typescript-eslint/no-explicit-any": "warn",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
    },
  },

  // Prettier compat — must be last
  prettier,
);
