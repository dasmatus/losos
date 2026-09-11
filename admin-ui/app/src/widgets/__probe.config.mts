/* Throwaway probe config — NOT part of the app. Deleted before this lands. */
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const here = resolve(fileURLToPath(import.meta.url), "..");
const app = resolve(here, "../..");
const out =
  "/tmp/claude-1000/-var-home-matus-Dokumente-codeberg-personal-losos--claude-worktrees-twinkling-roaming-candy/1c8d98d3-7bb6-48b2-8653-5b5e4a8c2a7a/scratchpad/probe-dist";

export default defineConfig({
  root: app,
  base: "./",
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": resolve(app, "src") } },
  build: {
    outDir: out,
    emptyOutDir: true,
    rollupOptions: { input: resolve(here, "__probe.html") },
  },
});
