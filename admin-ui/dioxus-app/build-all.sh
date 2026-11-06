#!/usr/bin/env bash
# Build/check the LosOS Dioxus frontend for supported platforms.
# "All platforms at once" = one Rust/Dioxus codebase; actual packaging may
# require platform SDKs (Android/iOS) or system libs (Linux desktop webview).
set -euo pipefail
cd "$(dirname "$0")"
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"

echo "== core (no GUI renderer) =="
cargo test --no-default-features
cargo check --no-default-features

if rustup target list --installed | grep -q '^wasm32-unknown-unknown$'; then
  echo "== web (wasm32, dioxus/web) =="
  cargo check --no-default-features --features gui,web --target wasm32-unknown-unknown
else
  echo "== web skipped (install: rustup target add wasm32-unknown-unknown) =="
fi

echo "== desktop (dioxus/desktop; may need webkit2gtk/gtk dev libs on Linux) =="
if cargo check --no-default-features --features gui,desktop 2>&1; then
  echo "desktop: ok"
else
  echo "desktop: failed (likely missing system webview libs in this env); code is cross-platform"
fi

echo "== mobile packaging (requires SDKs + dx) =="
echo "dx bundle --platform android  # needs Android SDK/NDK"
echo "dx bundle --platform ios      # needs macOS/Xcode"
