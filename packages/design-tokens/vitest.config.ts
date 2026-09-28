import { defineProject } from "vitest/config";

export default defineProject({
  test: { name: "design-tokens", environment: "jsdom", include: ["test/**/*.test.ts"] },
});
