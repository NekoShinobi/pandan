import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

export default defineConfig(() => {
  const port = process.env.PORT ?? "9651";
  const apiTarget = process.env.API_URL ?? `http://localhost:${port}`;

  return {
    plugins: [tailwindcss(), sveltekit()],
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
