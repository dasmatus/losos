//! The optional storage / compute market, settled through Stripe Connect.
//!
//! Boxes that share disk or CPU with the mesh (see `wiki/Mesh.md`) can *sell*
//! it, and any other box can *buy* it. The edge is the marketplace: it keeps
//! the listings and the orders, and it is the Stripe **platform** account.
//! Money never touches a losos box. A buyer pays through a Stripe Checkout
//! Session created as a *destination charge*: the full amount is charged on
//! the platform, `transfer_data[destination]` forwards it to the seller's
//! connected Express account, and `application_fee_amount` — the platform's
//! cut, 4% (400 basis points) by default — stays behind to cover the edge's
//! running costs.
//!
//! Everything here is **opt-in at three levels**, and each is closed by
//! default:
//!   * the edge only serves the routes when it is started with
//!     `--market-gate-socket` (`losos.edge.market.enable`); otherwise every
//!     market route answers 503, exactly like `/cluster/join` on a proxy-only
//!     edge;
//!   * a tenant may only buy or sell when the operator set
//!     `losos.edge.tenants.<id>.market`, the same second-whitelist shape as
//!     `cluster`;
//!   * a seller can list only after Stripe reports its connected account able
//!     to receive transfers — a listing that cannot be paid out is never shown.
//!
//! The module splits like the rest of the crate: the pricing, the capacity
//! accounting, the webhook signature check and the order state machine are
//! pure functions over [`MarketState`] and are unit-tested below; [`Market`]
//! adds the persisted store, and every Stripe call goes through
//! [`crate::stripe_gate::GateClient`] to the separate process that holds the key.
//! Integration tests drive the real routes against a Stripe stub.
//!
//! What a *paid* order means: the order record is the entitlement. Nothing
//! here provisions a Longhorn volume or schedules a pod yet — fulfilment reads
//! `paid` orders and is deliberately a separate step, so that the money path
//! can be run and audited before anything is handed out against it.
//!
//! Refunds and disputes are handled in the Stripe dashboard. A destination
//! charge refunded there should use `reverse_transfer` and
//! `refund_application_fee`; this module does not model either.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::action::Action;
use crate::fsutil::atomic_write;
use crate::hardware::{total, Catalogue, Line};
use crate::stripe_gate::{CheckoutRequest, GateClient, HardwareRequest};

/// The cut kept by the platform, in basis points of the gross amount: 4%.
pub const DEFAULT_FEE_BPS: u32 = 400;
/// The most an operator may configure. A typo (`--market-fee-bps 4000`) would
/// otherwise silently take 40% of every sale.
pub const MAX_FEE_BPS: u32 = 2_000;

/// Smallest charge, in minor units. Stripe refuses anything below roughly 50
/// cents, so rejecting it here gives the caller a precise 400 rather than an
/// opaque 502.
const MIN_CHARGE_MINOR: u64 = 50;
/// Upper bounds. Generous, but they make every `price * quantity` product fit
/// `u64` with room to spare and keep one typo from minting a six-figure
/// Checkout Session.
const MAX_UNIT_PRICE_MINOR: u64 = 1_000_000;
const MAX_QUANTITY: u64 = 1_000_000;
const MAX_CAPACITY: u64 = 1_000_000_000;
/// Active listings one seller may hold at once.
const MAX_LISTINGS_PER_SELLER: usize = 20;
/// Unpaid checkouts one buyer may hold at once. Every order reserves its
/// units the moment it is written and keeps them until Stripe says the
/// session expired (31 minutes) or, if that webhook is lost, for three days
/// more. With no cap a buyer could order every listing's whole stock, never
/// pay, and reorder each half hour, so nothing on the market could ever be
/// bought; each order is also a row `market.json` keeps for a month.
const MAX_PENDING_PER_BUYER: usize = 3;
/// Two-decimal currencies only. The admin UI converts prices with a fixed
/// factor of 100 (`toMinorUnits` and `formatMoney` in
/// admin-ui/app/src/screens/settings/market.ts), and [`MIN_CHARGE_MINOR`] is in
/// those units too, so a zero- or three-decimal currency added here would be
/// charged and shown 100x or 10x off until both learn its exponent.
pub const SUPPORTED_CURRENCIES: &[&str] = &["aud", "cad", "chf", "eur", "gbp", "nzd", "usd"];

/// Lifetime requested for a Checkout Session. Stripe requires at least 30 minutes
/// from receipt, so include one minute for persistence and network transit.
const CHECKOUT_TTL_SECS: u64 = 31 * 60;
/// How long past the session's expiry a pending order keeps its capacity when
/// no terminal event (`checkout.session.completed` or `.expired`) has arrived.
///
/// The reservation is released by the event, not by the clock: Stripe sends
/// `checkout.session.expired` as the session lapses, so an abandoned checkout
/// frees its units at about the 31 minute mark. The clock is only the backstop
/// for an event that never comes, and it is Stripe's own retry horizon (three
/// days in live mode) on purpose. A shorter one released the units while a
/// `completed` event for them could still be on its way, held up by an edge or
/// gate outage, and the late event then marked the order paid over units
/// another buyer had already reserved.
const PENDING_HOLD_SECS: u64 = 3 * 24 * 3600;
/// How long a paid order's entitlement lasts. Storage is priced per GiB-month
/// and compute per vCPU-hour, but both are sold as a one-month rental: the
/// units are the size of the grant, the 30 days its lifetime.
pub const ENTITLEMENT_SECS: u64 = 30 * 24 * 3600;
/// The Longhorn `StorageClass` a purchased volume is provisioned from, when
/// `--market-storage-class` does not say otherwise (Longhorn's own default).
pub const DEFAULT_STORAGE_CLASS: &str = "longhorn";
/// Namespace prefix for everything bought by one appliance.
const NAMESPACE_PREFIX: &str = "market-";
/// Expired orders are only history; drop them after a month.
const EXPIRED_RETENTION_SECS: u64 = 30 * 24 * 3600;
/// Orders shown per side (purchases, sales) in the account view. Paid orders
/// are the ledger and are never pruned, so without a cap an active trader's
/// view would outgrow lososd's 1 MiB relay limit and the pane would stop
/// loading. Live orders sort first, so a cut only ever drops old history.
const ACCOUNT_HISTORY: usize = 100;
/// Stripe's recommended replay window for webhook timestamps.
const WEBHOOK_TOLERANCE_SECS: u64 = 5 * 60;

const STORE_FILE_MODE: u32 = 0o600;

/// What is being sold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Capacity in the mesh's Longhorn pool, priced per GiB-month.
    Storage,
    /// Scheduling time on the seller's node inside its compute window, priced
    /// per vCPU-hour.
    Compute,
}

impl Kind {
    /// The unit a listing of this kind is priced and sized in.
    #[must_use]
    pub const fn unit(self) -> &'static str {
        match self {
            Kind::Storage => "GiB-month",
            Kind::Compute => "vCPU-hour",
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Kind::Storage => "Storage",
            Kind::Compute => "Compute",
        }
    }
}

/// Which appliances are sharing what *right now*, as the edge knows it from
/// `/cluster/join`.
///
/// The market sells nothing of its own: it monetises what an owner already
/// contributes to the mesh. An appliance may therefore only sell storage while
/// its node is enrolled in the mesh cluster, and compute only while it is
/// enrolled *and* has `losos.cluster.shareCompute` on. The set is rebuilt from
/// the registry on every call, so an owner who stops sharing withdraws their
/// listings from the shelf at once; the listings themselves are kept and
/// reappear when sharing resumes. Orders already paid are unaffected.
///
/// The operator's per-tenant `market` bit narrows it the same way (see
/// [`Sharing::only_sellers`]): turning a seller's opt-in off takes their
/// listings off the shelf and out of reach of new orders at once, rather than
/// only stopping them from creating new ones.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sharing {
    enrolled: BTreeSet<String>,
    compute: BTreeSet<String>,
}

impl Sharing {
    #[must_use]
    pub fn from_windows(windows: &BTreeMap<String, crate::window::ComputeWindow>) -> Self {
        Self {
            enrolled: windows.keys().cloned().collect(),
            compute: windows
                .iter()
                .filter(|(_, w)| w.share_compute)
                .map(|(k, _)| k.clone())
                .collect(),
        }
    }

    /// Keep only the sellers `permitted` accepts: the tenants whose `market`
    /// bit is on right now.
    #[must_use]
    pub fn only_sellers(mut self, permitted: impl Fn(&str) -> bool) -> Self {
        self.enrolled.retain(|seller| permitted(seller));
        self.compute.retain(|seller| permitted(seller));
        self
    }

    #[must_use]
    pub fn allows(&self, seller: &str, kind: Kind) -> bool {
        match kind {
            Kind::Storage => self.enrolled.contains(seller),
            Kind::Compute => self.compute.contains(seller),
        }
    }
}

/// A seller's Stripe connected account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seller {
    /// `acct_...`.
    pub account_id: String,
    /// Stripe reports details submitted, payouts enabled and the `transfers`
    /// capability active. Only a ready seller's listings are shown or bought.
    pub ready: bool,
    /// The box's UUID, as it was written onto the Stripe account's metadata.
    /// `None` for an account created before the box sent one.
    #[serde(default)]
    pub box_uuid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listing {
    pub id: String,
    /// The appliance id of the seller. Never put on the wire to anyone else:
    /// the browse view is anonymous so it cannot be used to enumerate which
    /// appliance ids exist.
    pub seller: String,
    pub kind: Kind,
    /// Price per unit in the currency's minor unit (cents).
    pub unit_price: u64,
    /// Units offered in total.
    pub capacity: u64,
    pub active: bool,
    pub created_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    /// A Checkout Session exists; capacity remains reserved until Stripe sends
    /// a terminal event.
    Pending,
    /// Stripe confirmed payment. This is the entitlement.
    Paid,
    /// The session lapsed or could not be created; capacity is released.
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub listing_id: String,
    pub buyer: String,
    pub seller: String,
    pub kind: Kind,
    pub quantity: u64,
    pub unit_price: u64,
    /// Gross amount charged, minor units.
    pub amount: u64,
    /// Platform's share of `amount`.
    pub fee: u64,
    pub currency: String,
    pub status: OrderStatus,
    pub session_id: Option<String>,
    pub created_at: u64,
    pub paid_at: Option<u64>,
    /// When the entitlement lapses: `paid_at` plus [`ENTITLEMENT_SECS`].
    /// `None` on an order that is not paid, or was paid before fulfilment
    /// existed (which therefore never lapses).
    #[serde(default)]
    pub expires_at: Option<u64>,
    /// The Kubernetes volume claim provisioned for a paid storage order, as
    /// `<namespace>/<pvc>`. Written only once the claim is `Bound`, so its
    /// absence on a live paid order means "still to do".
    #[serde(default)]
    pub volume: Option<String>,
    /// The claim the edge is about to ask the apiserver for, as
    /// `<namespace>/<pvc>`. Written *before* the create, so neither a crash
    /// after it nor a claim that never binds can lose track of storage
    /// Longhorn may be holding. The claim may not exist (the create failed);
    /// a `404` then returns the units like any deleted claim.
    #[serde(default)]
    pub claim: Option<String>,
}

/// Everything the market persists, in `market.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketState {
    #[serde(default)]
    pub sellers: BTreeMap<String, Seller>,
    #[serde(default)]
    pub listings: BTreeMap<String, Listing>,
    #[serde(default)]
    pub orders: BTreeMap<String, Order>,
}

/// Market failures, mapped onto HTTP statuses by
/// [`crate::error::ApiError`]. Client-fault variants carry text that is safe
/// to return; `Stripe` and `Store` carry server detail that only reaches the
/// log.
#[derive(Debug, thiserror::Error)]
pub enum MarketError {
    #[error("market not configured on this edge")]
    Unconfigured,
    #[error("market access not permitted for this id")]
    Forbidden,
    #[error("{0}")]
    Invalid(&'static str),
    #[error("no such listing or order")]
    NotFound,
    #[error("{0}")]
    Conflict(&'static str),
    #[error("stripe: {0}")]
    Stripe(String),
    #[error("invalid webhook signature")]
    BadSignature,
    #[error("market store: {0}")]
    Store(String),
}

impl From<io::Error> for MarketError {
    fn from(e: io::Error) -> Self {
        MarketError::Store(e.to_string())
    }
}

impl From<serde_json::Error> for MarketError {
    fn from(e: serde_json::Error) -> Self {
        MarketError::Store(e.to_string())
    }
}

#[must_use]
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// ── pure core ────────────────────────────────────────────────────────────

/// The platform's cut of `amount`, rounded half up. `bps` is clamped to 100%,
/// so the fee can never exceed the charge.
#[must_use]
pub fn fee_for(amount: u64, bps: u32) -> u64 {
    let bps = u128::from(bps.min(10_000));
    let fee = (u128::from(amount) * bps + 5_000) / 10_000;
    u64::try_from(fee).unwrap_or(amount)
}

/// What one order costs and how it splits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Quote {
    pub amount: u64,
    pub fee: u64,
    pub seller_net: u64,
}

/// Price `quantity` units at `unit_price`, enforcing every bound.
pub fn quote(unit_price: u64, quantity: u64, fee_bps: u32) -> Result<Quote, MarketError> {
    if unit_price == 0 || unit_price > MAX_UNIT_PRICE_MINOR {
        return Err(MarketError::Invalid("unit_price out of range"));
    }
    if quantity == 0 || quantity > MAX_QUANTITY {
        return Err(MarketError::Invalid("quantity out of range"));
    }
    let amount = unit_price
        .checked_mul(quantity)
        .ok_or(MarketError::Invalid("amount overflows"))?;
    if amount < MIN_CHARGE_MINOR {
        return Err(MarketError::Invalid(
            "order total is below the minimum charge",
        ));
    }
    let fee = fee_for(amount, fee_bps);
    Ok(Quote {
        amount,
        fee,
        seller_net: amount - fee,
    })
}

/// Whether a pending order still holds its units at `now`.
fn pending_live(order: &Order, now: u64) -> bool {
    order.status == OrderStatus::Pending
        && now < order.created_at + CHECKOUT_TTL_SECS + PENDING_HOLD_SECS
}

/// One side of an account's order history: live orders first, then newest
/// first, at most [`ACCOUNT_HISTORY`] of them.
fn history<'a>(orders: impl Iterator<Item = &'a Order>, now: u64) -> Vec<OrderView> {
    let mut orders: Vec<&Order> = orders.collect();
    orders.sort_by_key(|o| {
        let live = paid_live(o, now) || pending_live(o, now);
        (std::cmp::Reverse(live), std::cmp::Reverse(o.created_at))
    });
    orders
        .into_iter()
        .take(ACCOUNT_HISTORY)
        .map(|o| OrderView::of(o, now))
        .collect()
}

/// Release an order whose Checkout Session could not be created. Only a
/// still-pending order is released: an error from the gate does not prove
/// Stripe made no session (a timeout after Stripe answered looks the same),
/// and a signed `completed` event for that session may already have marked
/// the order paid while the call was in flight. Expiring it then would hand
/// the buyer's paid units back to the shelf.
fn release_failed_checkout(state: &mut MarketState, order_id: &str) -> bool {
    match state.orders.get_mut(order_id) {
        Some(o) if o.status == OrderStatus::Pending => {
            o.status = OrderStatus::Expired;
            true
        }
        _ => false,
    }
}

/// Mark every pending order whose hold has run out as `Expired`, drop expired
/// orders past their retention, and drop closed listings no order still names.
/// Returns the ids that expired and how many records were dropped; both zero
/// means nothing changed.
///
/// [`reserved`] already stops counting such an order, so this changes no
/// availability; it makes the release durable and visible (the order reads
/// `expired` rather than a three-day-old `pending`) and keeps `market.json`
/// from carrying abandoned checkouts forever.
pub fn expire_stale(state: &mut MarketState, now: u64) -> (Vec<String>, usize) {
    let mut expired = Vec::new();
    for order in state.orders.values_mut() {
        if order.status == OrderStatus::Pending && !pending_live(order, now) {
            order.status = OrderStatus::Expired;
            expired.push(order.id.clone());
        }
    }
    let before = state.orders.len() + state.listings.len();
    state.orders.retain(|_, o| {
        !(o.status == OrderStatus::Expired && now > o.created_at + EXPIRED_RETENTION_SECS)
    });
    // A closed listing is only kept while an order still names it; otherwise
    // create-and-close would grow `market.json` without bound.
    let referenced: std::collections::BTreeSet<&str> = state
        .orders
        .values()
        .map(|o| o.listing_id.as_str())
        .collect();
    let keep: std::collections::BTreeSet<String> = state
        .listings
        .values()
        .filter(|l| l.active || referenced.contains(l.id.as_str()))
        .map(|l| l.id.clone())
        .collect();
    state.listings.retain(|id, _| keep.contains(id));
    (expired, before - state.orders.len() - state.listings.len())
}

/// Units of `listing` that are sold, held by a live checkout, or still
/// occupied by a volume.
#[must_use]
pub fn reserved(state: &MarketState, listing_id: &str, now: u64) -> u64 {
    state
        .orders
        .values()
        .filter(|o| o.listing_id == listing_id)
        .filter(|o| paid_live(o, now) || pending_live(o, now) || holds_volume(o))
        .map(|o| o.quantity)
        .sum()
}

/// Whether a storage order's claim may still exist. The volume holds the
/// buyer's data, so nothing deletes it at expiry, and while it exists Longhorn
/// keeps its GiB. Releasing them to the listing at expiry would sell the same
/// capacity twice and leave the next buyer's claim `Pending`. That holds for a
/// claim that never bound too: it still asks for the GiB, and may bind later.
/// The units come back when an operator removes the claim and the reconcile
/// pass sees it gone ([`lapsed_volumes`], `Market::mark_reclaimed`).
fn holds_volume(order: &Order) -> bool {
    order.status == OrderStatus::Paid && order.kind == Kind::Storage && claim_of(order).is_some()
}

/// The order's claim, bound or not. `volume` alone is enough for an order
/// recorded before `claim` existed: a bound claim was certainly created.
fn claim_of(order: &Order) -> Option<&str> {
    order.claim.as_deref().or(order.volume.as_deref())
}

/// Lapsed storage orders whose claim was created and has not been seen gone:
/// `(order id, "<namespace>/<claim>")`. The reconcile pass checks each one
/// against the cluster.
#[must_use]
pub fn lapsed_volumes(state: &MarketState, now: u64) -> Vec<(String, String)> {
    state
        .orders
        .values()
        .filter(|o| holds_volume(o) && !paid_live(o, now))
        .filter_map(|o| Some((o.id.clone(), claim_of(o)?.to_string())))
        .collect()
}

/// Whether a paid order's entitlement is still running at `now`. A lapsed
/// order hands its units back to the listing.
fn paid_live(order: &Order, now: u64) -> bool {
    order.status == OrderStatus::Paid && order.expires_at.is_none_or(|e| now < e)
}

/// What one appliance holds right now, summed over its live paid orders.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Entitlements {
    /// GiB of Longhorn capacity it may claim.
    pub storage_gib: u64,
    /// vCPU-hours of compute credit. This is a ledger entry only: nothing
    /// meters or schedules against it yet.
    pub compute_vcpu_hours: u64,
    /// The soonest `expires_at` among the live orders, if any lapses.
    pub next_expiry: Option<u64>,
}

#[must_use]
pub fn entitlements(state: &MarketState, buyer: &str, now: u64) -> Entitlements {
    let mut out = Entitlements::default();
    for o in state
        .orders
        .values()
        .filter(|o| o.buyer == buyer && paid_live(o, now))
    {
        match o.kind {
            Kind::Storage => out.storage_gib = out.storage_gib.saturating_add(o.quantity),
            Kind::Compute => {
                out.compute_vcpu_hours = out.compute_vcpu_hours.saturating_add(o.quantity);
            }
        }
        if let Some(e) = o.expires_at {
            out.next_expiry = Some(out.next_expiry.map_or(e, |n| n.min(e)));
        }
    }
    out
}

/// The namespace holding everything `buyer` has bought, or `None` if no valid
/// Kubernetes namespace (a DNS label) can be built from the id.
#[must_use]
pub fn namespace_for(buyer: &str) -> Option<String> {
    let name = format!("{NAMESPACE_PREFIX}{buyer}");
    let label = name.len() <= 63
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.ends_with('-');
    label.then_some(name)
}

/// The claim name for an order. Order ids are `ord_<hex>`; `_` is not a legal
/// DNS character.
#[must_use]
pub fn pvc_name(order_id: &str) -> String {
    order_id.replace('_', "-").to_ascii_lowercase()
}

/// One storage order still waiting for its volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provision {
    pub order_id: String,
    pub namespace: String,
    pub pvc: String,
    pub gib: u64,
}

/// Paid, unexpired storage orders with no volume yet. An order whose buyer id
/// cannot name a namespace is skipped (and logged by the caller) rather than
/// retried forever.
#[must_use]
pub fn pending_provisions(state: &MarketState, now: u64) -> Vec<Provision> {
    state
        .orders
        .values()
        .filter(|o| o.kind == Kind::Storage && o.volume.is_none() && paid_live(o, now))
        .filter_map(|o| {
            Some(Provision {
                order_id: o.id.clone(),
                namespace: namespace_for(&o.buyer)?,
                pvc: pvc_name(&o.id),
                gib: o.quantity,
            })
        })
        .collect()
}

/// Units still buyable.
#[must_use]
pub fn available(state: &MarketState, listing: &Listing, now: u64) -> u64 {
    listing
        .capacity
        .saturating_sub(reserved(state, &listing.id, now))
}

/// Check a Stripe webhook's `Stripe-Signature` header.
///
/// The header is `t=<unix>,v1=<hex>[,v1=<hex>...]` and each `v1` is
/// HMAC-SHA256 of `"<t>.<raw body>"` under the endpoint secret. More than one
/// `v1` appears while a secret is being rolled. The comparison is
/// [`ring::hmac::verify`], which is constant time, and a timestamp outside
/// [`WEBHOOK_TOLERANCE_SECS`] is refused so a captured request cannot be
/// replayed later.
pub fn verify_signature(
    secret: &str,
    header: &str,
    body: &[u8],
    now: u64,
) -> Result<(), MarketError> {
    let mut timestamp: Option<u64> = None;
    let mut candidates: Vec<Vec<u8>> = Vec::new();
    for part in header.split(',') {
        match part.trim().split_once('=') {
            Some(("t", v)) => timestamp = v.parse().ok(),
            Some(("v1", v)) => {
                if let Some(bytes) = decode_hex(v) {
                    candidates.push(bytes);
                }
            }
            _ => {}
        }
    }
    let t = timestamp.ok_or(MarketError::BadSignature)?;
    if now.abs_diff(t) > WEBHOOK_TOLERANCE_SECS {
        return Err(MarketError::BadSignature);
    }
    let mut signed = format!("{t}.").into_bytes();
    signed.extend_from_slice(body);
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
    if candidates
        .iter()
        .any(|sig| hmac::verify(&key, &signed, sig).is_ok())
    {
        Ok(())
    } else {
        Err(MarketError::BadSignature)
    }
}

/// The header a sender would produce; exposed for tests and tooling.
#[must_use]
pub fn sign_webhook(secret: &str, body: &[u8], timestamp: u64) -> String {
    let mut signed = format!("{timestamp}.").into_bytes();
    signed.extend_from_slice(body);
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
    let tag = hmac::sign(&key, &signed);
    let hex: String = tag.as_ref().iter().map(|b| format!("{b:02x}")).collect();
    format!("t={timestamp},v1={hex}")
}

pub(crate) fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || !s.is_ascii() {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Apply one verified Stripe event to the state. Returns whether anything
/// changed (and so whether to persist).
///
/// Unknown event types and events about objects this edge never created are
/// ignored, not errors: Stripe delivers every event type the endpoint
/// subscribed to, and answering non-2xx only makes it retry for days.
///
/// A payment only counts if the session's id, amount and currency all equal
/// what this edge recorded when it created the order. The webhook is the one
/// place a buyer's money becomes an entitlement, so it trusts the order, not
/// the event.
pub fn apply_event(state: &mut MarketState, event: &Value, now: u64) -> bool {
    let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
    let object = &event["data"]["object"];
    match kind {
        "checkout.session.completed" => {
            if object["payment_status"].as_str() != Some("paid") {
                return false;
            }
            let Some(order) = object["client_reference_id"]
                .as_str()
                .and_then(|id| state.orders.get(id))
            else {
                return false;
            };
            let session = object["id"].as_str();
            // A missing `session_id` means the edge stopped between Stripe
            // creating the session and the id being written. The event is
            // signed and names this order, which only this edge's gate can
            // have put in `client_reference_id`, so its id is adopted.
            let matches = session.is_some_and(|s| s.starts_with("cs_"))
                && order
                    .session_id
                    .as_deref()
                    .is_none_or(|s| Some(s) == session)
                && object["amount_total"].as_u64() == Some(order.amount)
                && object["currency"]
                    .as_str()
                    .is_some_and(|c| c.eq_ignore_ascii_case(&order.currency));
            if !matches {
                tracing::error!(
                    target: Action::Market.target(),
                    "paid session for order {} does not match the recorded order; not fulfilling",
                    order.id,
                );
                return false;
            }
            match order.status {
                OrderStatus::Paid => return false,
                OrderStatus::Pending => {}
                // Paid after its reservation was released: Stripe retried for
                // longer than the hold. Honour it if the units are still free;
                // otherwise they belong to someone else now and the buyer is
                // owed a refund, which only the dashboard can issue.
                OrderStatus::Expired => {
                    let room = state
                        .listings
                        .get(&order.listing_id)
                        .map_or(0, |l| available(state, l, now));
                    if order.quantity > room {
                        tracing::error!(
                            target: Action::Market.target(),
                            "order {} was paid after its reservation lapsed and its units are sold; refund session {} in the Stripe dashboard",
                            order.id,
                            session.unwrap_or_default(),
                        );
                        return false;
                    }
                    tracing::warn!(
                        target: Action::Market.target(),
                        "order {} was paid after its reservation lapsed; its units are still free, so it is fulfilled",
                        order.id,
                    );
                }
            }
            let id = order.id.clone();
            let Some(order) = state.orders.get_mut(&id) else {
                return false;
            };
            order.session_id = session.map(str::to_string);
            order.status = OrderStatus::Paid;
            order.paid_at = Some(now);
            order.expires_at = Some(now + ENTITLEMENT_SECS);
            true
        }
        "checkout.session.expired" => {
            let Some(order) = object["client_reference_id"]
                .as_str()
                .and_then(|id| state.orders.get_mut(id))
            else {
                return false;
            };
            // Only the session the order is waiting on can release it. A
            // retried order gets a new session, and the old one's `expired`
            // event names the same order; without this it would free units
            // the live session still holds. An order with no recorded
            // session is left to `expire_stale`.
            let current =
                order.session_id.is_some() && order.session_id.as_deref() == object["id"].as_str();
            if order.status == OrderStatus::Pending && current {
                order.status = OrderStatus::Expired;
                true
            } else {
                false
            }
        }
        "account.updated" => {
            let Some(id) = object["id"].as_str() else {
                return false;
            };
            let ready = account_ready(object);
            let mut changed = false;
            for seller in state.sellers.values_mut().filter(|s| s.account_id == id) {
                if seller.ready != ready {
                    seller.ready = ready;
                    changed = true;
                }
            }
            changed
        }
        _ => false,
    }
}

/// Whether a Stripe account object can receive destination-charge transfers.
pub(crate) fn account_ready(account: &Value) -> bool {
    account["details_submitted"].as_bool() == Some(true)
        && account["payouts_enabled"].as_bool() == Some(true)
        && account["capabilities"]["transfers"].as_str() == Some("active")
}

// ── wire views ───────────────────────────────────────────────────────────

/// What `GET /market/listings` shows anyone. No seller identity.
#[derive(Debug, Serialize)]
pub struct PublicListing {
    pub id: String,
    pub kind: Kind,
    pub unit: &'static str,
    pub unit_price: u64,
    pub currency: String,
    pub available: u64,
}

/// A listing as its own seller sees it.
#[derive(Debug, Serialize)]
pub struct OwnListing {
    pub id: String,
    pub kind: Kind,
    pub unit: &'static str,
    pub unit_price: u64,
    pub capacity: u64,
    pub available: u64,
    pub active: bool,
}

/// An order as either party sees it: the counterparty is not named.
#[derive(Debug, Serialize)]
pub struct OrderView {
    pub id: String,
    pub listing_id: String,
    pub kind: Kind,
    pub unit: &'static str,
    pub quantity: u64,
    pub amount: u64,
    pub fee: u64,
    pub seller_net: u64,
    pub currency: String,
    pub status: OrderStatus,
    pub created_at: u64,
    pub paid_at: Option<u64>,
    pub expires_at: Option<u64>,
    /// Whether the entitlement has run out.
    pub expired: bool,
    /// `<namespace>/<claim>` once a storage order's volume exists.
    pub volume: Option<String>,
}

impl OrderView {
    fn of(o: &Order, now: u64) -> Self {
        Self {
            id: o.id.clone(),
            listing_id: o.listing_id.clone(),
            kind: o.kind,
            unit: o.kind.unit(),
            quantity: o.quantity,
            amount: o.amount,
            fee: o.fee,
            seller_net: o.amount - o.fee,
            currency: o.currency.clone(),
            status: o.status,
            created_at: o.created_at,
            paid_at: o.paid_at,
            expires_at: o.expires_at,
            expired: o.status == OrderStatus::Paid && !paid_live(o, now),
            volume: o.volume.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AccountView {
    pub fee_bps: u32,
    pub currency: String,
    pub seller_onboarded: bool,
    pub seller_ready: bool,
    /// Whether the seller's node is enrolled in the mesh, which is what lets
    /// it list storage.
    pub can_sell_storage: bool,
    /// Whether it is enrolled and sharing compute, which is what lets it list
    /// compute.
    pub can_sell_compute: bool,
    pub listings: Vec<OwnListing>,
    pub entitlements: Entitlements,
    pub purchases: Vec<OrderView>,
    pub sales: Vec<OrderView>,
}

#[derive(Debug, Serialize)]
pub struct OnboardView {
    pub ready: bool,
    /// Stripe-hosted onboarding link, absent once the account is ready.
    pub url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CheckoutView {
    pub order_id: String,
    pub checkout_url: String,
    pub amount: u64,
    pub fee: u64,
    pub currency: String,
}

/// A hardware Checkout, as the box that asked for it sees it.
#[derive(Debug, Serialize)]
pub struct HardwareCheckoutView {
    pub order_id: String,
    pub checkout_url: String,
    pub amount: u64,
    pub currency: String,
}

#[derive(Debug, Deserialize)]
pub struct NewListing {
    pub kind: Kind,
    pub unit_price: u64,
    pub capacity: u64,
}

#[derive(Debug, Deserialize)]
pub struct NewOrder {
    pub listing_id: String,
    pub quantity: u64,
}

// ── the market ───────────────────────────────────────────────────────────

/// `--market-*` settings. Present only when the operator enabled the market.
#[derive(Debug, Clone)]
pub struct MarketOpts {
    /// `market.json`: sellers, listings and orders. 0600.
    pub state_file: String,
    /// The Unix socket of the Stripe gate (`losos-registrar stripe-gate`), the
    /// only way this process reaches Stripe. The key never comes here.
    pub gate_socket: String,
    /// Where Stripe sends a buyer or seller back to after Checkout or
    /// onboarding.
    pub return_url: String,
    /// ISO 4217, lowercase. One currency per edge: mixed-currency listings
    /// cannot be compared.
    pub currency: String,
    pub fee_bps: u32,
    /// `StorageClass` purchased volumes are claimed from.
    pub storage_class: String,
    /// The hardware catalogue (`crate::hardware`), when the edge sells boxes
    /// and gateways. Unset, `/market/hardware*` answers 503.
    pub hardware_catalogue: Option<String>,
}

pub struct Market {
    opts: MarketOpts,
    path: PathBuf,
    state: Mutex<MarketState>,
}

/// A canonical lowercase hyphenated UUID, the shape `losos-ctl` derives the
/// box's id in. Checked here so the Stripe metadata never carries free text.
#[must_use]
pub fn valid_box_uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}

fn random_id(prefix: &str) -> Result<String, MarketError> {
    let mut bytes = [0u8; 12];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| MarketError::Store("no randomness".to_string()))?;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("{prefix}_{hex}"))
}

impl Market {
    /// Open the store. A missing file is an empty market; an unparseable one is
    /// an error — silently starting empty would forget who has paid for what.
    pub async fn open(opts: MarketOpts) -> Result<Self, MarketError> {
        let path = PathBuf::from(&opts.state_file);
        let state = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => MarketState::default(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self {
            opts,
            path,
            state: Mutex::new(state),
        })
    }

    #[must_use]
    pub fn fee_bps(&self) -> u32 {
        self.opts.fee_bps
    }

    pub async fn validate_secrets(&self) -> Result<(), MarketError> {
        self.stripe().validate_secrets().await
    }

    async fn persist(&self, state: &MarketState) -> Result<(), MarketError> {
        let bytes = serde_json::to_vec_pretty(state)?;
        atomic_write(&self.path, &bytes, STORE_FILE_MODE).await?;
        Ok(())
    }

    /// Make `next` the live state, but only once it is on disk. Every write
    /// goes through here, each built on a clone taken under the lock. A change
    /// made to the live state first would outlive a failed write (a full
    /// disk): the caller is told it failed, yet the change stands in memory,
    /// is acted on, and the next successful write commits it silently.
    async fn commit(&self, live: &mut MarketState, next: MarketState) -> Result<(), MarketError> {
        self.persist(&next).await?;
        *live = next;
        Ok(())
    }

    fn stripe(&self) -> GateClient {
        GateClient::new(&self.opts.gate_socket)
    }

    /// The tenant's Stripe connected account as last recorded, if it has one.
    /// Read by [`crate::domains`]: a box's public names hang off it.
    pub async fn seller(&self, id: &str) -> Option<Seller> {
        self.state.lock().await.sellers.get(id).cloned()
    }

    /// Every recorded seller, for the reconciler's zone pass.
    pub async fn sellers(&self) -> BTreeMap<String, Seller> {
        self.state.lock().await.sellers.clone()
    }

    /// Everything one appliance may see about its own market activity.
    pub async fn account(&self, id: &str, sharing: &Sharing) -> AccountView {
        let now = now_secs();
        let state = self.state.lock().await;
        let seller = state.sellers.get(id);
        AccountView {
            fee_bps: self.opts.fee_bps,
            currency: self.opts.currency.clone(),
            seller_onboarded: seller.is_some(),
            seller_ready: seller.is_some_and(|s| s.ready),
            can_sell_storage: sharing.allows(id, Kind::Storage),
            can_sell_compute: sharing.allows(id, Kind::Compute),
            listings: state
                .listings
                .values()
                .filter(|l| l.seller == id)
                .map(|l| OwnListing {
                    id: l.id.clone(),
                    kind: l.kind,
                    unit: l.kind.unit(),
                    unit_price: l.unit_price,
                    capacity: l.capacity,
                    available: available(&state, l, now),
                    active: l.active,
                })
                .collect(),
            entitlements: entitlements(&state, id, now),
            purchases: history(state.orders.values().filter(|o| o.buyer == id), now),
            sales: history(state.orders.values().filter(|o| o.seller == id), now),
        }
    }

    /// Every listing a buyer could order right now. Anonymous.
    pub async fn browse(&self, sharing: &Sharing) -> Vec<PublicListing> {
        let now = now_secs();
        let state = self.state.lock().await;
        state
            .listings
            .values()
            .filter(|l| l.active && state.sellers.get(&l.seller).is_some_and(|s| s.ready))
            .filter(|l| sharing.allows(&l.seller, l.kind))
            .filter_map(|l| {
                let left = available(&state, l, now);
                (left > 0).then(|| PublicListing {
                    id: l.id.clone(),
                    kind: l.kind,
                    unit: l.kind.unit(),
                    unit_price: l.unit_price,
                    currency: self.opts.currency.clone(),
                    available: left,
                })
            })
            .collect()
    }

    /// Start (or resume) Stripe Connect onboarding for `id`.
    ///
    /// `box_uuid`, when the box sent one, is written onto the Stripe account's
    /// metadata so the account can be traced back to the box that owns it. It
    /// is also written onto an account that already exists without it.
    pub async fn onboard(
        &self,
        id: &str,
        box_uuid: Option<&str>,
    ) -> Result<OnboardView, MarketError> {
        if box_uuid.is_some_and(|u| !valid_box_uuid(u)) {
            return Err(MarketError::Invalid("box_uuid is not a canonical UUID"));
        }
        let stripe = self.stripe();
        let existing = self.state.lock().await.sellers.get(id).cloned();
        let account_id = match existing {
            Some(seller) => {
                if let Some(uuid) = box_uuid {
                    if seller.box_uuid.as_deref() != Some(uuid) {
                        stripe.tag_account(&seller.account_id, uuid).await?;
                        self.set_box_uuid(id, uuid).await?;
                    }
                }
                if seller.ready || stripe.account_ready(&seller.account_id).await? {
                    // Refresh a stale flag: the `account.updated` webhook may
                    // have been missed.
                    self.set_ready(id, true).await?;
                    return Ok(OnboardView {
                        ready: true,
                        url: None,
                    });
                }
                seller.account_id
            }
            None => {
                let account_id = stripe.create_account(id, box_uuid).await?;
                let mut state = self.state.lock().await;
                let mut next = state.clone();
                next.sellers.insert(
                    id.to_string(),
                    Seller {
                        account_id: account_id.clone(),
                        ready: false,
                        box_uuid: box_uuid.map(str::to_string),
                    },
                );
                self.commit(&mut state, next).await?;
                account_id
            }
        };
        let url = stripe
            .account_link(&account_id, &self.opts.return_url)
            .await?;
        Ok(OnboardView {
            ready: false,
            url: Some(url),
        })
    }

    async fn set_box_uuid(&self, id: &str, uuid: &str) -> Result<(), MarketError> {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        if let Some(seller) = next.sellers.get_mut(id) {
            seller.box_uuid = Some(uuid.to_string());
            self.commit(&mut state, next).await?;
        }
        Ok(())
    }

    async fn set_ready(&self, id: &str, ready: bool) -> Result<(), MarketError> {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        if let Some(seller) = next.sellers.get_mut(id) {
            if seller.ready != ready {
                seller.ready = ready;
                self.commit(&mut state, next).await?;
            }
        }
        Ok(())
    }

    pub async fn create_listing(
        &self,
        seller: &str,
        new: NewListing,
        sharing: &Sharing,
    ) -> Result<String, MarketError> {
        if !sharing.allows(seller, new.kind) {
            return Err(MarketError::Conflict(match new.kind {
                Kind::Storage => "only an appliance sharing its storage on the mesh can sell it",
                Kind::Compute => "only an appliance sharing its compute on the mesh can sell it",
            }));
        }
        // The price and capacity are validated as an order of one unit and the
        // whole capacity would be, so a listing nobody could ever buy is
        // refused up front.
        if new.capacity == 0 || new.capacity > MAX_CAPACITY {
            return Err(MarketError::Invalid("capacity out of range"));
        }
        if new.unit_price == 0 || new.unit_price > MAX_UNIT_PRICE_MINOR {
            return Err(MarketError::Invalid("unit_price out of range"));
        }
        if new
            .unit_price
            .checked_mul(new.capacity.min(MAX_QUANTITY))
            .is_none_or(|total| total < MIN_CHARGE_MINOR)
        {
            return Err(MarketError::Invalid(
                "listing could never reach the minimum charge",
            ));
        }
        let mut state = self.state.lock().await;
        if !state.sellers.get(seller).is_some_and(|s| s.ready) {
            return Err(MarketError::Conflict(
                "finish Stripe onboarding before listing",
            ));
        }
        let active = state
            .listings
            .values()
            .filter(|l| l.seller == seller && l.active)
            .count();
        if active >= MAX_LISTINGS_PER_SELLER {
            return Err(MarketError::Conflict("too many active listings"));
        }
        let id = random_id("lst")?;
        let mut next = state.clone();
        next.listings.insert(
            id.clone(),
            Listing {
                id: id.clone(),
                seller: seller.to_string(),
                kind: new.kind,
                unit_price: new.unit_price,
                capacity: new.capacity,
                active: true,
                created_at: now_secs(),
            },
        );
        self.commit(&mut state, next).await?;
        Ok(id)
    }

    /// Stop selling. Orders already placed keep their units.
    pub async fn close_listing(&self, seller: &str, listing_id: &str) -> Result<(), MarketError> {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        let listing = next
            .listings
            .get_mut(listing_id)
            .filter(|l| l.seller == seller)
            .ok_or(MarketError::NotFound)?;
        if listing.active {
            listing.active = false;
            // Nothing refers to a listing nobody ordered from, so it goes now
            // rather than lingering as a closed row.
            if !next.orders.values().any(|o| o.listing_id == listing_id) {
                next.listings.remove(listing_id);
            }
            self.commit(&mut state, next).await?;
        }
        Ok(())
    }

    /// Reserve the units, then ask Stripe for a Checkout Session. The
    /// reservation is written first and under the lock, so two buyers racing
    /// for the last units cannot both get a session; a Stripe failure releases
    /// it again.
    /// The hardware catalogue, read per call so the operator's next rebuild
    /// needs no restart.
    ///
    /// # Errors
    /// [`MarketError::Unconfigured`] when the edge sells no hardware or its
    /// catalogue does not load.
    pub fn hardware(&self) -> Result<Catalogue, MarketError> {
        let path = self
            .opts
            .hardware_catalogue
            .as_deref()
            .ok_or(MarketError::Unconfigured)?;
        let catalogue = Catalogue::load(path).map_err(|e| {
            tracing::error!(target: Action::Market.target(), "hardware catalogue: {e}");
            MarketError::Unconfigured
        })?;
        if catalogue.currency != self.opts.currency {
            tracing::error!(
                target: Action::Market.target(),
                "hardware catalogue is in {}, the market in {}",
                catalogue.currency,
                self.opts.currency,
            );
            return Err(MarketError::Unconfigured);
        }
        Ok(catalogue)
    }

    /// Start a Checkout for hardware the platform sells. Nothing is stored:
    /// the session's metadata names the box, and the operator ships from the
    /// Stripe dashboard.
    ///
    /// # Errors
    /// [`MarketError::Invalid`] for a cart the catalogue refuses; the gate's
    /// and Stripe's faults otherwise.
    pub async fn hardware_checkout(
        &self,
        buyer: &str,
        lines: Vec<Line>,
    ) -> Result<HardwareCheckoutView, MarketError> {
        let catalogue = self.hardware()?;
        let amount = total(&catalogue.price(&lines).map_err(MarketError::Invalid)?);
        let order_id = random_id("hw")?;
        let (_, checkout_url) = self
            .stripe()
            .hardware_checkout(HardwareRequest {
                order_id: order_id.clone(),
                appliance_id: buyer.to_string(),
                lines,
                return_url: self.opts.return_url.clone(),
                expires_at: now_secs() + CHECKOUT_TTL_SECS,
            })
            .await?;
        tracing::info!(
            target: Action::Market.target(),
            "hardware checkout {order_id} for {buyer}: {amount} {}",
            catalogue.currency,
        );
        Ok(HardwareCheckoutView {
            order_id,
            checkout_url,
            amount,
            currency: catalogue.currency,
        })
    }

    pub async fn create_order(
        &self,
        buyer: &str,
        new: NewOrder,
        sharing: &Sharing,
    ) -> Result<CheckoutView, MarketError> {
        let stripe = self.stripe();
        let now = now_secs();
        let (order, destination) = {
            let mut state = self.state.lock().await;
            let listing = state
                .listings
                .get(&new.listing_id)
                .filter(|l| l.active)
                .cloned()
                .ok_or(MarketError::NotFound)?;
            let seller = state
                .sellers
                .get(&listing.seller)
                .filter(|s| s.ready)
                .cloned()
                .ok_or(MarketError::NotFound)?;
            // Not for sale while the seller has stopped sharing it.
            if !sharing.allows(&listing.seller, listing.kind) {
                return Err(MarketError::NotFound);
            }
            if listing.kind == Kind::Storage && namespace_for(buyer).is_none() {
                return Err(MarketError::Invalid(
                    "appliance id cannot name a storage namespace",
                ));
            }
            if listing.seller == buyer {
                return Err(MarketError::Invalid("cannot buy your own listing"));
            }
            let q = quote(listing.unit_price, new.quantity, self.opts.fee_bps)?;
            let unpaid = state
                .orders
                .values()
                .filter(|o| o.buyer == buyer && pending_live(o, now))
                .count();
            if unpaid >= MAX_PENDING_PER_BUYER {
                return Err(MarketError::Conflict(
                    "too many unpaid checkouts; pay for one or let it expire first",
                ));
            }
            if new.quantity > available(&state, &listing, now) {
                return Err(MarketError::Conflict("not enough units available"));
            }
            let order = Order {
                id: random_id("ord")?,
                listing_id: listing.id.clone(),
                buyer: buyer.to_string(),
                seller: listing.seller.clone(),
                kind: listing.kind,
                quantity: new.quantity,
                unit_price: listing.unit_price,
                amount: q.amount,
                fee: q.fee,
                currency: self.opts.currency.clone(),
                status: OrderStatus::Pending,
                session_id: None,
                created_at: now,
                paid_at: None,
                expires_at: None,
                volume: None,
                claim: None,
            };
            let mut next = state.clone();
            expire_stale(&mut next, now);
            next.orders.insert(order.id.clone(), order.clone());
            self.commit(&mut state, next).await?;
            (order, seller.account_id)
        };

        let session = stripe
            .checkout(CheckoutRequest {
                order_id: order.id.clone(),
                kind: order.kind,
                quantity: order.quantity,
                unit_price: order.unit_price,
                currency: order.currency.clone(),
                fee: order.fee,
                destination,
                return_url: self.opts.return_url.clone(),
                expires_at: now + CHECKOUT_TTL_SECS,
            })
            .await;
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        match session {
            Ok((session_id, url)) => {
                if let Some(o) = next.orders.get_mut(&order.id) {
                    o.session_id = Some(session_id);
                }
                self.commit(&mut state, next).await?;
                Ok(CheckoutView {
                    order_id: order.id,
                    checkout_url: url,
                    amount: order.amount,
                    fee: order.fee,
                    currency: order.currency,
                })
            }
            Err(e) => {
                if release_failed_checkout(&mut next, &order.id) {
                    self.commit(&mut state, next).await?;
                }
                Err(e)
            }
        }
    }

    /// Persist the release of every pending order whose hold has run out.
    /// Run on each reconcile pass; returns the orders it expired.
    pub async fn expire_stale(&self) -> Result<Vec<String>, MarketError> {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        let (expired, pruned) = expire_stale(&mut next, now_secs());
        if !expired.is_empty() || pruned > 0 {
            self.commit(&mut state, next).await?;
        }
        Ok(expired)
    }

    pub fn storage_class(&self) -> &str {
        &self.opts.storage_class
    }

    /// Storage orders that are paid and live but have no volume yet.
    pub async fn pending_provisions(&self) -> Vec<Provision> {
        pending_provisions(&*self.state.lock().await, now_secs())
    }

    /// Record that `order_id`'s claim is about to be created. Called before
    /// the create, and a failure here stops it, so a claim is never made that
    /// the market does not know about.
    pub async fn mark_claimed(
        &self,
        order_id: &str,
        namespace: &str,
        pvc: &str,
    ) -> Result<(), MarketError> {
        let claim = format!("{namespace}/{pvc}");
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        match next.orders.get_mut(order_id) {
            Some(o) if o.claim.as_deref() != Some(&claim) => o.claim = Some(claim),
            _ => return Ok(()),
        }
        self.commit(&mut state, next).await
    }

    /// Record that `order_id`'s volume is bound.
    pub async fn mark_provisioned(
        &self,
        order_id: &str,
        namespace: &str,
        pvc: &str,
    ) -> Result<(), MarketError> {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        if let Some(o) = next.orders.get_mut(order_id) {
            o.volume = Some(format!("{namespace}/{pvc}"));
            self.commit(&mut state, next).await?;
        }
        Ok(())
    }

    /// Lapsed storage orders whose claim may still exist.
    pub async fn lapsed_volumes(&self) -> Vec<(String, String)> {
        lapsed_volumes(&*self.state.lock().await, now_secs())
    }

    /// Record that `order_id`'s claim is gone, which returns its units to the
    /// listing.
    pub async fn mark_reclaimed(&self, order_id: &str) -> Result<(), MarketError> {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        if let Some(o) = next.orders.get_mut(order_id) {
            o.volume = None;
            o.claim = None;
            self.commit(&mut state, next).await?;
        }
        Ok(())
    }

    /// Handle a Stripe webhook delivery. `signature` is the raw header.
    pub async fn webhook(&self, signature: &str, body: &[u8]) -> Result<(), MarketError> {
        // The gate holds the signing secrets, so it does the check.
        self.stripe().verify_webhook(signature, body).await?;
        let event: Value = serde_json::from_slice(body)
            .map_err(|_| MarketError::Invalid("webhook body is not JSON"))?;
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        if apply_event(&mut next, &event, now_secs()) {
            self.commit(&mut state, next).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(id: &str, status: OrderStatus, qty: u64, created_at: u64) -> Order {
        Order {
            id: id.to_string(),
            listing_id: "lst_1".to_string(),
            buyer: "b".to_string(),
            seller: "s".to_string(),
            kind: Kind::Storage,
            quantity: qty,
            unit_price: 100,
            amount: qty * 100,
            fee: fee_for(qty * 100, DEFAULT_FEE_BPS),
            currency: "eur".to_string(),
            status,
            session_id: Some(format!("cs_{id}")),
            created_at,
            paid_at: None,
            expires_at: None,
            volume: None,
            claim: None,
        }
    }

    fn completed(order: &Order) -> Value {
        serde_json::json!({
            "type": "checkout.session.completed",
            "data": { "object": {
                "id": order.session_id,
                "client_reference_id": order.id,
                "payment_status": "paid",
                "amount_total": order.amount,
                "currency": "EUR",
            }}
        })
    }

    fn paid(id: &str, kind: Kind, qty: u64, buyer: &str, expires_at: Option<u64>) -> Order {
        let mut o = order(id, OrderStatus::Paid, qty, 0);
        o.kind = kind;
        o.buyer = buyer.to_string();
        o.paid_at = Some(0);
        o.expires_at = expires_at;
        o
    }

    fn state_of(orders: Vec<Order>) -> MarketState {
        let mut st = MarketState::default();
        for o in orders {
            st.orders.insert(o.id.clone(), o);
        }
        st
    }

    #[test]
    fn a_payment_starts_a_thirty_day_entitlement() {
        let mut st = state_of(vec![order("ord_a", OrderStatus::Pending, 4, 0)]);
        let o = st.orders["ord_a"].clone();
        assert!(apply_event(&mut st, &completed(&o), 1_000));
        assert_eq!(
            st.orders["ord_a"].expires_at,
            Some(1_000 + ENTITLEMENT_SECS)
        );
    }

    #[test]
    fn delayed_payment_events_do_not_fulfil_orders() {
        let mut st = state_of(vec![order("ord_a", OrderStatus::Pending, 4, 0)]);
        let o = st.orders["ord_a"].clone();
        let mut event = completed(&o);
        event["type"] = "checkout.session.async_payment_succeeded".into();
        assert!(!apply_event(&mut st, &event, 1_000));
        assert_eq!(st.orders["ord_a"].status, OrderStatus::Pending);
    }

    #[test]
    fn entitlements_sum_live_paid_orders_per_buyer() {
        let st = state_of(vec![
            paid("ord_a", Kind::Storage, 5, "b", Some(100)),
            paid("ord_b", Kind::Storage, 7, "b", Some(50)),
            paid("ord_c", Kind::Compute, 3, "b", None),
            paid("ord_d", Kind::Storage, 99, "other", Some(100)),
            order("ord_e", OrderStatus::Pending, 99, 0),
        ]);
        let e = entitlements(&st, "b", 10);
        assert_eq!(e.storage_gib, 12);
        assert_eq!(e.compute_vcpu_hours, 3);
        assert_eq!(e.next_expiry, Some(50));
    }

    #[test]
    fn a_lapsed_order_stops_counting_and_frees_its_units() {
        let st = state_of(vec![paid("ord_a", Kind::Storage, 5, "b", Some(100))]);
        assert_eq!(entitlements(&st, "b", 99).storage_gib, 5);
        assert_eq!(reserved(&st, "lst_1", 99), 5);
        assert_eq!(entitlements(&st, "b", 100), Entitlements::default());
        assert_eq!(reserved(&st, "lst_1", 100), 0);
    }

    #[test]
    fn only_live_paid_storage_orders_await_a_volume() {
        let mut done = paid("ord_done", Kind::Storage, 1, "b", Some(100));
        done.volume = Some("market-b/ord-done".to_string());
        let st = state_of(vec![
            paid("ord_todo", Kind::Storage, 2, "b", Some(100)),
            done,
            paid("ord_late", Kind::Storage, 3, "b", Some(5)),
            paid("ord_cpu", Kind::Compute, 4, "b", Some(100)),
            order("ord_pending", OrderStatus::Pending, 5, 0),
            paid("ord_bad", Kind::Storage, 6, "Bad_Name", Some(100)),
        ]);
        assert_eq!(
            pending_provisions(&st, 10),
            vec![Provision {
                order_id: "ord_todo".to_string(),
                namespace: "market-b".to_string(),
                pvc: "ord-todo".to_string(),
                gib: 2,
            }]
        );
    }

    #[test]
    fn namespaces_are_dns_labels_or_nothing() {
        assert_eq!(
            namespace_for("mattbox-01").as_deref(),
            Some("market-mattbox-01")
        );
        assert_eq!(namespace_for("a.b"), None);
        assert_eq!(namespace_for("A"), None);
        assert_eq!(namespace_for("x-"), None);
        assert_eq!(namespace_for(&"a".repeat(60)), None);
        assert_eq!(pvc_name("ord_ab12"), "ord-ab12");
    }

    #[test]
    fn selling_needs_the_matching_mesh_contribution() {
        use crate::window::ComputeWindow;
        let w = |share| ComputeWindow {
            share_compute: share,
            window_start: "23:00".to_string(),
            window_end: "07:00".to_string(),
            tz: "UTC".to_string(),
        };
        let sharing = Sharing::from_windows(&BTreeMap::from([
            ("on".to_string(), w(true)),
            ("off".to_string(), w(false)),
        ]));
        assert!(sharing.allows("on", Kind::Storage) && sharing.allows("on", Kind::Compute));
        assert!(sharing.allows("off", Kind::Storage) && !sharing.allows("off", Kind::Compute));
        assert!(!sharing.allows("absent", Kind::Storage));

        // The operator's opt-in narrows it: a revoked seller sells nothing.
        let sharing = sharing.only_sellers(|seller| seller == "off");
        assert!(!sharing.allows("on", Kind::Storage) && !sharing.allows("on", Kind::Compute));
        assert!(sharing.allows("off", Kind::Storage));
    }

    #[test]
    fn four_percent_cut_rounds_half_up() {
        assert_eq!(fee_for(1000, 400), 40);
        assert_eq!(fee_for(50, 400), 2);
        assert_eq!(fee_for(112, 400), 4); // 4.48
        assert_eq!(fee_for(113, 400), 5); // 4.52
        assert_eq!(fee_for(0, 400), 0);
    }

    #[test]
    fn fee_never_exceeds_the_charge_and_never_overflows() {
        assert!(fee_for(u64::MAX, 400) < u64::MAX / 20);
        assert!(fee_for(100, 50_000) <= 100);
    }

    #[test]
    fn quote_splits_the_amount_exactly() {
        let q = quote(250, 8, DEFAULT_FEE_BPS).unwrap();
        assert_eq!(q.amount, 2000);
        assert_eq!(q.fee, 80);
        assert_eq!(q.seller_net, 1920);
        assert_eq!(q.fee + q.seller_net, q.amount);
    }

    #[test]
    fn quote_enforces_every_bound() {
        assert!(quote(0, 1, 400).is_err());
        assert!(quote(MAX_UNIT_PRICE_MINOR + 1, 1, 400).is_err());
        assert!(quote(100, 0, 400).is_err());
        assert!(quote(100, MAX_QUANTITY + 1, 400).is_err());
        assert!(quote(10, 4, 400).is_err(), "below the minimum charge");
        assert!(quote(10, 5, 400).is_ok(), "exactly the minimum");
    }

    #[test]
    fn capacity_counts_paid_and_pending_until_terminal_event() {
        let mut state = MarketState::default();
        let now = 1_000_000;
        for o in [
            order("paid", OrderStatus::Paid, 3, 0),
            order("live", OrderStatus::Pending, 2, now - 60),
            // Past its session with no terminal event yet: a `completed` for
            // it may still arrive, so its units stay held.
            order("late", OrderStatus::Pending, 7, now - 40 * 60),
            order(
                "lapsed",
                OrderStatus::Pending,
                4,
                now - CHECKOUT_TTL_SECS - PENDING_HOLD_SECS - 1,
            ),
            order("expired", OrderStatus::Expired, 5, now - 60),
        ] {
            state.orders.insert(o.id.clone(), o);
        }
        assert_eq!(reserved(&state, "lst_1", now), 12);
        assert_eq!(reserved(&state, "other", now), 0);
    }

    #[test]
    fn webhook_signature_roundtrip_and_failures() {
        let secret = "whsec_test";
        let body = br#"{"type":"x"}"#;
        let header = sign_webhook(secret, body, 1_000);
        assert!(verify_signature(secret, &header, body, 1_000).is_ok());
        assert!(verify_signature(secret, &header, body, 1_000 + 299).is_ok());
        assert!(
            verify_signature(secret, &header, body, 1_000 + 301).is_err(),
            "stale timestamp"
        );
        assert!(
            verify_signature(secret, &header, b"{}", 1_000).is_err(),
            "tampered body"
        );
        assert!(
            verify_signature("whsec_other", &header, body, 1_000).is_err(),
            "wrong secret"
        );
        assert!(verify_signature(secret, "", body, 1_000).is_err());
        assert!(verify_signature(secret, "t=1000", body, 1_000).is_err());
        assert!(verify_signature(secret, "t=1000,v1=zz", body, 1_000).is_err());
    }

    #[test]
    fn signature_accepts_any_matching_v1_during_secret_rotation() {
        let secret = "whsec_new";
        let body = b"{}";
        let good = sign_webhook(secret, body, 5);
        let good_v1 = good.split("v1=").nth(1).unwrap();
        let header = format!("t=5,v1=00ff,v1={good_v1}");
        assert!(verify_signature(secret, &header, body, 5).is_ok());
    }

    #[test]
    fn paid_event_marks_the_order_paid_once() {
        let mut state = MarketState::default();
        let o = order("o1", OrderStatus::Pending, 2, 0);
        state.orders.insert(o.id.clone(), o.clone());
        assert!(apply_event(&mut state, &completed(&o), 77));
        assert_eq!(state.orders["o1"].status, OrderStatus::Paid);
        assert_eq!(state.orders["o1"].paid_at, Some(77));
        assert!(
            !apply_event(&mut state, &completed(&o), 99),
            "redelivery is a no-op"
        );
        assert_eq!(state.orders["o1"].paid_at, Some(77));
    }

    #[test]
    fn paid_event_must_match_the_recorded_order() {
        let o = order("o1", OrderStatus::Pending, 2, 0);
        let mut event = completed(&o);

        for (field, value) in [
            ("amount_total", serde_json::json!(1)),
            ("currency", serde_json::json!("usd")),
            ("id", serde_json::json!("cs_forged")),
            ("payment_status", serde_json::json!("unpaid")),
        ] {
            let mut state = MarketState::default();
            state.orders.insert(o.id.clone(), o.clone());
            event["data"]["object"][field] = value;
            assert!(!apply_event(&mut state, &event, 1), "{field}");
            assert_eq!(state.orders["o1"].status, OrderStatus::Pending, "{field}");
            event = completed(&o);
        }
    }

    #[test]
    fn expired_session_releases_only_a_pending_order() {
        let mut state = MarketState::default();
        let pending = order("p", OrderStatus::Pending, 1, 0);
        let paid = order("d", OrderStatus::Paid, 1, 0);
        state.orders.insert("p".into(), pending);
        state.orders.insert("d".into(), paid);
        assert_eq!(reserved(&state, "lst_1", CHECKOUT_TTL_SECS + 900), 2);
        let event = |id: &str| {
            serde_json::json!({
                "type": "checkout.session.expired",
                "data": { "object": { "id": format!("cs_{id}"), "client_reference_id": id } }
            })
        };
        assert!(apply_event(&mut state, &event("p"), 1));
        assert!(!apply_event(&mut state, &event("d"), 1));
        assert_eq!(state.orders["p"].status, OrderStatus::Expired);
        assert_eq!(state.orders["d"].status, OrderStatus::Paid);
        assert_eq!(reserved(&state, "lst_1", CHECKOUT_TTL_SECS + 900), 1);
    }

    #[test]
    fn a_failed_checkout_does_not_expire_an_order_already_paid() {
        let mut state = state_of(vec![
            order("p", OrderStatus::Pending, 1, 0),
            order("d", OrderStatus::Paid, 1, 0),
        ]);
        assert!(release_failed_checkout(&mut state, "p"));
        assert!(!release_failed_checkout(&mut state, "d"));
        assert!(!release_failed_checkout(&mut state, "missing"));
        assert_eq!(state.orders["p"].status, OrderStatus::Expired);
        assert_eq!(state.orders["d"].status, OrderStatus::Paid);
    }

    #[test]
    fn only_the_recorded_session_expiring_releases_an_order() {
        let mut state = MarketState::default();
        state
            .orders
            .insert("p".into(), order("p", OrderStatus::Pending, 1, 0));
        let mut lost = order("lost", OrderStatus::Pending, 1, 0);
        lost.session_id = None;
        state.orders.insert("lost".into(), lost);
        let event = |session: Option<&str>, id: &str| {
            serde_json::json!({
                "type": "checkout.session.expired",
                "data": { "object": { "id": session, "client_reference_id": id } }
            })
        };
        // An older session for the same order, and one with no id at all.
        assert!(!apply_event(&mut state, &event(Some("cs_older"), "p"), 1));
        assert!(!apply_event(&mut state, &event(None, "p"), 1));
        assert_eq!(state.orders["p"].status, OrderStatus::Pending);
        // With nothing recorded there is nothing to bind to, so the sweep
        // decides, not an event.
        assert!(!apply_event(&mut state, &event(Some("cs_any"), "lost"), 1));
        assert!(!apply_event(&mut state, &event(None, "lost"), 1));
        assert_eq!(state.orders["lost"].status, OrderStatus::Pending);
        assert!(apply_event(&mut state, &event(Some("cs_p"), "p"), 1));
        assert_eq!(state.orders["p"].status, OrderStatus::Expired);
    }

    #[test]
    fn a_stale_pending_order_is_recorded_as_expired_and_old_ones_are_dropped() {
        let hold = CHECKOUT_TTL_SECS + PENDING_HOLD_SECS;
        let now = EXPIRED_RETENTION_SECS + hold + 10;
        let mut st = state_of(vec![
            order("fresh", OrderStatus::Pending, 1, now - 60),
            order("stale", OrderStatus::Pending, 2, now - hold),
            order("paid", OrderStatus::Paid, 3, 0),
            order("ancient", OrderStatus::Expired, 4, 0),
        ]);
        let (expired, pruned) = expire_stale(&mut st, now);
        assert_eq!(expired, vec!["stale".to_string()]);
        assert_eq!(pruned, 1);
        assert_eq!(st.orders["stale"].status, OrderStatus::Expired);
        assert_eq!(st.orders["fresh"].status, OrderStatus::Pending);
        assert_eq!(st.orders["paid"].status, OrderStatus::Paid);
        assert!(!st.orders.contains_key("ancient"));
        // A second pass has nothing left to do, so nothing is rewritten.
        assert_eq!(expire_stale(&mut st, now), (vec![], 0));
    }

    fn listing(id: &str, active: bool) -> Listing {
        Listing {
            id: id.to_string(),
            seller: "s".to_string(),
            kind: Kind::Storage,
            unit_price: 100,
            capacity: 10,
            active,
            created_at: 0,
        }
    }

    /// A write that fails leaves the live state as it was, so nothing the
    /// caller was told failed is acted on, or committed by a later write.
    #[tokio::test]
    async fn a_failed_write_changes_nothing_in_memory() {
        let dir = std::env::temp_dir().join(format!("market-commit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut st = state_of(vec![order("p", OrderStatus::Pending, 1, 0)]);
        st.listings.insert("lst_1".into(), listing("lst_1", true));
        st.sellers.insert(
            "s".into(),
            Seller {
                account_id: "acct_1".into(),
                ready: false,
                box_uuid: None,
            },
        );
        let file = dir.join("market.json");
        std::fs::write(&file, serde_json::to_vec(&st).unwrap()).unwrap();
        let market = Market::open(MarketOpts {
            state_file: file.to_string_lossy().into_owned(),
            gate_socket: "/nonexistent".into(),
            return_url: "https://example.test/market".into(),
            currency: "eur".into(),
            fee_bps: DEFAULT_FEE_BPS,
            storage_class: DEFAULT_STORAGE_CLASS.into(),
            hardware_catalogue: None,
        })
        .await
        .unwrap();
        // Every write now fails: the directory it renames into is gone.
        std::fs::remove_dir_all(&dir).unwrap();

        assert!(market.close_listing("s", "lst_1").await.is_err());
        assert!(market.set_ready("s", true).await.is_err());
        assert!(market.mark_claimed("p", "market-b", "p").await.is_err());
        assert!(market.mark_provisioned("p", "market-b", "p").await.is_err());
        assert_eq!(*market.state.lock().await, st);
    }

    #[test]
    fn a_closed_listing_is_kept_only_while_an_order_names_it() {
        let mut st = state_of(vec![order("o", OrderStatus::Paid, 1, 0)]);
        for l in [
            listing("lst_1", false),
            listing("lst_open", true),
            listing("lst_gone", false),
        ] {
            st.listings.insert(l.id.clone(), l);
        }
        assert_eq!(expire_stale(&mut st, 10), (vec![], 1));
        let left: Vec<&str> = st.listings.keys().map(String::as_str).collect();
        assert_eq!(left, ["lst_1", "lst_open"]);
    }

    #[test]
    fn a_lapsed_volume_holds_its_units_until_it_is_reclaimed() {
        let now = 2 * ENTITLEMENT_SECS;
        let mut lapsed = paid("lapsed", Kind::Storage, 3, "b", Some(now - 1));
        lapsed.volume = Some("market-b/lapsed".to_string());
        let unprovisioned = paid("never", Kind::Storage, 4, "b", Some(now - 1));
        // Created but never bound: Longhorn may still be holding its GiB.
        let mut unbound = paid("unbound", Kind::Storage, 7, "b", Some(now - 1));
        unbound.claim = Some("market-b/unbound".to_string());
        let mut live = paid("live", Kind::Storage, 2, "b", Some(now + 1));
        live.volume = Some("market-b/live".to_string());
        let mut st = state_of(vec![lapsed, unprovisioned, unbound, live]);
        assert_eq!(reserved(&st, "lst_1", now), 12);
        assert_eq!(
            lapsed_volumes(&st, now),
            vec![
                ("lapsed".to_string(), "market-b/lapsed".to_string()),
                ("unbound".to_string(), "market-b/unbound".to_string()),
            ]
        );
        st.orders.get_mut("lapsed").unwrap().volume = None;
        st.orders.get_mut("unbound").unwrap().claim = None;
        assert_eq!(reserved(&st, "lst_1", now), 2);
        assert!(lapsed_volumes(&st, now).is_empty());
    }

    #[test]
    fn a_payment_for_an_order_whose_session_id_was_lost_is_still_fulfilled() {
        let mut lost = order("o1", OrderStatus::Pending, 2, 0);
        lost.session_id = None;
        let mut st = state_of(vec![lost.clone()]);
        let mut event = completed(&lost);
        event["data"]["object"]["id"] = "cs_recovered".into();
        assert!(apply_event(&mut st, &event, 10));
        assert_eq!(st.orders["o1"].status, OrderStatus::Paid);
        assert_eq!(st.orders["o1"].session_id.as_deref(), Some("cs_recovered"));
        // Only something shaped like a Checkout Session id is adopted.
        let mut st = state_of(vec![lost.clone()]);
        event["data"]["object"]["id"] = "pi_not_a_session".into();
        assert!(!apply_event(&mut st, &event, 10));
        assert_eq!(st.orders["o1"].status, OrderStatus::Pending);
    }

    #[test]
    fn a_payment_after_the_hold_is_honoured_only_while_the_units_are_free() {
        let listing = Listing {
            capacity: 5,
            ..listing("lst_1", true)
        };
        let late = order("late", OrderStatus::Expired, 3, 0);
        let mut st = state_of(vec![late.clone()]);
        st.listings.insert(listing.id.clone(), listing);
        assert!(apply_event(&mut st, &completed(&late), 10));
        assert_eq!(st.orders["late"].status, OrderStatus::Paid);

        // The same payment when someone else has bought the units since.
        let mut st2 = state_of(vec![late.clone(), order("other", OrderStatus::Paid, 4, 0)]);
        st2.listings = st.listings.clone();
        assert!(!apply_event(&mut st2, &completed(&late), 10));
        assert_eq!(st2.orders["late"].status, OrderStatus::Expired);
    }

    #[test]
    fn the_account_history_is_bounded_and_keeps_live_orders() {
        let now = 10 * ENTITLEMENT_SECS;
        let mut orders: Vec<Order> = (0..ACCOUNT_HISTORY as u64 + 50)
            .map(|i| order(&format!("old{i:03}"), OrderStatus::Expired, 1, i))
            .collect();
        orders.push(paid("live", Kind::Storage, 1, "b", Some(now + 1)));
        let st = state_of(orders);
        let view = history(st.orders.values(), now);
        assert_eq!(view.len(), ACCOUNT_HISTORY);
        assert_eq!(view[0].id, "live");
        // Then newest first.
        assert_eq!(view[1].id, format!("old{:03}", ACCOUNT_HISTORY + 49));
    }

    #[test]
    fn account_updated_flips_readiness_both_ways() {
        let mut state = MarketState::default();
        state.sellers.insert(
            "box".into(),
            Seller {
                account_id: "acct_1".into(),
                ready: false,
                box_uuid: None,
            },
        );
        let event = |ready: bool| {
            serde_json::json!({
                "type": "account.updated",
                "data": { "object": {
                    "id": "acct_1",
                    "details_submitted": ready,
                    "payouts_enabled": ready,
                    "capabilities": { "transfers": if ready { "active" } else { "pending" } },
                }}
            })
        };
        assert!(apply_event(&mut state, &event(true), 1));
        assert!(state.sellers["box"].ready);
        assert!(!apply_event(&mut state, &event(true), 1));
        assert!(apply_event(&mut state, &event(false), 1));
        assert!(!state.sellers["box"].ready);
    }

    #[test]
    fn unknown_events_and_objects_are_ignored() {
        let mut state = MarketState::default();
        let before = state.clone();
        for event in [
            serde_json::json!({ "type": "charge.refunded" }),
            serde_json::json!({ "type": "checkout.session.completed", "data": { "object": {
                "client_reference_id": "nope", "payment_status": "paid" }}}),
            serde_json::json!({ "type": "account.updated", "data": { "object": { "id": "acct_x" }}}),
            serde_json::json!({}),
        ] {
            assert!(!apply_event(&mut state, &event, 1));
        }
        assert_eq!(state, before);
    }
}
