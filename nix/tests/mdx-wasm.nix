# Not a VM: losos-mdx's browser package (nix/packages.nix, losos-mdx-wasm)
# loaded by Node the way a page loads it, and checked against the crate's
# own expectations: the HTML, the refusals, and the size and depth limits
# inside WebAssembly's stack (nix/tests/mdx-wasm.mjs).
{ pkgs, mdxWasm }:

pkgs.runCommand "losos-mdx-wasm-check" { nativeBuildInputs = [ pkgs.nodejs ]; } ''
  node ${./mdx-wasm.mjs} ${mdxWasm}
  touch $out
''
