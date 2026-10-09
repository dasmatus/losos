//! The owner's own disk image, streamed to the edge (`PUT /api/vms/images`).
//!
//! The edge keeps uploaded QCOW2 images for its machines
//! (`backend-registrar/src/server/vm.rs`). The browser cannot reach it, and
//! the file is gigabytes, so lososd neither buffers it nor holds its backend
//! lock while it moves: the handler asks the edge for a single-use upload
//! ticket under the lock, like any market call, then pipes the request body
//! into `curl --upload-file -` a chunk at a time. The ticket is in a curl
//! config file readable by root alone, never in an argument vector, and never
//! reaches the browser.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};

use actix_web::web;
use anyhow::Context;
use futures_core::Stream;

/// What the edge answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub body: String,
}

/// The path a ticket answer names, checked to be exactly the upload route
/// and a 64-hex-character ticket, so nothing else is ever written into the
/// curl config.
#[must_use]
pub fn upload_path_ok(path: &str) -> bool {
    path.strip_prefix("/market/vm-images/upload/")
        .is_some_and(|t| t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit()))
}

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A curl config file holding the upload URL, removed when dropped.
struct CurlConfig(PathBuf);

impl CurlConfig {
    fn write(url: &str) -> anyhow::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "losos-vm-upload-{}-{}.curlrc",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .with_context(|| format!("creating {}", path.display()))?;
        writeln!(file, "url = \"{url}\"").context("writing the curl config")?;
        Ok(Self(path))
    }
}

impl Drop for CurlConfig {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Stream `payload` to `<registrar><upload_path>`.
///
/// # Errors
/// The path is not an upload path, curl cannot be started, the browser
/// broke off, or curl failed to reach the edge. An answer from the edge,
/// refusals included, is an `Ok`.
pub async fn stream_to_edge(
    registrar_url: &str,
    upload_path: &str,
    mut payload: web::Payload,
) -> anyhow::Result<Answer> {
    /// The edge's answer is one small JSON object or a sentence.
    const MAX_ANSWER: &str = "65536";
    if !upload_path_ok(upload_path) {
        anyhow::bail!("the edge named an upload path that is not one");
    }
    let config = CurlConfig::write(&format!("{registrar_url}{upload_path}"))?;
    let mut child = std::process::Command::new("curl")
        .args(["--silent", "--show-error"])
        .args([
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-redirs",
            "0",
        ])
        // A transfer that moves nothing for two minutes is dead.
        .args(["--speed-limit", "1", "--speed-time", "120"])
        .args(["--max-filesize", MAX_ANSWER])
        .args(["--header", "Content-Type: application/octet-stream"])
        .args(["--header", "Expect:"])
        .args(["--write-out", "\n%{http_code}"])
        .arg("--config")
        .arg(&config.0)
        .args(["--upload-file", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("running curl (is it on lososd's unit path? see modules/daemon.nix)")?;
    let mut stdin = child.stdin.take();
    loop {
        let next = std::future::poll_fn(|cx| Pin::new(&mut payload).poll_next(cx)).await;
        let Some(chunk) = next else { break };
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!("the upload broke off: {e}");
            }
        };
        let Some(mut pipe) = stdin.take() else { break };
        // A write blocks while curl waits on the edge; it runs on actix's
        // blocking pool so the server's own thread keeps answering.
        match web::block(move || pipe.write_all(&chunk).map(|()| pipe)).await {
            Ok(Ok(pipe)) => stdin = Some(pipe),
            // curl has gone, which its exit status explains below.
            _ => break,
        }
    }
    drop(stdin);
    let out = web::block(move || child.wait_with_output())
        .await
        .context("waiting for curl")?
        .context("waiting for curl")?;
    drop(config);
    if !out.status.success() {
        anyhow::bail!(
            "the upload to the edge failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let text = String::from_utf8(out.stdout).context("the edge answered non-UTF-8")?;
    let (body, code) = text
        .rsplit_once('\n')
        .context("curl returned no status line")?;
    Ok(Answer {
        status: code
            .trim()
            .parse()
            .context("curl returned a status that is not a number")?,
        body: body.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_upload_route_with_a_ticket_is_written_into_the_config() {
        let ticket = "ab".repeat(32);
        assert!(upload_path_ok(&format!(
            "/market/vm-images/upload/{ticket}"
        )));
        assert!(!upload_path_ok("/market/vm-images/upload/short"));
        assert!(!upload_path_ok(&format!(
            "/market/vm-images/upload/{ticket}\"\nurl = \"https://elsewhere"
        )));
        assert!(!upload_path_ok(&format!("/market/orders/{ticket}")));
    }
}
