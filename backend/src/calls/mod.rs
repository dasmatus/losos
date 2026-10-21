//! LosOS calls: video and voice calls from chat rooms, end to end encrypted.
//!
//! The `losos-calls` binary is a forwarding SFU on str0m. It runs as the
//! `shared` user, and only while the box is on its shared side (sharing on
//! and in a mesh), like chat itself (`modules/calls.nix`).
//!
//! What the SFU does and does not know:
//!
//! * It knows who is in a call: each page proves its Matrix id with an
//!   OpenID token ([`openid`]), and only the people the caller listed from
//!   the room get in ([`admit`]).
//! * It never has a media key. Every frame is encrypted in the sender's
//!   browser before it reaches WebRTC's packetizer and decrypted in each
//!   receiver's (`admin-ui/app/src/calls/`), with keys the browsers send
//!   each other over Olm. The SFU forwards that ciphertext inside DTLS-SRTP
//!   ([`sfu`]); [`tap`] writes it down so a test can prove it.
//!
//! The protocol is in `docs/calls.md` and [`wire`].

pub mod admit;
pub mod openid;
pub mod server;
pub mod sfu;
pub mod tap;
pub mod wire;
