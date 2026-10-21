//! Personal access tokens and the OpenPGP envelope of the `losos` terminal
//! client, for both servers that answer it: `lososd` on a box and
//! `losos-registrar` on an edge. The wire contract is
//! `docs/terminal-access.md`; this crate is its server half.
//!
//! A call is honoured only with two things at once: a token the owner made
//! in the web UI, and a signature by the GPG key that token is bound to,
//! which had to be seeded before the server ran (`losos.access.gpgKeys`,
//! `losos.edge.access.gpgKeys`). Neither alone gets in. The envelope is one
//! OpenPGP message, signed by the client and encrypted to the server's own
//! key ([`HostKey`]); the answer is signed by the server and encrypted back
//! to the key that signed the request. That is why the plain HTTP of a LAN
//! box carries neither the token nor the answers in the clear.
//!
//! Everything here is pure apart from [`HostKey::load_or_create`] and
//! [`Tokens`], which own one file each. Time is passed in, so freshness,
//! expiry and the replay window are tested without sleeping.

mod client;
mod envelope;
mod keys;
mod tokens;

pub use client::{delegate, open_call, seal_call, AnswerError, Unlocked, UserKey, UserKeyError};
pub use envelope::{
    open, open_answer, seal, seal_request, Answer, Delegated, Delegation, Opened, Refusal, Replay,
    Request, DELEGATION_KIND, MAX_SKEW_SECS, REPLAY_SECS,
};
pub use keys::{HostKey, KeyError, Keyring, SeededKey};
pub use tokens::{
    agent_route, path_allowed, Scope, TokenError, TokenRecord, TokenView, Tokens, AGENT_ROUTES,
    MAX_NAME_CHARS, MAX_TOKENS,
};

/// A fingerprint as the contract writes it: upper-case hex, no spaces.
pub(crate) fn fingerprint_hex(fp: &pgp::types::Fingerprint) -> String {
    format!("{fp:X}")
}

/// Now, as Unix seconds. The one clock read in the crate, for callers.
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
