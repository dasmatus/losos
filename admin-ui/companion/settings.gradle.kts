// LosOS Companion — the box in a phone's pocket.
//
// An Android client (Jetpack Compose) for the LosOS appliance: it finds the
// box on the LAN, links to it with a Settings → Access tokens bearer token,
// and mirrors the admin pages' dashboard in a mobile shape. It speaks the
// same design tokens as the admin SPA (`admin-ui/app/src/styles/tokens.css`),
// so owner should not be able to tell where one ends and the other begins.
//
// Build with a Gradle 8.9+ and JDK 17:
//   gradle :app:assembleDebug
// then `adb install app/build/outputs/apk/debug/app-debug.apk`, or open the
// folder in Android Studio.

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "losos-companion"
include(":app")
