/* Metro compiles global stylesheets on the web target with PostCSS (Expo's
 * transformer, web only — the native bundles' files never import CSS), and
 * this file is the config: Tailwind v4 through its own plugin, the job
 * bun-plugin-tailwind did for Bun's bundler (build.ts, still the auxiliary
 * pages' build). The sheet stays src/styles/index.css; its @import
 * "tailwindcss" is what the plugin expands, tokens.css beside it is the
 * palette both targets read.
 */
module.exports = {
  plugins: {
    "@tailwindcss/postcss": {},
  },
};
