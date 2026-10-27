//! Open gateways: what a box checks before it trusts an edge it does not
//! know with the market. Anyone may run one; a box trusts it when four
//! things hold, and nothing else is asked of the operator:
//!
//! - its owner signed a passport with their OpenPGP key ([`passport`]),
//! - its TPM shows it runs the published gateway image, untouched since it
//!   booted ([`tpm`], [`ekcert`], [`eventlog`], [`pe`]),
//! - its fee settings pay LosOS's 2% to the addresses the box ships, and
//!   the owner's 2% to the addresses the owner signed ([`check`]),
//! - and LosOS's bot has blessed it and not revoked it (outside this crate).
//!
//! Everything here is pure: the box fetches, this module judges.

pub mod check;
pub mod ekcert;
pub mod eventlog;
pub mod passport;
pub mod pe;
pub mod tpm;
pub mod uki;
