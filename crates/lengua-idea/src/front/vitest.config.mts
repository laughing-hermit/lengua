import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

const from = process.env["LENGUA_MUTANT_FROM"] ?? "";
const to = process.env["LENGUA_MUTANT_TO"] ?? "";
const here = fileURLToPath(new URL("./", import.meta.url));

function same(a: string, b: string): boolean {
  return a.replaceAll("\\", "/").toLowerCase() === b.replaceAll("\\", "/").toLowerCase();
}

export default defineConfig({
  resolve: { alias: [{ find: /^@\//, replacement: here }] },
  oxc: { jsx: { runtime: "automatic", importSource: "react" } },
  plugins: [
    {
      name: "lengua-mutant",
      enforce: "pre",
      load(id: string) {
        const file = id.split("?")[0] ?? id;
        return from !== "" && same(file, from) ? readFileSync(to, "utf8") : null;
      },
    },
  ],
  test: {
    environment: "happy-dom",
    include: ["tests/**/*.test.tsx"],
    testTimeout: 10000,
    fileParallelism: false,
  },
});
