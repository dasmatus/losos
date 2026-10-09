//! The Stripe gate: the one process on the edge that holds the Stripe key.
//!
//! `losos-registrar serve` is internet-reachable and runs as root next to
//! Traefik and rathole. The market only needs a few things from Stripe, so the
//! key lives in a second, small process (`losos-registrar stripe-gate`, its
//! own systemd unit and its own dynamic user) and the registrar asks it for
//! those few things over a Unix socket. The registrar never reads the key or
//! the webhook secrets, never opens a connection to Stripe, and cannot ask for
//! anything outside the list.
//!
//! What the split buys, and what it does not. A bug in the registrar's public
//! surface can no longer leak the key out of the registrar's memory or files,
//! and the key can no longer be used for refunds, payouts, customer lists or
//! any other endpoint: the gate has no such operation. It does **not** stop a
//! compromised registrar asking the gate to create a Checkout Session, so the
//! gate also bounds what that request may say (the currency, the destination
//! being an `acct_` id, the platform fee within [`MAX_FEE_BPS`], the session's
//! lifetime), and prices a hardware order from its own copy of the catalogue
//! (`crate::hardware`) rather than from the request. Both processes still run on one machine and the registrar is
//! root; `modules/edge.nix` hides the key's files and credentials directory
//! from the registrar's mount namespace as a second layer, not a boundary a
//! determined root process could not cross.
//!
//! Wire protocol: one connection is one request. The client sends one line of
//! JSON ([`Request`]) and closes its write half; the gate answers one line of
//! JSON ([`Reply`]) and closes. No new dependency: tokio's `net` and `io-util`
//! are already enabled for the registrar.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::action::Action;
use crate::hardware::{Catalogue, Line};
use crate::market::{
    account_ready, decode_hex, now_secs, valid_box_uuid, verify_signature, Kind, MarketError,
    MAX_FEE_BPS,
};

/// Budget for one Stripe request.
const STRIPE_TIMEOUT: Duration = Duration::from_secs(4);
/// Budget for a whole gate round trip, from the registrar's side. Every
/// operation makes at most one Stripe request, so one Stripe budget plus the
/// socket exchange around it. The registrar's routes that call the gate size
/// their own budget from this (`STRIPE_ROUTE_TIMEOUT` in `server.rs`), so a
/// slow but healthy Stripe answers before the route gives up on it.
pub const GATE_TIMEOUT: Duration = Duration::from_secs(STRIPE_TIMEOUT.as_secs() + 1);
/// Most gate round trips one registrar request makes: onboarding an existing
/// seller validates the secrets, re-tags the account, re-reads its readiness
/// and asks for a link.
pub const MAX_GATE_CALLS_PER_REQUEST: u32 = 4;
/// Largest request line the gate reads. A webhook body is capped at 256 KiB by
/// the registrar and travels hex-encoded, so twice that, plus the framing.
const MAX_REQUEST_BYTES: u64 = 2 * 256 * 1024 + 4096;
/// Largest reply the registrar reads back.
const MAX_REPLY_BYTES: u64 = 16 * 1024;
/// Connections handled at once. The registrar is the only client.
const MAX_IN_FLIGHT: usize = 16;
/// Stripe takes a Checkout Session expiry between 30 minutes and 24 hours out.
const MAX_SESSION_SECS: u64 = 24 * 3600;
const SOCKET_MODE: u32 = 0o600;

// ── Stripe ───────────────────────────────────────────────────────────────

/// The few Stripe calls the market makes, over plain form-encoded REST so no
/// SDK has to be vendored.
///
/// The key is deliberately not held here. Each call takes it as an argument
/// and it goes only into that request's `Authorization` header (which
/// `bearer_auth` marks sensitive, so it is redacted from debug output), never
/// into anything a URL is built from. Static analysis (CodeQL's cleartext
/// transmission query) treats a struct holding the key as tainted as a whole,
/// URL base included; keeping it out is what lets the URLs read as clean.
struct StripeClient {
    http: reqwest::Client,
    /// The API base, e.g. `https://api.stripe.com`. Paths are appended as
    /// percent-encoded segments by [`StripeClient::url`].
    api: reqwest::Url,
}

impl StripeClient {
    fn new(api: &str) -> Result<Self, MarketError> {
        let api = reqwest::Url::parse(api)
            .ok()
            .filter(|url| !url.cannot_be_a_base())
            .ok_or_else(|| MarketError::Stripe("stripe_api is not a base URL".to_string()))?;
        // The key travels in every request: https, or plain http to this
        // machine only (the test stub). `host_str` keeps an IPv6 literal's
        // brackets, hence `[::1]`.
        let loopback = api.scheme() == "http"
            && matches!(api.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
        if api.scheme() != "https" && !loopback {
            return Err(MarketError::Stripe(
                "stripe_api must use https:// to protect the key in transit".to_string(),
            ));
        }
        let http = reqwest::Client::builder()
            .timeout(STRIPE_TIMEOUT)
            .user_agent(concat!("losos-registrar/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| MarketError::Stripe(format!("build client: {e}")))?;
        Ok(Self { http, api })
    }

    /// The API base plus `segments`, each percent-encoded as one segment.
    fn url(&self, segments: &[&str]) -> reqwest::Url {
        let mut url = self.api.clone();
        if let Ok(mut path) = url.path_segments_mut() {
            path.pop_if_empty().extend(segments);
        }
        url
    }

    async fn send(
        &self,
        key: &str,
        request: reqwest::RequestBuilder,
        what: &str,
    ) -> Result<Value, MarketError> {
        let response = request
            .bearer_auth(key)
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
        key: &str,
        segments: &[&str],
        form: &[(&str, String)],
        idempotency_key: Option<&str>,
    ) -> Result<Value, MarketError> {
        let url = self.url(segments);
        let what = format!("POST {}", url.path());
        let mut request = self.http.post(url).form(form);
        if let Some(key) = idempotency_key {
            request = request.header("Idempotency-Key", key);
        }
        self.send(key, request, &what).await
    }

    /// Create the seller's Express account. The idempotency key makes a retry
    /// or a racing second request return the same account.
    async fn create_account(
        &self,
        key: &str,
        appliance_id: &str,
        box_uuid: Option<&str>,
    ) -> Result<String, MarketError> {
        let mut form = vec![
            ("type", "express".to_string()),
            ("capabilities[transfers][requested]", "true".to_string()),
            ("metadata[losos_appliance_id]", appliance_id.to_string()),
        ];
        if let Some(uuid) = box_uuid {
            form.push(("metadata[losos_box_uuid]", uuid.to_string()));
        }
        let body = self
            .post(
                key,
                &["v1", "accounts"],
                &form,
                Some(&format!("losos-account-{appliance_id}")),
            )
            .await?;
        body["id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| MarketError::Stripe("account response had no id".to_string()))
    }

    /// Write the box's UUID onto an existing account's metadata.
    async fn tag_account(
        &self,
        key: &str,
        account_id: &str,
        box_uuid: &str,
    ) -> Result<(), MarketError> {
        self.post(
            key,
            &["v1", "accounts", account_id],
            &[("metadata[losos_box_uuid]", box_uuid.to_string())],
            None,
        )
        .await
        .map(|_| ())
    }

    /// Whether an existing account can already receive transfers.
    async fn account_ready(&self, key: &str, account_id: &str) -> Result<bool, MarketError> {
        let url = self.url(&["v1", "accounts", account_id]);
        let what = format!("GET {}", url.path());
        let body = self.send(key, self.http.get(url), &what).await?;
        Ok(account_ready(&body))
    }

    /// A one-time Stripe-hosted onboarding URL.
    async fn account_link(
        &self,
        key: &str,
        account_id: &str,
        return_url: &str,
    ) -> Result<String, MarketError> {
        let body = self
            .post(
                key,
                &["v1", "account_links"],
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
    /// minus the fee to `destination`, and keeps `fee`.
    async fn checkout(
        &self,
        key: &str,
        c: &CheckoutRequest,
    ) -> Result<(String, String), MarketError> {
        let return_url = &c.return_url;
        let sep = if return_url.contains('?') { '&' } else { '?' };
        let form = [
            ("mode", "payment".to_string()),
            ("payment_method_types[]", "card".to_string()),
            ("client_reference_id", c.order_id.clone()),
            (
                "success_url",
                format!("{return_url}{sep}order={}&status=paid", c.order_id),
            ),
            (
                "cancel_url",
                format!("{return_url}{sep}order={}&status=cancelled", c.order_id),
            ),
            ("expires_at", c.expires_at.to_string()),
            ("metadata[order_id]", c.order_id.clone()),
            ("line_items[0][quantity]", c.quantity.to_string()),
            ("line_items[0][price_data][currency]", c.currency.clone()),
            (
                "line_items[0][price_data][unit_amount]",
                c.unit_price.to_string(),
            ),
            (
                "line_items[0][price_data][product_data][name]",
                format!("{} ({}) - losos market", c.kind.label(), c.kind.unit()),
            ),
            (
                "payment_intent_data[application_fee_amount]",
                c.fee.to_string(),
            ),
            (
                "payment_intent_data[transfer_data][destination]",
                c.destination.clone(),
            ),
            (
                "payment_intent_data[metadata][order_id]",
                c.order_id.clone(),
            ),
        ];
        let body = self
            .post(
                key,
                &["v1", "checkout", "sessions"],
                &form,
                Some(&format!("losos-checkout-{}", c.order_id)),
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

impl StripeClient {
    /// A Checkout Session for hardware the platform sells itself: a plain
    /// charge, no destination and no fee, with Stripe collecting where to
    /// ship. `lines` were priced by the gate from its own catalogue.
    async fn hardware_checkout(
        &self,
        key: &str,
        h: &HardwareRequest,
        currency: &str,
        countries: &[String],
        lines: &[crate::hardware::Priced<'_>],
    ) -> Result<(String, String), MarketError> {
        let owned = hardware_form(h, currency, countries, lines);
        let form: Vec<(&str, String)> =
            owned.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        let body = self
            .post(
                key,
                &["v1", "checkout", "sessions"],
                &form,
                Some(&format!("losos-hardware-{}", h.order_id)),
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

impl StripeClient {
    /// A Checkout Session for widget builder credit: a plain charge for one
    /// of the operator's packs, no destination and no fee.
    async fn credit_checkout(
        &self,
        key: &str,
        c: &CreditRequest,
        currency: &str,
    ) -> Result<(String, String), MarketError> {
        let owned = credit_form(c, currency);
        let form: Vec<(&str, String)> =
            owned.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        let body = self
            .post(
                key,
                &["v1", "checkout", "sessions"],
                &form,
                Some(&format!("losos-credit-{}", c.order_id)),
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

/// The form a credit Checkout Session is created with. Pure, so the exact
/// fields Stripe sees are asserted in a test.
fn credit_form(c: &CreditRequest, currency: &str) -> Vec<(String, String)> {
    let return_url = &c.return_url;
    let sep = if return_url.contains('?') { '&' } else { '?' };
    vec![
        ("mode".into(), "payment".into()),
        ("payment_method_types[]".into(), "card".into()),
        ("client_reference_id".into(), c.order_id.clone()),
        (
            "success_url".into(),
            format!("{return_url}{sep}order={}&status=paid", c.order_id),
        ),
        (
            "cancel_url".into(),
            format!("{return_url}{sep}order={}&status=cancelled", c.order_id),
        ),
        ("expires_at".into(), c.expires_at.to_string()),
        ("metadata[order_id]".into(), c.order_id.clone()),
        ("metadata[losos_kind]".into(), "builder_credit".into()),
        (
            "metadata[losos_appliance_id]".into(),
            c.appliance_id.clone(),
        ),
        ("line_items[0][quantity]".into(), "1".into()),
        (
            "line_items[0][price_data][currency]".into(),
            currency.to_string(),
        ),
        (
            "line_items[0][price_data][unit_amount]".into(),
            c.amount.to_string(),
        ),
        (
            "line_items[0][price_data][product_data][name]".into(),
            "Widget builder credit - LosOS".into(),
        ),
    ]
}

/// The form a hardware Checkout Session is created with. Pure, so the exact
/// fields Stripe sees are asserted in a test.
fn hardware_form(
    h: &HardwareRequest,
    currency: &str,
    countries: &[String],
    lines: &[crate::hardware::Priced<'_>],
) -> Vec<(String, String)> {
    let return_url = &h.return_url;
    let sep = if return_url.contains('?') { '&' } else { '?' };
    let mut form: Vec<(String, String)> = vec![
        ("mode".into(), "payment".into()),
        ("payment_method_types[]".into(), "card".into()),
        ("client_reference_id".into(), h.order_id.clone()),
        (
            "success_url".into(),
            format!("{return_url}{sep}order={}&status=paid", h.order_id),
        ),
        (
            "cancel_url".into(),
            format!("{return_url}{sep}order={}&status=cancelled", h.order_id),
        ),
        ("expires_at".into(), h.expires_at.to_string()),
        ("metadata[order_id]".into(), h.order_id.clone()),
        ("metadata[losos_kind]".into(), "hardware".into()),
        (
            "metadata[losos_appliance_id]".into(),
            h.appliance_id.clone(),
        ),
        ("phone_number_collection[enabled]".into(), "true".into()),
    ];
    for (i, c) in countries.iter().enumerate() {
        form.push((
            format!("shipping_address_collection[allowed_countries][{i}]"),
            c.clone(),
        ));
    }
    for (i, l) in lines.iter().enumerate() {
        let p = format!("line_items[{i}]");
        form.push((format!("{p}[quantity]"), l.quantity.to_string()));
        form.push((format!("{p}[price_data][currency]"), currency.to_string()));
        form.push((
            format!("{p}[price_data][unit_amount]"),
            l.item.unit_amount.to_string(),
        ));
        form.push((
            format!("{p}[price_data][product_data][name]"),
            l.item.name.clone(),
        ));
        if !l.item.detail.is_empty() {
            form.push((
                format!("{p}[price_data][product_data][description]"),
                l.item.detail.clone(),
            ));
        }
        form.push((
            format!("{p}[price_data][product_data][metadata][sku]"),
            l.item.sku.clone(),
        ));
    }
    form
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

/// Read a secret file. One that is absent is "not configured" (503), not an
/// upstream failure: the Stripe secrets are stored sealed and unsealed into
/// tmpfs at start, so an edge that has not sealed them yet, or whose host key
/// changed, has no file here and must say so rather than blame Stripe.
async fn read_secret_file(path: &str) -> Result<String, MarketError> {
    match tokio::fs::read_to_string(path).await {
        Ok(raw) => Ok(raw),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::error!(target: Action::Market.target(), "secret file {path} is absent");
            Err(MarketError::Unconfigured)
        }
        Err(e) => Err(MarketError::Stripe(format!("read secret {path}: {e}"))),
    }
}

async fn read_secret(path: &str, prefixes: &[&str]) -> Result<String, MarketError> {
    let raw = read_secret_file(path).await?;
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
    let raw = read_secret_file(path).await?;
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

// ── the gate's own settings ──────────────────────────────────────────────

/// `losos-registrar stripe-gate` settings.
#[derive(Debug, Clone)]
pub struct GateOpts {
    /// Unix socket the registrar connects to. Created 0600.
    pub socket: String,
    /// The platform's Stripe secret key (`sk_...` or a restricted `rk_...`),
    /// read per request so a rotated credential needs no restart.
    pub stripe_key_file: String,
    /// The webhook endpoints' signing secrets (`whsec_...`), one per line.
    pub webhook_secret_file: String,
    /// `https://api.stripe.com`; overridden only by tests.
    pub stripe_api: String,
    /// The one currency this edge sells in. A request in any other is refused.
    pub currency: String,
    /// The platform cut every Checkout must carry, in basis points, when set:
    /// the operator's `--market-fee-bps`. The fee must then be exactly what
    /// that rate gives, so a compromised registrar can neither raise the cut
    /// nor waive it. Unset, the fee is only held under the 20% hard ceiling
    /// ([`MAX_FEE_BPS`]).
    pub fee_bps: Option<u32>,
    /// The only return URL Checkout and onboarding may name, when set: the
    /// operator's `--market-return-url`. Unset, any https or http URL passes
    /// [`url_ok`], which let a compromised registrar send buyers and sellers
    /// anywhere once Stripe was done with them.
    pub return_url: Option<String>,
    /// The operator's hardware catalogue (`crate::hardware`), when the edge
    /// sells boxes and gateways. Read per request, like the key. Unset, the
    /// gate refuses every hardware checkout.
    pub hardware_catalogue: Option<String>,
    /// The widget builder's credit packs, in minor units: the only amounts a
    /// credit Checkout may charge. Empty, the gate refuses every one.
    pub credit_packs: Vec<u64>,
}

// ── wire protocol ────────────────────────────────────────────────────────

/// What a Checkout Session may say. The gate checks every field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CheckoutRequest {
    pub(crate) order_id: String,
    pub(crate) kind: Kind,
    pub(crate) quantity: u64,
    pub(crate) unit_price: u64,
    pub(crate) currency: String,
    pub(crate) fee: u64,
    pub(crate) destination: String,
    pub(crate) return_url: String,
    pub(crate) expires_at: u64,
}

/// A hardware Checkout. Carries skus and quantities only: the gate prices
/// every line from its own copy of the catalogue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HardwareRequest {
    pub(crate) order_id: String,
    pub(crate) appliance_id: String,
    pub(crate) lines: Vec<Line>,
    pub(crate) return_url: String,
    pub(crate) expires_at: u64,
}

/// A widget builder top-up: one of the operator's packs, bought by a box.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CreditRequest {
    pub(crate) order_id: String,
    pub(crate) appliance_id: String,
    pub(crate) amount: u64,
    pub(crate) return_url: String,
    pub(crate) expires_at: u64,
}

/// The only things the gate will do. There is deliberately no generic
/// "forward this to Stripe" operation.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum Request {
    ValidateSecrets,
    CreateAccount {
        appliance_id: String,
        #[serde(default)]
        box_uuid: Option<String>,
    },
    TagAccount {
        account_id: String,
        box_uuid: String,
    },
    AccountReady {
        account_id: String,
    },
    AccountLink {
        account_id: String,
        return_url: String,
    },
    Checkout(CheckoutRequest),
    HardwareCheckout(HardwareRequest),
    CreditCheckout(CreditRequest),
    VerifyWebhook {
        signature: String,
        body_hex: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FaultKind {
    /// No usable key or secret: the market is not set up on this edge.
    Unconfigured,
    /// The request was outside what the gate allows.
    Refused,
    BadSignature,
    /// Stripe failed or refused.
    Stripe,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Fault {
    pub(crate) kind: FaultKind,
    pub(crate) message: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Reply {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ready: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<Fault>,
}

impl Reply {
    fn fault(kind: FaultKind, message: impl Into<String>) -> Self {
        Self {
            error: Some(Fault {
                kind,
                message: message.into(),
            }),
            ..Self::default()
        }
    }
}

impl From<MarketError> for Reply {
    fn from(e: MarketError) -> Self {
        match e {
            MarketError::Unconfigured => Self::fault(FaultKind::Unconfigured, "not configured"),
            MarketError::BadSignature => Self::fault(FaultKind::BadSignature, "bad signature"),
            MarketError::Stripe(m) => Self::fault(FaultKind::Stripe, m),
            other => Self::fault(FaultKind::Stripe, other.to_string()),
        }
    }
}

// ── request validation ───────────────────────────────────────────────────

fn refuse(what: &str) -> Reply {
    Reply::fault(FaultKind::Refused, what)
}

/// `acct_...`, the shape of a Stripe account id.
fn account_id_ok(id: &str) -> bool {
    id.len() <= 64
        && id.strip_prefix("acct_").is_some_and(|rest| {
            !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
}

fn token_ok(id: &str, max: usize) -> bool {
    !id.is_empty()
        && id.len() <= max
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Whether `url` can be handed to Stripe as a return or refresh URL: an
/// absolute http(s) URL with a host, no credentials, and no fragment, since
/// `?order=...&status=...` is appended and would land after a `#` unread.
/// Shared with the registrar's `--market-return-url` check, so a value the
/// registrar starts with is one the gate accepts.
pub fn url_ok(url: &str) -> bool {
    if url.len() > 512 || url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return false;
    }
    // The WHATWG parser reads `https:///market` as host `market`; insist the
    // authority is actually written where it belongs.
    let Some(authority) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };
    if authority.starts_with(['/', '\\']) {
        return false;
    }
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    matches!(parsed.scheme(), "https" | "http")
        && parsed.host_str().is_some_and(|h| !h.is_empty())
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.fragment().is_none()
}

/// Why a Checkout request is outside what the gate allows, or `None`.
fn checkout_fault(
    c: &CheckoutRequest,
    currency: &str,
    fee_bps: Option<u32>,
    return_url: Option<&str>,
    now: u64,
) -> Option<&'static str> {
    if !token_ok(&c.order_id, 64) {
        return Some("bad order id");
    }
    if !account_id_ok(&c.destination) {
        return Some("destination is not a Stripe account id");
    }
    if !url_ok(&c.return_url) || return_url.is_some_and(|want| c.return_url != want) {
        return Some("bad return url");
    }
    if c.currency != currency {
        return Some("currency is not the one this edge sells in");
    }
    if c.quantity == 0 || c.unit_price == 0 {
        return Some("empty order");
    }
    let Some(amount) = c.quantity.checked_mul(c.unit_price) else {
        return Some("amount overflows");
    };
    // The platform's cut is exactly the operator's rate when the gate knows
    // it, never above the MAX_FEE_BPS ceiling otherwise, and can never take
    // the whole charge.
    let fee_ok = match fee_bps {
        Some(bps) => c.fee == crate::market::fee_for(amount, bps.min(MAX_FEE_BPS)),
        None => c.fee <= crate::market::fee_for(amount, MAX_FEE_BPS),
    };
    if c.fee >= amount || !fee_ok {
        return Some("fee is outside the allowed range");
    }
    if c.expires_at <= now || c.expires_at > now + MAX_SESSION_SECS {
        return Some("session lifetime is outside the allowed range");
    }
    None
}

/// Why a hardware Checkout is outside what the gate allows, or `None`. The
/// lines themselves are checked when they are priced.
fn hardware_fault(
    h: &HardwareRequest,
    catalogue: &Catalogue,
    currency: &str,
    return_url: Option<&str>,
    now: u64,
) -> Option<&'static str> {
    if !token_ok(&h.order_id, 64) || !h.order_id.starts_with("hw_") {
        return Some("bad order id");
    }
    if !token_ok(&h.appliance_id, 64) {
        return Some("bad appliance id");
    }
    if !url_ok(&h.return_url) || return_url.is_some_and(|want| h.return_url != want) {
        return Some("bad return url");
    }
    if catalogue.currency != currency {
        return Some("the catalogue's currency is not the one this edge sells in");
    }
    if h.expires_at <= now || h.expires_at > now + MAX_SESSION_SECS {
        return Some("session lifetime is outside the allowed range");
    }
    None
}

/// Why a credit Checkout is outside what the gate allows, or `None`.
fn credit_fault(
    c: &CreditRequest,
    packs: &[u64],
    return_url: Option<&str>,
    now: u64,
) -> Option<&'static str> {
    if !token_ok(&c.order_id, 64) || !c.order_id.starts_with("cr_") {
        return Some("bad order id");
    }
    if !token_ok(&c.appliance_id, 64) {
        return Some("bad appliance id");
    }
    if !packs.contains(&c.amount) {
        return Some("amount is not one of the credit packs");
    }
    if !url_ok(&c.return_url) || return_url.is_some_and(|want| c.return_url != want) {
        return Some("bad return url");
    }
    if c.expires_at <= now || c.expires_at > now + MAX_SESSION_SECS {
        return Some("session lifetime is outside the allowed range");
    }
    None
}

async fn hardware_catalogue(opts: &GateOpts) -> Result<Catalogue, Reply> {
    let Some(path) = opts.hardware_catalogue.as_deref() else {
        return Err(refuse("this edge sells no hardware"));
    };
    let bytes = tokio::fs::read(path).await.map_err(|e| {
        tracing::error!(target: Action::Market.target(), "hardware catalogue {path}: {e}");
        Reply::from(MarketError::Unconfigured)
    })?;
    Catalogue::parse(&bytes).map_err(|e| {
        tracing::error!(target: Action::Market.target(), "hardware catalogue {path}: {e}");
        Reply::from(MarketError::Unconfigured)
    })
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ── the gate ─────────────────────────────────────────────────────────────

async fn stripe_key(opts: &GateOpts) -> Result<String, MarketError> {
    read_secret(&opts.stripe_key_file, &["sk_", "rk_"]).await
}

/// Bind the Stripe key and a client as `$key` and `$client`, or return the
/// fault as the reply. Two bindings rather than one value holding both: see
/// [`StripeClient`] for why the key is never stored next to the URL base.
macro_rules! stripe_or_reply {
    ($opts:expr, $key:ident, $client:ident) => {
        let $key = match stripe_key($opts).await {
            Ok(key) => key,
            Err(e) => return e.into(),
        };
        let $client = match StripeClient::new(&$opts.stripe_api) {
            Ok(client) => client,
            Err(e) => return e.into(),
        };
    };
}

async fn handle(opts: &GateOpts, request: Request) -> Reply {
    match request {
        Request::ValidateSecrets => {
            stripe_or_reply!(opts, _key, _client);
            match read_webhook_secrets(&opts.webhook_secret_file).await {
                Ok(_) => Reply::default(),
                Err(e) => e.into(),
            }
        }
        Request::CreateAccount {
            appliance_id,
            box_uuid,
        } => {
            if !token_ok(&appliance_id, 64) {
                return refuse("bad appliance id");
            }
            if box_uuid.as_deref().is_some_and(|u| !valid_box_uuid(u)) {
                return refuse("bad box uuid");
            }
            stripe_or_reply!(opts, key, s);
            match s
                .create_account(&key, &appliance_id, box_uuid.as_deref())
                .await
            {
                Ok(id) => Reply {
                    id: Some(id),
                    ..Reply::default()
                },
                Err(e) => e.into(),
            }
        }
        Request::TagAccount {
            account_id,
            box_uuid,
        } => {
            if !account_id_ok(&account_id) {
                return refuse("bad account id");
            }
            if !valid_box_uuid(&box_uuid) {
                return refuse("bad box uuid");
            }
            stripe_or_reply!(opts, key, s);
            match s.tag_account(&key, &account_id, &box_uuid).await {
                Ok(()) => Reply::default(),
                Err(e) => e.into(),
            }
        }
        Request::AccountReady { account_id } => {
            if !account_id_ok(&account_id) {
                return refuse("bad account id");
            }
            stripe_or_reply!(opts, key, s);
            match s.account_ready(&key, &account_id).await {
                Ok(ready) => Reply {
                    ready: Some(ready),
                    ..Reply::default()
                },
                Err(e) => e.into(),
            }
        }
        Request::AccountLink {
            account_id,
            return_url,
        } => {
            if !account_id_ok(&account_id) {
                return refuse("bad account id");
            }
            if !url_ok(&return_url)
                || opts
                    .return_url
                    .as_deref()
                    .is_some_and(|want| return_url != want)
            {
                return refuse("bad return url");
            }
            stripe_or_reply!(opts, key, s);
            match s.account_link(&key, &account_id, &return_url).await {
                Ok(url) => Reply {
                    url: Some(url),
                    ..Reply::default()
                },
                Err(e) => e.into(),
            }
        }
        Request::Checkout(c) => {
            if let Some(why) = checkout_fault(
                &c,
                &opts.currency,
                opts.fee_bps,
                opts.return_url.as_deref(),
                now_secs(),
            ) {
                return refuse(why);
            }
            stripe_or_reply!(opts, key, s);
            match s.checkout(&key, &c).await {
                Ok((id, url)) => Reply {
                    id: Some(id),
                    url: Some(url),
                    ..Reply::default()
                },
                Err(e) => e.into(),
            }
        }
        Request::HardwareCheckout(h) => {
            let catalogue = match hardware_catalogue(opts).await {
                Ok(c) => c,
                Err(reply) => return reply,
            };
            if let Some(why) = hardware_fault(
                &h,
                &catalogue,
                &opts.currency,
                opts.return_url.as_deref(),
                now_secs(),
            ) {
                return refuse(why);
            }
            let lines = match catalogue.price(&h.lines) {
                Ok(lines) => lines,
                Err(why) => return refuse(why),
            };
            stripe_or_reply!(opts, key, s);
            match s
                .hardware_checkout(&key, &h, &catalogue.currency, &catalogue.countries, &lines)
                .await
            {
                Ok((id, url)) => Reply {
                    id: Some(id),
                    url: Some(url),
                    ..Reply::default()
                },
                Err(e) => e.into(),
            }
        }
        Request::CreditCheckout(c) => {
            if let Some(why) = credit_fault(
                &c,
                &opts.credit_packs,
                opts.return_url.as_deref(),
                now_secs(),
            ) {
                return refuse(why);
            }
            stripe_or_reply!(opts, key, s);
            match s.credit_checkout(&key, &c, &opts.currency).await {
                Ok((id, url)) => Reply {
                    id: Some(id),
                    url: Some(url),
                    ..Reply::default()
                },
                Err(e) => e.into(),
            }
        }
        Request::VerifyWebhook {
            signature,
            body_hex,
        } => {
            let Some(body) = decode_hex(&body_hex) else {
                return refuse("body is not hex");
            };
            let secrets = match read_webhook_secrets(&opts.webhook_secret_file).await {
                Ok(s) => s,
                Err(e) => return e.into(),
            };
            let now = now_secs();
            if secrets
                .iter()
                .any(|secret| verify_signature(secret, &signature, &body, now).is_ok())
            {
                Reply::default()
            } else {
                MarketError::BadSignature.into()
            }
        }
    }
}

async fn serve_connection(opts: &GateOpts, stream: UnixStream) -> io::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut line = Vec::new();
    BufReader::new(read.take(MAX_REQUEST_BYTES))
        .read_until(b'\n', &mut line)
        .await?;
    let reply = match serde_json::from_slice::<Request>(&line) {
        Ok(request) => handle(opts, request).await,
        Err(_) => refuse("unknown or malformed request"),
    };
    let mut out = serde_json::to_vec(&reply).map_err(io::Error::other)?;
    out.push(b'\n');
    write.write_all(&out).await?;
    write.shutdown().await
}

/// Bind the socket, replacing a stale one left by a previous run.
///
/// # Errors
/// If the socket cannot be created or its mode set.
pub fn bind(path: &str) -> io::Result<UnixListener> {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let listener = UnixListener::bind(path)?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(SOCKET_MODE))?;
    }
    Ok(listener)
}

/// Serve the gate until `shutdown` resolves.
pub async fn serve<F>(listener: UnixListener, opts: GateOpts, shutdown: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let opts = std::sync::Arc::new(opts);
    let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_IN_FLIGHT));
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            () = &mut shutdown => break,
            accepted = listener.accept() => {
                let Ok((stream, _)) = accepted else { continue };
                let Ok(permit) = slots.clone().try_acquire_owned() else {
                    // Over the cap: drop the connection; the registrar reads
                    // that as the gate being unavailable.
                    continue;
                };
                let opts = opts.clone();
                tokio::spawn(async move {
                    let _permit = permit;
                    let work = serve_connection(&opts, stream);
                    match tokio::time::timeout(GATE_TIMEOUT, work).await {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => tracing::warn!(target: Action::Market.target(), "gate connection: {e}"),
                        Err(_) => tracing::warn!(target: Action::Market.target(), "gate connection timed out"),
                    }
                });
            }
        }
    }
}

/// Run the gate as the process's main job.
///
/// # Errors
/// If the socket cannot be bound.
pub async fn run(opts: GateOpts) -> miette::Result<()> {
    use miette::{Context, IntoDiagnostic};
    let listener = bind(&opts.socket)
        .into_diagnostic()
        .with_context(|| format!("bind {}", opts.socket))?;
    tracing::info!(target: Action::Bind.target(), "stripe gate listening on {}", opts.socket);
    serve(listener, opts, crate::server::shutdown_signal()).await;
    Ok(())
}

// ── the registrar's side ─────────────────────────────────────────────────

/// How the registrar reaches Stripe: only through the gate.
#[derive(Debug, Clone)]
pub struct GateClient {
    socket: PathBuf,
}

impl GateClient {
    #[must_use]
    pub fn new(socket: impl AsRef<Path>) -> Self {
        Self {
            socket: socket.as_ref().to_path_buf(),
        }
    }

    async fn exchange(&self, request: &Request) -> Result<Reply, MarketError> {
        let stream = match UnixStream::connect(&self.socket).await {
            Ok(s) => s,
            // No gate running: the key was never sealed, or the unit could not
            // decrypt it. That is "the market is off here", not a Stripe fault.
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) =>
            {
                tracing::error!(
                    target: Action::Market.target(),
                    "stripe gate is not running at {}",
                    self.socket.display(),
                );
                return Err(MarketError::Unconfigured);
            }
            Err(e) => return Err(MarketError::Stripe(format!("gate: {e}"))),
        };
        let (read, mut write) = stream.into_split();
        let mut line = serde_json::to_vec(request)?;
        line.push(b'\n');
        write
            .write_all(&line)
            .await
            .map_err(|e| MarketError::Stripe(format!("gate: {e}")))?;
        write
            .shutdown()
            .await
            .map_err(|e| MarketError::Stripe(format!("gate: {e}")))?;
        let mut raw = Vec::new();
        read.take(MAX_REPLY_BYTES)
            .read_to_end(&mut raw)
            .await
            .map_err(|e| MarketError::Stripe(format!("gate: {e}")))?;
        serde_json::from_slice(&raw).map_err(|_| MarketError::Stripe("gate: no reply".to_string()))
    }

    async fn call(&self, request: &Request) -> Result<Reply, MarketError> {
        let reply = tokio::time::timeout(GATE_TIMEOUT, self.exchange(request))
            .await
            .map_err(|_| MarketError::Stripe("gate: timed out".to_string()))??;
        match reply.error {
            None => Ok(Reply {
                error: None,
                ..reply
            }),
            Some(Fault { kind, message }) => Err(match kind {
                FaultKind::Unconfigured => MarketError::Unconfigured,
                FaultKind::BadSignature => MarketError::BadSignature,
                FaultKind::Refused => MarketError::Stripe(format!("gate refused: {message}")),
                FaultKind::Stripe => MarketError::Stripe(message),
            }),
        }
    }

    pub async fn validate_secrets(&self) -> Result<(), MarketError> {
        self.call(&Request::ValidateSecrets).await.map(|_| ())
    }

    /// Create the seller's Express account; a retry returns the same one.
    ///
    /// # Errors
    /// If the gate is unavailable or Stripe refuses.
    pub async fn create_account(
        &self,
        appliance_id: &str,
        box_uuid: Option<&str>,
    ) -> Result<String, MarketError> {
        self.call(&Request::CreateAccount {
            appliance_id: appliance_id.to_string(),
            box_uuid: box_uuid.map(str::to_string),
        })
        .await?
        .id
        .ok_or_else(|| MarketError::Stripe("gate: no account id".to_string()))
    }

    /// Write the box's UUID onto an existing account.
    ///
    /// # Errors
    /// If the gate is unavailable or Stripe refuses.
    pub async fn tag_account(&self, account_id: &str, box_uuid: &str) -> Result<(), MarketError> {
        self.call(&Request::TagAccount {
            account_id: account_id.to_string(),
            box_uuid: box_uuid.to_string(),
        })
        .await
        .map(|_| ())
    }

    /// Whether an existing account can already receive transfers.
    ///
    /// # Errors
    /// If the gate is unavailable or Stripe refuses.
    pub async fn account_ready(&self, account_id: &str) -> Result<bool, MarketError> {
        self.call(&Request::AccountReady {
            account_id: account_id.to_string(),
        })
        .await?
        .ready
        .ok_or_else(|| MarketError::Stripe("gate: no readiness".to_string()))
    }

    /// A one-time Stripe-hosted onboarding URL.
    ///
    /// # Errors
    /// If the gate is unavailable or Stripe refuses.
    pub async fn account_link(
        &self,
        account_id: &str,
        return_url: &str,
    ) -> Result<String, MarketError> {
        self.call(&Request::AccountLink {
            account_id: account_id.to_string(),
            return_url: return_url.to_string(),
        })
        .await?
        .url
        .ok_or_else(|| MarketError::Stripe("gate: no url".to_string()))
    }

    /// A Checkout Session; returns its id and hosted URL.
    ///
    /// # Errors
    /// If the gate is unavailable, refuses the request, or Stripe refuses.
    pub(crate) async fn checkout(
        &self,
        request: CheckoutRequest,
    ) -> Result<(String, String), MarketError> {
        let reply = self.call(&Request::Checkout(request)).await?;
        match (reply.id, reply.url) {
            (Some(id), Some(url)) => Ok((id, url)),
            _ => Err(MarketError::Stripe(
                "checkout session had no id or url".to_string(),
            )),
        }
    }

    /// A hardware Checkout Session; returns its id and hosted URL.
    ///
    /// # Errors
    /// If the gate is unavailable, refuses the request, or Stripe refuses.
    pub(crate) async fn hardware_checkout(
        &self,
        request: HardwareRequest,
    ) -> Result<(String, String), MarketError> {
        let reply = self.call(&Request::HardwareCheckout(request)).await?;
        match (reply.id, reply.url) {
            (Some(id), Some(url)) => Ok((id, url)),
            _ => Err(MarketError::Stripe(
                "checkout session had no id or url".to_string(),
            )),
        }
    }

    /// A widget builder credit Checkout Session; returns its id and hosted
    /// URL.
    ///
    /// # Errors
    /// If the gate is unavailable, refuses the request, or Stripe refuses.
    pub(crate) async fn credit_checkout(
        &self,
        request: CreditRequest,
    ) -> Result<(String, String), MarketError> {
        let reply = self.call(&Request::CreditCheckout(request)).await?;
        match (reply.id, reply.url) {
            (Some(id), Some(url)) => Ok((id, url)),
            _ => Err(MarketError::Stripe(
                "checkout session had no id or url".to_string(),
            )),
        }
    }

    /// Whether `signature` is a valid `Stripe-Signature` for `body` under any
    /// of the webhook secrets the gate holds.
    ///
    /// # Errors
    /// [`MarketError::BadSignature`] if it is not; others if the gate is
    /// unavailable.
    pub async fn verify_webhook(&self, signature: &str, body: &[u8]) -> Result<(), MarketError> {
        self.call(&Request::VerifyWebhook {
            signature: signature.to_string(),
            body_hex: encode_hex(body),
        })
        .await
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_000_000;

    fn checkout() -> CheckoutRequest {
        CheckoutRequest {
            order_id: "ord_0a1b2c".to_string(),
            kind: Kind::Storage,
            quantity: 10,
            unit_price: 100,
            currency: "eur".to_string(),
            fee: 40,
            destination: "acct_1Abc".to_string(),
            return_url: "https://losos.example/market".to_string(),
            expires_at: NOW + 1800,
        }
    }

    #[test]
    fn secret_files_are_shape_checked() {
        assert_eq!(secret_fault("", &["sk_"]), Some("empty"));
        assert!(secret_fault("sk_live_x", &["sk_", "rk_"]).is_none());
        assert!(secret_fault("pk_live_x", &["sk_", "rk_"]).is_some());
        assert!(secret_fault("sk_a b", &["sk_"]).is_some());
    }

    #[test]
    fn an_absent_secret_file_means_not_configured() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let missing = std::env::temp_dir().join("losos-no-such-secret-for-test");
        let path = missing.to_str().unwrap();
        assert!(matches!(
            rt.block_on(read_secret(path, &["sk_"])),
            Err(MarketError::Unconfigured)
        ));
        assert!(matches!(
            rt.block_on(read_webhook_secrets(path)),
            Err(MarketError::Unconfigured)
        ));
    }

    #[test]
    fn a_well_formed_checkout_is_allowed() {
        assert_eq!(checkout_fault(&checkout(), "eur", None, None, NOW), None);
    }

    #[test]
    fn stripe_api_allows_http_only_for_loopback_hosts() {
        assert!(StripeClient::new("https://api.stripe.com").is_ok());
        for url in [
            "http://127.0.0.1:8080",
            "http://localhost:8080",
            "http://[::1]:8080",
        ] {
            assert!(StripeClient::new(url).is_ok(), "{url}");
        }
        assert!(StripeClient::new("https://").is_err());
        assert!(StripeClient::new("http://127.0.0.1.example:8080").is_err());
        assert!(StripeClient::new("http://localhost.attacker.example").is_err());
    }

    #[test]
    fn stripe_paths_are_appended_as_encoded_segments() {
        let client = StripeClient::new("https://api.stripe.com/").expect("client");
        assert_eq!(
            client.url(&["v1", "accounts", "acct_1"]).as_str(),
            "https://api.stripe.com/v1/accounts/acct_1"
        );
        // A segment can never climb out of its place in the path.
        assert_eq!(
            client.url(&["v1", "accounts", "../x"]).as_str(),
            "https://api.stripe.com/v1/accounts/..%2Fx"
        );
    }

    #[test]
    fn a_checkout_outside_the_limits_is_refused() {
        type Mutation = Box<dyn Fn(&mut CheckoutRequest)>;
        let cases: Vec<(&str, Mutation)> = vec![
            (
                "destination",
                Box::new(|c| c.destination = "cus_123".into()),
            ),
            ("destination", Box::new(|c| c.destination = "acct_".into())),
            (
                "return",
                Box::new(|c| c.return_url = "javascript:alert(1)".into()),
            ),
            ("currency", Box::new(|c| c.currency = "usd".into())),
            ("empty", Box::new(|c| c.quantity = 0)),
            ("overflow", Box::new(|c| c.unit_price = u64::MAX)),
            ("fee over ceiling", Box::new(|c| c.fee = 201)),
            ("fee is all of it", Box::new(|c| c.fee = 1000)),
            ("already expired", Box::new(|c| c.expires_at = NOW)),
            ("too far out", Box::new(|c| c.expires_at = NOW + 25 * 3600)),
            ("order id", Box::new(|c| c.order_id = "ord 1".into())),
        ];
        for (name, mutate) in cases {
            let mut c = checkout();
            mutate(&mut c);
            assert!(
                checkout_fault(&c, "eur", None, None, NOW).is_some(),
                "{name} was allowed"
            );
        }
    }

    #[test]
    fn checkout_is_held_to_the_operators_fee_and_return_url() {
        // checkout() is 10 x 100 with a fee of 40: exactly 400 bps.
        let c = checkout();
        let at = |fee: u64| CheckoutRequest { fee, ..checkout() };
        let url = Some(c.return_url.as_str());
        assert_eq!(checkout_fault(&c, "eur", Some(400), url, NOW), None);
        // Above the operator's rate, below it, and waived entirely.
        assert!(checkout_fault(&at(41), "eur", Some(400), url, NOW).is_some());
        assert!(checkout_fault(&at(39), "eur", Some(400), url, NOW).is_some());
        assert!(checkout_fault(&at(0), "eur", Some(400), url, NOW).is_some());
        // Without a configured rate only the hard ceiling applies.
        assert_eq!(checkout_fault(&at(0), "eur", None, None, NOW), None);
        assert!(checkout_fault(&at(201), "eur", None, None, NOW).is_some());
        assert!(checkout_fault(
            &c,
            "eur",
            Some(400),
            Some("https://elsewhere.example/"),
            NOW
        )
        .is_some());
    }

    #[test]
    fn return_urls_are_parsed_not_prefix_matched() {
        for good in [
            "https://losos.cfd/market",
            "https://losos.cfd/market?from=stripe",
            "http://localhost:8080/back",
        ] {
            assert!(url_ok(good), "{good} was refused");
        }
        for bad in [
            "https://",
            "http://",
            "https:///market",
            "https://losos.cfd/market#done",
            "https://user:pw@losos.cfd/market",
            "https://user@losos.cfd/market",
            "ftp://losos.cfd/market",
            "javascript:alert(1)",
            "https://losos.cfd/a b",
            "losos.cfd/market",
        ] {
            assert!(!url_ok(bad), "{bad} was allowed");
        }
    }

    #[test]
    fn the_fee_ceiling_is_the_operators_ceiling() {
        let mut c = checkout();
        c.fee = 200;
        assert_eq!(checkout_fault(&c, "eur", None, None, NOW), None);
    }

    #[test]
    fn requests_name_only_the_market_operations() {
        for ok in [
            r#"{"op":"validate_secrets"}"#,
            r#"{"op":"create_account","appliance_id":"box-1"}"#,
            r#"{"op":"tag_account","account_id":"acct_1","box_uuid":"x"}"#,
            r#"{"op":"account_ready","account_id":"acct_1"}"#,
            r#"{"op":"verify_webhook","signature":"s","body_hex":"00"}"#,
        ] {
            assert!(serde_json::from_str::<Request>(ok).is_ok(), "{ok}");
        }
        for bad in [
            r#"{"op":"refund","payment_intent":"pi_1"}"#,
            r#"{"op":"forward","path":"/v1/payouts"}"#,
            r#"{}"#,
        ] {
            assert!(serde_json::from_str::<Request>(bad).is_err(), "{bad}");
        }
    }

    fn hardware() -> (HardwareRequest, Catalogue) {
        let catalogue = Catalogue::parse(
            br#"{"currency":"eur","countries":["SK"],"items":[
                {"sku":"box","name":"LosOS box","detail":"16 GB, 1 TB","unit_amount":44900}]}"#,
        )
        .unwrap();
        let request = HardwareRequest {
            order_id: "hw_0123abcd".to_string(),
            appliance_id: "mattbox".to_string(),
            lines: vec![Line {
                sku: "box".to_string(),
                quantity: 2,
            }],
            return_url: "https://losos.example/market".to_string(),
            expires_at: NOW + 1800,
        };
        (request, catalogue)
    }

    #[test]
    fn a_hardware_checkout_is_held_to_the_operators_settings() {
        let (ok, c) = hardware();
        let ret = Some("https://losos.example/market");
        assert_eq!(hardware_fault(&ok, &c, "eur", ret, NOW), None);
        let bad = |f: &dyn Fn(&mut HardwareRequest)| {
            let mut h = ok.clone();
            f(&mut h);
            hardware_fault(&h, &c, "eur", ret, NOW)
        };
        assert!(bad(&|h| h.order_id = "ord_0123".to_string()).is_some());
        assert!(bad(&|h| h.appliance_id = "a b".to_string()).is_some());
        assert!(bad(&|h| h.return_url = "https://evil.example/".to_string()).is_some());
        assert!(bad(&|h| h.expires_at = NOW).is_some());
        assert!(bad(&|h| h.expires_at = NOW + MAX_SESSION_SECS + 1).is_some());
        assert!(hardware_fault(&ok, &c, "usd", ret, NOW).is_some());
    }

    #[test]
    fn the_hardware_session_ships_and_carries_no_destination_or_fee() {
        let (h, c) = hardware();
        let lines = c.price(&h.lines).unwrap();
        let form: std::collections::HashMap<String, String> =
            hardware_form(&h, &c.currency, &c.countries, &lines)
                .into_iter()
                .collect();
        assert_eq!(form["line_items[0][price_data][unit_amount]"], "44900");
        assert_eq!(form["line_items[0][quantity]"], "2");
        assert_eq!(
            form["line_items[0][price_data][product_data][description]"],
            "16 GB, 1 TB"
        );
        assert_eq!(
            form["shipping_address_collection[allowed_countries][0]"],
            "SK"
        );
        assert_eq!(form["metadata[losos_kind]"], "hardware");
        assert_eq!(
            form["success_url"],
            "https://losos.example/market?order=hw_0123abcd&status=paid"
        );
        assert!(!form
            .keys()
            .any(|k| k.contains("transfer_data") || k.contains("application_fee")));
    }

    fn credit() -> CreditRequest {
        CreditRequest {
            order_id: "cr_0123abcd".to_string(),
            appliance_id: "mattbox".to_string(),
            amount: 1_000,
            return_url: "https://losos.example/market".to_string(),
            expires_at: NOW + 1800,
        }
    }

    #[test]
    fn a_credit_checkout_charges_only_a_pack() {
        let ok = credit();
        let packs = [500, 1_000];
        let ret = Some("https://losos.example/market");
        assert_eq!(credit_fault(&ok, &packs, ret, NOW), None);
        let bad = |f: &dyn Fn(&mut CreditRequest)| {
            let mut c = ok.clone();
            f(&mut c);
            credit_fault(&c, &packs, ret, NOW)
        };
        assert!(bad(&|c| c.amount = 999).is_some());
        assert!(bad(&|c| c.amount = 0).is_some());
        assert!(bad(&|c| c.order_id = "hw_0123".to_string()).is_some());
        assert!(bad(&|c| c.appliance_id = "a b".to_string()).is_some());
        assert!(bad(&|c| c.return_url = "https://evil.example/".to_string()).is_some());
        assert!(bad(&|c| c.expires_at = NOW + MAX_SESSION_SECS + 1).is_some());
        assert!(credit_fault(&ok, &[], ret, NOW).is_some());
    }

    #[test]
    fn the_credit_session_is_a_plain_charge_in_the_edges_currency() {
        let form: std::collections::HashMap<String, String> =
            credit_form(&credit(), "eur").into_iter().collect();
        assert_eq!(form["line_items[0][price_data][unit_amount]"], "1000");
        assert_eq!(form["line_items[0][price_data][currency]"], "eur");
        assert_eq!(form["line_items[0][quantity]"], "1");
        assert_eq!(form["client_reference_id"], "cr_0123abcd");
        assert_eq!(form["metadata[losos_kind]"], "builder_credit");
        assert!(!form
            .keys()
            .any(|k| k.contains("transfer_data") || k.contains("application_fee")));
        assert!(serde_json::from_str::<Request>(
            r#"{"op":"credit_checkout","order_id":"cr_1","appliance_id":"b","amount":500,"return_url":"https://x/","expires_at":1}"#
        )
        .is_ok());
    }
}
