//! Guests: people with no account on the box, let into one call by a link.
//!
//! A member asks for a link; the SFU opens the call to a fresh invite
//! ([`super::admit::Calls::invite`]) and asks the box's edge to make the
//! link (`POST /calls/guest-link` on the registrar). Only an official edge
//! makes one, on this box's own public name. The guest opens it, gives a
//! name, and joins on `/calls/guest/ws` with the invite; the invite lets
//! one guest at a time into that call and nothing else: no LosOS Login, no
//! chat, no other call.
//!
//! The link the edge makes carries the call and the invite. The inviting
//! browser adds a secret after the `#`, which no server sees, and hands
//! the same secret to the other members over Olm. The guest and each
//! member then agree on a key over the SFU with throwaway ECDH keys, each
//! side proving with the secret that it is who the link was for
//! (`admin-ui/app/src/calls/guests.ts`), and only then trade media keys.
//! The edge knows enough to knock on the call, never enough to hear it.

use anyhow::{bail, Context};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

/// The edge this box tunnels through, as the SFU talks to it.
pub struct Edge {
    /// The registrar's URL, `https://register.<edge>`.
    pub registrar: String,
    /// The appliance id the box registers under.
    pub id: String,
    pub token_file: PathBuf,
    pub curl: PathBuf,
}

const ASK_WITHIN: Duration = Duration::from_secs(20);

impl Edge {
    /// The link a guest opens for `invite` into `call`, as the edge made it.
    pub async fn link(&self, call: &str, invite: &str) -> anyhow::Result<String> {
        let token = tokio::fs::read_to_string(&self.token_file)
            .await
            .with_context(|| format!("reading {}", self.token_file.display()))?;
        let body = serde_json::json!({
            "appliance_id": self.id,
            "token": token.trim(),
            "call": call,
            "invite": invite,
        });
        // The token goes on stdin, never on a command line.
        let mut child = tokio::process::Command::new(&self.curl)
            .args(["-sS", "--max-time", "15", "-w", "\n%{http_code}"])
            .args([
                "-H",
                "Content-Type: application/json",
                "--data-binary",
                "@-",
            ])
            .arg(format!(
                "{}/calls/guest-link",
                self.registrar.trim_end_matches('/')
            ))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("starting curl")?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(body.to_string().as_bytes()).await?;
        }
        let out = tokio::time::timeout(ASK_WITHIN, child.wait_with_output())
            .await
            .context("the edge did not answer")??;
        let text = String::from_utf8_lossy(&out.stdout);
        let (answer, status) = text.rsplit_once('\n').unwrap_or(("", &text));
        if status.trim() != "200" {
            bail!(
                "the edge answered {} {}{}",
                status.trim(),
                answer.trim(),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        let url = serde_json::from_str::<serde_json::Value>(answer)?
            .get("url")
            .and_then(|u| u.as_str())
            .map(str::to_owned)
            .context("the edge's answer has no url")?;
        check_link(&url, call, invite)?;
        Ok(url)
    }
}

/// A link is `https://<host>/calls/guest/?call=<call>&invite=<invite>`:
/// whatever the edge says, the page gets nothing else to hand out.
pub fn check_link(url: &str, call: &str, invite: &str) -> anyhow::Result<()> {
    let rest = url.strip_prefix("https://").context("not an https link")?;
    let (host, tail) = rest.split_once('/').context("no path")?;
    let host_ok = !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-:[]".contains(&b));
    if !host_ok || tail != format!("calls/guest/?call={call}&invite={invite}") {
        bail!("an unexpected link: {url}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: &str = "0123456789abcdef0123456789abcdef";
    const I: &str = "fedcba9876543210fedcba9876543210";

    #[test]
    fn only_a_link_to_this_call_on_a_plain_host_passes() {
        let good = format!("https://mattbox.losos.cfd/calls/guest/?call={C}&invite={I}");
        check_link(&good, C, I).unwrap();
        for bad in [
            format!("http://mattbox.losos.cfd/calls/guest/?call={C}&invite={I}"),
            format!("https://evil.example@mattbox/calls/guest/?call={C}&invite={I}"),
            format!("https://mattbox.losos.cfd/calls/guest/?call={I}&invite={C}"),
            format!("https://mattbox.losos.cfd/calls/guest/?call={C}&invite={I}#x"),
            format!("https://mattbox.losos.cfd/other/?call={C}&invite={I}"),
            "javascript:alert(1)".to_string(),
        ] {
            assert!(check_link(&bad, C, I).is_err(), "{bad}");
        }
    }

    #[tokio::test]
    async fn the_link_comes_from_the_registrar_with_the_token_on_stdin() {
        let dir = assert_fs::TempDir::new().unwrap();
        let token = dir.path().join("token");
        std::fs::write(&token, "secret-token\n").unwrap();
        // A curl that writes what it got and answers as the edge would.
        let curl = dir.path().join("curl");
        let seen = dir.path().join("seen");
        std::fs::write(
            &curl,
            format!(
                "#!/bin/sh\necho \"$@\" > {seen}.args\ncat > {seen}\nprintf '{{\"url\":\"https://mattbox.example/calls/guest/?call={C}&invite={I}\"}}\\n200'\n",
                seen = seen.display()
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&curl, std::fs::Permissions::from_mode(0o755)).unwrap();
        let edge = Edge {
            registrar: "https://register.example/".into(),
            id: "mattbox".into(),
            token_file: token,
            curl,
        };
        let url = edge.link(C, I).await.unwrap();
        assert!(url.starts_with("https://mattbox.example/calls/guest/"));
        let body: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&seen).unwrap()).unwrap();
        assert_eq!(body["token"], "secret-token");
        assert_eq!(body["appliance_id"], "mattbox");
        let args = std::fs::read_to_string(dir.path().join("seen.args")).unwrap();
        assert!(!args.contains("secret-token"), "{args}");
        assert!(
            args.contains("https://register.example/calls/guest-link"),
            "{args}"
        );
    }
}
