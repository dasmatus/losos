//! Bitcoin, Monero and shielded Zcash orders: the parts that do not depend on
//! whose address a payment lands on.
//!
//! These coins move in price, so an order in one of them is a quote: the USD
//! price of the order turned into an exact amount of the coin at a price two
//! independent sources agree on, held for [`QUOTE_SECS`]. The edge watches the
//! order's own address with its own node, adds up every payment to it, and
//! hands the order out once the payments that cover it are deep enough for the
//! order's value ([`confirmations_for`]). [`settle`] turns what the node saw
//! into one of the states the buyer is shown.
//!
//! Money never goes through a float: USD is whole cents, a price is
//! micro-dollars per whole coin, and an amount is the coin's smallest unit
//! (satoshi, piconero, zatoshi).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How long a quote holds. A payment first seen inside the window counts at
/// the quoted amount however long it then waits for its blocks.
pub const QUOTE_SECS: u64 = 15 * 60;

/// The most two price sources may differ, in basis points of the lower one,
/// before the edge refuses to quote at all.
pub const MAX_SPREAD_BPS: u128 = 100;

/// How far under a fresh quote a payment that arrived after its quote lapsed
/// may be and still complete the order, in basis points.
pub const LATE_TOLERANCE_BPS: u128 = 100;

const PRICE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Coin {
    Btc,
    Xmr,
    Zec,
    /// Priced only, for the network fee of a USDC payment on Base, Arbitrum
    /// and OP. Orders are never quoted in it.
    Eth,
    /// Priced only, for the network fee of a USDC payment on Solana.
    Sol,
}

impl Coin {
    /// The coins an order can be quoted in.
    pub const ORDERS: [Coin; 3] = [Coin::Btc, Coin::Xmr, Coin::Zec];
    /// The coins USDC payments spend on network fees.
    pub const FEES: [Coin; 2] = [Coin::Eth, Coin::Sol];

    /// Digits in the smallest unit: satoshi, piconero, zatoshi.
    pub fn decimals(self) -> u32 {
        match self {
            Coin::Btc | Coin::Zec => 8,
            Coin::Xmr => 12,
            Coin::Eth => 18,
            Coin::Sol => 9,
        }
    }

    pub fn ticker(self) -> &'static str {
        match self {
            Coin::Btc => "BTC",
            Coin::Xmr => "XMR",
            Coin::Zec => "ZEC",
            Coin::Eth => "ETH",
            Coin::Sol => "SOL",
        }
    }

    /// The key the browser reads a price under.
    pub fn key(self) -> &'static str {
        match self {
            Coin::Btc => "btc",
            Coin::Xmr => "xmr",
            Coin::Zec => "zec",
            Coin::Eth => "eth",
            Coin::Sol => "sol",
        }
    }

    /// CoinGecko's id for the coin.
    fn gecko_id(self) -> &'static str {
        match self {
            Coin::Btc => "bitcoin",
            Coin::Xmr => "monero",
            Coin::Zec => "zcash",
            Coin::Eth => "ethereum",
            Coin::Sol => "solana",
        }
    }

    /// Kraken's name for the coin's USD pair.
    fn kraken_pair(self) -> &'static str {
        match self {
            Coin::Btc => "XBTUSD",
            Coin::Xmr => "XMRUSD",
            Coin::Zec => "ZECUSD",
            Coin::Eth => "ETHUSD",
            Coin::Sol => "SOLUSD",
        }
    }

    /// An order coin by name. ETH and SOL are priced, never ordered in.
    pub fn parse(text: &str) -> Option<Coin> {
        match text {
            "btc" | "bitcoin" => Some(Coin::Btc),
            "xmr" | "monero" => Some(Coin::Xmr),
            "zec" | "zcash" => Some(Coin::Zec),
            _ => None,
        }
    }
}

// ── Prices ────────────────────────────────────────────────────────────────

/// Where a price comes from. Each is a public endpoint read without a key;
/// the base URL is configurable so a test can stand one in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PriceSource {
    /// `GET {base}/0/public/Ticker?pair=XBTUSD`, the last trade price.
    Kraken { base: String },
    /// `GET {base}/api/v3/simple/price?ids=bitcoin&vs_currencies=usd`.
    CoinGecko { base: String },
}

impl PriceSource {
    pub fn kraken() -> PriceSource {
        PriceSource::Kraken {
            base: "https://api.kraken.com".to_string(),
        }
    }

    pub fn coingecko() -> PriceSource {
        PriceSource::CoinGecko {
            base: "https://api.coingecko.com".to_string(),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            PriceSource::Kraken { .. } => "Kraken",
            PriceSource::CoinGecko { .. } => "CoinGecko",
        }
    }

    fn url(&self, coin: Coin) -> String {
        match self {
            PriceSource::Kraken { base } => format!(
                "{}/0/public/Ticker?pair={}",
                base.trim_end_matches('/'),
                coin.kraken_pair()
            ),
            PriceSource::CoinGecko { base } => {
                let id = coin.gecko_id();
                format!(
                    "{}/api/v3/simple/price?ids={id}&vs_currencies=usd",
                    base.trim_end_matches('/')
                )
            }
        }
    }

    /// The price in micro-dollars per whole coin from this source's reply.
    pub fn read(&self, coin: Coin, reply: &Value) -> Option<u128> {
        let text = match self {
            // {"error":[],"result":{"XXBTZUSD":{"c":["67123.40000","0.01"],…}}}
            // The result key is Kraken's own pair name, so take the only one.
            PriceSource::Kraken { .. } => {
                if reply
                    .get("error")
                    .and_then(Value::as_array)
                    .is_some_and(|e| !e.is_empty())
                {
                    return None;
                }
                let result = reply.get("result")?.as_object()?;
                if result.len() != 1 {
                    return None;
                }
                let pair = result.values().next()?;
                pair.get("c")?.get(0)?.as_str()?.to_string()
            }
            // {"bitcoin":{"usd":67123.4}}
            PriceSource::CoinGecko { .. } => match reply.get(coin.gecko_id())?.get("usd")? {
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.clone(),
                _ => return None,
            },
        };
        parse_micros(&text).filter(|&p| p > 0)
    }

    pub async fn fetch(&self, coin: Coin) -> Result<u128, PriceError> {
        let client = reqwest::Client::builder()
            .timeout(PRICE_TIMEOUT)
            .build()
            .map_err(|_| PriceError::Unreachable(self.name()))?;
        let reply: Value = client
            .get(self.url(coin))
            .header("accept", "application/json")
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|_| PriceError::Unreachable(self.name()))?
            .json()
            .await
            .map_err(|_| PriceError::Unreadable(self.name()))?;
        self.read(coin, &reply)
            .ok_or(PriceError::Unreadable(self.name()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PriceError {
    #[error("{0} did not answer for a price")]
    Unreachable(&'static str),
    #[error("{0} answered without a price")]
    Unreadable(&'static str),
    #[error("the price sources disagree by more than 1%")]
    Disagree,
    #[error("a quote needs two price sources")]
    TooFew,
}

/// "67123.45" → 67_123_450_000 micro-dollars. Digits past the sixth decimal
/// are cut, not rounded: a price is never read as higher than it was.
pub fn parse_micros(text: &str) -> Option<u128> {
    let text = text.trim();
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole.bytes().all(|b| b.is_ascii_digit()) || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if whole.len() > 15 {
        return None;
    }
    let whole: u128 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let mut micros = whole * 1_000_000;
    for (i, digit) in fraction.bytes().take(6).enumerate() {
        micros += u128::from(digit - b'0') * 10u128.pow(5 - i as u32);
    }
    Some(micros)
}

/// One price from each source, or the reason there is none. Two agreeing
/// prices give their mean; one, or two more than [`MAX_SPREAD_BPS`] apart,
/// give no quote at all.
pub fn agree(prices: &[u128]) -> Result<u128, PriceError> {
    let [a, b] = prices else {
        return Err(PriceError::TooFew);
    };
    let (low, high) = if a <= b { (*a, *b) } else { (*b, *a) };
    if low == 0 {
        return Err(PriceError::TooFew);
    }
    if (high - low) * 10_000 > low * MAX_SPREAD_BPS {
        return Err(PriceError::Disagree);
    }
    Ok((low + high) / 2)
}

/// Ask both sources at once and agree on a price.
pub async fn price(sources: &[PriceSource; 2], coin: Coin) -> Result<u128, PriceError> {
    let (a, b) = tokio::join!(sources[0].fetch(coin), sources[1].fetch(coin));
    agree(&[a?, b?])
}

/// How long a fetched price is shown before it counts as stale.
pub const PRICE_MAX_AGE: u64 = 120;

/// The last agreed price per coin, refreshed in the background so a page
/// load never waits on a price source.
pub struct PriceBook {
    sources: [PriceSource; 2],
    cache: std::sync::Mutex<std::collections::HashMap<Coin, (u128, u64)>>,
    refreshing: std::sync::atomic::AtomicBool,
}

impl PriceBook {
    pub fn new(sources: [PriceSource; 2]) -> PriceBook {
        PriceBook {
            sources,
            cache: std::sync::Mutex::new(std::collections::HashMap::new()),
            refreshing: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// The price fetched less than [`PRICE_MAX_AGE`] ago, if any.
    pub fn cached(&self, coin: Coin, now: u64) -> Option<u128> {
        let cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        cache
            .get(&coin)
            .filter(|(_, at)| now.saturating_sub(*at) < PRICE_MAX_AGE)
            .map(|(price, _)| *price)
    }

    /// Fresh prices for `coins`, keyed for the browser, in micro-dollars as
    /// text (a JSON number would lose digits past 2^53).
    pub fn snapshot(
        &self,
        coins: &[Coin],
        now: u64,
    ) -> std::collections::BTreeMap<&'static str, String> {
        coins
            .iter()
            .filter_map(|&c| self.cached(c, now).map(|p| (c.key(), p.to_string())))
            .collect()
    }

    /// Fetch every coin in `coins` now; a coin whose sources fail or
    /// disagree keeps no price.
    pub async fn refresh(&self, coins: &[Coin], now: u64) {
        for &coin in coins {
            match price(&self.sources, coin).await {
                Ok(p) => {
                    self.cache
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .insert(coin, (p, now));
                }
                Err(e) => tracing::warn!("no {} price: {e}", coin.ticker()),
            }
        }
    }

    /// Start a refresh unless one is running or every coin is fresh.
    pub fn refresh_in_background(self: &std::sync::Arc<Self>, coins: &'static [Coin], now: u64) {
        use std::sync::atomic::Ordering;
        if coins.iter().all(|&c| self.cached(c, now).is_some()) {
            return;
        }
        if self.refreshing.swap(true, Ordering::AcqRel) {
            return;
        }
        let book = std::sync::Arc::clone(self);
        tokio::spawn(async move {
            book.refresh(coins, now).await;
            book.refreshing.store(false, Ordering::Release);
        });
    }
}

// ── Quotes ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    pub coin: Coin,
    /// What the order costs, in US cents.
    pub usd_cents: u64,
    /// The agreed price, micro-dollars per whole coin.
    pub price_micros: u128,
    /// What the buyer pays, in the coin's smallest unit.
    pub amount: u128,
    pub quoted_at: u64,
    pub pay_before: u64,
}

impl Quote {
    pub fn new(coin: Coin, usd_cents: u64, price_micros: u128, now: u64) -> Option<Quote> {
        Some(Quote {
            coin,
            usd_cents,
            price_micros,
            amount: amount_for(coin, usd_cents, price_micros)?,
            quoted_at: now,
            pay_before: now + QUOTE_SECS,
        })
    }
}

/// The coin amount for `usd_cents` at `price_micros`, rounded up to the next
/// smallest unit so the seller is never short by a rounding.
pub fn amount_for(coin: Coin, usd_cents: u64, price_micros: u128) -> Option<u128> {
    if price_micros == 0 || usd_cents == 0 {
        return None;
    }
    let micros = u128::from(usd_cents) * 10_000;
    let scaled = micros.checked_mul(10u128.pow(coin.decimals()))?;
    Some(scaled.div_ceil(price_micros))
}

/// The USD value of `amount` at `price_micros`, in whole cents, rounded down.
pub fn cents_for(coin: Coin, amount: u128, price_micros: u128) -> u128 {
    amount.saturating_mul(price_micros) / 10u128.pow(coin.decimals()) / 10_000
}

/// "0.00150000" for 150_000 satoshi: every digit of the smallest unit, as
/// payment URIs and wallets expect.
pub fn format_amount(coin: Coin, amount: u128) -> String {
    let scale = 10u128.pow(coin.decimals());
    format!(
        "{}.{:0width$}",
        amount / scale,
        amount % scale,
        width = coin.decimals() as usize
    )
}

// ── Confirmations ─────────────────────────────────────────────────────────

/// How many blocks must sit on a payment before the order is handed out,
/// growing with what the order is worth: a reorg or a replaced transaction
/// has to undo more work the more is at stake.
///
/// Bitcoin blocks come every ten minutes, so one block is enough for a small
/// order and six for a large one. Zcash blocks come every 75 seconds, so it
/// asks for more of them for the same time. Monero cannot spend an output for
/// ten blocks anyway, so ten is its floor.
pub fn confirmations_for(coin: Coin, usd_cents: u64) -> u64 {
    let dollars = usd_cents / 100;
    match coin {
        Coin::Btc => match dollars {
            0..=50 => 1,
            51..=500 => 2,
            501..=5_000 => 3,
            _ => 6,
        },
        Coin::Zec => match dollars {
            0..=50 => 3,
            51..=500 => 6,
            501..=5_000 => 10,
            _ => 24,
        },
        Coin::Xmr => match dollars {
            0..=5_000 => 10,
            _ => 20,
        },
        // Never an order coin; a USDC payment's depth is the stablecoin
        // module's.
        Coin::Eth | Coin::Sol => u64::MAX,
    }
}

// ── What the node saw, and what the buyer is told ─────────────────────────

/// One payment to the order's address as the node reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Received {
    /// The transaction, for the explorer link where the chain has one.
    pub tx: String,
    pub amount: u128,
    /// 0 while it is in the mempool.
    pub confirmations: u64,
    /// When the node first saw it, mempool included.
    pub first_seen: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Settlement {
    /// Nothing has reached the address.
    Waiting,
    /// Enough was sent, none of it is in a block yet. The order does not
    /// lapse while its payment waits in the mempool.
    Seen { paid: u128 },
    /// Less than the quote reached the address. "Send the rest to the same
    /// address."
    Underpaid { paid: u128, due: u128 },
    /// Enough was sent; the shallowest payment needed has `confirmations` of
    /// `required` blocks.
    Confirming {
        paid: u128,
        confirmations: u64,
        required: u64,
    },
    /// The order is paid. `extra` is anything over the quote, recorded for a
    /// refund by a person.
    Paid { paid: u128, extra: u128 },
    /// The payment came after the quote lapsed and is more than 1% short of
    /// a fresh quote: offer the current price or a refund.
    Late { paid: u128, due_now: u128 },
    /// The payment came after the quote lapsed and the price sources are not
    /// answering, so there is nothing to measure it against yet.
    Repricing { paid: u128 },
}

impl Settlement {
    pub fn is_paid(&self) -> bool {
        matches!(self, Settlement::Paid { .. })
    }
}

/// The state of an order from the payments its address received.
///
/// Only payments first seen before the quote lapsed count at the quoted
/// amount. When they fall short and later ones made up the difference, the
/// whole sum is measured against `fresh`, a quote made now: within
/// [`LATE_TOLERANCE_BPS`] of it the order completes, otherwise the buyer is
/// asked to top up or take a refund. Without a fresh quote (the price sources
/// are down) a late payment waits.
pub fn settle(quote: &Quote, received: &[Received], fresh: Option<&Quote>) -> Settlement {
    if received.is_empty() {
        return Settlement::Waiting;
    }
    let required = confirmations_for(quote.coin, quote.usd_cents);
    let paid: u128 = received.iter().map(|r| r.amount).sum();
    let in_time: u128 = received
        .iter()
        .filter(|r| r.first_seen <= quote.pay_before)
        .map(|r| r.amount)
        .sum();
    if in_time >= quote.amount {
        return settled(paid, quote.amount, received, required);
    }
    if paid == in_time {
        return Settlement::Underpaid {
            paid,
            due: quote.amount,
        };
    }
    // Some of it came after the quote lapsed: measure the whole sum against
    // the price now.
    let Some(fresh) = fresh else {
        return Settlement::Repricing { paid };
    };
    let floor = fresh.amount - fresh.amount * LATE_TOLERANCE_BPS / 10_000;
    if paid < floor {
        return Settlement::Late {
            paid,
            due_now: fresh.amount,
        };
    }
    settled(paid, paid.min(fresh.amount), received, required)
}

/// Enough reached the address; how deep is it? The deepest payments are
/// counted first, so a fresh top-up never holds back an order that older
/// payments already cover.
fn settled(paid: u128, due: u128, received: &[Received], required: u64) -> Settlement {
    let mut by_depth: Vec<&Received> = received.iter().collect();
    by_depth.sort_by_key(|r| std::cmp::Reverse(r.confirmations));
    let mut covered = 0u128;
    let mut shallowest = u64::MAX;
    for r in by_depth {
        covered += r.amount;
        shallowest = shallowest.min(r.confirmations);
        if covered >= due {
            break;
        }
    }
    if shallowest == 0 {
        Settlement::Seen { paid }
    } else if shallowest < required {
        Settlement::Confirming {
            paid,
            confirmations: shallowest,
            required,
        }
    } else {
        Settlement::Paid {
            paid,
            extra: paid.saturating_sub(due),
        }
    }
}

// ── Payment requests ──────────────────────────────────────────────────────

/// The payment link a wallet opens, with the exact amount, so nobody types
/// or picks an address by hand: BIP 21, `monero:` and ZIP 321. `label` names
/// the order in the wallet's history.
pub fn payment_uri(coin: Coin, address: &str, amount: u128, label: &str) -> String {
    let amount = trim_amount(&format_amount(coin, amount));
    let label = percent_encode(label);
    match coin {
        Coin::Btc => format!("bitcoin:{address}?amount={amount}&label={label}"),
        Coin::Xmr => {
            format!("monero:{address}?tx_amount={amount}&tx_description={label}")
        }
        // ZIP 321 carries a memo as base64url; the label stays readable.
        Coin::Zec => format!("zcash:{address}?amount={amount}&label={label}"),
        Coin::Eth | Coin::Sol => String::new(),
    }
}

/// "0.00150000" → "0.0015"; "1.00000000" → "1".
fn trim_amount(text: &str) -> String {
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const BTC_PRICE: u128 = 60_000_000_000; // $60,000.000000

    fn quote(usd_cents: u64) -> Quote {
        Quote::new(Coin::Btc, usd_cents, BTC_PRICE, 1_000).expect("quote")
    }

    fn got(amount: u128, confirmations: u64, first_seen: u64) -> Received {
        Received {
            tx: format!("tx{amount}-{first_seen}"),
            amount,
            confirmations,
            first_seen,
        }
    }

    #[test]
    fn prices_parse_without_floats() {
        assert_eq!(parse_micros("67123.45"), Some(67_123_450_000));
        assert_eq!(parse_micros("0.1234567"), Some(123_456));
        assert_eq!(parse_micros("150"), Some(150_000_000));
        assert_eq!(parse_micros(".5"), Some(500_000));
        assert_eq!(parse_micros("-1"), None);
        assert_eq!(parse_micros("1e5"), None);
        assert_eq!(parse_micros(""), None);
        assert_eq!(parse_micros("."), None);
    }

    #[test]
    fn each_source_reply_is_read() {
        let kraken = json!({"error": [], "result": {"XXBTZUSD": {"c": ["67123.40000", "0.01"]}}});
        assert_eq!(
            PriceSource::kraken().read(Coin::Btc, &kraken),
            Some(67_123_400_000)
        );
        let refused = json!({"error": ["EQuery:Unknown asset pair"]});
        assert_eq!(PriceSource::kraken().read(Coin::Btc, &refused), None);
        let gecko = json!({"monero": {"usd": 161.37}});
        assert_eq!(
            PriceSource::coingecko().read(Coin::Xmr, &gecko),
            Some(161_370_000)
        );
        assert_eq!(PriceSource::coingecko().read(Coin::Zec, &gecko), None);
        let zero = json!({"zcash": {"usd": 0}});
        assert_eq!(PriceSource::coingecko().read(Coin::Zec, &zero), None);
    }

    #[test]
    fn two_sources_must_agree_within_one_percent() {
        assert_eq!(agree(&[100_000_000, 100_900_000]), Ok(100_450_000));
        assert_eq!(agree(&[100_000_000, 101_000_000]), Ok(100_500_000));
        assert_eq!(
            agree(&[100_000_000, 101_000_001]),
            Err(PriceError::Disagree)
        );
        assert_eq!(agree(&[100_000_000]), Err(PriceError::TooFew));
        assert_eq!(agree(&[0, 1]), Err(PriceError::TooFew));
    }

    #[test]
    fn amounts_round_up_for_the_seller() {
        // $6.00 at $60,000 is exactly 10,000 sat.
        assert_eq!(amount_for(Coin::Btc, 600, BTC_PRICE), Some(10_000));
        // $6.01 is 10,016.66… sat, so 10,017.
        assert_eq!(amount_for(Coin::Btc, 601, BTC_PRICE), Some(10_017));
        // Monero has twelve decimals: $9.00 at $150 is 0.06 XMR.
        assert_eq!(
            amount_for(Coin::Xmr, 900, 150_000_000),
            Some(60_000_000_000)
        );
        assert_eq!(amount_for(Coin::Zec, 0, 1), None);
        assert_eq!(amount_for(Coin::Zec, 1, 0), None);
        assert_eq!(cents_for(Coin::Btc, 10_017, BTC_PRICE), 601);
        assert_eq!(format_amount(Coin::Btc, 150_000), "0.00150000");
        assert_eq!(format_amount(Coin::Xmr, 60_000_000_000), "0.060000000000");
    }

    #[test]
    fn deeper_blocks_for_bigger_orders() {
        assert_eq!(confirmations_for(Coin::Btc, 600), 1);
        assert_eq!(confirmations_for(Coin::Btc, 50_000), 2);
        assert_eq!(confirmations_for(Coin::Btc, 1_000_000), 6);
        assert_eq!(confirmations_for(Coin::Zec, 600), 3);
        assert_eq!(confirmations_for(Coin::Xmr, 600), 10);
        assert!(Coin::ORDERS
            .iter()
            .all(|&c| confirmations_for(c, u64::MAX) >= confirmations_for(c, 1)));
    }

    #[test]
    fn nothing_seen_is_waiting() {
        assert_eq!(settle(&quote(600), &[], None), Settlement::Waiting);
    }

    #[test]
    fn a_payment_in_the_mempool_is_seen_and_does_not_lapse() {
        let q = quote(600);
        let s = settle(&q, &[got(10_000, 0, 1_100)], None);
        assert_eq!(s, Settlement::Seen { paid: 10_000 });
    }

    #[test]
    fn short_payments_add_up() {
        let q = quote(600);
        assert_eq!(
            settle(&q, &[got(4_000, 1, 1_100)], None),
            Settlement::Underpaid {
                paid: 4_000,
                due: 10_000
            }
        );
        // An exchange that pays in pieces: counted together.
        assert_eq!(
            settle(&q, &[got(4_000, 2, 1_100), got(6_000, 1, 1_200)], None),
            Settlement::Paid {
                paid: 10_000,
                extra: 0
            }
        );
    }

    #[test]
    fn a_big_order_waits_for_its_blocks() {
        let q = quote(100_000); // $1,000 → three blocks
        let s = settle(&q, &[got(q.amount, 2, 1_100)], None);
        assert_eq!(
            s,
            Settlement::Confirming {
                paid: q.amount,
                confirmations: 2,
                required: 3
            }
        );
        assert!(settle(&q, &[got(q.amount, 3, 1_100)], None).is_paid());
    }

    #[test]
    fn a_fresh_top_up_does_not_hold_back_a_covered_order() {
        let q = quote(600);
        let s = settle(&q, &[got(10_000, 1, 1_100), got(500, 0, 1_300)], None);
        assert_eq!(
            s,
            Settlement::Paid {
                paid: 10_500,
                extra: 500
            }
        );
    }

    #[test]
    fn overpaid_is_recorded() {
        let q = quote(600);
        assert_eq!(
            settle(&q, &[got(12_000, 1, 1_100)], None),
            Settlement::Paid {
                paid: 12_000,
                extra: 2_000
            }
        );
    }

    #[test]
    fn late_payment_within_one_percent_of_a_fresh_quote_completes() {
        let q = quote(600);
        let late = q.pay_before + 60;
        // The price rose 0.5%: the same amount is 0.5% short now.
        let fresh = Quote::new(Coin::Btc, 600, BTC_PRICE * 1000 / 1005, late).expect("fresh");
        assert!(settle(&q, &[got(10_000, 1, late)], Some(&fresh)).is_paid());
    }

    #[test]
    fn late_payment_far_off_a_fresh_quote_is_offered_the_new_price() {
        let q = quote(600);
        let late = q.pay_before + 60;
        // The coin fell 10%: the buyer now owes more.
        let fresh = Quote::new(Coin::Btc, 600, BTC_PRICE * 9 / 10, late).expect("fresh");
        assert_eq!(
            settle(&q, &[got(10_000, 1, late)], Some(&fresh)),
            Settlement::Late {
                paid: 10_000,
                due_now: fresh.amount
            }
        );
        // Without a fresh price it waits rather than guessing.
        assert_eq!(
            settle(&q, &[got(10_000, 1, late)], None),
            Settlement::Repricing { paid: 10_000 }
        );
    }

    #[test]
    fn a_late_top_up_is_measured_against_the_fresh_quote() {
        let q = quote(600);
        let late = q.pay_before + 60;
        let fresh = Quote::new(Coin::Btc, 600, BTC_PRICE, late).expect("fresh");
        let s = settle(
            &q,
            &[got(6_000, 2, 1_100), got(4_000, 1, late)],
            Some(&fresh),
        );
        assert!(s.is_paid());
    }

    #[test]
    fn payment_links_carry_the_exact_amount() {
        assert_eq!(
            payment_uri(Coin::Btc, "bc1qexample", 150_000, "LosOS order 1"),
            "bitcoin:bc1qexample?amount=0.0015&label=LosOS%20order%201"
        );
        assert_eq!(
            payment_uri(Coin::Xmr, "8Ab", 60_000_000_000, "o"),
            "monero:8Ab?tx_amount=0.06&tx_description=o"
        );
        assert_eq!(
            payment_uri(Coin::Zec, "u1x", 100_000_000, "o"),
            "zcash:u1x?amount=1&label=o"
        );
    }
}
