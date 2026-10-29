# A JS workspace's node_modules, installed by Bun from its bun.lock: one
# fixed-output derivation per workspace, the Bun counterpart of
# nixpkgs' npm dependency fetch. Only package.json and bun.lock go into it, so
# an edit anywhere else in the workspace never refetches.
#
#   bunDeps { pname = "losos-admin-ui"; src = ./../admin-ui/app; hash = "sha256-…"; }
#
# $out/node_modules is the tree `bun install --frozen-lockfile` makes. No
# install scripts run: Playwright's would download browsers, and the native
# tools (rolldown, Tailwind's oxide, lightningcss) ship their binaries as
# platform packages. After a change to bun.lock, set `hash = lib.fakeHash`,
# build, and paste the hash the error names; .github/scripts/fix-hashes.py
# does that in CI. `local` names what a `file:` dependency installs from.
#
# A build takes it in with `nodeModules.install`, the shell lines that copy
# the tree into the working directory and point its `#!/usr/bin/env node`
# scripts at Bun: the sandbox has neither /usr/bin/env nor Node.
{ pkgs }:
{
  pname,
  src,
  hash,
  local ? [ ],
}:
pkgs.stdenvNoCC.mkDerivation (finalAttrs: {
  pname = "${pname}-node-modules";
  version = "0";
  src = pkgs.lib.fileset.toSource {
    root = src;
    fileset = pkgs.lib.fileset.unions (
      [
        (src + "/package.json")
        (src + "/bun.lock")
      ]
      ++ map (path: src + "/${path}") local
    );
  };

  nativeBuildInputs = [ pkgs.bun ];
  # A proxy the builder was given, as nixpkgs' own fetchers take it.
  impureEnvVars = pkgs.lib.fetchers.proxyImpureEnvVars;

  dontConfigure = true;
  buildPhase = ''
    runHook preBuild
    export HOME=$TMPDIR
    export BUN_INSTALL_CACHE_DIR=$TMPDIR/bun-cache
    # Bun carries its own roots; a builder behind an intercepting proxy hands
    # its CA in as Nix's ssl-cert-file.
    if [ -e /etc/ssl/certs/ca-certificates.crt ]; then
      export NODE_EXTRA_CA_CERTS=/etc/ssl/certs/ca-certificates.crt
    fi
    bun install --frozen-lockfile --ignore-scripts --no-progress --no-save
    runHook postBuild
  '';
  installPhase = ''
    runHook preInstall
    mkdir -p $out
    cp -R node_modules $out/
    runHook postInstall
  '';
  # The tree is copied as Bun left it: no shebangs rewritten, no store paths
  # added, so the hash depends on bun.lock alone.
  dontFixup = true;

  outputHash = hash;
  outputHashAlgo = "sha256";
  outputHashMode = "recursive";

  # Copied rather than linked: Vite and webpack keep caches in node_modules.
  # Bun answers as `node` the way `bun --bun` makes it, by a link named node.
  passthru.install = ''
    cp -R ${finalAttrs.finalPackage}/node_modules .
    chmod -R u+w node_modules
    mkdir -p "$TMPDIR/bun-as-node"
    ln -sf ${pkgs.bun}/bin/bun "$TMPDIR/bun-as-node/node"
    mapfile -t bins < <(find -L node_modules -path '*/.bin/*' -type f -exec readlink -f {} + | sort -u)
    PATH="$TMPDIR/bun-as-node:$PATH" patchShebangs --build "''${bins[@]}"
  '';
})
