//! Stripe Connect behind one interface.
//!
//! The market is a Connect platform. A seller gets an Express account and
//! finishes Stripe's hosted onboarding; a buyer pays through a *destination
//! charge*, where the platform charges the card, keeps its fee and forwards
//! the rest to the seller's account. [`Connect`] is that list of operations
//! and nothing else, so the market's flow is written once and runs against
//! either platform:
//!
//!   * [`GateClient`], the real one, which asks the Stripe gate
//!     (`crate::stripe_gate`), the only process that holds the key;
//!   * [`MemoryConnect`], an in-memory platform that keeps the same rules
//!     (one account per tenant, transfers only to a ready account, one
//!     session per order, signed webhooks in its own mode), so the whole
//!     seller and buyer lifecycle is tested without a socket or Stripe.
//!
//! How a charge splits is a value, [`Split`], built once from the rate. A
//! [`DestinationCharge`] cannot be made with a fee that does not add up, a
//! fee that takes the whole charge, or a destination that is not an account
//! id, so every caller hands a platform the same well-formed request. The
//! gate still checks every field again: it does not trust the registrar.
//!
//! Plain charges the platform makes for itself (hardware, builder credits)
//! pay no seller and are not Connect; they stay on the gate client.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

use serde_json::{json, Value};

use crate::market::{fee_for, now_secs, sign_webhook, verify_signature, Kind, MarketError};
use crate::stripe_gate::{CheckoutRequest, GateClient};
use crate::stripe_mode::StripeMode;

/// What every [`Connect`] operation returns: a boxed future, so the market can
/// hold any platform as `Arc<dyn Connect>`.
pub type ConnectFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, MarketError>> + Send + 'a>>;

/// Whether `id` has the shape of a Stripe connected account id. The gate
/// applies the same check to every destination it is asked to pay.
#[must_use]
pub fn account_id_ok(id: &str) -> bool {
    id.len() <= 64
        && id.strip_prefix("acct_").is_some_and(|rest| {
            !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
}

// ── the split ────────────────────────────────────────────────────────────

/// How one charge divides between the platform and the seller, in minor
/// units. `fee + net == gross` always holds, and the seller always keeps
/// something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Split {
    gross: u64,
    fee: u64,
}

impl Split {
    /// The platform's cut of `gross` at `bps` basis points, rounded half up
    /// to the minor unit (the rounding [`fee_for`] has always used).
    ///
    /// # Errors
    /// [`MarketError::Invalid`] when `gross` is zero, `bps` is over 100%, or
    /// the fee would leave the seller nothing.
    pub fn at_bps(gross: u64, bps: u32) -> Result<Self, MarketError> {
        if bps > 10_000 {
            return Err(MarketError::Invalid("fee rate is over 100%"));
        }
        Self::new(gross, fee_for(gross, bps))
    }

    /// A split with an explicit fee.
    ///
    /// # Errors
    /// [`MarketError::Invalid`] when `gross` is zero or `fee` is not below it.
    pub fn new(gross: u64, fee: u64) -> Result<Self, MarketError> {
        if gross == 0 {
            return Err(MarketError::Invalid("empty charge"));
        }
        if fee >= gross {
            return Err(MarketError::Invalid("the fee leaves the seller nothing"));
        }
        Ok(Self { gross, fee })
    }

    #[must_use]
    pub const fn gross(&self) -> u64 {
        self.gross
    }

    /// What the platform keeps.
    #[must_use]
    pub const fn fee(&self) -> u64 {
        self.fee
    }

    /// What reaches the seller's account.
    #[must_use]
    pub const fn net(&self) -> u64 {
        self.gross - self.fee
    }
}

// ── the requests ─────────────────────────────────────────────────────────

/// A destination charge: `quantity` units at `unit_price`, the split's fee
/// kept by the platform and the rest sent to `destination`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestinationCharge {
    order_id: String,
    kind: Kind,
    quantity: u64,
    unit_price: u64,
    currency: String,
    split: Split,
    destination: String,
    return_url: String,
    expires_at: u64,
}

/// Everything a [`DestinationCharge`] is made from.
#[derive(Debug, Clone)]
pub struct ChargeSpec<'a> {
    pub order_id: &'a str,
    pub kind: Kind,
    pub quantity: u64,
    pub unit_price: u64,
    pub currency: &'a str,
    pub split: Split,
    pub destination: &'a str,
    pub return_url: &'a str,
    pub expires_at: u64,
}

impl DestinationCharge {
    /// # Errors
    /// [`MarketError::Invalid`] when the split's gross is not `quantity`
    /// times `unit_price`, or `destination` is not an account id.
    pub fn new(spec: ChargeSpec<'_>) -> Result<Self, MarketError> {
        if spec.order_id.is_empty() {
            return Err(MarketError::Invalid("charge has no order id"));
        }
        let gross = spec
            .quantity
            .checked_mul(spec.unit_price)
            .ok_or(MarketError::Invalid("amount overflows"))?;
        if gross != spec.split.gross() {
            return Err(MarketError::Invalid(
                "the split does not match the line items",
            ));
        }
        if !account_id_ok(spec.destination) {
            return Err(MarketError::Invalid(
                "destination is not a Stripe account id",
            ));
        }
        Ok(Self {
            order_id: spec.order_id.to_string(),
            kind: spec.kind,
            quantity: spec.quantity,
            unit_price: spec.unit_price,
            currency: spec.currency.to_string(),
            split: spec.split,
            destination: spec.destination.to_string(),
            return_url: spec.return_url.to_string(),
            expires_at: spec.expires_at,
        })
    }

    #[must_use]
    pub fn order_id(&self) -> &str {
        &self.order_id
    }

    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    #[must_use]
    pub const fn split(&self) -> Split {
        self.split
    }

    #[must_use]
    pub fn destination(&self) -> &str {
        &self.destination
    }

    #[must_use]
    pub fn currency(&self) -> &str {
        &self.currency
    }

    /// The gate's wire form.
    pub(crate) fn to_request(&self) -> CheckoutRequest {
        CheckoutRequest {
            order_id: self.order_id.clone(),
            kind: self.kind,
            quantity: self.quantity,
            unit_price: self.unit_price,
            currency: self.currency.clone(),
            fee: self.split.fee(),
            destination: self.destination.clone(),
            return_url: self.return_url.clone(),
            expires_at: self.expires_at,
        }
    }
}

/// A hosted Checkout Session for a destination charge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    pub url: String,
    /// The mode of the key the session was made with.
    pub mode: StripeMode,
}

// ── the interface ────────────────────────────────────────────────────────

/// The Stripe Connect operations the market uses.
pub trait Connect: Send + Sync {
    /// The mode of the platform's key, after checking the key and the webhook
    /// secrets are in place. [`MarketError::Unconfigured`] when they are not.
    fn mode(&self) -> ConnectFuture<'_, StripeMode>;

    /// Create `tenant`'s Express account, or return the one already made for
    /// it, and the mode it lives in.
    fn create_account<'a>(
        &'a self,
        tenant: &'a str,
        box_uuid: Option<&'a str>,
    ) -> ConnectFuture<'a, (String, StripeMode)>;

    /// Write the box's UUID onto an existing account.
    fn tag_account<'a>(&'a self, account: &'a str, box_uuid: &'a str) -> ConnectFuture<'a, ()>;

    /// Whether the account can receive transfers.
    fn account_ready<'a>(&'a self, account: &'a str) -> ConnectFuture<'a, bool>;

    /// A one-time URL to Stripe's hosted onboarding for the account.
    fn onboarding_link<'a>(
        &'a self,
        account: &'a str,
        return_url: &'a str,
    ) -> ConnectFuture<'a, String>;

    /// A Checkout Session for the charge. Asking twice for one order returns
    /// the same session.
    fn destination_charge<'a>(
        &'a self,
        charge: &'a DestinationCharge,
    ) -> ConnectFuture<'a, Session>;

    /// Check a webhook delivery's `Stripe-Signature` header and that the
    /// event is in the platform's mode; returns that mode.
    fn verify_webhook<'a>(
        &'a self,
        signature: &'a str,
        body: &'a [u8],
    ) -> ConnectFuture<'a, StripeMode>;
}

impl Connect for GateClient {
    fn mode(&self) -> ConnectFuture<'_, StripeMode> {
        Box::pin(self.key_mode())
    }

    fn create_account<'a>(
        &'a self,
        tenant: &'a str,
        box_uuid: Option<&'a str>,
    ) -> ConnectFuture<'a, (String, StripeMode)> {
        Box::pin(GateClient::create_account(self, tenant, box_uuid))
    }

    fn tag_account<'a>(&'a self, account: &'a str, box_uuid: &'a str) -> ConnectFuture<'a, ()> {
        Box::pin(GateClient::tag_account(self, account, box_uuid))
    }

    fn account_ready<'a>(&'a self, account: &'a str) -> ConnectFuture<'a, bool> {
        Box::pin(GateClient::account_ready(self, account))
    }

    fn onboarding_link<'a>(
        &'a self,
        account: &'a str,
        return_url: &'a str,
    ) -> ConnectFuture<'a, String> {
        Box::pin(self.account_link(account, return_url))
    }

    fn destination_charge<'a>(
        &'a self,
        charge: &'a DestinationCharge,
    ) -> ConnectFuture<'a, Session> {
        Box::pin(async move {
            let (id, url, mode) = self.checkout(charge.to_request()).await?;
            Ok(Session { id, url, mode })
        })
    }

    fn verify_webhook<'a>(
        &'a self,
        signature: &'a str,
        body: &'a [u8],
    ) -> ConnectFuture<'a, StripeMode> {
        Box::pin(GateClient::verify_webhook(self, signature, body))
    }
}

// ── an edge without card payments ────────────────────────────────────────

/// The platform of an edge that takes coins only (`--market-no-cards`): no
/// Stripe key, no gate. The mode is the chains', so the ledger it reads is
/// the one those chains settle into. Everything Stripe would answer is
/// [`MarketError::Unconfigured`], which reaches the box as 503.
#[derive(Debug, Clone, Copy)]
pub struct NoCards {
    pub mode: StripeMode,
}

fn no_cards<'a, T: Send + 'a>() -> ConnectFuture<'a, T> {
    Box::pin(async { Err(MarketError::Unconfigured) })
}

impl Connect for NoCards {
    fn mode(&self) -> ConnectFuture<'_, StripeMode> {
        let mode = self.mode;
        Box::pin(async move { Ok(mode) })
    }

    fn create_account<'a>(
        &'a self,
        _tenant: &'a str,
        _box_uuid: Option<&'a str>,
    ) -> ConnectFuture<'a, (String, StripeMode)> {
        no_cards()
    }

    fn tag_account<'a>(&'a self, _account: &'a str, _box_uuid: &'a str) -> ConnectFuture<'a, ()> {
        no_cards()
    }

    fn account_ready<'a>(&'a self, _account: &'a str) -> ConnectFuture<'a, bool> {
        no_cards()
    }

    fn onboarding_link<'a>(
        &'a self,
        _account: &'a str,
        _return_url: &'a str,
    ) -> ConnectFuture<'a, String> {
        no_cards()
    }

    fn destination_charge<'a>(
        &'a self,
        _charge: &'a DestinationCharge,
    ) -> ConnectFuture<'a, Session> {
        no_cards()
    }

    fn verify_webhook<'a>(
        &'a self,
        _signature: &'a str,
        _body: &'a [u8],
    ) -> ConnectFuture<'a, StripeMode> {
        no_cards()
    }
}

// ── the in-memory platform ───────────────────────────────────────────────

/// The webhook signing secret of every [`MemoryConnect`].
pub const MEMORY_WEBHOOK_SECRET: &str = "whsec_memory_connect";

/// One connected account as the in-memory platform keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryAccount {
    pub tenant: String,
    pub box_uuid: Option<String>,
    pub ready: bool,
}

#[derive(Debug, Default)]
struct Memory {
    configured: bool,
    accounts: BTreeMap<String, MemoryAccount>,
    by_tenant: BTreeMap<String, String>,
    sessions: BTreeMap<String, (Session, DestinationCharge)>,
    links: u64,
    events: u64,
}

/// A Connect platform in memory, with Stripe's rules where the market relies
/// on them. Built for tests and demos: it moves no money.
#[derive(Debug)]
pub struct MemoryConnect {
    mode: StripeMode,
    inner: Mutex<Memory>,
}

impl MemoryConnect {
    #[must_use]
    pub fn new(mode: StripeMode) -> Self {
        Self {
            mode,
            inner: Mutex::new(Memory {
                configured: true,
                ..Memory::default()
            }),
        }
    }

    fn with<T>(&self, f: impl FnOnce(&mut Memory) -> T) -> T {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut guard)
    }

    /// Take the key away (or give it back): every operation then answers
    /// [`MarketError::Unconfigured`], as the gate does with no key sealed.
    pub fn set_configured(&self, configured: bool) {
        self.with(|m| m.configured = configured);
    }

    /// Finish (or undo) the seller's onboarding, as Stripe does once the
    /// seller has filled in the hosted form.
    ///
    /// # Errors
    /// [`MarketError::Stripe`] for an account the platform never made.
    pub fn set_ready(&self, account: &str, ready: bool) -> Result<(), MarketError> {
        self.with(|m| {
            m.accounts
                .get_mut(account)
                .map(|a| a.ready = ready)
                .ok_or_else(|| no_such_account(account))
        })
    }

    #[must_use]
    pub fn account(&self, id: &str) -> Option<MemoryAccount> {
        self.with(|m| m.accounts.get(id).cloned())
    }

    /// The account made for `tenant`, if any.
    #[must_use]
    pub fn account_of(&self, tenant: &str) -> Option<String> {
        self.with(|m| m.by_tenant.get(tenant).cloned())
    }

    /// Every charge asked for, by session id.
    #[must_use]
    pub fn charges(&self) -> Vec<DestinationCharge> {
        self.with(|m| m.sessions.values().map(|(_, c)| c.clone()).collect())
    }

    /// The session made for `order_id`, if any.
    #[must_use]
    pub fn session_of(&self, order_id: &str) -> Option<String> {
        self.with(|m| {
            m.sessions
                .iter()
                .find(|(_, (_, c))| c.order_id == order_id)
                .map(|(id, _)| id.clone())
        })
    }

    /// How many onboarding links were handed out.
    #[must_use]
    pub fn links(&self) -> u64 {
        self.with(|m| m.links)
    }

    /// An `account.updated` event for `account` as it stands now, signed:
    /// the body and its `Stripe-Signature` header.
    ///
    /// # Errors
    /// [`MarketError::Stripe`] for an account the platform never made.
    pub fn account_updated(&self, account: &str) -> Result<(Vec<u8>, String), MarketError> {
        self.account_updated_at(account, now_secs())
    }

    /// As [`MemoryConnect::account_updated`], with the event created at
    /// `created`: a late delivery of an older state.
    ///
    /// # Errors
    /// [`MarketError::Stripe`] for an account the platform never made.
    pub fn account_updated_at(
        &self,
        account: &str,
        created: u64,
    ) -> Result<(Vec<u8>, String), MarketError> {
        let a = self
            .account(account)
            .ok_or_else(|| no_such_account(account))?;
        let status = if a.ready { "active" } else { "inactive" };
        Ok(self.signed(&json!({
            "id": self.next_event_id(),
            "created": created,
            "type": "account.updated",
            "livemode": self.mode == StripeMode::Live,
            "data": { "object": {
                "id": account,
                "details_submitted": a.ready,
                "payouts_enabled": a.ready,
                "capabilities": { "transfers": status },
            }},
        })))
    }

    /// A `checkout.session.completed` event for a session this platform
    /// made, paid in full, signed.
    ///
    /// # Errors
    /// [`MarketError::Stripe`] for a session it never made.
    pub fn completed(&self, session_id: &str) -> Result<(Vec<u8>, String), MarketError> {
        self.session_event(session_id, "checkout.session.completed")
    }

    /// A `checkout.session.expired` event, signed.
    ///
    /// # Errors
    /// [`MarketError::Stripe`] for a session it never made.
    pub fn expired(&self, session_id: &str) -> Result<(Vec<u8>, String), MarketError> {
        self.session_event(session_id, "checkout.session.expired")
    }

    fn session_event(
        &self,
        session_id: &str,
        kind: &str,
    ) -> Result<(Vec<u8>, String), MarketError> {
        let (session, charge) = self
            .with(|m| m.sessions.get(session_id).cloned())
            .ok_or_else(|| {
                MarketError::Stripe(format!("No such checkout.session: {session_id}"))
            })?;
        let paid = kind == "checkout.session.completed";
        Ok(self.signed(&json!({
            "id": self.next_event_id(),
            "created": now_secs(),
            "type": kind,
            "livemode": session.mode == StripeMode::Live,
            "data": { "object": {
                "id": session.id,
                "client_reference_id": charge.order_id,
                "payment_status": if paid { "paid" } else { "unpaid" },
                "amount_total": charge.split.gross(),
                "currency": charge.currency,
                "metadata": { "order_id": charge.order_id },
            }},
        })))
    }

    fn next_event_id(&self) -> String {
        let n = self.with(|m| {
            m.events += 1;
            m.events
        });
        format!("evt_memory_{n}")
    }

    fn signed(&self, event: &Value) -> (Vec<u8>, String) {
        let body = serde_json::to_vec(event).unwrap_or_default();
        let header = sign_webhook(MEMORY_WEBHOOK_SECRET, &body, now_secs());
        (body, header)
    }

    fn configured(&self) -> Result<(), MarketError> {
        if self.with(|m| m.configured) {
            Ok(())
        } else {
            Err(MarketError::Unconfigured)
        }
    }
}

fn no_such_account(id: &str) -> MarketError {
    MarketError::Stripe(format!("No such account: '{id}'"))
}

/// A platform-made id: `prefix` and sixteen hex digits of the SHA-256 of
/// `seed`, so the same seed always gets the same id.
fn memory_id(prefix: &str, seed: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, seed.as_bytes());
    let hex: String = digest.as_ref()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("{prefix}{hex}")
}

impl Connect for MemoryConnect {
    fn mode(&self) -> ConnectFuture<'_, StripeMode> {
        Box::pin(async move {
            self.configured()?;
            Ok(self.mode)
        })
    }

    fn create_account<'a>(
        &'a self,
        tenant: &'a str,
        box_uuid: Option<&'a str>,
    ) -> ConnectFuture<'a, (String, StripeMode)> {
        Box::pin(async move {
            self.configured()?;
            // Stripe's idempotency key on account creation is per tenant, so
            // a retry or a racing second request gets the same account.
            let id = self.with(|m| {
                if let Some(id) = m.by_tenant.get(tenant) {
                    return id.clone();
                }
                let id = memory_id("acct_", tenant);
                m.accounts.insert(
                    id.clone(),
                    MemoryAccount {
                        tenant: tenant.to_string(),
                        box_uuid: box_uuid.map(str::to_string),
                        ready: false,
                    },
                );
                m.by_tenant.insert(tenant.to_string(), id.clone());
                id
            });
            Ok((id, self.mode))
        })
    }

    fn tag_account<'a>(&'a self, account: &'a str, box_uuid: &'a str) -> ConnectFuture<'a, ()> {
        Box::pin(async move {
            self.configured()?;
            self.with(|m| {
                m.accounts
                    .get_mut(account)
                    .map(|a| a.box_uuid = Some(box_uuid.to_string()))
                    .ok_or_else(|| no_such_account(account))
            })
        })
    }

    fn account_ready<'a>(&'a self, account: &'a str) -> ConnectFuture<'a, bool> {
        Box::pin(async move {
            self.configured()?;
            self.account(account)
                .map(|a| a.ready)
                .ok_or_else(|| no_such_account(account))
        })
    }

    fn onboarding_link<'a>(
        &'a self,
        account: &'a str,
        return_url: &'a str,
    ) -> ConnectFuture<'a, String> {
        Box::pin(async move {
            self.configured()?;
            if self.account(account).is_none() {
                return Err(no_such_account(account));
            }
            let n = self.with(|m| {
                m.links += 1;
                m.links
            });
            Ok(format!(
                "https://connect.stripe.com/setup/e/{account}/{n}?return={return_url}"
            ))
        })
    }

    fn destination_charge<'a>(
        &'a self,
        charge: &'a DestinationCharge,
    ) -> ConnectFuture<'a, Session> {
        Box::pin(async move {
            self.configured()?;
            match self.account(charge.destination()) {
                None => return Err(no_such_account(charge.destination())),
                // Stripe refuses a transfer to an account whose `transfers`
                // capability is not active.
                Some(a) if !a.ready => {
                    return Err(MarketError::Stripe(format!(
                        "{} cannot receive transfers yet",
                        charge.destination()
                    )))
                }
                Some(_) => {}
            }
            let session = self.with(|m| {
                // The order id is the idempotency key: one session per order.
                if let Some((s, _)) = m
                    .sessions
                    .values()
                    .find(|(_, c)| c.order_id == charge.order_id)
                {
                    return s.clone();
                }
                let prefix = match self.mode {
                    StripeMode::Test => "cs_test_",
                    StripeMode::Live => "cs_live_",
                };
                let id = memory_id(prefix, charge.order_id());
                let session = Session {
                    url: format!("https://checkout.stripe.com/c/pay/{id}"),
                    id: id.clone(),
                    mode: self.mode,
                };
                m.sessions.insert(id, (session.clone(), charge.clone()));
                session
            });
            Ok(session)
        })
    }

    fn verify_webhook<'a>(
        &'a self,
        signature: &'a str,
        body: &'a [u8],
    ) -> ConnectFuture<'a, StripeMode> {
        Box::pin(async move {
            self.configured()?;
            verify_signature(MEMORY_WEBHOOK_SECRET, signature, body, now_secs())?;
            let event: Value = serde_json::from_slice(body)
                .map_err(|_| MarketError::Invalid("webhook body is not JSON"))?;
            if let Some(live) = event["livemode"].as_bool() {
                if live != (self.mode == StripeMode::Live) {
                    return Err(MarketError::Stripe(format!(
                        "gate refused: a {} event reached a {} key",
                        if live { "live" } else { "test" },
                        self.mode
                    )));
                }
            }
            Ok(self.mode)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_000_000;

    fn spec(split: Split, destination: &str) -> ChargeSpec<'_> {
        ChargeSpec {
            order_id: "ord_1",
            kind: Kind::Storage,
            quantity: 8,
            unit_price: 250,
            currency: "eur",
            split,
            destination,
            return_url: "https://box.example/settings/market",
            expires_at: NOW + 31 * 60,
        }
    }

    fn block<T>(f: ConnectFuture<'_, T>) -> Result<T, MarketError> {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime")
            .block_on(f)
    }

    #[test]
    fn a_split_always_adds_up() {
        for gross in [50, 51, 99, 2_000, 1_000_001] {
            for bps in [0, 1, 400, 2_000, 5_000] {
                let s = Split::at_bps(gross, bps).unwrap();
                assert_eq!(s.fee() + s.net(), s.gross());
                assert_eq!(s.fee(), fee_for(gross, bps));
                assert!(s.net() > 0);
            }
        }
    }

    #[test]
    fn the_market_rate_takes_four_percent_and_a_machine_half() {
        let s = Split::at_bps(2_000, 400).unwrap();
        assert_eq!((s.fee(), s.net()), (80, 1_920));
        let vm = Split::at_bps(1_001, crate::vms::VM_FEE_BPS).unwrap();
        assert_eq!(vm.fee() + vm.net(), 1_001);
        assert!(vm.fee().abs_diff(vm.net()) <= 1);
    }

    #[test]
    fn a_split_that_leaves_the_seller_nothing_is_refused() {
        assert!(Split::at_bps(0, 400).is_err());
        assert!(Split::at_bps(100, 10_000).is_err());
        assert!(Split::at_bps(100, 10_001).is_err());
        assert!(Split::new(100, 100).is_err());
        // 99.99% of 50 rounds up to all of it.
        assert!(Split::at_bps(50, 9_999).is_err());
        assert!(Split::new(100, 99).is_ok());
    }

    #[test]
    fn a_charge_must_match_its_lines_and_name_an_account() {
        let good = Split::at_bps(2_000, 400).unwrap();
        assert!(DestinationCharge::new(spec(good, "acct_1Seller")).is_ok());
        let off = Split::at_bps(1_999, 400).unwrap();
        assert!(DestinationCharge::new(spec(off, "acct_1Seller")).is_err());
        for bad in ["", "acct_", "cus_123", "acct_x/../y", &"acct_x".repeat(20)] {
            assert!(
                DestinationCharge::new(spec(good, bad)).is_err(),
                "{bad:?} passed as a destination"
            );
        }
    }

    #[test]
    fn the_gate_receives_the_splits_fee() {
        let charge =
            DestinationCharge::new(spec(Split::at_bps(2_000, 400).unwrap(), "acct_1Seller"))
                .unwrap();
        let wire = charge.to_request();
        assert_eq!(wire.fee, 80);
        assert_eq!(wire.destination, "acct_1Seller");
        assert_eq!(wire.quantity * wire.unit_price, 2_000);
    }

    #[test]
    fn the_memory_platform_makes_one_account_per_tenant() {
        let p = MemoryConnect::new(StripeMode::Test);
        let (a, mode) = block(p.create_account("seller", None)).unwrap();
        let (b, _) = block(p.create_account("seller", Some("u"))).unwrap();
        let (c, _) = block(p.create_account("other", None)).unwrap();
        assert_eq!(mode, StripeMode::Test);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(account_id_ok(&a));
        assert!(!block(p.account_ready(&a)).unwrap());
        assert!(block(p.account_ready("acct_nobody")).is_err());
    }

    #[test]
    fn the_memory_platform_pays_only_a_ready_account_once_per_order() {
        let p = MemoryConnect::new(StripeMode::Test);
        let (acct, _) = block(p.create_account("seller", None)).unwrap();
        let charge =
            DestinationCharge::new(spec(Split::at_bps(2_000, 400).unwrap(), &acct)).unwrap();
        assert!(
            block(p.destination_charge(&charge)).is_err(),
            "not onboarded"
        );
        p.set_ready(&acct, true).unwrap();
        let first = block(p.destination_charge(&charge)).unwrap();
        let again = block(p.destination_charge(&charge)).unwrap();
        assert_eq!(first, again);
        assert_eq!(p.charges().len(), 1);
        assert!(first.url.starts_with("https://checkout.stripe.com/"));
    }

    #[test]
    fn the_memory_platform_signs_its_events_in_its_own_mode() {
        let test = MemoryConnect::new(StripeMode::Test);
        let (acct, _) = block(test.create_account("seller", None)).unwrap();
        let (body, sig) = test.account_updated(&acct).unwrap();
        assert_eq!(
            block(test.verify_webhook(&sig, &body)).unwrap(),
            StripeMode::Test
        );
        assert!(matches!(
            block(test.verify_webhook("t=1,v1=00", &body)),
            Err(MarketError::BadSignature)
        ));
        let live = MemoryConnect::new(StripeMode::Live);
        assert!(
            block(live.verify_webhook(&sig, &body)).is_err(),
            "test event on a live key"
        );
    }

    #[test]
    fn without_a_key_nothing_answers() {
        let p = MemoryConnect::new(StripeMode::Test);
        p.set_configured(false);
        assert!(matches!(block(p.mode()), Err(MarketError::Unconfigured)));
        assert!(matches!(
            block(p.create_account("seller", None)),
            Err(MarketError::Unconfigured)
        ));
    }
}
