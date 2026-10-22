//! LosOS calls: video and voice calls from chat rooms, end to end encrypted.
//!
//! The `losos-calls` binary is a forwarding SFU on str0m. It runs as the
//! `shared` user, and only while the box is on its shared side (sharing on
//! and in a mesh), like chat itself (`modules/calls.nix`).
//!
//! What the SFU does and does not know:
//!
//! * It knows who is in a call: nginx names the person LosOS Login signed
//!   in ([`person`]), and only the people the caller listed from the room
//!   get in ([`admit`]).
//! * It never has a media key. Every frame is encrypted in the sender's
//!   browser before it reaches WebRTC's packetizer and decrypted in each
//!   receiver's (`admin-ui/app/src/calls/`), with keys the browsers send
//!   each other over Olm. The SFU forwards that ciphertext inside DTLS-SRTP
//!   ([`sfu`]); [`tap`] writes it down so a test can prove it.
//!
//! * In podcast mode one member's browser composes the call and sends the
//!   recording here; [`stream`] hands it to ffmpeg for an RTMP address.
//!   That stream is public by choice, and the page says so.
//!
//! * Someone with no account comes in only on a guest link to one call,
//!   which only the box's official edge makes ([`guests`]).
//!
//! Where the box tunnels through an edge, media goes through it too
//! ([`relay`]), and the box opens no port for calls.
//!
//! The protocol is in [`wire`] and [`stream`]; `handbook/docs/manual/calls.md`
//! is the owner's side.

pub mod admit;
pub mod guests;
pub mod person;
pub mod relay;
pub mod server;
pub mod sfu;
pub mod stream;
pub mod tap;
pub mod wire;
