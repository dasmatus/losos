//! libvirt's remote protocol, written for a browser.
//!
//! libvirtd, virtqemud and virtproxyd speak XDR-encoded RPC over a byte
//! stream (a unix socket, TCP 16509 or TLS 16514). A web page can open none
//! of those, but it can open a WebSocket, and a relay that copies bytes
//! between a WebSocket and the daemon's socket understands nothing of the
//! protocol. This crate is the other half: the whole client, as a sans-IO
//! state machine ([`Connection`]) plus a wasm-bindgen wrapper (`VirtClient`,
//! wasm32 only) that drives it over a WebSocket or a Direct Sockets
//! `TCPSocket`. See README.md for what the host needs and what it costs.

#![forbid(unsafe_code)]

pub mod client;
pub mod proto;
pub mod xdr;

#[cfg(not(target_arch = "wasm32"))]
pub mod blocking;
#[cfg(target_arch = "wasm32")]
mod web;

pub use client::{
    version_string, Connection, Event, Reply, CONSOLE_FORCE, START_AUTODESTROY, START_PAUSED,
};
pub use proto::{AuthType, Domain, ProtoError, RpcError};
