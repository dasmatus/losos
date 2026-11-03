# LosOS Companion

An Android client for the LosOS box, in Jetpack Compose: it reaches the box
over the LAN (`<name>.local` or its IP), links with a token from the admin
pages' Settings → Access tokens, and mirrors the admin homepage in a mobile
shape — connection card, storage card, app tiles, widget board, plus quick
links into LosOS Cloud.

It shares the admin front ends' palette (`admin-ui/app/src/styles/tokens.css`),
ported to Compose in `app/src/main/java/us/dasmat/losos/companion/LosTheme.kt`.

## Build

Needs JDK 17 and Gradle 8.10+ (or Android Studio, opening this folder):

```sh
gradle :app:assembleDebug
adb install app/build/outputs/apk/debug/app-debug.apk
```

## Status

Early: the dashboard reads demo data, the probe verifies reachability, and the
theme, screens and navigation of the phone shape are in place. Next: wiring the
cards to the boxes' real endpoints, LosOS Login sign-in, and a release signing
config.
