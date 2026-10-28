// markdown-it-footnote ships no types, and DefinitelyTyped's are written
// against markdown-it 14's. markdown-it 15 bundles its own.
declare module "markdown-it-footnote" {
  import type { MarkdownIt } from "markdown-it";

  export default function footnote(md: MarkdownIt): void;
}
