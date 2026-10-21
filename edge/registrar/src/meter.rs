//! Pay as you go: a prepaid USDC balance in the escrow, drawn down through
//! taps.
//!
//! A buyer keeps a balance in `LososEscrow` (EVM) or `losos-escrow` (Solana)
//! and opens a *tap* for every seller it draws on. A tap names one seller, a
//! most it may be billed per hour, and the platform's fee; the buyer's wallet
//! signs it, so it is the buyer's authorisation. The edge meters what was
//! really used and settles each tap from time to time with one call the
//! escrow checks against the rate: a stolen edge key can bill no more than
//! the buyer allowed, and only to the seller the buyer named.
//!
//! What a buyer meters here is an AI model run across a group of boxes
//! ([`crate::inference`]): one [`Meter`] per model and network, with one
//! [`Tap`] per box in the group, each billed its share of every request by
//! the memory it lends. A box that is offline serves nothing, so it bills
//! nothing; there is no refund rule because nothing is paid ahead.
//!
//! Amounts on the chain are USDC base units (a millionth of a dollar).
//! Usage accrues in millionths of a base unit, so a token priced at a
//! fraction of a base unit is still counted exactly, and is settled in whole
//! base units with the remainder carried over.
//!
//! Everything here is a pure function over [`Taps`]; `crate::market::Market`
//! keeps it in the market's ledger and moves the money.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// How long a buyer may wait between asking for a withdrawal and taking the
/// money. The escrow's `NOTICE`.
pub const NOTICE_SECS: u64 = 48 * 3600;
/// The most time one settle may bill for. The escrow's `MAX_ACCRUAL`.
pub const MAX_ACCRUAL_SECS: u64 = 7 * 24 * 3600;
/// How far into its accrual a tap opens: the escrow sets a new tap's
/// `settledAt` this long before the moment it opens, so a buyer can be served
/// up to an hour's worth at once rather than nothing until time passes. The
/// most a tap can ever bill is its rate times the time since it opened plus
/// this. The escrow's `HEADSTART`.
pub const HEADSTART_SECS: u64 = 3600;
/// How long a quote waits for the wallet to open its taps.
pub const QUOTE_SECS: u64 = 30 * 60;
/// Millionths of a base unit per base unit.
pub const E6: u64 = 1_000_000;
/// The most taps one meter opens: the escrow's `MAX_TAPS_PER_CALL`.
pub const MAX_TAPS: usize = 16;
/// An EVM tap is settled once it owes this much (a dollar), or a day after
/// its last settle, whichever comes first. Gas makes smaller ones wasteful.
pub const EVM_SETTLE_AT: u64 = 1_000_000;
pub const EVM_SETTLE_EVERY_SECS: u64 = 24 * 3600;
/// A Solana transaction costs a fraction of a cent: hourly.
pub const SOLANA_SETTLE_EVERY_SECS: u64 = 3600;
/// Clock drift between the edge and the chain the cap allows for. The edge
/// meters against a cap this much smaller than the escrow's, so a settle it
/// sends is never over the escrow's cap.
pub const CLOCK_MARGIN_SECS: u64 = 120;
/// How long a settle or close may sit unconfirmed before it is sent again.
pub const RESEND_SECS: u64 = 10 * 60;
/// The least and the most a buyer may allow per hour, in base units: a
/// cent, and a hundred dollars.
pub const MIN_SPEND_PER_HOUR: u64 = 10_000;
pub const MAX_SPEND_PER_HOUR: u64 = 100_000_000;
/// Meters one buyer may have waiting for their wallet at once.
pub const MAX_QUOTED_PER_BUYER: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MeterState {
    /// Waiting for the buyer's wallet to open the taps.
    Quoted,
    /// The taps are open; requests are served and billed.
    Open,
    /// No more requests. The taps are being closed and settled one last time.
    Ending,
    /// Every tap is closed and settled.
    Ended,
    /// The wallet never opened the taps.
    Lapsed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EndReason {
    /// The buyer stopped it.
    Buyer,
    /// A box in the group left, so the model cannot run on it any more.
    GroupLeft,
    /// The buyer's wallet closed a tap itself.
    TapClosed,
    /// The boxes picked for the model could not load it or keep it running.
    GroupFailed,
}

impl EndReason {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::Buyer => "You stopped it.",
            Self::GroupLeft => {
                "A box that ran this model left its group. Start the model again to use another group."
            }
            Self::TapClosed => "The wallet closed one of its taps.",
            Self::GroupFailed => {
                "The boxes picked for this model could not run it. Start it again to use other boxes."
            }
        }
    }
}

/// One buyer's pay-as-you-go use of one model on one network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meter {
    pub id: String,
    /// The buyer's appliance id.
    pub buyer: String,
    pub model: String,
    /// The group of boxes the model runs on for this meter.
    pub group: String,
    pub network: String,
    /// The wallet that opened the taps, once the edge has seen them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet: Option<String>,
    /// What the buyer allowed per hour, base units, over all its taps.
    pub spend_per_hour: u64,
    pub taps: Vec<String>,
    pub state: MeterState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<EndReason>,
    pub created_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opened_at: Option<u64>,
    #[serde(default)]
    pub requests: u64,
    #[serde(default)]
    pub tokens_in: u64,
    #[serde(default)]
    pub tokens_out: u64,
    /// Billed in all, millionths of a base unit.
    #[serde(default)]
    pub billed_e6: u64,
}

/// A transaction the edge sent for a tap and is following.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sent {
    /// What a settle bills, base units. Zero for a close.
    pub amount: u64,
    pub tx: String,
    pub sent_at: u64,
}

/// One seller's tap, as the edge keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tap {
    pub id: String,
    pub meter: String,
    pub buyer: String,
    pub seller: String,
    pub network: String,
    /// The seller's payout address the tap pays, as the quote named it.
    pub payee: String,
    /// The seller's share of every request, in millionths.
    pub share_ppm: u32,
    /// The most the escrow lets the edge bill per hour, base units.
    pub rate: u64,
    pub fee_bps: u32,
    /// The escrow has the tap.
    #[serde(default)]
    pub opened: bool,
    /// The escrow's `settledAt`: billing time starts from here.
    #[serde(default)]
    pub settled_at: u64,
    /// The escrow's `closedAt`, once closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<u64>,
    /// Used and not yet settled, millionths of a base unit.
    #[serde(default)]
    pub accrued_e6: u64,
    /// Settled in all, base units, fee included.
    #[serde(default)]
    pub paid: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settle: Option<Sent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close: Option<Sent>,
    /// The tap is finished: closed and settled to its close.
    #[serde(default)]
    pub done: bool,
}

/// The pay-as-you-go part of a market ledger.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Taps {
    #[serde(default)]
    pub meters: BTreeMap<String, Meter>,
    #[serde(default)]
    pub taps: BTreeMap<String, Tap>,
}

impl Taps {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.meters.is_empty() && self.taps.is_empty()
    }
}

/// A tap as the escrow records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainTap {
    /// The buyer's wallet, `0x` lowercase on EVM or base58 on Solana.
    pub buyer: [u8; 32],
    pub seller: [u8; 32],
    pub rate: u64,
    pub fee_bps: u32,
    pub settled_at: u64,
    pub closed_at: Option<u64>,
}

/// What the escrow holds for one wallet, read from the chain.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletView {
    /// Base units.
    pub balance: u64,
    /// What the wallet asked to withdraw.
    pub asked: u64,
    /// When it may take it, unix seconds; zero without an ask.
    pub not_before: u64,
}

/// One member of a quote: a seller, what they lend, and where they are paid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payee {
    pub seller: String,
    pub mib: u64,
    pub address: String,
}

/// Why a request is not served.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The balance cannot cover the request.
    Balance,
    /// The hour's limit is used up; it frees up as time passes.
    Rate,
    /// The meter is not open.
    Closed,
}

impl Refusal {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::Balance => {
                "the balance cannot cover this request; top it up, or ask for fewer tokens"
            }
            Self::Rate => {
                "this model has used what you allowed per hour; wait a little, or allow more"
            }
            Self::Closed => "this model is not running for you; start it again",
        }
    }
}

/// Split a million parts between the payees by the memory they lend. The
/// rounding remainder goes to the one lending most, so the shares always
/// add up to exactly a million.
#[must_use]
pub fn shares(payees: &[Payee]) -> Vec<u32> {
    let total: u128 = payees.iter().map(|p| u128::from(p.mib)).sum();
    if total == 0 {
        return vec![0; payees.len()];
    }
    let mut out: Vec<u32> = payees
        .iter()
        .map(|p| u32::try_from(u128::from(p.mib) * 1_000_000 / total).unwrap_or(0))
        .collect();
    let given: u32 = out.iter().sum();
    if let Some(big) = (0..payees.len()).max_by_key(|&i| (payees[i].mib, std::cmp::Reverse(i))) {
        out[big] += 1_000_000 - given;
    }
    out
}

/// Each tap's hourly rate: its share of what the buyer allowed, rounded up
/// so the taps together allow at least that much.
#[must_use]
pub fn rate_for(spend_per_hour: u64, share_ppm: u32) -> u64 {
    let r = (u128::from(spend_per_hour) * u128::from(share_ppm)).div_ceil(1_000_000);
    u64::try_from(r).unwrap_or(u64::MAX).max(1)
}

/// What a quote asks for.
#[derive(Debug, Clone)]
pub struct QuoteSpec<'a> {
    pub buyer: &'a str,
    pub model: &'a str,
    pub group: &'a str,
    pub network: &'a str,
    pub spend_per_hour: u64,
    pub fee_bps: u32,
    pub payees: &'a [Payee],
    pub now: u64,
}

/// Write down a meter and its taps, waiting for the wallet. `ids` makes one
/// fresh id per call: the meter's first, then one per tap.
///
/// # Errors
/// A static reason when the quote cannot be made.
pub fn quote(
    state: &mut Taps,
    spec: &QuoteSpec<'_>,
    mut ids: impl FnMut(&str) -> Option<String>,
) -> Result<String, &'static str> {
    if spec.payees.is_empty() || spec.payees.len() > MAX_TAPS {
        return Err("a model runs on one to sixteen boxes");
    }
    if !(MIN_SPEND_PER_HOUR..=MAX_SPEND_PER_HOUR).contains(&spec.spend_per_hour) {
        return Err("allow between one cent and a hundred dollars per hour");
    }
    let waiting = state
        .meters
        .values()
        .filter(|m| m.buyer == spec.buyer && m.state == MeterState::Quoted)
        .count();
    if waiting >= MAX_QUOTED_PER_BUYER {
        return Err("too many models are waiting for your wallet; finish or wait for one");
    }
    let id = ids("mtr").ok_or("no randomness")?;
    let split = shares(spec.payees);
    let mut taps = Vec::with_capacity(spec.payees.len());
    for (payee, share) in spec.payees.iter().zip(split) {
        let tap = ids("tap").ok_or("no randomness")?;
        state.taps.insert(
            tap.clone(),
            Tap {
                id: tap.clone(),
                meter: id.clone(),
                buyer: spec.buyer.to_string(),
                seller: payee.seller.clone(),
                network: spec.network.to_string(),
                payee: payee.address.clone(),
                share_ppm: share,
                rate: rate_for(spec.spend_per_hour, share),
                fee_bps: spec.fee_bps,
                opened: false,
                settled_at: 0,
                closed_at: None,
                accrued_e6: 0,
                paid: 0,
                settle: None,
                close: None,
                done: false,
            },
        );
        taps.push(tap);
    }
    state.meters.insert(
        id.clone(),
        Meter {
            id: id.clone(),
            buyer: spec.buyer.to_string(),
            model: spec.model.to_string(),
            group: spec.group.to_string(),
            network: spec.network.to_string(),
            wallet: None,
            spend_per_hour: spec.spend_per_hour,
            taps,
            state: MeterState::Quoted,
            reason: None,
            created_at: spec.now,
            opened_at: None,
            requests: 0,
            tokens_in: 0,
            tokens_out: 0,
            billed_e6: 0,
        },
    );
    Ok(id)
}

/// Check what the escrow holds against the quote and open the meter. Every
/// tap must be there, opened by one wallet, for the payee, rate and fee the
/// quote named. `wallet` renders a 32-byte key the way the network writes
/// addresses; `payee` turns a quoted address into the same 32 bytes.
///
/// # Errors
/// A static reason, with nothing changed.
pub fn confirm(
    state: &mut Taps,
    meter_id: &str,
    chain: &BTreeMap<String, ChainTap>,
    payee_key: impl Fn(&str) -> Option<[u8; 32]>,
    wallet: impl Fn(&[u8; 32]) -> String,
    now: u64,
) -> Result<(), &'static str> {
    let meter = state.meters.get(meter_id).ok_or("no such meter")?;
    match meter.state {
        MeterState::Quoted => {}
        MeterState::Open => return Ok(()),
        _ => return Err("this meter has ended; start the model again"),
    }
    let mut buyer: Option<[u8; 32]> = None;
    for id in &meter.taps {
        let tap = state.taps.get(id).ok_or("a tap of this meter is missing")?;
        let seen = chain
            .get(id)
            .ok_or("the escrow does not have every tap yet")?;
        if payee_key(&tap.payee) != Some(seen.seller) {
            return Err("a tap pays someone other than the box it was quoted for");
        }
        if seen.rate != tap.rate || seen.fee_bps != tap.fee_bps {
            return Err("a tap's rate or fee is not the quoted one");
        }
        if seen.closed_at.is_some() {
            return Err("a tap is already closed");
        }
        if *buyer.get_or_insert(seen.buyer) != seen.buyer {
            return Err("the taps were opened by more than one wallet");
        }
    }
    let wallet = buyer.map(|b| wallet(&b));
    let taps = meter.taps.clone();
    for id in &taps {
        if let (Some(tap), Some(seen)) = (state.taps.get_mut(id), chain.get(id)) {
            tap.opened = true;
            tap.settled_at = seen.settled_at;
        }
    }
    if let Some(m) = state.meters.get_mut(meter_id) {
        m.state = MeterState::Open;
        m.wallet = wallet;
        m.opened_at = Some(now);
    }
    Ok(())
}

/// What a tap may still be billed now without going over the escrow's cap,
/// millionths of a base unit. A settle on its way counts as landing now.
#[must_use]
pub fn tap_room(tap: &Tap, now: u64) -> u64 {
    let (since, owed) = match &tap.settle {
        Some(s) => (
            s.sent_at.max(tap.settled_at),
            tap.accrued_e6.saturating_sub(s.amount.saturating_mul(E6)),
        ),
        None => (tap.settled_at, tap.accrued_e6),
    };
    let end = tap.closed_at.unwrap_or(now).min(now);
    let elapsed = end
        .saturating_sub(since)
        .min(MAX_ACCRUAL_SECS)
        .saturating_sub(CLOCK_MARGIN_SECS);
    let cap = u128::from(tap.rate) * u128::from(elapsed) * u128::from(E6) / 3600;
    u64::try_from(cap).unwrap_or(u64::MAX).saturating_sub(owed)
}

/// What the buyer's wallet still has on `network` once everything used and
/// not yet settled is taken off, and what it asked to withdraw. Millionths.
#[must_use]
pub fn funds(state: &Taps, wallet: &str, network: &str, view: &WalletView) -> u64 {
    let owed: u64 = state
        .meters
        .values()
        .filter(|m| m.network == network && m.wallet.as_deref() == Some(wallet))
        .flat_map(|m| m.taps.iter())
        .filter_map(|id| state.taps.get(id))
        .map(|t| t.accrued_e6)
        .fold(0, u64::saturating_add);
    view.balance
        .saturating_sub(view.asked)
        .saturating_mul(E6)
        .saturating_sub(owed)
}

/// The most one request on `meter_id` may cost now, millionths: what the
/// wallet can cover, and what every tap can still be billed for its share.
///
/// # Errors
/// Why nothing may be served.
pub fn headroom(state: &Taps, meter_id: &str, view: &WalletView, now: u64) -> Result<u64, Refusal> {
    let meter = state.meters.get(meter_id).ok_or(Refusal::Closed)?;
    if meter.state != MeterState::Open {
        return Err(Refusal::Closed);
    }
    let wallet = meter.wallet.as_deref().ok_or(Refusal::Closed)?;
    let money = funds(state, wallet, &meter.network, view);
    let mut room = money;
    for id in &meter.taps {
        let tap = state.taps.get(id).ok_or(Refusal::Closed)?;
        if tap.closed_at.is_some() || tap.close.is_some() {
            return Err(Refusal::Closed);
        }
        if tap.share_ppm == 0 {
            continue;
        }
        let total = u128::from(tap_room(tap, now)) * 1_000_000 / u128::from(tap.share_ppm);
        room = room.min(u64::try_from(total).unwrap_or(u64::MAX));
    }
    if money == 0 {
        return Err(Refusal::Balance);
    }
    if room == 0 {
        return Err(Refusal::Rate);
    }
    Ok(room)
}

/// What `tokens_in` and `tokens_out` cost at prices in base units per million
/// tokens, millionths of a base unit.
#[must_use]
pub fn cost_e6(tokens_in: u64, tokens_out: u64, price_in: u64, price_out: u64) -> u64 {
    tokens_in
        .saturating_mul(price_in)
        .saturating_add(tokens_out.saturating_mul(price_out))
}

/// Bill one served request: `cost` millionths split between the meter's
/// taps by their shares, rounded so the parts add up to `cost`.
pub fn bill(state: &mut Taps, meter_id: &str, cost: u64, tokens_in: u64, tokens_out: u64) {
    let Some(meter) = state.meters.get_mut(meter_id) else {
        return;
    };
    meter.requests += 1;
    meter.tokens_in = meter.tokens_in.saturating_add(tokens_in);
    meter.tokens_out = meter.tokens_out.saturating_add(tokens_out);
    meter.billed_e6 = meter.billed_e6.saturating_add(cost);
    let ids = meter.taps.clone();
    let mut left = cost;
    let last = ids.len().saturating_sub(1);
    for (n, id) in ids.iter().enumerate() {
        let Some(tap) = state.taps.get_mut(id) else {
            continue;
        };
        let part = if n == last {
            left
        } else {
            let p = u128::from(cost) * u128::from(tap.share_ppm) / 1_000_000;
            u64::try_from(p).unwrap_or(0).min(left)
        };
        left -= part;
        tap.accrued_e6 = tap.accrued_e6.saturating_add(part);
    }
}

/// A settle the edge should send now: the tap and what it bills, base units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Due {
    pub tap: String,
    pub amount: u64,
}

/// Taps worth settling now. `every` is the network's cadence, `at` the
/// amount that settles at once (zero for any amount); a closed tap, a meter
/// that is ending and a wallet that asked to withdraw settle at once.
/// Nothing is ever billed past what the wallet holds.
#[must_use]
pub fn due(
    state: &Taps,
    now: u64,
    network: &str,
    every: u64,
    at: u64,
    views: &BTreeMap<String, WalletView>,
) -> Vec<Due> {
    let mut out = Vec::new();
    let mut spent: BTreeMap<&str, u64> = BTreeMap::new();
    for tap in state.taps.values() {
        if tap.network != network || !tap.opened || tap.done || tap.settle.is_some() {
            continue;
        }
        let Some(meter) = state.meters.get(&tap.meter) else {
            continue;
        };
        let Some(wallet) = meter.wallet.as_deref() else {
            continue;
        };
        let amount = tap.accrued_e6 / E6;
        if amount == 0 {
            continue;
        }
        let view = views.get(wallet).copied().unwrap_or_default();
        let urgent = tap.closed_at.is_some() || meter.state != MeterState::Open || view.asked > 0;
        let old = now.saturating_sub(tap.settled_at) >= every;
        if !(urgent || old || (at > 0 && amount >= at)) {
            continue;
        }
        // The escrow's cap, with the edge's own margin.
        let end = tap.closed_at.unwrap_or(now).min(now);
        let elapsed = end.saturating_sub(tap.settled_at).min(MAX_ACCRUAL_SECS);
        let cap =
            u64::try_from(u128::from(tap.rate) * u128::from(elapsed) / 3600).unwrap_or(u64::MAX);
        let used = spent.entry(wallet).or_default();
        let left = view.balance.saturating_sub(*used);
        let amount = amount.min(cap).min(left);
        if amount == 0 {
            continue;
        }
        *used += amount;
        out.push(Due {
            tap: tap.id.clone(),
            amount,
        });
    }
    out
}

/// A settle of `amount` landed: the escrow now says the tap was settled to
/// `settled_at`.
pub fn settled(state: &mut Taps, tap_id: &str, amount: u64, settled_at: u64) {
    if let Some(tap) = state.taps.get_mut(tap_id) {
        tap.accrued_e6 = tap.accrued_e6.saturating_sub(amount.saturating_mul(E6));
        tap.paid = tap.paid.saturating_add(amount);
        tap.settled_at = tap.settled_at.max(settled_at);
        tap.settle = None;
        finish_if_done(tap);
    }
}

/// The escrow recorded the tap closed at `closed_at`.
pub fn closed(state: &mut Taps, tap_id: &str, closed_at: u64) {
    let Some(tap) = state.taps.get_mut(tap_id) else {
        return;
    };
    tap.closed_at = Some(tap.closed_at.map_or(closed_at, |c| c.min(closed_at)));
    tap.close = None;
    let meter = tap.meter.clone();
    finish_if_done(tap);
    if let Some(m) = state.meters.get_mut(&meter) {
        if m.state == MeterState::Open {
            m.state = MeterState::Ending;
            m.reason.get_or_insert(EndReason::TapClosed);
        }
    }
}

/// A closed tap is done once nothing more can be billed on it: it is settled
/// to its close, or what it still owes is under a base unit, or its cap is
/// spent.
fn finish_if_done(tap: &mut Tap) {
    let Some(closed_at) = tap.closed_at else {
        return;
    };
    if tap.settle.is_some() {
        return;
    }
    if tap.settled_at >= closed_at || tap.accrued_e6 < E6 {
        tap.done = true;
    }
}

/// Stop serving a meter: its taps are to be closed and settled one last
/// time. Returns whether anything changed.
pub fn end(state: &mut Taps, meter_id: &str, reason: EndReason) -> bool {
    let Some(m) = state.meters.get_mut(meter_id) else {
        return false;
    };
    match m.state {
        MeterState::Open => {
            m.state = MeterState::Ending;
            m.reason = Some(reason);
            true
        }
        MeterState::Quoted => {
            m.state = MeterState::Lapsed;
            m.reason = Some(reason);
            true
        }
        _ => false,
    }
}

/// The buyer stops a quote its wallet opened only in part, for example a
/// Solana quote whose later batch the wallet refused. `chain` is what the
/// escrow holds of the quote's taps under `wallet`. The taps it holds are
/// marked opened, so the edge closes them; the rest are done. The meter
/// keeps the wallet either way, so the balance it topped up stays in view.
/// Returns whether anything changed: false unless the meter is quoted.
pub fn end_quoted(
    state: &mut Taps,
    meter_id: &str,
    chain: &BTreeMap<String, ChainTap>,
    wallet: &str,
) -> bool {
    let Some(m) = state.meters.get(meter_id) else {
        return false;
    };
    if m.state != MeterState::Quoted {
        return false;
    }
    let taps = m.taps.clone();
    for id in &taps {
        let Some(tap) = state.taps.get_mut(id) else {
            continue;
        };
        match chain.get(id) {
            Some(seen) => {
                tap.opened = true;
                tap.settled_at = seen.settled_at;
                tap.closed_at = seen.closed_at;
                finish_if_done(tap);
            }
            None => tap.done = true,
        }
    }
    let Some(m) = state.meters.get_mut(meter_id) else {
        return false;
    };
    m.state = if chain.is_empty() {
        MeterState::Lapsed
    } else {
        MeterState::Ending
    };
    m.reason = Some(EndReason::Buyer);
    m.wallet = Some(wallet.to_string());
    true
}

/// Taps of ending meters still open on the chain, for the edge to close.
#[must_use]
pub fn to_close(state: &Taps, network: &str) -> Vec<String> {
    state
        .meters
        .values()
        .filter(|m| m.state == MeterState::Ending && m.network == network)
        .flat_map(|m| m.taps.iter())
        .filter_map(|id| state.taps.get(id))
        .filter(|t| t.opened && t.closed_at.is_none() && t.close.is_none())
        .map(|t| t.id.clone())
        .collect()
}

/// Lapse quotes the wallet never opened, and end meters whose taps are all
/// done. Returns the meters that changed.
pub fn tidy(state: &mut Taps, now: u64) -> Vec<String> {
    let mut changed = Vec::new();
    for m in state.meters.values_mut() {
        match m.state {
            MeterState::Quoted if now >= m.created_at.saturating_add(QUOTE_SECS) => {
                m.state = MeterState::Lapsed;
                changed.push(m.id.clone());
            }
            MeterState::Ending
                if m.taps
                    .iter()
                    .all(|id| state.taps.get(id).is_none_or(|t| t.done)) =>
            {
                m.state = MeterState::Ended;
                changed.push(m.id.clone());
            }
            _ => {}
        }
    }
    changed
}

/// The buyer's open meter for `model`, if any.
#[must_use]
pub fn open_meter<'a>(state: &'a Taps, buyer: &str, model: &str) -> Option<&'a Meter> {
    state
        .meters
        .values()
        .find(|m| m.buyer == buyer && m.model == model && m.state == MeterState::Open)
}

/// Whether a group still has a meter that is waiting, open or settling.
#[must_use]
pub fn group_in_use(state: &Taps, group: &str) -> bool {
    state
        .meters
        .values()
        .any(|m| m.group == group && matches!(m.state, MeterState::Quoted | MeterState::Open))
}

/// What a seller earned from taps: paid out, and used but not yet settled,
/// both base units before the fee.
#[must_use]
pub fn earned(state: &Taps, seller: &str) -> (u64, u64) {
    state
        .taps
        .values()
        .filter(|t| t.seller == seller)
        .fold((0, 0), |(paid, owed), t| {
            (
                paid.saturating_add(t.paid),
                owed.saturating_add(t.accrued_e6 / E6),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;
    const HOUR: u64 = 3600;

    fn payees(mibs: &[u64]) -> Vec<Payee> {
        mibs.iter()
            .enumerate()
            .map(|(n, mib)| Payee {
                seller: format!("box{n}"),
                mib: *mib,
                address: format!("0x{n:040x}"),
            })
            .collect()
    }

    fn key_of(address: &str) -> Option<[u8; 32]> {
        let n = u8::from_str_radix(address.trim_start_matches("0x"), 16).ok()?;
        let mut k = [0u8; 32];
        k[31] = n;
        Some(k)
    }

    fn ids() -> impl FnMut(&str) -> Option<String> {
        let mut n = 0;
        move |p: &str| {
            n += 1;
            Some(format!("{p}_{n}"))
        }
    }

    fn spec<'a>(payees: &'a [Payee], spend: u64) -> QuoteSpec<'a> {
        QuoteSpec {
            buyer: "buyer",
            model: "big",
            group: "grp_1",
            network: "base",
            spend_per_hour: spend,
            fee_bps: 400,
            payees,
            now: NOW,
        }
    }

    /// A ledger with one open meter on three boxes, settled to `NOW`.
    fn open(spend: u64) -> (Taps, String) {
        let p = payees(&[16_384, 8_192, 8_192]);
        let mut state = Taps::default();
        let id = quote(&mut state, &spec(&p, spend), ids()).unwrap();
        let chain = chain_of(&state, &id, 7);
        confirm(&mut state, &id, &chain, key_of, |_| "0xwallet".into(), NOW).unwrap();
        (state, id)
    }

    fn chain_of(state: &Taps, meter: &str, wallet: u8) -> BTreeMap<String, ChainTap> {
        state.meters[meter]
            .taps
            .iter()
            .map(|id| {
                let t = &state.taps[id];
                let mut buyer = [0u8; 32];
                buyer[0] = wallet;
                (
                    id.clone(),
                    ChainTap {
                        buyer,
                        seller: key_of(&t.payee).unwrap(),
                        rate: t.rate,
                        fee_bps: t.fee_bps,
                        settled_at: NOW,
                        closed_at: None,
                    },
                )
            })
            .collect()
    }

    fn rich() -> WalletView {
        WalletView {
            balance: 50_000_000,
            asked: 0,
            not_before: 0,
        }
    }

    #[test]
    fn shares_follow_the_memory_and_always_add_up() {
        assert_eq!(shares(&payees(&[2, 1, 1])), [500_000, 250_000, 250_000]);
        let odd = shares(&payees(&[1, 1, 1]));
        assert_eq!(odd.iter().sum::<u32>(), 1_000_000);
        assert_eq!(odd, [333_334, 333_333, 333_333]);
        assert_eq!(shares(&payees(&[7])), [1_000_000]);
    }

    #[test]
    fn the_rates_together_allow_at_least_what_the_buyer_allowed() {
        let s = shares(&payees(&[3, 3, 3]));
        let total: u64 = s.iter().map(|&p| rate_for(1_000_000, p)).sum();
        assert!((1_000_000..1_000_003).contains(&total), "{total}");
        assert_eq!(rate_for(1, 1), 1);
    }

    #[test]
    fn a_quote_needs_a_sane_limit_and_one_to_sixteen_boxes() {
        let mut state = Taps::default();
        let none: Vec<Payee> = Vec::new();
        assert!(quote(&mut state, &spec(&none, 1_000_000), ids()).is_err());
        let many = payees(&[1; 17]);
        assert!(quote(&mut state, &spec(&many, 1_000_000), ids()).is_err());
        let p = payees(&[1]);
        assert!(quote(&mut state, &spec(&p, MIN_SPEND_PER_HOUR - 1), ids()).is_err());
        assert!(quote(&mut state, &spec(&p, MAX_SPEND_PER_HOUR + 1), ids()).is_err());
        assert!(state.meters.is_empty() && state.taps.is_empty());
    }

    #[test]
    fn a_buyer_cannot_pile_up_quotes() {
        let mut state = Taps::default();
        let p = payees(&[1]);
        let mut ids = ids();
        for _ in 0..MAX_QUOTED_PER_BUYER {
            quote(&mut state, &spec(&p, 1_000_000), &mut ids).unwrap();
        }
        assert!(quote(&mut state, &spec(&p, 1_000_000), &mut ids).is_err());
        tidy(&mut state, NOW + QUOTE_SECS);
        assert!(state.meters.values().all(|m| m.state == MeterState::Lapsed));
        quote(&mut state, &spec(&p, 1_000_000), &mut ids).unwrap();
    }

    #[test]
    fn confirm_wants_every_tap_as_quoted_from_one_wallet() {
        let p = payees(&[2, 1]);
        let mut state = Taps::default();
        let id = quote(&mut state, &spec(&p, 1_000_000), ids()).unwrap();
        let good = chain_of(&state, &id, 7);
        let wallet = |_: &[u8; 32]| "0xw".to_string();

        let mut missing = good.clone();
        missing.pop_first();
        assert!(confirm(&mut state, &id, &missing, key_of, wallet, NOW).is_err());

        let mut other_seller = good.clone();
        other_seller.values_mut().next().unwrap().seller = [9; 32];
        assert!(confirm(&mut state, &id, &other_seller, key_of, wallet, NOW).is_err());

        let mut cheap = good.clone();
        cheap.values_mut().next().unwrap().fee_bps = 0;
        assert!(confirm(&mut state, &id, &cheap, key_of, wallet, NOW).is_err());

        let mut fast = good.clone();
        fast.values_mut().next().unwrap().rate += 1;
        assert!(confirm(&mut state, &id, &fast, key_of, wallet, NOW).is_err());

        let mut two = good.clone();
        two.values_mut().next().unwrap().buyer = [8; 32];
        assert!(confirm(&mut state, &id, &two, key_of, wallet, NOW).is_err());

        let mut shut = good.clone();
        shut.values_mut().next().unwrap().closed_at = Some(NOW);
        assert!(confirm(&mut state, &id, &shut, key_of, wallet, NOW).is_err());

        assert_eq!(state.meters[&id].state, MeterState::Quoted);
        confirm(&mut state, &id, &good, key_of, wallet, NOW).unwrap();
        assert_eq!(state.meters[&id].state, MeterState::Open);
        assert_eq!(state.meters[&id].wallet.as_deref(), Some("0xw"));
        // Again is fine; a lapsed quote is not.
        confirm(&mut state, &id, &good, key_of, wallet, NOW).unwrap();
        let id2 = quote(&mut state, &spec(&p, 1_000_000), |p: &str| {
            Some(format!("{p}_x"))
        })
        .unwrap();
        tidy(&mut state, NOW + QUOTE_SECS);
        let late = chain_of(&state, &id2, 7);
        assert!(confirm(&mut state, &id2, &late, key_of, wallet, NOW).is_err());
    }

    #[test]
    fn billing_splits_by_share_and_adds_up_exactly() {
        let (mut state, id) = open(1_000_000);
        bill(&mut state, &id, 1_000_001, 10, 20);
        let parts: Vec<u64> = state.meters[&id]
            .taps
            .iter()
            .map(|t| state.taps[t].accrued_e6)
            .collect();
        assert_eq!(parts.iter().sum::<u64>(), 1_000_001);
        assert_eq!(parts[0], 500_000);
        assert_eq!(state.meters[&id].tokens_out, 20);
        assert_eq!(state.meters[&id].requests, 1);
    }

    #[test]
    fn a_request_may_cost_no_more_than_the_wallet_holds() {
        let (mut state, id) = open(100_000_000);
        let poor = WalletView {
            balance: 10,
            asked: 0,
            not_before: 0,
        };
        let room = headroom(&state, &id, &poor, NOW + HOUR).unwrap();
        assert_eq!(room, 10 * E6);
        bill(&mut state, &id, room, 1, 1);
        assert_eq!(
            headroom(&state, &id, &poor, NOW + HOUR),
            Err(Refusal::Balance)
        );
        // An ask to withdraw takes the money out of reach at once.
        let asking = WalletView {
            balance: 1_000,
            asked: 1_000,
            not_before: NOW + NOTICE_SECS,
        };
        assert_eq!(
            headroom(&state, &id, &asking, NOW + HOUR),
            Err(Refusal::Balance)
        );
    }

    #[test]
    fn a_request_may_cost_no_more_than_the_hour_allows() {
        // A dollar an hour over three taps.
        let (mut state, id) = open(1_000_000);
        // Inside the clock margin nothing may be billed yet.
        assert_eq!(headroom(&state, &id, &rich(), NOW + 60), Err(Refusal::Rate));
        let room = headroom(&state, &id, &rich(), NOW + HOUR).unwrap();
        // About a dollar less the two minutes' margin.
        assert!(
            room > 960_000 * E6 / 1_000 && room <= 1_000_000 * E6,
            "{room}"
        );
        bill(&mut state, &id, room, 1, 1);
        assert_eq!(
            headroom(&state, &id, &rich(), NOW + HOUR),
            Err(Refusal::Rate)
        );
        // Time frees it up again.
        assert!(headroom(&state, &id, &rich(), NOW + 2 * HOUR).is_ok());
        // Never more than a week's worth, however long the tap was idle.
        let (state, id) = open(1_000_000);
        let year = headroom(&state, &id, &rich(), NOW + 365 * 24 * HOUR).unwrap();
        let week = headroom(&state, &id, &rich(), NOW + MAX_ACCRUAL_SECS).unwrap();
        assert_eq!(year, week);
    }

    #[test]
    fn a_closed_or_ending_meter_serves_nothing() {
        let (mut state, id) = open(1_000_000);
        let first = state.meters[&id].taps[0].clone();
        closed(&mut state, &first, NOW + 10);
        assert_eq!(
            headroom(&state, &id, &rich(), NOW + HOUR),
            Err(Refusal::Closed)
        );
        assert_eq!(state.meters[&id].state, MeterState::Ending);
        let (mut state, id) = open(1_000_000);
        assert!(end(&mut state, &id, EndReason::Buyer));
        assert_eq!(
            headroom(&state, &id, &rich(), NOW + HOUR),
            Err(Refusal::Closed)
        );
        assert!(!end(&mut state, &id, EndReason::Buyer));
    }

    #[test]
    fn settles_wait_for_a_dollar_or_a_day_and_never_exceed_the_cap() {
        let (mut state, id) = open(10_000_000);
        let views = BTreeMap::from([("0xwallet".to_string(), rich())]);
        let at = NOW + HOUR;
        bill(&mut state, &id, 900_000 * E6, 1, 1);
        assert!(due(
            &state,
            at,
            "base",
            EVM_SETTLE_EVERY_SECS,
            EVM_SETTLE_AT,
            &views
        )
        .is_empty());
        bill(&mut state, &id, 300_000 * E6, 1, 1);
        // The first tap has half of 1.2 dollars; the others a quarter each.
        let d = due(
            &state,
            at,
            "base",
            EVM_SETTLE_EVERY_SECS,
            EVM_SETTLE_AT,
            &views,
        );
        assert!(d.is_empty(), "{d:?}");
        let d = due(
            &state,
            NOW + EVM_SETTLE_EVERY_SECS,
            "base",
            EVM_SETTLE_EVERY_SECS,
            EVM_SETTLE_AT,
            &views,
        );
        assert_eq!(d.iter().map(|d| d.amount).sum::<u64>(), 1_200_000);
        // Another network's cadence does not touch it.
        assert!(due(&state, NOW + 30 * HOUR, "solana", 1, 0, &views).is_empty());
        // Over the cap (one second after the tap opened), only the cap.
        let d = due(&state, NOW + 1, "base", 0, 0, &views);
        assert!(d.iter().all(|d| d.amount <= 10_000_000 / 3600 + 1), "{d:?}");
    }

    #[test]
    fn settles_never_bill_past_the_balance() {
        let (mut state, id) = open(10_000_000);
        bill(&mut state, &id, 6_000_000 * E6, 1, 1);
        let views = BTreeMap::from([(
            "0xwallet".to_string(),
            WalletView {
                balance: 2_000_000,
                asked: 0,
                not_before: 0,
            },
        )]);
        let d = due(&state, NOW + 2 * HOUR, "base", 0, 0, &views);
        assert_eq!(d.iter().map(|d| d.amount).sum::<u64>(), 2_000_000);
    }

    #[test]
    fn an_ask_to_withdraw_or_a_close_settles_at_once() {
        let (mut state, id) = open(10_000_000);
        bill(&mut state, &id, 3 * E6, 1, 1);
        let quiet = BTreeMap::from([("0xwallet".to_string(), rich())]);
        let at = NOW + HOUR;
        assert!(due(
            &state,
            at,
            "base",
            EVM_SETTLE_EVERY_SECS,
            EVM_SETTLE_AT,
            &quiet
        )
        .is_empty());
        let asking = BTreeMap::from([("0xwallet".to_string(), WalletView { asked: 1, ..rich() })]);
        assert!(!due(
            &state,
            at,
            "base",
            EVM_SETTLE_EVERY_SECS,
            EVM_SETTLE_AT,
            &asking
        )
        .is_empty());
        end(&mut state, &id, EndReason::Buyer);
        assert!(!due(
            &state,
            at,
            "base",
            EVM_SETTLE_EVERY_SECS,
            EVM_SETTLE_AT,
            &quiet
        )
        .is_empty());
        assert_eq!(to_close(&state, "base").len(), 3);
    }

    #[test]
    fn a_closed_tap_is_billed_only_up_to_its_close() {
        let (mut state, id) = open(3_600_000);
        let first = state.meters[&id].taps[0].clone();
        bill(&mut state, &id, 10_000_000 * E6, 1, 1);
        closed(&mut state, &first, NOW + 100);
        let views = BTreeMap::from([("0xwallet".to_string(), rich())]);
        let d = due(&state, NOW + 10 * HOUR, "base", 0, 0, &views);
        let mine = d.iter().find(|d| d.tap == first).unwrap();
        // Half the dollar-per-second rate (its share), for 100 seconds.
        assert_eq!(mine.amount, state.taps[&first].rate * 100 / 3600);
        settled(&mut state, &first, mine.amount, NOW + 100);
        assert!(state.taps[&first].done);
        // What it could not bill is not billed later.
        assert!(due(&state, NOW + 20 * HOUR, "base", 0, 0, &views)
            .iter()
            .all(|d| d.tap != first));
    }

    #[test]
    fn a_settle_in_flight_counts_as_landed_for_the_cap() {
        let (mut state, id) = open(3_600_000);
        let first = state.meters[&id].taps[0].clone();
        bill(&mut state, &id, 2_000_000 * E6, 1, 1);
        let before = tap_room(&state.taps[&first], NOW + HOUR);
        state.taps.get_mut(&first).unwrap().settle = Some(Sent {
            amount: 1_000_000,
            tx: "0x1".into(),
            sent_at: NOW + HOUR,
        });
        // Billing time restarts at the send, and the sent part is not owed.
        let after = tap_room(&state.taps[&first], NOW + HOUR + 600);
        assert!(after < before);
        settled(&mut state, &first, 1_000_000, NOW + HOUR + 5);
        assert_eq!(state.taps[&first].accrued_e6, 0);
        assert_eq!(state.taps[&first].paid, 1_000_000);
        assert_eq!(state.taps[&first].settled_at, NOW + HOUR + 5);
    }

    #[test]
    fn an_ending_meter_ends_once_every_tap_is_done() {
        let (mut state, id) = open(1_000_000);
        end(&mut state, &id, EndReason::GroupLeft);
        let taps = state.meters[&id].taps.clone();
        for t in &taps {
            closed(&mut state, t, NOW + 50);
        }
        // Nothing owed: done at once.
        assert_eq!(tidy(&mut state, NOW + 60), std::slice::from_ref(&id));
        assert_eq!(state.meters[&id].state, MeterState::Ended);
        assert!(!group_in_use(&state, "grp_1"));
    }

    #[test]
    fn earnings_are_per_seller() {
        let (mut state, id) = open(1_000_000);
        bill(&mut state, &id, 4 * E6, 1, 1);
        let first = state.meters[&id].taps[0].clone();
        settled(&mut state, &first, 2, NOW + 60);
        assert_eq!(earned(&state, "box0"), (2, 0));
        assert_eq!(earned(&state, "box1"), (0, 1));
        assert_eq!(earned(&state, "nobody"), (0, 0));
    }
}
