//! The office plugin: LosOS Office's conversions, for anyone signed in.
//!
//! - `POST /api/office/export/{format}`: `{ "source", "lang"?, "pictures":
//!   { name: base64 } }`, a document and the pictures it shows, answered
//!   with the file. `docx`, `odt`, `pdf`, `html` or `pptx`.
//! - `POST /api/office/import/{format}?folder=NAME`: a `docx` or `odt` file,
//!   answered with `{ "source", "pictures" }`, the pictures under NAME.
//! - `GET /api/office/formats`: `{ "exports", "imports" }`, or 503 on a box
//!   whose converter is off, so the page offers only what works.
//!
//! The work is losos-convert's (backend/mdx/src/bin/losos-convert.rs) on
//! the socket modules/office-convert.nix opens, which runs Pandoc in a
//! container with no network for each call. This plugin decides who may
//! ask and passes on only the formats it knows.

use losos_plugin::{config, http, Plugin, Request, Response};

/// What a document can be exported as, with the type the browser saves.
pub const EXPORTS: &[&str] = &["docx", "odt", "pdf", "html", "pptx"];

/// What can be opened as a document.
pub const IMPORTS: &[&str] = &["docx", "odt"];

/// A folder name for an imported file's pictures: plain letters, digits
/// and `-_.`, so it is one segment of a path and a Markdown link needs no
/// brackets.
#[must_use]
pub fn folder_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Where the call goes on the converter's socket, or the answer that
/// refuses it.
pub fn target(req: &Request) -> Result<String, Response> {
    if req.caller.person().is_none() && !req.caller.is_admin() {
        return Err(Response::error(403, "sign in to convert documents"));
    }
    let format = req.param("format").unwrap_or("");
    match req.route.as_str() {
        "/api/office/formats" => Ok("/formats".into()),
        "/api/office/export/{format}" if EXPORTS.contains(&format) => {
            Ok(format!("/export/{format}"))
        }
        "/api/office/import/{format}" if IMPORTS.contains(&format) => {
            let folder = req
                .query_param("folder")
                .unwrap_or_else(|| "pictures".into());
            if !folder_ok(&folder) {
                return Err(Response::error(
                    400,
                    "the pictures' folder takes letters, digits, - _ and .",
                ));
            }
            Ok(format!("/import/{format}?folder={folder}"))
        }
        _ => Err(Response::error(404, "LosOS Office cannot convert that")),
    }
}

pub struct OfficePlugin;

impl Plugin for OfficePlugin {
    fn handle(req: Request) -> Response {
        let path = match target(&req) {
            Ok(p) => p,
            Err(refused) => return refused,
        };
        let socket = config::get_or("socket", "/run/losos-convert.sock");
        let url = format!("unix:{socket}{path}");
        let answer = if path == "/formats" {
            http::Request::get(&url).timeout_ms(10_000).fetch(64 << 10)
        } else {
            http::Request::post(&url)
                .header(
                    "content-type",
                    req.header("content-type")
                        .unwrap_or("application/octet-stream"),
                )
                .body(req.body)
                .timeout_ms(230_000)
                .fetch(128 << 20)
        };
        match answer {
            Ok(a) => {
                let kind = a
                    .header("content-type")
                    .unwrap_or("application/octet-stream")
                    .to_string();
                Response::bytes(a.status, &kind, a.body)
            }
            Err(e) if e.is_not_found() => {
                Response::error(503, "LosOS Office's converter is not on this box")
            }
            Err(e) => {
                losos_plugin::log::error(&format!("the converter did not answer: {e}"));
                Response::error(503, "LosOS Office's converter did not answer")
            }
        }
    }
}

losos_plugin::export!(OfficePlugin);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_are_one_plain_segment() {
        assert!(folder_ok("Garden-day"));
        assert!(folder_ok("report_2026.files"));
        for bad in ["", ".x", "a/b", "../x", "a b", "a%2Fb", "a<b"] {
            assert!(!folder_ok(bad), "{bad}");
        }
    }
}
