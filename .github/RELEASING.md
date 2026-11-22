# Publishing LosOS releases

## One-time repository setting

Enable GitHub release immutability before publishing the next version:

1. Open [repository Settings](https://github.com/dasmatus/losos/settings).
2. Find the **Releases** section.
3. Enable **Release immutability**.

GitHub applies this setting to future releases only. Releases already published before the setting was enabled remain mutable unless they are individually republished. The CI action checks the result when it publishes a draft and fails rather than leaving a successful-looking mutable release.

## Publish a new version from CI

1. Open **Actions → Release → Run workflow** on the `main` branch.
2. Enter the version, such as `0.7.0` or `0.7.0-rc.1`. Do not reuse a version; the workflow adds the leading `v`.
3. Choose **stable** or **prerelease**, then run the workflow.

The workflow creates an annotated tag at the current `main` commit, explicitly dispatches the normal CI on that tag (GitHub does not trigger other workflows from a tag push made with `GITHUB_TOKEN`), and asks the version workflow to sync `nix/version.nix` on `main`. CI builds the release media, uploads all GitHub release assets to a draft, and publishes it only after the assets are complete. With release immutability enabled, GitHub locks the published assets and tag.

Publication waits for the installer ISO, `losos-ctl`, the registrar and admin UI builds, and the rust gates (`test`, `clippy`, `clippy-wasm`). A failed phone build, Playwright run, discovery leg, eval lint, install VM, or the GitHub Pages copy of the binary cache does not skip publication. Those stay visible on the run; they are not the release.

If the release build fails before publication, the release remains a draft so CI can retry its asset uploads. Re-dispatch **CI** on the existing tag (Actions → CI → Run workflow, ref = the tag, release channel as you chose). Do not run **Release** again for the same version: the tag is write-once, and that workflow refuses to move it. Once published as immutable, use a new version for fixes instead of re-running CI against that release.

The nightly channel cut (`channel/daily`, `channel/weekly`, `channel/monthly`) uses the same idea. It moves to the newest commit of `main` whose `result` check passed and whose `nix: publish built paths to GHCR` job succeeded. A red workflow whose only failures are advisory does not hold the cut. A failed rust gate, or a cache upload that did not happen, does.

## Existing releases

The repository setting does not retroactively lock historical releases. GitHub would require each old release to be republished to gain native immutability; this workflow deliberately does not delete or replace historical releases automatically.
