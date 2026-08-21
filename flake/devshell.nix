# devShells.${system} — the dev shell for hacking on the Haskell backend
# (lososd + losos-ctl). Mirrors the host fish config (cat/ls/cd aliases,
# fish_greeting + fastfetch, zoxide, starship) with two deliberate omissions,
# matching the user's "same $SHELL config minus Zellij" request:
#   - zellij auto-start: would wrap the dev shell in a multiplexer — noise for
#     `cabal` output; fish itself is the win over bash
#   - the `claude`/`codex` aliases: they call a host `ollama launch` service
#     wrapper, neither reproducible nor dev-relevant here
#
# Toolchain (verified against the pinned nixpkgs rev):
#   Haskell: GHC 9.10.3 + a broad "full stdlib" library set via ghcWithPackages,
#            cabal-install 3.16, HLS 2.13, hpack. The HUnit suite in backend/
#            runs via `nix build .#losos-ctl` (doCheck) or `cabal test`.
#
# `--no-config` makes fish ignore ~/.config/fish (the home-manager symlink) so
# only the rebuilt config below is loaded; `--init-command` sources it before
# going interactive. The `case $- in *i*)` guard keeps `nix develop -c <cmd>`
# working under bash instead of being hijacked.
{ pkgs }:
let
  # A broad, batteries-included Haskell library set bundled into GHC's private
  # package db. "full Haskell stdlib" per the user's request — core/boot libs
  # plus the ecosystem (data, networking, servant, parsing, testing, system).
  # All attrs verified present in haskellPackages at the pinned rev.
  haskellLibs = p: with p; [
    # core / boot (explicit so cabal sees consistent versions)
    text containers mtl transformers bytestring directory filepath process
    array base-compat unix
    # data / serialisation
    aeson aeson-pretty attoparsec vector unordered-containers hashmap cereal
    binary binary-orphans store scientific case-insensitive split
    # concurrency / control
    async stm concurrency parallel monad-loops unliftio
    resource-pool retry safe exceptions typed-process
    # networking (http-conduit/warp-tls/tls are dropped: they pull the
    # `connection` package, which is broken in nixpkgs at this rev)
    network http-client http-types wai warp servant servant-server http2
    cryptonite
    # config / parsing
    yaml optparse-applicative megaparsec configurator ini toml-reader
    # lenses / recursion
    lens extra these semigroupoids comonad bifunctors profunctors free
    recursion-schemes transformers-base
    # logging / formatting
    fast-logger monad-logger logging-effect prettyprinter text-show
    # testing
    tasty tasty-hunit tasty-quickcheck hspec QuickCheck
    # system / backend
    systemd dbus fsnotify
    # templating / time / misc
    mustache ginger chronos time-compat uuid random mime-types mime
    streams conduit conduit-extra resourcet
  ];

  ghc = pkgs.haskellPackages.ghcWithPackages haskellLibs;

  fishInit = pkgs.writeText "devshell-fish-init.fish" ''
    set -g fish_greeting
    fastfetch

    alias cat 'bat --paging=never'
    alias ls  'eza -lhi --git --icons always'
    alias cd  z

    ${pkgs.lib.getExe pkgs.zoxide} init fish | source
    ${pkgs.lib.getExe pkgs.starship} init fish | source

    # Dev shortcut for the Haskell backend.
    alias bcd 'cd backend'                # Haskell backend (user-authored)
  '';
in
{
  default = pkgs.mkShell {
    nativeBuildInputs = [
      # ── Haskell (full stdlib) ───────────────────────────────────────────
      ghc
      pkgs.haskellPackages.cabal-install
      pkgs.haskellPackages.haskell-language-server
      pkgs.haskellPackages.hpack
      pkgs.nodejs
      # ── Shell UX (host config, minus Zellij) ─────────────────────────────
      pkgs.fish
      pkgs.bat
      pkgs.eza
      pkgs.zoxide
      pkgs.fastfetch
      pkgs.starship
    ];

    # GHC's source path so HLS/cabal can find base etc.
    GHC_PKGS_GHC = "${ghc}";

    shellHook = ''
      # Switch to fish only for an interactive `nix develop`, detected by the
      # bash `i` flag in $-. In command mode (`nix develop -c <cmd>`, where $-
      # has no `i`) fall through so nix runs the given command under bash as
      # usual instead of being hijacked by exec.
      case $- in
        *i*) exec ${pkgs.lib.getExe pkgs.fish} -i -N -C "source ${fishInit}" ;;
      esac
    '';
  };
}