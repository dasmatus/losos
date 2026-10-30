//! The log reader (`losos-logs`): the journal of the box's services for
//! Settings › Logs, so nobody has to sign in to the box to see why
//! something failed.
//!
//! It runs apart from lososd, as its own user in the `systemd-journal`
//! group, on a Unix socket only nginx and root may open
//! (`modules/log-reader.nix`), so the page keeps answering while lososd
//! is down or restarting, which is when it is needed most. It reads with
//! [`journal`], hides secrets with [`redact`] before anything else sees a
//! line, groups units by [`catalogue`], and asks lososd who is signed in
//! with [`lososd`] when lososd answers. [`server`] is the HTTP side.

pub mod catalogue;
pub mod journal;
pub mod lososd;
pub mod redact;
pub mod server;
