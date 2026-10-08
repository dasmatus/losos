/* Webpack's url-loader turns an imported picture into its address. The
 * Docusaurus type aliases declare SVGs only; the landing page also imports
 * the handbook's PNG screenshots. */
declare module '*.png' {
  const src: string;
  export default src;
}
