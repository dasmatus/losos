/* Which copy of the Lab this build is.
 *
 * The box's copy (the default) is served by the appliance at /lab/: it links
 * back to the admin page and draws "This box" from the box's own API. The
 * hosted copy (LOSOS_LAB_HOSTED=1, for a cross-origin isolated host beside
 * qemu-wasm) has neither. Either copy probes for qemu-wasm at run time.
 *
 * LOSOS_LAB_MEME at build time names a folder with no.jpg and yes.jpg; the
 * splash then draws them left of its two halves (build.ts copies them
 * beside dist/diagrams/index.html, and `losos:build` says so). The
 * repository carries no such folder. */
import { LAB_HOSTED, LAB_MEME } from "losos:build";

export const BOX_COPY: boolean = !LAB_HOSTED;
export const MEME: boolean = LAB_MEME;
