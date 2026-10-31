import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { basicClasses } from "./scripts/basic-classes.mjs";
import { segmenterQuiet } from "./vite.segmenter-quiet";

/* The guest call page (guest.html, src/calls/guest-main.tsx), built on its
 * own under /calls/guest/ after the main build. nginx serves this folder to
 * anyone holding a guest link, so it carries its own copy of every chunk it
 * needs rather than the LAN-only /assets/. The segmenter's files are not
 * copied again: nginx serves dist/chat/calls-ml/ under the guest path too
 * (modules/calls.nix), which is where background.ts looks. */
const root = import.meta.dirname;

export default defineConfig({
  plugins: [basicClasses(), react(), tailwindcss(), segmenterQuiet()],
  base: "/calls/guest/",
  resolve: {
    alias: [{ find: /^@\//, replacement: "/src/" }],
  },
  build: {
    target: "es2022",
    outDir: "dist/calls/guest",
    emptyOutDir: true,
    assetsDir: "assets",
    sourcemap: false,
    modulePreload: { polyfill: false },
    rolldownOptions: {
      input: { guest: `${root}/guest.html` },
    },
  },
});
