import { resolve } from "node:path";

import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// Tauri expects a fixed port and must see Rust-side errors in the terminal.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    // WebView2 on Windows is evergreen Chromium; WebKitGTK is used only for development on Linux.
    target: ["chrome120", "safari16"],
    sourcemap: false,
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, "index.html"),
        companion: resolve(import.meta.dirname, "companion.html"),
        confirmation: resolve(import.meta.dirname, "confirmation.html"),
      },
    },
  },
  test: {
    name: "desktop",
    environment: "jsdom",
    // Whole-surface tests (Settings rendered in three languages) take about
    // 0.5 s alone but can exceed the 5 s default when every file runs in
    // parallel on a busy machine.
    testTimeout: 15_000,
    include: ["test/**/*.test.{ts,tsx}"],
    setupFiles: ["test/setup.ts"],
  },
});
