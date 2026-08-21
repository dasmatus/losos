# devShells.${system} — the dev shell for hacking on the Rust backends
# (lososd + losos-ctl, and the edge registrar) and the design-system's React
# wrapper package. Mirrors the host fish config (cat/ls/cd aliases,
# fish_greeting + fastfetch, zoxide, starship) with two deliberate omissions,
# matching the user's "same $SHELL config minus Zellij" request:
#   - zellij auto-start: would wrap the dev shell in a multiplexer — noise for
#     `cargo` output; fish itself is the win over bash
#   - the `claude`/`codex` aliases: they call a host `ollama launch` service
#     wrapper, neither reproducible nor dev-relevant here
#
# Toolchain (verified against the pinned nixpkgs rev):
#   Rust:    cargo/rustc + clippy, rustfmt and rust-analyzer, for the backend/
#            crate (lososd + losos-ctl) and backend-registrar/. `cargo test`
#            runs the suites; `nix build .#losos-ctl` runs them again at build
#            time via doCheck. No rust-toolchain.toml is shipped: the nix
#            builders use nixpkgs' rustc, so a channel pin here would be
#            silently ignored by the build that actually ships.
#   Node:    pkgs.nodejs, for design-system/react/ only — a dev-machine-only
#            TypeScript/React wrapper (npm install/build/typecheck/test) around
#            the shipped tokens.css/losos.css. It is deliberately outside the
#            Nix closure for every losos system: the design system that
#            actually ships is the plain CSS under design-system/, served at
#            /ds/ by the front nginx vhost. Node exists in this shell purely
#            so the npm scripts have somewhere to run.
#
# There is no Haskell toolchain any more — the backend was a cabal project
# until the Rust rewrite, and GHC (plus its ghcWithPackages closure, the
# heaviest thing in this shell) left with the last .hs file.
#
# `--no-config` makes fish ignore ~/.config/fish (the home-manager symlink) so
# only the rebuilt config below is loaded; `--init-command` sources it before
# going interactive. The `case $- in *i*)` guard keeps `nix develop -c <cmd>`
# working under bash instead of being hijacked. Note that `nix develop -c`
# does not change directory, so cargo invocations from the repo root need
# `--manifest-path backend/Cargo.toml`.
{ pkgs }:
let
  fishInit = pkgs.writeText "devshell-fish-init.fish" ''
    set -g fish_greeting
    fastfetch

    alias cat 'bat --paging=never'
    alias ls  'eza -lhi --git --icons always'
    alias cd  z

    ${pkgs.lib.getExe pkgs.zoxide} init fish | source
    ${pkgs.lib.getExe pkgs.starship} init fish | source

    # Dev shortcuts for the two backends.
    alias bcd 'cd backend'                # control plane (lososd + losos-ctl)
    alias rcd 'cd backend-registrar'      # master-proxy registrar
  '';
in
{
  default = pkgs.mkShell {
    nativeBuildInputs = [
      # ── Rust ─────────────────────────────────────────────────────────────
      pkgs.cargo
      pkgs.rustc
      pkgs.clippy
      pkgs.rustfmt
      pkgs.rust-analyzer
      # ── Node (design-system/react/ only, dev-machine-only) ──────────────
      pkgs.nodejs
      # ── Shell UX (host config, minus Zellij) ─────────────────────────────
      pkgs.fish
      pkgs.bat
      pkgs.eza
      pkgs.zoxide
      pkgs.fastfetch
      pkgs.starship
    ];

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
