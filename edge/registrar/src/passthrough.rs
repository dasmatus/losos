//! TLS passthrough: a box that ends TLS itself, so its edge carries bytes it
//! cannot read.
//!
//! The edge used to end every box's TLS with a Let's Encrypt certificate of
//! its own and send the request on as plain HTTP through the tunnel. Whoever
//! ran the edge, or the machine it runs on, could read every sign-in, file
//! and page that went through it. A box that takes part here keeps its own
//! certificate for each of its public names instead
//! (`modules/public-tls.nix` on the box), and the edge routes those names by
//! the SNI in the ClientHello alone (`config::TcpRoutes`, Traefik's
//! `tls.passthrough`), to the same tunnel port as before. What the edge
//! still sees: the name, the box's address, when and how much.
//!
//! The conversation, one heartbeat at a time:
//!
//! 1. Every box's names get one path on plain `web`, Let's Encrypt's
//!    HTTP-01 ([`ACME_PATH`]), routed to the box. The box's certificate unit
//!    asks for a certificate for its own hostname that way, before it says
//!    anything to the edge, so the move to passthrough costs no moment
//!    without https.
//! 2. Once it holds one, the box heartbeats with `tls`: the names it holds a
//!    valid certificate for, read from a file its certificate unit keeps
//!    ([`read_ready`]). A box that sends no `tls` leaves TLS to the edge, as
//!    every box did before; one that sends an empty list (every certificate
//!    lapsed) gets no `websecure` route at all rather than the edge's.
//! 3. The edge answers a box that sends `tls` with `names`: every public name
//!    it routes to the box (its own hostname, a name in the edge's zone,
//!    custom domains, and on a spoke the hub's custom domains for it). The box
//!    writes them ([`record_names`]) and gets a certificate for each.
//! 4. A name the box holds a certificate for gets a passthrough router; one
//!    it does not gets none on `websecure`. From the moment a box asks, the
//!    edge never asks for a certificate for its names again.
//!
//! What it costs, on purpose: Traefik's offline page needs the edge to end
//! TLS, so a passthrough box that is switched off is a connection error; its
//! names carry no HTTP/3 (Traefik has no QUIC passthrough) and no ECH (the
//! edge would need to hand the box a decrypted ClientHello, `crate::zone`
//! leaves `ech=` off those names). An edge can still get a certificate for
//! a name in its own zone and intercept on purpose; that is an active
//! attack, logged in Certificate Transparency, which the box's hourly check
//! of its own name looks for.

use std::path::Path;

use serde::Deserialize;

use crate::relay::dns_name;

/// Where Let's Encrypt asks for an HTTP-01 answer: the only path of a box's
/// names the edge routes on plain `web`.
pub const ACME_PATH: &str = "/.well-known/acme-challenge/";

/// Most names one box may say it holds, or be told it has. A box with more
/// names than this is a bug or an attempt to grow the edge's router table.
pub const MAX_NAMES: usize = 64;

/// The names in `names` a router may carry: DNS names, lowercase, sorted,
/// each once, at most [`MAX_NAMES`]. Anything else is dropped rather than
/// refused, so one odd line does not cost the box its other names.
#[must_use]
pub fn clean(names: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = names
        .into_iter()
        .map(|n| n.trim().trim_end_matches('.').to_ascii_lowercase())
        .filter(|n| dns_name(n))
        .collect();
    out.sort();
    out.dedup();
    out.truncate(MAX_NAMES);
    out
}

/// The names in the box's ready file, one per line, re-read every
/// heartbeat. No file is `None`: the box leaves TLS to the edge. An empty
/// file is `Some` of nothing: passthrough, and no certificate that is still
/// good.
pub async fn read_ready(file: Option<&str>) -> Option<Vec<String>> {
    let text = match tokio::fs::read_to_string(file?).await {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        // Unreadable is "no certificate yet", never "the edge may end TLS":
        // a box that asked for passthrough must not fall back to it by
        // accident.
        Err(_) => return Some(Vec::new()),
    };
    Some(clean(text.lines().map(str::to_string)))
}

/// The names in a heartbeat's answer, or `None` when it names none (an
/// older edge, or a box that does not end TLS).
#[must_use]
pub fn names_from_answer(body: Option<&str>) -> Option<Vec<String>> {
    #[derive(Deserialize)]
    struct Answer {
        #[serde(default)]
        names: Option<Vec<String>>,
    }
    let names = serde_json::from_str::<Answer>(body?).ok()?.names?;
    let names = clean(names);
    (!names.is_empty()).then_some(names)
}

/// Keep the names the edge routes to this box in `file`, one per line, for
/// the certificate unit; written only when they change, since a path unit
/// watches it. An answer with none leaves the file as it was: the names of
/// a box do not vanish because one heartbeat went to an older edge.
///
/// # Errors
/// The file could not be written.
pub async fn record_names(file: &str, body: Option<&str>) -> std::io::Result<()> {
    let Some(names) = names_from_answer(body) else {
        return Ok(());
    };
    let mut text = names.join("\n");
    text.push('\n');
    let path = Path::new(file);
    if tokio::fs::read_to_string(path).await.ok().as_deref() == Some(text.as_str()) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, text).await?;
    tokio::fs::rename(&tmp, path).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_keeps_dns_names_once_in_lowercase() {
        assert_eq!(
            clean(vec![
                "Box.Example.".to_string(),
                "box.example".to_string(),
                "../etc/passwd".to_string(),
                "a b".to_string(),
                String::new(),
                "x.example".to_string(),
            ]),
            vec!["box.example".to_string(), "x.example".to_string()]
        );
    }

    #[test]
    fn clean_caps_the_list() {
        let many = (0..200).map(|n| format!("n{n}.example"));
        assert_eq!(clean(many).len(), MAX_NAMES);
    }

    #[test]
    fn an_answer_without_names_is_none() {
        assert_eq!(names_from_answer(None), None);
        assert_eq!(names_from_answer(Some("")), None);
        assert_eq!(names_from_answer(Some(r#"{"hidden":{}}"#)), None);
        assert_eq!(names_from_answer(Some(r#"{"names":[]}"#)), None);
        assert_eq!(
            names_from_answer(Some(r#"{"names":["B.example","a.example","bad name"]}"#)),
            Some(vec!["a.example".to_string(), "b.example".to_string()])
        );
    }

    #[tokio::test]
    async fn the_ready_file_tells_passthrough_from_none() {
        let dir = std::env::temp_dir().join(format!("losos-tls-ready-{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let file = dir.join("ready");
        let path = file.to_str().unwrap();
        assert_eq!(read_ready(Some(path)).await, None);
        assert_eq!(read_ready(None).await, None);
        tokio::fs::write(&file, "").await.unwrap();
        assert_eq!(read_ready(Some(path)).await, Some(Vec::new()));
        tokio::fs::write(&file, "box.example\nnot a name\n")
            .await
            .unwrap();
        assert_eq!(
            read_ready(Some(path)).await,
            Some(vec!["box.example".to_string()])
        );
    }

    #[tokio::test]
    async fn names_are_written_only_when_an_answer_has_some() {
        let dir = std::env::temp_dir().join(format!("losos-tls-names-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&dir).await;
        let file = dir.join("sub/names");
        let path = file.to_str().unwrap();
        record_names(path, Some(r#"{"names":["b.example","a.example"]}"#))
            .await
            .unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&file).await.unwrap(),
            "a.example\nb.example\n"
        );
        record_names(path, None).await.unwrap();
        record_names(path, Some("{}")).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&file).await.unwrap(),
            "a.example\nb.example\n"
        );
    }
}
