//! Box discovery (mDNS `_losos._tcp`), port of the Kotlin companion's
//! `Discovery.kt` intent.
//!
//! Full mDNS/Bonjour/DNS-SD is platform-specific:
//! - Linux: Avahi (`avahi-browse -rtp _losos._tcp`) or systemd-resolved/dbus
//! - macOS/iOS: Bonjour
//! - Android: NsdManager (requires platform bridge in Dioxus mobile)
//! - Web: not available
//!
//! This module provides a cross-platform interface and a best-effort native
//! implementation where system tools are available. The UI can fall back to
//! manual address entry (always supported).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DiscoveredBox {
    pub name: String,
    pub host: String,
    pub port: Option<u16>,
    pub txt: Vec<(String, String)>,
}

impl DiscoveredBox {
    pub fn address(&self) -> String {
        if let Some(p) = self.port {
            if p == 443 || p == 0 {
                self.host.clone()
            } else {
                format!("{}:{}", self.host, p)
            }
        } else {
            self.host.clone()
        }
    }
}

pub fn discovery_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("avahi-browse")
            .arg("--version")
            .output()
            .is_ok()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Best-effort browse. Returns quickly; empty when unavailable.
#[cfg(not(target_arch = "wasm32"))]
pub async fn browse_once() -> Vec<DiscoveredBox> {
    tokio::task::spawn_blocking(browse_sync)
        .await
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
pub async fn browse_once() -> Vec<DiscoveredBox> {
    Vec::new()
}

#[cfg(not(target_arch = "wasm32"))]
fn browse_sync() -> Vec<DiscoveredBox> {
    #[cfg(target_os = "linux")]
    {
        let out = match std::process::Command::new("avahi-browse")
            .args(["-rtp", "_losos._tcp"])
            .output()
        {
            Ok(o) if o.status.success() || !o.stdout.is_empty() => o,
            _ => return Vec::new(),
        };
        let text = String::from_utf8_lossy(&out.stdout);
        let mut found = Vec::new();
        for line in text.lines() {
            // parseable: =;iface;proto;name;type;domain;host;address;port;txt
            if !line.starts_with("=;") {
                continue;
            }
            let parts: Vec<&str> = line.split(';').collect();
            if parts.len() < 10 {
                continue;
            }
            let name = parts[3].to_string();
            let host = parts[7].to_string();
            let port = parts[8].parse::<u16>().ok();
            let txt = parts[9]
                .trim_matches('"')
                .split_whitespace()
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            if !host.is_empty()
                && !found
                    .iter()
                    .any(|b: &DiscoveredBox| b.host == host && b.name == name)
            {
                found.push(DiscoveredBox {
                    name,
                    host,
                    port,
                    txt,
                });
            }
        }
        found
    }
    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovered_address_formats_port() {
        let b = DiscoveredBox {
            name: "box".into(),
            host: "losos.local".into(),
            port: Some(8443),
            txt: vec![],
        };
        assert_eq!(b.address(), "losos.local:8443");
        let b2 = DiscoveredBox {
            host: "losos.local".into(),
            port: Some(443),
            ..Default::default()
        };
        assert_eq!(b2.address(), "losos.local");
    }
}
