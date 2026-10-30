#!/usr/bin/env bash
# Decide which lanes of ci.yml a change needs.
#
#   ci-changes.sh < changed-files          one repository path per line
#   ci-changes.sh --full "<reason>"        every lane, and why
#
# Prints `<lane>=true|false` for every lane below, plus `full=` and
# `reason=`, which the changes job appends to $GITHUB_OUTPUT. With
# GITHUB_STEP_SUMMARY set it also writes a table of the lanes and the first
# file that turned each one on.
#
# A lane is a list of patterns, matched in order like a .gitignore: a plain
# pattern adds the files it matches, a `!` pattern takes them away again, and
# the last match wins. `*` matches across `/`, so `backend/*` is everything
# under backend/. A lane is on when any changed file ends up matched.
#
# When in doubt a path belongs in a lane: a lane that runs for nothing costs
# minutes, one that misses a change lets main go red. Main, tags, the weekly
# run, manual runs and pull requests labelled `ci: full` run every lane
# anyway, so a pattern here only decides what a pull request may skip.
set -euo pipefail

lanes=(rust wasm solana evm eval ctl ui tui images iso install discovery)

# CI's own wiring and the toolchain every lane builds with: any change here
# runs everything.
full_patterns=(
  .github/workflows/ci.yml
  ".github/actions/*"
  .github/scripts/ci-changes.sh
  tools/losos-ghcr-cache.py
  flake.lock
  devenv.nix
  devenv.yaml
  devenv.lock
  nix/fast-build.nix
)

# The flake and the package set: every lane that builds with nix.
pkgs=(flake.nix nix/packages.nix nix/bun-deps.nix)

# What admin-ui/tui/build.rs reads out of the web UI.
tui_reads=(
  "admin-ui/app/src/i18n/*"
  admin-ui/app/src/styles/tokens.css
  admin-ui/app/src/screens/settings/panes.ts
  admin-ui/themes/brand/salmon-128.png
)

# clippy, rustfmt and the suites of the eleven crates.
lane_rust=(
  "backend/*"
  "edge/registrar/*"
  "edge/vercel/*"
  "edge/contracts/solana/*"
  "admin-ui/tui/*"
  "admin-ui/lab/*"
  # backend/src/overrides.rs include_str!s it.
  modules/overrides.nix
  # backend/plugins/basic's tests keep it in step with the pages' sources.
  admin-ui/app/src/styles/basic-classes.txt
  "${tui_reads[@]}"
)

# clippy for wasm32: lososd's plugins, losos-mdx and the Diagrams canvas.
lane_wasm=("backend/plugins/*" "backend/mdx/*" "admin-ui/lab/*")

# The escrow programs and the registrar that pays through them.
lane_solana=("edge/contracts/solana/*" "edge/registrar/*" "backend/access/*")
lane_evm=("edge/contracts/*" "!edge/contracts/solana/*" "edge/registrar/*" "backend/access/*")

# The flake evaluation. It is the one lane kept by exclusion: Nix reads JSON,
# lock files and the odd source file all over the tree, so every change runs
# it except the kinds of file no evaluation reads.
lane_eval=(
  "*"
  "!*.md" "!*.mdx" "!*.rs"
  "!*.png" "!*.jpg" "!*.jpeg" "!*.gif" "!*.webp" "!*.svg" "!*.mp4" "!*.webm"
  "!docs/*"
  "!handbook/docs/*" "!handbook/i18n/*"
  "!.github/*"
  "!.claude/*"
  "!LICENSES/*" "!REUSE.toml"
  "!edge/vercel/*" "!edge/contracts/solana/*"
  # Rust sources Nix reads (nix/tests/git-issues.nix).
  backend/src/cloud/boards.rs
)

# lososd and losos-ctl, the backend crate's binaries. losos-ctl builds the
# report crate in by path (src/install_report.rs).
lane_ctl=("backend/*" "!backend/plugins/*" "backend/plugins/report/*" modules/overrides.nix "${pkgs[@]}")

# losos-registrar, the admin UI with Diagrams, the handbook, and the
# Playwright suite against the built SPA (nix/tests/admin-ui.nix, which reads
# the options document).
lane_ui=(
  "admin-ui/*" "!admin-ui/design-system/*" "!admin-ui/tui/*"
  "handbook/*"
  "edge/registrar/*" "backend/access/*"
  # LosOS Office and Notes parse documents with losos-mdx in the page.
  "backend/mdx/*"
  modules/options.nix nix/options-doc.nix
  nix/tests/admin-ui.nix nix/tests/fonts.nix
  "${pkgs[@]}"
)

# losos, the terminal client, as the static binary curl | sh installs.
lane_tui=("admin-ui/tui/*" "backend/access/*" "${tui_reads[@]}" "${pkgs[@]}")

# The OCI image of the local cluster.
lane_images=(nix/images.nix "${pkgs[@]}")

# The installer the ISO boots into.
installer=(
  "backend/src/installer*" backend/src/install_screen.rs
  backend/src/install_report.rs "backend/plugins/report/*"
  backend/src/bin/losos-ctl.rs backend/Cargo.toml backend/Cargo.lock
  modules/options.nix modules/disko.nix modules/installer.nix
)

# The ISO: the modules its system imports (flake.nix, nixosConfigurations.iso),
# their files, the installer, the signing tool and the boot legs.
lane_iso=(
  "${installer[@]}"
  modules/cache.nix modules/secure-boot.nix modules/branding.nix
  modules/live-branding.nix modules/splash.nix modules/brand-art.nix
  "modules/splash/*"
  "admin-ui/themes/brand/*"
  nix/sign-iso.sh nix/version.nix "nix/keys/*"
  nix/tests/iso-boot.py
  "${pkgs[@]}"
)

# nix/tests/iso-install.nix: the installer from the medium's system onto an
# empty disk, then that disk booted on its own. The installer, the layout it
# formats, the boot path and unlock, and the test.
lane_install=(
  "${installer[@]}"
  modules/cache.nix modules/branding.nix modules/splash.nix
  modules/boot.nix modules/impermanence.nix modules/configuration.nix
  modules/credentials.nix modules/console.nix modules/defaults.nix
  nix/tests/iso-install.nix
  "${pkgs[@]}"
)

# nix/tests/discovery: the box it boots (box.nix imports these modules) and
# the parts of lososd and LosOS Cloud the claim, SMB and WebDAV go through.
lane_discovery=(
  "nix/tests/discovery/*"
  modules/options.nix modules/mdns.nix modules/containers.nix modules/tls.nix
  modules/daemon.nix modules/identity.nix modules/cloud.nix modules/windows.nix
  modules/config-repo.nix
  "backend/src/cloud/*" backend/src/windows.rs backend/src/people.rs
  backend/src/setup.rs backend/src/signin.rs backend/src/login.rs
  backend/src/http.rs backend/src/guard.rs backend/src/sections.rs
  backend/sections.json backend/src/lib.rs
  backend/src/bin/lososd.rs backend/src/bin/losos-cloud.rs
  backend/Cargo.toml backend/Cargo.lock
  "${pkgs[@]}"
)

# matches <file> <pattern>...: the .gitignore rule, last match wins.
matches() {
  local file=$1 hit=false pat
  shift
  for pat in "$@"; do
    if [[ $pat == '!'* ]]; then
      # shellcheck disable=SC2053 # the pattern is a glob on purpose
      [[ $file == ${pat:1} ]] && hit=false
    else
      # shellcheck disable=SC2053
      [[ $file == $pat ]] && hit=true
    fi
  done
  [ "$hit" = true ]
}

declare -A on=() why=()
full=false
reason=""

if [ "${1:-}" = --full ]; then
  full=true
  reason=${2:-"asked for"}
else
  files=()
  while IFS= read -r f; do
    [ -n "$f" ] && files+=("$f")
  done
  if [ ${#files[@]} -eq 0 ]; then
    # GitHub lists no files for an empty diff; there is nothing to skip on
    # the strength of that.
    full=true
    reason="no changed files listed"
  fi
  for f in "${files[@]}"; do
    if matches "$f" "${full_patterns[@]}"; then
      full=true
      reason="$f changes CI or the toolchain"
      break
    fi
    for lane in "${lanes[@]}"; do
      [ -n "${on[$lane]:-}" ] && continue
      declare -n pats="lane_$lane"
      if matches "$f" "${pats[@]}"; then
        on[$lane]=true
        why[$lane]=$f
      fi
      unset -n pats
    done
  done
fi

echo "full=$full"
echo "reason=$reason"
for lane in "${lanes[@]}"; do
  if [ "$full" = true ] || [ -n "${on[$lane]:-}" ]; then
    echo "$lane=true"
  else
    echo "$lane=false"
  fi
done

if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  {
    echo "## Lanes for this change"
    echo
    if [ "$full" = true ]; then
      echo "Every lane runs: $reason."
    else
      echo "| Lane | Runs | Because of |"
      echo "|---|---|---|"
      for lane in "${lanes[@]}"; do
        if [ -n "${on[$lane]:-}" ]; then
          echo "| $lane | yes | \`${why[$lane]}\` |"
        else
          echo "| $lane | no | |"
        fi
      done
      echo
      echo "Label the pull request \`ci: full\` and re-run the workflow to run every lane."
    fi
  } >> "$GITHUB_STEP_SUMMARY"
fi
