/* Which copy of the Lab this build is.
 *
 * The box's copy (the default) is served by the appliance at /lab/: it links
 * back to the admin page and draws "This box" from the box's own API. The
 * hosted copy (VITE_LAB_HOSTED=1, for a cross-origin isolated host beside
 * qemu-wasm) has neither. Either copy probes for qemu-wasm at run time.
 *
 * LOSOS_LAB_MEME at build time names a folder with no.jpg and yes.jpg; the
 * splash then draws them left of its two halves (vite.config.ts copies them
 * into dist/lab/ and sets __LAB_MEME__). The repository carries no such
 * folder. */

export const BOX_COPY: boolean = import.meta.env["VITE_LAB_HOSTED"] !== "1";

declare const __LAB_MEME__: boolean;
export const MEME: boolean = typeof __LAB_MEME__ !== "undefined" && __LAB_MEME__;
