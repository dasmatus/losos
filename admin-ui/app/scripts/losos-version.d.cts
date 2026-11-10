/* The types scripts/losos-version.cjs exports (app.config.ts's import):
 * the same words backend/build.rs's stamp_version yields — the tag the
 * tree is, and the semver Expo's version field carries. */
export declare function tag(raw: string): string;
export declare function lososTag(): string;
export declare function lososSemver(): string;
