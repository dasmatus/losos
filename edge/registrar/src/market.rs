//! The optional storage / compute market, settled through Stripe Connect.
//!
//! Boxes that share disk or CPU with the mesh (see
//! `handbook/docs/in-depth/mesh.md`) can *sell* it, and any other box can *buy*
//! it. The edge is the marketplace: it keeps the listings and the orders, and
//! it is the Stripe **platform** account. Money never touches a losos box. A
//! buyer pays through a Stripe Checkout Session created as a *destination
//! charge*: the full amount is charged on the platform,
//! `transfer_data[destination]` forwards it to the seller's connected Express
//! account, and `application_fee_amount` — the platform's cut, 4% (400 basis
//! points) by default — stays behind to cover the edge's running costs.
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
//! adds the persisted store. Every Connect call (accounts, onboarding,
//! destination charges, webhook checks) goes through [`crate::connect::Connect`],
//! which on an edge is [`crate::stripe_gate::GateClient`], the client of the
//! separate process that holds the key. `tests/connect.rs` runs the market on
//! the in-memory platform, and the integration tests drive the real routes
//! against a Stripe stub.
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
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ring::hmac;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::action::Action;
use crate::connect::{ChargeSpec, Connect, DestinationCharge, Split};
use crate::fsutil::atomic_write;
use crate::hardware::{total, Catalogue, Line};
use crate::stripe_gate::{GateClient, HardwareRequest};
use crate::stripe_mode::StripeMode;
use crate::sync::{Delivery, Event, SyncLog};
use crate::vms::{Upload, UploadView, VmOrder, VmProvision, VM_FEE_BPS};

mod taps;
pub use taps::{
    ChainFuture, EscrowChain, LendingView, MeterView, QuotedTap, TapChain, TapQuote, WalletReport,
    WARN_RUNWAY_SECS,
};

/// The cut kept by the platform, in basis points of the gross amount: 4%.
pub const DEFAULT_FEE_BPS: u32 = 400;
/// The most an operator may configure. A typo (`--market-fee-bps 4000`) would
/// otherwise silently take 40% of every sale.
pub const MAX_FEE_BPS: u32 = 2_000;

/// What a seller may owe in platform fees on coin sales before buyers can no
/// longer pay them in coins, in the market's minor unit: US$10. Coin orders
/// are quoted in US dollars, so the limit is too.
pub const DEFAULT_COIN_FEE_LIMIT: u64 = 1_000;
/// The most an operator may set it to, US$1,000.
pub const MAX_COIN_FEE_LIMIT: u64 = 100_000;
/// How long a seller may owe coin fees at all before buyers can no longer
/// pay them in coins, whatever the amount.
pub const COIN_FEE_GRACE_SECS: u64 = 30 * 24 * 3600;
/// What `pay_with_coin` answers while a seller's coin fees are overdue.
/// Why a listing cannot be closed: a buyer paid for a slot on it that is
/// still running (`crate::slots::locked_until`).
const SLOT_RUNNING: &str =
    "someone has paid for this listing; it can be closed once their slot ends";
/// Why a payout was refused: every held payment is for a slot still running.
const NOTHING_TO_PAY_OUT: &str =
    "nothing is ready to pay out yet; held payments are paid once their slots end";
const COINS_PAUSED: &str = "the seller owes platform fees on earlier coin sales, so this order cannot be paid in coins until they are settled; pay in USDC where the seller takes it";

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
    /// Virtual machines on the seller's node, priced per replica-month and
    /// split half and half with the platform (`crate::vms`).
    Vm,
}

impl Kind {
    /// The unit a listing of this kind is priced and sized in.
    #[must_use]
    pub const fn unit(self) -> &'static str {
        crate::plans::plan(self).unit
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
    vms: BTreeSet<String>,
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
            vms: windows
                .iter()
                .filter(|(_, w)| w.host_vms)
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
        self.vms.retain(|seller| permitted(seller));
        self
    }

    #[must_use]
    pub fn allows(&self, seller: &str, kind: Kind) -> bool {
        match kind {
            Kind::Storage => self.enrolled.contains(seller),
            Kind::Compute => self.compute.contains(seller),
            Kind::Vm => self.vms.contains(seller),
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
    /// Where USDC is paid on an EVM chain, `0x` + 40 hex. Set when the
    /// wallet that controls it signs [`crate::stablecoin::payout_message`].
    /// `None` on a Stripe-only seller.
    #[serde(default)]
    pub payout: Option<String>,
    /// Where USDC is paid on Solana, a base58 owner. Set the same way, with
    /// an ed25519 signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payout_solana: Option<String>,
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

/// How an order is paid. Absent on a ledger written before USDC, which is
/// read as Stripe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Rail {
    #[default]
    Stripe,
    Usdc,
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
    /// What a machine order runs and how far the edge has got with it.
    /// `None` on every other kind.
    #[serde(default)]
    pub vm: Option<VmOrder>,
    /// Stripe unless the edge settles this order in USDC.
    #[serde(default)]
    pub rail: Rail,
    /// The transaction that paid a USDC order. Unique across the ledger.
    #[serde(default)]
    pub tx_hash: Option<String>,
    /// The seller's payout address when the order was made, which is the
    /// address the buyer's invoice names. A later change of payout address
    /// must not turn a payment made to the invoice into a refused one.
    /// `None` on Stripe orders and on USDC orders made before it was kept.
    #[serde(default)]
    pub pay_to: Option<String>,
    /// The seller's Solana payout owner when the order was made, for the
    /// same reason. `None` when the seller had none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pay_to_solana: Option<String>,
    /// The network `tx_hash` is on. `None` on an order paid before the edge
    /// settled on more than one, which was its only network.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paid_network: Option<String>,
    /// Set once the buyer chose to pay in a coin other than USDC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coin: Option<CoinPayment>,
    /// Where an EVM payment held in `LososEscrow` is going. `None` on Stripe
    /// and Solana orders, and on orders paid before the escrow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escrow: Option<Escrow>,
    /// What a USDC order's invoice takes for the treasury on top of `fee`,
    /// toward platform fees the seller owes on earlier coin sales. The buyer
    /// pays `amount` either way; the seller receives this much less. Taken
    /// off what the seller owes only once the payment reaches the treasury
    /// ([`collect_offset`]), never at quote time.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fee_offset: u64,
    /// Whether `fee_offset` has been taken off what the seller owes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub offset_collected: bool,
    /// Where a card payment held on the platform is going. `None` on every
    /// other rail, and on card orders paid before the platform held them
    /// (a destination charge, already with the seller).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<Card>,
    /// The seller's [`crate::slots::Presence::offline`] when the order was
    /// paid, so the offline time that counts against it is only its own.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub offline_base: u64,
}

/// A card payment the platform holds until the slot it bought is over
/// (`crate::slots`). Stripe charged the platform; the seller's share is
/// transferred to their connected account only at the end, and the gate
/// refunds from the platform's balance before that. One step at a time, the
/// same steps as an [`Escrow`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    /// `pi_...`, from the paid Checkout Session.
    pub payment_intent: String,
    pub step: EscrowStep,
    /// When the last settle of this step failed, so the next waits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tried_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<RefundReason>,
    /// On [`EscrowStep::Split`], what goes back to the buyer, minor units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refund: Option<u64>,
}

/// What the edge does with one deposit in `LososEscrow` or `losos-escrow`,
/// and how far it got.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Escrow {
    pub network: String,
    /// The escrow's deposit id, from the `Held` log. Zero on Solana, where
    /// a deposit is an account: [`Escrow::account`].
    pub deposit: u64,
    /// Solana: the deposit account. It exists while the payment is held.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// Solana: when the payer may take the deposit back alone, from the
    /// deposit itself. A deposit gone before then was finished by the edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<u64>,
    pub step: EscrowStep,
    /// Why the deposit goes back to the payer. `None` on a release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<RefundReason>,
    /// The edge's last release or refund transaction, until it is mined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent_at: Option<u64>,
    /// On [`EscrowStep::Split`], what goes back to the payer, in the
    /// market's minor unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refund: Option<u64>,
}

impl Escrow {
    /// Whether the edge still has a transaction to send or follow.
    #[must_use]
    pub fn pending(&self) -> bool {
        self.step.pending()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EscrowStep {
    /// Kept while the slot runs, so the buyer can still be refunded. Nothing
    /// to send yet (`crate::slots`).
    Held,
    /// To the seller and the treasury, once the edge's call is mined.
    Release,
    Released,
    /// Back to the payer, once the edge's call is mined.
    Refund,
    Refunded,
    /// [`Escrow::refund`] back to the payer and the rest to the seller and
    /// the treasury, once the edge's call is mined.
    Split,
    Settled,
}

impl EscrowStep {
    /// Whether the edge has a call to make or follow.
    #[must_use]
    pub fn pending(self) -> bool {
        matches!(self, Self::Release | Self::Refund | Self::Split)
    }
}

/// `GET /escrow/releaser` from the terminal (`crate::operator`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaserReport {
    /// `None` until a key is sealed.
    pub address: Option<String>,
    /// The same key's Solana address, when a Solana network has the escrow
    /// program.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solana_address: Option<String>,
    /// One row per network with an escrow, in the order they are
    /// configured.
    pub networks: Vec<ReleaserNetwork>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaserNetwork {
    pub network: &'static str,
    /// The `LososEscrow` this network's payments go to.
    pub escrow: String,
    /// The gas the key holds there, in wei as a decimal string. EVM only.
    pub balance_wei: Option<String>,
    /// The SOL the key holds there, in lamports as a decimal string. Solana
    /// only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance_lamports: Option<String>,
    /// Whether that escrow's `releaser()` is this key. `false` until the
    /// treasury calls `setReleaser` with the address. On Solana `None` until
    /// the treasury has made its config with `initialize`.
    pub escrow_agrees: Option<bool>,
    /// Why the other two are missing, when the key exists.
    pub error: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefundReason {
    /// The seller's payout address is a contract, or one money must not go
    /// to, on the network the buyer paid on.
    SellerCannotReceive,
    /// Another transaction paid the order first.
    AlreadyPaid,
    /// The hold lapsed before the payment and its units were sold.
    Lapsed,
    /// The edge did not release before the deadline and the buyer took the
    /// deposit back.
    BuyerReclaimed,
    /// What was bought could not be set up before the deadline: a storage
    /// claim that never bound, or machines never created. The edge sends the
    /// deposit back rather than pay the seller for nothing.
    NotDelivered,
    /// The seller's box was offline for too long while the slot ran
    /// (`crate::slots`). The buyer gets back the share not delivered.
    SellerOffline,
}

impl RefundReason {
    /// What the buyer's box is told when it reports the payment.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            Self::SellerCannotReceive => {
                "the seller's payout address cannot receive USDC on that network; the payment is being refunded"
            }
            Self::AlreadyPaid => "the order was already paid; this payment is being refunded",
            Self::Lapsed => {
                "the order lapsed before it was paid and its units are sold; the payment is being refunded"
            }
            Self::BuyerReclaimed => "the buyer took the payment back from the escrow",
            Self::NotDelivered => {
                "what was bought could not be set up in time; the payment is being refunded"
            }
            Self::SellerOffline => {
                "the seller's box was offline too long; the part not delivered is being refunded"
            }
        }
    }
}

/// A deposit for an order that cannot take it: a second payment, or one
/// that came after the units were sold. Keyed by the paying transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrayPayment {
    pub order_id: String,
    pub buyer: String,
    pub received_at: u64,
    pub escrow: Escrow,
}

/// Everything the market persists for one Stripe mode: `market.json` for
/// live, `market-test.json` for test (see [`ledger_path`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketState {
    /// The mode this ledger was written in. Absent on a file written before
    /// the edge kept the two apart; such a file is taken as the ledger of
    /// the mode it is opened for, and stamped on its next write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<StripeMode>,
    #[serde(default)]
    pub sellers: BTreeMap<String, Seller>,
    #[serde(default)]
    pub listings: BTreeMap<String, Listing>,
    #[serde(default)]
    pub orders: BTreeMap<String, Order>,
    /// Buyers' own machine images, kept on the edge (`crate::vms`).
    #[serde(default)]
    pub uploads: BTreeMap<String, Upload>,
    /// Which webhook events were applied, and how new each account's state
    /// is (`crate::sync`).
    #[serde(default, skip_serializing_if = "SyncLog::is_empty")]
    pub sync: SyncLog,
    /// Escrowed payments the edge is sending back, by paying transaction.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub strays: BTreeMap<String, StrayPayment>,
    /// What each seller takes in coins other than USDC, by appliance id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub coin_sellers: BTreeMap<String, CoinSeller>,
    /// USDC transfers sellers sent the treasury to settle coin fees, by
    /// transaction, so each one is counted once.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fee_payments: BTreeMap<String, FeePayment>,
    /// How long each seller with a running slot has been offline
    /// (`crate::slots`), by appliance id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub presence: BTreeMap<String, crate::slots::Presence>,
    /// When the edge last looked, so time it was not looking (down, or too
    /// busy to hear heartbeats) is never counted against a seller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_tick: Option<u64>,
    /// Pay as you go: meters and their taps (`crate::meter`).
    #[serde(default, skip_serializing_if = "crate::meter::Taps::is_empty")]
    pub taps: crate::meter::Taps,
    /// The backup door of each bought volume a box keeps its backups on
    /// (`server/storage.rs`), by order id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub doors: BTreeMap<String, Door>,
}

/// One bought volume's backup door.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Door {
    /// The SHA-256, in hex, of a secret the buyer's box derives from its
    /// recovery code and the order id. The secret itself is never sent here.
    /// Only the buyer sets it; whoever knows the recovery code (the buyer's
    /// box, or a new box the owner restores onto) can then open the door.
    pub key: String,
    /// The password the edge alone uses to reach the order's server, made
    /// here once and kept for the life of the order. Every mesh node can
    /// reach a Service's cluster address; without this, any of them could
    /// read or delete the backups there.
    pub upstream: String,
}

/// What is behind an opened door: the volume, and how the edge reaches its
/// server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenDoor {
    pub volume: StorageVolume,
    pub upstream: String,
}

/// A seller's watch-only keys for coin orders (`crate::coins`), and the
/// platform fees those sales owe. The coins go straight to the seller's own
/// wallet, so the fee cannot be split off on the way. The edge collects it
/// from the seller's next USDC sales ([`fee_offset_for`]) or from a transfer
/// the seller sends the treasury ([`Market::pay_coin_fees`]), and stops coin
/// payments to a seller who owes too much or for too long
/// ([`CoinSeller::coins_paused`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoinSeller {
    /// By coin key (`btc`).
    #[serde(default)]
    pub keys: BTreeMap<String, CoinKey>,
    /// Platform fees of paid coin orders not yet collected, in the
    /// currency's minor unit.
    #[serde(default)]
    pub fees_owed: u64,
    /// When `fees_owed` last rose from nothing. `None` while nothing is owed.
    /// A ledger written before it was kept gets the time it is first read
    /// ([`stamp_debts`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owed_since: Option<u64>,
    /// Collected from USDC sales beyond what was owed by the time the
    /// payment landed (another payment settled the same debt meanwhile).
    /// Pays for the next coin fees before anything is owed again.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fee_credit: u64,
}

impl CoinSeller {
    /// Add the platform fee of a paid coin order. Credit pays for it first.
    pub fn charge(&mut self, fee: u64, now: u64) {
        let from_credit = fee.min(self.fee_credit);
        self.fee_credit -= from_credit;
        let rest = fee - from_credit;
        if rest == 0 {
            return;
        }
        if self.fees_owed == 0 {
            self.owed_since = Some(now);
        }
        self.fees_owed = self.fees_owed.saturating_add(rest);
    }

    /// Take `paid` off what is owed and return how much of it counted. What
    /// is left over becomes credit when `keep_surplus`, and is not counted
    /// otherwise.
    pub fn settle(&mut self, paid: u64, keep_surplus: bool) -> u64 {
        let credited = paid.min(self.fees_owed);
        self.fees_owed -= credited;
        if self.fees_owed == 0 {
            self.owed_since = None;
        }
        if keep_surplus {
            self.fee_credit = self.fee_credit.saturating_add(paid - credited);
        }
        credited
    }

    /// Whether buyers may not pay this seller in coins now: more than
    /// `limit` is owed, or anything has been owed for
    /// [`COIN_FEE_GRACE_SECS`].
    #[must_use]
    pub fn coins_paused(&self, limit: u64, now: u64) -> bool {
        self.fees_owed > limit
            || (self.fees_owed > 0
                && self
                    .owed_since
                    .is_some_and(|since| now >= since.saturating_add(COIN_FEE_GRACE_SECS)))
    }
}

/// A USDC transfer a seller sent the treasury to settle coin fees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeePayment {
    pub seller: String,
    pub network: String,
    /// What reached the treasury, in the currency's minor unit.
    pub paid: u64,
    /// What of it counted: no more than was owed.
    pub credited: u64,
    pub received_at: u64,
}

/// A seller's key for one coin, as the edge's node checked it, and the next
/// address index to hand out. For Bitcoin the key is a descriptor; for
/// Monero it is the primary address, with the private view key beside it;
/// for Zcash it is the wallet's first address, shown back to the seller,
/// with the incoming viewing key beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoinKey {
    pub descriptor: String,
    pub next: u32,
    /// Monero: the private view key the edge's view-only wallet is made from.
    /// Zcash: the Orchard incoming viewing key (`uivk…`) the edge decrypts
    /// with. Either sees what arrives and cannot spend. Never shown back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_key: Option<String>,
    /// Monero: the chain height the wallet scans from, so a wallet made
    /// again later still finds the payments of orders in flight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_height: Option<u64>,
}

impl CoinKey {
    fn watch(&self) -> Option<crate::monero::WatchKey<'_>> {
        Some(crate::monero::WatchKey {
            address: &self.descriptor,
            view_key: self.view_key.as_deref()?,
            from_height: self.from_height.unwrap_or(0),
        })
    }

    /// Zcash: the viewing key the edge decrypts this seller's orders with.
    fn zec_key(&self) -> Option<crate::zcash::ViewingKey> {
        crate::zcash::parse_key(self.view_key.as_deref()?).ok()
    }
}

/// An order being paid in a coin: the quote, the address derived for it from
/// the seller's key, and what the edge's node last saw arrive there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoinPayment {
    pub quote: crate::coins::Quote,
    pub address: String,
    pub index: u32,
    /// When the address was given to this order. Only payments the node saw
    /// from then on count: anything older reached the address before it was
    /// this order's.
    #[serde(default)]
    pub issued: u64,
    /// Monero: the chain height when the address was given to this order.
    /// The wallet reports a payment's block, not when it first saw it, so
    /// only payments in blocks from here on (or still in the pool) count.
    /// Zcash: the first block after the best one when the address was
    /// given, with the same meaning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_height: Option<u64>,
    /// Zcash: when the edge first saw each paying transaction, mempool
    /// included. The servers say only which block a payment is in, so a
    /// payment sent in time keeps its time once it is mined.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub first_seen: BTreeMap<String, u64>,
    #[serde(default)]
    pub settlement: Option<crate::coins::Settlement>,
    /// Transactions that paid to the address.
    #[serde(default)]
    pub txs: Vec<String>,
}

/// Market failures, mapped onto HTTP statuses by
/// [`crate::error::ApiError`]. Client-fault variants carry text that is safe
/// to return; `Stripe` and `Store` carry server detail that only reaches the
/// log.
#[derive(Debug, Clone, thiserror::Error)]
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
    /// The buyer's balance or allowance cannot cover the request (402).
    #[error("{0}")]
    PaymentRequired(&'static str),
    /// The same meter already has a request running (429).
    #[error("{0}")]
    Busy(&'static str),
    /// What the request needs is not running right now, such as a model still
    /// loading on its boxes (503).
    #[error("{0}")]
    Unavailable(&'static str),
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

/// The platform's cut for an order of `kind`, as its plan
/// ([`crate::plans`]) takes it: the operator's rate, except on a machine,
/// which is always split half and half ([`VM_FEE_BPS`]).
#[must_use]
pub const fn fee_bps_for(kind: Kind, operator_bps: u32) -> u32 {
    crate::plans::plan(kind).fee_bps(operator_bps)
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

/// A USDC checkout is the buyer's own transaction. There is no webhook that
/// can arrive days later, so an unpaid order only holds its units for a
/// quarter of an hour.
const USDC_HOLD_SECS: u64 = 15 * 60;

/// How long a coin order keeps its units after its quote lapses, for the
/// payment it saw in time to reach its confirmations: twenty Monero blocks,
/// or six Bitcoin blocks on a slow hour.
const COIN_CONFIRM_SECS: u64 = 3 * 60 * 60;
/// The `deadline` a USDC order's invoice names for `LososEscrow.pay`.
pub(crate) fn usdc_deadline(order: &Order) -> u64 {
    crate::stablecoin::refund_deadline(order.created_at.saturating_add(USDC_HOLD_SECS))
}

/// How long a pending order keeps its units. Stripe's window is the session
/// lifetime plus the three-day retry horizon of a webhook that may still be
/// in flight; USDC's is [`USDC_HOLD_SECS`].
fn pending_window(order: &Order) -> u64 {
    if let Some(coin) = &order.coin {
        return coin
            .quote
            .pay_before
            .saturating_sub(order.created_at)
            .saturating_add(COIN_CONFIRM_SECS);
    }
    match order.rail {
        Rail::Usdc => USDC_HOLD_SECS,
        Rail::Stripe => CHECKOUT_TTL_SECS + PENDING_HOLD_SECS,
    }
}

/// Whether a pending order still holds its units at `now`.
fn pending_live(order: &Order, now: u64) -> bool {
    order.status == OrderStatus::Pending
        && now < order.created_at.saturating_add(pending_window(order))
}

/// Whether an order's fee offset may still reach the treasury: a USDC order
/// still pending, or paid with its deposit held or its release or split
/// still to come.
fn offset_in_flight(order: &Order, now: u64) -> bool {
    order.fee_offset > 0
        && !order.offset_collected
        && order.rail == Rail::Usdc
        && (pending_live(order, now)
            || (order.status == OrderStatus::Paid
                && order.escrow.as_ref().is_some_and(|e| {
                    matches!(
                        e.step,
                        EscrowStep::Held | EscrowStep::Release | EscrowStep::Split
                    )
                })))
}

/// What a new USDC order of `amount` for `seller`, whose platform fee is
/// `fee`, adds to its fee toward the seller's coin fees owed: what is owed
/// and not already riding on another order in flight, and never more than
/// takes the whole fee past half the gross, which is as far as
/// `LososEscrow` lets a fee go (`MAX_FEE_BPS`). The Solana split keeps the
/// same ceiling, so a seller's share is the same on every network.
#[must_use]
pub fn fee_offset_for(state: &MarketState, seller: &str, amount: u64, fee: u64, now: u64) -> u64 {
    let owed = state.coin_sellers.get(seller).map_or(0, |s| s.fees_owed);
    if owed == 0 {
        return 0;
    }
    let riding: u64 = state
        .orders
        .values()
        .filter(|o| o.seller == seller && offset_in_flight(o, now))
        .map(|o| o.fee_offset)
        .sum();
    owed.saturating_sub(riding)
        .min((amount / 2).saturating_sub(fee))
}

/// Take a USDC order's fee offset off what its seller owes, once the
/// payment has reached the treasury: the escrow released it, or the Solana
/// transfer that split it was confirmed. Returns what was taken. Collected
/// beyond what is owed by then becomes the seller's credit.
pub fn collect_offset(state: &mut MarketState, order_id: &str) -> u64 {
    let Some(order) = state.orders.get_mut(order_id) else {
        return 0;
    };
    if order.fee_offset == 0 || order.offset_collected {
        return 0;
    }
    order.offset_collected = true;
    let (seller, offset) = (order.seller.clone(), order.fee_offset);
    state
        .coin_sellers
        .entry(seller)
        .or_default()
        .settle(offset, true);
    offset
}

/// The share of `order_id`'s fee offset that reached the treasury when its
/// deposit was split with `refund` going back to the buyer: the offset
/// scaled to what the seller kept, rounded down as `LososEscrow.settle`
/// scales the fee.
pub fn collect_offset_share(state: &mut MarketState, order_id: &str, refund: u64) -> u64 {
    let Some(order) = state.orders.get_mut(order_id) else {
        return 0;
    };
    if order.fee_offset == 0 || order.offset_collected || order.amount == 0 {
        return 0;
    }
    order.offset_collected = true;
    let kept = order.amount.saturating_sub(refund);
    let share = u128::from(order.fee_offset) * u128::from(kept) / u128::from(order.amount);
    let share = u64::try_from(share).unwrap_or(0);
    let seller = order.seller.clone();
    if share > 0 {
        state
            .coin_sellers
            .entry(seller)
            .or_default()
            .settle(share, true);
    }
    share
}

/// Give a debt from a ledger written before the edge kept its start the
/// time it was first read, so the age rule has something to count from.
pub fn stamp_debts(state: &mut MarketState, now: u64) {
    for seller in state.coin_sellers.values_mut() {
        if seller.fees_owed > 0 && seller.owed_since.is_none() {
            seller.owed_since = Some(now);
        }
    }
}

/// The seller of `order_id`, or nothing.
fn seller_of(state: &MarketState, order_id: &str) -> String {
    state
        .orders
        .get(order_id)
        .map(|o| o.seller.clone())
        .unwrap_or_default()
}

/// One side of an account's order history: live orders first, then newest
/// first, at most [`ACCOUNT_HISTORY`] of them.
fn history<'a>(
    orders: impl Iterator<Item = &'a Order>,
    now: u64,
    vm_domain: Option<&str>,
) -> Vec<OrderView> {
    let mut orders: Vec<&Order> = orders.collect();
    orders.sort_by_key(|o| {
        let live = paid_live(o, now) || pending_live(o, now);
        (std::cmp::Reverse(live), std::cmp::Reverse(o.created_at))
    });
    orders
        .into_iter()
        .take(ACCOUNT_HISTORY)
        .map(|o| OrderView::of(o, now, vm_domain))
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
    let before = state.orders.len() + state.listings.len() + state.strays.len();
    state.orders.retain(|_, o| {
        !(o.status == OrderStatus::Expired
            && now > o.created_at + EXPIRED_RETENTION_SECS
            && !o.escrow.as_ref().is_some_and(Escrow::pending))
    });
    state
        .strays
        .retain(|_, s| s.escrow.pending() || now <= s.received_at + EXPIRED_RETENTION_SECS);
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
    (
        expired,
        before - state.orders.len() - state.listings.len() - state.strays.len(),
    )
}

/// Units of `listing` that are sold, held by a live checkout, or still
/// occupied by a volume.
#[must_use]
pub fn reserved(state: &MarketState, listing_id: &str, now: u64) -> u64 {
    state
        .orders
        .values()
        .filter(|o| o.listing_id == listing_id)
        .filter(|o| {
            paid_live(o, now) || pending_live(o, now) || holds_volume(o) || holds_replicas(o)
        })
        .map(|o| o.quantity)
        .sum()
}

/// Whether a machine order's replicas may still be running. A lapsed order
/// keeps its replicas reserved until the reconciler has halted every one of
/// them, so the host is never sold more machines than it can start.
fn holds_replicas(order: &Order) -> bool {
    order.status == OrderStatus::Paid
        && order.kind == Kind::Vm
        && order.vm.as_ref().is_some_and(|vm| vm.created && !vm.halted)
}

/// Whether what a paid order bought exists: the storage claim bound, the
/// machines created. Compute is a ledger credit and exists once paid. The
/// escrow pays the seller only after this, so a buyer never pays for a
/// volume the mesh could not give them.
pub(crate) fn delivered(order: &Order) -> bool {
    match order.kind {
        Kind::Storage => order.volume.is_some(),
        Kind::Vm => order.vm.as_ref().is_some_and(|vm| vm.created),
        Kind::Compute => true,
    }
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
pub(crate) fn paid_live(order: &Order, now: u64) -> bool {
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
    /// Virtual machine replicas it is paying for.
    pub vm_replicas: u64,
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
            Kind::Vm => out.vm_replicas = out.vm_replicas.saturating_add(o.quantity),
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
    crate::relay::dns_label(&name).then_some(name)
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

/// Where one of a buyer's bought volumes is, for the backup door
/// (`server/storage.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageVolume {
    pub namespace: String,
    pub pvc: String,
}

/// The volume of `buyer`'s storage order `order_id`, once its claim is bound.
/// `None` for any other order, another buyer's, or one not bound yet; the
/// caller answers all of them alike, so a box cannot learn which orders
/// exist. A lapsed order keeps its volume until the operator deletes the
/// claim, and so does the buyer's way to it: the data is still theirs.
#[must_use]
pub fn storage_volume(state: &MarketState, order_id: &str, buyer: &str) -> Option<StorageVolume> {
    let order = state.orders.get(order_id)?;
    if order.kind != Kind::Storage || order.buyer != buyer {
        return None;
    }
    let (namespace, pvc) = order.volume.as_deref()?.split_once('/')?;
    Some(StorageVolume {
        namespace: namespace.to_string(),
        pvc: pvc.to_string(),
    })
}

/// Whether `hash` is the shape of a door key: 64 lowercase hex characters.
#[must_use]
pub fn door_hash_ok(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// What is behind `order_id`'s backup door, when `hash` is its key. The
/// comparison takes the same time however much of the key matches.
#[must_use]
pub fn door_volume(state: &MarketState, order_id: &str, hash: &str) -> Option<OpenDoor> {
    use subtle::ConstantTimeEq;
    let door = state.doors.get(order_id)?;
    if !bool::from(door.key.as_bytes().ct_eq(hash.as_bytes())) {
        return None;
    }
    let buyer = &state.orders.get(order_id)?.buyer;
    Some(OpenDoor {
        volume: storage_volume(state, order_id, buyer)?,
        upstream: door.upstream.clone(),
    })
}

/// Paid, live machine orders whose replicas do not exist yet, resolved to
/// what the reconciler creates. `resolve` turns the order's image id into a
/// disk source, its smallest disk and memory and its firmware; an order whose
/// image no longer resolves (an upload deleted under it, a catalogue entry the
/// operator removed) or whose buyer cannot name a namespace is skipped and
/// stays paid, for the operator to look at.
#[must_use]
pub fn pending_vms(
    state: &MarketState,
    now: u64,
    resolve: impl Fn(&Order, &VmOrder) -> Option<crate::vms::Resolved>,
) -> Vec<VmProvision> {
    state
        .orders
        .values()
        .filter(|o| o.kind == Kind::Vm && paid_live(o, now))
        .filter_map(|o| {
            let vm = o.vm.as_ref().filter(|vm| !vm.created)?;
            let image = resolve(o, vm)?;
            Some(VmProvision {
                order_id: o.id.clone(),
                namespace: namespace_for(&o.buyer)?,
                host: o.seller.clone(),
                replicas: o.quantity,
                source: image.source,
                disk_gib: image.disk_gib,
                memory_mib: image.memory_mib,
                efi: image.efi,
                user_data: vm.user_data.clone(),
            })
        })
        .collect()
}

/// Lapsed machine orders whose replicas are still set to run:
/// `(order id, namespace, replica count)`.
#[must_use]
pub fn lapsed_vms(state: &MarketState, now: u64) -> Vec<(String, String, u64)> {
    state
        .orders
        .values()
        .filter(|o| holds_replicas(o) && !paid_live(o, now))
        .filter_map(|o| Some((o.id.clone(), namespace_for(&o.buyer)?, o.quantity)))
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
                if let Some(bytes) = crate::identity::from_hex(v) {
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
    format!("t={timestamp},v1={}", crate::identity::to_hex(tag.as_ref()))
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
    let delivery = Delivery::parse(event);
    if let Some(id) = &delivery.id {
        if state.sync.applied(id) {
            return false;
        }
    }
    let changed = match delivery.event {
        Event::CheckoutCompleted {
            session,
            order,
            paid,
            amount,
            currency,
            payment_intent,
        } => {
            paid && order.is_some_and(|order| {
                complete_checkout(
                    state,
                    &order,
                    session,
                    amount,
                    currency.as_deref(),
                    payment_intent,
                    now,
                )
            })
        }
        Event::CheckoutExpired { session, order } => {
            order.is_some_and(|order| expire_checkout(state, &order, session.as_deref()))
        }
        Event::AccountUpdated { account, ready } => {
            update_account(state, &account, ready, delivery.created)
        }
        Event::Other(_) => false,
    };
    if changed {
        if let Some(id) = &delivery.id {
            state.sync.record(id);
        }
    }
    changed
}

/// Whether `id` has the shape of a Stripe payment intent id.
pub(crate) fn payment_intent_ok(id: &str) -> bool {
    id.len() <= 64
        && id.strip_prefix("pi_").is_some_and(|rest| {
            !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
}

/// `checkout.session.completed`, paid, for `order_id`. The platform holds
/// the payment ([`Card`]) until the slot is over.
fn complete_checkout(
    state: &mut MarketState,
    order_id: &str,
    session: Option<String>,
    amount: Option<u64>,
    currency: Option<&str>,
    payment_intent: Option<String>,
    now: u64,
) -> bool {
    let card = |step, reason| {
        payment_intent
            .clone()
            .filter(|pi| payment_intent_ok(pi))
            .map(|payment_intent| Card {
                payment_intent,
                step,
                reason,
                refund: None,
                tried_at: None,
            })
    };
    let Some(order) = state.orders.get(order_id) else {
        return false;
    };
    // A missing `session_id` means the edge stopped between Stripe
    // creating the session and the id being written. The event is
    // signed and names this order, which only this edge's gate can
    // have put in `client_reference_id`, so its id is adopted.
    let matches = session.as_deref().is_some_and(|s| s.starts_with("cs_"))
        && order
            .session_id
            .as_deref()
            .is_none_or(|s| Some(s) == session.as_deref())
        && amount == Some(order.amount)
        && currency.is_some_and(|c| c.eq_ignore_ascii_case(&order.currency));
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
        // otherwise they belong to someone else now and the payment goes
        // back to the buyer.
        OrderStatus::Expired => {
            let room = state
                .listings
                .get(&order.listing_id)
                .map_or(0, |l| available(state, l, now));
            if order.quantity > room {
                let refund = card(EscrowStep::Refund, Some(RefundReason::Lapsed));
                if refund.is_none() {
                    tracing::error!(
                        target: Action::Market.target(),
                        "order {} was paid after its reservation lapsed and its units are sold, but the event names no payment to refund; refund session {} in the Stripe dashboard",
                        order.id,
                        session.unwrap_or_default(),
                    );
                    return false;
                }
                tracing::warn!(
                    target: Action::Market.target(),
                    "order {} was paid after its reservation lapsed and its units are sold; refunding it",
                    order.id,
                );
                let Some(order) = state.orders.get_mut(order_id) else {
                    return false;
                };
                if order.card.is_some() {
                    return false;
                }
                order.session_id = session;
                order.card = refund;
                return true;
            }
            tracing::warn!(
                target: Action::Market.target(),
                "order {} was paid after its reservation lapsed; its units are still free, so it is fulfilled",
                order.id,
            );
        }
    }
    let held = card(EscrowStep::Held, None);
    if held.is_none() {
        tracing::error!(
            target: Action::Market.target(),
            "paid session for order {} names no payment intent; the seller's share must be transferred by hand",
            order.id,
        );
    }
    let offline_base = state.presence.get(&order.seller).map_or(0, |p| p.offline);
    let Some(order) = state.orders.get_mut(order_id) else {
        return false;
    };
    order.session_id = session;
    order.status = OrderStatus::Paid;
    order.paid_at = Some(now);
    order.expires_at = Some(crate::plans::plan(order.kind).expiry(now));
    order.card = held;
    order.offline_base = offline_base;
    true
}

/// `checkout.session.expired` for `order_id`.
fn expire_checkout(state: &mut MarketState, order_id: &str, session: Option<&str>) -> bool {
    let Some(order) = state.orders.get_mut(order_id) else {
        return false;
    };
    // Only the session the order is waiting on can release it. A
    // retried order gets a new session, and the old one's `expired`
    // event names the same order; without this it would free units
    // the live session still holds. An order with no recorded
    // session is left to `expire_stale`.
    let current = order.session_id.is_some() && order.session_id.as_deref() == session;
    if order.status == OrderStatus::Pending && current {
        order.status = OrderStatus::Expired;
        true
    } else {
        false
    }
}

/// `account.updated`: the account as Stripe had it at `created`. A state
/// older than the one the ledger holds is dropped ([`crate::sync`]).
fn update_account(
    state: &mut MarketState,
    account: &str,
    ready: bool,
    created: Option<u64>,
) -> bool {
    if !state.sellers.values().any(|s| s.account_id == account) {
        return false;
    }
    if created.is_some_and(|c| state.sync.stale(account, c)) {
        tracing::info!(
            target: Action::Market.target(),
            "ignoring an account.updated for {account} older than the state already held",
        );
        return false;
    }
    let mut changed = created.is_some_and(|c| state.sync.saw_account(account, c));
    for seller in state
        .sellers
        .values_mut()
        .filter(|s| s.account_id == account)
    {
        if seller.ready != ready {
            seller.ready = ready;
            changed = true;
        }
    }
    changed
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
    /// `test` while the edge runs on a test key: ordering takes test cards
    /// and buys nothing real.
    pub mode: StripeMode,
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
    /// While a slot someone paid for runs on it, the seller cannot stop
    /// selling it before this time (`crate::slots::locked_until`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_until: Option<u64>,
}

/// A card payment's progress as either party sees it. The payment intent
/// stays on the edge.
#[derive(Debug, Serialize)]
pub struct CardView {
    pub step: EscrowStep,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<RefundReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund: Option<u64>,
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
    /// What the treasury takes on top of `fee` toward the seller's platform
    /// fees owed on coin sales. Absent when none.
    #[serde(skip_serializing_if = "is_zero")]
    pub fee_offset: u64,
    /// `amount` less `fee` and `fee_offset`.
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
    /// A machine order's image, name and address. Absent on other kinds.
    pub vm: Option<VmView>,
    /// The transaction that paid a USDC order, for a link to an explorer.
    pub tx: Option<String>,
    /// The network it was paid on. Absent on an order paid before the edge
    /// named networks, which was paid on its first EVM network.
    pub network: Option<String>,
    /// Where the escrowed payment is going, on an EVM USDC order.
    pub escrow: Option<Escrow>,
    /// The coin payment, when the buyer chose one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coin: Option<CoinView>,
    /// Where the card payment is going, on a card order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<CardView>,
    /// While the payment is held: when the edge pays the seller by itself if
    /// the seller has not asked sooner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payout_at: Option<u64>,
}

/// A coin payment as the buyer's wallet shows it. Amounts are in whole coins
/// as decimal text, so no precision is lost on the way to the page.
#[derive(Debug, Serialize)]
pub struct CoinView {
    pub coin: &'static str,
    pub address: String,
    pub amount: String,
    /// The payment link with the exact amount: BIP 21, `monero:` or ZIP 321.
    pub uri: String,
    pub pay_before: u64,
    /// `waiting`, `seen`, `underpaid`, `confirming`, `paid`, `late` or
    /// `repricing` (see [`crate::coins::Settlement`]).
    pub state: &'static str,
    pub paid: String,
    /// What is owed when it is short: the quote, or the price now when late.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    /// Paid over the quote, recorded for a person to refund.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmations: Option<u64>,
    pub required: u64,
    pub txs: Vec<String>,
}

impl CoinView {
    pub fn of(order_id: &str, c: &CoinPayment) -> Self {
        use crate::coins::{format_amount, Settlement};
        let coin = c.quote.coin;
        let fmt = |a: u128| format_amount(coin, a);
        let waiting = Settlement::Waiting;
        let settlement = c.settlement.as_ref().unwrap_or(&waiting);
        let (state, paid, due, extra, confirmations) = match *settlement {
            Settlement::Waiting => ("waiting", 0, None, None, None),
            Settlement::Seen { paid } => ("seen", paid, None, None, Some(0)),
            Settlement::Underpaid { paid, due } => ("underpaid", paid, Some(due), None, None),
            Settlement::Confirming {
                paid,
                confirmations,
                ..
            } => ("confirming", paid, None, None, Some(confirmations)),
            Settlement::Paid { paid, extra } => {
                ("paid", paid, None, (extra > 0).then_some(extra), None)
            }
            Settlement::Late { paid, due_now } => ("late", paid, Some(due_now), None, None),
            Settlement::Repricing { paid } => ("repricing", paid, None, None, None),
        };
        Self {
            coin: coin.key(),
            address: c.address.clone(),
            amount: fmt(c.quote.amount),
            uri: crate::coins::payment_uri(coin, &c.address, c.quote.amount, order_id),
            pay_before: c.quote.pay_before,
            state,
            paid: fmt(paid),
            due: due.map(fmt),
            extra: extra.map(fmt),
            confirmations,
            required: crate::coins::confirmations_for(coin, c.quote.usd_cents),
            txs: c.txs.clone(),
        }
    }
}

/// A machine order as either party sees it. The cloud-init is not echoed: it
/// may carry a password.
#[derive(Debug, Serialize)]
pub struct VmView {
    pub image: String,
    pub name: String,
    /// `https://<order>.<vm-domain>`, when the edge publishes machines.
    pub address: Option<String>,
    /// The replicas exist on the host.
    pub created: bool,
    /// The order lapsed and every replica was stopped.
    pub halted: bool,
}

impl OrderView {
    fn of(o: &Order, now: u64, vm_domain: Option<&str>) -> Self {
        Self {
            id: o.id.clone(),
            listing_id: o.listing_id.clone(),
            kind: o.kind,
            unit: o.kind.unit(),
            quantity: o.quantity,
            amount: o.amount,
            fee: o.fee,
            fee_offset: o.fee_offset,
            seller_net: o.amount.saturating_sub(o.fee).saturating_sub(o.fee_offset),
            currency: o.currency.clone(),
            status: o.status,
            created_at: o.created_at,
            paid_at: o.paid_at,
            expires_at: o.expires_at,
            expired: o.status == OrderStatus::Paid && !paid_live(o, now),
            volume: o.volume.clone(),
            vm: o.vm.as_ref().map(|vm| VmView {
                image: vm.image.clone(),
                name: vm.name.clone(),
                address: vm_domain
                    .filter(|_| vm.service_ip.is_some())
                    .map(|d| format!("https://{}.{d}", crate::vms::stem(&o.id))),
                created: vm.created,
                halted: vm.halted,
            }),
            tx: o.tx_hash.clone(),
            network: o.paid_network.clone(),
            coin: o.coin.as_ref().map(|c| CoinView::of(&o.id, c)),
            escrow: o.escrow.clone(),
            card: o.card.as_ref().map(|c| CardView {
                step: c.step,
                reason: c.reason,
                refund: c.refund,
            }),
            payout_at: crate::slots::payout_at(o),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AccountView {
    /// The Stripe mode the edge's market runs in, and so which ledger this
    /// view was read from.
    pub mode: StripeMode,
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
    /// Whether it is enrolled and hosting machines, which is what lets it list
    /// them. Always false on an edge that runs no machines.
    pub can_host_vms: bool,
    /// The platform's share of a machine sale, when the edge runs machines.
    pub vm_fee_bps: Option<u32>,
    /// The buyer's own machine images on the edge.
    pub uploads: Vec<UploadView>,
    pub listings: Vec<OwnListing>,
    pub entitlements: Entitlements,
    pub purchases: Vec<OrderView>,
    pub sales: Vec<OrderView>,
    /// The seller's money held for running slots and ready to pay out.
    pub payouts: crate::slots::PayoutView,
    /// Whether buyers may pay in ways that cannot be refunded
    /// ([`MarketOpts::direct_pay`]). Off, a Solana payout address is not
    /// offered to buyers.
    pub direct_pay: bool,
    /// How long a seller's box may be offline in a row during a slot before
    /// its buyer is refunded the share not delivered.
    pub offline_refund_secs: u64,
    /// The seller's USDC address, when they have set one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payout: Option<String>,
    /// The seller's Solana USDC owner, when they have set one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payout_solana: Option<String>,
    /// Present when this edge settles the market in USDC. No RPC URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usdc: Option<crate::stablecoin::UsdcPublic>,
    /// Coins other than USDC this edge takes for orders (`btc`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub coins: Vec<&'static str>,
    /// This seller's keys for them, by coin, as the edge's node read them.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub coin_keys: BTreeMap<String, String>,
    /// Platform fees owed on this seller's coin sales, in the currency's
    /// minor unit.
    #[serde(skip_serializing_if = "is_zero")]
    pub coin_fees_owed: u64,
    /// When this seller started to owe them, unix seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coin_fees_since: Option<u64>,
    /// Credit toward the next coin fees, from USDC sales that collected
    /// more than was owed by the time they landed.
    #[serde(skip_serializing_if = "is_zero")]
    pub coin_fee_credit: u64,
    /// What a seller may owe before buyers can no longer pay them in coins,
    /// when this edge takes coins.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coin_fee_limit: Option<u64>,
    /// How long a seller may owe anything before the same, in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coin_fee_grace_secs: Option<u64>,
    /// Buyers cannot pay this seller in coins until the fees are settled.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub coin_sales_paused: bool,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// `POST /market/fees/pay`: what a transfer to the treasury settled.
#[derive(Debug, Serialize)]
pub struct FeePaymentView {
    /// What reached the treasury, in the currency's minor unit.
    pub paid: u64,
    /// What of it counted toward the fees owed.
    pub credited: u64,
    /// What is still owed.
    pub owed: u64,
}

#[derive(Debug, Serialize)]
pub struct OnboardView {
    pub mode: StripeMode,
    pub ready: bool,
    /// Stripe-hosted onboarding link, absent once the account is ready.
    pub url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CheckoutView {
    pub mode: StripeMode,
    pub order_id: String,
    /// Stripe's hosted page. Absent when the order is paid in USDC: the
    /// wallet pays [`stablecoin`] on the box, and there is no hosted page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout_url: Option<String>,
    pub amount: u64,
    pub fee: u64,
    pub currency: String,
    /// Set on a USDC order. The wallet approves `usdc` for `pay` and calls
    /// `pay(order_id_hash, seller, gross, fee)`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stablecoin: Option<crate::stablecoin::StablecoinInvoice>,
}

/// A hardware Checkout, as the box that asked for it sees it.
#[derive(Debug, Serialize)]
pub struct HardwareCheckoutView {
    pub mode: StripeMode,
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
    /// A machine order's image: a catalogue id or one of the buyer's uploads.
    #[serde(default)]
    pub image: Option<String>,
    /// A machine order's display name.
    #[serde(default)]
    pub name: Option<String>,
    /// A machine order's cloud-init, optional.
    #[serde(default)]
    pub user_data: Option<String>,
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
    /// The `rest-server` image the backup door runs on a bought volume
    /// (`server/storage.rs`); `None` keeps the door shut.
    pub storage_image: Option<String>,
    /// The hardware catalogue (`crate::hardware`), when the edge sells boxes
    /// and gateways. Unset, `/market/hardware*` answers 503.
    pub hardware_catalogue: Option<String>,
    /// Virtual machines (`crate::vms`), when the edge's mesh runs KubeVirt.
    /// Unset, nobody can list or order a machine.
    pub vms: Option<crate::vms::VmOpts>,
    /// USDC settlement. `None` keeps the Stripe rail. Hardware checkouts stay
    /// on Stripe either way.
    pub usdc: Option<crate::stablecoin::UsdcOpts>,
    /// The edge's Bitcoin node, for orders paid in BTC.
    pub btc: Option<crate::bitcoin::BtcOpts>,
    /// Monero orders, through the edge's own wallet service and node.
    pub xmr: Option<crate::monero::XmrOpts>,
    /// Whether buyers may pay by card, through the Stripe gate. Off, the
    /// edge takes coins only and needs no Stripe account: its mode is the
    /// chains' ([`crate::connect::NoCards`]) and `gate_socket` is unused.
    pub cards: bool,
    /// Shielded Zcash orders, watched through public lightwalletd servers.
    pub zec: Option<crate::zcash::ZecOpts>,
    /// What a seller may owe in platform fees on coin sales, in the minor
    /// unit, before buyers can no longer pay them in coins
    /// (`--market-coin-fee-limit`, [`DEFAULT_COIN_FEE_LIMIT`]).
    pub coin_fee_limit: u64,
    /// How long a seller's box may be offline in a row before its buyers are
    /// refunded (`--market-offline-refund-after`,
    /// [`crate::slots::DEFAULT_OFFLINE_SECS`]).
    pub offline_refund_secs: u64,
    /// Whether buyers may also pay in ways that reach the seller's wallet at
    /// once: Bitcoin, Monero, Zcash and Solana USDC (`--market-direct-pay`).
    /// Nothing paid that way can be held for the slot or refunded, so it is
    /// off unless the operator turns it on.
    pub direct_pay: bool,
}

/// How long an escrow call may sit unmined before the edge checks the
/// deposit and sends it again at a higher fee.
const ESCROW_RESEND_SECS: u64 = 10 * 60;

/// How long the edge waits for a Solana transaction it sent before it reads
/// the deposit again. Its blockhash lasts about a minute, so after this it
/// either landed or never will.
const SOLANA_RESEND_SECS: u64 = 3 * 60;

/// Whose escrow record a pass is updating.
enum EscrowJob {
    Order(String),
    /// A stray payment, by its paying transaction.
    Stray(String),
}

/// What a USDC payment must show, per family.
enum Expected {
    Evm(crate::stablecoin::ExpectedPayment),
    /// Held in `losos-escrow`.
    SolanaEscrow(crate::solana_escrow::ExpectedDeposit),
    /// Straight to the seller, on a direct-pay edge.
    Solana(crate::solana::ExpectedTransfer),
}

/// Where a confirmed payment is held: the deposit, and whether the seller
/// can receive it.
struct HeldAt {
    deposit: u64,
    account: Option<String>,
    deadline: Option<u64>,
    seller_ok: bool,
}

pub struct Market {
    opts: MarketOpts,
    state: Mutex<Ledger>,
    /// The Connect platform: the gate on an edge.
    connect: Arc<dyn Connect>,
    /// The ENS endpoint answered with this mode's chain once.
    ens_checked: std::sync::atomic::AtomicBool,
    /// ETH and SOL prices for comparing USDC network fees, refreshed in the
    /// background as pages ask.
    prices: Arc<crate::coins::PriceBook>,
    /// One escrow pass at a time: the releaser key's nonce is taken from the
    /// chain, so two passes sending at once would collide.
    escrow_lock: Mutex<()>,
    /// The Zcash servers and what each served before, when the edge takes
    /// Zcash.
    zec: Option<crate::zcash::Scanner>,
    /// When the slots pass last wrote what it watched to disk
    /// ([`Market::slots_pass`]).
    presence_saved: std::sync::atomic::AtomicU64,
    /// One card settle pass at a time.
    card_lock: Mutex<()>,
    /// The escrows taps are read from and settled in.
    tap_chain: Arc<dyn TapChain>,
    taps_live: taps::TapsLive,
}

/// How often the slots pass writes the offline time it counted even when
/// nothing turned. A restart loses at most this much of it, which counts in
/// the seller's favour.
const PRESENCE_SAVE_SECS: u64 = 5 * 60;

/// How long a card settle that failed waits before it is tried again.
const CARD_RETRY_SECS: u64 = 10 * 60;

/// The ledger of the mode the gate's key is in, and where it is kept. The
/// edge reads the mode off the key on every market request
/// ([`Market::check_mode`]) and opens the other ledger when it changes,
/// so going live is sealing a live key and restarting the gate: test sellers,
/// listings and orders stay in the test ledger, and a test payment never
/// becomes a live entitlement.
///
/// Derefs to the state, so code that only reads or writes the market sees a
/// [`MarketState`] as before.
struct Ledger {
    /// `None` until the gate has said which mode its key is in. Nothing is
    /// written then.
    mode: Option<StripeMode>,
    path: PathBuf,
    state: MarketState,
}

impl std::ops::Deref for Ledger {
    type Target = MarketState;
    fn deref(&self) -> &MarketState {
        &self.state
    }
}

impl std::ops::DerefMut for Ledger {
    fn deref_mut(&mut self) -> &mut MarketState {
        &mut self.state
    }
}

/// Where the ledger of `mode` is kept: `state_file` itself for live, and a
/// `-test` sibling for test (`market.json` and `market-test.json`).
#[must_use]
pub fn ledger_path(state_file: &Path, mode: StripeMode) -> PathBuf {
    match mode {
        StripeMode::Live => state_file.to_path_buf(),
        StripeMode::Test => {
            let stem = state_file
                .file_stem()
                .map_or_else(|| "market".into(), |s| s.to_string_lossy().into_owned());
            let name = match state_file.extension() {
                Some(ext) => format!("{stem}-test.{}", ext.to_string_lossy()),
                None => format!("{stem}-test"),
            };
            state_file.with_file_name(name)
        }
    }
}

/// Read the ledger of `mode`. A missing file is an empty ledger; one that does
/// not parse, or that says it belongs to the other mode, is an error, because
/// starting empty would forget who has paid for what.
async fn read_ledger(path: &Path, mode: StripeMode) -> Result<MarketState, MarketError> {
    let mut state: MarketState = match tokio::fs::read(path).await {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => MarketState::default(),
        Err(e) => return Err(e.into()),
    };
    match state.mode {
        Some(written) if written != mode => Err(MarketError::Store(format!(
            "{} is the {written} ledger, not the {mode} one",
            path.display()
        ))),
        _ => {
            state.mode = Some(mode);
            stamp_debts(&mut state, now_secs());
            Ok(state)
        }
    }
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

/// 32 random bytes as 64 hex characters: an upload's ticket or fetch token.
fn random_capability() -> Result<String, MarketError> {
    crate::identity::random_hex(32).ok_or_else(|| MarketError::Store("no randomness".to_string()))
}

/// Drop uploads whose ticket lapsed with no file stored; their ids.
pub fn sweep_uploads(state: &mut MarketState, now: u64) -> Vec<String> {
    let dropped: Vec<String> = state
        .uploads
        .values()
        .filter(|u| !u.stored() && now >= u.created_at + crate::vms::UPLOAD_TICKET_SECS)
        .map(|u| u.id.clone())
        .collect();
    for id in &dropped {
        state.uploads.remove(id);
    }
    dropped
}

fn random_id(prefix: &str) -> Result<String, MarketError> {
    let hex = crate::identity::random_hex(12)
        .ok_or_else(|| MarketError::Store("no randomness".to_string()))?;
    Ok(format!("{prefix}_{hex}"))
}

impl Market {
    /// Open the store. Which ledger is read depends on the mode of the gate's
    /// key, so nothing is read until the first market request asks the gate
    /// ([`Market::check_mode`]). Both ledgers are checked here so a
    /// file that does not parse fails the start rather than the first buyer.
    pub async fn open(opts: MarketOpts) -> Result<Self, MarketError> {
        if opts.cards {
            let gate = Arc::new(GateClient::new(&opts.gate_socket));
            return Self::open_with(opts, gate).await;
        }
        // Coins only. The parser refuses this without USDC, whose networks
        // are all test or all live; that is the mode.
        let mode = match &opts.usdc {
            Some(usdc) if usdc.test() => StripeMode::Test,
            Some(_) => StripeMode::Live,
            None => return Err(MarketError::Unconfigured),
        };
        Self::open_with(opts, Arc::new(crate::connect::NoCards { mode })).await
    }

    /// As [`Market::open`], settling through `connect` rather than the gate
    /// at `opts.gate_socket`. Hardware checkouts still go to the gate: they
    /// pay no seller.
    pub async fn open_with(
        opts: MarketOpts,
        connect: Arc<dyn Connect>,
    ) -> Result<Self, MarketError> {
        let path = PathBuf::from(&opts.state_file);
        for mode in [StripeMode::Live, StripeMode::Test] {
            read_ledger(&ledger_path(&path, mode), mode).await?;
        }
        let prices = Arc::new(crate::coins::PriceBook::new(
            opts.usdc.as_ref().map_or_else(
                || {
                    [
                        crate::coins::PriceSource::kraken(),
                        crate::coins::PriceSource::coingecko(),
                    ]
                },
                |u| u.price_sources.clone(),
            ),
        ));
        let zec = opts.zec.clone().map(crate::zcash::Scanner::new);
        let opts_usdc = opts.usdc.clone();
        Ok(Self {
            opts,
            zec,
            state: Mutex::new(Ledger {
                mode: None,
                path,
                state: MarketState::default(),
            }),
            connect,
            ens_checked: std::sync::atomic::AtomicBool::new(false),
            prices,
            escrow_lock: Mutex::new(()),
            presence_saved: std::sync::atomic::AtomicU64::new(0),
            card_lock: Mutex::new(()),
            tap_chain: Arc::new(EscrowChain::new(opts_usdc)),
            taps_live: taps::TapsLive::default(),
        })
    }

    #[must_use]
    pub fn fee_bps(&self) -> u32 {
        self.opts.fee_bps
    }

    /// Check the gate has its secrets, and switch to the ledger of its key's
    /// mode. Every market route calls this first.
    pub async fn check_mode(&self) -> Result<StripeMode, MarketError> {
        let mode = self.connect.mode().await?;
        if let Some(usdc) = &self.opts.usdc {
            // The chain is the mode, the same way the key's prefix is. A
            // Sepolia contract must not settle an order the live ledger
            // would treat as paid, or the other way around.
            if usdc.test() != (mode == StripeMode::Test) {
                tracing::error!(
                    target: Action::Market.target(),
                    "USDC networks are {} but the Stripe key is in {mode} mode",
                    if usdc.test() { "test" } else { "live" },
                );
                return Err(MarketError::Unconfigured);
            }
        }
        self.use_mode(mode).await?;
        Ok(mode)
    }

    /// The mode last learned from the gate, if any request has asked yet.
    pub async fn mode(&self) -> Option<StripeMode> {
        self.state.lock().await.mode
    }

    /// Make `mode`'s ledger the live one. Every write has already reached the
    /// disk, so the other ledger is simply dropped from memory.
    async fn use_mode(&self, mode: StripeMode) -> Result<(), MarketError> {
        let mut ledger = self.state.lock().await;
        if ledger.mode == Some(mode) {
            return Ok(());
        }
        let path = ledger_path(Path::new(&self.opts.state_file), mode);
        let state = read_ledger(&path, mode).await?;
        match ledger.mode {
            Some(was) => tracing::warn!(
                target: Action::Market.target(),
                "the Stripe key changed from {was} to {mode} mode; the market now reads {}",
                path.display(),
            ),
            None => tracing::info!(
                target: Action::Market.target(),
                "the market runs in {mode} mode from {}",
                path.display(),
            ),
        }
        *ledger = Ledger {
            mode: Some(mode),
            path,
            state,
        };
        Ok(())
    }

    async fn persist(&self, path: &Path, state: &MarketState) -> Result<(), MarketError> {
        let bytes = serde_json::to_vec_pretty(state)?;
        atomic_write(path, &bytes, STORE_FILE_MODE).await?;
        Ok(())
    }

    /// Make `next` the live state, but only once it is on disk. Every write
    /// goes through here, each built on a clone taken under the lock. A change
    /// made to the live state first would outlive a failed write (a full
    /// disk): the caller is told it failed, yet the change stands in memory,
    /// is acted on, and the next successful write commits it silently.
    ///
    /// Refused while no mode is known: a write then could land in the wrong
    /// ledger.
    async fn commit(&self, live: &mut Ledger, next: MarketState) -> Result<(), MarketError> {
        if live.mode.is_none() || next.mode != live.mode {
            return Err(MarketError::Unconfigured);
        }
        self.persist(&live.path, &next).await?;
        live.state = next;
        Ok(())
    }

    /// The mode of the ledger behind `ledger`, for a view built from it.
    fn mode_of(ledger: &Ledger) -> Result<StripeMode, MarketError> {
        ledger.mode.ok_or(MarketError::Unconfigured)
    }

    fn stripe(&self) -> GateClient {
        GateClient::new(&self.opts.gate_socket)
    }

    /// The tenant's Stripe connected account as last recorded, if it has one.
    /// Read by [`crate::domains`]: a box's public names hang off it.
    pub async fn seller(&self, id: &str) -> Option<Seller> {
        self.vouching_sellers().await.remove(id)
    }

    /// Every recorded seller, for the reconciler's zone pass.
    pub async fn sellers(&self) -> BTreeMap<String, Seller> {
        self.vouching_sellers().await
    }

    /// The sellers of the mode the gate's key is in. A box's public names
    /// hang off these, so they must not vanish while the gate cannot be
    /// asked (a registrar that starts before it): until a mode is known they
    /// are read from the live ledger on disk, which holds the accounts that
    /// moved real money.
    async fn vouching_sellers(&self) -> BTreeMap<String, Seller> {
        if self.mode().await.is_none() {
            // The fault, if any, is the gate's to report; the routes do.
            let _ = self.check_mode().await;
        }
        let ledger = self.state.lock().await;
        if ledger.mode.is_some() {
            return ledger.sellers.clone();
        }
        drop(ledger);
        let live = ledger_path(Path::new(&self.opts.state_file), StripeMode::Live);
        read_ledger(&live, StripeMode::Live)
            .await
            .map(|s| s.sellers)
            .unwrap_or_default()
    }

    /// Everything one appliance may see about its own market activity.
    pub async fn account(&self, id: &str, sharing: &Sharing) -> Result<AccountView, MarketError> {
        let now = now_secs();
        let state = self.state.lock().await;
        let mode = Self::mode_of(&state)?;
        let seller = state.sellers.get(id);
        let coin_seller = state.coin_sellers.get(id);
        let takes_coins = !self.coins().is_empty();
        Ok(AccountView {
            mode,
            fee_bps: self.opts.fee_bps,
            currency: self.opts.currency.clone(),
            seller_onboarded: seller.is_some(),
            seller_ready: self.accepts(&state, id),
            can_sell_storage: sharing.allows(id, Kind::Storage),
            can_sell_compute: sharing.allows(id, Kind::Compute),
            can_host_vms: self.opts.vms.is_some() && sharing.allows(id, Kind::Vm),
            vm_fee_bps: self.opts.vms.as_ref().map(|_| VM_FEE_BPS),
            uploads: state
                .uploads
                .values()
                .filter(|u| u.owner == id)
                .map(UploadView::of)
                .collect(),
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
                    locked_until: crate::slots::locked_until(&state, &l.id, now),
                })
                .collect(),
            entitlements: entitlements(&state, id, now),
            purchases: history(
                state.orders.values().filter(|o| o.buyer == id),
                now,
                self.vm_domain(),
            ),
            sales: history(
                state.orders.values().filter(|o| o.seller == id),
                now,
                self.vm_domain(),
            ),
            payouts: crate::slots::payout_view(&state, id, now),
            direct_pay: self.opts.direct_pay,
            offline_refund_secs: self.opts.offline_refund_secs,
            payout: seller.and_then(|s| s.payout.clone()),
            payout_solana: seller.and_then(|s| s.payout_solana.clone()),
            usdc: self.opts.usdc.as_ref().and_then(|usdc| {
                let mut public = usdc.public();
                // Solana without the escrow program pays the seller at once,
                // so without direct pay it is neither a way to pay nor a
                // payout to set up.
                if !self.opts.direct_pay {
                    public.chains.retain(|c| c.pay.is_some());
                    if !public
                        .chains
                        .iter()
                        .any(|c| c.family == crate::stablecoin::Family::Solana)
                    {
                        public.solana = None;
                    }
                    if public.chains.is_empty() {
                        return None;
                    }
                }
                public.prices = self.prices.snapshot(&crate::coins::Coin::FEES, now);
                self.prices
                    .refresh_in_background(&crate::coins::Coin::FEES, now);
                Some(public)
            }),
            coins: self.coins().iter().map(|c| c.key()).collect(),
            coin_keys: state
                .coin_sellers
                .get(id)
                .map(|s| {
                    s.keys
                        .iter()
                        .map(|(coin, k)| (coin.clone(), k.descriptor.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            coin_fees_owed: coin_seller.map_or(0, |s| s.fees_owed),
            coin_fees_since: coin_seller.and_then(|s| s.owed_since),
            coin_fee_credit: coin_seller.map_or(0, |s| s.fee_credit),
            coin_fee_limit: takes_coins.then_some(self.opts.coin_fee_limit),
            coin_fee_grace_secs: takes_coins.then_some(COIN_FEE_GRACE_SECS),
            coin_sales_paused: coin_seller
                .is_some_and(|s| s.coins_paused(self.opts.coin_fee_limit, now)),
        })
    }

    /// Whether `seller` can be paid for a listing. USDC wants a signed payout
    /// address. Stripe wants the connected account ready. Hardware checkouts
    /// do not come through here.
    /// Whether `id` can be paid for a listing: a payout address or a ready
    /// Stripe account, or a key for a coin this edge takes.
    fn accepts(&self, state: &MarketState, id: &str) -> bool {
        state
            .sellers
            .get(id)
            .is_some_and(|s| self.accepts_payouts(s))
            || !self.coins_of(state, id).is_empty()
    }

    /// Whether orders may be paid in USDC on Solana: held in the escrow
    /// program, or straight to the seller on a direct-pay edge.
    fn solana_takes_orders(&self) -> bool {
        self.opts.direct_pay
            || self
                .opts
                .usdc
                .as_ref()
                .is_some_and(crate::stablecoin::UsdcOpts::solana_escrowed)
    }

    /// Coins this edge takes for orders, in the order the buyer sees them.
    /// None without [`MarketOpts::direct_pay`]: a coin payment reaches the
    /// seller at once, so the slot has nothing held to refund from.
    pub fn coins(&self) -> Vec<crate::coins::Coin> {
        let mut coins = Vec::new();
        if !self.opts.direct_pay {
            return coins;
        }
        if self.opts.btc.is_some() {
            coins.push(crate::coins::Coin::Btc);
        }
        if self.opts.xmr.is_some() {
            coins.push(crate::coins::Coin::Xmr);
        }
        if self.opts.zec.is_some() {
            coins.push(crate::coins::Coin::Zec);
        }
        coins
    }

    /// The coins `seller` can be paid in on this edge.
    fn coins_of(&self, state: &MarketState, seller: &str) -> Vec<crate::coins::Coin> {
        let Some(keys) = state.coin_sellers.get(seller) else {
            return Vec::new();
        };
        self.coins()
            .into_iter()
            .filter(|c| keys.keys.contains_key(c.key()))
            .collect()
    }

    /// Whether a buyer can pay `seller` in USDC or by card. Solana USDC
    /// counts when the escrow program holds it, or with
    /// [`MarketOpts::direct_pay`]: without the program it goes straight to
    /// the seller, so the slot has nothing held to refund from.
    fn accepts_payouts(&self, seller: &Seller) -> bool {
        if let Some(usdc) = &self.opts.usdc {
            (seller.payout.is_some() && usdc.has(crate::stablecoin::Family::Evm))
                || (self.solana_takes_orders()
                    && seller.payout_solana.is_some()
                    && usdc.has(crate::stablecoin::Family::Solana))
        } else {
            seller.ready
        }
    }

    /// Every listing a buyer could order right now. Anonymous.
    pub async fn browse(&self, sharing: &Sharing) -> Result<Vec<PublicListing>, MarketError> {
        let now = now_secs();
        let state = self.state.lock().await;
        let mode = Self::mode_of(&state)?;
        Ok(state
            .listings
            .values()
            .filter(|l| l.active && self.accepts(&state, &l.seller))
            .filter(|l| sharing.allows(&l.seller, l.kind))
            .filter_map(|l| {
                let left = available(&state, l, now);
                (left > 0).then(|| PublicListing {
                    mode,
                    id: l.id.clone(),
                    kind: l.kind,
                    unit: l.kind.unit(),
                    unit_price: l.unit_price,
                    currency: self.opts.currency.clone(),
                    available: left,
                })
            })
            .collect())
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
        let connect = &self.connect;
        let (mode, existing) = {
            let state = self.state.lock().await;
            (Self::mode_of(&state)?, state.sellers.get(id).cloned())
        };
        let account_id = match existing {
            Some(seller) if !seller.account_id.is_empty() => {
                if let Some(uuid) = box_uuid {
                    if seller.box_uuid.as_deref() != Some(uuid) {
                        connect.tag_account(&seller.account_id, uuid).await?;
                        self.set_box_uuid(id, uuid).await?;
                    }
                }
                // Refresh a stale flag: the `account.updated` webhook may
                // have been missed. What Stripe answers now is newer than any
                // event still on its way, so the sync is told when it read.
                let read_at = now_secs();
                if seller.ready || connect.account_ready(&seller.account_id).await? {
                    self.set_ready(id, true, (!seller.ready).then_some(read_at))
                        .await?;
                    return Ok(OnboardView {
                        mode,
                        ready: true,
                        url: None,
                    });
                }
                seller.account_id
            }
            _ => {
                let (account_id, made_in) = connect.create_account(id, box_uuid).await?;
                let mut state = self.state.lock().await;
                // The gate was restarted on a key of the other mode while
                // the account was being made: it belongs in the other ledger.
                if state.mode != Some(made_in) || made_in != mode {
                    return Err(MarketError::Conflict(
                        "the edge's Stripe mode changed; try again",
                    ));
                }
                let mut next = state.clone();
                let previous = next.sellers.get(id);
                let payout = previous.and_then(|s| s.payout.clone());
                let payout_solana = previous.and_then(|s| s.payout_solana.clone());
                let box_uuid = box_uuid
                    .map(str::to_string)
                    .or_else(|| previous.and_then(|s| s.box_uuid.clone()));
                next.sellers.insert(
                    id.to_string(),
                    Seller {
                        account_id: account_id.clone(),
                        ready: false,
                        box_uuid,
                        payout,
                        payout_solana,
                    },
                );
                self.commit(&mut state, next).await?;
                account_id
            }
        };
        let url = connect
            .onboarding_link(&account_id, &self.opts.return_url)
            .await?;
        Ok(OnboardView {
            mode,
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

    /// Set the seller's readiness. `read_at` is when Stripe was asked, if it
    /// was: an `account.updated` created before then is older news.
    async fn set_ready(
        &self,
        id: &str,
        ready: bool,
        read_at: Option<u64>,
    ) -> Result<(), MarketError> {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        if let Some(seller) = next.sellers.get_mut(id) {
            let mut changed = seller.ready != ready;
            seller.ready = ready;
            let account = seller.account_id.clone();
            if let Some(at) = read_at {
                changed |= next.sync.saw_account(&account, at);
            }
            if changed {
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
        if new.kind == Kind::Vm && self.opts.vms.is_none() {
            return Err(MarketError::Conflict(
                "this edge does not run virtual machines",
            ));
        }
        if !sharing.allows(seller, new.kind) {
            return Err(MarketError::Conflict(match new.kind {
                Kind::Storage => "only an appliance sharing its storage on the mesh can sell it",
                Kind::Compute => "only an appliance sharing its compute on the mesh can sell it",
                Kind::Vm => {
                    "only an appliance sharing its storage on the mesh can host virtual machines"
                }
            }));
        }
        if new.kind == Kind::Vm && new.capacity > crate::vms::MAX_HOSTED_REPLICAS {
            return Err(MarketError::Invalid("a box can host at most 50 replicas"));
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
        if !self.accepts(&state, seller) {
            return Err(MarketError::Conflict(if self.opts.usdc.is_some() {
                "set a USDC payout address before listing"
            } else {
                "finish Stripe onboarding before listing"
            }));
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
            if crate::slots::locked_until(&state, listing_id, now_secs()).is_some() {
                return Err(MarketError::Conflict(SLOT_RUNNING));
            }
            let Some(listing) = next.listings.get_mut(listing_id) else {
                return Err(MarketError::NotFound);
            };
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
        let (_, checkout_url, mode) = self
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
            mode,
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
        let now = now_secs();
        let (order, destination, mode, coins) = {
            let mut state = self.state.lock().await;
            let mode = Self::mode_of(&state)?;
            let listing = state
                .listings
                .get(&new.listing_id)
                .filter(|l| l.active)
                .cloned()
                .ok_or(MarketError::NotFound)?;
            let seller = state
                .sellers
                .get(&listing.seller)
                .filter(|_| self.accepts(&state, &listing.seller))
                .cloned()
                .ok_or(MarketError::NotFound)?;
            // Not for sale while the seller has stopped sharing it.
            if !sharing.allows(&listing.seller, listing.kind) {
                return Err(MarketError::NotFound);
            }
            if matches!(listing.kind, Kind::Storage | Kind::Vm) && namespace_for(buyer).is_none() {
                return Err(MarketError::Invalid(
                    "appliance id cannot name a storage namespace",
                ));
            }
            if listing.seller == buyer {
                return Err(MarketError::Invalid("cannot buy your own listing"));
            }
            let vm = self.vm_order_of(&state, buyer, listing.kind, &new)?;
            let q = quote(
                listing.unit_price,
                new.quantity,
                fee_bps_for(listing.kind, self.opts.fee_bps),
            )?;
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
            let rail = if self.opts.usdc.is_some() {
                Rail::Usdc
            } else {
                Rail::Stripe
            };
            let usdc_payable = rail == Rail::Usdc && self.accepts_payouts(&seller);
            // A seller behind on coin fees is paid in USDC only, which
            // collects them; one who takes nothing else cannot be paid.
            let coins_paused = state
                .coin_sellers
                .get(&listing.seller)
                .is_some_and(|s| s.coins_paused(self.opts.coin_fee_limit, now));
            if rail == Rail::Usdc && !usdc_payable && coins_paused {
                return Err(MarketError::Conflict(COINS_PAUSED));
            }
            let fee_offset = if usdc_payable {
                fee_offset_for(&state, &listing.seller, q.amount, q.fee, now)
            } else {
                0
            };
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
                vm,
                rail,
                tx_hash: None,
                pay_to: if rail == Rail::Usdc {
                    seller.payout.clone()
                } else {
                    None
                },
                pay_to_solana: if rail == Rail::Usdc && self.solana_takes_orders() {
                    seller.payout_solana.clone()
                } else {
                    None
                },
                paid_network: None,
                coin: None,
                escrow: None,
                fee_offset,
                offset_collected: false,
                card: None,
                offline_base: 0,
            };
            let mut next = state.clone();
            expire_stale(&mut next, now);
            next.orders.insert(order.id.clone(), order.clone());
            self.commit(&mut state, next).await?;
            let coins = if coins_paused {
                Vec::new()
            } else {
                self.coins_of(&state, &order.seller)
            };
            (order, seller.account_id, mode, coins)
        };

        if order.rail == Rail::Usdc {
            let opts = self.opts.usdc.clone().ok_or(MarketError::Unconfigured)?;
            let mut invoice = crate::stablecoin::invoice(
                &opts,
                &order.id,
                crate::stablecoin::PayTo {
                    evm: order.pay_to.as_deref(),
                    solana: order.pay_to_solana.as_deref(),
                },
                order.amount,
                order.fee + order.fee_offset,
                order.created_at.saturating_add(USDC_HOLD_SECS),
            );
            invoice.fee_offset = (order.fee_offset > 0)
                .then(|| crate::stablecoin::cents_to_usdc(order.fee_offset).to_string());
            if invoice.chains.is_empty() && coins.is_empty() {
                return Err(MarketError::Conflict("the seller has no payout address"));
            }
            invoice.coins = coins.iter().map(|c| c.key()).collect();
            return Ok(CheckoutView {
                mode,
                order_id: order.id.clone(),
                checkout_url: None,
                amount: order.amount,
                fee: order.fee,
                currency: order.currency.clone(),
                stablecoin: Some(invoice),
            });
        }

        let session = match Split::new(order.amount, order.fee).and_then(|split| {
            DestinationCharge::new(ChargeSpec {
                order_id: &order.id,
                kind: order.kind,
                quantity: order.quantity,
                unit_price: order.unit_price,
                currency: &order.currency,
                split,
                destination: &destination,
                return_url: &self.opts.return_url,
                expires_at: now + CHECKOUT_TTL_SECS,
            })
        }) {
            Ok(charge) => self
                .connect
                .destination_charge(&charge)
                .await
                .map(|s| (s.id, s.url, s.mode)),
            Err(e) => Err(e),
        };
        let mut state = self.state.lock().await;
        if state.mode != Some(mode) {
            // The gate came back on a key of the other mode while the session
            // was being made. The order sits in a ledger that is no longer
            // read; its hold lapses there on its own.
            return Err(MarketError::Conflict(
                "the edge's Stripe mode changed; order again",
            ));
        }
        let mut next = state.clone();
        let session = session.and_then(|(id, url, made_in)| {
            if made_in == mode {
                Ok((id, url))
            } else {
                Err(MarketError::Conflict(
                    "the edge's Stripe mode changed; order again",
                ))
            }
        });
        match session {
            Ok((session_id, url)) => {
                if let Some(o) = next.orders.get_mut(&order.id) {
                    o.session_id = Some(session_id);
                }
                self.commit(&mut state, next).await?;
                Ok(CheckoutView {
                    mode,
                    order_id: order.id,
                    checkout_url: Some(url),
                    amount: order.amount,
                    fee: order.fee,
                    currency: order.currency,
                    stablecoin: None,
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

    pub fn storage_image(&self) -> Option<&str> {
        self.opts.storage_image.as_deref()
    }

    // ── virtual machines (crate::vms) ───────────────────────────────────

    /// The machine settings, when this edge runs machines.
    #[must_use]
    pub fn vm_opts(&self) -> Option<&crate::vms::VmOpts> {
        self.opts.vms.as_ref()
    }

    /// The domain machines are published under, when it is set.
    #[must_use]
    pub fn vm_domain(&self) -> Option<&str> {
        self.opts.vms.as_ref().and_then(|v| v.domain.as_deref())
    }

    /// The image catalogue, read per call like the hardware catalogue.
    ///
    /// # Errors
    /// [`MarketError::Unconfigured`] when the edge runs no machines or its
    /// catalogue file does not load.
    pub fn vm_catalogue(&self) -> Result<crate::vms::Catalogue, MarketError> {
        let opts = self.opts.vms.as_ref().ok_or(MarketError::Unconfigured)?;
        crate::vms::Catalogue::load(opts.catalogue.as_deref()).map_err(|e| {
            tracing::error!(target: Action::Market.target(), "virtual machine catalogue: {e}");
            MarketError::Unconfigured
        })
    }

    /// The machine half of a new order, checked: `None` for any other kind,
    /// which may not carry machine fields.
    fn vm_order_of(
        &self,
        state: &MarketState,
        buyer: &str,
        kind: Kind,
        new: &NewOrder,
    ) -> Result<Option<VmOrder>, MarketError> {
        if kind != Kind::Vm {
            if new.image.is_some() || new.name.is_some() || new.user_data.is_some() {
                return Err(MarketError::Invalid(
                    "an image, a name and cloud-init belong on virtual machine orders only",
                ));
            }
            return Ok(None);
        }
        // An edge that stopped running machines has nothing to sell.
        if self.opts.vms.is_none() {
            return Err(MarketError::NotFound);
        }
        if new.quantity > crate::vms::MAX_REPLICAS {
            return Err(MarketError::Invalid("at most 10 replicas in one order"));
        }
        let image = new
            .image
            .as_deref()
            .ok_or(MarketError::Invalid("pick an image for the machine"))?;
        let name = new
            .name
            .as_deref()
            .ok_or(MarketError::Invalid("give the machine a name"))?;
        if !crate::vms::display_name_ok(name) {
            return Err(MarketError::Invalid(
                "a machine's name is 1 to 40 printable characters",
            ));
        }
        if let Some(user_data) = &new.user_data {
            crate::vms::check_user_data(user_data)?;
        }
        let known = if image.starts_with("upl_") {
            state
                .uploads
                .get(image)
                .is_some_and(|u| u.owner == buyer && u.stored())
        } else {
            self.vm_catalogue()?.image(image).is_some()
        };
        if !known {
            return Err(MarketError::Invalid("no such image"));
        }
        Ok(Some(VmOrder {
            image: image.to_string(),
            name: name.to_string(),
            user_data: new.user_data.clone(),
            created: false,
            halted: false,
            service_ip: None,
        }))
    }

    /// Machine orders that are paid and live but have no replicas yet.
    pub async fn pending_vms(&self) -> Vec<VmProvision> {
        let Some(opts) = &self.opts.vms else {
            return Vec::new();
        };
        let Ok(catalogue) = self.vm_catalogue() else {
            return Vec::new();
        };
        let state = self.state.lock().await;
        pending_vms(&state, now_secs(), |o, vm| {
            opts.resolve(&catalogue, &state.uploads, &o.buyer, &vm.image)
        })
    }

    /// Lapsed machine orders whose replicas are still set to run.
    pub async fn lapsed_vms(&self) -> Vec<(String, String, u64)> {
        lapsed_vms(&*self.state.lock().await, now_secs())
    }

    /// Record that every replica of `order_id` and its Service exist.
    pub async fn mark_vm_created(
        &self,
        order_id: &str,
        service_ip: Option<String>,
    ) -> Result<(), MarketError> {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        match next.orders.get_mut(order_id).and_then(|o| o.vm.as_mut()) {
            Some(vm) => {
                vm.created = true;
                vm.service_ip = service_ip;
            }
            None => return Ok(()),
        }
        self.commit(&mut state, next).await
    }

    /// Record that every replica of a lapsed `order_id` is halted, which
    /// returns them to the listing.
    pub async fn mark_vm_halted(&self, order_id: &str) -> Result<(), MarketError> {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        match next.orders.get_mut(order_id).and_then(|o| o.vm.as_mut()) {
            Some(vm) => vm.halted = true,
            None => return Ok(()),
        }
        self.commit(&mut state, next).await
    }

    /// `(hostname, http://<service ip>)` for every live machine order the
    /// edge publishes, sorted, for the Traefik file.
    pub async fn vm_routes(&self) -> Vec<(String, String)> {
        let Some(domain) = self.vm_domain() else {
            return Vec::new();
        };
        let now = now_secs();
        let state = self.state.lock().await;
        let mut routes: Vec<(String, String)> = state
            .orders
            .values()
            .filter(|o| o.kind == Kind::Vm && paid_live(o, now))
            .filter_map(|o| {
                let vm = o.vm.as_ref().filter(|vm| vm.created && !vm.halted)?;
                let ip = vm.service_ip.as_deref()?;
                ip.parse::<std::net::IpAddr>().ok()?;
                Some((
                    format!("{}.{domain}", crate::vms::stem(&o.id)),
                    format!("http://{ip}:80"),
                ))
            })
            .collect();
        routes.sort();
        routes
    }

    /// `buyer`'s machine orders that have replicas to report on:
    /// `(order id, namespace, replicas)`.
    pub async fn vm_orders_of(&self, buyer: &str) -> Vec<(String, String, u64)> {
        let state = self.state.lock().await;
        state
            .orders
            .values()
            .filter(|o| o.buyer == buyer && o.kind == Kind::Vm && o.status == OrderStatus::Paid)
            .filter(|o| o.vm.as_ref().is_some_and(|vm| vm.created))
            .filter_map(|o| Some((o.id.clone(), namespace_for(&o.buyer)?, o.quantity)))
            .collect()
    }

    /// Reserve an upload for `owner`: its id and the single-use ticket the box
    /// streams the file to.
    ///
    /// # Errors
    /// Machines are off, the name is not one, or the owner already keeps
    /// [`crate::vms::MAX_UPLOADS_PER_BUYER`] images.
    pub async fn upload_ticket(
        &self,
        owner: &str,
        name: &str,
        efi: bool,
    ) -> Result<(String, String), MarketError> {
        if self.opts.vms.is_none() {
            return Err(MarketError::Unconfigured);
        }
        if !crate::vms::display_name_ok(name) {
            return Err(MarketError::Invalid(
                "an image's name is 1 to 40 printable characters",
            ));
        }
        if namespace_for(owner).is_none() {
            return Err(MarketError::Invalid(
                "appliance id cannot name a storage namespace",
            ));
        }
        let now = now_secs();
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        sweep_uploads(&mut next, now);
        let held = next.uploads.values().filter(|u| u.owner == owner).count();
        if held >= crate::vms::MAX_UPLOADS_PER_BUYER {
            return Err(MarketError::Conflict(
                "you already keep three images; remove one first",
            ));
        }
        let id = random_id("upl")?;
        let ticket = random_capability()?;
        next.uploads.insert(
            id.clone(),
            Upload {
                id: id.clone(),
                owner: owner.to_string(),
                name: name.to_string(),
                ticket: Some(ticket.clone()),
                token: random_capability()?,
                created_at: now,
                size: None,
                virtual_size: None,
                efi,
            },
        );
        self.commit(&mut state, next).await?;
        Ok((id, ticket))
    }

    /// The upload waiting for `ticket`, if it is still waiting.
    pub async fn upload_for_ticket(&self, ticket: &str) -> Option<Upload> {
        let now = now_secs();
        let state = self.state.lock().await;
        state
            .uploads
            .values()
            .find(|u| u.ticket.as_deref() == Some(ticket))
            .filter(|u| !u.stored() && now < u.created_at + crate::vms::UPLOAD_TICKET_SECS)
            .cloned()
    }

    /// Record that `id`'s file is on disk. Spends its ticket.
    pub async fn upload_stored(
        &self,
        id: &str,
        size: u64,
        virtual_size: u64,
    ) -> Result<(), MarketError> {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        let upload = next.uploads.get_mut(id).ok_or(MarketError::NotFound)?;
        upload.ticket = None;
        upload.size = Some(size);
        upload.virtual_size = Some(virtual_size);
        self.commit(&mut state, next).await
    }

    /// The stored upload `token` names. CDI's importer is the only caller
    /// that knows a token.
    pub async fn upload_for_token(&self, token: &str) -> Option<Upload> {
        let state = self.state.lock().await;
        state
            .uploads
            .values()
            .find(|u| u.token == token && u.stored())
            .cloned()
    }

    /// Forget one of `owner`'s uploads. Refused while an order that has not
    /// imported it yet still names it; the caller deletes the file.
    ///
    /// # Errors
    /// Not the owner's, or still needed.
    pub async fn remove_upload(&self, owner: &str, id: &str) -> Result<(), MarketError> {
        let now = now_secs();
        let mut state = self.state.lock().await;
        if !state.uploads.get(id).is_some_and(|u| u.owner == owner) {
            return Err(MarketError::NotFound);
        }
        let needed = state.orders.values().any(|o| {
            o.vm.as_ref()
                .is_some_and(|vm| vm.image == id && !vm.created)
                && (pending_live(o, now) || paid_live(o, now))
        });
        if needed {
            return Err(MarketError::Conflict(
                "a machine that is still being set up uses this image",
            ));
        }
        let mut next = state.clone();
        next.uploads.remove(id);
        self.commit(&mut state, next).await
    }

    /// Drop uploads whose ticket ran out before a file arrived. Returns their
    /// ids, so the caller can remove any partial file.
    pub async fn sweep_uploads(&self) -> Result<Vec<String>, MarketError> {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        let dropped = sweep_uploads(&mut next, now_secs());
        if !dropped.is_empty() {
            self.commit(&mut state, next).await?;
        }
        Ok(dropped)
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

    /// See [`door_volume`].
    pub async fn door_volume(&self, order_id: &str, hash: &str) -> Option<OpenDoor> {
        door_volume(&*self.state.lock().await, order_id, hash)
    }

    /// Set the backup door's key of `buyer`'s bought volume `order_id`. A new
    /// key replaces the old one: the box's recovery code changed (an erase),
    /// and the old backups, encrypted under the old code, are no longer its.
    /// The upstream password is made with the first key and kept.
    ///
    /// # Errors
    /// [`MarketError::NotFound`] for anything but the buyer's own bound
    /// storage, [`MarketError::Invalid`] for a key of the wrong shape.
    pub async fn set_door(
        &self,
        order_id: &str,
        buyer: &str,
        hash: &str,
    ) -> Result<(), MarketError> {
        if !door_hash_ok(hash) {
            return Err(MarketError::Invalid("the door key is not a SHA-256 in hex"));
        }
        let mut state = self.state.lock().await;
        if storage_volume(&state, order_id, buyer).is_none() {
            return Err(MarketError::NotFound);
        }
        if state.doors.get(order_id).is_some_and(|d| d.key == hash) {
            return Ok(());
        }
        let upstream = match state.doors.get(order_id) {
            Some(d) => d.upstream.clone(),
            None => crate::identity::random_hex(32)
                .ok_or_else(|| MarketError::Store("no randomness for the door".to_string()))?,
        };
        let mut next = state.clone();
        next.doors.insert(
            order_id.to_string(),
            Door {
                key: hash.to_string(),
                upstream,
            },
        );
        self.commit(&mut state, next).await
    }

    /// See [`storage_volume`].
    pub async fn storage_volume(&self, order_id: &str, buyer: &str) -> Option<StorageVolume> {
        storage_volume(&*self.state.lock().await, order_id, buyer)
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

    /// Record the address a seller wants USDC paid to on one family of
    /// networks. On an EVM chain the signature is a `personal_sign` of
    /// [`crate::stablecoin::payout_message`] for the edge's first EVM chain,
    /// so a signature made for a test network cannot set a live payout and a
    /// signature from another key cannot name this one; the address is the
    /// same on every EVM chain. On Solana it is the owner's ed25519
    /// signature of the same message, naming the Solana network.
    pub async fn set_payout(
        &self,
        id: &str,
        address: &str,
        signature: &str,
        family: crate::stablecoin::Family,
    ) -> Result<(), MarketError> {
        use crate::stablecoin::Family;
        let usdc = self.opts.usdc.clone().ok_or(MarketError::Unconfigured)?;
        let address = match family {
            Family::Evm => {
                let chain_id = usdc.payout_chain_id().ok_or(MarketError::Conflict(
                    "this edge does not settle on an EVM chain",
                ))?;
                let address = crate::stablecoin::normalize_address(address)
                    .ok_or(MarketError::Invalid("payout address is not an 0x address"))?;
                let message = crate::stablecoin::payout_message(chain_id, &address);
                let recovered = crate::stablecoin::recover_address(message.as_bytes(), signature)
                    .ok_or(MarketError::Invalid(
                    "the payout signature does not recover",
                ))?;
                if recovered != address {
                    return Err(MarketError::Invalid(
                        "the payout signature is not from that address",
                    ));
                }
                address
            }
            Family::Solana => {
                let network = usdc
                    .payout_solana()
                    .ok_or(MarketError::Conflict("this edge does not settle on Solana"))?;
                let address = crate::solana::normalize_address(address).ok_or(
                    MarketError::Invalid("payout address is not a Solana address"),
                )?;
                let message = crate::stablecoin::payout_message(network, &address);
                if !crate::solana::verify_message(message.as_bytes(), &address, signature) {
                    return Err(MarketError::Invalid(
                        "the payout signature is not from that address",
                    ));
                }
                address
            }
        };
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        let seller = next
            .sellers
            .entry(id.to_string())
            .or_insert_with(|| Seller {
                account_id: String::new(),
                ready: false,
                box_uuid: None,
                payout: None,
                payout_solana: None,
            });
        match family {
            Family::Evm => seller.payout = Some(address),
            Family::Solana => seller.payout_solana = Some(address),
        }
        self.commit(&mut state, next).await?;
        Ok(())
    }

    /// Set the watch-only key `id` is paid to in `coin`, after the edge's
    /// node checked it is a public key for the network this edge settles on.
    /// Setting the same key again keeps its next address index, so no
    /// address is handed out twice. Monero takes the primary address as
    /// `key` and the private view key as `view_key`. Zcash takes the unified
    /// viewing key as `key` and keeps only its Orchard incoming part; a
    /// spending key or a seed is refused by name.
    pub async fn set_coin_key(
        &self,
        id: &str,
        coin: &str,
        key: &str,
        view_key: Option<&str>,
    ) -> Result<(), MarketError> {
        let coin = crate::coins::Coin::parse(coin)
            .filter(|c| self.coins().contains(c))
            .ok_or(MarketError::Invalid("this edge does not take that coin"))?;
        let test = {
            let state = self.state.lock().await;
            Self::mode_of(&state)? == StripeMode::Test
        };
        let (descriptor, view_key, from_height) = match coin {
            crate::coins::Coin::Btc => {
                let btc = self.opts.btc.as_ref().ok_or(MarketError::Unconfigured)?;
                let node = crate::bitcoin::Node::new(btc);
                Self::btc_chain_fits(&node, test).await?;
                (node.check_key(key).await?, None, None)
            }
            crate::coins::Coin::Xmr => {
                let xmr = self.opts.xmr.as_ref().ok_or(MarketError::Unconfigured)?;
                let node = crate::monero::Node::new(xmr);
                let address = key.trim();
                let parsed = crate::monero::parse_address(address).map_err(MarketError::Invalid)?;
                let view_key = view_key
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .ok_or(MarketError::Invalid(
                        "Monero needs the wallet's private view key too",
                    ))?
                    .to_ascii_lowercase();
                crate::monero::check_view_key(&parsed, &view_key).map_err(MarketError::Invalid)?;
                let height = Self::xmr_net_fits(&node, &parsed, test).await?;
                // The same address keeps the height its wallet scans from;
                // a new one scans from a little below the tip, so nothing
                // paid while the key is set is missed.
                let from = {
                    let state = self.state.lock().await;
                    state
                        .coin_sellers
                        .get(id)
                        .and_then(|s| s.keys.get(coin.key()))
                        .filter(|k| {
                            k.descriptor == address && k.view_key.as_deref() == Some(&view_key)
                        })
                        .and_then(|k| k.from_height)
                }
                .unwrap_or(height.saturating_sub(10));
                node.prepare(&crate::monero::WatchKey {
                    address,
                    view_key: &view_key,
                    from_height: from,
                })
                .await?;
                (address.to_string(), Some(view_key), Some(from))
            }
            crate::coins::Coin::Zec => {
                let zec = self.zec.as_ref().ok_or(MarketError::Unconfigured)?;
                if view_key.is_some_and(|v| !v.trim().is_empty()) {
                    return Err(MarketError::Invalid(
                        "Zcash takes the unified viewing key alone",
                    ));
                }
                let parsed = crate::zcash::parse_key(key).map_err(MarketError::Invalid)?;
                Self::zec_net_fits(zec, &parsed, test).await?;
                (parsed.wallet_address(), Some(parsed.encode()), None)
            }
            _ => return Err(MarketError::Invalid("this edge does not take that coin")),
        };
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        next.sellers
            .entry(id.to_string())
            .or_insert_with(|| Seller {
                account_id: String::new(),
                ready: false,
                box_uuid: None,
                payout: None,
                payout_solana: None,
            });
        let keys = &mut next.coin_sellers.entry(id.to_string()).or_default().keys;
        // Monero's subaddress 0 is the primary address, which the seller
        // uses elsewhere; orders start at 1.
        let first = u32::from(coin == crate::coins::Coin::Xmr);
        let start = keys
            .get(coin.key())
            .filter(|k| k.descriptor == descriptor)
            .map_or(first, |k| k.next.max(first));
        keys.insert(
            coin.key().to_string(),
            CoinKey {
                descriptor,
                next: start,
                view_key,
                from_height,
            },
        );
        self.commit(&mut state, next).await?;
        Ok(())
    }

    /// Refuse a node on the wrong network: a test edge on mainnet would take
    /// real coins for test orders, and a live one on a test chain would hand
    /// out worthless addresses.
    async fn btc_chain_fits(
        node: &crate::bitcoin::Node<'_>,
        test: bool,
    ) -> Result<(), MarketError> {
        let chain = node.chain().await?;
        if crate::bitcoin::chain_fits(&chain, test) {
            return Ok(());
        }
        tracing::error!(
            target: Action::Market.target(),
            "the Bitcoin node follows {chain}, which a {} edge does not settle on",
            if test { "test" } else { "live" },
        );
        Err(MarketError::Conflict(
            "the edge's Bitcoin node is on the wrong network",
        ))
    }

    /// Refuse a Monero node on the wrong network for this edge, as for
    /// Bitcoin, and an address for another network than the node's. The
    /// chain's height on success.
    async fn xmr_net_fits(
        node: &crate::monero::Node<'_>,
        address: &crate::monero::Address,
        test: bool,
    ) -> Result<u64, MarketError> {
        let (nettype, height) = node.info().await?;
        if !crate::monero::nettype_fits(&nettype, test) {
            tracing::error!(
                target: Action::Market.target(),
                "the Monero node follows {nettype}, which a {} edge does not settle on",
                if test { "test" } else { "live" },
            );
            return Err(MarketError::Conflict(
                "the edge's Monero node is on the wrong network",
            ));
        }
        if !address.net.fits(&nettype) {
            return Err(MarketError::Invalid(
                "that Monero address is for another network than this edge's",
            ));
        }
        Ok(height)
    }

    /// Refuse Zcash servers on the wrong network for this edge, as for
    /// Bitcoin, and a viewing key for another network than theirs. The best
    /// block two operators have reached on success.
    async fn zec_net_fits(
        zec: &crate::zcash::Scanner,
        key: &crate::zcash::ViewingKey,
        test: bool,
    ) -> Result<u64, MarketError> {
        let network = zec.opts().network;
        if !crate::zcash::network_fits(network, test) {
            tracing::error!(
                target: Action::Market.target(),
                "the edge's Zcash servers are set to {}, which a {} edge does not settle on",
                network.chain_name(),
                if test { "test" } else { "live" },
            );
            return Err(MarketError::Conflict(
                "the edge's Zcash servers are on the wrong network",
            ));
        }
        if key.network != network.network_type() {
            return Err(MarketError::Invalid(
                "that viewing key is for another network than this edge's",
            ));
        }
        zec.tip().await
    }

    /// The edge's price for `coin` now: the cached one while fresh, else
    /// asked of both sources.
    async fn coin_price(&self, coin: crate::coins::Coin, now: u64) -> Option<u128> {
        if let Some(p) = self.prices.cached(coin, now) {
            return Some(p);
        }
        self.prices.refresh(&[coin], now).await;
        self.prices.cached(coin, now)
    }

    /// Pay `order_id` in `coin`: quote it at the price now and derive an
    /// address for it from the seller's key, which the edge's node watches.
    /// Asked again, it reads what arrived and answers the same address; a
    /// lapsed quote with nothing paid is quoted again on the same address.
    /// Switching coins is refused once anything reached the first address.
    pub async fn pay_with_coin(
        &self,
        buyer: &str,
        order_id: &str,
        coin: &str,
    ) -> Result<CoinView, MarketError> {
        use crate::coins::{Quote, Settlement};
        let coin = crate::coins::Coin::parse(coin)
            .filter(|c| self.coins().contains(c))
            .ok_or(MarketError::Invalid("this edge does not take that coin"))?;
        let now = now_secs();
        let (order, key, paused) = {
            let state = self.state.lock().await;
            let order = state
                .orders
                .get(order_id)
                .filter(|o| o.buyer == buyer)
                .cloned()
                .ok_or(MarketError::NotFound)?;
            let seller = state.coin_sellers.get(&order.seller);
            let key = seller.and_then(|s| s.keys.get(coin.key())).cloned();
            let paused = seller.is_some_and(|s| s.coins_paused(self.opts.coin_fee_limit, now));
            (order, key, paused)
        };
        if let Some(existing) = &order.coin {
            let received = !matches!(existing.settlement, None | Some(Settlement::Waiting));
            if existing.quote.coin == coin && (received || now < existing.quote.pay_before) {
                self.settle_coins_of(std::slice::from_ref(&order.id)).await;
                let state = self.state.lock().await;
                let c = state
                    .orders
                    .get(order_id)
                    .and_then(|o| o.coin.as_ref())
                    .ok_or(MarketError::NotFound)?;
                return Ok(CoinView::of(order_id, c));
            }
            if received {
                return Err(MarketError::Conflict(
                    "a payment already reached this order's address",
                ));
            }
        }
        if !pending_live(&order, now) {
            return Err(MarketError::Conflict(
                "this order is no longer held; order again",
            ));
        }
        if order.currency != "usd" {
            return Err(MarketError::Conflict(
                "coin prices are quoted in US dollars",
            ));
        }
        let key = key.ok_or(MarketError::Conflict("the seller does not take that coin"))?;
        // A payment already under way above runs its course; a new quote
        // waits until the seller has settled what coin sales owe.
        if paused {
            return Err(MarketError::Conflict(COINS_PAUSED));
        }
        let price = self
            .coin_price(coin, now)
            .await
            .ok_or(MarketError::Conflict(
                "the price sources disagree or are not answering; try again shortly",
            ))?;
        let quote = Quote::new(coin, order.amount, price, now)
            .ok_or(MarketError::Invalid("the order has no amount to quote"))?;

        // The same coin with nothing paid keeps its address; otherwise derive
        // the next one nobody else was given.
        let (address, index, issued, issued_height) = match &order.coin {
            Some(c) if c.quote.coin == coin => {
                (c.address.clone(), c.index, c.issued, c.issued_height)
            }
            _ if coin == crate::coins::Coin::Xmr => {
                let xmr = self.opts.xmr.as_ref().ok_or(MarketError::Unconfigured)?;
                let node = crate::monero::Node::new(xmr);
                let watch = key
                    .watch()
                    .ok_or(MarketError::Conflict("the seller does not take that coin"))?;
                let parsed = crate::monero::parse_address(watch.address)
                    .map_err(|_| MarketError::Conflict("the seller does not take that coin"))?;
                let test = self.mode().await == Some(StripeMode::Test);
                let height = Self::xmr_net_fits(&node, &parsed, test).await?;
                let used = self.coin_addresses().await;
                let mut index = key.next.max(1);
                let address = loop {
                    let a = node.subaddress(&watch, index).await?;
                    if !used.contains(&a) {
                        break a;
                    }
                    index = index.checked_add(1).ok_or(MarketError::Conflict(
                        "the seller's key has no addresses left",
                    ))?;
                };
                (address, index, now, Some(height))
            }
            _ if coin == crate::coins::Coin::Zec => {
                let zec = self.zec.as_ref().ok_or(MarketError::Unconfigured)?;
                let vk = key
                    .zec_key()
                    .ok_or(MarketError::Conflict("the seller does not take that coin"))?;
                let test = self.mode().await == Some(StripeMode::Test);
                let tip = Self::zec_net_fits(zec, &vk, test).await?;
                let used = self.coin_addresses().await;
                let mut index = key.next;
                let address = loop {
                    let a = vk.order_address(index);
                    if !used.contains(&a) {
                        break a;
                    }
                    index = index.checked_add(1).ok_or(MarketError::Conflict(
                        "the seller's key has no addresses left",
                    ))?;
                };
                // Payments count from the first block after the best one
                // now: anything older reached the address before it was
                // this order's.
                (address, index, now, Some(tip.saturating_add(1)))
            }
            _ => {
                let used = self.coin_addresses().await;
                let btc = self.opts.btc.as_ref().ok_or(MarketError::Unconfigured)?;
                let node = crate::bitcoin::Node::new(btc);
                let test = self.mode().await == Some(StripeMode::Test);
                Self::btc_chain_fits(&node, test).await?;
                let mut index = key.next;
                let address = loop {
                    let a = node.derive(&key.descriptor, index).await?;
                    if !used.contains(&a) {
                        break a;
                    }
                    index = index.checked_add(1).ok_or(MarketError::Conflict(
                        "the seller's key has no addresses left",
                    ))?;
                };
                node.watch(&address, order_id).await?;
                (address, index, now, None)
            }
        };

        let mut state = self.state.lock().await;
        let mut next = state.clone();
        let o = next
            .orders
            .get_mut(order_id)
            .filter(|o| o.buyer == buyer && o.status == OrderStatus::Pending)
            .ok_or(MarketError::Conflict(
                "this order is no longer held; order again",
            ))?;
        if o.coin
            .as_ref()
            .is_some_and(|c| !matches!(c.settlement, None | Some(Settlement::Waiting)))
        {
            return Err(MarketError::Conflict(
                "a payment already reached this order's address",
            ));
        }
        // Two orders paid at once can be handed the same next address; the
        // second to get here asks again for the one after.
        if state.orders.values().any(|other| {
            other.id != order_id && other.coin.as_ref().is_some_and(|c| c.address == address)
        }) {
            return Err(MarketError::Conflict(
                "another order was given that address just now; try again",
            ));
        }
        let o = next.orders.get_mut(order_id).ok_or(MarketError::NotFound)?;
        o.coin = Some(CoinPayment {
            quote,
            address,
            index,
            issued,
            issued_height,
            first_seen: BTreeMap::new(),
            settlement: Some(Settlement::Waiting),
            txs: Vec::new(),
        });
        let view = CoinView::of(order_id, o.coin.as_ref().expect("just set"));
        let seller = o.seller.clone();
        if let Some(k) = next
            .coin_sellers
            .get_mut(&seller)
            .and_then(|s| s.keys.get_mut(coin.key()))
            .filter(|k| k.descriptor == key.descriptor)
        {
            k.next = k.next.max(index.saturating_add(1));
        }
        self.commit(&mut state, next).await?;
        Ok(view)
    }

    /// Every Zcash order the edge waits on, as the scanner watches it: the
    /// seller's key, the order's address number and its issue height.
    async fn zec_watches(&self) -> Vec<crate::zcash::Watch> {
        let state = self.state.lock().await;
        let mut keys: BTreeMap<&str, Option<crate::zcash::ViewingKey>> = BTreeMap::new();
        let mut out = Vec::new();
        for o in state.orders.values() {
            let Some(c) = o.coin.as_ref() else {
                continue;
            };
            if c.quote.coin != crate::coins::Coin::Zec || o.status == OrderStatus::Paid {
                continue;
            }
            let Some(since) = c.issued_height else {
                continue;
            };
            let key = keys.entry(o.seller.as_str()).or_insert_with(|| {
                state
                    .coin_sellers
                    .get(&o.seller)
                    .and_then(|s| s.keys.get(crate::coins::Coin::Zec.key()))
                    .and_then(CoinKey::zec_key)
            });
            if let Some(key) = key {
                out.push(crate::zcash::Watch {
                    order: o.id.clone(),
                    key: key.clone(),
                    index: c.index,
                    since,
                });
            }
        }
        out
    }

    /// Every address any order was given for a coin payment.
    async fn coin_addresses(&self) -> std::collections::BTreeSet<String> {
        let state = self.state.lock().await;
        state
            .orders
            .values()
            .filter_map(|o| o.coin.as_ref().map(|c| c.address.clone()))
            .collect()
    }

    /// Read what reached every coin order's address that is not paid yet,
    /// and mark paid the ones that have their confirmations. Run on each
    /// reconcile pass.
    pub async fn settle_coins(&self) {
        let ids: Vec<String> = {
            let state = self.state.lock().await;
            state
                .orders
                .values()
                .filter(|o| o.coin.is_some() && o.status != OrderStatus::Paid)
                .map(|o| o.id.clone())
                .collect()
        };
        if !ids.is_empty() {
            self.settle_coins_of(&ids).await;
        }
    }

    async fn settle_coins_of(&self, ids: &[String]) {
        use crate::coins::{settle, Coin, Quote};
        let now = now_secs();
        // With each order, the seller's key for its coin: Monero reads
        // through the seller's own view-only wallet.
        let work: Vec<(String, CoinPayment, Option<CoinKey>)> = {
            let state = self.state.lock().await;
            ids.iter()
                .filter_map(|id| state.orders.get(id))
                .filter(|o| o.status != OrderStatus::Paid)
                .filter_map(|o| {
                    let c = o.coin.clone()?;
                    let key = state
                        .coin_sellers
                        .get(&o.seller)
                        .and_then(|s| s.keys.get(c.quote.coin.key()))
                        .cloned();
                    Some((o.id.clone(), c, key))
                })
                .collect()
        };
        let mut found = Vec::new();
        // Which chain the node follows names the network a paid order shows,
        // for the explorer link: `bitcoin` on mainnet, `bitcoin-signet` and
        // so on elsewhere.
        let mut btc_network: Option<String> = None;
        // Zcash: one scan for every order the edge waits on, whichever of
        // them were asked about, so each server's memory keeps them all.
        let zec_scan = match self.zec.as_ref() {
            Some(zec) if work.iter().any(|(_, c, _)| c.quote.coin == Coin::Zec) => {
                Some(zec.scan(self.zec_watches().await, now).await)
            }
            _ => None,
        };
        for (id, c, key) in work {
            let received = match c.quote.coin {
                // Only payments the node saw from the moment the address was
                // the order's count.
                Coin::Btc => match self.opts.btc.as_ref() {
                    Some(btc) => crate::bitcoin::Node::new(btc)
                        .received(&c.address)
                        .await
                        .map(|r| r.into_iter().filter(|r| r.first_seen >= c.issued).collect()),
                    None => Err(MarketError::Unconfigured),
                },
                // The wallet filters by block height; see `issued_height`.
                Coin::Xmr => match (
                    self.opts.xmr.as_ref(),
                    key.as_ref().and_then(CoinKey::watch),
                ) {
                    (Some(xmr), Some(watch)) => {
                        crate::monero::Node::new(xmr)
                            .received(&watch, c.index, c.issued_height.unwrap_or(0))
                            .await
                    }
                    _ => Err(MarketError::Unconfigured),
                },
                // What two operators' servers agree on, with the time the
                // edge first saw each payment kept from earlier passes.
                Coin::Zec => match &zec_scan {
                    Some(Ok(agreed)) => {
                        let since = c.issued_height.unwrap_or(0);
                        if since.saturating_add(crate::zcash::MAX_SCAN_BLOCKS)
                            < agreed.tip.saturating_add(crate::zcash::RESCAN_DEPTH)
                        {
                            // Older than a pass reads back: it keeps the
                            // state it last had.
                            continue;
                        }
                        let mut got = agreed.received.get(&id).cloned().unwrap_or_default();
                        for r in &mut got {
                            if let Some(t) = c.first_seen.get(&r.tx) {
                                r.first_seen = r.first_seen.min(*t);
                            }
                        }
                        Ok(got)
                    }
                    Some(Err(e)) => Err(e.clone()),
                    None => Err(MarketError::Unconfigured),
                },
                _ => Err(MarketError::Unconfigured),
            };
            let received: Vec<crate::coins::Received> = match received {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(
                        target: Action::Market.target(),
                        "could not read the payments of order {id}: {e}",
                    );
                    continue;
                }
            };
            let fresh = if now > c.quote.pay_before {
                match self.coin_price(c.quote.coin, now).await {
                    Some(p) => Quote::new(c.quote.coin, c.quote.usd_cents, p, now),
                    None => None,
                }
            } else {
                None
            };
            let settlement = settle(&c.quote, &received, fresh.as_ref());
            let mut first_seen = c.first_seen.clone();
            if c.quote.coin == Coin::Zec {
                for r in &received {
                    first_seen.entry(r.tx.clone()).or_insert(r.first_seen);
                }
            }
            let txs: Vec<String> = received.into_iter().map(|r| r.tx).collect();
            let network = match c.quote.coin {
                Coin::Btc if settlement.is_paid() => {
                    if btc_network.is_none() {
                        let chain = match self.opts.btc.as_ref() {
                            Some(btc) => crate::bitcoin::Node::new(btc).chain().await.ok(),
                            None => None,
                        };
                        btc_network = Some(match chain.as_deref() {
                            None | Some("main") => "bitcoin".to_string(),
                            Some(other) => format!("bitcoin-{other}"),
                        });
                    }
                    btc_network.clone()
                }
                _ => None,
            };
            found.push((id, settlement, txs, network, first_seen));
        }
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        let mut changed = false;
        for (id, settlement, txs, network, first_seen) in found {
            let Some(o) = next.orders.get_mut(&id) else {
                continue;
            };
            if o.status == OrderStatus::Paid {
                continue;
            }
            let live = pending_live(o, now);
            let Some(c) = o.coin.as_mut() else {
                continue;
            };
            if c.settlement.as_ref() == Some(&settlement)
                && c.txs == txs
                && c.first_seen == first_seen
            {
                continue;
            }
            changed = true;
            let paid = settlement.is_paid();
            c.settlement = Some(settlement);
            c.txs = txs;
            c.first_seen = first_seen;
            if !paid {
                continue;
            }
            let coin = c.quote.coin;
            if !live {
                tracing::warn!(
                    target: Action::Market.target(),
                    "order {id} was paid in {} after its hold lapsed; its units are still free, so it is fulfilled",
                    coin.ticker(),
                );
            }
            o.status = OrderStatus::Paid;
            o.tx_hash = c.txs.first().cloned();
            o.paid_network = network.or_else(|| Some(coin.key().to_string()));
            o.paid_at = Some(now);
            o.expires_at = Some(now + ENTITLEMENT_SECS);
            let (seller, fee) = (o.seller.clone(), o.fee);
            next.coin_sellers
                .entry(seller)
                .or_default()
                .charge(fee, now);
            tracing::info!(
                target: Action::Market.target(),
                "order {id} is paid in {}",
                coin.ticker(),
            );
        }
        if changed {
            if let Err(e) = self.commit(&mut state, next).await {
                tracing::error!(
                    target: Action::Market.target(),
                    "could not record coin payments: {e}",
                );
            }
        }
    }

    /// The network a request names, or the edge's first when it names none,
    /// which is what a box built before there were several sends.
    fn usdc_chain(
        usdc: &crate::stablecoin::UsdcOpts,
        network: Option<&str>,
    ) -> Result<crate::stablecoin::UsdcChain, MarketError> {
        match network {
            Some(key) => usdc.chain(key).cloned().ok_or(MarketError::Invalid(
                "this edge does not settle on that network",
            )),
            None => usdc
                .chains
                .first()
                .cloned()
                .ok_or(MarketError::Unconfigured),
        }
    }

    /// Mark a USDC order paid when `tx` paid it on `network`. On an EVM chain
    /// that is a `LososEscrow.pay` holding the order's amount for the seller
    /// the invoice named, fetched after checking the endpoint is on that
    /// chain and the escrow is the one the config names and takes orders from
    /// this edge's key, with [`crate::stablecoin::CONFIRMATIONS`] blocks. The
    /// deposit is then queued for release, or for a refund when the seller's
    /// address cannot receive USDC there, the order was already paid, or it
    /// lapsed and its units are sold; [`Market::settle_escrow`] sends either.
    /// On Solana it is a transaction at the `confirmed` commitment, on the
    /// configured cluster, that names the order in a memo and moved the
    /// order's USDC to the seller and the treasury. A transaction already
    /// used for another order is refused, so a payment cannot be replayed.
    pub async fn confirm_usdc(
        &self,
        buyer: &str,
        order_id: &str,
        tx: &str,
        network: Option<&str>,
    ) -> Result<(), MarketError> {
        use crate::stablecoin::Family;
        let usdc = self.opts.usdc.clone().ok_or(MarketError::Unconfigured)?;
        let chain = Self::usdc_chain(&usdc, network)?;
        let tx = match chain.network.family {
            Family::Evm => crate::stablecoin::normalize_tx(tx),
            Family::Solana => crate::solana::normalize_signature(tx),
        }
        .ok_or(MarketError::Invalid("tx is not a transaction hash"))?;
        let expected = {
            let state = self.state.lock().await;
            if let Some(seen) = Self::usdc_seen(&state, buyer, order_id, &tx) {
                return seen;
            }
            self.usdc_expected(&state, buyer, order_id, &tx, &chain)?.0
        };
        let url = crate::stablecoin::read_rpc_url(std::path::Path::new(&chain.rpc_file)).await?;
        let misconfigured = |e: &MarketError| {
            tracing::error!(
                target: Action::Market.target(),
                "USDC settlement on {} is misconfigured: {e}",
                chain.network.key,
            );
        };
        let mut held = None;
        match &expected {
            Expected::Evm(expected) => {
                let releaser = crate::evm_tx::Releaser::read(std::path::Path::new(
                    usdc.releaser_file
                        .as_deref()
                        .ok_or(MarketError::Unconfigured)?,
                ))
                .await
                .inspect_err(misconfigured)?;
                crate::stablecoin::check_endpoint(&url, &chain, releaser.address())
                    .await
                    .inspect_err(misconfigured)?;
                let receipt = crate::stablecoin::fetch_receipt(&url, &tx).await?;
                let deposit = crate::stablecoin::payment_matches(&receipt, expected)
                    .map_err(MarketError::Conflict)?;
                let head = crate::stablecoin::block_number(&url).await?;
                crate::stablecoin::confirmed(&receipt, &head).map_err(MarketError::Conflict)?;
                let seller = crate::stablecoin::check_seller(&url, &chain, expected.seller)
                    .await?
                    .inspect_err(|why| {
                        tracing::warn!(
                            target: Action::Market.target(),
                            "order {order_id} was paid on {} to a seller who cannot receive it ({why}); refunding",
                            chain.network.key,
                        );
                    })
                    .is_ok();
                held = Some(HeldAt {
                    deposit,
                    account: None,
                    deadline: None,
                    seller_ok: seller,
                });
            }
            Expected::SolanaEscrow(expected) => {
                let releaser = crate::evm_tx::Releaser::read(std::path::Path::new(
                    usdc.releaser_file
                        .as_deref()
                        .ok_or(MarketError::Unconfigured)?,
                ))
                .await
                .inspect_err(misconfigured)?;
                let releaser = crate::solana_escrow::SolanaReleaser::from_releaser(&releaser);
                let genesis = chain
                    .genesis
                    .as_deref()
                    .or(chain.network.genesis)
                    .ok_or(MarketError::Unconfigured)?;
                crate::solana::check_genesis(&url, genesis)
                    .await
                    .inspect_err(misconfigured)?;
                crate::solana_escrow::check_config(
                    &url,
                    &expected.program,
                    &chain.treasury,
                    chain.network.usdc,
                    &releaser.address(),
                )
                .await
                .inspect_err(misconfigured)?;
                // A test network may run a program still being worked on.
                if !chain.network.test {
                    crate::solana_escrow::check_final(&url, &expected.program)
                        .await
                        .inspect_err(misconfigured)?;
                }
                let found = crate::solana::fetch_transaction(&url, &tx).await?;
                let account = crate::solana_escrow::deposit_of(&found, &expected.program)
                    .map_err(MarketError::Conflict)?;
                let deposit = crate::solana_escrow::read_deposit(&url, &expected.program, &account)
                    .await?
                    .ok_or(MarketError::Conflict(
                        "the escrow holds no such deposit; it may have been paid out already",
                    ))?;
                crate::solana_escrow::deposit_matches(&deposit, expected)
                    .map_err(MarketError::Conflict)?;
                held = Some(HeldAt {
                    deposit: 0,
                    account: Some(account),
                    deadline: Some(expected.deadline),
                    seller_ok: true,
                });
            }
            Expected::Solana(expected) => {
                let genesis = chain
                    .genesis
                    .as_deref()
                    .or(chain.network.genesis)
                    .ok_or(MarketError::Unconfigured)?;
                crate::solana::check_genesis(&url, genesis)
                    .await
                    .inspect_err(misconfigured)?;
                let found = crate::solana::fetch_transaction(&url, &tx).await?;
                crate::solana::transfer_matches(&found, expected).map_err(MarketError::Conflict)?;
            }
        }
        let mut state = self.state.lock().await;
        if let Some(seen) = Self::usdc_seen(&state, buyer, order_id, &tx) {
            return seen;
        }
        let (_, stray) = self.usdc_expected(&state, buyer, order_id, &tx, &chain)?;
        let mut next = state.clone();
        let now = now_secs();
        // A deposit the order takes is held until its slot is over
        // (`crate::slots`); one it cannot take goes back at once.
        let escrow = |at: &HeldAt, reason: Option<RefundReason>| Escrow {
            network: chain.network.key.to_string(),
            deposit: at.deposit,
            account: at.account.clone(),
            deadline: at.deadline,
            step: if reason.is_some() {
                EscrowStep::Refund
            } else {
                EscrowStep::Held
            },
            reason,
            tx: None,
            sent_at: None,
            refund: None,
        };
        let offline_base = next
            .presence
            .get(&seller_of(&next, order_id))
            .map_or(0, |p| p.offline);
        if let (Some(reason), Some(at)) = (stray, &held) {
            tracing::warn!(
                target: Action::Market.target(),
                "order {order_id} cannot take the payment in {tx} ({reason:?}); refunding it",
            );
            next.strays.insert(
                tx,
                StrayPayment {
                    order_id: order_id.to_string(),
                    buyer: buyer.to_string(),
                    received_at: now,
                    escrow: escrow(at, Some(reason)),
                },
            );
            self.commit(&mut state, next).await?;
            return Err(MarketError::Conflict(reason.message()));
        }
        let order = next.orders.get_mut(order_id).ok_or(MarketError::NotFound)?;
        order.tx_hash = Some(tx);
        order.paid_network = Some(chain.network.key.to_string());
        if let Some(at) = held.as_ref().filter(|at| !at.seller_ok) {
            let reason = RefundReason::SellerCannotReceive;
            order.status = OrderStatus::Expired;
            order.escrow = Some(escrow(at, Some(reason)));
            self.commit(&mut state, next).await?;
            return Err(MarketError::Conflict(reason.message()));
        }
        if !pending_live(order, now) {
            tracing::warn!(
                target: Action::Market.target(),
                "order {order_id} was paid in USDC after its hold lapsed; its units are still free, so it is fulfilled",
            );
        }
        order.status = OrderStatus::Paid;
        order.paid_at = Some(now);
        order.expires_at = Some(now + ENTITLEMENT_SECS);
        order.escrow = held.as_ref().map(|at| escrow(at, None));
        order.offline_base = offline_base;
        // A Solana transfer paid the treasury its share as it was confirmed;
        // an escrowed one does so when the edge releases it.
        if held.is_none() {
            collect_offset(&mut next, order_id);
        }
        self.commit(&mut state, next).await?;
        Ok(())
    }

    /// The answer an earlier report of the same payment got, so a box that
    /// reports it twice hears the same thing.
    fn usdc_seen(
        state: &MarketState,
        buyer: &str,
        order_id: &str,
        tx: &str,
    ) -> Option<Result<(), MarketError>> {
        if let Some(stray) = state.strays.get(tx) {
            if stray.order_id == order_id && stray.buyer == buyer {
                let reason = stray.escrow.reason.unwrap_or(RefundReason::AlreadyPaid);
                return Some(Err(MarketError::Conflict(reason.message())));
            }
            return None;
        }
        let order = state
            .orders
            .get(order_id)
            .filter(|o| o.buyer == buyer && o.tx_hash.as_deref() == Some(tx))?;
        if order.status == OrderStatus::Paid {
            return Some(Ok(()));
        }
        let reason = order.escrow.as_ref().and_then(|e| e.reason)?;
        Some(Err(MarketError::Conflict(reason.message())))
    }

    /// Count `tx` on `network` as `seller` paying the platform fees its coin
    /// sales owe: a mined USDC transfer from the seller's EVM payout address
    /// to this edge's treasury on that network, with
    /// [`crate::stablecoin::CONFIRMATIONS`] blocks, read over the edge's own
    /// endpoint after checking it is on that chain. The payout address is
    /// the sender because the seller signed for it: a transfer anyone could
    /// see on chain cannot be claimed by another seller. A transaction is
    /// counted once, and no more of it than is owed; the rest is not
    /// refunded.
    pub async fn pay_coin_fees(
        &self,
        seller: &str,
        tx: &str,
        network: Option<&str>,
    ) -> Result<FeePaymentView, MarketError> {
        use crate::stablecoin::{address_bytes, Family};
        let usdc = self.opts.usdc.clone().ok_or(MarketError::Unconfigured)?;
        let chain = Self::usdc_chain(&usdc, network)?;
        if chain.network.family != Family::Evm {
            return Err(MarketError::Invalid(
                "coin fees are paid in USDC on an EVM network",
            ));
        }
        let tx = crate::stablecoin::normalize_tx(tx)
            .ok_or(MarketError::Invalid("tx is not a transaction hash"))?;
        let from = {
            let state = self.state.lock().await;
            Self::fee_tx_unused(&state, &tx)?;
            if state.coin_sellers.get(seller).map_or(0, |s| s.fees_owed) == 0 {
                return Err(MarketError::Conflict(
                    "no platform fees are owed on coin sales",
                ));
            }
            state
                .sellers
                .get(seller)
                .and_then(|s| s.payout.as_deref())
                .and_then(address_bytes)
                .ok_or(MarketError::Conflict(
                    "set a USDC payout address first: the fees are paid from it",
                ))?
        };
        let token = address_bytes(chain.network.usdc).ok_or(MarketError::Unconfigured)?;
        let treasury = address_bytes(&chain.treasury).ok_or(MarketError::Unconfigured)?;
        let url = crate::stablecoin::read_rpc_url(Path::new(&chain.rpc_file)).await?;
        crate::stablecoin::check_chain_id(&url, &chain)
            .await
            .inspect_err(|e| {
                tracing::error!(
                    target: Action::Market.target(),
                    "USDC settlement on {} is misconfigured: {e}",
                    chain.network.key,
                );
            })?;
        let receipt = crate::stablecoin::fetch_receipt(&url, &tx).await?;
        let paid = crate::stablecoin::fee_transfer(&receipt, token, treasury, from)
            .map_err(MarketError::Conflict)?;
        let head = crate::stablecoin::block_number(&url).await?;
        crate::stablecoin::confirmed(&receipt, &head).map_err(MarketError::Conflict)?;
        // USDC has six decimals and the market counts cents; a fraction of a
        // cent is not counted.
        let paid = u64::try_from(paid / 10_000).unwrap_or(u64::MAX);

        let mut state = self.state.lock().await;
        Self::fee_tx_unused(&state, &tx)?;
        let mut next = state.clone();
        let owing = next.coin_sellers.entry(seller.to_string()).or_default();
        let credited = owing.settle(paid, false);
        let owed = owing.fees_owed;
        next.fee_payments.insert(
            tx.clone(),
            FeePayment {
                seller: seller.to_string(),
                network: chain.network.key.to_string(),
                paid,
                credited,
                received_at: now_secs(),
            },
        );
        self.commit(&mut state, next).await?;
        tracing::info!(
            target: Action::Market.target(),
            "{seller} paid {credited} of its coin fees in {tx} on {}; {owed} still owed",
            chain.network.key,
        );
        Ok(FeePaymentView {
            paid,
            credited,
            owed,
        })
    }

    /// Refuse a transaction already counted toward coin fees, or one that
    /// paid an order.
    fn fee_tx_unused(state: &MarketState, tx: &str) -> Result<(), MarketError> {
        if state.fee_payments.contains_key(tx) {
            return Err(MarketError::Conflict(
                "that transaction was already counted",
            ));
        }
        if state.strays.contains_key(tx)
            || state
                .orders
                .values()
                .any(|o| o.tx_hash.as_deref() == Some(tx))
        {
            return Err(MarketError::Conflict("that transaction paid an order"));
        }
        Ok(())
    }

    /// Whether this edge settles on an EVM network, so it needs a releaser.
    #[must_use]
    pub fn wants_releaser(&self) -> bool {
        self.opts
            .usdc
            .as_ref()
            .is_some_and(|u| u.releaser_file.is_some())
    }

    /// Whether a Solana network holds payments in the escrow program, so the
    /// releaser key has a Solana address too.
    pub fn solana_escrowed(&self) -> bool {
        self.opts
            .usdc
            .as_ref()
            .is_some_and(crate::stablecoin::UsdcOpts::solana_escrowed)
    }

    /// The releaser key as the operator needs to see it: its addresses, and
    /// on each network with an escrow the gas it holds and whether the
    /// escrow there takes orders from it. The key itself never leaves this process.
    pub async fn releaser_report(&self) -> Result<ReleaserReport, MarketError> {
        let usdc = self.opts.usdc.as_ref().ok_or(MarketError::Unconfigured)?;
        let file = usdc
            .releaser_file
            .as_deref()
            .ok_or(MarketError::Unconfigured)?;
        let releaser = crate::evm_tx::Releaser::read(Path::new(file)).await.ok();
        let solana = releaser
            .as_ref()
            .map(crate::solana_escrow::SolanaReleaser::from_releaser);
        let mut networks = Vec::new();
        for chain in usdc.chains.iter().filter(|c| c.pay.is_some()) {
            let mut row = ReleaserNetwork {
                network: chain.network.key,
                escrow: chain.pay.clone().unwrap_or_default(),
                balance_wei: None,
                balance_lamports: None,
                escrow_agrees: None,
                error: None,
            };
            match crate::stablecoin::read_rpc_url(Path::new(&chain.rpc_file)).await {
                Err(_) => row.error = Some("the network's endpoint is not sealed yet"),
                Ok(url) if chain.network.family == crate::stablecoin::Family::Solana => {
                    if let Some(r) = &solana {
                        match crate::solana_escrow::releaser_on_chain(
                            &url,
                            &row.escrow,
                            &chain.treasury,
                            chain.network.usdc,
                            &r.address(),
                        )
                        .await
                        {
                            Ok((lamports, agrees)) => {
                                row.balance_lamports = Some(lamports.to_string());
                                row.escrow_agrees = agrees;
                                if !chain.network.test {
                                    if let Err(MarketError::Conflict(why)) =
                                        crate::solana_escrow::check_final(&url, &row.escrow).await
                                    {
                                        row.error = Some(why);
                                    }
                                }
                            }
                            Err(MarketError::Conflict(why) | MarketError::Invalid(why)) => {
                                row.error = Some(why);
                            }
                            Err(_) => row.error = Some("the chain endpoint did not answer"),
                        }
                    }
                }
                Ok(url) => {
                    if let Some(r) = &releaser {
                        match crate::stablecoin::releaser_on_chain(&url, &row.escrow, r.address())
                            .await
                        {
                            Ok((balance, agrees)) => {
                                row.balance_wei = Some(balance.to_string());
                                row.escrow_agrees = Some(agrees);
                            }
                            Err(MarketError::Conflict(why) | MarketError::Invalid(why)) => {
                                row.error = Some(why);
                            }
                            Err(_) => row.error = Some("the chain endpoint did not answer"),
                        }
                    }
                }
            }
            networks.push(row);
        }
        Ok(ReleaserReport {
            address: releaser.map(|r| r.address_hex()),
            solana_address: solana
                .filter(|_| usdc.solana_escrowed())
                .map(|r| r.address_base58()),
            networks,
        })
    }

    /// Send the escrow calls confirmed payments are waiting on, and follow
    /// the ones already sent until they are mined. The escrow's own record
    /// of a deposit is the truth: a call that reverted, was dropped or sat
    /// unmined for [`ESCROW_RESEND_SECS`] is checked against it, and sent
    /// again only while the deposit is still held. A failure on one deposit
    /// is logged and retried on the next pass.
    pub async fn settle_escrow(&self) {
        let Some(usdc) = self.opts.usdc.as_ref() else {
            return;
        };
        let jobs: Vec<(EscrowJob, Escrow)> = {
            let state = self.state.lock().await;
            // A release waits until what was bought exists. One already sent
            // is followed to the end whatever happens to the order meanwhile.
            let orders = state.orders.values().filter_map(|o| {
                o.escrow
                    .as_ref()
                    .filter(|e| e.pending())
                    .filter(|e| e.step != EscrowStep::Release || e.tx.is_some() || delivered(o))
                    .map(|e| (EscrowJob::Order(o.id.clone()), e.clone()))
            });
            let strays = state
                .strays
                .iter()
                .filter(|(_, s)| s.escrow.pending())
                .map(|(tx, s)| (EscrowJob::Stray(tx.clone()), s.escrow.clone()));
            orders.chain(strays).collect()
        };
        if jobs.is_empty() {
            return;
        }
        let _one_pass = self.escrow_lock.lock().await;
        let releaser = match usdc.releaser_file.as_deref() {
            Some(file) => crate::evm_tx::Releaser::read(std::path::Path::new(file)).await,
            None => Err(MarketError::Unconfigured),
        };
        let releaser = match releaser {
            Ok(releaser) => releaser,
            Err(e) => {
                tracing::error!(
                    target: Action::Market.target(),
                    "{} escrowed payments are waiting, but the releaser key cannot be read: {e}",
                    jobs.len(),
                );
                return;
            }
        };
        let mut moved = Vec::new();
        for (job, escrow) in jobs {
            match self.escrow_step(usdc, &releaser, &escrow).await {
                Ok(Some(after)) => moved.push((job, escrow, after)),
                Ok(None) => {}
                Err(e) => tracing::warn!(
                    target: Action::Market.target(),
                    "escrow deposit {} on {}: {e}",
                    escrow
                        .account
                        .clone()
                        .unwrap_or_else(|| escrow.deposit.to_string()),
                    escrow.network,
                ),
            }
        }
        if moved.is_empty() {
            return;
        }
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        let now = now_secs();
        for (job, before, mut after) in moved {
            match job {
                EscrowJob::Order(id) => {
                    let Some(order) = next.orders.get_mut(&id) else {
                        continue;
                    };
                    if order.escrow.as_ref() != Some(&before) {
                        continue;
                    }
                    let reclaimed = matches!(before.step, EscrowStep::Release | EscrowStep::Split)
                        && after.step == EscrowStep::Refunded;
                    if reclaimed {
                        tracing::warn!(
                            target: Action::Market.target(),
                            "order {id}: the buyer took the payment back before the edge released it; the order lapses",
                        );
                        after.reason = Some(RefundReason::BuyerReclaimed);
                        order.expires_at = Some(order.expires_at.map_or(now, |at| at.min(now)));
                    }
                    let released =
                        before.step == EscrowStep::Release && after.step == EscrowStep::Released;
                    let settled =
                        before.step == EscrowStep::Split && after.step == EscrowStep::Settled;
                    let refund = after.refund.unwrap_or(0);
                    order.escrow = Some(after);
                    // The treasury has the fee, offset included, only now.
                    if released {
                        collect_offset(&mut next, &id);
                    } else if settled {
                        collect_offset_share(&mut next, &id, refund);
                    }
                }
                EscrowJob::Stray(tx) => {
                    let Some(stray) = next.strays.get_mut(&tx) else {
                        continue;
                    };
                    if stray.escrow != before {
                        continue;
                    }
                    stray.escrow = after;
                }
            }
        }
        if let Err(e) = self.commit(&mut state, next).await {
            tracing::error!(
                target: Action::Market.target(),
                "could not record escrow progress: {e}",
            );
        }
    }

    /// The sellers whose boxes the slots pass needs to hear about.
    pub async fn watched_sellers(&self) -> BTreeSet<String> {
        let state = self.state.lock().await;
        crate::slots::watched(&state, now_secs())
    }

    /// Run the slot rules once (`crate::slots`): count offline time against
    /// the watched sellers not in `online`, refund the buyers of a seller
    /// offline too long, refund slots never set up, and pay out the slots
    /// that are over. `can_tell` is false when the edge could not have heard
    /// the boxes; such a pass counts no offline time. Money moves in the
    /// escrow and card passes that follow.
    pub async fn slots_pass(&self, online: &BTreeSet<String>, can_tell: bool, max_gap: u64) {
        use std::sync::atomic::Ordering;
        let now = now_secs();
        let mut state = self.state.lock().await;
        if state.mode.is_none() {
            return;
        }
        let mut next = state.clone();
        let observed = crate::slots::observe(&mut next, online, now, can_tell, max_gap);
        let offline = crate::slots::refund_offline(&mut next, now, self.opts.offline_refund_secs);
        let undelivered = crate::slots::refund_undelivered(&mut next, now);
        let released = crate::slots::release_due(&mut next, now);
        for (id, refund) in &offline {
            tracing::warn!(
                target: Action::Market.target(),
                "order {id}: the seller's box was offline too long; refunding {refund} of it to the buyer",
            );
        }
        for id in &undelivered {
            tracing::warn!(
                target: Action::Market.target(),
                "order {id} was not set up within a day of payment; refunding the buyer",
            );
        }
        for id in &released {
            tracing::info!(target: Action::Market.target(), "order {id}: slot over; paying the seller");
        }
        let moved = !offline.is_empty() || !undelivered.is_empty() || !released.is_empty();
        let due = now
            >= self
                .presence_saved
                .load(Ordering::Relaxed)
                .saturating_add(PRESENCE_SAVE_SECS);
        if !(moved || observed.turned || due) {
            // Only the watch moved on: keep it in memory until the next write.
            state.presence = next.presence;
            state.presence_tick = next.presence_tick;
            return;
        }
        match self.commit(&mut state, next).await {
            Ok(()) => self.presence_saved.store(now, Ordering::Relaxed),
            Err(e) => tracing::error!(
                target: Action::Market.target(),
                "could not record the slots pass: {e}",
            ),
        }
    }

    /// `POST /market/sales/release`: the seller asks to be paid for every
    /// sale whose slot is over. Returns what goes out.
    ///
    /// # Errors
    /// [`MarketError::Conflict`] while nothing is ready: a slot still runs,
    /// or there is nothing held.
    pub async fn release_sales(
        &self,
        seller: &str,
    ) -> Result<crate::slots::PayoutView, MarketError> {
        let now = now_secs();
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        let (ids, _) = crate::slots::release_for(&mut next, seller, now);
        if ids.is_empty() {
            return Err(MarketError::Conflict(NOTHING_TO_PAY_OUT));
        }
        self.commit(&mut state, next).await?;
        Ok(crate::slots::payout_view(&state, seller, now))
    }

    /// Settle the card payments the slot rules moved on: pay the seller,
    /// refund the buyer, or split it ([`Connect::settle`]). A failure waits
    /// [`CARD_RETRY_SECS`] before it is tried again.
    pub async fn settle_cards(&self) {
        let now = now_secs();
        let jobs: Vec<(String, Card, u64)> = {
            let state = self.state.lock().await;
            state
                .orders
                .values()
                .filter_map(|o| {
                    let c = o.card.as_ref()?;
                    let refund = match c.step {
                        EscrowStep::Release => 0,
                        EscrowStep::Refund => o.amount,
                        EscrowStep::Split => c.refund?,
                        _ => return None,
                    };
                    let waiting = c
                        .tried_at
                        .is_some_and(|t| now < t.saturating_add(CARD_RETRY_SECS));
                    (!waiting).then(|| (o.id.clone(), c.clone(), refund))
                })
                .collect()
        };
        if jobs.is_empty() {
            return;
        }
        let _one_pass = self.card_lock.lock().await;
        let mut outcomes = Vec::new();
        for (id, card, refund) in jobs {
            let result = self.connect.settle(&card.payment_intent, refund).await;
            if let Err(e) = &result {
                tracing::warn!(target: Action::Market.target(), "order {id}: card settle: {e}");
            }
            outcomes.push((id, card, result.is_ok()));
        }
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        for (id, before, ok) in outcomes {
            let Some(order) = next.orders.get_mut(&id) else {
                continue;
            };
            let Some(card) = order.card.as_mut().filter(|c| **c == before) else {
                continue;
            };
            if ok {
                card.step = match card.step {
                    EscrowStep::Release => EscrowStep::Released,
                    EscrowStep::Refund => EscrowStep::Refunded,
                    _ => EscrowStep::Settled,
                };
                card.tried_at = None;
            } else {
                card.tried_at = Some(now);
            }
        }
        if let Err(e) = self.commit(&mut state, next).await {
            tracing::error!(
                target: Action::Market.target(),
                "could not record card settlements: {e}",
            );
        }
    }

    /// One deposit's next move: `Some` with its new record, `None` while
    /// there is nothing to do but wait.
    async fn escrow_step(
        &self,
        usdc: &crate::stablecoin::UsdcOpts,
        releaser: &crate::evm_tx::Releaser,
        escrow: &Escrow,
    ) -> Result<Option<Escrow>, MarketError> {
        use crate::stablecoin::DepositState;
        let chain = Self::usdc_chain(usdc, Some(&escrow.network))?;
        if chain.network.family == crate::stablecoin::Family::Solana {
            let releaser = crate::solana_escrow::SolanaReleaser::from_releaser(releaser);
            return self.solana_escrow_step(&chain, &releaser, escrow).await;
        }
        let chain_id = chain.network.chain_id.ok_or(MarketError::Unconfigured)?;
        let pay = chain.pay.as_deref().ok_or(MarketError::Unconfigured)?;
        let to = crate::stablecoin::address_bytes(pay).ok_or(MarketError::Unconfigured)?;
        let url = crate::stablecoin::read_rpc_url(std::path::Path::new(&chain.rpc_file)).await?;
        let now = now_secs();
        let done = |step| Escrow {
            step,
            tx: None,
            sent_at: None,
            ..escrow.clone()
        };
        let finished = match escrow.step {
            EscrowStep::Release => EscrowStep::Released,
            EscrowStep::Refund => EscrowStep::Refunded,
            EscrowStep::Split => EscrowStep::Settled,
            EscrowStep::Held
            | EscrowStep::Released
            | EscrowStep::Refunded
            | EscrowStep::Settled => return Ok(None),
        };
        if let Some(tx) = &escrow.tx {
            match crate::stablecoin::fetch_receipt(&url, tx).await {
                Ok(receipt) if receipt["status"].as_str() == Some("0x1") => {
                    let head = crate::stablecoin::block_number(&url).await?;
                    if crate::stablecoin::confirmed(&receipt, &head).is_err() {
                        return Ok(None);
                    }
                    return Ok(Some(done(finished)));
                }
                // Reverted: the deposit says why.
                Ok(_) => {}
                Err(_) if now < escrow.sent_at.unwrap_or(0) + ESCROW_RESEND_SECS => {
                    return Ok(None);
                }
                // Dropped, or stuck behind a low fee.
                Err(_) => {}
            }
        }
        match crate::stablecoin::deposit_state(&url, pay, escrow.deposit).await? {
            DepositState::Held => {
                let data = match escrow.step {
                    EscrowStep::Release => crate::evm_tx::release_data(escrow.deposit),
                    EscrowStep::Split => crate::evm_tx::settle_data(
                        escrow.deposit,
                        crate::stablecoin::cents_to_usdc(escrow.refund.unwrap_or(0)),
                    ),
                    _ => crate::evm_tx::refund_data(escrow.deposit),
                };
                let replace = escrow.tx.is_some();
                let hash = crate::evm_tx::send(&url, chain_id, releaser, to, data, replace).await?;
                tracing::info!(
                    target: Action::Market.target(),
                    "escrow deposit {} on {}: sent {:?} as {hash}",
                    escrow.deposit,
                    escrow.network,
                    escrow.step,
                );
                Ok(Some(Escrow {
                    tx: Some(hash),
                    sent_at: Some(now),
                    ..escrow.clone()
                }))
            }
            DepositState::Released => Ok(Some(done(EscrowStep::Released))),
            DepositState::Refunded => Ok(Some(done(EscrowStep::Refunded))),
            DepositState::Split => Ok(Some(done(EscrowStep::Settled))),
            DepositState::None => Err(MarketError::Conflict(
                "the escrow has no such deposit; the payment may have been reorganised away",
            )),
        }
    }

    /// [`Market::escrow_step`] on Solana. The program closes a deposit when
    /// it is released, refunded or split, so the edge's own transaction is
    /// the first truth and the deposit the second: one still there is still
    /// held, and one gone before its deadline was finished by this edge,
    /// the only key that can before then. One gone after it, by a
    /// transaction that is not the edge's, was taken back by the payer.
    async fn solana_escrow_step(
        &self,
        chain: &crate::stablecoin::UsdcChain,
        releaser: &crate::solana_escrow::SolanaReleaser,
        escrow: &Escrow,
    ) -> Result<Option<Escrow>, MarketError> {
        use crate::solana_escrow::{Landed, Move};
        let program = chain.pay.as_deref().ok_or(MarketError::Unconfigured)?;
        let account = escrow
            .account
            .as_deref()
            .ok_or(MarketError::Conflict("the escrow record names no deposit"))?;
        let url = crate::stablecoin::read_rpc_url(std::path::Path::new(&chain.rpc_file)).await?;
        let now = now_secs();
        let done = |step| Escrow {
            step,
            tx: None,
            sent_at: None,
            ..escrow.clone()
        };
        let (finished, step) = match escrow.step {
            EscrowStep::Release => (EscrowStep::Released, Move::Release),
            EscrowStep::Refund => (EscrowStep::Refunded, Move::Refund),
            EscrowStep::Split => (
                EscrowStep::Settled,
                Move::Split(
                    u64::try_from(crate::stablecoin::cents_to_usdc(escrow.refund.unwrap_or(0)))
                        .map_err(|_| MarketError::Conflict("the refund is too large"))?,
                ),
            ),
            EscrowStep::Held
            | EscrowStep::Released
            | EscrowStep::Refunded
            | EscrowStep::Settled => return Ok(None),
        };
        if let Some(tx) = &escrow.tx {
            match crate::solana_escrow::status(&url, tx).await? {
                Landed::Done => return Ok(Some(done(finished))),
                // Refused: the deposit says why.
                Landed::Failed => {}
                Landed::Unknown if now < escrow.sent_at.unwrap_or(0) + SOLANA_RESEND_SECS => {
                    return Ok(None);
                }
                // Its blockhash has expired, so it can no longer land.
                Landed::Unknown => {}
            }
        }
        let held = crate::solana_escrow::read_deposit(&url, program, account).await?;
        let Some(held) = held else {
            let before_deadline = escrow.deadline.is_some_and(|d| now < d);
            return Ok(Some(done(if before_deadline {
                finished
            } else {
                EscrowStep::Refunded
            })));
        };
        let parties = crate::solana_escrow::Parties {
            program: crate::solana_escrow::key(program).ok_or(MarketError::Unconfigured)?,
            mint: crate::solana_escrow::key(chain.network.usdc).ok_or(MarketError::Unconfigured)?,
            treasury: crate::solana_escrow::key(&chain.treasury)
                .ok_or(MarketError::Unconfigured)?,
            deposit: crate::solana_escrow::key(account)
                .ok_or(MarketError::Conflict("the escrow record names no deposit"))?,
            held,
        };
        let signature = crate::solana_escrow::send(&url, releaser, &parties, step).await?;
        tracing::info!(
            target: Action::Market.target(),
            "escrow deposit {account} on {}: sent {:?} as {signature}",
            escrow.network,
            escrow.step,
        );
        Ok(Some(Escrow {
            tx: Some(signature),
            sent_at: Some(now),
            ..escrow.clone()
        }))
    }

    /// Forward one allowlisted chain call to `network`, or to the ENS
    /// endpoint for `"ens"`, where [`crate::stablecoin::ENS_CCIP_METHOD`]
    /// goes to ENS's DNS gateway instead. The URL stays in the credential
    /// file; the caller gets the JSON-RPC object and nothing else.
    pub async fn forward_rpc(
        &self,
        network: Option<&str>,
        method: &str,
        params: &serde_json::Value,
        id: &serde_json::Value,
    ) -> Result<serde_json::Value, MarketError> {
        use crate::stablecoin::{Family, Relay};
        let usdc = self.opts.usdc.as_ref().ok_or(MarketError::Unconfigured)?;
        if network == Some("ens") {
            let file = usdc
                .ens_rpc_file
                .as_deref()
                .ok_or(MarketError::Unconfigured)?;
            if method == crate::stablecoin::ENS_CCIP_METHOD {
                return crate::stablecoin::forward_ccip(
                    crate::stablecoin::ENS_DNS_GATEWAY,
                    params,
                    id,
                )
                .await;
            }
            let url = crate::stablecoin::read_rpc_url(std::path::Path::new(file)).await?;
            if method != "eth_chainId" {
                self.check_ens_once(&url, usdc.test()).await?;
            }
            return crate::stablecoin::forward_rpc(&url, Relay::Ens, method, params, id).await;
        }
        let chain = Self::usdc_chain(usdc, network)?;
        let url = crate::stablecoin::read_rpc_url(std::path::Path::new(&chain.rpc_file)).await?;
        let relay = match chain.network.family {
            Family::Evm => Relay::Evm,
            Family::Solana => Relay::Solana,
        };
        crate::stablecoin::forward_rpc(&url, relay, method, params, id).await
    }

    /// Check the ENS endpoint's chain the first time a lookup goes through
    /// it, rather than on every lookup.
    async fn check_ens_once(&self, url: &str, test: bool) -> Result<(), MarketError> {
        use std::sync::atomic::Ordering;
        if self.ens_checked.load(Ordering::Relaxed) {
            return Ok(());
        }
        crate::stablecoin::check_ens_endpoint(url, test)
            .await
            .inspect_err(|e| {
                tracing::error!(
                    target: Action::Market.target(),
                    "ENS lookups are misconfigured: {e}",
                );
            })?;
        self.ens_checked.store(true, Ordering::Relaxed);
        Ok(())
    }

    /// The payment a USDC order must show on `chain`, or why it cannot be
    /// confirmed. On an EVM chain the payment sits in the escrow, so one
    /// the order cannot take (it was already paid, or it lapsed and its
    /// units are sold) is still matched, and comes with the reason it goes
    /// back to the payer.
    fn usdc_expected(
        &self,
        state: &MarketState,
        buyer: &str,
        order_id: &str,
        tx: &str,
        chain: &crate::stablecoin::UsdcChain,
    ) -> Result<(Expected, Option<RefundReason>), MarketError> {
        use crate::stablecoin::Family;
        if state
            .orders
            .values()
            .any(|o| o.id != order_id && o.tx_hash.as_deref() == Some(tx))
            || state.strays.get(tx).is_some_and(|s| s.order_id != order_id)
        {
            return Err(MarketError::Conflict(
                "that transaction already paid another order",
            ));
        }
        let order = state
            .orders
            .get(order_id)
            .filter(|o| o.buyer == buyer)
            .ok_or(MarketError::NotFound)?;
        if order.rail != Rail::Usdc {
            return Err(MarketError::Conflict("this order is not a USDC order"));
        }
        let escrowed = chain.pay.is_some();
        let now = now_secs();
        let mut stray = None;
        match order.status {
            OrderStatus::Pending if pending_live(order, now) => {}
            // The buyer's transaction can land after the hold: the wallet was
            // slow, the box went offline before it reported the hash. The
            // order is honoured while its units are still free, the same rule
            // a late Stripe webhook gets. A refunded order stays refunded.
            OrderStatus::Pending | OrderStatus::Expired => {
                let room = state
                    .listings
                    .get(&order.listing_id)
                    .map_or(0, |l| available(state, l, now));
                let refunded = order.escrow.as_ref().map(|e| e.reason);
                if refunded.is_some() || order.quantity > room {
                    if escrowed {
                        stray = Some(refunded.flatten().unwrap_or(RefundReason::Lapsed));
                    } else {
                        tracing::error!(
                            target: Action::Market.target(),
                            "order {order_id} was paid in USDC by {tx} after its hold lapsed and its units are sold; the buyer is owed a refund",
                        );
                        return Err(MarketError::Conflict(
                            "the order lapsed before it was paid and its units are sold",
                        ));
                    }
                }
            }
            OrderStatus::Paid if escrowed => stray = Some(RefundReason::AlreadyPaid),
            OrderStatus::Paid => {
                return Err(MarketError::Conflict("the order is not awaiting payment"));
            }
        }
        let no_payout = MarketError::Conflict("the seller has no payout address on that network");
        let seller_now = state.sellers.get(&order.seller);
        match chain.network.family {
            Family::Evm => {
                // An order made before the edge kept `pay_to` pays whatever
                // the seller had set; one made after pays what it invoiced.
                let seller = order
                    .pay_to
                    .as_deref()
                    .or_else(|| seller_now.and_then(|s| s.payout.as_deref()))
                    .and_then(crate::stablecoin::address_bytes)
                    .ok_or(no_payout)?;
                let pay = chain
                    .pay
                    .as_deref()
                    .and_then(crate::stablecoin::address_bytes)
                    .ok_or(MarketError::Unconfigured)?;
                Ok((
                    Expected::Evm(crate::stablecoin::ExpectedPayment {
                        pay,
                        seller,
                        order_id_hash: crate::stablecoin::order_id_hash(order_id),
                        gross: crate::stablecoin::cents_to_usdc(order.amount),
                        fee: crate::stablecoin::cents_to_usdc(order.fee + order.fee_offset),
                        deadline: usdc_deadline(order),
                    }),
                    stray,
                ))
            }
            Family::Solana => {
                // Solana came with `pay_to_solana`, so every order that can
                // be paid there kept its address.
                let seller = order.pay_to_solana.clone().ok_or(no_payout)?;
                let gross = crate::stablecoin::cents_to_usdc(order.amount);
                let fee = crate::stablecoin::cents_to_usdc(order.fee + order.fee_offset);
                if chain.pay.is_some() {
                    let expected = crate::solana_escrow::ExpectedDeposit::new(
                        chain,
                        &seller,
                        crate::stablecoin::order_id_hash(order_id),
                        gross,
                        fee,
                        usdc_deadline(order),
                    )
                    .ok_or(MarketError::Unconfigured)?;
                    return Ok((Expected::SolanaEscrow(expected), stray));
                }
                Ok((
                    Expected::Solana(crate::solana::ExpectedTransfer {
                        mint: chain.network.usdc.to_string(),
                        seller,
                        treasury: chain.treasury.clone(),
                        seller_amount: gross - fee,
                        fee,
                        memo: crate::stablecoin::order_id_hash_hex(order_id),
                    }),
                    stray,
                ))
            }
        }
    }

    /// Handle a Stripe webhook delivery. `signature` is the raw header.
    pub async fn webhook(&self, signature: &str, body: &[u8]) -> Result<(), MarketError> {
        // The gate holds the signing secrets, so it does the check.
        // It also refuses an event from the other mode, so `mode` is the
        // event's own when it says one.
        let mode = self.connect.verify_webhook(signature, body).await?;
        let event: Value = serde_json::from_slice(body)
            .map_err(|_| MarketError::Invalid("webhook body is not JSON"))?;
        self.use_mode(mode).await?;
        let mut state = self.state.lock().await;
        if state.mode != Some(mode) {
            return Err(MarketError::Conflict("the edge's Stripe mode changed"));
        }
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
            vm: None,
            rail: Rail::Stripe,
            tx_hash: None,
            pay_to: None,
            pay_to_solana: None,
            paid_network: None,
            coin: None,
            escrow: None,
            card: None,
            fee_offset: 0,
            offset_collected: false,
            offline_base: 0,
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
    fn a_redelivered_event_is_applied_once() {
        let mut st = MarketState::default();
        st.sellers.insert(
            "s".into(),
            Seller {
                account_id: "acct_x".into(),
                ready: false,
                box_uuid: None,
                payout: None,
                payout_solana: None,
            },
        );
        let event = serde_json::json!({
            "id": "evt_1",
            "type": "account.updated",
            "data": { "object": {
                "id": "acct_x",
                "details_submitted": true,
                "payouts_enabled": true,
                "capabilities": { "transfers": "active" },
            }},
        });
        assert!(apply_event(&mut st, &event, 1_000));
        assert!(st.sync.applied("evt_1"));
        // Something else took the seller off the shelf since; a second
        // delivery of the old event must not put it back.
        st.sellers.get_mut("s").unwrap().ready = false;
        assert!(!apply_event(&mut st, &event, 2_000));
        assert!(!st.sellers["s"].ready);
    }

    #[test]
    fn an_event_that_changes_nothing_is_not_remembered() {
        let mut st = MarketState::default();
        let event = serde_json::json!({ "id": "evt_9", "type": "charge.refunded" });
        assert!(!apply_event(&mut st, &event, 1_000));
        assert!(st.sync.is_empty());
    }

    #[test]
    fn an_account_event_for_no_seller_is_not_tracked() {
        let mut st = MarketState::default();
        let event = serde_json::json!({
            "id": "evt_2",
            "created": 5,
            "type": "account.updated",
            "data": { "object": { "id": "acct_stranger" }},
        });
        assert!(!apply_event(&mut st, &event, 1_000));
        assert!(st.sync.is_empty());
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
    fn only_the_buyer_reaches_a_bound_storage_volume() {
        let mut bound = paid("ord_a", Kind::Storage, 1, "b", Some(100));
        bound.volume = Some("market-b/ord-a".to_string());
        let mut lapsed = paid("ord_old", Kind::Storage, 1, "b", Some(5));
        lapsed.volume = Some("market-b/ord-old".to_string());
        let mut vm = paid("ord_vm", Kind::Compute, 1, "b", Some(100));
        vm.volume = Some("market-b/ord-vm".to_string());
        let st = state_of(vec![
            bound,
            lapsed,
            vm,
            paid("ord_todo", Kind::Storage, 2, "b", Some(100)),
        ]);
        assert_eq!(
            storage_volume(&st, "ord_a", "b"),
            Some(StorageVolume {
                namespace: "market-b".to_string(),
                pvc: "ord-a".to_string(),
            })
        );
        // Someone else's order, or another buyer asking, is nothing.
        assert_eq!(storage_volume(&st, "ord_a", "other"), None);
        assert_eq!(storage_volume(&st, "ord_missing", "b"), None);
        // Not bound yet, or not storage.
        assert_eq!(storage_volume(&st, "ord_todo", "b"), None);
        assert_eq!(storage_volume(&st, "ord_vm", "b"), None);
        // A lapsed order's data stays reachable while its claim exists.
        assert!(storage_volume(&st, "ord_old", "b").is_some());
    }

    #[test]
    fn a_door_opens_only_with_its_own_key() {
        let mut bound = paid("ord_a", Kind::Storage, 1, "b", Some(100));
        bound.volume = Some("market-b/ord-a".to_string());
        let mut st = state_of(vec![bound]);
        let key = "ab".repeat(32);
        assert_eq!(door_volume(&st, "ord_a", &key), None, "no key set yet");
        st.doors.insert(
            "ord_a".to_string(),
            Door {
                key: key.clone(),
                upstream: "cd".repeat(32),
            },
        );
        let open = door_volume(&st, "ord_a", &key).expect("the right key opens it");
        assert_eq!(open.volume.pvc, "ord-a");
        assert_eq!(open.upstream, "cd".repeat(32));
        assert_eq!(door_volume(&st, "ord_a", &"ac".repeat(32)), None);
        assert_eq!(door_volume(&st, "ord_b", &key), None);
        assert!(door_hash_ok(&key));
        assert!(!door_hash_ok(&"AB".repeat(32)));
        assert!(!door_hash_ok("ab"));
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
            host_vms: share,
        };
        let sharing = Sharing::from_windows(&BTreeMap::from([
            ("on".to_string(), w(true)),
            ("off".to_string(), w(false)),
        ]));
        assert!(sharing.allows("on", Kind::Storage) && sharing.allows("on", Kind::Compute));
        assert!(sharing.allows("off", Kind::Storage) && !sharing.allows("off", Kind::Compute));
        assert!(!sharing.allows("absent", Kind::Storage));
        // Hosting machines is its own opt-in, sent only by a box whose shared
        // storage is on.
        assert!(sharing.allows("on", Kind::Vm) && !sharing.allows("off", Kind::Vm));

        // The operator's opt-in narrows it: a revoked seller sells nothing.
        let sharing = sharing.only_sellers(|seller| seller == "off");
        assert!(!sharing.allows("on", Kind::Storage) && !sharing.allows("on", Kind::Compute));
        assert!(!sharing.allows("on", Kind::Vm));
        assert!(sharing.allows("off", Kind::Storage));
    }

    fn vm_order(id: &str, qty: u64, buyer: &str, expires_at: Option<u64>) -> Order {
        let mut o = paid(id, Kind::Vm, qty, buyer, expires_at);
        o.fee = fee_for(o.amount, VM_FEE_BPS);
        o.vm = Some(VmOrder {
            image: "debian-13".to_string(),
            name: "web".to_string(),
            user_data: None,
            created: false,
            halted: false,
            service_ip: None,
        });
        o
    }

    fn vm_opts() -> crate::vms::VmOpts {
        crate::vms::VmOpts {
            catalogue: None,
            shape: crate::vms::Shape::default(),
            storage_class: None,
            upload_dir: "/nonexistent".to_string(),
            max_upload_bytes: 1 << 30,
            fetch_base: "https://register.example".to_string(),
            domain: Some("vm.example".to_string()),
        }
    }

    #[test]
    fn a_machine_is_split_half_and_half_whatever_the_operator_set() {
        assert_eq!(fee_bps_for(Kind::Vm, 400), 5_000);
        assert_eq!(fee_bps_for(Kind::Storage, 400), 400);
        let q = quote(799, 3, fee_bps_for(Kind::Vm, DEFAULT_FEE_BPS)).unwrap();
        assert_eq!((q.amount, q.fee, q.seller_net), (2397, 1199, 1198));
    }

    #[test]
    fn a_paid_machine_order_is_created_once_and_its_replicas_hold_the_host() {
        let catalogue = crate::vms::Catalogue::load(None).unwrap();
        let opts = vm_opts();
        let mut st = state_of(vec![vm_order(
            "ord_0123456789abcdef",
            3,
            "buyer-box",
            Some(100),
        )]);
        st.listings
            .insert("lst_1".to_string(), listing("lst_1", true));
        let resolve =
            |o: &Order, vm: &VmOrder| opts.resolve(&catalogue, &st.uploads, &o.buyer, &vm.image);
        let todo = pending_vms(&st, 10, resolve);
        assert_eq!(todo.len(), 1);
        assert_eq!(todo[0].namespace, "market-buyer-box");
        assert_eq!(todo[0].host, "s");
        assert_eq!(todo[0].replicas, 3);
        assert!(matches!(todo[0].source, crate::vms::Source::Http(_)));
        // While paid it holds three of the listing's units.
        assert_eq!(reserved(&st, "lst_1", 10), 3);

        let vm = st
            .orders
            .get_mut("ord_0123456789abcdef")
            .unwrap()
            .vm
            .as_mut()
            .unwrap();
        vm.created = true;
        vm.service_ip = Some("10.43.0.9".to_string());
        assert!(pending_vms(&st, 10, |_, _| None).is_empty());
        // Lapsed but still running: held, and due to be halted.
        assert_eq!(reserved(&st, "lst_1", 200), 3);
        assert_eq!(
            lapsed_vms(&st, 200),
            vec![(
                "ord_0123456789abcdef".to_string(),
                "market-buyer-box".to_string(),
                3
            )]
        );
        // Halted: the host gets its replicas back.
        st.orders
            .get_mut("ord_0123456789abcdef")
            .unwrap()
            .vm
            .as_mut()
            .unwrap()
            .halted = true;
        assert_eq!(reserved(&st, "lst_1", 200), 0);
        assert!(lapsed_vms(&st, 200).is_empty());
        assert_eq!(entitlements(&st, "buyer-box", 10).vm_replicas, 3);
        assert_eq!(entitlements(&st, "buyer-box", 200).vm_replicas, 0);
    }

    #[test]
    fn an_order_for_an_upload_imports_it_from_the_edge() {
        let catalogue = crate::vms::Catalogue::load(None).unwrap();
        let opts = vm_opts();
        let mut st = state_of(vec![]);
        let upload = |owner: &str, stored: bool| Upload {
            id: "upl_1".to_string(),
            owner: owner.to_string(),
            name: "mine".to_string(),
            ticket: None,
            token: "ab".repeat(32),
            created_at: 0,
            size: stored.then_some(1),
            virtual_size: stored.then_some(40 * 1024 * 1024 * 1024),
            efi: true,
        };
        st.uploads
            .insert("upl_1".to_string(), upload("buyer-box", true));
        let r = opts
            .resolve(&catalogue, &st.uploads, "buyer-box", "upl_1")
            .unwrap();
        assert_eq!(
            r.source,
            crate::vms::Source::Http(format!(
                "https://register.example/market/vm-images/fetch/{}",
                "ab".repeat(32)
            ))
        );
        assert_eq!(r.disk_gib, 40);
        assert!(r.efi);
        // Nobody else's upload, and nothing not yet stored.
        assert!(opts
            .resolve(&catalogue, &st.uploads, "other", "upl_1")
            .is_none());
        st.uploads
            .insert("upl_1".to_string(), upload("buyer-box", false));
        assert!(opts
            .resolve(&catalogue, &st.uploads, "buyer-box", "upl_1")
            .is_none());
        // An unstored upload goes once its ticket lapses; a stored one stays.
        assert!(sweep_uploads(&mut st, 10).is_empty());
        assert_eq!(
            sweep_uploads(&mut st, crate::vms::UPLOAD_TICKET_SECS),
            ["upl_1"]
        );
        st.uploads
            .insert("upl_2".to_string(), upload("buyer-box", true));
        assert!(sweep_uploads(&mut st, u64::MAX / 2).is_empty());
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
                payout: None,
                payout_solana: None,
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
            storage_image: None,
            hardware_catalogue: None,
            vms: None,
            usdc: None,
            btc: None,
            xmr: None,
            cards: true,
            zec: None,
            coin_fee_limit: DEFAULT_COIN_FEE_LIMIT,
            offline_refund_secs: crate::slots::DEFAULT_OFFLINE_SECS,
            direct_pay: true,
        })
        .await
        .unwrap();
        market.use_mode(StripeMode::Live).await.unwrap();
        // Every write now fails: the directory it renames into is gone.
        std::fs::remove_dir_all(&dir).unwrap();

        assert!(market.close_listing("s", "lst_1").await.is_err());
        assert!(market.set_ready("s", true, None).await.is_err());
        assert!(market.mark_claimed("p", "market-b", "p").await.is_err());
        assert!(market.mark_provisioned("p", "market-b", "p").await.is_err());
        st.mode = Some(StripeMode::Live);
        assert_eq!(market.state.lock().await.state, st);
    }

    fn opts_at(file: &Path) -> MarketOpts {
        MarketOpts {
            state_file: file.to_string_lossy().into_owned(),
            gate_socket: "/nonexistent".into(),
            return_url: "https://example.test/market".into(),
            currency: "eur".into(),
            fee_bps: DEFAULT_FEE_BPS,
            storage_class: DEFAULT_STORAGE_CLASS.into(),
            storage_image: None,
            hardware_catalogue: None,
            vms: None,
            usdc: None,
            btc: None,
            xmr: None,
            cards: true,
            zec: None,
            coin_fee_limit: DEFAULT_COIN_FEE_LIMIT,
            offline_refund_secs: crate::slots::DEFAULT_OFFLINE_SECS,
            direct_pay: true,
        }
    }

    #[test]
    fn each_mode_has_its_own_ledger_file() {
        let file = Path::new("/var/lib/losos-registrar/market.json");
        assert_eq!(ledger_path(file, StripeMode::Live), file);
        assert_eq!(
            ledger_path(file, StripeMode::Test),
            Path::new("/var/lib/losos-registrar/market-test.json")
        );
        assert_eq!(
            ledger_path(Path::new("/m/market"), StripeMode::Test),
            Path::new("/m/market-test")
        );
    }

    /// Nothing is written before the gate has said which mode its key is in,
    /// so a write can never land in the wrong ledger.
    #[tokio::test]
    async fn nothing_is_written_before_the_mode_is_known() {
        let dir = std::env::temp_dir().join(format!("market-nomode-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("market.json");
        let market = Market::open(opts_at(&file)).await.unwrap();
        assert_eq!(market.mode().await, None);
        assert!(matches!(
            market.expire_stale().await,
            Ok(ref v) if v.is_empty()
        ));
        assert!(matches!(
            market.browse(&Sharing::default()).await,
            Err(MarketError::Unconfigured)
        ));
        assert!(!file.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Going live opens the live ledger and leaves the test one on disk;
    /// coming back to test finds it as it was.
    #[tokio::test]
    async fn switching_the_mode_switches_the_ledger() {
        let dir = std::env::temp_dir().join(format!("market-switch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("market.json");
        let mut test = MarketState {
            mode: Some(StripeMode::Test),
            ..MarketState::default()
        };
        test.listings.insert("lst_1".into(), listing("lst_1", true));
        std::fs::write(
            ledger_path(&file, StripeMode::Test),
            serde_json::to_vec(&test).unwrap(),
        )
        .unwrap();
        let market = Market::open(opts_at(&file)).await.unwrap();

        market.use_mode(StripeMode::Test).await.unwrap();
        assert_eq!(market.mode().await, Some(StripeMode::Test));
        assert!(market.state.lock().await.listings.contains_key("lst_1"));

        market.use_mode(StripeMode::Live).await.unwrap();
        assert_eq!(market.mode().await, Some(StripeMode::Live));
        assert!(market.state.lock().await.listings.is_empty());

        market.use_mode(StripeMode::Test).await.unwrap();
        assert!(market.state.lock().await.listings.contains_key("lst_1"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Only the buyer sets a door's key, only on its own bound storage, and
    /// the key survives a restart.
    #[tokio::test]
    async fn the_buyer_alone_sets_a_door_key() {
        let dir = std::env::temp_dir().join(format!("market-door-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("market.json");
        let mut bound = paid("ord_a", Kind::Storage, 1, "b", Some(u64::MAX));
        bound.volume = Some("market-b/ord-a".to_string());
        let mut todo = paid("ord_t", Kind::Storage, 1, "b", Some(u64::MAX));
        todo.volume = None;
        let mut test = state_of(vec![bound, todo]);
        test.mode = Some(StripeMode::Test);
        std::fs::write(
            ledger_path(&file, StripeMode::Test),
            serde_json::to_vec(&test).unwrap(),
        )
        .unwrap();
        let market = Market::open(opts_at(&file)).await.unwrap();
        market.use_mode(StripeMode::Test).await.unwrap();
        let key = "0f".repeat(32);

        assert!(matches!(
            market.set_door("ord_a", "other", &key).await,
            Err(MarketError::NotFound)
        ));
        assert!(matches!(
            market.set_door("ord_t", "b", &key).await,
            Err(MarketError::NotFound)
        ));
        assert!(matches!(
            market.set_door("ord_a", "b", "not-hex").await,
            Err(MarketError::Invalid(_))
        ));
        market.set_door("ord_a", "b", &key).await.unwrap();
        let upstream = market.door_volume("ord_a", &key).await.unwrap().upstream;
        assert!(door_hash_ok(&upstream), "{upstream}");
        assert_ne!(upstream, key);

        let again = Market::open(opts_at(&file)).await.unwrap();
        again.use_mode(StripeMode::Test).await.unwrap();
        assert!(again.door_volume("ord_a", &key).await.is_some());
        let other = "1e".repeat(32);
        again.set_door("ord_a", "b", &other).await.unwrap();
        assert!(again.door_volume("ord_a", &key).await.is_none());
        // A new key keeps the server's password: the server keeps running.
        assert_eq!(
            again.door_volume("ord_a", &other).await.unwrap().upstream,
            upstream
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A ledger stamped with the other mode is a mix-up, not an empty market.
    #[tokio::test]
    async fn a_ledger_of_the_other_mode_is_refused() {
        let dir = std::env::temp_dir().join(format!("market-stamp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("market.json");
        let test = MarketState {
            mode: Some(StripeMode::Test),
            ..MarketState::default()
        };
        std::fs::write(&file, serde_json::to_vec(&test).unwrap()).unwrap();
        assert!(matches!(
            Market::open(opts_at(&file)).await,
            Err(MarketError::Store(_))
        ));
        std::fs::remove_dir_all(&dir).unwrap();
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
        let view = history(st.orders.values(), now, None);
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
                payout: None,
                payout_solana: None,
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

    fn owing(fees_owed: u64, owed_since: Option<u64>) -> CoinSeller {
        CoinSeller {
            fees_owed,
            owed_since,
            ..CoinSeller::default()
        }
    }

    fn usdc_order(id: &str, amount: u64, fee_offset: u64, created_at: u64) -> Order {
        let mut o = order(id, OrderStatus::Pending, 1, created_at);
        o.rail = Rail::Usdc;
        o.amount = amount;
        o.fee = fee_for(amount, DEFAULT_FEE_BPS);
        o.fee_offset = fee_offset;
        o
    }

    #[test]
    fn a_usdc_sale_takes_what_is_owed_up_to_half_the_gross() {
        let now = 1_000;
        let mut state = MarketState::default();
        assert_eq!(fee_offset_for(&state, "s", 200, 8, now), 0);
        state.coin_sellers.insert("s".into(), owing(50, Some(now)));
        assert_eq!(fee_offset_for(&state, "s", 200, 8, now), 50);
        // The escrow refuses a fee over half the gross: 100 - 8.
        state
            .coin_sellers
            .insert("s".into(), owing(5_000, Some(now)));
        assert_eq!(fee_offset_for(&state, "s", 200, 8, now), 92);
        // A machine's fee is already half: nothing on top.
        assert_eq!(fee_offset_for(&state, "s", 200, 100, now), 0);
        // What rides on another order in flight is not taken twice.
        state.coin_sellers.insert("s".into(), owing(120, Some(now)));
        state
            .orders
            .insert("ord_a".into(), usdc_order("ord_a", 200, 92, now));
        assert_eq!(fee_offset_for(&state, "s", 200, 8, now), 28);
        // A lapsed order holds nothing back; a paid one awaiting release does.
        let late = now + USDC_HOLD_SECS + 1;
        assert_eq!(fee_offset_for(&state, "s", 200, 8, late), 92);
        let a = state.orders.get_mut("ord_a").unwrap();
        a.status = OrderStatus::Paid;
        a.escrow = Some(Escrow {
            network: "base".into(),
            deposit: 1,
            account: None,
            deadline: None,
            step: EscrowStep::Held,
            reason: None,
            refund: None,
            tx: None,
            sent_at: None,
        });
        // Held for its slot, then on its way: still riding on that order.
        for step in [EscrowStep::Held, EscrowStep::Release, EscrowStep::Split] {
            let a = state.orders.get_mut("ord_a").unwrap();
            a.escrow.as_mut().unwrap().step = step;
            assert_eq!(fee_offset_for(&state, "s", 200, 8, late), 28, "{step:?}");
        }
        // Refunded whole, nothing of it reached the treasury.
        let a = state.orders.get_mut("ord_a").unwrap();
        a.escrow.as_mut().unwrap().step = EscrowStep::Refunded;
        assert_eq!(fee_offset_for(&state, "s", 200, 8, late), 92);
    }

    #[test]
    fn a_split_collects_the_offset_share_the_seller_kept() {
        let mut state = MarketState::default();
        state.coin_sellers.insert("s".into(), owing(90, Some(7)));
        state
            .orders
            .insert("ord_a".into(), usdc_order("ord_a", 300, 90, 1));
        // A third went back to the buyer: two thirds of the offset arrived.
        assert_eq!(collect_offset_share(&mut state, "ord_a", 100), 60);
        assert_eq!(state.coin_sellers["s"].fees_owed, 30);
        // Once only, and a whole refund collects nothing.
        assert_eq!(collect_offset_share(&mut state, "ord_a", 100), 0);
        state
            .orders
            .insert("ord_b".into(), usdc_order("ord_b", 300, 30, 1));
        assert_eq!(collect_offset_share(&mut state, "ord_b", 300), 0);
        assert_eq!(state.coin_sellers["s"].fees_owed, 30);
    }

    #[test]
    fn an_offset_is_collected_once_and_what_is_left_over_is_credit() {
        let mut state = MarketState::default();
        state.coin_sellers.insert("s".into(), owing(50, Some(7)));
        state
            .orders
            .insert("ord_a".into(), usdc_order("ord_a", 200, 50, 0));
        state
            .orders
            .insert("ord_b".into(), usdc_order("ord_b", 200, 30, 0));
        assert_eq!(collect_offset(&mut state, "ord_a"), 50);
        assert_eq!(collect_offset(&mut state, "ord_a"), 0);
        let s = &state.coin_sellers["s"];
        assert_eq!((s.fees_owed, s.owed_since, s.fee_credit), (0, None, 0));
        // Settled meanwhile: the second offset becomes credit, and pays the
        // next coin fee before anything is owed again.
        assert_eq!(collect_offset(&mut state, "ord_b"), 30);
        let s = state.coin_sellers.get_mut("s").unwrap();
        assert_eq!((s.fees_owed, s.fee_credit), (0, 30));
        s.charge(20, 100);
        assert_eq!((s.fees_owed, s.owed_since, s.fee_credit), (0, None, 10));
        s.charge(25, 200);
        assert_eq!(
            (s.fees_owed, s.owed_since, s.fee_credit),
            (15, Some(200), 0)
        );
        // A later fee does not restart the clock.
        s.charge(5, 300);
        assert_eq!((s.fees_owed, s.owed_since), (20, Some(200)));
    }

    #[test]
    fn coins_stop_over_the_limit_or_after_thirty_days_owed() {
        let now = 100 * 24 * 3600;
        assert!(!owing(0, None).coins_paused(1_000, now));
        assert!(!owing(1_000, Some(now)).coins_paused(1_000, now));
        assert!(owing(1_001, Some(now)).coins_paused(1_000, now));
        let month = Some(now - COIN_FEE_GRACE_SECS);
        assert!(owing(1, month).coins_paused(1_000, now));
        assert!(!owing(1, Some(now - COIN_FEE_GRACE_SECS + 1)).coins_paused(1_000, now));
        // Paid off, the clock stops; a direct payment counts no surplus.
        let mut s = owing(300, month);
        assert_eq!(s.settle(500, false), 300);
        assert_eq!((s.fees_owed, s.owed_since, s.fee_credit), (0, None, 0));
        assert!(!s.coins_paused(1_000, now));
    }

    #[test]
    fn an_old_ledger_loads_and_its_debts_start_counting_when_read() {
        let old = serde_json::json!({
            "coin_sellers": { "s": { "keys": {}, "fees_owed": 40 }, "t": { "fees_owed": 0 } },
            "orders": { "ord_1": {
                "id": "ord_1", "listing_id": "lst_1", "buyer": "b", "seller": "s",
                "kind": "storage", "quantity": 1, "unit_price": 100, "amount": 100,
                "fee": 4, "currency": "usd", "status": "paid", "session_id": null,
                "created_at": 1, "paid_at": 2,
            }},
        });
        let mut state: MarketState = serde_json::from_value(old).unwrap();
        assert_eq!(state.orders["ord_1"].fee_offset, 0);
        assert!(state.fee_payments.is_empty());
        stamp_debts(&mut state, 500);
        assert_eq!(state.coin_sellers["s"].owed_since, Some(500));
        assert_eq!(state.coin_sellers["t"].owed_since, None);
        stamp_debts(&mut state, 900);
        assert_eq!(state.coin_sellers["s"].owed_since, Some(500));
        // The view's share for the seller is net of both.
        let mut o = usdc_order("ord_2", 200, 50, 0);
        o.status = OrderStatus::Paid;
        let view = OrderView::of(&o, 0, None);
        assert_eq!((view.fee, view.fee_offset, view.seller_net), (8, 50, 142));
    }
}
