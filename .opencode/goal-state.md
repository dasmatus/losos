# Goal State

## Goal
Start implementing the LosOS iOS companion app (native SwiftUI, approach A, Apple HIG) and add GitHub Actions macOS-runner CI that builds it.

**Measurable outcome:** `admin-ui/companion-ios/` contains an initial SwiftUI app scaffold mapped from `admin-ui/companion/` (Android) with core contracts (API routes/Bearer+401 handling, `_losos._tcp` discovery, Keychain/TOFU placeholders, HIG-oriented structure, README/AltStore notes); `.github/workflows/companion-ios.yml` runs on `macos-*` and invokes `xcodebuild` for `admin-ui/companion-ios`; milestones recorded with evidence.

## Milestones
| # | Milestone | Verification | Status |
|---|-----------|--------------|--------|
| 1 | Record goal/milestones; inspect CI/macOS patterns | `grep 'macos-15-intel' .github/workflows/ci.yml` found existing macOS runner usage; state file created with goal+milestones | verified |
| 2 | Create `admin-ui/companion-ios/` scaffold (SwiftUI, Android-parity contracts, HIG/AltStore README) | `find admin-ui/companion-ios ...` => 13 key files; `grep` confirms Bearer+401, `_losos._tcp`, KeychainStore, BoxRoutes (health/status/storage/apps/settings/edge/options/config/sync/backup/run/cloud), TOFU/AltStore notes, status table; theme aligned to canonical `tokens.css` (ok/warn/crit, radii 9/6) | verified |
| 3 | Add GitHub Actions workflow on `macos-*` to build the iOS scaffold | `.github/workflows/companion-ios.yml`: `runs-on: macos-15`, `working-directory: admin-ui/companion-ios`, `xcodegen generate`, `xcodebuild ... -destination 'generic/platform=iOS' ... CODE_SIGNING_ALLOWED=NO build` (static verification in Linux; build executes on GitHub macOS runners) | verified |
| 4 | Finalize state with evidence/limits and complete goal | State file updated with evidence per milestone and shipped/not-done | verified |

## Key decisions
- Approach A: native SwiftUI, Apple HIG (owner direction).
- **Design system directive (owner):** UI must be up to the LosOS design system and reuse existing components/patterns. Before adding UI, check `admin-ui/design-system/react` (`chrome/display/feedback/forms`) and `admin-ui/app/src/styles/tokens.css`; align spacing, typography, colors, states, behavior. SwiftUI cannot reuse React components, so port canonical tokens and mirror DS patterns natively.
- Android-parity baseline: anonymous health/probe, Bearer token for `/api/*`, 401 → clear token/connect, `_losos._tcp` Bonjour, Keychain token storage, TOFU cert pinning placeholder.
- XcodeGen (`project.yml`) generates the Xcode project on CI/macOS; no committed `.xcodeproj`. CI uses `macos-15`, unsigned build.

## Blockers
- None for this goal. Owner decisions remain: final v1 actions/scope, min iOS confirmation, TOFU cert UX, AltStore source hosting, Apple signing/provisioning/IPA export.

## Completion
**Status: completed (2026-11-05).** Follow-up: applied design-system alignment directive (README + theme corrected to canonical `tokens.css` values/radii).

**What shipped:**
- `admin-ui/companion-ios/` scaffold (SwiftUI app, core API/discovery/session/keychain, token theme, connect/panes/settings, README, AltStore source template, XcodeGen spec).
- `.github/workflows/companion-ios.yml` (macOS-15, XcodeGen + `xcodebuild`, no signing).

**Not done / not verifiable here:**
- Real `xcodebuild`/runtime verification (Linux); CI covers macOS build.
- Full data binding, TOFU pinning impl, complete pane parity/polish/accessibility/tests.
- Signed IPA, AltStore hosting/URLs/checksums, Apple signing (owner).
- Not committed/pushed from this environment: worktree `.git` points to a missing gitdir (`/home/matus/Dokumente/github/losos/.git/worktrees/t3-3c8fba2c`), so `git status/commit/push` cannot run here. Changes are present in `/home/matus/.t3/worktrees/losos/t3-3c8fba2c` and need commit/push/PR from a valid git worktree/checkout.
