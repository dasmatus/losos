# shellcheck shell=bash
# Shared by the git hooks in this folder (pre-commit, pre-push, commit-msg).
# Sourced, not run. The hooks replay CI's gates for the files a commit or a
# push touches, using .github/scripts/ci-changes.sh to pick the same lanes CI
# would, so a change that would turn main red is stopped on the machine that
# made it.
#
# What each hook runs is in its own header. Everything here is plumbing:
# finding the tools (cargo is the flake's pinned toolchain; bun comes from
# the flake when it is not on PATH), running one check with its output in a log file,
# and the summary at the end.
#
#   LOSOS_HOOKS_SKIP=name,name   skip these checks, by the names the hook
#                                prints; each skip is printed, never silent
#   LOSOS_HOOKS_VERBOSE=1        stream every check's output, not just a
#                                failing one's tail
#
# Logs: $(git rev-parse --git-common-dir)/losos-hooks/logs/<time>-<hook>/,
# one file per check; the last 20 runs are kept.
#
# A check that passed is remembered against the tree it ran on (hook_tree,
# a git tree id), under losos-hooks/passed/, so pushing a commit whose tree
# pre-commit already checked does not run the same check twice. A tree id
# names the exact content, so a change anywhere runs everything again.

set -uo pipefail

hook_root=$(git rev-parse --show-toplevel)
hook_common=$(cd "$(git rev-parse --git-common-dir)" && pwd)
hook_logs_base="$hook_common/losos-hooks/logs"
hook_passed_base="$hook_common/losos-hooks/passed"
# Set by a hook when what it checks is exactly one git tree.
hook_tree=""

# The flake of the tree being checked, or of the checkout given. A shallow
# clone, and a worktree of one, evaluates only with ?shallow=1.
hook_flake() {
  local dir=${1:-$hook_root}
  if [ "$(git -C "$dir" rev-parse --is-shallow-repository)" = true ]; then
    echo "git+file://$dir?shallow=1"
  else
    echo "git+file://$dir"
  fi
}

# Every linted crate, as devenv.nix's `linted` and CI's clippy job list them.
hook_crates=(
  backend
  backend/access
  backend/plugins
  backend/mdx
  edge/registrar
  edge/vercel
  admin-ui/lab/core
  admin-ui/lab/virt-rpc
  admin-ui/lab/render
  admin-ui/tui
  edge/contracts/solana
)

hook_failed=()
hook_passed=()
hook_skipped=()

hook_start() {
  hook_name=$1
  hook_logdir="$hook_logs_base/$(date -u +%Y%m%dT%H%M%SZ)-$hook_name"
  mkdir -p "$hook_logdir"
  # Keep the last 20 runs.
  # shellcheck disable=SC2012 # names are ours: a timestamp and a hook name
  ls -1d "$hook_logs_base"/*/ 2>/dev/null | sort | head -n -20 | xargs -r rm -rf
  # And what passed on trees not seen for a week.
  [ -d "$hook_passed_base" ] && find "$hook_passed_base" -mindepth 1 -maxdepth 1 -mtime +7 -exec rm -rf {} +
  hook_t0=$(date +%s)
  echo "losos $hook_name: logs in $hook_logdir"
}

hook_skipping() {
  local name=$1
  case ",${LOSOS_HOOKS_SKIP:-}," in
    *",$name,"* | *",all,"*) return 0 ;;
  esac
  return 1
}

# git exports the repository to its hooks: GIT_DIR (in a linked worktree)
# and GIT_INDEX_FILE (in pre-commit). The hook's own git calls want them; a
# check must not see them. A test that makes a repository of its own (the
# backend's text search, config repo and forge suites) would otherwise
# commit into, and reconfigure, the repository being checked.
hook_git_env=(GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_PREFIX
  GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_NAMESPACE)
hook_unexport_git() {
  unset "${hook_git_env[@]}"
}

# run <name> <command…>: one check, its output in <logdir>/<name>.log,
# without git's repository variables.
run() {
  local name=$1
  shift
  if hook_skipping "$name"; then
    echo "  - $name: SKIPPED (LOSOS_HOOKS_SKIP)"
    hook_skipped+=("$name")
    return 0
  fi
  local file=${name//\//_}
  local log="$hook_logdir/$file.log" t0 rc mark=""
  if [ -n "$hook_tree" ]; then
    mark="$hook_passed_base/$hook_tree/$file"
    if [ -e "$mark" ]; then
      printf '  ✓ %s (passed on this tree before: %s)\n' "$name" "$(cat "$mark")"
      hook_passed+=("$name")
      return 0
    fi
  fi
  t0=$(date +%s)
  printf '  … %s\n' "$name"
  {
    echo "# $name"
    echo "# in $PWD"
    printf '# %q ' "$@"
    echo
  } >"$log"
  if [ "${LOSOS_HOOKS_VERBOSE:-}" = 1 ]; then
    (hook_unexport_git && "$@") 2>&1 | tee -a "$log"
    rc=${PIPESTATUS[0]}
  else
    (hook_unexport_git && "$@") >>"$log" 2>&1
    rc=$?
  fi
  local dt=$(($(date +%s) - t0))
  if [ "$rc" -eq 0 ]; then
    printf '  ✓ %s (%ss)\n' "$name" "$dt"
    hook_passed+=("$name")
    if [ -n "$mark" ]; then
      mkdir -p "${mark%/*}"
      echo "$log" >"$mark"
    fi
  else
    printf '  ✗ %s (%ss, exit %s): %s\n' "$name" "$dt" "$rc" "$log"
    tail -n 40 "$log" | sed 's/^/      /'
    hook_failed+=("$name")
  fi
  return 0
}

hook_finish() {
  local dt=$(($(date +%s) - hook_t0))
  echo "losos $hook_name: ${#hook_passed[@]} passed, ${#hook_failed[@]} failed, ${#hook_skipped[@]} skipped in ${dt}s"
  if [ ${#hook_failed[@]} -gt 0 ]; then
    echo "losos $hook_name: refused. Failed: ${hook_failed[*]}" >&2
    echo "  Full logs: $hook_logdir" >&2
    echo "  Fix them, or skip one on purpose with LOSOS_HOOKS_SKIP=<name>." >&2
    exit 1
  fi
  exit 0
}

# The lanes CI would run for the files on stdin, as `lane_<name>=true|false`
# shell variables (rust, wasm, eval, ui, …; see .github/scripts/ci-changes.sh).
hook_lanes() {
  local line
  while IFS= read -r line; do
    case $line in
      *=true | *=false) eval "lane_${line%%=*}=${line#*=}" ;;
    esac
  done < <(bash "$hook_root/.github/scripts/ci-changes.sh")
}

# The crates a set of changed files reaches, its dependents included: a crate
# that depends on another by path is linted and tested with it, as CI does
# by running every crate.
hook_crates_for() {
  local f c
  declare -A want=()
  while IFS= read -r f; do
    for c in "${hook_crates[@]}"; do
      case $f in "$c"/*) want[$c]=1 ;; esac
    done
    case $f in
      backend/access/*) want[backend]=1 want[edge/registrar]=1 want[edge/vercel]=1 want[admin-ui/tui]=1 ;;
      backend/plugins/*) want[backend]=1 ;;
      edge/registrar/*) want[edge/vercel]=1 ;;
      admin-ui/lab/core/*) want[admin-ui/lab/render]=1 ;;
      modules/overrides.nix) want[backend]=1 ;;
      admin-ui/app/src/styles/basic-classes.txt) want[backend/plugins]=1 ;;
      admin-ui/app/src/i18n/* | admin-ui/app/src/styles/tokens.css | \
        admin-ui/app/src/screens/settings/panes.ts | admin-ui/themes/brand/salmon-128.png)
        want[admin-ui/tui]=1
        ;;
    esac
  done
  # The backend crate's path dependencies live inside it.
  [ -n "${want[backend/access]:-}" ] && want[backend]=1
  for c in "${hook_crates[@]}"; do
    [ -n "${want[$c]:-}" ] && echo "$c"
  done
}

# Prefer nix-portable when system nix is missing or unusable.
if [ -x "$hook_root/tools/ensure-nix-portable" ]; then
  # shellcheck source=/dev/null
  . "$hook_root/tools/ensure-nix-portable"
fi

# cargo from the flake's pinned toolchain, the rustc CI lints, tests and
# ships with: the one on PATH inside devenv or nix develop, else the flake's
# CI shell. A cargo from elsewhere (rustup) is a different compiler, and its
# clippy can disagree with CI's; it is used only when there is no nix.
hook_cargo() {
  local found
  found=$(command -v cargo 2>/dev/null || true)
  case $found in
    /nix/store/*) cargo "$@" ;;
    *)
      if command -v nix >/dev/null 2>&1; then
        nix develop "$(hook_flake)#ci" -c cargo "$@"
      elif [ -n "$found" ]; then
        echo "note: $found is not the flake's toolchain; CI may disagree" >&2
        cargo "$@"
      else
        echo "cargo is not on PATH and nix is not installed: enter devenv shell or nix develop" >&2
        return 127
      fi
      ;;
  esac
}

# A program from the flake's nixpkgs when it is not on PATH.
# hook_tool <program> <nixpkgs attribute> <args…>
hook_tool() {
  local prog=$1 attr=$2
  shift 2
  if command -v "$prog" >/dev/null 2>&1; then
    "$prog" "$@"
  elif command -v nix >/dev/null 2>&1; then
    nix shell --inputs-from "$(hook_flake)" "nixpkgs#$attr" -c "$prog" "$@"
  else
    echo "$prog is not on PATH and nix is not installed" >&2
    return 127
  fi
}

hook_bun() { hook_tool bun bun "$@"; }

# The WebAssembly packages admin-ui/app imports and git ignores: Diagrams'
# three (src/lab/{core,virt,render}-pkg) and losos-mdx (src/mdx/pkg). tsc
# needs them, so a checkout without one gets the package the flake builds
# (`bun run lab:<name>` and `bun run mdx` would need cargo for wasm32 and
# the pinned wasm-bindgen). A copy made here names its store path in
# .losos-hooks-out and is replaced once the crate builds to another one,
# since a stale package fails tsc on whatever the crate added since. One
# made by `bun run` has no such file and is left alone.
hook_wasm_pkgs() {
  local pair rel attr out dir stamp
  for pair in src/lab/core-pkg:losos-lab-core src/lab/virt-pkg:losos-lab-virt \
    src/lab/render-pkg:losos-lab-render src/mdx/pkg:losos-mdx-wasm; do
    rel=${pair%%:*} attr=${pair#*:}
    dir=$hook_root/admin-ui/app/$rel
    stamp=$dir/.losos-hooks-out
    [ -d "$dir" ] && [ ! -f "$stamp" ] && continue
    hook_need_nix || return 1
    out=$(nix build --no-link --print-out-paths "$(hook_flake)#$attr") || return 1
    [ -f "$stamp" ] && [ "$(cat "$stamp")" = "$out" ] && continue
    echo "── $rel from .#$attr"
    rm -rf "$dir"
    mkdir -p "$dir"
    cp -r "$out"/. "$dir"/
    chmod -R u+w "$dir"
    echo "$out" >"$stamp"
  done
}

hook_need_nix() {
  if ! command -v nix >/dev/null 2>&1; then
    echo "nix is not installed; this check evaluates or builds the flake" >&2
    return 127
  fi
}

# Make the flake's GitHub-hosted inputs resolvable where GitHub's tarball
# endpoint is not (a proxy that allows git but not codeload): fetch the
# locked rev over git and add it to the store under the locked NAR hash.
# A no-op where the inputs fetch normally or are already in the store.
hook_inputs() {
  command -v python3 >/dev/null 2>&1 || return 0
  python3 "$hook_root/nix/hooks/flake-inputs.py" "$hook_root"
}

# rustfmt --check over every crate, as CI's clippy job does on a PR.
hook_fmt_all() {
  local c rc=0 all
  for c in "${hook_crates[@]}"; do
    all=""
    [ "$c" = backend/plugins ] && all=--all
    echo "── $c"
    # shellcheck disable=SC2086 # $all is one flag or none
    hook_cargo fmt $all --manifest-path "$c/Cargo.toml" --check || rc=1
  done
  return "$rc"
}

# nixfmt, deadnix and statix over these .nix files: nix/lint-nix.sh, the
# script CI's eval job and devenv's lint-nix run over the whole tree.
hook_nix_lints() {
  [ $# -gt 0 ] || return 0
  run nix-lints bash "$hook_root/nix/lint-nix.sh" "$@"
}

# The eval lane: CI's eval job, every step of it.
hook_eval_checks() {
  run flake-inputs hook_inputs
  run check-flake bash "$hook_root/nix/check-flake.sh"
  local cfg
  for cfg in install iso edge-gateway; do
    run "eval-$cfg" nix eval --raw "$(hook_flake)#nixosConfigurations.$cfg.config.system.build.toplevel.drvPath"
  done
  run eval-iso-image nix path-info --derivation "$(hook_flake)#nixosConfigurations.iso.config.system.build.isoImage"
  run eval-edge-qcow2 nix path-info --derivation "$(hook_flake)#losos-disk-edge-qcow2"
}
