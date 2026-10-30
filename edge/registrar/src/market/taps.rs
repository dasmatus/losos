//! Pay as you go in the market: AI models billed per token through taps
//! ([`crate::meter`]) on the groups [`crate::inference`] runs.
//!
//! A buyer box asks for a quote, its wallet tops up its balance and opens
//! the quote's taps in the escrow, and the box confirms. From then on every
//! request is checked against what the wallet holds and what the taps allow
//! per hour, served by the group's head here, and billed for the tokens the
//! head counted. The settle pass sends what was used to each box's tap in
//! one escrow call per tap, hourly on Solana and daily (or at a dollar) on
//! EVM chains, and closes the taps of a meter that ended.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::Serialize;
use serde_json::{json, Value};

use super::{now_secs, random_id, Market, MarketError, MarketState};
use crate::action::Action;
use crate::inference::{chat_ask, chat_body, GroupState, Inference};
use crate::meter::{self, ChainTap, EndReason, MeterState, Payee, Refusal, WalletView, E6};
use crate::stablecoin::{Family, UsdcChain, UsdcOpts};

pub type ChainFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, MarketError>> + Send + 'a>>;

/// What the market needs of the escrow for taps. [`EscrowChain`] talks to
/// the configured chains; tests use an escrow kept in memory.
pub trait TapChain: Send + Sync {
    /// The 32 bytes the wallet passes for tap `tap` on `network`. The
    /// escrow keeps the tap under these and the wallet that opened it, so
    /// every call that reads or moves a tap names the wallet too.
    fn tap_key(&self, network: &str, tap: &str) -> Option<[u8; 32]>;
    /// A payout address as the escrow's 32-byte key.
    fn payee_key(&self, network: &str, address: &str) -> Option<[u8; 32]>;
    /// A 32-byte key written the way `network` writes addresses.
    fn wallet_name(&self, network: &str, key: &[u8; 32]) -> String;
    /// `wallet`'s tap `tap`, if the escrow has it.
    fn read_tap<'a>(
        &'a self,
        network: &'a str,
        wallet: &'a str,
        tap: &'a str,
    ) -> ChainFuture<'a, Option<ChainTap>>;
    fn read_wallet<'a>(&'a self, network: &'a str, wallet: &'a str) -> ChainFuture<'a, WalletView>;
    /// Send a settle of `amount` on `wallet`'s tap and return its
    /// transaction.
    fn settle<'a>(
        &'a self,
        network: &'a str,
        wallet: &'a str,
        tap: &'a str,
        amount: u64,
        replace: bool,
    ) -> ChainFuture<'a, String>;
    fn close<'a>(
        &'a self,
        network: &'a str,
        wallet: &'a str,
        tap: &'a str,
        replace: bool,
    ) -> ChainFuture<'a, String>;
}

/// Solana: the program, its config, the treasury and the mint.
type SolanaParts = ([u8; 32], [u8; 32], [u8; 32], [u8; 32]);

/// The escrows of the configured USDC networks, signed by the releaser key.
pub struct EscrowChain {
    usdc: Option<UsdcOpts>,
}

impl EscrowChain {
    #[must_use]
    pub fn new(usdc: Option<UsdcOpts>) -> Self {
        Self { usdc }
    }

    fn chain(&self, network: &str) -> Result<&UsdcChain, MarketError> {
        self.usdc
            .as_ref()
            .and_then(|u| u.chain(network))
            .filter(|c| c.pay.is_some())
            .ok_or(MarketError::Unconfigured)
    }

    async fn releaser(&self) -> Result<crate::evm_tx::Releaser, MarketError> {
        match self.usdc.as_ref().and_then(|u| u.releaser_file.as_deref()) {
            Some(file) => crate::evm_tx::Releaser::read(std::path::Path::new(file)).await,
            None => Err(MarketError::Unconfigured),
        }
    }

    async fn url(chain: &UsdcChain) -> Result<String, MarketError> {
        crate::stablecoin::read_rpc_url(std::path::Path::new(&chain.rpc_file)).await
    }

    /// Solana: the program, its config for this treasury and mint, and the
    /// two keys.
    fn solana_parts(chain: &UsdcChain) -> Result<SolanaParts, MarketError> {
        use crate::solana_escrow::{config_address, key};
        let program = chain
            .pay
            .as_deref()
            .and_then(key)
            .ok_or(MarketError::Unconfigured)?;
        let treasury = key(&chain.treasury).ok_or(MarketError::Unconfigured)?;
        let mint = key(chain.network.usdc).ok_or(MarketError::Unconfigured)?;
        Ok((
            program,
            config_address(&program, &treasury, &mint),
            treasury,
            mint,
        ))
    }

    async fn solana_step(
        &self,
        network: &str,
        wallet: &str,
        tap: &str,
        step: crate::solana_escrow::TapStep,
    ) -> Result<String, MarketError> {
        use crate::solana_escrow::{send_tap, tap_id_bytes, tap_state, SolanaReleaser, TapParties};
        let chain = self.chain(network)?;
        let url = Self::url(chain).await?;
        let (program, config, treasury, mint) = Self::solana_parts(chain)?;
        let id = tap_id_bytes(tap);
        let buyer = crate::solana_escrow::key(wallet)
            .ok_or(MarketError::Invalid("not a wallet address"))?;
        let pay = chain.pay.as_deref().unwrap_or_default();
        let seen = tap_state(&url, pay, &config, &buyer, id)
            .await?
            .ok_or(MarketError::Conflict("the escrow has no such tap"))?;
        let parties = TapParties::new(program, mint, treasury, id, &seen);
        let releaser = SolanaReleaser::from_releaser(&self.releaser().await?);
        send_tap(&url, &releaser, &parties, step).await
    }

    async fn evm_send(
        &self,
        network: &str,
        data: Vec<u8>,
        replace: bool,
    ) -> Result<String, MarketError> {
        let chain = self.chain(network)?;
        let chain_id = chain.network.chain_id.ok_or(MarketError::Unconfigured)?;
        let to = chain
            .pay
            .as_deref()
            .and_then(crate::stablecoin::address_bytes)
            .ok_or(MarketError::Unconfigured)?;
        let url = Self::url(chain).await?;
        let releaser = self.releaser().await?;
        crate::evm_tx::send(&url, chain_id, &releaser, to, data, replace).await
    }

    fn family(&self, network: &str) -> Option<Family> {
        self.usdc
            .as_ref()
            .and_then(|u| u.chain(network))
            .map(|c| c.network.family)
    }
}

impl TapChain for EscrowChain {
    fn tap_key(&self, network: &str, tap: &str) -> Option<[u8; 32]> {
        match self.family(network)? {
            Family::Evm => Some(crate::stablecoin::order_id_hash(tap)),
            Family::Solana => Some(crate::solana_escrow::tap_id_bytes(tap)),
        }
    }

    fn payee_key(&self, network: &str, address: &str) -> Option<[u8; 32]> {
        match self.family(network)? {
            Family::Evm => {
                crate::stablecoin::address_bytes(address).map(crate::evm_tx::address_word)
            }
            Family::Solana => crate::solana_escrow::key(address),
        }
    }

    fn wallet_name(&self, network: &str, key: &[u8; 32]) -> String {
        match self.family(network) {
            Some(Family::Solana) => crate::solana::base58_encode(key),
            _ => format!("0x{}", crate::identity::to_hex(&key[12..])),
        }
    }

    fn read_tap<'a>(
        &'a self,
        network: &'a str,
        wallet: &'a str,
        tap: &'a str,
    ) -> ChainFuture<'a, Option<ChainTap>> {
        Box::pin(async move {
            let chain = self.chain(network)?;
            let url = Self::url(chain).await?;
            let pay = chain.pay.as_deref().unwrap_or_default();
            match chain.network.family {
                Family::Evm => {
                    let w = crate::stablecoin::address_bytes(wallet)
                        .ok_or(MarketError::Invalid("not a wallet address"))?;
                    crate::stablecoin::tap_state(&url, pay, crate::stablecoin::tap_key(w, tap))
                        .await
                }
                Family::Solana => {
                    let (_, config, _, _) = Self::solana_parts(chain)?;
                    let w = crate::solana_escrow::key(wallet)
                        .ok_or(MarketError::Invalid("not a wallet address"))?;
                    crate::solana_escrow::tap_state(
                        &url,
                        pay,
                        &config,
                        &w,
                        crate::solana_escrow::tap_id_bytes(tap),
                    )
                    .await
                }
            }
        })
    }

    fn read_wallet<'a>(&'a self, network: &'a str, wallet: &'a str) -> ChainFuture<'a, WalletView> {
        Box::pin(async move {
            let chain = self.chain(network)?;
            let url = Self::url(chain).await?;
            let pay = chain.pay.as_deref().unwrap_or_default();
            match chain.network.family {
                Family::Evm => {
                    let w = crate::stablecoin::address_bytes(wallet)
                        .ok_or(MarketError::Invalid("not a wallet address"))?;
                    crate::stablecoin::wallet_view(&url, pay, w).await
                }
                Family::Solana => {
                    let (_, config, _, _) = Self::solana_parts(chain)?;
                    let w = crate::solana_escrow::key(wallet)
                        .ok_or(MarketError::Invalid("not a wallet address"))?;
                    crate::solana_escrow::wallet_view(&url, pay, &config, w).await
                }
            }
        })
    }

    fn settle<'a>(
        &'a self,
        network: &'a str,
        wallet: &'a str,
        tap: &'a str,
        amount: u64,
        replace: bool,
    ) -> ChainFuture<'a, String> {
        Box::pin(async move {
            match self.family(network).ok_or(MarketError::Unconfigured)? {
                Family::Evm => {
                    let data = crate::evm_tx::settle_tap_data(evm_tap_key(wallet, tap)?, amount);
                    self.evm_send(network, data, replace).await
                }
                Family::Solana => {
                    self.solana_step(
                        network,
                        wallet,
                        tap,
                        crate::solana_escrow::TapStep::Settle(amount),
                    )
                    .await
                }
            }
        })
    }

    fn close<'a>(
        &'a self,
        network: &'a str,
        wallet: &'a str,
        tap: &'a str,
        replace: bool,
    ) -> ChainFuture<'a, String> {
        Box::pin(async move {
            match self.family(network).ok_or(MarketError::Unconfigured)? {
                Family::Evm => {
                    let data = crate::evm_tx::close_tap_data(evm_tap_key(wallet, tap)?);
                    self.evm_send(network, data, replace).await
                }
                Family::Solana => {
                    self.solana_step(network, wallet, tap, crate::solana_escrow::TapStep::Close)
                        .await
                }
            }
        })
    }
}

/// The wallet whose taps the meter of `tap` runs on, once confirmed.
fn wallet_of<'a>(state: &'a meter::Taps, tap: &str) -> Option<&'a str> {
    let meter = state.taps.get(tap)?.meter.as_str();
    state.meters.get(meter)?.wallet.as_deref()
}

/// The key `LososEscrow` keeps `wallet`'s tap `tap` under.
fn evm_tap_key(wallet: &str, tap: &str) -> Result<[u8; 32], MarketError> {
    crate::stablecoin::address_bytes(wallet)
        .map(|w| crate::stablecoin::tap_key(w, tap))
        .ok_or(MarketError::Invalid("not a wallet address"))
}

/// How long a wallet's balance read from the chain is trusted. It can only
/// fall by the edge's own settles, which the ledger already counts, or by a
/// withdrawal, which waits two days behind an ask the read shows.
const WALLET_SECS: u64 = 60;
/// How often an open tap is read back, to notice a wallet that closed it.
const TAP_CHECK_SECS: u64 = 5 * 60;
/// How long a Solana transaction may stay unseen before it is sent again:
/// its blockhash lasts about a minute.
const SOLANA_RESEND_SECS: u64 = 3 * 60;
/// Runway under which the buyer is warned.
pub const WARN_RUNWAY_SECS: u64 = 24 * 3600;

/// One tap as the wallet opens it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotedTap {
    pub id: String,
    /// The 32 bytes the wallet passes to `openTaps` / `open_tap`, `0x` hex.
    pub key: String,
    /// The box's payout address the tap pays.
    pub seller: String,
    /// Base units per hour, decimal.
    pub rate: String,
}

/// What a buyer's wallet needs to open a meter's taps.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TapQuote {
    pub meter: String,
    pub model: String,
    pub network: String,
    pub chain: crate::stablecoin::ChainPublic,
    pub fee_bps: u32,
    pub spend_per_hour: String,
    pub taps: Vec<QuotedTap>,
    /// The wallet must open the taps by then, unix seconds.
    pub open_before: u64,
}

/// A buyer's meter as its box shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeterView {
    pub id: String,
    pub model: String,
    pub network: String,
    pub state: MeterState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet: Option<String>,
    pub spend_per_hour: String,
    pub boxes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupState>,
    pub requests: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    /// Billed in all, base units with six more decimals, decimal text.
    pub billed_e6: String,
    /// Settled to the boxes so far, base units.
    pub paid: String,
    pub created_at: u64,
}

/// A wallet as its box shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletReport {
    pub network: String,
    pub wallet: String,
    #[serde(flatten)]
    pub view: WalletView,
    /// What is left once usage not yet settled is counted, base units.
    pub available: String,
    /// How long the balance lasts if every open meter on it spends as much
    /// as it allows, seconds. `None` while nothing is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runway_secs: Option<u64>,
    /// The runway is under a day.
    pub low: bool,
}

/// A lending box's side.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LendingView {
    /// Base units settled to it, before the fee came off.
    pub paid: String,
    /// Used and not yet settled, base units.
    pub owed: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lent_mib: Option<u64>,
}

/// Releases the meter's place when a request ends, however it ends.
struct OneAtATime<'a> {
    busy: &'a std::sync::Mutex<BTreeSet<String>>,
    meter: String,
}

impl Drop for OneAtATime<'_> {
    fn drop(&mut self) {
        if let Ok(mut b) = self.busy.lock() {
            b.remove(&self.meter);
        }
    }
}

/// Taps state kept beside the ledger, never written to it.
#[derive(Default)]
pub(super) struct TapsLive {
    /// Wallet balances read from the chain, by (network, wallet), and when.
    wallets: std::sync::Mutex<BTreeMap<(String, String), (WalletView, u64)>>,
    /// Meters with a request running.
    busy: std::sync::Mutex<BTreeSet<String>>,
    /// When each open tap was last read back.
    checked: std::sync::Mutex<BTreeMap<String, u64>>,
}

fn payout_of(state: &MarketState, seller: &str, family: Family) -> Option<String> {
    let s = state.sellers.get(seller)?;
    match family {
        Family::Evm => s.payout.clone(),
        Family::Solana => s.payout_solana.clone(),
    }
}

fn family_key(family: Family) -> &'static str {
    match family {
        Family::Evm => "evm",
        Family::Solana => "solana",
    }
}

impl Market {
    fn tap_chain(&self) -> &Arc<dyn TapChain> {
        &self.tap_chain
    }

    /// Use `chain` for taps instead of the configured escrows.
    #[must_use]
    pub fn with_tap_chain(mut self, chain: Arc<dyn TapChain>) -> Self {
        self.tap_chain = chain;
        self
    }

    fn tap_network(&self, network: Option<&str>) -> Result<UsdcChain, MarketError> {
        let usdc = self.opts.usdc.as_ref().ok_or(MarketError::Unconfigured)?;
        let chain = Self::usdc_chain(usdc, network)?;
        if chain.pay.is_none() {
            return Err(MarketError::Invalid(
                "this network has no escrow on this edge; pick another",
            ));
        }
        Ok(chain)
    }

    async fn wallet(
        &self,
        network: &str,
        wallet: &str,
        fresh: bool,
    ) -> Result<WalletView, MarketError> {
        let key = (network.to_string(), wallet.to_string());
        let now = now_secs();
        if !fresh {
            if let Ok(cache) = self.taps_live.wallets.lock() {
                if let Some((view, at)) = cache.get(&key) {
                    if now.saturating_sub(*at) < WALLET_SECS {
                        return Ok(*view);
                    }
                }
            }
        }
        let view = self.tap_chain().read_wallet(network, wallet).await?;
        if let Ok(mut cache) = self.taps_live.wallets.lock() {
            cache.insert(key, (view, now));
        }
        Ok(view)
    }

    /// The models this edge runs, as buyers paying on `network` see them,
    /// and the buyer's own meters, wallets and lending.
    ///
    /// # Errors
    /// No USDC escrow on this edge.
    pub async fn llm_overview(
        &self,
        inference: &Inference,
        lenders: &[(String, u16, u64)],
        tenant: &str,
        network: Option<&str>,
    ) -> Result<Value, MarketError> {
        let chain = self.tap_network(network)?;
        let models = inference
            .views(family_key(chain.network.family), lenders)
            .await;
        let (meters, wallets) = self.taps_account(inference, tenant).await;
        let lending = self.lending(inference, lenders, tenant).await;
        Ok(json!({
            "network": chain.network.key,
            "feeBps": self.opts.fee_bps,
            "models": models,
            "meters": meters,
            "wallets": wallets,
            "lending": lending,
            "noticeSecs": meter::NOTICE_SECS,
            "minSpendPerHour": meter::MIN_SPEND_PER_HOUR.to_string(),
            "maxSpendPerHour": meter::MAX_SPEND_PER_HOUR.to_string(),
        }))
    }

    async fn taps_account(
        &self,
        inference: &Inference,
        tenant: &str,
    ) -> (Vec<MeterView>, Vec<WalletReport>) {
        let state = self.state.lock().await.taps.clone();
        let now = now_secs();
        let mut views = Vec::new();
        let mut wallets: BTreeSet<(String, String)> = BTreeSet::new();
        for m in state.meters.values().filter(|m| m.buyer == tenant) {
            // Old ended meters drop off after a month.
            if matches!(m.state, MeterState::Ended | MeterState::Lapsed)
                && now.saturating_sub(m.created_at) > 30 * 24 * 3600
            {
                continue;
            }
            if let Some(w) = &m.wallet {
                wallets.insert((m.network.clone(), w.clone()));
            }
            let paid: u64 = m
                .taps
                .iter()
                .filter_map(|t| state.taps.get(t))
                .map(|t| t.paid)
                .sum();
            views.push(MeterView {
                id: m.id.clone(),
                model: m.model.clone(),
                network: m.network.clone(),
                state: m.state,
                reason: m.reason.map(EndReason::message),
                wallet: m.wallet.clone(),
                spend_per_hour: m.spend_per_hour.to_string(),
                boxes: m.taps.len(),
                group: inference.group(&m.group).await.map(|g| g.state),
                requests: m.requests,
                tokens_in: m.tokens_in,
                tokens_out: m.tokens_out,
                billed_e6: m.billed_e6.to_string(),
                paid: paid.to_string(),
                created_at: m.created_at,
            });
        }
        views.sort_by_key(|v| std::cmp::Reverse(v.created_at));
        let mut reports = Vec::new();
        for (network, wallet) in wallets {
            let Ok(view) = self.wallet(&network, &wallet, false).await else {
                continue;
            };
            let available = meter::funds(&state, &wallet, &network, &view) / E6;
            let rate: u64 = state
                .meters
                .values()
                .filter(|m| {
                    m.state == MeterState::Open
                        && m.network == network
                        && m.wallet.as_deref() == Some(wallet.as_str())
                })
                .map(|m| m.spend_per_hour)
                .sum();
            let runway_secs = (rate > 0).then(|| {
                u64::try_from(u128::from(available) * 3600 / u128::from(rate)).unwrap_or(u64::MAX)
            });
            reports.push(WalletReport {
                network,
                wallet,
                view,
                available: available.to_string(),
                runway_secs,
                low: runway_secs.is_some_and(|r| r < WARN_RUNWAY_SECS),
            });
        }
        (views, reports)
    }

    async fn lending(
        &self,
        inference: &Inference,
        lenders: &[(String, u16, u64)],
        tenant: &str,
    ) -> LendingView {
        let (paid, owed) = {
            let state = self.state.lock().await;
            meter::earned(&state.taps, tenant)
        };
        let group = inference.group_of(tenant).await;
        LendingView {
            paid: paid.to_string(),
            owed: owed.to_string(),
            model: group.as_ref().map(|g| g.model.clone()),
            group: group.map(|g| g.state),
            lent_mib: lenders.iter().find(|l| l.0 == tenant).map(|l| l.2),
        }
    }

    /// Quote `model` for `buyer`, paying on `network` at most
    /// `spend_per_hour` base units per hour: pick or reuse its group and
    /// write down one tap per box, waiting for the wallet.
    ///
    /// # Errors
    /// Not enough boxes lend memory, the buyer already runs the model, or
    /// the limit is out of range.
    pub async fn tap_quote(
        &self,
        inference: &Inference,
        lenders: &[(String, u16, u64)],
        buyer: &str,
        model: &str,
        network: Option<&str>,
        spend_per_hour: u64,
    ) -> Result<TapQuote, MarketError> {
        let chain = self.tap_network(network)?;
        let family = chain.network.family;
        let sellers = self.state.lock().await.sellers.clone();
        let eligible = |id: &str| {
            sellers.get(id).is_some_and(|s| match family {
                Family::Evm => s.payout.is_some(),
                Family::Solana => s.payout_solana.is_some(),
            })
        };
        if inference.model(model).is_none() {
            return Err(MarketError::Invalid("this edge does not run that model"));
        }
        let group = inference
            .group_for(model, family_key(family), lenders, eligible)
            .await?;
        let mut state = self.state.lock().await;
        if meter::open_meter(&state.taps, buyer, model).is_some() {
            return Err(MarketError::Conflict(
                "this model already runs for you; stop it first",
            ));
        }
        let payees: Vec<Payee> = group
            .members
            .iter()
            .map(|m| {
                payout_of(&state, &m.tenant, family).map(|address| Payee {
                    seller: m.tenant.clone(),
                    mib: m.mib,
                    address,
                })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(MarketError::Unavailable(
                "a box in this model's group can no longer be paid; try again later",
            ))?;
        let mut next = state.clone();
        let now = now_secs();
        let spec = meter::QuoteSpec {
            buyer,
            model,
            group: &group.id,
            network: chain.network.key,
            spend_per_hour,
            fee_bps: self.opts.fee_bps,
            payees: &payees,
            now,
        };
        let id = meter::quote(&mut next.taps, &spec, |prefix| random_id(prefix).ok())
            .map_err(MarketError::Invalid)?;
        let m = next.taps.meters[&id].clone();
        let mut taps = Vec::new();
        for t in &m.taps {
            let tap = &next.taps.taps[t];
            let key = self
                .tap_chain()
                .tap_key(chain.network.key, t)
                .ok_or(MarketError::Unconfigured)?;
            taps.push(QuotedTap {
                id: t.clone(),
                key: format!("0x{}", crate::identity::to_hex(&key)),
                seller: tap.payee.clone(),
                rate: tap.rate.to_string(),
            });
        }
        self.commit(&mut state, next).await?;
        Ok(TapQuote {
            meter: id,
            model: model.to_string(),
            network: chain.network.key.to_string(),
            chain: chain.public(),
            fee_bps: self.opts.fee_bps,
            spend_per_hour: spend_per_hour.to_string(),
            taps,
            open_before: now + meter::QUOTE_SECS,
        })
    }

    fn own_meter(
        state: &MarketState,
        buyer: &str,
        meter_id: &str,
    ) -> Result<meter::Meter, MarketError> {
        state
            .taps
            .meters
            .get(meter_id)
            .filter(|m| m.buyer == buyer)
            .cloned()
            .ok_or(MarketError::NotFound)
    }

    /// The buyer's wallet says it opened the meter's taps: read them back
    /// under that wallet and open the meter if every one is as quoted. The
    /// escrow keeps a tap under its id and the wallet that opened it, so a
    /// wallet that copied the ids first holds taps of its own and changes
    /// nothing here.
    ///
    /// # Errors
    /// A tap is missing or differs from the quote, or `wallet` is not an
    /// address on the meter's network.
    pub async fn tap_confirm(
        &self,
        buyer: &str,
        meter_id: &str,
        wallet: &str,
    ) -> Result<MeterState, MarketError> {
        let m = Self::own_meter(&*self.state.lock().await, buyer, meter_id)?;
        if m.state != MeterState::Quoted {
            return Ok(m.state);
        }
        let wallet_key = self
            .tap_chain()
            .payee_key(&m.network, wallet)
            .ok_or(MarketError::Invalid("not a wallet address on this network"))?;
        let wallet = self.tap_chain().wallet_name(&m.network, &wallet_key);
        let mut seen = BTreeMap::new();
        for t in &m.taps {
            if let Some(chain_tap) = self.tap_chain().read_tap(&m.network, &wallet, t).await? {
                seen.insert(t.clone(), chain_tap);
            }
        }
        let chain = Arc::clone(self.tap_chain());
        let network = m.network.clone();
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        meter::confirm(
            &mut next.taps,
            meter_id,
            &seen,
            |address| chain.payee_key(&network, address),
            |key| chain.wallet_name(&network, key),
            now_secs(),
        )
        .map_err(MarketError::Conflict)?;
        let opened = next.taps.meters[meter_id].state;
        self.commit(&mut state, next).await?;
        if let Some(w) = state.taps.meters[meter_id].wallet.clone() {
            if let Ok(mut cache) = self.taps_live.wallets.lock() {
                cache.remove(&(network, w));
            }
        }
        tracing::info!(target: Action::Market.target(), "meter {meter_id} is open");
        Ok(opened)
    }

    /// The buyer stops a meter: no more requests, and its taps are closed and
    /// settled one last time by the next pass.
    ///
    /// A quote stopped with the `wallet` that began opening it (the wallet
    /// failed partway) is read back from the escrow first: the taps it opened
    /// are closed like those of an open meter, and the wallet stays on the
    /// meter so its balance stays in view.
    ///
    /// # Errors
    /// Not the buyer's meter, or a `wallet` that is not an address there.
    pub async fn tap_stop(
        &self,
        buyer: &str,
        meter_id: &str,
        wallet: Option<&str>,
    ) -> Result<MeterState, MarketError> {
        let m = Self::own_meter(&*self.state.lock().await, buyer, meter_id)?;
        let mut opened = None;
        if let (MeterState::Quoted, Some(wallet)) = (m.state, wallet) {
            let key = self
                .tap_chain()
                .payee_key(&m.network, wallet)
                .ok_or(MarketError::Invalid("not a wallet address on this network"))?;
            let wallet = self.tap_chain().wallet_name(&m.network, &key);
            let mut seen = BTreeMap::new();
            for t in &m.taps {
                if let Some(chain_tap) = self.tap_chain().read_tap(&m.network, &wallet, t).await? {
                    seen.insert(t.clone(), chain_tap);
                }
            }
            opened = Some((wallet, seen));
        }
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        let changed = match &opened {
            Some((wallet, seen)) => meter::end_quoted(&mut next.taps, meter_id, seen, wallet),
            None => false,
        } || meter::end(&mut next.taps, meter_id, EndReason::Buyer);
        if !changed {
            return Ok(next.taps.meters[meter_id].state);
        }
        let after = next.taps.meters[meter_id].state;
        self.commit(&mut state, next).await?;
        Ok(after)
    }

    /// Serve one request on the buyer's meter and bill what the head counted.
    ///
    /// The prompt is counted first by the model's own tokenizer, so the most
    /// the request can cost (that prompt and every answer token asked for) is
    /// known before anything runs; a request the wallet or the hourly limit
    /// cannot cover is refused. One request at a time per meter. A request
    /// that fails is not billed.
    ///
    /// # Errors
    /// [`MarketError::PaymentRequired`], [`MarketError::Busy`],
    /// [`MarketError::Unavailable`], or a malformed request.
    pub async fn tap_chat(
        &self,
        inference: &Inference,
        buyer: &str,
        meter_id: &str,
        req: &Value,
    ) -> Result<Value, MarketError> {
        let m = Self::own_meter(&*self.state.lock().await, buyer, meter_id)?;
        if m.state != MeterState::Open {
            return Err(MarketError::Conflict(Refusal::Closed.message()));
        }
        let model = inference
            .model(&m.model)
            .ok_or(MarketError::Unavailable(
                "this edge no longer runs that model",
            ))?
            .clone();
        let group = inference
            .group(&m.group)
            .await
            .ok_or(MarketError::Conflict(EndReason::GroupLeft.message()))?;
        match group.state {
            GroupState::Ready => {}
            GroupState::Loading => {
                return Err(MarketError::Unavailable(
                    "the model is still loading onto its boxes; try again in a few minutes",
                ))
            }
            GroupState::Down => {
                return Err(MarketError::Unavailable(
                    "a box of this model is restarting; try again in a few minutes",
                ))
            }
        }
        let ask = chat_ask(req, model.context)?;
        let _turn = {
            let mut busy = self
                .taps_live
                .busy
                .lock()
                .map_err(|_| MarketError::Store("lock".into()))?;
            if !busy.insert(meter_id.to_string()) {
                return Err(MarketError::Busy(
                    "this model is still answering your last request",
                ));
            }
            OneAtATime {
                busy: &self.taps_live.busy,
                meter: meter_id.to_string(),
            }
        };
        let wallet = m
            .wallet
            .clone()
            .ok_or(MarketError::Conflict(Refusal::Closed.message()))?;
        let view = self.wallet(&m.network, &wallet, false).await?;
        let prompt = inference.prompt_tokens(&group.id, &ask.messages).await?;
        if prompt.saturating_add(ask.max_tokens) > u64::from(model.context) {
            return Err(MarketError::Invalid(
                "the messages and the answer do not fit the model's context; send less or ask for fewer tokens",
            ));
        }
        let reserve = meter::cost_e6(prompt, ask.max_tokens, model.price_in, model.price_out);
        let room = {
            let state = self.state.lock().await;
            meter::headroom(&state.taps, meter_id, &view, now_secs())
        };
        match room {
            Err(Refusal::Closed) => return Err(MarketError::Conflict(Refusal::Closed.message())),
            Err(Refusal::Balance) => {
                return Err(MarketError::PaymentRequired(Refusal::Balance.message()))
            }
            Err(Refusal::Rate) => return Err(MarketError::PaymentRequired(Refusal::Rate.message())),
            Ok(room) if room < reserve => {
                return Err(MarketError::PaymentRequired(
                    "this request could cost more than your balance or hourly limit allows; ask for fewer tokens, top up, or allow more per hour",
                ))
            }
            Ok(_) => {}
        }
        let answer = inference.chat(&group.id, &chat_body(&ask)).await?;
        // What the head counted, never more than was reserved for.
        let tokens_in = answer.tokens_in.min(prompt);
        let tokens_out = answer.tokens_out.min(ask.max_tokens);
        let cost =
            meter::cost_e6(tokens_in, tokens_out, model.price_in, model.price_out).min(reserve);
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        meter::bill(&mut next.taps, meter_id, cost, tokens_in, tokens_out);
        let billed = next.taps.meters[meter_id].billed_e6;
        self.commit(&mut state, next).await?;
        Ok(json!({
            "content": answer.content,
            "finishReason": answer.finish_reason,
            "tokensIn": tokens_in,
            "tokensOut": tokens_out,
            "costE6": cost.to_string(),
            "billedE6": billed.to_string(),
        }))
    }

    /// Groups whose meters are still waiting or open.
    pub async fn groups_in_use(&self) -> BTreeSet<String> {
        let state = self.state.lock().await;
        state
            .taps
            .meters
            .values()
            .filter(|m| matches!(m.state, MeterState::Quoted | MeterState::Open))
            .map(|m| m.group.clone())
            .collect()
    }

    /// End every meter on `group`, which stopped for good.
    pub async fn end_group(&self, group: &str, reason: EndReason) {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        let ids: Vec<String> = next
            .taps
            .meters
            .values()
            .filter(|m| m.group == group)
            .map(|m| m.id.clone())
            .collect();
        let mut changed = false;
        for id in ids {
            changed |= meter::end(&mut next.taps, &id, reason);
        }
        if changed {
            if let Err(e) = self.commit(&mut state, next).await {
                tracing::error!(target: Action::Market.target(), "could not end the meters of group {group}: {e}");
            }
        }
    }

    /// The taps of meters with a request running now.
    fn answering_taps(&self, taps: &meter::Taps) -> BTreeSet<String> {
        let busy = self
            .taps_live
            .busy
            .lock()
            .map(|b| b.clone())
            .unwrap_or_default();
        busy.iter()
            .filter_map(|m| taps.meters.get(m))
            .flat_map(|m| m.taps.iter().cloned())
            .collect()
    }

    /// Follow the taps once: record settles and closes the escrow shows,
    /// notice taps a wallet closed, close the taps of ended meters, settle
    /// what is due, and finish meters with nothing left to bill.
    pub async fn taps_pass(&self) {
        let Some(usdc) = self.opts.usdc.as_ref() else {
            return;
        };
        let now = now_secs();
        let snapshot = {
            let mut state = self.state.lock().await;
            let mut next = state.clone();
            if !meter::tidy(&mut next.taps, now).is_empty() {
                if let Err(e) = self.commit(&mut state, next).await {
                    tracing::error!(target: Action::Market.target(), "could not tidy meters: {e}");
                }
            }
            state.taps.clone()
        };
        if snapshot.taps.values().all(|t| t.done || !t.opened) {
            return;
        }
        let _one_pass = self.escrow_lock.lock().await;
        let chain = Arc::clone(self.tap_chain());
        // What the chain says, applied after the reads.
        let mut seen: Vec<(String, Option<ChainTap>)> = Vec::new();
        let mut stale_settles: Vec<String> = Vec::new();
        let mut stale_closes: Vec<String> = Vec::new();
        // A meter answering a request keeps its taps as they are until the
        // answer is billed: nothing of it is marked finished or closed
        // under the request, so a stop during an answer cannot leave the
        // answer unpaid.
        let answering = self.answering_taps(&snapshot);
        for tap in snapshot
            .taps
            .values()
            .filter(|t| t.opened && !t.done && !answering.contains(&t.id))
        {
            let resend = match usdc.chain(&tap.network).map(|c| c.network.family) {
                Some(Family::Solana) => SOLANA_RESEND_SECS,
                _ => meter::RESEND_SECS,
            };
            let following = tap.settle.is_some() || tap.close.is_some();
            let due_check = {
                let checked = self
                    .taps_live
                    .checked
                    .lock()
                    .map(|c| c.get(&tap.id).copied());
                checked
                    .ok()
                    .flatten()
                    .is_none_or(|at| now.saturating_sub(at) >= TAP_CHECK_SECS)
            };
            if !following && !due_check {
                continue;
            }
            let Some(wallet) = wallet_of(&snapshot, &tap.id) else {
                continue;
            };
            match chain.read_tap(&tap.network, wallet, &tap.id).await {
                Ok(t) => {
                    if let Ok(mut c) = self.taps_live.checked.lock() {
                        c.insert(tap.id.clone(), now);
                    }
                    let landed_settle = tap.settle.as_ref().is_some_and(|_| match t {
                        Some(ct) => ct.settled_at > tap.settled_at,
                        None => true,
                    });
                    if !landed_settle
                        && tap
                            .settle
                            .as_ref()
                            .is_some_and(|s| now.saturating_sub(s.sent_at) >= resend)
                    {
                        stale_settles.push(tap.id.clone());
                    }
                    let landed_close = t.is_none_or(|ct| ct.closed_at.is_some());
                    if !landed_close
                        && tap
                            .close
                            .as_ref()
                            .is_some_and(|s| now.saturating_sub(s.sent_at) >= resend)
                    {
                        stale_closes.push(tap.id.clone());
                    }
                    seen.push((tap.id.clone(), t));
                }
                Err(e) => {
                    tracing::warn!(target: Action::Market.target(), "reading tap {}: {e}", tap.id)
                }
            }
        }
        // Apply what was seen, then decide what to send against that.
        let work = {
            let mut state = self.state.lock().await;
            let mut next = state.clone();
            for (id, t) in &seen {
                let Some(tap) = next.taps.taps.get(id).cloned() else {
                    continue;
                };
                match t {
                    Some(ct) => {
                        if let Some(s) = &tap.settle {
                            if ct.settled_at > tap.settled_at {
                                meter::settled(&mut next.taps, id, s.amount, ct.settled_at);
                            }
                        }
                        if let Some(closed_at) = ct.closed_at {
                            if tap.closed_at.is_none() {
                                meter::closed(&mut next.taps, id, closed_at);
                            }
                        }
                    }
                    // Solana closes a tap's account once it is closed and
                    // settled to its close: finished.
                    None => {
                        if let Some(s) = &tap.settle {
                            meter::settled(
                                &mut next.taps,
                                id,
                                s.amount,
                                tap.closed_at.unwrap_or(now),
                            );
                        }
                        let at = tap.closed_at.unwrap_or(now);
                        meter::closed(&mut next.taps, id, at);
                        if let Some(t) = next.taps.taps.get_mut(id) {
                            t.done = true;
                        }
                    }
                }
            }
            for id in &stale_settles {
                if let Some(t) = next.taps.taps.get_mut(id) {
                    t.settle = None;
                }
            }
            for id in &stale_closes {
                if let Some(t) = next.taps.taps.get_mut(id) {
                    t.close = None;
                }
            }
            meter::tidy(&mut next.taps, now);
            if next != state.state {
                if let Err(e) = self.commit(&mut state, next).await {
                    tracing::error!(target: Action::Market.target(), "could not record tap progress: {e}");
                    return;
                }
            }
            state.taps.clone()
        };
        // Wallets with something to settle, read fresh so a settle never
        // asks for more than the escrow holds.
        let mut views: BTreeMap<(String, String), WalletView> = BTreeMap::new();
        for m in work
            .meters
            .values()
            .filter(|m| m.state != MeterState::Lapsed)
        {
            let Some(w) = &m.wallet else { continue };
            let key = (m.network.clone(), w.clone());
            if views.contains_key(&key) {
                continue;
            }
            let owes = m
                .taps
                .iter()
                .filter_map(|t| work.taps.get(t))
                .any(|t| !t.done && t.accrued_e6 >= E6);
            if !owes {
                continue;
            }
            match self.wallet(&m.network, w, true).await {
                Ok(v) => {
                    views.insert(key, v);
                }
                Err(e) => {
                    tracing::warn!(target: Action::Market.target(), "reading wallet {w}: {e}")
                }
            }
        }
        let mut sent: Vec<(String, bool, meter::Sent)> = Vec::new();
        for c in &usdc.chains {
            let network = c.network.key;
            let (every, at) = match c.network.family {
                Family::Solana => (meter::SOLANA_SETTLE_EVERY_SECS, 0),
                Family::Evm => (meter::EVM_SETTLE_EVERY_SECS, meter::EVM_SETTLE_AT),
            };
            let by_wallet: BTreeMap<String, WalletView> = views
                .iter()
                .filter(|((n, _), _)| n == network)
                .map(|((_, w), v)| (w.clone(), *v))
                .collect();
            for d in meter::due(&work, now, network, every, at, &by_wallet) {
                let replace = stale_settles.contains(&d.tap);
                let Some(wallet) = wallet_of(&work, &d.tap) else {
                    continue;
                };
                match chain
                    .settle(network, wallet, &d.tap, d.amount, replace)
                    .await
                {
                    Ok(tx) => {
                        tracing::info!(target: Action::Market.target(), "tap {}: settling {} as {tx}", d.tap, d.amount);
                        sent.push((
                            d.tap.clone(),
                            true,
                            meter::Sent {
                                amount: d.amount,
                                tx,
                                sent_at: now,
                            },
                        ));
                    }
                    Err(e) => {
                        tracing::warn!(target: Action::Market.target(), "tap {}: settle of {}: {e}", d.tap, d.amount)
                    }
                }
            }
            let answering = self.answering_taps(&work);
            for tap in meter::to_close(&work, network)
                .into_iter()
                .filter(|t| !answering.contains(t))
            {
                let replace = stale_closes.contains(&tap);
                let Some(wallet) = wallet_of(&work, &tap) else {
                    continue;
                };
                match chain.close(network, wallet, &tap, replace).await {
                    Ok(tx) => {
                        tracing::info!(target: Action::Market.target(), "tap {tap}: closing as {tx}");
                        sent.push((
                            tap.clone(),
                            false,
                            meter::Sent {
                                amount: 0,
                                tx,
                                sent_at: now,
                            },
                        ));
                    }
                    Err(e) => {
                        tracing::warn!(target: Action::Market.target(), "tap {tap}: close: {e}")
                    }
                }
            }
        }
        if sent.is_empty() {
            return;
        }
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        for (id, settle, s) in sent {
            if let Some(t) = next.taps.taps.get_mut(&id) {
                if settle {
                    t.settle = Some(s);
                } else {
                    t.close = Some(s);
                }
            }
        }
        if let Err(e) = self.commit(&mut state, next).await {
            tracing::error!(target: Action::Market.target(), "could not record sent tap calls: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;
    use std::time::Duration;

    use super::*;
    use crate::inference::{HeadFuture, HeadSpec, Heads, InferenceOpts};
    use crate::market::{MarketOpts, Seller, DEFAULT_COIN_FEE_LIMIT, DEFAULT_STORAGE_CLASS};
    use crate::stripe_mode::StripeMode;

    const BUYER: &str = "buyer-box";
    const WALLET: &str = "0x00000000000000000000000000000000000000b1";
    const THIEF: &str = "0x00000000000000000000000000000000000000ee";
    const HOUR: u64 = 3600;
    /// Base units per million tokens: a token in costs one base unit and a
    /// token out two, so the sums below stay readable.
    const PRICE_IN: u64 = 1_000_000;
    const PRICE_OUT: u64 = 2_000_000;

    fn seller_address(n: u8) -> String {
        format!("0x{:040x}", 0xa0 + u32::from(n))
    }

    /// An escrow kept in memory, with the contract's rules: a tap is kept
    /// under the wallet that opened it and its id.
    #[derive(Default)]
    struct MemoryEscrow {
        taps: StdMutex<BTreeMap<([u8; 32], String), ChainTap>>,
        balances: StdMutex<BTreeMap<[u8; 32], u64>>,
        asks: StdMutex<BTreeMap<[u8; 32], (u64, u64)>>,
        /// Paid out: seller keys and the treasury (all ones).
        paid: StdMutex<BTreeMap<[u8; 32], u64>>,
        refused: StdMutex<Vec<String>>,
        sends: StdMutex<u32>,
    }

    fn key(address: &str) -> [u8; 32] {
        crate::evm_tx::address_word(crate::stablecoin::address_bytes(address).unwrap())
    }

    const TREASURY: [u8; 32] = [1; 32];

    impl MemoryEscrow {
        fn top_up(&self, wallet: &str, amount: u64) {
            *self
                .balances
                .lock()
                .unwrap()
                .entry(key(wallet))
                .or_default() += amount;
        }
        fn balance(&self, wallet: &str) -> u64 {
            self.balances
                .lock()
                .unwrap()
                .get(&key(wallet))
                .copied()
                .unwrap_or(0)
        }
        fn paid(&self, who: [u8; 32]) -> u64 {
            self.paid.lock().unwrap().get(&who).copied().unwrap_or(0)
        }
        /// `openTaps` as `wallet` signs it, with the quote's sellers and
        /// rates unless the test changes them.
        fn open(&self, wallet: &str, q: &TapQuote, tweak: impl Fn(usize, &mut ChainTap)) {
            let now = now_secs();
            let mut taps = self.taps.lock().unwrap();
            for (n, t) in q.taps.iter().enumerate() {
                let mut ct = ChainTap {
                    buyer: key(wallet),
                    seller: key(&t.seller),
                    rate: t.rate.parse().unwrap(),
                    fee_bps: q.fee_bps,
                    settled_at: now - meter::HEADSTART_SECS,
                    closed_at: None,
                };
                tweak(n, &mut ct);
                taps.insert((ct.buyer, t.id.clone()), ct);
            }
        }
        fn backdate(&self, secs: u64) {
            for t in self.taps.lock().unwrap().values_mut() {
                t.settled_at -= secs;
                if let Some(c) = t.closed_at.as_mut() {
                    *c -= secs;
                }
            }
        }
        fn buyer_close(&self, tap: &str) {
            let mut taps = self.taps.lock().unwrap();
            let t = taps.get_mut(&(key(WALLET), tap.to_string())).unwrap();
            t.closed_at = Some(now_secs().max(t.settled_at));
        }
    }

    impl TapChain for MemoryEscrow {
        fn tap_key(&self, _: &str, tap: &str) -> Option<[u8; 32]> {
            Some(crate::stablecoin::order_id_hash(tap))
        }
        fn payee_key(&self, _: &str, address: &str) -> Option<[u8; 32]> {
            crate::stablecoin::address_bytes(address).map(crate::evm_tx::address_word)
        }
        fn wallet_name(&self, _: &str, key: &[u8; 32]) -> String {
            format!("0x{}", crate::identity::to_hex(&key[12..]))
        }
        fn read_tap<'a>(
            &'a self,
            _: &'a str,
            wallet: &'a str,
            tap: &'a str,
        ) -> ChainFuture<'a, Option<ChainTap>> {
            let t = self
                .taps
                .lock()
                .unwrap()
                .get(&(key(wallet), tap.to_string()))
                .copied();
            Box::pin(async move { Ok(t) })
        }
        fn read_wallet<'a>(&'a self, _: &'a str, wallet: &'a str) -> ChainFuture<'a, WalletView> {
            let k = key(wallet);
            let balance = self.balances.lock().unwrap().get(&k).copied().unwrap_or(0);
            let (asked, not_before) = self.asks.lock().unwrap().get(&k).copied().unwrap_or((0, 0));
            Box::pin(async move {
                Ok(WalletView {
                    balance,
                    asked,
                    not_before,
                })
            })
        }
        fn settle<'a>(
            &'a self,
            _: &'a str,
            wallet: &'a str,
            tap: &'a str,
            amount: u64,
            _: bool,
        ) -> ChainFuture<'a, String> {
            *self.sends.lock().unwrap() += 1;
            let refuse = |why: &str| {
                self.refused.lock().unwrap().push(format!("{tap}: {why}"));
                Err(MarketError::Conflict("the escrow refused the call"))
            };
            let now = now_secs();
            let mut taps = self.taps.lock().unwrap();
            let result = match taps.get_mut(&(key(wallet), tap.to_string())) {
                None => refuse("no tap"),
                Some(t) => {
                    let end = t.closed_at.unwrap_or(now);
                    let cap = t.rate
                        * (end.saturating_sub(t.settled_at)).min(meter::MAX_ACCRUAL_SECS)
                        / 3600;
                    let mut balances = self.balances.lock().unwrap();
                    let balance = balances.entry(t.buyer).or_default();
                    if amount == 0 {
                        refuse("zero")
                    } else if end <= t.settled_at {
                        refuse("too early")
                    } else if amount > cap {
                        refuse("over cap")
                    } else if amount > *balance {
                        refuse("over balance")
                    } else {
                        *balance -= amount;
                        t.settled_at = end;
                        let fee = amount * u64::from(t.fee_bps) / 10_000;
                        let mut paid = self.paid.lock().unwrap();
                        *paid.entry(TREASURY).or_default() += fee;
                        *paid.entry(t.seller).or_default() += amount - fee;
                        Ok(format!("0xsettle{tap}{amount}"))
                    }
                }
            };
            Box::pin(async move { result })
        }
        fn close<'a>(
            &'a self,
            _: &'a str,
            wallet: &'a str,
            tap: &'a str,
            _: bool,
        ) -> ChainFuture<'a, String> {
            let now = now_secs();
            let mut taps = self.taps.lock().unwrap();
            let result = match taps.get_mut(&(key(wallet), tap.to_string())) {
                Some(t) if t.closed_at.is_none() => {
                    t.closed_at = Some(now.max(t.settled_at));
                    Ok(format!("0xclose{tap}"))
                }
                _ => Err(MarketError::Conflict("the escrow refused the call")),
            };
            Box::pin(async move { result })
        }
    }

    /// What a fake head says about the next request.
    #[derive(Clone)]
    struct Script {
        healthy: bool,
        /// Answer tokens reported, `None` for as many as were asked for.
        tokens_out: Option<u64>,
        /// Prompt tokens reported over what the tokenizer counted.
        extra_in: u64,
        fail: bool,
        /// Before every answer to the head's routes.
        delay: Duration,
        /// Before an answer to a chat only.
        answer_delay: Duration,
    }

    struct FakeHeads {
        script: StdMutex<Script>,
        started: StdMutex<Vec<(String, HeadSpec)>>,
        stopped: StdMutex<Vec<String>>,
    }

    impl FakeHeads {
        fn new() -> Self {
            Self {
                script: StdMutex::new(Script {
                    healthy: true,
                    tokens_out: None,
                    extra_in: 0,
                    fail: false,
                    delay: Duration::ZERO,
                    answer_delay: Duration::ZERO,
                }),
                started: StdMutex::new(Vec::new()),
                stopped: StdMutex::new(Vec::new()),
            }
        }
        fn set(&self, f: impl FnOnce(&mut Script)) {
            f(&mut self.script.lock().unwrap());
        }
    }

    impl Heads for FakeHeads {
        fn start<'a>(&'a self, group: &'a str, spec: &'a HeadSpec) -> HeadFuture<'a, ()> {
            self.started
                .lock()
                .unwrap()
                .push((group.to_string(), spec.clone()));
            Box::pin(async { Ok(()) })
        }
        fn stop<'a>(&'a self, group: &'a str) -> HeadFuture<'a, ()> {
            self.stopped.lock().unwrap().push(group.to_string());
            Box::pin(async { Ok(()) })
        }
        fn request<'a>(
            &'a self,
            _: &'a str,
            _: &'a str,
            path: &'a str,
            body: Option<Vec<u8>>,
            _: Duration,
        ) -> HeadFuture<'a, (u16, Vec<u8>)> {
            let s = self.script.lock().unwrap().clone();
            Box::pin(async move {
                tokio::time::sleep(s.delay).await;
                if path == "/v1/chat/completions" {
                    tokio::time::sleep(s.answer_delay).await;
                }
                let body: Value = body.map_or(Value::Null, |b| serde_json::from_slice(&b).unwrap());
                let reply = match path {
                    "/health" if s.healthy => json!({"status": "ok"}),
                    "/health" => return Err("down".into()),
                    "/apply-template" => {
                        let text: String = body["messages"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|m| m["content"].as_str().unwrap().to_string())
                            .collect();
                        json!({ "prompt": format!("<s>{text}") })
                    }
                    // One token per character.
                    "/tokenize" => json!({
                        "tokens": vec![1; body["content"].as_str().unwrap().chars().count()]
                    }),
                    "/v1/chat/completions" if s.fail => return Ok((500, b"{}".to_vec())),
                    "/v1/chat/completions" => {
                        let text: usize = body["messages"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|m| m["content"].as_str().unwrap().chars().count())
                            .sum();
                        let max = body["max_tokens"].as_u64().unwrap();
                        json!({
                            "choices": [{"message": {"content": "Once upon a time"}, "finish_reason": "length"}],
                            "usage": {
                                "prompt_tokens": text as u64 + 3 + s.extra_in,
                                "completion_tokens": s.tokens_out.unwrap_or(max),
                            }
                        })
                    }
                    _ => return Ok((404, Vec::new())),
                };
                Ok((200, serde_json::to_vec(&reply).unwrap()))
            })
        }
    }

    /// A directory removed when the test ends.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new() -> Self {
            let n = crate::identity::random_hex(6).unwrap();
            let p = std::env::temp_dir().join(format!("market-taps-{n}"));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct World {
        _dir: Scratch,
        market: Market,
        chain: Arc<MemoryEscrow>,
        heads: Arc<FakeHeads>,
        inference: Inference,
        lenders: Vec<(String, u16, u64)>,
    }

    fn usdc() -> UsdcOpts {
        UsdcOpts {
            chains: vec![UsdcChain {
                network: crate::stablecoin::network("base-sepolia").unwrap(),
                rpc_file: "/nonexistent".into(),
                pay: Some("0x00000000000000000000000000000000000e5c40".into()),
                treasury: "0x0101010101010101010101010101010101010101".into(),
                genesis: None,
            }],
            ens_rpc_file: None,
            price_sources: [
                crate::coins::PriceSource::kraken(),
                crate::coins::PriceSource::coingecko(),
            ],
            releaser_file: None,
        }
    }

    /// An edge with two boxes lending 8 and 24 GiB, a 32 GiB model, and a
    /// buyer with nothing in the escrow yet.
    async fn world() -> World {
        let dir = Scratch::new();
        let mut st = MarketState::default();
        for n in 0..3u8 {
            st.sellers.insert(
                format!("seller-{n}"),
                Seller {
                    account_id: String::new(),
                    ready: false,
                    box_uuid: None,
                    payout: Some(seller_address(n)),
                    payout_solana: None,
                },
            );
        }
        st.mode = Some(StripeMode::Test);
        let file = dir.path().join("market.json");
        std::fs::write(
            dir.path().join("market-test.json"),
            serde_json::to_vec(&st).unwrap(),
        )
        .unwrap();
        let chain = Arc::new(MemoryEscrow::default());
        let market = Market::open(MarketOpts {
            state_file: file.to_string_lossy().into_owned(),
            gate_socket: "/nonexistent".into(),
            return_url: "https://example.test/market".into(),
            currency: "usd".into(),
            fee_bps: 400,
            storage_class: DEFAULT_STORAGE_CLASS.into(),
            storage_image: None,
            hardware_catalogue: None,
            vms: None,
            usdc: Some(usdc()),
            btc: None,
            xmr: None,
            cards: false,
            zec: None,
            coin_fee_limit: DEFAULT_COIN_FEE_LIMIT,
            offline_refund_secs: crate::slots::DEFAULT_OFFLINE_SECS,
            direct_pay: false,
        })
        .await
        .unwrap()
        .with_tap_chain(chain.clone());
        market.use_mode(StripeMode::Test).await.unwrap();
        let models = dir.path().join("models.json");
        std::fs::write(
            &models,
            serde_json::to_vec(&json!([{
                "id": "big", "name": "Big model", "file": "/models/big.gguf",
                "needMiB": 32768, "context": 4096,
                "priceIn": PRICE_IN, "priceOut": PRICE_OUT, "licence": "test"
            }]))
            .unwrap(),
        )
        .unwrap();
        let heads = Arc::new(FakeHeads::new());
        let inference = Inference::open(
            InferenceOpts {
                models_file: models.to_string_lossy().into_owned(),
                port_range: (23000, 23100),
                env_dir: "/nonexistent".into(),
                socket_dir: "/nonexistent".into(),
            },
            (20000, 20100),
            dir.path().join("inference.json"),
            heads.clone(),
        )
        .await
        .unwrap();
        World {
            _dir: dir,
            market,
            chain,
            heads,
            inference,
            lenders: vec![
                ("seller-0".into(), 20000, 8192),
                ("seller-1".into(), 20001, 24576),
            ],
        }
    }

    impl World {
        async fn quote(&self, spend: u64) -> Result<TapQuote, MarketError> {
            self.market
                .tap_quote(&self.inference, &self.lenders, BUYER, "big", None, spend)
                .await
        }

        /// Quoted, opened by the wallet with `balance`, confirmed, and the
        /// group's head loaded.
        async fn open(&self, spend: u64, balance: u64) -> String {
            let q = self.quote(spend).await.unwrap();
            self.chain.top_up(WALLET, balance);
            self.chain.open(WALLET, &q, |_, _| {});
            assert_eq!(
                self.market
                    .tap_confirm(BUYER, &q.meter, WALLET)
                    .await
                    .unwrap(),
                MeterState::Open
            );
            self.groups_pass().await;
            q.meter
        }

        async fn groups_pass(&self) {
            let in_use = self.market.groups_in_use().await;
            for b in self
                .inference
                .pass(&self.lenders, |g| in_use.contains(g))
                .await
            {
                self.market.end_group(&b.group, b.reason).await;
            }
        }

        async fn ask(&self, meter: &str, text: &str, max: u64) -> Result<Value, MarketError> {
            self.market
                .tap_chat(
                    &self.inference,
                    BUYER,
                    meter,
                    &json!({"messages": [{"role": "user", "content": text}], "maxTokens": max}),
                )
                .await
        }

        async fn taps(&self) -> meter::Taps {
            self.market.state.lock().await.taps.clone()
        }

        /// Let `secs` go by for the ledger, the escrow and the groups.
        async fn later(&self, secs: u64) {
            let mut state = self.market.state.lock().await;
            for m in state.taps.meters.values_mut() {
                m.created_at -= secs;
                if let Some(o) = m.opened_at.as_mut() {
                    *o -= secs;
                }
            }
            for t in state.taps.taps.values_mut() {
                t.settled_at = t.settled_at.saturating_sub(secs);
                if let Some(c) = t.closed_at.as_mut() {
                    *c -= secs;
                }
                for s in [t.settle.as_mut(), t.close.as_mut()].into_iter().flatten() {
                    s.sent_at -= secs;
                }
            }
            drop(state);
            self.chain.backdate(secs);
            self.inference.backdate(secs).await;
            if let Ok(mut c) = self.market.taps_live.wallets.lock() {
                c.clear();
            }
            if let Ok(mut c) = self.market.taps_live.checked.lock() {
                c.clear();
            }
        }
    }

    #[tokio::test]
    async fn a_request_is_billed_its_tokens_split_by_memory_and_settled_to_each_box() {
        let w = world().await;
        let meter = w.open(1_000_000, 5_000_000).await;
        // The head spreads the layers by what each box lends.
        let (_, spec) = w.heads.started.lock().unwrap()[0].clone();
        assert_eq!(spec.split, [24576, 8192]);
        // "<s>" and 7 characters: 10 tokens in, 100 out.
        let a = w.ask(&meter, "a story", 100).await.unwrap();
        assert_eq!(a["tokensIn"], 10);
        assert_eq!(a["tokensOut"], 100);
        assert_eq!(a["costE6"], (10 * PRICE_IN + 100 * PRICE_OUT).to_string());
        let t = w.taps().await;
        let accrued: Vec<u64> = t.meters[&meter]
            .taps
            .iter()
            .map(|id| t.taps[id].accrued_e6)
            .collect();
        assert_eq!(accrued.iter().sum::<u64>(), 210 * E6);
        assert_eq!(accrued, [157_500_000, 52_500_000]);
        // A day later the EVM pass settles each box's whole units.
        w.later(25 * HOUR).await;
        w.market.taps_pass().await;
        w.later(60).await;
        w.market.taps_pass().await;
        assert!(
            w.chain.refused.lock().unwrap().is_empty(),
            "{:?}",
            w.chain.refused.lock().unwrap()
        );
        assert_eq!(w.chain.balance(WALLET), 5_000_000 - 209);
        // 4 % to the treasury, rounded down per tap.
        assert_eq!(w.chain.paid(key(&seller_address(1))), 157 - 6);
        assert_eq!(w.chain.paid(key(&seller_address(0))), 52 - 2);
        assert_eq!(w.chain.paid(TREASURY), 8);
        let t = w.taps().await;
        let paid: u64 = t.taps.values().map(|t| t.paid).sum();
        assert_eq!(paid, 209, "the half units stay owed, not lost");
        let (seller_paid, _) = meter::earned(&t, "seller-1");
        assert_eq!(seller_paid, 157);
    }

    #[tokio::test]
    async fn stopping_closes_the_taps_and_settles_them_one_last_time() {
        let w = world().await;
        let meter = w.open(1_000_000, 5_000_000).await;
        w.ask(&meter, "hello", 50).await.unwrap();
        w.later(HOUR).await;
        assert_eq!(
            w.market.tap_stop(BUYER, &meter, None).await.unwrap(),
            MeterState::Ending
        );
        assert!(matches!(
            w.ask(&meter, "more", 5).await,
            Err(MarketError::Conflict(_))
        ));
        for _ in 0..3 {
            w.market.taps_pass().await;
            w.later(30).await;
        }
        let t = w.taps().await;
        assert_eq!(t.meters[&meter].state, MeterState::Ended);
        assert!(w
            .chain
            .taps
            .lock()
            .unwrap()
            .values()
            .all(|t| t.closed_at.is_some()));
        assert_eq!(w.chain.balance(WALLET), 5_000_000 - 108);
        assert!(w.chain.refused.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_answer_running_when_the_buyer_stops_is_still_paid() {
        let w = Arc::new(world().await);
        let meter = w.open(1_000_000, 5_000_000).await;
        w.ask(&meter, "hello", 50).await.unwrap();
        w.later(HOUR).await;
        w.heads.set(|s| s.answer_delay = Duration::from_millis(600));
        let (answer, ()) = tokio::join!(w.ask(&meter, "a long story", 50), async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            w.market.tap_stop(BUYER, &meter, None).await.unwrap();
            // The settle pass runs under the answer and must leave its taps.
            for _ in 0..3 {
                w.market.taps_pass().await;
                w.later(30).await;
            }
        });
        answer.unwrap();
        for _ in 0..4 {
            w.market.taps_pass().await;
            w.later(30).await;
        }
        let t = w.taps().await;
        assert_eq!(t.meters[&meter].requests, 2);
        assert_eq!(t.meters[&meter].state, MeterState::Ended);
        for id in &t.meters[&meter].taps {
            let tap = &t.taps[id];
            assert!(
                tap.accrued_e6.saturating_sub(tap.paid.saturating_mul(E6)) < E6,
                "tap {id} left {} unpaid",
                tap.accrued_e6 - tap.paid * E6
            );
        }
        let paid: u64 = t.taps.values().map(|t| t.paid).sum();
        assert!(paid > 108, "{paid}");
        assert_eq!(w.chain.balance(WALLET), 5_000_000 - paid);
        assert!(w.chain.refused.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_wallet_must_open_every_tap_as_quoted() {
        let w = world().await;
        w.chain.top_up(WALLET, 1_000_000);
        // A lower rate, another payee, a missing tap, two wallets.
        type Tweak<'a> = &'a dyn Fn(usize, &mut ChainTap);
        let cases: [Tweak; 3] = [
            &|_, t| t.rate -= 1,
            &|n, t| {
                if n == 0 {
                    t.seller = key(THIEF);
                }
            },
            &|n, t| {
                if n == 1 {
                    t.buyer = key(THIEF);
                }
            },
        ];
        for tweak in cases {
            let q = w.quote(1_000_000).await.unwrap();
            w.chain.open(WALLET, &q, tweak);
            assert!(w.market.tap_confirm(BUYER, &q.meter, WALLET).await.is_err());
            assert_eq!(w.taps().await.meters[&q.meter].state, MeterState::Quoted);
            w.market.tap_stop(BUYER, &q.meter, None).await.unwrap();
        }
        let q = w.quote(1_000_000).await.unwrap();
        w.chain.open(WALLET, &q, |_, _| {});
        w.chain
            .taps
            .lock()
            .unwrap()
            .remove(&(key(WALLET), q.taps[0].id.clone()));
        assert!(w.market.tap_confirm(BUYER, &q.meter, WALLET).await.is_err());
        // Nobody else can confirm, stop or use the buyer's meter.
        assert!(matches!(
            w.market.tap_confirm("other", &q.meter, WALLET).await,
            Err(MarketError::NotFound)
        ));
        assert!(matches!(
            w.market.tap_stop("other", &q.meter, None).await,
            Err(MarketError::NotFound)
        ));
    }

    #[tokio::test]
    async fn a_quote_the_wallet_opened_in_part_is_closed_and_its_wallet_kept() {
        let w = world().await;
        w.chain.top_up(WALLET, 1_000_000);
        let q = w.quote(1_000_000).await.unwrap();
        assert!(q.taps.len() > 1, "{q:?}");
        // The top-up and the first batch landed; the wallet refused the rest.
        w.chain.open(WALLET, &q, |_, _| {});
        for t in &q.taps[1..] {
            w.chain
                .taps
                .lock()
                .unwrap()
                .remove(&(key(WALLET), t.id.clone()));
        }
        assert!(matches!(
            w.market
                .tap_stop(BUYER, &q.meter, Some("not-a-wallet"))
                .await,
            Err(MarketError::Invalid(_))
        ));
        assert_eq!(
            w.market
                .tap_stop(BUYER, &q.meter, Some(WALLET))
                .await
                .unwrap(),
            MeterState::Ending
        );
        let t = w.taps().await;
        assert_eq!(t.meters[&q.meter].wallet.as_deref(), Some(WALLET));
        assert!(t.taps[&q.taps[0].id].opened);
        assert!(q.taps[1..].iter().all(|x| t.taps[&x.id].done));
        for _ in 0..3 {
            w.market.taps_pass().await;
            w.later(30).await;
        }
        let first = w.chain.taps.lock().unwrap()[&(key(WALLET), q.taps[0].id.clone())];
        assert!(first.closed_at.is_some(), "{first:?}");
        assert_eq!(w.taps().await.meters[&q.meter].state, MeterState::Ended);
        // The balance it topped up stays in the account view.
        let view = w
            .market
            .llm_overview(&w.inference, &w.lenders, BUYER, None)
            .await
            .unwrap();
        assert_eq!(view["wallets"][0]["wallet"], WALLET, "{view}");
        assert_eq!(w.chain.balance(WALLET), 1_000_000);
        assert!(w.chain.refused.lock().unwrap().is_empty());

        // A quote the wallet opened none of lapses, and keeps the wallet too.
        let q = w.quote(1_000_000).await.unwrap();
        assert_eq!(
            w.market
                .tap_stop(BUYER, &q.meter, Some(WALLET))
                .await
                .unwrap(),
            MeterState::Lapsed
        );
        assert_eq!(
            w.taps().await.meters[&q.meter].wallet.as_deref(),
            Some(WALLET)
        );
    }

    #[tokio::test]
    async fn a_wallet_that_copies_the_quoted_taps_first_changes_nothing() {
        let w = world().await;
        let q = w.quote(1_000_000).await.unwrap();
        // Someone who saw the quote's ids opens them first, with no balance.
        w.chain.open(THIEF, &q, |_, _| {});
        // Only the wallet that opened them can confirm them.
        assert!(w.market.tap_confirm(BUYER, &q.meter, WALLET).await.is_err());
        assert!(matches!(
            w.market.tap_confirm(BUYER, &q.meter, "not-a-wallet").await,
            Err(MarketError::Invalid(_))
        ));
        w.chain.top_up(WALLET, 5_000_000);
        w.chain.open(WALLET, &q, |_, _| {});
        assert_eq!(
            w.market
                .tap_confirm(BUYER, &q.meter, &WALLET.to_uppercase().replace("0X", "0x"))
                .await
                .unwrap(),
            MeterState::Open
        );
        assert_eq!(
            w.taps().await.meters[&q.meter].wallet.as_deref(),
            Some(WALLET)
        );
        w.groups_pass().await;
        w.ask(&q.meter, "a story", 100).await.unwrap();
        w.later(25 * HOUR).await;
        w.market.taps_pass().await;
        assert!(w.chain.refused.lock().unwrap().is_empty());
        assert_eq!(w.chain.balance(WALLET), 5_000_000 - 209);
        assert_eq!(w.chain.balance(THIEF), 0);
    }

    #[tokio::test]
    async fn nothing_is_served_past_the_balance_or_the_hourly_limit() {
        let w = world().await;
        let meter = w.open(1_000_000, 300).await;
        // 10 in and 200 out could cost 410 > 300.
        let err = w.ask(&meter, "a story", 200).await.unwrap_err();
        assert!(matches!(err, MarketError::PaymentRequired(_)), "{err:?}");
        assert_eq!(w.taps().await.meters[&meter].requests, 0);
        w.ask(&meter, "a story", 100).await.unwrap();
        // 210 used: 90 left, and 50 more answer tokens would need 110.
        assert!(matches!(
            w.ask(&meter, "a story", 50).await,
            Err(MarketError::PaymentRequired(_))
        ));
        // An ask to withdraw counts against what may be spent.
        w.chain.top_up(WALLET, 10_000);
        w.chain
            .asks
            .lock()
            .unwrap()
            .insert(key(WALLET), (10_000, now_secs() + meter::NOTICE_SECS));
        w.later(1).await;
        assert!(matches!(
            w.ask(&meter, "a story", 50).await,
            Err(MarketError::PaymentRequired(_))
        ));

        // The hourly limit: a cent an hour (10 000 base units), less the
        // two minutes of clock margin. The escrow opens a tap an hour into
        // its accrual, so the first hour's worth is there at once.
        let w = world().await;
        let meter = w.open(10_000, 5_000_000).await;
        // 5 tokens in and 3000 out: 6005.
        w.ask(&meter, "hi", 3000).await.unwrap();
        let err = w.ask(&meter, "hi", 3000).await.unwrap_err();
        assert!(matches!(err, MarketError::PaymentRequired(_)), "{err:?}");
        w.later(HOUR).await;
        w.ask(&meter, "hi", 3000).await.unwrap();
        // Nothing billed ever passes what the escrow lets the edge settle.
        w.later(25 * HOUR).await;
        w.market.taps_pass().await;
        w.later(60).await;
        w.market.taps_pass().await;
        assert!(
            w.chain.refused.lock().unwrap().is_empty(),
            "{:?}",
            w.chain.refused.lock().unwrap()
        );
        let paid: u64 = w.taps().await.taps.values().map(|t| t.paid).sum();
        assert_eq!(w.chain.balance(WALLET), 5_000_000 - paid);
        // Whole units per tap; the fractions stay owed.
        assert!((2 * 6005 - 1..=2 * 6005).contains(&paid), "{paid}");
    }

    #[tokio::test]
    async fn a_box_cannot_bill_for_more_tokens_than_were_asked_for() {
        let w = world().await;
        let meter = w.open(1_000_000, 5_000_000).await;
        w.heads.set(|s| {
            s.tokens_out = Some(10_000);
            s.extra_in = 1_000;
        });
        let a = w.ask(&meter, "a story", 20).await.unwrap();
        assert_eq!(a["tokensOut"], 20);
        assert_eq!(a["tokensIn"], 10);
        assert_eq!(w.taps().await.meters[&meter].billed_e6, (10 + 40) * E6);
    }

    #[tokio::test]
    async fn a_failed_answer_is_not_billed() {
        let w = world().await;
        let meter = w.open(1_000_000, 5_000_000).await;
        w.heads.set(|s| s.fail = true);
        assert!(matches!(
            w.ask(&meter, "hi", 20).await,
            Err(MarketError::Unavailable(_))
        ));
        let m = &w.taps().await.meters[&meter];
        assert_eq!((m.requests, m.billed_e6), (0, 0));
    }

    #[tokio::test]
    async fn one_request_at_a_time_per_meter() {
        let w = Arc::new(world().await);
        let meter = w.open(1_000_000, 5_000_000).await;
        w.heads.set(|s| s.delay = Duration::from_millis(300));
        let (a, b) = tokio::join!(w.ask(&meter, "one", 5), async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            w.ask(&meter, "two", 5).await
        });
        assert!(a.is_ok());
        assert!(matches!(b, Err(MarketError::Busy(_))));
        assert!(w.ask(&meter, "three", 5).await.is_ok());
    }

    #[tokio::test]
    async fn a_box_that_leaves_ends_its_buyers_meters_after_the_grace() {
        let mut w = world().await;
        let meter = w.open(1_000_000, 5_000_000).await;
        w.ask(&meter, "hello", 10).await.unwrap();
        let gone = w.lenders.remove(0);
        w.heads.set(|s| s.healthy = false);
        w.groups_pass().await;
        // Inside the grace (a nightly reboot) the meter stays open, but
        // nothing is served while the head is down.
        assert_eq!(w.taps().await.meters[&meter].state, MeterState::Open);
        assert!(matches!(
            w.ask(&meter, "hi", 5).await,
            Err(MarketError::Unavailable(_))
        ));
        w.lenders.insert(0, gone.clone());
        w.heads.set(|s| s.healthy = true);
        w.groups_pass().await; // started again
        w.groups_pass().await; // answers
        w.ask(&meter, "hi", 5).await.unwrap();
        // Gone for longer than the grace: the group breaks.
        w.lenders.remove(0);
        w.groups_pass().await;
        w.later(crate::inference::GRACE_SECS).await;
        w.groups_pass().await;
        let t = w.taps().await;
        assert_eq!(t.meters[&meter].state, MeterState::Ending);
        assert_eq!(t.meters[&meter].reason, Some(EndReason::GroupLeft));
        assert!(!w.heads.stopped.lock().unwrap().is_empty());
        // What it served is still paid when the taps close.
        for _ in 0..3 {
            w.market.taps_pass().await;
            w.later(30).await;
        }
        assert_eq!(w.taps().await.meters[&meter].state, MeterState::Ended);
        assert!(w.chain.paid(key(&seller_address(0))) > 0);
        // A new quote picks other boxes: the one that left is not lending.
        w.lenders.push(("seller-2".into(), 20002, 8192));
        let q = w.quote(1_000_000).await.unwrap();
        assert!(q.taps.iter().all(|t| t.seller != seller_address(0)));
    }

    #[tokio::test]
    async fn a_wallet_that_closes_a_tap_itself_stops_being_served() {
        let w = world().await;
        let meter = w.open(1_000_000, 5_000_000).await;
        w.ask(&meter, "hello", 10).await.unwrap();
        let first = w.taps().await.meters[&meter].taps[0].clone();
        w.later(HOUR).await;
        w.chain.buyer_close(&first);
        w.market.taps_pass().await;
        let t = w.taps().await;
        assert_eq!(t.meters[&meter].state, MeterState::Ending);
        assert_eq!(t.meters[&meter].reason, Some(EndReason::TapClosed));
        assert!(w.ask(&meter, "more", 5).await.is_err());
        for _ in 0..3 {
            w.later(30).await;
            w.market.taps_pass().await;
        }
        assert_eq!(w.taps().await.meters[&meter].state, MeterState::Ended);
        assert!(w.chain.refused.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn not_enough_memory_means_no_quote() {
        let mut w = world().await;
        w.lenders.truncate(1);
        assert!(matches!(
            w.quote(1_000_000).await,
            Err(MarketError::Unavailable(_))
        ));
        // A box with no payout address for the network is not picked.
        let w = world().await;
        w.market
            .state
            .lock()
            .await
            .state
            .sellers
            .get_mut("seller-1")
            .unwrap()
            .payout = None;
        assert!(matches!(
            w.quote(1_000_000).await,
            Err(MarketError::Unavailable(_))
        ));
    }

    #[tokio::test]
    async fn a_buyer_runs_a_model_once_and_cannot_pile_up_quotes() {
        let w = world().await;
        let meter = w.open(1_000_000, 5_000_000).await;
        assert!(matches!(
            w.quote(1_000_000).await,
            Err(MarketError::Conflict(_))
        ));
        w.market.tap_stop(BUYER, &meter, None).await.unwrap();
        for _ in 0..meter::MAX_QUOTED_PER_BUYER {
            w.quote(1_000_000).await.unwrap();
        }
        assert!(w.quote(1_000_000).await.is_err());
    }
}
