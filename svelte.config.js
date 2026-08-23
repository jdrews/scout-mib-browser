import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

// No global `runes: true`: every app component uses runes syntax (auto-detected),
// while lucide-svelte ships legacy-mode components ($$props) that break when
// forced into runes mode.
export default {
  preprocess: vitePreprocess(),
  compilerOptions: {
    css: "injected",
  },
};
