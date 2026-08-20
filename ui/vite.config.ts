import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

export default defineConfig(() => {
  const port = process.env.PORT ?? "9651";
  const apiTarget = process.env.API_URL ?? `http://localhost:${port}`;

  return {
    plugins: [tailwindcss(), sveltekit()],
    // `ogl` is reached only through `await import("ogl")` in the WebGL
    // backdrops, so Vite's cold-start scanner never sees it. Without this it is
    // discovered once the page is already running, which re-optimizes the
    // dependency graph, invalidates every module URL the browser has loaded,
    // and forces a full reload mid-boot.
    optimizeDeps: {
      include: ["ogl"],
    },
    server: {
      watch: {
        usePolling: process.env.VITE_USE_POLLING === "true",
        interval: 250,
        ignored: [
          "**/build/**",
          "**/.svelte-kit/output/**",
          "**/.svelte-kit/generated/**",
        ],
      },
      proxy: {
        "/api": {
          target: apiTarget,
        },
      },
    },
  };
});
