import { svelte, vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vitest/config";
import path from "path";

export default defineConfig({
  plugins: [
    svelte({
      preprocess: vitePreprocess(),
      compilerOptions: {
        css: "injected",
        dev: true,
      },
    }),
  ],
  resolve: {
    alias: {
      $lib: path.resolve(__dirname, "src/lib"),
    },
    conditions: ["browser"],
  },
  test: {
    environment: "happy-dom",
    include: ["src/**/*.test.ts"],
    globals: true,
    setupFiles: ["src/test/setup.ts"],
  },
});
