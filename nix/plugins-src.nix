# lososd's plugin workspace (backend/plugins) with the one crate outside it
# that a plugin reads by path: losos-mdx (backend/mdx), which the pages
# plugin checks a page with. Both builds of the workspace (losos-plugins in
# packages.nix, Mesh CI's pm plugin) unpack it as source/plugins beside
# source/mdx and build in source/plugins, so `../../mdx` resolves as it
# does in a checkout.
{ lib }:
{
  src = lib.fileset.toSource {
    root = ./../backend;
    fileset = lib.fileset.unions [
      (lib.fileset.fromSource (lib.cleanSource ./../backend/plugins))
      (lib.fileset.fromSource (lib.cleanSource ./../backend/mdx))
    ];
  };
  sourceRoot = "source/plugins";
}
