# devenv.sh configuration — the single entry point for developing losos.
#
# Run standalone by the devenv CLI against devenv.yaml, NOT wired into the
# flake. `devenv.lib.mkShell` cannot evaluate purely — it needs an absolute
# path to the project root for `.devenv/`, and the documented escape needs
# --impure, which would spread to CI. The cost is a second nixpkgs pin, in
# devenv.yaml; `check-pins` below asserts it still agrees with flake.lock.
# The long version of that argument is in flake.nix's `inputs` block.
#
# Everything below is dev-machine-only. None of it enters any losos system
# closure — same rule as admin-ui/design-system/react.
{ pkgs, lib, ... }:

let
  # Both crates, since this repo is deliberately not a Cargo workspace: each
  # has its own Cargo.lock and its own pinned cargoHash in flake/packages.nix.
  # Every Rust command therefore has to be run twice with --manifest-path.
  crates = [
    "backend"
    "backend-registrar"
  ];
  forEachCrate = cmd: lib.concatMapStringsSep "\n" (c: ''echo "── ${c}"; ${cmd c}'') crates;

  # Print the attribute names of one of this flake's per-system output sets,
  # space separated, for the loops in `vm-tests`, `build-pkgs` and
  # `build-images` to iterate over.
  #
  # Asking the flake rather than writing the list down here is the whole point.
  # `vm-tests` used to carry four hand-typed names against six exported checks,
  # under a description that said "all four" — and the two it had never been
  # taught about, losos-front-vhost and losos-impermanence, were exactly the
  # two the k3s/fscrypt rework broke. A local gate that quietly skips whatever
  # nobody remembered to add to it does not merely miss the regression: it
  # prints a row of green ticks over the top of it. Same for `build-pkgs` and
  # the three OCI images. If you add a check or a package, this picks it up;
  # do not "clarify" it back into a literal list.
  #
  # x86_64-linux is a literal because the flake exports exactly one system (see
  # `system` in flake.nix). builtins.attrNames forces the set and none of its
  # values, so this stays instant even though every value behind it is a VM
  # test or a multi-gigabyte image.
  flakeAttrs =
    output:
    "command nix eval --raw --apply 'as: builtins.concatStringsSep \" \" (builtins.attrNames as)' .#${output}.x86_64-linux";

  # The host fish config minus Zellij, and minus the claude/codex aliases,
  # which call a host ollama wrapper that is neither reproducible nor relevant
  # here. Carried over verbatim from flake/devshell.nix.
  fishInit = pkgs.writeText "devshell-fish-init.fish" ''
    set -g fish_greeting
    fastfetch

    alias cat 'bat --paging=never'
    alias ls  'eza -lhi --git --icons always'
    alias cd  z

    ${lib.getExe pkgs.zoxide} init fish | source
    ${lib.getExe pkgs.starship} init fish | source

    # Dev shortcuts for the two backends.
    alias bcd 'cd backend'                # control plane (lososd + losos-ctl)
    alias rcd 'cd backend-registrar'      # master-proxy registrar
  '';
in
{
  name = "losos";

  # ── Toolchain ────────────────────────────────────────────────────────────
  # channel = "nixpkgs" on purpose. The repo ships no rust-toolchain.toml
  # because the Nix builders use nixpkgs' rustc regardless; pinning a
  # different channel here would mean linting with one compiler and shipping
  # with another, which is how a clippy-clean tree still fails in CI.
  languages.rust = {
    enable = true;
    channel = "nixpkgs";
    components = [
      "rustc"
      "cargo"
      "clippy"
      "rustfmt"
      "rust-analyzer"
    ];
  };

  # admin-ui/design-system/react only — a dev-machine-only TypeScript wrapper
  # around the shipped tokens.css/losos.css. Never in any system closure.
  #
  # `bun.install.enable` is deliberately left off. It auto-runs `bun install`
  # on shell entry and warns that yarn.lock/package-lock.json are obsolete —
  # and that package has a committed package-lock.json driving `npm run
  # typecheck`/`npm test`. Enabling bun the *tool* is free; migrating the
  # lockfile is a separate decision.
  languages.javascript = {
    enable = true;
    bun.enable = true;
  };

  packages = with pkgs; [
    # Nix tooling the git hooks below call.
    nixfmt-rfc-style
    deadnix
    statix

    # Handy when poking at the appliance's own surfaces.
    jq
    curl

    # Shell UX, carried over from the flake/devshell.nix this file replaces:
    # the host fish config minus Zellij, which would wrap the shell in a
    # multiplexer and make cargo output noisier for no gain.
    fish
    bat
    eza
    zoxide
    fastfetch
    starship
  ];

  # ── Pre-commit hooks ─────────────────────────────────────────────────────
  # These are the gate CI runs, moved to before the commit instead of after
  # the push. Until the `lint` CI job was added, nothing enforced clippy or
  # rustfmt anywhere, and backend-registrar had drifted to 17 rustfmt hunks
  # plus a clippy error without anyone noticing.
  #
  # The stock `rustfmt`/`clippy` hooks are not used: they assume one crate at
  # the repo root, and this tree has two crates in subdirectories with no
  # workspace linking them. Custom entries pass --manifest-path for each.
  git-hooks.hooks = {
    rustfmt-both = {
      enable = true;
      name = "rustfmt (both crates)";
      entry = "${pkgs.writeShellScript "rustfmt-both" ''
        set -e
        ${forEachCrate (c: "cargo fmt --manifest-path ${c}/Cargo.toml --check")}
      ''}";
      files = "\\.rs$";
      pass_filenames = false;
    };

    clippy-both = {
      enable = true;
      name = "clippy -D warnings (both crates)";
      entry = "${pkgs.writeShellScript "clippy-both" ''
        set -e
        ${forEachCrate (c: "cargo clippy --manifest-path ${c}/Cargo.toml --all-targets -- -D warnings")}
      ''}";
      files = "\\.(rs|toml)$";
      pass_filenames = false;
    };

    nixfmt-rfc-style.enable = true;
    deadnix.enable = true;
    statix.enable = true;

    # Codeberg's Terms of Use ban repos that mostly consist of generated code,
    # and an attribution trailer is the cheapest thing to flag on. Keep the
    # history free of them.
    no-attribution-trailers = {
      enable = true;
      name = "no attribution trailers in the commit message";
      entry = "${pkgs.writeShellScript "no-attribution-trailers" ''
        set -e
        msg="$1"
        [ -n "$msg" ] && [ -f "$msg" ] || exit 0
        if grep -qiE '^(co-authored-by|generated[- ]with):' "$msg"; then
          echo "commit message carries an attribution trailer; drop it" >&2
          exit 1
        fi
      ''}";
      stages = [ "commit-msg" ];
    };
  };

  # ── Scripts ──────────────────────────────────────────────────────────────
  # `nix` is invoked as `command nix` throughout: on the maintainer's machine
  # `nix` is a shell alias to a toolbox wrapper that fails when
  # NIX_TOOLBOX_STORE is unset, and the alias would otherwise shadow the real
  # binary inside these scripts.

  scripts.fmt.exec = forEachCrate (c: "cargo fmt --manifest-path ${c}/Cargo.toml");
  scripts.fmt.description = "Format both Rust crates.";

  scripts.lint.exec = ''
    set -e
    ${forEachCrate (c: "cargo fmt --manifest-path ${c}/Cargo.toml --check")}
    ${forEachCrate (c: "cargo clippy --manifest-path ${c}/Cargo.toml --all-targets -- -D warnings")}
  '';
  scripts.lint.description = "rustfmt --check + clippy -D warnings, both crates.";

  scripts.test-rust.exec = forEachCrate (c: "cargo test --manifest-path ${c}/Cargo.toml");
  scripts.test-rust.description = "Unit tests for both Rust crates.";

  scripts.check-flake.exec = "command nix flake check --no-build -L";
  scripts.check-flake.description = "Evaluate every flake output (builds nothing).";

  # devenv runs standalone (see flake.nix), so there are two lock files:
  # flake.lock pins the nixpkgs that BUILDS the appliance, devenv.lock pins the
  # nixpkgs that provides the rustc which LINTS and TESTS it. Nothing in either
  # tool enforces agreement. If they drift, the tree is clippy-clean here and
  # fails inside `nix build .#losos-ctl`, or the reverse — an expensive and
  # very confusing failure. This is the whole cost of standalone mode, so it is
  # checked mechanically rather than left to whoever remembers.
  scripts.check-pins.exec = ''
    set -eu
    lock=$(${lib.getExe pkgs.jq} -r '.nodes.nixpkgs.locked.rev' flake.lock)
    yaml=$(${pkgs.gnugrep}/bin/grep -oE '[0-9a-f]{40}' devenv.yaml | head -1)
    if [ "$lock" != "$yaml" ]; then
      echo "nixpkgs pins disagree:" >&2
      echo "  flake.lock  $lock" >&2
      echo "  devenv.yaml $yaml" >&2
      echo "Bump both together: nix flake update nixpkgs, copy the rev into" >&2
      echo "devenv.yaml, then devenv update." >&2
      exit 1
    fi
    echo "nixpkgs pin agrees across flake.lock and devenv.yaml ($lock)"
  '';
  scripts.check-pins.description = "Assert flake.lock and devenv.yaml pin the same nixpkgs.";

  # flake check only forces each nixosConfiguration's toplevel *derivation*;
  # these two force the full module merge, which is what actually catches
  # option conflicts, type errors and failed assertions.
  scripts.check-eval.exec = ''
    set -e
    for cfg in install iso; do
      echo "── evaluating nixosConfigurations.$cfg"
      command nix eval --raw \
        ".#nixosConfigurations.$cfg.config.system.build.toplevel.drvPath"
      echo
    done
  '';
  scripts.check-eval.description = "Force the full NixOS module merge for both systems.";

  # Every flake package except the OCI images, which are their own script
  # below because they are their own order of magnitude.
  scripts.build-pkgs.exec = ''
    set -eu
    for p in $(${flakeAttrs "packages"}); do
      case $p in
        losos-image-*) continue ;;
      esac
      echo "── $p"
      command nix build --no-link -L ".#$p"
    done
  '';
  scripts.build-pkgs.description = "Build every flake package except the OCI images.";

  # The three images the LOCAL k3s cluster runs as static pods
  # (flake/images.nix, wired to losos.workloads.* in modules/defaults.nix).
  #
  # Opt-in, and deliberately not part of `devenv test`, because of what they
  # cost: measured on a dev machine, the Nextcloud image's content closure is
  # 2.3 GiB across 244 store paths (nextcloud34 with ~30 apps, php-with-
  # extensions, apacheHttpd) and the tarball dockerTools writes out is of the
  # same order again. That is precisely the "multi-gigabyte closure" the
  # enterTest block below promises not to spend on you. Forgejo is a 585 MiB
  # tarball over 126 paths, pause 51 MiB.
  #
  # It has to be gated *somewhere*, though, and until CI grows a job for it
  # this is the only place. The appliance never builds these: it substitutes
  # them from a binary cache during system.autoUpgrade at 03:00, on a
  # repurposed mini-PC with a tmpfs root, no shell and nobody watching, three
  # hours before an unconditional reboot. `check-flake` does not cover it —
  # `nix flake check --no-build` evaluates these derivations, and evaluation
  # cannot see an entrypoint that fails shellcheck (writeShellApplication runs
  # it at build time) or a buildEnv whose paths collide. Run this before
  # pushing anything that touches flake/images.nix or
  # modules/nextcloud-stack.nix.
  scripts.build-images.exec = ''
    set -eu
    for p in $(${flakeAttrs "packages"}); do
      case $p in
        losos-image-*) ;;
        *) continue ;;
      esac
      echo "── $p"
      command nix build --no-link -L ".#$p"
    done
  '';
  scripts.build-images.description = "Build the OCI images the local cluster runs (gigabytes; opt-in).";

  # The real acceptance gate for the control plane and the appliance's front
  # door. CI cannot run these — the Codeberg runners cap at 10 minutes and 8 GB
  # and offer no /dev/kvm — so they are a local gate, and the only one.
  #
  # The list comes from the flake (see flakeAttrs above for what that fixed).
  scripts.vm-tests.exec = ''
    set -eu
    for t in $(${flakeAttrs "checks"}); do
      echo "══ VM test: $t"
      command nix build --no-link -L ".#checks.x86_64-linux.$t"
    done
  '';
  scripts.vm-tests.description = "Run every nixos-test VM the flake exports (needs /dev/kvm).";

  # Not a local-only gate any more — the `iso` job in
  # .forgejo/workflows/ci.yml builds this on every push, by importing
  # losos-ctl from a sibling job instead of compiling it. This script stays
  # because it is still the fastest way to get an image onto a USB stick, and
  # because `devenv test` does not run it.
  scripts.build-iso.exec = "command nix build --no-link -L .#nixosConfigurations.iso.config.system.build.isoImage";
  scripts.build-iso.description = "Build the installer ISO (local-only gate).";

  # ── devenv test ──────────────────────────────────────────────────────────
  # `devenv test` runs the fast gates: everything that does not need KVM or a
  # multi-gigabyte closure. The VM tests, the OCI images and the ISO stay
  # opt-in — `vm-tests`, `build-images`, `build-iso`.
  enterTest = ''
    set -e
    check-pins
    lint
    test-rust
    check-flake
    check-eval
  '';

  enterShell = ''
    echo "losos dev shell"
    echo "  fmt · lint · test-rust        Rust"
    echo "  check-flake · check-eval      Nix eval gates"
    echo "  build-pkgs · build-images     builds"
    echo "  build-iso                     the installer image"
    echo "  vm-tests                      the real acceptance gate (needs KVM)"
    echo "  devenv test                   all of it bar VM tests, images, ISO"

    # Switch to fish only for an INTERACTIVE shell, detected by the bash `i`
    # flag in $-. This guard is load-bearing: `devenv shell lint` and every CI
    # job run non-interactively, and without the guard the exec below would
    # hijack them into an interactive fish that never runs the command and
    # never exits. --no-config makes fish ignore ~/.config/fish (the
    # home-manager symlink) so only the config built here is loaded.
    case $- in
      *i*) exec ${lib.getExe pkgs.fish} -i -N -C "source ${fishInit}" ;;
    esac
  '';
}
