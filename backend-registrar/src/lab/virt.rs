//! The Lab's second way to libvirt: a byte relay for the browser's own
//! libvirt client.
//!
//! `admin-ui/lab/virt-rpc` (`losos-lab-virt`) speaks libvirt's remote
//! protocol from WebAssembly, so the page needs no `virsh` on the host. A
//! page cannot open a unix socket, though, so `GET /lab/v1/virt` upgrades to
//! a WebSocket and copies bytes between it and the daemon's socket for the
//! helper's `--connect` URI. The relay parses nothing: every binary message
//! goes to the socket as it is, and every read from the socket goes back as
//! one binary message.
//!
//! Security: whoever holds this socket has libvirt at the helper's uid.
//! libvirt identifies a unix-socket peer by `SO_PEERCRED`, so it sees the
//! helper and never the page, and polkit asks about the helper too. Under
//! `qemu:///system` that is root-equivalent, because a domain may attach any
//! host file or block device, and the relay cannot narrow it without parsing
//! the protocol. So the socket is admitted only with a ticket from `POST
//! /lab/v1/virt-ticket`: 30 seconds, one use, carried as the `ticket.<hex>`
//! WebSocket subprotocol like the guest sockets'. Minting one wants the
//! bearer token when the helper has `--token-file` (on the box, the admin
//! token, which can already rebuild the whole system), and otherwise the
//! loopback-only rule every route has. The Origin and Host checks of
//! [`super::gate`] apply as everywhere else, which matters more here than
//! anywhere: a WebSocket is not bound by CORS, so any site open in the same
//! browser can try `ws://127.0.0.1:8095`.
//!
//! Domains made through the relay are the page's, not the helper's: the
//! helper does not name, count, sweep or destroy them. The client creates
//! them with `AUTODESTROY`, so libvirt ends them when the relayed connection
//! closes, and the helper closes it when the WebSocket goes or the helper
//! stops.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use subtle::ConstantTimeEq;
use tokio::net::UnixStream;

/// How long a ticket from `POST virt-ticket` stays good.
pub const TICKET_TTL: Duration = Duration::from_secs(30);
/// Tickets held at once; minting another drops the oldest.
const MAX_TICKETS: usize = 16;
/// Longest the helper waits for the daemon's socket to accept.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// The unix sockets that may serve a local `qemu` URI, in the order libvirt
/// itself tries them: the modular `virtqemud` first, then the monolithic
/// `libvirtd` (or `virtproxyd`), which both answer on `libvirt-sock`.
///
/// `?socket=PATH` in the URI names the socket outright, as it does for
/// `virsh`. `qemu:///session` lives under `$XDG_RUNTIME_DIR/libvirt`, or
/// `~/.cache/libvirt` without one (GLib's fallback, which libvirt uses);
/// `qemu:///system` under `/run/libvirt`. A URI with a host, or another
/// transport than `unix`, has no local socket.
pub fn socket_candidates(
    uri: &str,
    runtime_dir: Option<&Path>,
    home: Option<&Path>,
) -> Result<Vec<PathBuf>, String> {
    let (base, query) = uri.split_once('?').unwrap_or((uri, ""));
    for pair in query.split('&') {
        if let Some(raw) = pair.strip_prefix("socket=") {
            let path = PathBuf::from(percent_decode(raw)?);
            if !path.is_absolute() {
                return Err(format!("socket= in {uri} is not an absolute path"));
            }
            return Ok(vec![path]);
        }
    }
    let rest = base
        .strip_prefix("qemu+unix://")
        .or_else(|| base.strip_prefix("qemu://"))
        .ok_or_else(|| format!("{uri} is not a local qemu URI"))?;
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    if !host.is_empty() {
        return Err(format!(
            "{uri} names a host; the relay reaches local sockets only"
        ));
    }
    let dir = match path.trim_end_matches('/') {
        "system" => PathBuf::from("/run/libvirt"),
        "session" => match (runtime_dir, home) {
            (Some(r), _) if r.is_absolute() => r.join("libvirt"),
            (_, Some(h)) if h.is_absolute() => h.join(".cache/libvirt"),
            _ => return Err("no XDG_RUNTIME_DIR and no HOME to find the session socket in".into()),
        },
        other => return Err(format!("{uri}: /{other} is neither /session nor /system")),
    };
    Ok(vec![dir.join("virtqemud-sock"), dir.join("libvirt-sock")])
}

/// `%2F` and friends, as libvirt's URI parser decodes a query value.
fn percent_decode(s: &str) -> Result<String, String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
                .ok_or_else(|| format!("bad escape in {s:?}"))?;
            out.push(hex);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| format!("{s:?} is not UTF-8"))
}

/// The first candidate that accepts a connection, and that connection.
pub async fn connect(candidates: &[PathBuf]) -> Result<(PathBuf, UnixStream), String> {
    let mut why = Vec::new();
    for p in candidates {
        match tokio::time::timeout(CONNECT_TIMEOUT, UnixStream::connect(p)).await {
            Ok(Ok(s)) => return Ok((p.clone(), s)),
            Ok(Err(e)) => why.push(format!("{}: {e}", p.display())),
            Err(_) => why.push(format!("{}: no answer", p.display())),
        }
    }
    if why.is_empty() {
        return Err("no socket to try".into());
    }
    Err(why.join("; "))
}

/// Short-lived, single-use tickets for the relay socket.
#[derive(Default)]
pub struct Tickets {
    live: Mutex<Vec<(String, Instant)>>,
}

impl Tickets {
    fn prune(live: &mut Vec<(String, Instant)>, now: Instant) {
        live.retain(|(_, at)| now.duration_since(*at) < TICKET_TTL);
    }

    /// Keep a fresh ticket and hand it out.
    pub fn issue(&self, ticket: String) {
        let mut live = super::lock(&self.live);
        Self::prune(&mut live, Instant::now());
        if live.len() >= MAX_TICKETS {
            live.remove(0);
        }
        live.push((ticket, Instant::now()));
    }

    /// Spend a ticket: true once for a live one, false ever after. Every
    /// held ticket is compared in constant time.
    pub fn take(&self, given: &str) -> bool {
        let mut live = super::lock(&self.live);
        Self::prune(&mut live, Instant::now());
        let mut hit = None;
        for (i, (t, _)) in live.iter().enumerate() {
            if bool::from(t.as_bytes().ct_eq(given.as_bytes())) {
                hit = Some(i);
            }
        }
        match hit {
            Some(i) if !given.is_empty() => {
                live.remove(i);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sockets_follow_libvirts_own_layout() {
        let run = Path::new("/run/user/1000");
        let home = Path::new("/home/me");
        assert_eq!(
            socket_candidates("qemu:///session", Some(run), Some(home)).expect("session"),
            vec![
                PathBuf::from("/run/user/1000/libvirt/virtqemud-sock"),
                PathBuf::from("/run/user/1000/libvirt/libvirt-sock"),
            ]
        );
        assert_eq!(
            socket_candidates("qemu:///session", None, Some(home)).expect("no runtime dir"),
            vec![
                PathBuf::from("/home/me/.cache/libvirt/virtqemud-sock"),
                PathBuf::from("/home/me/.cache/libvirt/libvirt-sock"),
            ]
        );
        assert_eq!(
            socket_candidates("qemu+unix:///system", Some(run), Some(home)).expect("system"),
            vec![
                PathBuf::from("/run/libvirt/virtqemud-sock"),
                PathBuf::from("/run/libvirt/libvirt-sock"),
            ]
        );
        assert_eq!(
            socket_candidates("qemu:///system?socket=/run/lvr/virtqemud-sock", None, None)
                .expect("socket="),
            vec![PathBuf::from("/run/lvr/virtqemud-sock")]
        );
        assert_eq!(
            socket_candidates("qemu:///session?mode=direct&socket=%2Ftmp%2Fs", None, None)
                .expect("escaped"),
            vec![PathBuf::from("/tmp/s")]
        );
        for bad in [
            "qemu://box.example/system",
            "qemu+ssh://me@box/system",
            "qemu+tcp:///system",
            "qemu:///embed",
            "qemu:///system?socket=relative",
            "qemu:///system?socket=%zz",
        ] {
            assert!(
                socket_candidates(bad, Some(run), Some(home)).is_err(),
                "{bad}"
            );
        }
        assert!(socket_candidates("qemu:///session", None, None).is_err());
    }

    #[test]
    fn a_ticket_is_good_once() {
        let t = Tickets::default();
        t.issue("aa".repeat(16));
        assert!(!t.take(""));
        assert!(!t.take("bb"));
        assert!(t.take(&"aa".repeat(16)));
        assert!(!t.take(&"aa".repeat(16)), "spent");
    }

    #[test]
    fn old_tickets_make_room() {
        let t = Tickets::default();
        for i in 0..=MAX_TICKETS {
            t.issue(format!("{i:032x}"));
        }
        assert!(!t.take(&format!("{:032x}", 0)), "the oldest went");
        assert!(t.take(&format!("{MAX_TICKETS:032x}")));
    }
}
