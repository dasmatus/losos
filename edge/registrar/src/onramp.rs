//! Buying USDC with euros at Binance (Binance Connect, the hosted on-ramp of
//! Binance Pay Onchain), delivered on Base to the buyer's own wallet.
//!
//! The box never talks to Binance. It asks this edge for an order with the
//! wallet address and the amount; the edge signs a pre-order with the
//! partner's RSA key and hands back Binance's checkout link. The buyer signs
//! in to Binance there, passes Binance's identity and anti-money-laundering
//! checks, pays, and Binance withdraws the USDC to the address in the order.
//! LosOS holds no money and learns no identity: what comes back is the
//! order's status, the amount and the withdrawal's transaction.
//!
//! The partner key and Binance's access token are systemd credentials read
//! on every call, never flags. Binance's webhook key is public. Requests are
//! signed `SHA256withRSA(body + timestamp)` in the `X-Tesla-*` headers and
//! webhooks the same way in the `X-BN-Connect-*` ones, as Binance documents
//! (`developers.binance.com/en/docs/products/connect-2.0`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{self, RsaKeyPair};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;

/// The pre-order: Binance answers with the checkout link.
pub const PRE_ORDER: &str = "/papi/v1/ramp/connect/buy/pre-order";
/// An order's details and status.
pub const ORDER: &str = "/papi/v1/ramp/connect/order";

/// What is sold, for what, and where it goes. Binance's own codes.
pub const FIAT: &str = "EUR";
pub const CRYPTO: &str = "USDC";
pub const NETWORK: &str = "BASE";

/// Euro amounts the edge passes on. Binance shows its own limits for the
/// buyer's payment method on its page and refuses what is outside them.
pub const MIN_EUROS: u64 = 1;
pub const MAX_EUROS: u64 = 100_000;

/// Orders one box may start in a day. A buyer needs one or two; this stops a
/// stolen token from filling Binance's partner quota.
pub const MAX_ORDERS_PER_DAY: usize = 20;

/// How long an order is kept after it was made.
const KEEP_SECS: u64 = 30 * 24 * 3600;

/// How long a status read from Binance is reused before asking again.
const REFRESH_SECS: u64 = 10;

/// How far a webhook's timestamp may be from this edge's clock.
const WEBHOOK_SKEW_MS: u64 = 10 * 60 * 1000;

const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum OnrampError {
    /// No Binance partner configured here, or its credentials are missing.
    #[error("buying USDC with euros is not offered by this edge")]
    Unconfigured,
    #[error("{0}")]
    Invalid(&'static str),
    #[error("no such order")]
    NotFound,
    #[error("{0}")]
    Busy(&'static str),
    /// Binance answered, and said no. Its message, when it is plain text.
    #[error("Binance declined the order: {0}")]
    Declined(String),
    /// Binance could not be reached or answered something unreadable. The
    /// detail is for the log.
    #[error("Binance: {0}")]
    Upstream(String),
    #[error("bad webhook signature")]
    BadSignature,
    #[error("onramp store: {0}")]
    Store(String),
}

/// `serve --onramp-binance-*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnrampOpts {
    /// Binance's API origin for this partner, `https://…` (plain `http` only
    /// to loopback, for a stand-in). Binance hands it out with the account.
    pub api_url: String,
    /// `X-Tesla-ClientId`.
    pub client_id: String,
    /// The partner's RSA private key, PKCS#8, base64 or PEM: a credential
    /// file.
    pub key_file: String,
    /// `X-Tesla-SignAccessToken`: a credential file.
    pub token_file: String,
    /// Binance's webhook public key, DER (`SubjectPublicKeyInfo` or PKCS#1),
    /// already decoded.
    pub webhook_key: Vec<u8>,
    /// `onramp.json`.
    pub state_file: String,
}

/// Whether `url` may be Binance's API origin: `https://host[:port]`, or
/// `http://` to loopback for a stand-in. No path, no userinfo.
#[must_use]
pub fn api_url_ok(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let loopback = matches!(parsed.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    let scheme_ok = parsed.scheme() == "https" || (parsed.scheme() == "http" && loopback);
    scheme_ok
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.host_str().is_some()
        && matches!(parsed.path(), "" | "/")
        && parsed.query().is_none()
        && parsed.fragment().is_none()
}

/// The webhook key from its base64 text: Binance's `SubjectPublicKeyInfo`,
/// or a bare PKCS#1 `RSAPublicKey`, as the DER ring verifies with.
#[must_use]
pub fn parse_public_key(text: &str) -> Option<Vec<u8>> {
    let der = crate::solana_escrow::base64_decode(&strip_pem(text))?;
    let key = spki_rsa_key(&der).unwrap_or(&der);
    rsa_public_key(key).then(|| key.to_vec())
}

/// A PKCS#1 RSAPublicKey: a SEQUENCE of the modulus, 1024 bits or more, and
/// the exponent, with nothing after it.
fn rsa_public_key(key: &[u8]) -> bool {
    let Some((0x30, body, [])) = der(key) else {
        return false;
    };
    let Some((0x02, modulus, rest)) = der(body) else {
        return false;
    };
    let Some((0x02, exponent, [])) = der(rest) else {
        return false;
    };
    modulus.len() >= 128 && !exponent.is_empty()
}

/// The base64 inside a PEM block, or the text itself, without whitespace.
fn strip_pem(text: &str) -> String {
    text.lines()
        .filter(|l| !l.trim_start().starts_with("-----"))
        .flat_map(|l| l.chars().filter(|c| !c.is_whitespace()))
        .collect()
}

/// One DER element: its tag, its contents and what follows it.
fn der(input: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&tag, rest) = input.split_first()?;
    let (&first, rest) = rest.split_first()?;
    let (len, rest) = if first < 0x80 {
        (usize::from(first), rest)
    } else {
        let n = usize::from(first & 0x7f);
        if n == 0 || n > 3 || rest.len() < n {
            return None;
        }
        let len = rest[..n]
            .iter()
            .fold(0usize, |acc, b| (acc << 8) | usize::from(*b));
        (len, &rest[n..])
    };
    (rest.len() >= len).then(|| (tag, &rest[..len], &rest[len..]))
}

/// The `RSAPublicKey` inside a `SubjectPublicKeyInfo`, if `der_bytes` is one.
fn spki_rsa_key(der_bytes: &[u8]) -> Option<&[u8]> {
    let (0x30, spki, _) = der(der_bytes)? else {
        return None;
    };
    let (0x30, _algorithm, rest) = der(spki)? else {
        return None;
    };
    let (0x03, bits, _) = der(rest)? else {
        return None;
    };
    let (0, key) = bits.split_first()? else {
        return None;
    };
    Some(key)
}

/// What an order is doing, in the words the box shows. Binance's numbers
/// (`basics/7.order-status`) map onto them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    /// Made; the buyer has not paid on Binance's page yet.
    Open,
    /// Binance is taking the payment.
    Paying,
    /// Bought; Binance is withdrawing the USDC to the wallet.
    Sending,
    /// The USDC left Binance for the wallet.
    Done,
    /// Bought, but the withdrawal did not happen: the USDC is in the buyer's
    /// Binance account.
    Kept,
    /// Nothing was bought.
    Failed,
}

impl Stage {
    #[must_use]
    pub fn from_status(status: i64) -> Self {
        match status {
            0 => Stage::Open,
            1 | 6 => Stage::Paying,
            2..=4 | 10 | 11 | 15 => Stage::Sending,
            20 => Stage::Done,
            93 | 96 | 98 => Stage::Kept,
            _ => Stage::Failed,
        }
    }

    #[must_use]
    pub fn is_final(self) -> bool {
        matches!(self, Stage::Done | Stage::Kept | Stage::Failed)
    }
}

/// One order, as kept in `onramp.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub appliance_id: String,
    pub address: String,
    pub euros: String,
    pub created: u64,
    pub link: String,
    /// Unix milliseconds Binance gave for the link.
    pub link_expires: u64,
    /// Binance's status number; 0 until it says otherwise.
    pub status: i64,
    #[serde(default)]
    pub usdc: Option<String>,
    #[serde(default)]
    pub tx: Option<String>,
    /// When the status was last read or pushed.
    #[serde(default)]
    pub checked: u64,
}

/// What a box gets for one order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OrderView {
    pub order_id: String,
    pub link: String,
    pub link_expires: u64,
    pub euros: String,
    pub address: String,
    pub network: &'static str,
    pub stage: Stage,
    pub status: i64,
    pub usdc: Option<String>,
    pub tx: Option<String>,
}

impl Order {
    fn view(&self, id: &str) -> OrderView {
        OrderView {
            order_id: id.to_string(),
            link: self.link.clone(),
            link_expires: self.link_expires,
            euros: self.euros.clone(),
            address: self.address.clone(),
            network: "base",
            stage: Stage::from_status(self.status),
            status: self.status,
            usdc: self.usdc.clone(),
            tx: self.tx.clone(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Store {
    orders: BTreeMap<String, Order>,
}

pub struct Onramp {
    opts: OnrampOpts,
    http: reqwest::Client,
    rng: SystemRandom,
    store: Mutex<Store>,
}

/// `0x` and 40 hex digits, lowercased.
#[must_use]
pub fn normalize_address(raw: &str) -> Option<String> {
    let hex = raw.trim().strip_prefix("0x")?;
    (hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| format!("0x{}", hex.to_ascii_lowercase()))
}

/// A euro amount with at most two decimals, in range, as Binance's
/// `requestedAmount`: no leading zeros, no trailing ".00".
#[must_use]
pub fn normalize_euros(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let (whole, cents) = raw.split_once('.').unwrap_or((raw, ""));
    if whole.is_empty()
        || whole.len() > 6
        || cents.len() > 2
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !cents.bytes().all(|b| b.is_ascii_digit())
        || (raw.contains('.') && cents.is_empty())
    {
        return None;
    }
    let whole: u64 = whole.parse().ok()?;
    let cents: u64 = format!("{cents:0<2}").parse().ok()?;
    let total = whole * 100 + cents;
    if !(MIN_EUROS * 100..=MAX_EUROS * 100).contains(&total) {
        return None;
    }
    Some(match total % 100 {
        0 => (total / 100).to_string(),
        c if c % 10 == 0 => format!("{}.{}", total / 100, c / 10),
        c => format!("{}.{c:02}", total / 100),
    })
}

/// A checkout link Binance may send the buyer to: https on binance.com.
#[must_use]
pub fn binance_link(link: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(link) else {
        return false;
    };
    url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url
            .host_str()
            .is_some_and(|h| h == "binance.com" || h.ends_with(".binance.com"))
}

/// Binance's message, when it is short plain text a box may show.
fn plain(message: &str) -> Option<String> {
    let m = message.trim();
    (!m.is_empty() && m.len() <= 200 && m.chars().all(|c| c.is_ascii_graphic() || c == ' '))
        .then(|| m.to_string())
}

fn now_ms() -> u64 {
    crate::market::now_secs() * 1000
}

impl Onramp {
    /// # Errors
    /// The store exists and does not parse.
    pub async fn open(opts: OnrampOpts) -> Result<Self, OnrampError> {
        let store = match tokio::fs::read(&opts.state_file).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| OnrampError::Store(format!("{}: {e}", opts.state_file)))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Store::default(),
            Err(e) => return Err(OnrampError::Store(format!("{}: {e}", opts.state_file))),
        };
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(concat!("losos-registrar/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| OnrampError::Store(e.to_string()))?;
        Ok(Self {
            opts,
            http,
            rng: SystemRandom::new(),
            store: Mutex::new(store),
        })
    }

    async fn save(&self, store: &Store) -> Result<(), OnrampError> {
        let bytes = serde_json::to_vec(store).map_err(|e| OnrampError::Store(e.to_string()))?;
        crate::fsutil::atomic_write(Path::new(&self.opts.state_file), &bytes, 0o600)
            .await
            .map_err(|e| OnrampError::Store(format!("{}: {e}", self.opts.state_file)))
    }

    /// Whether the partner's key and token are both sealed. The account view
    /// offers the purchase only then, so a box never shows a row that can
    /// only answer 503.
    pub async fn configured(&self) -> bool {
        Self::credential(&self.opts.key_file).await.is_ok()
            && Self::credential(&self.opts.token_file).await.is_ok()
    }

    async fn credential(path: &str) -> Result<String, OnrampError> {
        let text = tokio::fs::read_to_string(PathBuf::from(path))
            .await
            .map_err(|_| OnrampError::Unconfigured)?;
        let text = text.trim();
        // The fallback systemd sets when nothing was sealed yet.
        if text.is_empty() || text == "unsealed" {
            return Err(OnrampError::Unconfigured);
        }
        Ok(text.to_string())
    }

    /// One signed call to Binance: `data` of a successful answer.
    async fn call(&self, path: &str, body: &Value) -> Result<Value, OnrampError> {
        let key = Self::credential(&self.opts.key_file).await?;
        let token = Self::credential(&self.opts.token_file).await?;
        let der = crate::solana_escrow::base64_decode(&strip_pem(&key))
            .ok_or(OnrampError::Unconfigured)?;
        // PKCS#8 (`BEGIN PRIVATE KEY`) or PKCS#1 (`BEGIN RSA PRIVATE KEY`).
        let pair = RsaKeyPair::from_pkcs8(&der)
            .or_else(|_| RsaKeyPair::from_der(&der))
            .map_err(|_| OnrampError::Unconfigured)?;
        let body = body.to_string();
        let timestamp = now_ms().to_string();
        let mut sig = vec![0; pair.public().modulus_len()];
        pair.sign(
            &signature::RSA_PKCS1_SHA256,
            &self.rng,
            format!("{body}{timestamp}").as_bytes(),
            &mut sig,
        )
        .map_err(|_| OnrampError::Upstream("signing failed".into()))?;
        let url = format!("{}{path}", self.opts.api_url.trim_end_matches('/'));
        let reply = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-Tesla-ClientId", &self.opts.client_id)
            .header("X-Tesla-SignAccessToken", token)
            .header("X-Tesla-Timestamp", &timestamp)
            .header(
                "X-Tesla-Signature",
                crate::solana_escrow::base64_encode(&sig),
            )
            .body(body)
            .send()
            .await
            .map_err(|e| OnrampError::Upstream(format!("{path}: {e}")))?;
        let status = reply.status();
        let doc: Value = reply
            .json()
            .await
            .map_err(|e| OnrampError::Upstream(format!("{path}: {status}: {e}")))?;
        if doc["success"].as_bool() != Some(true) {
            let code = doc["code"].as_str().unwrap_or("");
            tracing::warn!(
                target: crate::action::Action::Market.target(),
                "Binance refused {path}: {status} {code} {}",
                doc["message"].as_str().unwrap_or(""),
            );
            return Err(match doc["message"].as_str().and_then(plain) {
                Some(m) => OnrampError::Declined(m),
                None => OnrampError::Upstream(format!("{path}: {status} {code}")),
            });
        }
        Ok(doc["data"].clone())
    }

    /// A new order for `euros` of USDC on Base to `address`, made for
    /// `appliance_id`.
    ///
    /// # Errors
    /// A bad address or amount, too many orders today, or Binance saying no.
    pub async fn create(
        &self,
        appliance_id: &str,
        address: &str,
        euros: &str,
    ) -> Result<OrderView, OnrampError> {
        let address = normalize_address(address)
            .ok_or(OnrampError::Invalid("address is not an 0x address"))?;
        let euros = normalize_euros(euros).ok_or(OnrampError::Invalid(
            "euros must be an amount from 1 to 100000 with at most two decimals",
        ))?;
        let now = crate::market::now_secs();
        {
            let store = self.store.lock().await;
            let today = store
                .orders
                .values()
                .filter(|o| {
                    o.appliance_id == appliance_id && now.saturating_sub(o.created) < 86_400
                })
                .count();
            if today >= MAX_ORDERS_PER_DAY {
                return Err(OnrampError::Busy(
                    "this box started too many purchases today; try again tomorrow",
                ));
            }
        }
        let mut raw = [0u8; 16];
        self.rng
            .fill(&mut raw)
            .map_err(|_| OnrampError::Store("no randomness".into()))?;
        let id: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        let data = self
            .call(
                PRE_ORDER,
                &json!({
                    "externalOrderId": id,
                    "fiatCurrency": FIAT,
                    "cryptoCurrency": CRYPTO,
                    "amountType": 1,
                    "requestedAmount": euros,
                    "network": NETWORK,
                    "address": address,
                }),
            )
            .await?;
        let link = data["link"].as_str().unwrap_or("");
        if !binance_link(link) {
            return Err(OnrampError::Upstream(
                "the pre-order answered no binance.com link".into(),
            ));
        }
        let order = Order {
            appliance_id: appliance_id.to_string(),
            address,
            euros,
            created: now,
            link: link.to_string(),
            link_expires: data["linkExpireTime"].as_u64().unwrap_or(0),
            status: 0,
            usdc: None,
            tx: None,
            // Never read yet: the first status call asks Binance.
            checked: 0,
        };
        let view = order.view(&id);
        let mut store = self.store.lock().await;
        store
            .orders
            .retain(|_, o| now.saturating_sub(o.created) < KEEP_SECS);
        store.orders.insert(id, order);
        self.save(&store).await?;
        Ok(view)
    }

    /// Where `order_id` is, for the box that made it. Asks Binance unless
    /// it answered a moment ago or the order is over.
    ///
    /// # Errors
    /// Unknown, or another box's order.
    pub async fn status(
        &self,
        appliance_id: &str,
        order_id: &str,
    ) -> Result<OrderView, OnrampError> {
        let order = {
            let store = self.store.lock().await;
            store
                .orders
                .get(order_id)
                .filter(|o| o.appliance_id == appliance_id)
                .cloned()
                .ok_or(OnrampError::NotFound)?
        };
        let now = crate::market::now_secs();
        if Stage::from_status(order.status).is_final()
            || now.saturating_sub(order.checked) < REFRESH_SECS
        {
            return Ok(order.view(order_id));
        }
        let data = match self
            .call(ORDER, &json!({ "externalOrderId": order_id }))
            .await
        {
            Ok(data) => data,
            // The last known status is still the truth for the page.
            Err(OnrampError::Upstream(e)) => {
                tracing::warn!(target: crate::action::Action::Market.target(), "order {order_id}: {e}");
                return Ok(order.view(order_id));
            }
            Err(e) => return Err(e),
        };
        self.update(order_id, &data, now).await
    }

    /// Fold an order document from Binance (an order query's `data` or a
    /// webhook) into the store.
    async fn update(
        &self,
        order_id: &str,
        data: &Value,
        now: u64,
    ) -> Result<OrderView, OnrampError> {
        let mut store = self.store.lock().await;
        let order = store
            .orders
            .get_mut(order_id)
            .ok_or(OnrampError::NotFound)?;
        if data["externalOrderId"]
            .as_str()
            .is_some_and(|id| id != order_id)
        {
            return Err(OnrampError::Upstream("an answer for another order".into()));
        }
        // A withdrawal anywhere but the order's own wallet is not this order.
        if let Some(to) = data["withdrawWalletAddress"]
            .as_str()
            .filter(|a| !a.is_empty())
        {
            if normalize_address(to).as_deref() != Some(order.address.as_str()) {
                return Err(OnrampError::Upstream(
                    "Binance names a different withdrawal address".into(),
                ));
            }
        }
        if let Some(status) = data["status"].as_i64() {
            // A late update never moves a finished order back.
            if !Stage::from_status(order.status).is_final() {
                order.status = status;
            }
        }
        if let Some(amount) = data["cryptoAmount"].as_str().filter(|a| decimal(a)) {
            order.usdc = Some(amount.to_string());
        }
        if let Some(tx) = data["withdrawTxHash"]
            .as_str()
            .and_then(crate::stablecoin::normalize_tx)
        {
            order.tx = Some(tx);
        }
        order.checked = now;
        let view = order.view(order_id);
        self.save(&store).await?;
        Ok(view)
    }

    /// Binance's push of an order's new status. `body` and the two headers
    /// as they arrived.
    ///
    /// # Errors
    /// A signature that does not verify, a stale timestamp, or a body that
    /// is not an order event.
    pub async fn webhook(
        &self,
        client: &str,
        timestamp: &str,
        sig: &str,
        body: &[u8],
    ) -> Result<(), OnrampError> {
        if client != self.opts.client_id {
            return Err(OnrampError::BadSignature);
        }
        let at: u64 = timestamp.parse().map_err(|_| OnrampError::BadSignature)?;
        if now_ms().abs_diff(at) > WEBHOOK_SKEW_MS {
            return Err(OnrampError::BadSignature);
        }
        let sig =
            crate::solana_escrow::base64_decode(sig.trim()).ok_or(OnrampError::BadSignature)?;
        let mut signed = body.to_vec();
        signed.extend_from_slice(timestamp.as_bytes());
        signature::UnparsedPublicKey::new(
            &signature::RSA_PKCS1_1024_8192_SHA256_FOR_LEGACY_USE_ONLY,
            &self.opts.webhook_key,
        )
        .verify(&signed, &sig)
        .map_err(|_| OnrampError::BadSignature)?;
        let doc: Value =
            serde_json::from_slice(body).map_err(|_| OnrampError::Invalid("not JSON"))?;
        if doc["webhookEventType"].as_str() != Some("connect_order_event") {
            return Ok(());
        }
        let Some(id) = doc["externalOrderId"].as_str() else {
            return Err(OnrampError::Invalid("no externalOrderId"));
        };
        match self.update(id, &doc, crate::market::now_secs()).await {
            // Not one of ours (another edge of the same partner, or a
            // forgotten one): nothing to do, and Binance need not retry.
            Ok(_) | Err(OnrampError::NotFound) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

/// A decimal amount as Binance writes one.
fn decimal(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        && s.bytes().filter(|b| *b == b'.').count() <= 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_purchase_is_offered_only_once_key_and_token_are_sealed() {
        let dir = std::env::temp_dir().join(format!("losos-onramp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = |name: &str| dir.join(name).to_string_lossy().into_owned();
        let onramp = Onramp::open(OnrampOpts {
            api_url: "https://api.example".into(),
            client_id: "client".into(),
            key_file: path("key"),
            token_file: path("token"),
            webhook_key: vec![],
            state_file: path("onramp.json"),
        })
        .await
        .unwrap();
        // Missing files, then systemd's fallback word, then real values.
        assert!(!onramp.configured().await);
        std::fs::write(path("key"), "unsealed").unwrap();
        std::fs::write(path("token"), "unsealed").unwrap();
        assert!(!onramp.configured().await);
        std::fs::write(path("key"), "-----BEGIN PRIVATE KEY-----\nAAAA\n").unwrap();
        assert!(!onramp.configured().await, "the token is still unsealed");
        std::fs::write(path("token"), "token\n").unwrap();
        assert!(onramp.configured().await);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn amounts_are_euros_with_two_decimals_in_range() {
        assert_eq!(normalize_euros("50").as_deref(), Some("50"));
        assert_eq!(normalize_euros(" 050.00 ").as_deref(), Some("50"));
        assert_eq!(normalize_euros("25.5").as_deref(), Some("25.5"));
        assert_eq!(normalize_euros("25.05").as_deref(), Some("25.05"));
        assert_eq!(normalize_euros("100000").as_deref(), Some("100000"));
        for bad in [
            "",
            "0.99",
            "100000.01",
            "1,5",
            "1.",
            ".5",
            "1.234",
            "-5",
            "1e3",
            "abc",
        ] {
            assert_eq!(normalize_euros(bad), None, "{bad}");
        }
    }

    #[test]
    fn addresses_are_evm_addresses() {
        assert_eq!(
            normalize_address("0xF39Fd6e51aad88F6F4ce6aB8827279cffFb92266").as_deref(),
            Some("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266")
        );
        assert_eq!(
            normalize_address("f39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
            None
        );
        assert_eq!(normalize_address("0x1234"), None);
    }

    #[test]
    fn only_binance_checkout_links_are_handed_on() {
        assert!(binance_link(
            "https://app.binance.com/uni-qr/qccnt?merchantCode=x"
        ));
        assert!(binance_link("https://www.binance.com/en/connect"));
        for bad in [
            "http://app.binance.com/x",
            "https://binance.com.evil.example/x",
            "https://evilbinance.com/x",
            "https://user@app.binance.com/x",
            "https://app.binance.com:8443/x",
            "javascript:alert(1)",
        ] {
            assert!(!binance_link(bad), "{bad}");
        }
    }

    #[test]
    fn the_api_origin_is_https_or_a_loopback_stand_in() {
        assert!(api_url_ok("https://api.example.binance.com"));
        assert!(api_url_ok("http://127.0.0.1:12112"));
        assert!(!api_url_ok("http://api.example.com"));
        assert!(!api_url_ok("https://key@api.example.com"));
        assert!(!api_url_ok("https://api.example.com/papi"));
    }

    #[test]
    fn binance_statuses_map_to_what_the_box_says() {
        assert_eq!(Stage::from_status(0), Stage::Open);
        assert_eq!(Stage::from_status(1), Stage::Paying);
        assert_eq!(Stage::from_status(2), Stage::Sending);
        assert_eq!(Stage::from_status(11), Stage::Sending);
        assert_eq!(Stage::from_status(20), Stage::Done);
        assert_eq!(Stage::from_status(96), Stage::Kept);
        assert_eq!(Stage::from_status(97), Stage::Failed);
        assert!(Stage::Done.is_final() && Stage::Kept.is_final() && Stage::Failed.is_final());
        assert!(!Stage::Sending.is_final());
    }

    #[test]
    fn a_der_walk_finds_the_rsa_key_in_an_spki() {
        // SEQUENCE { SEQUENCE { OID, NULL }, BIT STRING { 0, SEQUENCE {} } }
        let spki = [
            0x30, 0x0e, 0x30, 0x07, 0x06, 0x03, 0x2a, 0x86, 0x48, 0x05, 0x00, 0x03, 0x03, 0x00,
            0x30, 0x00,
        ];
        assert_eq!(spki_rsa_key(&spki), Some(&[0x30, 0x00][..]));
        assert_eq!(spki_rsa_key(&[0x30, 0x05, 0x00]), None);
        // An empty SEQUENCE is no key; a 1024-bit modulus and an exponent are.
        assert!(!rsa_public_key(&[0x30, 0x00]));
        let mut key = vec![0x30, 0x81, 0x87, 0x02, 0x81, 0x81, 0x00];
        key.extend([0xc5; 128]);
        key.extend([0x02, 0x01, 0x03]);
        assert!(rsa_public_key(&key));
        key.push(0x00);
        assert!(!rsa_public_key(&key), "trailing bytes");
        assert!(parse_public_key("not a key").is_none());
    }
}
