import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "**/dist/**",
      "**/node_modules/**",
      "target/**",
      "apps/desktop/src-tauri/**",
      "packages/contracts/src/generated/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  {
    languageOptions: {
      globals: { ...globals.browser },
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    plugins: { "react-hooks": reactHooks },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "@typescript-eslint/restrict-template-expressions": ["error", { allowNumber: true }],
      // The UI must never reach Tauri directly: all IPC goes through src/ipc.
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["@tauri-apps/api", "@tauri-apps/api/*"],
              message: "Import IPC through src/ipc (the typed client) instead.",
            },
          ],
        },
      ],
    },
  },
  {
    files: ["apps/desktop/src/ipc/**"],
    rules: { "no-restricted-imports": "off" },
  },
  {
    files: ["**/*.config.{js,ts}", "scripts/**", "**/scripts/**/*.mjs"],
    languageOptions: { globals: { ...globals.node } },
    ...tseslint.configs.disableTypeChecked,
  },
);
