#!/usr/bin/env bash
# Automate/document Dioxus mobile bundling. Requires platform SDKs.
# Android: ANDROID_HOME/ANDROID_SDK_ROOT, NDK, Java; `cargo install dioxus-cli` or `dx` on PATH.
# iOS: macOS + Xcode command line tools; `dx` on PATH.
set -euo pipefail
cd "$(dirname "$0")"
platform="${1:-}"
case "$platform" in
  android)
    : "${ANDROID_HOME:=${ANDROID_SDK_ROOT:-}}"
    if [ -z "${ANDROID_HOME}" ]; then
      echo "Android bundling requires ANDROID_HOME/ANDROID_SDK_ROOT (and NDK). Set SDK env vars." >&2
      echo "Example: dx bundle --platform android --release" >&2
      exit 2
    fi
    command -v dx >/dev/null || { echo "dx (dioxus-cli) not found. Install: cargo install dioxus-cli" >&2; exit 2; }
    dx bundle --platform android --release
    ;;
  ios)
    if [ "$(uname -s)" != "Darwin" ]; then
      echo "iOS bundling requires macOS/Xcode. Run on macOS with: dx bundle --platform ios --release" >&2
      exit 2
    fi
    command -v dx >/dev/null || { echo "dx (dioxus-cli) not found. Install: cargo install dioxus-cli" >&2; exit 2; }
    dx bundle --platform ios --release
    ;;
  *)
    echo "Usage: $0 android|ios" >&2
    echo "Prerequisites documented in README.md and build-all.sh." >&2
    exit 2
    ;;
esac
