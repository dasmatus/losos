//! libvirt through its own CLI.
//!
//! The helper links no libvirt library: every call is one `virsh -c URI ...`
//! child with an argument vector (no shell), a timeout, and stdin closed
//! unless it carries a domain's XML. The binary is whatever `--virsh` or
//! `$LOSOS_LAB_VIRSH` names, so the tests put a fake one there.

use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Longest any one virsh call may take. `create` of a small guest takes well
/// under a second; libvirtd starting on demand for `qemu:///session` takes a
/// few.
const TIMEOUT: Duration = Duration::from_secs(30);

/// Every domain the helper makes carries this prefix, and the start-up sweep
/// destroys only names that carry it.
pub const PREFIX: &str = "losos-lab-";

#[derive(Debug, Clone)]
pub struct Virsh {
    pub bin: String,
    pub uri: String,
}

impl Virsh {
    async fn run(&self, args: &[&str], stdin: Option<&str>) -> Result<String, String> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("-q")
            .arg("-c")
            .arg(&self.uri)
            .args(args)
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("cannot run {}: {e}", self.bin))?;
        if let (Some(text), Some(mut pipe)) = (stdin, child.stdin.take()) {
            pipe.write_all(text.as_bytes())
                .await
                .map_err(|e| format!("writing to virsh: {e}"))?;
            drop(pipe);
        }
        let out = tokio::time::timeout(TIMEOUT, child.wait_with_output())
            .await
            .map_err(|_| format!("virsh {} took longer than {TIMEOUT:?}", args[0]))?
            .map_err(|e| format!("virsh {}: {e}", args[0]))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(first_lines(&String::from_utf8_lossy(&out.stderr)))
        }
    }

    /// Does libvirt answer on this URI? Returns the URI it settled on.
    pub async fn uri(&self) -> Result<String, String> {
        self.run(&["uri"], None).await.map(|s| s.trim().to_string())
    }

    /// Can this URI run `type='kvm'` domains?
    pub async fn kvm(&self) -> bool {
        self.run(&["domcapabilities", "--virttype", "kvm"], None)
            .await
            .is_ok()
    }

    /// Start a transient domain. It is never defined, so libvirt forgets it
    /// the moment it stops.
    pub async fn create(&self, xml: &str) -> Result<(), String> {
        self.run(&["create", "/dev/stdin"], Some(xml))
            .await
            .map(|_| ())
    }

    pub async fn destroy(&self, name: &str) -> Result<(), String> {
        self.run(&["destroy", name], None).await.map(|_| ())
    }

    /// The running domains whose names carry [`PREFIX`].
    pub async fn ours(&self) -> Result<Vec<String>, String> {
        let out = self.run(&["list", "--name"], None).await?;
        Ok(out
            .lines()
            .map(str::trim)
            .filter(|n| n.starts_with(PREFIX))
            .map(str::to_string)
            .collect())
    }
}

/// virsh's error, short enough for a JSON answer: the first two non-empty
/// lines, at most 300 characters.
fn first_lines(s: &str) -> String {
    let joined = s
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");
    let msg = if joined.is_empty() {
        "virsh failed and said nothing".to_string()
    } else {
        joined
    };
    msg.chars().take(300).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_cut_to_two_lines() {
        assert_eq!(
            first_lines("\nerror: failed to connect\nerror: no socket\nmore\n"),
            "error: failed to connect error: no socket"
        );
        assert_eq!(first_lines(""), "virsh failed and said nothing");
        assert_eq!(first_lines(&"x".repeat(1000)).len(), 300);
    }
}
