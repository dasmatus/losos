/* What the bundler hands the source besides code (bundler.ts, build.ts). */

/* The build's settings, a module bundler.ts makes from the environment. */
declare module "losos:build" {
  /* The public path the page is built for: "/" on the box, "/calls/guest/"
   * for the guest call page. */
  export const BASE: string;
  /* The hosted copy of Diagrams (LOSOS_LAB_HOSTED=1), not the box's. */
  export const LAB_HOSTED: boolean;
  /* The Lab's splash has its meme pictures (LOSOS_LAB_MEME). */
  export const LAB_MEME: boolean;
}

/* A file imported for its URL: copied into assets/ under a hashed name. */
declare module "*?url" {
  const url: string;
  export default url;
}
declare module "*.png" {
  const url: string;
  export default url;
}

/* Stylesheets are imported for their effect: the bundler links them. */
declare module "*.css";
