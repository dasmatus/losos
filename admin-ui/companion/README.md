# LosOS Companion (replaced)

The Kotlin/Jetpack Compose Android companion has been **removed** and replaced by the cross-platform Dioxus frontend:

- **New frontend:** [`../dioxus-app/`](../dioxus-app/) (Rust + Dioxus) targeting web, desktop, Android, and iOS from one codebase.
- **Kotlin sources removed:** this directory no longer contains the Android/Gradle/Kotlin implementation.

The Dioxus app ports the companion’s behavior (box linking by address/token, bearer-token `/api/*` access, pane-based admin UI) and reuses patterns/shared crates from the Rust TUI (`../tui/`) and `backend/access` (`losos-access`). See `../dioxus-app/README.md` for build targets and status.
