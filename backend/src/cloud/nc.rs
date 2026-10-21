//! The parts of Nextcloud's own API the desktop and phone apps need before
//! they get to WebDAV: discovery, sign-in and the account.
//!
//!   * `GET /status.php` says a server is here and which version it speaks;
//!   * `GET /ocs/v{1,2}.php/cloud/capabilities` says what it can do, and the
//!     apps turn their features on and off by it;
//!   * Login Flow v2: `POST /index.php/login/v2` starts a sign-in, the app
//!     opens the browser at the address it gets back, the person allows it
//!     on the web app's connect page, and the app's poll of
//!     `POST /index.php/login/v2/poll` hands over an app password once;
//!   * `GET /ocs/v{1,2}.php/cloud/user` names the account and its quota;
//!   * `GET /ocs/v2.php/core/getapppassword` swaps a real password for an
//!     app password (`losos cloud setup` uses it), and `DELETE
//!     /ocs/v2.php/core/apppassword` is an app signing itself out.
//!
//! OCS answers are XML unless the client asks for JSON with `format=json`
//! or an `Accept` header; v1 always answers 200 with the outcome in
//! `statuscode`, v2 answers with the real HTTP status.

use crate::cloud::auth::{Via, Who};
use crate::cloud::util::xml_escape;
use crate::cloud::Cloud;
use actix_web::http::StatusCode;
use actix_web::{HttpRequest, HttpResponse};
use serde_json::{json, Value};

/// The Nextcloud version the server reports. The apps warn about servers
/// they think too old, so it is a current one; the box speaks the client
/// API of this release.
pub const NC_VERSION: [u32; 4] = [34, 0, 4, 1];
pub const NC_VERSION_STRING: &str = "34.0.4";

/// The box's accent, from admin-ui/app/src/styles/tokens.css.
pub const ACCENT: &str = "#0e6e7d";

pub fn status(cloud: &Cloud) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Access-Control-Allow-Origin", "*"))
        .json(json!({
            "installed": true,
            "maintenance": false,
            "needsDbUpgrade": false,
            "version": NC_VERSION.iter().map(u32::to_string).collect::<Vec<_>>().join("."),
            "versionstring": NC_VERSION_STRING,
            "edition": "",
            "productname": cloud.config.product,
            "extendedSupport": false,
        }))
}

pub fn capabilities(cloud: &Cloud) -> Value {
    json!({
        "version": {
            "major": NC_VERSION[0],
            "minor": NC_VERSION[1],
            "micro": NC_VERSION[2],
            "string": NC_VERSION_STRING,
            "edition": "",
            "extendedSupport": false,
        },
        "capabilities": {
            "core": {
                "pollinterval": 60,
                "webdav-root": "remote.php/webdav",
                "reference-api": false,
                "mod-rewrite-working": true,
            },
            "bruteforce": { "delay": 0, "allow-listed": false },
            "files": {
                "bigfilechunking": true,
                "blacklisted_files": [".htaccess"],
                "forbidden_filename_characters": ["/", "\\"],
                "undelete": true,
                "versioning": true,
                "version_labeling": false,
                "version_deletion": false,
                "favorites": true,
            },
            "dav": {
                "chunking": "1.0",
                "bulkupload": "",
            },
            "checksums": {
                "supportedTypes": ["SHA1"],
                "preferredUploadType": "SHA1",
            },
            "files_sharing": crate::cloud::share::capabilities(cloud),
            "theming": {
                "name": cloud.config.product,
                "url": "",
                "slogan": "",
                "color": ACCENT,
                "color-text": "#ffffff",
                "color-element": ACCENT,
                "color-element-bright": ACCENT,
                "color-element-dark": ACCENT,
                "logo": "",
                "background": "",
                "background-plain": true,
                "background-default": true,
                "logoheader": "",
                "favicon": "",
            },
            "user": { "expire_date": { "enabled": false } },
        }
    })
}

/// Wants JSON rather than OCS's default XML.
fn wants_json(req: &HttpRequest) -> bool {
    req.query_string()
        .split('&')
        .any(|p| p == "format=json")
        || req
            .headers()
            .get("accept")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|a| a.contains("application/json"))
}

fn is_v2(req: &HttpRequest) -> bool {
    req.path().starts_with("/ocs/v2.php")
}

/// An OCS answer: `data` on success, or a failure with `status` and a message.
pub fn ocs(req: &HttpRequest, status: StatusCode, message: &str, data: Value) -> HttpResponse {
    let v2 = is_v2(req);
    let ok = status.is_success();
    let code = if v2 {
        status.as_u16()
    } else if ok {
        100
    } else {
        // OCS v1's own codes: 997 unauthorised, 998 not found, 996 error.
        match status.as_u16() {
            401 => 997,
            404 => 998,
            403 => 403,
            400 => 400,
            _ => 996,
        }
    };
    let meta = json!({
        "status": if ok { "ok" } else { "failure" },
        "statuscode": code,
        "message": if message.is_empty() { if ok { "OK" } else { "" } } else { message },
    });
    let http = if v2 { status } else { StatusCode::OK };
    if wants_json(req) {
        HttpResponse::build(http).json(json!({ "ocs": { "meta": meta, "data": data } }))
    } else {
        let mut out = String::from(r#"<?xml version="1.0"?>"#);
        out.push_str("\n<ocs>");
        to_xml("meta", &meta, &mut out);
        to_xml("data", &data, &mut out);
        out.push_str("</ocs>\n");
        HttpResponse::build(http)
            .content_type("application/xml; charset=utf-8")
            .body(out)
    }
}

/// OCS's XML form of a JSON value: objects become elements, list items
/// `<element>`.
fn to_xml(name: &str, v: &Value, out: &mut String) {
    let tag = if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
    {
        name
    } else {
        "element"
    };
    match v {
        Value::Null => {
            out.push_str(&format!("<{tag}/>"));
        }
        Value::Object(map) => {
            out.push_str(&format!("<{tag}>"));
            for (k, v) in map {
                to_xml(k, v, out);
            }
            out.push_str(&format!("</{tag}>"));
        }
        Value::Array(items) => {
            out.push_str(&format!("<{tag}>"));
            for v in items {
                to_xml("element", v, out);
            }
            out.push_str(&format!("</{tag}>"));
        }
        Value::Bool(b) => out.push_str(&format!("<{tag}>{}</{tag}>", if *b { 1 } else { 0 })),
        Value::Number(n) => out.push_str(&format!("<{tag}>{n}</{tag}>")),
        Value::String(s) => out.push_str(&format!("<{tag}>{}</{tag}>", xml_escape(s))),
    }
}

pub fn user(req: &HttpRequest, cloud: &Cloud, who: &Who) -> HttpResponse {
    let used = cloud.store.used(&who.login).unwrap_or(0);
    let free = cloud.store.free();
    let total = used.saturating_add(free);
    let relative = if total > 0 {
        (used as f64 / total as f64 * 10000.0).round() / 100.0
    } else {
        0.0
    };
    ocs(
        req,
        StatusCode::OK,
        "",
        json!({
            "enabled": true,
            "storageLocation": "",
            "id": who.login,
            "lastLogin": crate::cloud::util::now() * 1000,
            "backend": "LosOS",
            "subadmin": [],
            "quota": {
                "free": free,
                "used": used,
                "total": total,
                "relative": relative,
                "quota": -3,
            },
            "email": if who.email.is_empty() { Value::Null } else { Value::String(who.email.clone()) },
            "additional_mail": [],
            "displayname": who.name,
            "display-name": who.name,
            "phone": "",
            "address": "",
            "website": "",
            "twitter": "",
            "groups": [],
            "language": "en",
            "locale": "",
            "backendCapabilities": { "setDisplayName": false, "setPassword": false },
        }),
    )
}

/// A name for the Devices list from what the app calls itself:
/// "Mozilla/5.0 (Linux) mirall/3.16.0 (Nextcloud, nixos)" is the desktop
/// client on Linux, "Nextcloud-android/3.30.0" the Android app.
pub fn device_name(agent: &str) -> String {
    let lower = agent.to_ascii_lowercase();
    let os = if lower.contains("windows") {
        Some("Windows")
    } else if lower.contains("macintosh") || lower.contains("mac os") || lower.contains("macos") {
        Some("macOS")
    } else if lower.contains("linux") {
        Some("Linux")
    } else {
        None
    };
    if lower.contains("nextcloud-android") {
        return "Nextcloud on Android".to_owned();
    }
    if lower.contains("nextcloud-ios") || lower.contains("iphone") || lower.contains("ipad") {
        return "Nextcloud on iPhone or iPad".to_owned();
    }
    if lower.contains("mirall") || lower.contains("nextcloud-desktop") || lower.contains("nextcloudcmd") {
        return match os {
            Some(os) => format!("Nextcloud desktop on {os}"),
            None => "Nextcloud desktop".to_owned(),
        };
    }
    let trimmed: String = agent.chars().filter(|c| !c.is_control()).take(80).collect();
    if trimmed.trim().is_empty() {
        "A device".to_owned()
    } else {
        trimmed
    }
}

/// `getapppassword`: a real password in, an app password out. Refused for
/// a request that already uses an app password, as Nextcloud does.
pub fn get_app_password(req: &HttpRequest, cloud: &Cloud, who: &Who) -> HttpResponse {
    if matches!(who.via, Via::AppPassword(_)) {
        return ocs(req, StatusCode::FORBIDDEN, "already an app password", Value::Null);
    }
    let client = device_name(
        req.headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
    );
    match cloud.auth.new_app_password(&cloud.store, &who.login, &client) {
        Ok((_, pw)) => ocs(req, StatusCode::OK, "", json!({ "apppassword": pw })),
        Err(_) => ocs(req, StatusCode::INTERNAL_SERVER_ERROR, "", Value::Null),
    }
}

/// An app signing out: its own app password goes.
pub fn delete_app_password(req: &HttpRequest, cloud: &Cloud, who: &Who) -> HttpResponse {
    match who.via {
        Via::AppPassword(id) => {
            let _ = cloud.auth.revoke(&cloud.store, &who.login, id);
            ocs(req, StatusCode::OK, "", json!([]))
        }
        Via::Header => ocs(req, StatusCode::FORBIDDEN, "no app password", Value::Null),
    }
}

/// The origin the client reached the box on, for the addresses Login Flow
/// v2 hands back. nginx passes the scheme it saw (behind the edge, the
/// browser's) in `X-Forwarded-Proto`.
pub fn origin(req: &HttpRequest) -> String {
    let scheme = req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .filter(|s| *s == "http" || *s == "https")
        .unwrap_or("https");
    let host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .filter(|h| {
            h.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
        })
        .unwrap_or("localhost");
    format!("{scheme}://{host}")
}

pub fn flow_start(req: &HttpRequest, cloud: &Cloud) -> HttpResponse {
    let client = device_name(
        req.headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
    );
    match cloud.auth.start_flow(&cloud.store, &client) {
        Ok((poll, login)) => {
            let o = origin(req);
            HttpResponse::Ok().json(json!({
                "poll": { "token": poll, "endpoint": format!("{o}/index.php/login/v2/poll") },
                "login": format!("{o}/index.php/login/v2/flow/{login}"),
            }))
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

/// The poll, `token=<poll token>` in a form body.
pub fn flow_poll(req: &HttpRequest, cloud: &Cloud, body: &[u8]) -> HttpResponse {
    let form = String::from_utf8_lossy(body);
    let token = form
        .split('&')
        .find_map(|p| p.strip_prefix("token="))
        .map(|t| {
            percent_encoding::percent_decode_str(t)
                .decode_utf8_lossy()
                .into_owned()
        })
        .unwrap_or_default();
    match cloud.auth.poll_flow(&cloud.store, &token) {
        Ok(Some((login, pw))) => HttpResponse::Ok().json(json!({
            "server": origin(req),
            "loginName": login,
            "appPassword": pw,
        })),
        Ok(None) => HttpResponse::NotFound().json(json!([])),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_get_a_readable_name() {
        assert_eq!(
            device_name("Mozilla/5.0 (Linux) mirall/3.16.0 (Nextcloud, nixos)"),
            "Nextcloud desktop on Linux"
        );
        assert_eq!(
            device_name("Mozilla/5.0 (Windows) mirall/3.16.0 (Nextcloud)"),
            "Nextcloud desktop on Windows"
        );
        assert_eq!(device_name("Mozilla/5.0 (Android) Nextcloud-android/3.30.0"), "Nextcloud on Android");
        assert_eq!(device_name(""), "A device");
        assert_eq!(device_name("curl/8.0"), "curl/8.0");
    }

    #[test]
    fn ocs_xml_turns_lists_into_elements() {
        let mut out = String::new();
        to_xml("data", &json!({"a": [1, 2], "b": true, "c": "x<y"}), &mut out);
        assert_eq!(
            out,
            "<data><a><element>1</element><element>2</element></a><b>1</b><c>x&lt;y</c></data>"
        );
    }
}
