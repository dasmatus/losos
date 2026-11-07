//! Canonical endpoint builders/normalizers and contract notes for the Dioxus
//! rewrite middle layer.
//!
//! # Ownership / scope
//! This module owns path/shape contract alignment for: **apps (installed +
//! catalog), market, storage, backup, users/people**.
//!
//! # Endpoint matrix (Dioxus middle-layer vs TS contract vs prior drift)
//!
//! | Domain | Operation | Canonical path (TS: `admin-ui/app/src/lib/api.ts`) | Prior Dioxus drift (fixed/aligned) |
//! |---|---|---|---|
//! | Apps (installed) | list installed/catalog response | `GET /api/apps` (`getApps`) | `actions::Action::AppsList` called `GET /api/market` (market shelf); routes/panes already used `/api/apps` |
//! | Apps (catalog/chart) | search charts | `GET /api/apps/search?q=<term>` (used by panes; TS helper not named but contract is this path shape) | none in panes; builder provided here |
//! | Apps (catalog/chart) | chart details/defaults | `GET /api/apps/chart?repo=&name=&version=` (`getAppChart`) | panes built inline |
//! | Apps (install) | install/change chart release | `POST /api/apps/install` (`postAppInstall`) body `AppInstallRequest` | panes used correct path; actions had no chart-install action |
//! | Apps (install) | remove release | `POST /api/apps/remove` (`postAppRemove`) body `{"release"}` | panes used correct path |
//! | Market (shelf) | shelf + account | `GET /api/market` (`getMarket`) | conflated with installed apps in `AppsList` action |
//! | Market (legacy) | legacy install-by-id action | **no canonical TS endpoint**; market buy is `POST /api/market/orders` (`listing_id`,`quantity`) | `Action::MarketInstall(id)` posted `POST /api/market/install` (non-canonical) — preserved as explicit legacy constant |
//! | Storage | status | `GET /api/storage` (`getStorage`) | `Action::StorageStatus` called `GET /api/storage/status` |
//! | Storage | grow/claim reserve | `POST /api/grow` (`postGrow`) | already correct |
//! | Backup | status/view | `GET /api/backup` (`getBackup`) | `Action::BackupStatus` called `GET /api/backup/status` |
//! | Backup | run now | `POST /api/backup/run` (`postBackupRun`) | already correct in panes |
//! | Users/People | list people | `GET /api/people` (`getPeople`; `use-people.ts` hook) | `Action::UsersList` called `GET /api/users` |
//! | Users/People | mutations | `POST /api/people*` (person/access/password/remove/rules/...) | not modeled in Dioxus actions; escape hatch remains |
//!
//! # Shape notes (summary)
//! - `GET /api/storage`: `StorageResponse { totalBytes, usedBytes, reserveBytes }`
//!   (nullable). Panes normalize `used/total` GB fallbacks for compatibility.
//! - `GET /api/backup`: `BackupResponse { target, last, job, erase, lastErase,
//!   graceSeconds }`.
//! - `GET /api/people`: `PeopleResponse { standalone, you, sections, rules?,
//!   roles?, people[] }`. The TS hook normalizes partial/missing arrays.
//! - `GET /api/apps`: `AppsResponse = { available:false, reason:"off", apps:[] }
//!   | { available:true, sharedAvailable, kindasharedAvailable?, apps[] }`.
//! - `GET /api/market`: `MarketResponse = { available:false, reason? } | {
//!   available:true, listings[], account }` — **not** the installed-apps list.
//!
//! # Normalization policy
//! Builders produce canonical paths. Query builders percent-encode values via
//! `losos_client::encode_query` (same helper used by panes). Callers should
//! prefer these helpers over inline string formatting to avoid drift.

use losos_client::encode_query;

// ── Storage ───────────────────────────────────────────────────────────────

pub const STORAGE_STATUS_PATH: &str = "/api/storage";
pub const STORAGE_GROW_PATH: &str = "/api/grow";

/// Canonical storage status endpoint (`GET /api/storage`).
pub fn storage_status_path() -> &'static str {
    STORAGE_STATUS_PATH
}

/// Canonical grow endpoint (`POST /api/grow`).
pub fn storage_grow_path() -> &'static str {
    STORAGE_GROW_PATH
}

// ── Backup ────────────────────────────────────────────────────────────────

pub const BACKUP_STATUS_PATH: &str = "/api/backup";
pub const BACKUP_RUN_PATH: &str = "/api/backup/run";

/// Canonical backup view endpoint (`GET /api/backup`).
pub fn backup_status_path() -> &'static str {
    BACKUP_STATUS_PATH
}

/// Canonical run-backup endpoint (`POST /api/backup/run`).
pub fn backup_run_path() -> &'static str {
    BACKUP_RUN_PATH
}

// ── Users / People ────────────────────────────────────────────────────────

pub const USERS_LIST_PATH: &str = "/api/people";

/// Canonical people list endpoint (`GET /api/people`).
/// Contract name in TS is `getPeople`; older Dioxus action label `UsersList`
/// maps here.
pub fn users_list_path() -> &'static str {
    USERS_LIST_PATH
}

// ── Apps (installed + catalog/chart install flow) ─────────────────────────

pub const APPS_INSTALLED_PATH: &str = "/api/apps";
pub const APPS_SEARCH_BASE: &str = "/api/apps/search";
pub const APPS_CHART_BASE: &str = "/api/apps/chart";
pub const APPS_INSTALL_PATH: &str = "/api/apps/install";
pub const APPS_REMOVE_PATH: &str = "/api/apps/remove";

/// Installed apps endpoint (`GET /api/apps`).
pub fn apps_installed_path() -> &'static str {
    APPS_INSTALLED_PATH
}

/// Chart search (`GET /api/apps/search?q=<term>`). Empty/whitespace terms are
/// encoded as-is; callers may choose to reject them.
pub fn apps_search_path(term: &str) -> String {
    format!("{APPS_SEARCH_BASE}?q={}", encode_query(term))
}

/// Chart details/defaults (`GET /api/apps/chart?repo=&name=&version=`).
pub fn apps_chart_path(repo: &str, name: &str, version: &str) -> String {
    format!(
        "{APPS_CHART_BASE}?repo={}&name={}&version={}",
        encode_query(repo),
        encode_query(name),
        encode_query(version)
    )
}

/// Install/change a chart release (`POST /api/apps/install`).
pub fn apps_install_path() -> &'static str {
    APPS_INSTALL_PATH
}

/// Remove an installed release (`POST /api/apps/remove`).
pub fn apps_remove_path() -> &'static str {
    APPS_REMOVE_PATH
}

// ── Market (shelf / marketplace operations) ───────────────────────────────

pub const MARKET_PATH: &str = "/api/market";

/// Market shelf/account endpoint (`GET /api/market`). Distinct from installed
/// apps (`/api/apps`).
pub fn market_path() -> &'static str {
    MARKET_PATH
}

/// Legacy/non-canonical path previously used by `Action::MarketInstall(id)`.
/// Kept only to preserve wire behavior for that specific legacy action while
/// making drift explicit. Canonical marketplace purchase is
/// `POST /api/market/orders` with `listing_id` + `quantity`; canonical app
/// install is `POST /api/apps/install` with `AppInstallRequest`.
pub const MARKET_INSTALL_LEGACY_PATH: &str = "/api/market/install";

/// Canonical market order (buy) endpoint shape for reference/mapping.
pub const MARKET_ORDERS_PATH: &str = "/api/market/orders";

// ── Generic normalizer ────────────────────────────────────────────────────

/// Normalize a small set of known drifted paths to canonical endpoints.
/// Unknown paths are returned unchanged (escape hatches / forward compat).
pub fn normalize_api_path(path: &str) -> &str {
    match path {
        "/api/storage/status" => STORAGE_STATUS_PATH,
        "/api/backup/status" => BACKUP_STATUS_PATH,
        "/api/users" => USERS_LIST_PATH,
        // Common singular/plural/queryless aliases seen in rewrites; keep narrow.
        _ => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_paths_match_ts_contract() {
        assert_eq!(storage_status_path(), "/api/storage");
        assert_eq!(storage_grow_path(), "/api/grow");
        assert_eq!(backup_status_path(), "/api/backup");
        assert_eq!(backup_run_path(), "/api/backup/run");
        assert_eq!(users_list_path(), "/api/people");
        assert_eq!(apps_installed_path(), "/api/apps");
        assert_eq!(apps_install_path(), "/api/apps/install");
        assert_eq!(apps_remove_path(), "/api/apps/remove");
        assert_eq!(market_path(), "/api/market");
    }

    #[test]
    fn query_builders_encode() {
        let p = apps_search_path("next cloud");
        assert!(p.starts_with("/api/apps/search?q="));
        assert!(!p.contains(' '));
        let c = apps_chart_path("bitnami/nginx", "nginx", "1.2.3+meta");
        assert!(c.contains("repo=bitnami%2Fnginx") || c.contains("repo=bitnami/nginx"));
        assert!(c.contains("name=nginx"));
        assert!(c.contains("version="));
    }

    #[test]
    fn normalizes_known_drift() {
        assert_eq!(normalize_api_path("/api/storage/status"), "/api/storage");
        assert_eq!(normalize_api_path("/api/backup/status"), "/api/backup");
        assert_eq!(normalize_api_path("/api/users"), "/api/people");
        assert_eq!(normalize_api_path("/api/mesh/status"), "/api/mesh/status");
    }

    #[test]
    fn legacy_market_install_is_explicit() {
        assert_eq!(MARKET_INSTALL_LEGACY_PATH, "/api/market/install");
        assert_ne!(MARKET_INSTALL_LEGACY_PATH, MARKET_ORDERS_PATH);
    }
}
