# Not a VM: losos-mdx's browser package (nix/packages.nix, losos-mdx-wasm)
# loaded by Bun the way a page loads it, and checked against the crate's
# own expectations: the HTML, the refusals, and the size and depth limits
# inside WebAssembly's stack (nix/tests/mdx-wasm.mjs).
{ pkgs, mdxWasm }:

pkgs.runCommand "losos-mdx-wasm-check" { nativeBuildInputs = [ pkgs.bun ]; } ''
  bun ${./mdx-wasm.mjs} ${mdxWasm} ${../../admin-ui/app/tests/assert.mjs}
  touch $out
''
