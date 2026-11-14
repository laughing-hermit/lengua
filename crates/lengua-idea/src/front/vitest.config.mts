import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

const here = fileURLToPath(new URL("./", import.meta.url));

export default defineConfig({
  resolve: { alias: [{ find: /^@\//, replacement: here }] },
  oxc: { jsx: { runtime: "automatic", importSource: "react" } },
  test: {
    environment: "happy-dom",
    include: ["tests/**/*.test.tsx"],
    testTimeout: 10000,
    fileParallelism: false,
  },
});
