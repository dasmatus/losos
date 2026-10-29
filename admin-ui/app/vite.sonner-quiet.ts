import type { Plugin } from "vite";

/* Sonner writes its stylesheet into a <style> element when its module
 * loads. The admin CSP (`style-src 'self'`) refuses the element, so every
 * page logged two violations at load for a copy nothing used: main.tsx
 * imports `sonner/dist/styles.css` into the bundle, and that file is what
 * styles the stack. This drops the call, so nothing is injected and
 * nothing is refused. A Sonner release that changes the call fails the
 * build here instead of injecting again. */
const INSERT = '__insertCSS("';
const SKIP = '(function(){})("';

export function sonnerQuiet(): Plugin {
  return {
    name: "losos:sonner-quiet",
    apply: "build",
    transform(code, id) {
      if (!/[\\/]node_modules[\\/]sonner[\\/]dist[\\/]index\.m?js/.test(id)) return null;
      if (code.split(INSERT).length !== 2) {
        this.error("Sonner's stylesheet injection changed: look at vite.sonner-quiet.ts again");
      }
      return { code: code.replace(INSERT, SKIP), map: null };
    },
  };
}
