# How every Rust binary this repo ships is compiled and linked — mold as the
# linker, ccache in front of the C compiler, sccache in front of rustc — in
# ONE place, consumed by the three things that compile Rust here:
#
#   flake.nix           devShells.default and devShells.ci. CI's clippy and
#                       test jobs run `cargo` inside the latter, so CI gets
#                       this for free.
#   devenv.nix          the dev shell proper, through its `stdenv`, `env`
#                       and `packages` options.
#   flake/packages.nix  losos-ctl (lososd + losos-ctl) and losos-registrar,
#                       through `buildRustPackage` below.
#
# A plain function of `pkgs`, not a flake output, on purpose: devenv.nix runs
# standalone against its own nixpkgs instance (flake.nix's `inputs` block
# explains why), and a function both can import is the one shape that keeps
# them on a single definition. The two instances pin the same nixpkgs
# (check-pins), so the stdenv built here is the same on both sides.
#
# edge-vercel is the one crate this cannot reach: Vercel compiles it on its
# own builders, which carry neither mold nor a compiler cache. In the shells
# it is built like the other two; on Vercel it is built as before.
#
# What each piece buys here, and what it cannot:
#
#   mold     The linker, for every link: rustc's final link of each binary and
#            test harness, and anything the `cc` crate links. A deterministic
#            drop-in for GNU ld with no cache and no host setup, so it is not
#            gated on anything — the Nix builds and the shells use it alike,
#            and `readelf -p .comment` on any binary names it. Codegen is
#            untouched; this is the link step only, which on a `cargo test`
#            rebuild of a one-line change is most of the wait.
#
#   ccache   Caches C and C++ compiles. Across these crates that is what
#            `ring` builds through the `cc` crate — small, and ccache does not
#            understand rustc and never will. It is here because the ask was
#            ccache, and because it is harmless; it is not where the minutes
#            go. It wraps the stdenv's compiler (nixpkgs' ccacheStdenv), so
#            `cc` and `gcc` on PATH are ccache in front of gcc in the shells
#            and the builds alike — the only spelling that survives
#            buildRustPackage, whose cargoBuildHook sets
#            CC_x86_64_unknown_linux_gnu to the stdenv's compiler on the
#            cargo command line, over anything exported before it.
#
#   sccache  The rustc half of the same idea. Each dependency crate's rlib is
#            keyed on its source, flags and inputs, so a `cargo clean`, a lost
#            target/ tree, or the third crate compiling tokio again (the repo
#            is deliberately not a workspace, so backend-registrar and
#            edge-vercel share no target/) costs seconds, not minutes. It does
#            not cache a crate built incrementally, which is the local crate
#            in a dev profile — that one is small anyway. It reaches rustc
#            through a one-line wrapper script rather than as RUSTC_WRAPPER=
#            sccache directly, because the `cc` crate, seeing that exact
#            name there, puts sccache in front of the C compiler as well and
#            runs it with CCACHE_DISABLE set — which would leave ccache
#            wrapping nothing. Under a name it does not recognise, C stays
#            ccache's and Rust is sccache's.
#
# Inside the Nix sandbox nothing but /build is writable, so a cache there is
# empty on every build and worth less than none. Both caches are therefore
# GATED on a directory the host chose to expose to the sandbox:
# /var/cache/ccache and /var/cache/sccache, writable by the build user. Where
# they are not, ccache's wrapper sets CCACHE_DISABLE (ccache then execs the
# compiler and touches nothing) and sccache is never started, and the build
# is the plain stdenv build it was before — byte for byte, since both tools
# store the real compiler's output and nothing else. That default is
# load-bearing: the appliance rebuilds itself at 03:00 from its upgrade URI
# on a box with no shell and nothing exposed, and a cache that could fail a
# build there would surface as a box that stopped updating.
#
# To turn both on for `nix build` on a dev machine (NixOS: `nix.settings
# .extra-sandbox-paths` plus a `systemd.tmpfiles` rule, or
# `programs.ccache.cacheDir` for the first one):
#
#   sudo mkdir -m 0770 -p /var/cache/ccache /var/cache/sccache
#   sudo chown root:nixbld /var/cache/ccache /var/cache/sccache
#   # /etc/nix/nix.conf:
#   extra-sandbox-paths = /var/cache/ccache /var/cache/sccache
#
# Each build then says which it took at the start of buildPhase and prints
# sccache's hit rate at the end, which is also the proof that the directory
# was actually used and not merely created. One limit worth knowing on a
# multi-user daemon host: ccache honours CCACHE_UMASK and the nixbld users
# share its directory fully, but sccache writes its entries 0600 whatever
# the umask, so what nixbld1 cached nixbld2 cannot read. Builds run one at
# a time all land on nixbld1 and share everything; only builds running in
# parallel miss each other's entries.
#
# In the shells the caches need no setup: with CCACHE_DIR and SCCACHE_DIR
# unset both tools use ~/.cache/{ccache,sccache}, which CI persists between
# runs with actions/cache (see .github/workflows/ci.yml).
{
  pkgs,
  lib ? pkgs.lib,
}:

let
  ccacheDir = "/var/cache/ccache";
  sccacheDir = "/var/cache/sccache";

  sccacheExe = lib.getExe pkgs.sccache;

  # gcc with mold behind it. The adapter adds ld.mold to the bintools the
  # cc wrapper searches with -B, and puts -fuse-ld=mold on the link line of
  # every derivation and shell built from this stdenv (through
  # NIX_CFLAGS_LINK, which the cc wrapper reads on each invocation). rustc
  # links through that same `cc`, so it needs no flag of its own — and
  # nixpkgs' rustc disables upstream's self-contained lld default, so
  # nothing else is competing for the job (checked: the link line carries no
  # -fuse-ld before this adds one).
  moldStdenv = pkgs.stdenvAdapters.useMoldLinker pkgs.stdenv;

  # ...and ccache in front of the compiler. nixpkgs' ccacheStdenv wraps
  # `cc`, `gcc`, `c++` and `g++` with `ccache <unwrapped compiler>` and
  # leaves the rest of the cc wrapper — the mold bintools included — as it
  # was. extraConfig runs inside that wrapper on every call, which is the
  # one place that can tell a build from a shell.
  #
  # mold first, then ccache, and not the other way round: the adapter
  # decides whether gcc is new enough for -fuse-ld=mold from
  # `stdenv.cc.version`, and behind ccacheStdenv that reads as ccache's
  # version (4.x), which fails the ">= 12" check and silently leaves mold
  # out. Composed this way the check sees gcc 15.
  stdenv = pkgs.ccacheStdenv.override {
    stdenv = moldStdenv;
    extraConfig = ''
      # Every file in the store carries mtime 1, so ccache's default compiler
      # check (mtime + size) cannot tell one gcc from another. Hash the
      # binary instead; it is the unwrapped driver, ~1 MiB.
      export CCACHE_COMPILERCHECK=content
      # Nix gives every build HOME=/homeless-shelter; a shell keeps the
      # user's. Inside a build the only usable cache is the one the host
      # exposed, so take it if it is there and stand down if it is not.
      if [ "''${HOME:-}" = /homeless-shelter ]; then
        export CCACHE_DIR=${ccacheDir}
        # Shared between the nixbld users, hence group-writable.
        export CCACHE_UMASK=007
        [ -w "$CCACHE_DIR" ] || export CCACHE_DISABLE=1
      fi
    '';
  };

  # What cargo runs rustc through. A script of its own name, not sccache
  # itself, for the reason in the header: the `cc` crate recognises
  # "sccache" in RUSTC_WRAPPER and fronts the C compiler with it too.
  rustcWrapper = pkgs.writeShellScript "rustc-via-sccache" ''
    exec ${sccacheExe} "$@"
  '';

  # The sccache gate for `nix build`, as a buildPhase hook: on when the host
  # exposed the directory, else never started. (ccache's gate is in the
  # wrapper above, and needs no hook.)
  #
  # The server is started here, explicitly, rather than left to the first
  # client, for two reasons that each cost a build to learn:
  #   - a server that cargo's first client spawns inherits that client's
  #     stdout and stderr, which are pipes cargo reads to EOF, and cargo
  #     then waits on them for as long as the server lives: forever.
  #     Started here, undaemonised (SCCACHE_NO_DAEMON) with its stdio closed,
  #     it holds nothing of cargo's;
  #   - on a unix socket inside the build directory, not the default
  #     127.0.0.1:4226, so an unsandboxed build can never reach a server a
  #     dev shell on the same host left running (and vice versa). The
  #     sandbox would isolate the port anyway; the socket does it everywhere.
  # Clients find it through the same SCCACHE_SERVER_UDS. If it has not come
  # up within ten seconds the build goes on without it, uncached, and says
  # so — a cache is never a reason to fail a build.
  cachesOn = ''
    if [ -w ${ccacheDir} ]; then
      echo "fast-build: ${ccacheDir} is writable in this sandbox, caching C with ccache"
    else
      echo "fast-build: ${ccacheDir} not exposed to the sandbox, C compiles run uncached"
    fi
    if [ -w ${sccacheDir} ]; then
      echo "fast-build: ${sccacheDir} is writable in this sandbox, caching rustc with sccache"
      export SCCACHE_DIR=${sccacheDir}
      export SCCACHE_SERVER_UDS="$NIX_BUILD_TOP/sccache.sock"
      SCCACHE_NO_DAEMON=1 ${sccacheExe} --start-server </dev/null >/dev/null 2>&1 &
      for _ in $(seq 100); do
        [ -S "$SCCACHE_SERVER_UDS" ] && break
        sleep 0.1
      done
      if [ -S "$SCCACHE_SERVER_UDS" ] && ${sccacheExe} --show-stats >/dev/null 2>&1; then
        export RUSTC_WRAPPER=${rustcWrapper}
      else
        echo "fast-build: sccache's server did not come up, rustc runs uncached"
        unset SCCACHE_DIR SCCACHE_SERVER_UDS
      fi
    else
      echo "fast-build: ${sccacheDir} not exposed to the sandbox, rustc runs uncached"
    fi
  '';
  # The hit rate in the build log is the proof that the cache was taken;
  # stopping the server is tidiness for a build that runs unsandboxed.
  cachesOff = ''
    if [ -n "''${SCCACHE_DIR:-}" ]; then
      ${sccacheExe} --show-stats || true
      ${sccacheExe} --stop-server >/dev/null 2>&1 || true
    fi
  '';

  # buildRustPackage over the stdenv above. cargoSetupHook writes
  # `linker = <this stdenv's cc>` into the build's cargo config, which is
  # how rustc's link ends up on the ccache-and-mold `cc` rather than the
  # stock one. `rustc` and `cargo` are the stock ones: it is the C
  # compiler, the linker and the caches that change, not the Rust
  # toolchain, so the lint shells and the package builds still share one
  # rustc.
  rustPlatform = pkgs.makeRustPlatform {
    inherit (pkgs) rustc cargo;
    inherit stdenv;
  };

  # The one place the stdenv above does NOT reach on its own: the `cc`
  # crate's compiler. cargoBuildHook prefixes `cargo build` with an `env`
  # line (nixpkgs' rust.envVars.setEnv) that sets HOST_CC, HOST_CXX,
  # CC_X86_64_UNKNOWN_LINUX_GNU and CARGO_TARGET_…_LINKER to the STOCK
  # stdenv's cc — computed from pkgs.stdenv, not from the stdenv handed to
  # makeRustPlatform — and the `cc` crate reads HOST_CC before CC. ring's C
  # would compile uncached through the stock gcc while everything else goes
  # through ccache, with nothing in the log to say so.
  # So the two hooks carrying that line are rebuilt from nixpkgs' own hook
  # scripts with the stock cc's path swapped for this stdenv's. The script
  # paths and the @setEnv@/@rustcTargetSpec@ placeholders are nixpkgs
  # internals, chosen over copying the scripts in: a nixpkgs bump that
  # moves a script fails evaluation (the path below is a path, not a
  # string, so a missing file is an eval error CI's eval job sees), and
  # one that renames a placeholder fails the hook's build, where a stale
  # copy would silently keep building with whatever it was copied from.
  hooksDir = pkgs.path + "/pkgs/build-support/rust/hooks";
  setEnv =
    builtins.replaceStrings [ "${pkgs.stdenv.cc}/bin/" ] [ "${stdenv.cc}/bin/" ]
      pkgs.rust.envVars.setEnv;
  hook =
    name:
    pkgs.makeSetupHook {
      inherit name;
      substitutions = {
        inherit (stdenv.targetPlatform.rust) rustcTargetSpec;
        inherit setEnv;
      };
    } (hooksDir + "/${name}");
  buildRustPackage' = rustPlatform.buildRustPackage.override {
    cargoBuildHook = hook "cargo-build-hook.sh";
    cargoCheckHook = hook "cargo-check-hook.sh";
  };
in
{
  inherit stdenv;

  # For the shells. `cc` and `gcc` on PATH come from the stdenv and are
  # already ccache in front of gcc with mold behind it, so the packages are
  # the tools by their own names (ccache -s, sccache --show-stats, mold
  # --version), and `env` is the one export the shell needs.
  packages = [
    pkgs.mold
    pkgs.ccache
    pkgs.sccache
  ];
  env = {
    # cargo runs rustc through this. sccache passes clippy-driver straight
    # through (it caches rustc, not lints), so `cargo clippy` is unaffected
    # beyond its dependencies' rlibs, which are rustc's and do get cached.
    RUSTC_WRAPPER = rustcWrapper;
  };

  buildRustPackage =
    args:
    buildRustPackage' (
      args
      // {
        preBuild = (args.preBuild or "") + cachesOn;
        postBuild = cachesOff + (args.postBuild or "");
      }
    );
}
