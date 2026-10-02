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
//!     `--market-stripe-key-file` (`losos.edge.market.enable`); otherwise every
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
//! adds the persisted store and [`StripeClient`] adds the one HTTP dependency.
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

use std::collections::BTreeMap;
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

/// Lifetime of a Checkout Session. 30 minutes is Stripe's minimum.
const CHECKOUT_TTL_SECS: u64 = 30 * 60;
/// A pending order keeps its capacity reserved this long past the session's
/// expiry, so a late `completed` webhook can never find the units resold.
const PENDING_GRACE_SECS: u64 = 5 * 60;
/// Expired orders are only history; drop them after a month.
const EXPIRED_RETENTION_SECS: u64 = 30 * 24 * 3600;
/// Stripe's recommended replay window for webhook timestamps.
const WEBHOOK_TOLERANCE_SECS: u64 = 5 * 60;

/// Budget for one Stripe request. The router's guard allows a whole request
/// five seconds, so this must leave room for the store write around it.
const STRIPE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4);

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

    const fn label(self) -> &'static str {
        match self {
            Kind::Storage => "Storage",
            Kind::Compute => "Compute",
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
    /// A Checkout Session exists; capacity is reserved.
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
        && now < order.created_at + CHECKOUT_TTL_SECS + PENDING_GRACE_SECS
}

/// Units of `listing` that are sold or held by a live checkout.
#[must_use]
pub fn reserved(state: &MarketState, listing_id: &str, now: u64) -> u64 {
    state
        .orders
        .values()
        .filter(|o| o.listing_id == listing_id)
        .filter(|o| o.status == OrderStatus::Paid || pending_live(o, now))
        .map(|o| o.quantity)
        .sum()
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

fn decode_hex(s: &str) -> Option<Vec<u8>> {
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
        "checkout.session.completed" | "checkout.session.async_payment_succeeded" => {
            if object["payment_status"].as_str() != Some("paid") {
                return false;
            }
            let Some(order) = object["client_reference_id"]
                .as_str()
                .and_then(|id| state.orders.get_mut(id))
            else {
                return false;
            };
            if order.status != OrderStatus::Pending {
                return false;
            }
            let matches = order.session_id.as_deref() == object["id"].as_str()
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
            order.status = OrderStatus::Paid;
            order.paid_at = Some(now);
            true
        }
        "checkout.session.expired" => {
            let Some(order) = object["client_reference_id"]
                .as_str()
                .and_then(|id| state.orders.get_mut(id))
            else {
                return false;
            };
            if order.status == OrderStatus::Pending {
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
fn account_ready(account: &Value) -> bool {
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
}

impl OrderView {
    fn of(o: &Order) -> Self {
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
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AccountView {
    pub fee_bps: u32,
    pub currency: String,
    pub seller_onboarded: bool,
    pub seller_ready: bool,
    pub listings: Vec<OwnListing>,
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

// ── Stripe ───────────────────────────────────────────────────────────────

/// The few Stripe calls the market makes, over plain form-encoded REST so no
/// SDK has to be vendored.
pub struct StripeClient {
    http: reqwest::Client,
    api: String,
    key: String,
}

impl StripeClient {
    pub fn new(api: &str, key: &str) -> Result<Self, MarketError> {
        let http = reqwest::Client::builder()
            .timeout(STRIPE_TIMEOUT)
            .user_agent(concat!("losos-registrar/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| MarketError::Stripe(format!("build client: {e}")))?;
        Ok(Self {
            http,
            api: api.trim_end_matches('/').to_string(),
            key: key.to_string(),
        })
    }

    async fn send(
        &self,
        request: reqwest::RequestBuilder,
        what: &str,
    ) -> Result<Value, MarketError> {
        let response = request
            .bearer_auth(&self.key)
            .send()
            .await
            .map_err(|e| MarketError::Stripe(format!("{what}: {e}")))?;
        let status = response.status();
        let body: Value = response.json().await.unwrap_or(Value::Null);
        if status.is_success() {
            Ok(body)
        } else {
            let message = body["error"]["message"].as_str().unwrap_or("no detail");
            Err(MarketError::Stripe(format!(
                "{what} -> {status}: {message}"
            )))
        }
    }

    async fn post(
        &self,
        path: &str,
        form: &[(&str, String)],
        idempotency_key: Option<&str>,
    ) -> Result<Value, MarketError> {
        let mut request = self.http.post(format!("{}{path}", self.api)).form(form);
        if let Some(key) = idempotency_key {
            request = request.header("Idempotency-Key", key);
        }
        self.send(request, &format!("POST {path}")).await
    }

    /// Create the seller's Express account. The idempotency key makes a retry
    /// or a racing second request return the same account.
    pub async fn create_account(&self, appliance_id: &str) -> Result<String, MarketError> {
        let body = self
            .post(
                "/v1/accounts",
                &[
                    ("type", "express".to_string()),
                    ("capabilities[transfers][requested]", "true".to_string()),
                    ("metadata[losos_appliance_id]", appliance_id.to_string()),
                ],
                Some(&format!("losos-account-{appliance_id}")),
            )
            .await?;
        body["id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| MarketError::Stripe("account response had no id".to_string()))
    }

    /// Whether an existing account can already receive transfers.
    pub async fn account_ready(&self, account_id: &str) -> Result<bool, MarketError> {
        let path = format!("/v1/accounts/{account_id}");
        let body = self
            .send(
                self.http.get(format!("{}{path}", self.api)),
                &format!("GET {path}"),
            )
            .await?;
        Ok(account_ready(&body))
    }

    /// A one-time Stripe-hosted onboarding URL.
    pub async fn account_link(
        &self,
        account_id: &str,
        return_url: &str,
    ) -> Result<String, MarketError> {
        let body = self
            .post(
                "/v1/account_links",
                &[
                    ("account", account_id.to_string()),
                    ("type", "account_onboarding".to_string()),
                    ("refresh_url", return_url.to_string()),
                    ("return_url", return_url.to_string()),
                ],
                None,
            )
            .await?;
        body["url"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| MarketError::Stripe("account link had no url".to_string()))
    }

    /// A Checkout Session that charges the platform, forwards the amount
    /// minus the fee to `destination`, and keeps `order.fee`.
    pub async fn checkout(
        &self,
        order: &Order,
        destination: &str,
        return_url: &str,
        expires_at: u64,
    ) -> Result<(String, String), MarketError> {
        let sep = if return_url.contains('?') { '&' } else { '?' };
        let form = [
            ("mode", "payment".to_string()),
            ("client_reference_id", order.id.clone()),
            (
                "success_url",
                format!("{return_url}{sep}order={}&status=paid", order.id),
            ),
            (
                "cancel_url",
                format!("{return_url}{sep}order={}&status=cancelled", order.id),
            ),
            ("expires_at", expires_at.to_string()),
            ("metadata[order_id]", order.id.clone()),
            ("line_items[0][quantity]", order.quantity.to_string()),
            (
                "line_items[0][price_data][currency]",
                order.currency.clone(),
            ),
            (
                "line_items[0][price_data][unit_amount]",
                order.unit_price.to_string(),
            ),
            (
                "line_items[0][price_data][product_data][name]",
                format!(
                    "{} ({}) - losos market",
                    order.kind.label(),
                    order.kind.unit()
                ),
            ),
            (
                "payment_intent_data[application_fee_amount]",
                order.fee.to_string(),
            ),
            (
                "payment_intent_data[transfer_data][destination]",
                destination.to_string(),
            ),
            ("payment_intent_data[metadata][order_id]", order.id.clone()),
        ];
        let body = self
            .post(
                "/v1/checkout/sessions",
                &form,
                Some(&format!("losos-checkout-{}", order.id)),
            )
            .await?;
        match (body["id"].as_str(), body["url"].as_str()) {
            (Some(id), Some(url)) => Ok((id.to_string(), url.to_string())),
            _ => Err(MarketError::Stripe(
                "checkout session had no id or url".to_string(),
            )),
        }
    }
}

// ── the market ───────────────────────────────────────────────────────────

/// `--market-*` settings. Present only when the operator enabled the market.
#[derive(Debug, Clone)]
pub struct MarketOpts {
    /// `market.json`: sellers, listings and orders. 0600.
    pub state_file: String,
    /// The platform's Stripe secret key (`sk_...` or a restricted `rk_...`),
    /// read at request time so a rotated file needs no restart.
    pub stripe_key_file: String,
    /// The webhook endpoints' signing secrets (`whsec_...`), one per line.
    pub webhook_secret_file: String,
    /// `https://api.stripe.com`; overridden only by tests.
    pub stripe_api: String,
    /// Where Stripe sends a buyer or seller back to after Checkout or
    /// onboarding.
    pub return_url: String,
    /// ISO 4217, lowercase. One currency per edge: mixed-currency listings
    /// cannot be compared.
    pub currency: String,
    pub fee_bps: u32,
}

pub struct Market {
    opts: MarketOpts,
    path: PathBuf,
    state: Mutex<MarketState>,
}

/// Why a secret file is unusable, or `None`. These are hand-placed under
/// `/var/secrets`, so a wrong file must fail loudly rather than be sent to
/// Stripe, or worse, accepted by the webhook check.
fn secret_fault(secret: &str, prefixes: &[&str]) -> Option<&'static str> {
    if secret.is_empty() {
        Some("empty")
    } else if secret.chars().any(|c| c.is_control() || c.is_whitespace()) {
        Some("carrying whitespace or a control character")
    } else if !prefixes.iter().any(|p| secret.starts_with(p)) {
        Some("not shaped like the expected Stripe secret")
    } else {
        None
    }
}

async fn read_secret(path: &str, prefixes: &[&str]) -> Result<String, MarketError> {
    let raw = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| MarketError::Stripe(format!("read secret {path}: {e}")))?;
    let secret = raw.trim();
    if let Some(fault) = secret_fault(secret, prefixes) {
        tracing::error!(target: Action::Market.target(), "secret file {path} is {fault}");
        return Err(MarketError::Unconfigured);
    }
    Ok(secret.to_string())
}

/// The webhook secrets, one per line. Stripe signs platform events and
/// connected-account events (`account.updated`) with the secrets of two
/// different endpoints, so the edge accepts either.
async fn read_webhook_secrets(path: &str) -> Result<Vec<String>, MarketError> {
    let raw = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| MarketError::Stripe(format!("read secret {path}: {e}")))?;
    let secrets: Vec<String> = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    if secrets.is_empty()
        || secrets
            .iter()
            .any(|s| secret_fault(s, &["whsec_"]).is_some())
    {
        tracing::error!(
            target: Action::Market.target(),
            "webhook secret file {path} is empty or holds something that is not a whsec_ secret",
        );
        return Err(MarketError::Unconfigured);
    }
    Ok(secrets)
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

    async fn persist(&self, state: &MarketState) -> Result<(), MarketError> {
        let bytes = serde_json::to_vec_pretty(state)?;
        atomic_write(&self.path, &bytes, STORE_FILE_MODE).await?;
        Ok(())
    }

    async fn stripe(&self) -> Result<StripeClient, MarketError> {
        let key = read_secret(&self.opts.stripe_key_file, &["sk_", "rk_"]).await?;
        StripeClient::new(&self.opts.stripe_api, &key)
    }

    /// Everything one appliance may see about its own market activity.
    pub async fn account(&self, id: &str) -> AccountView {
        let now = now_secs();
        let state = self.state.lock().await;
        let seller = state.sellers.get(id);
        AccountView {
            fee_bps: self.opts.fee_bps,
            currency: self.opts.currency.clone(),
            seller_onboarded: seller.is_some(),
            seller_ready: seller.is_some_and(|s| s.ready),
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
            purchases: state
                .orders
                .values()
                .filter(|o| o.buyer == id)
                .map(OrderView::of)
                .collect(),
            sales: state
                .orders
                .values()
                .filter(|o| o.seller == id)
                .map(OrderView::of)
                .collect(),
        }
    }

    /// Every listing a buyer could order right now. Anonymous.
    pub async fn browse(&self) -> Vec<PublicListing> {
        let now = now_secs();
        let state = self.state.lock().await;
        state
            .listings
            .values()
            .filter(|l| l.active && state.sellers.get(&l.seller).is_some_and(|s| s.ready))
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
    pub async fn onboard(&self, id: &str) -> Result<OnboardView, MarketError> {
        let stripe = self.stripe().await?;
        let existing = self.state.lock().await.sellers.get(id).cloned();
        let account_id = match existing {
            Some(seller) => {
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
                let account_id = stripe.create_account(id).await?;
                let mut state = self.state.lock().await;
                state.sellers.insert(
                    id.to_string(),
                    Seller {
                        account_id: account_id.clone(),
                        ready: false,
                    },
                );
                self.persist(&state).await?;
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

    async fn set_ready(&self, id: &str, ready: bool) -> Result<(), MarketError> {
        let mut state = self.state.lock().await;
        if let Some(seller) = state.sellers.get_mut(id) {
            if seller.ready != ready {
                seller.ready = ready;
                self.persist(&state).await?;
            }
        }
        Ok(())
    }

    pub async fn create_listing(
        &self,
        seller: &str,
        new: NewListing,
    ) -> Result<String, MarketError> {
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
        state.listings.insert(
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
        self.persist(&state).await?;
        Ok(id)
    }

    /// Stop selling. Orders already placed keep their units.
    pub async fn close_listing(&self, seller: &str, listing_id: &str) -> Result<(), MarketError> {
        let mut state = self.state.lock().await;
        let listing = state
            .listings
            .get_mut(listing_id)
            .filter(|l| l.seller == seller)
            .ok_or(MarketError::NotFound)?;
        if listing.active {
            listing.active = false;
            self.persist(&state).await?;
        }
        Ok(())
    }

    /// Reserve the units, then ask Stripe for a Checkout Session. The
    /// reservation is written first and under the lock, so two buyers racing
    /// for the last units cannot both get a session; a Stripe failure releases
    /// it again.
    pub async fn create_order(
        &self,
        buyer: &str,
        new: NewOrder,
    ) -> Result<CheckoutView, MarketError> {
        let stripe = self.stripe().await?;
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
            if listing.seller == buyer {
                return Err(MarketError::Invalid("cannot buy your own listing"));
            }
            let q = quote(listing.unit_price, new.quantity, self.opts.fee_bps)?;
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
            };
            state.orders.retain(|_, o| {
                !(o.status == OrderStatus::Expired && now > o.created_at + EXPIRED_RETENTION_SECS)
            });
            state.orders.insert(order.id.clone(), order.clone());
            self.persist(&state).await?;
            (order, seller.account_id)
        };

        let session = stripe
            .checkout(
                &order,
                &destination,
                &self.opts.return_url,
                now + CHECKOUT_TTL_SECS,
            )
            .await;
        let mut state = self.state.lock().await;
        match session {
            Ok((session_id, url)) => {
                if let Some(o) = state.orders.get_mut(&order.id) {
                    o.session_id = Some(session_id);
                }
                self.persist(&state).await?;
                Ok(CheckoutView {
                    order_id: order.id,
                    checkout_url: url,
                    amount: order.amount,
                    fee: order.fee,
                    currency: order.currency,
                })
            }
            Err(e) => {
                if let Some(o) = state.orders.get_mut(&order.id) {
                    o.status = OrderStatus::Expired;
                }
                self.persist(&state).await?;
                Err(e)
            }
        }
    }

    /// Handle a Stripe webhook delivery. `signature` is the raw header.
    pub async fn webhook(&self, signature: &str, body: &[u8]) -> Result<(), MarketError> {
        let secrets = read_webhook_secrets(&self.opts.webhook_secret_file).await?;
        let now = now_secs();
        if !secrets
            .iter()
            .any(|secret| verify_signature(secret, signature, body, now).is_ok())
        {
            return Err(MarketError::BadSignature);
        }
        let event: Value = serde_json::from_slice(body)
            .map_err(|_| MarketError::Invalid("webhook body is not JSON"))?;
        let mut state = self.state.lock().await;
        if apply_event(&mut state, &event, now_secs()) {
            self.persist(&state).await?;
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
    fn capacity_counts_paid_and_live_pending_only() {
        let mut state = MarketState::default();
        let now = 10_000;
        for o in [
            order("paid", OrderStatus::Paid, 3, 0),
            order("live", OrderStatus::Pending, 2, now - 60),
            order(
                "lapsed",
                OrderStatus::Pending,
                4,
                now - CHECKOUT_TTL_SECS - PENDING_GRACE_SECS - 1,
            ),
            order("expired", OrderStatus::Expired, 5, now - 60),
        ] {
            state.orders.insert(o.id.clone(), o);
        }
        assert_eq!(reserved(&state, "lst_1", now), 5);
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
        let event = |id: &str| {
            serde_json::json!({
                "type": "checkout.session.expired",
                "data": { "object": { "client_reference_id": id } }
            })
        };
        assert!(apply_event(&mut state, &event("p"), 1));
        assert!(!apply_event(&mut state, &event("d"), 1));
        assert_eq!(state.orders["p"].status, OrderStatus::Expired);
        assert_eq!(state.orders["d"].status, OrderStatus::Paid);
    }

    #[test]
    fn account_updated_flips_readiness_both_ways() {
        let mut state = MarketState::default();
        state.sellers.insert(
            "box".into(),
            Seller {
                account_id: "acct_1".into(),
                ready: false,
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

    #[test]
    fn secret_files_are_shape_checked() {
        assert_eq!(secret_fault("", &["sk_"]), Some("empty"));
        assert!(secret_fault("sk_live_x", &["sk_", "rk_"]).is_none());
        assert!(secret_fault("pk_live_x", &["sk_", "rk_"]).is_some());
        assert!(secret_fault("sk_a b", &["sk_"]).is_some());
    }
}
