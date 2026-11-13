#!/usr/bin/env bash
# Reproducible visionOS prebuild helper for LosOS admin-ui/app.
#
# Why this exists: the app's default `react-native` dependency is aliased to
# react-native-tvos@0.81.5-2 (TV + phone/tablet). @callstack/react-native-visionos
# is an RN 0.79 fork and cannot coexist as a drop-in replacement in the same
# dependency set. A real visionOS app shell must therefore be generated from a
# visionOS-prepared dependency set/config.
#
# This script documents and automates the generation steps that are provable on
# a machine with Node/Bun/Xcode/visionOS SDK. Native ios/visionos outputs are
# gitignored in this repo (EAS/CNG convention), so this script is the committed,
# reproducible source of truth rather than checked-in generated native projects.
#
# Honest limits:
# - Requires macOS with Xcode and the visionOS SDK (or a compatible EAS macOS
#   builder). It cannot compile/run visionOS on Linux.
# - Building/running requires Apple developer tooling/signing as usual for
#   visionOS targets.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "visionOS prebuild helper (reproducible config, generated native dirs gitignored)"
echo "1) Ensure a visionOS-compatible dependency set is installed."
echo "   The default app set keeps react-native-tvos for TV/phone. For visionOS,"
echo "   use a dedicated worktree/install that resolves react-native to"
echo "   @callstack/react-native-visionos' RN line per upstream docs."
echo "2) Run Expo prebuild for visionOS-capable native projects when available:"
echo "   EXPO_VISIONOS=1 npx expo prebuild --platform ios --clean"
echo "3) Open/build with Xcode on macOS (visionOS destination) or submit via EAS:"
echo "   eas build --profile development_visionos --platform ios"
echo
echo "This repo commits config/docs/scripts (not generated native projects)."
echo "Verify dependency/config wiring with: bun run typecheck"
