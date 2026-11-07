/* The default pages' HTML, compiled from src/pages/*.mdx at build time by
 * pages.ts (bundler.ts serves it). */
declare module "losos:pages" {
  export const DEFAULTS: Readonly<Record<string, string>>;
}
