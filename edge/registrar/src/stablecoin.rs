//! USDC settlement for the market, beside Stripe.
//!
//! On an EVM chain a buyer's wallet calls `pay` on
//! [`LososEscrow`](https://github.com/dasmatus/losos/blob/main/edge/contracts/src/LososEscrow.sol),
//! which holds the payment as a deposit, and this module checks the receipt's
//! `Held` log. Before the order counts, the edge checks that the seller's
//! payout address can receive USDC on that chain; then it releases the
//! deposit to the treasury and that seller, or refunds the payer
//! ([`crate::evm_tx`] sends both). The edge's key can do nothing else with a
//! deposit: the contract fixes where each one may go. On Solana the same
//! holds with the `losos-escrow` program ([`crate::solana_escrow`]). A
//! Solana network without the program is paid straight to the seller: the
//! wallet makes the two transfers itself with the order in a memo, and
//! [`crate::solana`] checks the balances it moved. Only a direct-pay edge
//! offers that.
//!
//! The secrets are the RPC URLs, because a provider key usually sits in
//! them, and the escrow's releaser key. systemd-creds seals one URL per chain
//! (`usdc-rpc-<network>`) and the key once (`usdc-releaser-key`); the
//! registrar reads the decrypted files when it needs them and never logs
//! them. The browser talks to the box, and the box talks to the registrar, so
//! neither reaches a page.
//!
//! Prices stay in cents, the market's minor unit. USDC has six decimals on
//! every chain here, so one cent is 10_000 base units. Each network is test
//! or live, the same split the Stripe key's prefix makes: the chain is the
//! mode, and a setting cannot disagree with it. An edge settles on one or
//! more networks of the same mode; the buyer picks one per order.

use std::path::Path;
use std::time::Duration;

#[cfg(test)]
use k256::ecdsa::SigningKey;
use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use serde_json::{json, Value};
use sha3::{Digest, Keccak256};

use crate::identity::to_hex;
use crate::market::MarketError;

/// Base mainnet. Live ledger, real USDC.
pub const BASE: u64 = 8453;
/// Base Sepolia. Test ledger, test USDC.
pub const BASE_SEPOLIA: u64 = 84532;
pub const ARBITRUM: u64 = 42161;
pub const ARBITRUM_SEPOLIA: u64 = 421_614;
pub const OPTIMISM: u64 = 10;
pub const OPTIMISM_SEPOLIA: u64 = 11_155_420;
/// Circle's USDC on Base.
pub const USDC_BASE: &str = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913";
/// Circle's USDC on Base Sepolia.
pub const USDC_BASE_SEPOLIA: &str = "0x036cbd53842c5426634e7929541ec2318f3dcf7e";
/// Circle's USDC mint on Solana devnet.
pub const USDC_SOLANA_DEVNET: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";
/// Ethereum mainnet and Sepolia, where ENS names live. Only names are
/// looked up there; nothing is paid on them.
pub const ETHEREUM: u64 = 1;
pub const SEPOLIA: u64 = 11_155_111;

/// How a network is paid and checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    /// `LososEscrow.pay` on an EVM chain; the receipt's `Held` log is the
    /// proof.
    Evm,
    /// `pay` on the `losos-escrow` program, whose deposit account is the
    /// proof; or, without the program, two `TransferChecked` and a memo,
    /// whose token balances are.
    Solana,
}

/// A chain this market can settle on, with Circle's USDC on it. The token is
/// in this table, not in the config, so a config cannot point the market at
/// a lookalike token.
#[derive(Debug, PartialEq, Eq)]
pub struct Network {
    /// What the config, the wire and the credential name use.
    pub key: &'static str,
    pub name: &'static str,
    pub family: Family,
    /// EVM chain id. `None` on Solana.
    pub chain_id: Option<u64>,
    pub test: bool,
    /// Circle's USDC: a lowercase `0x` address, or the SPL mint.
    pub usdc: &'static str,
    /// Solana's genesis hash, which is what tells devnet from mainnet.
    pub genesis: Option<&'static str>,
}

pub const NETWORKS: &[Network] = &[
    Network {
        key: "base",
        name: "Base",
        family: Family::Evm,
        chain_id: Some(BASE),
        test: false,
        usdc: USDC_BASE,
        genesis: None,
    },
    Network {
        key: "base-sepolia",
        name: "Base Sepolia",
        family: Family::Evm,
        chain_id: Some(BASE_SEPOLIA),
        test: true,
        usdc: USDC_BASE_SEPOLIA,
        genesis: None,
    },
    Network {
        key: "arbitrum",
        name: "Arbitrum One",
        family: Family::Evm,
        chain_id: Some(ARBITRUM),
        test: false,
        usdc: "0xaf88d065e77c8cc2239327c5edb3a432268e5831",
        genesis: None,
    },
    Network {
        key: "arbitrum-sepolia",
        name: "Arbitrum Sepolia",
        family: Family::Evm,
        chain_id: Some(ARBITRUM_SEPOLIA),
        test: true,
        usdc: "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d",
        genesis: None,
    },
    Network {
        key: "optimism",
        name: "OP Mainnet",
        family: Family::Evm,
        chain_id: Some(OPTIMISM),
        test: false,
        usdc: "0x0b2c639c533813f4aa9d7837caf62653d097ff85",
        genesis: None,
    },
    Network {
        key: "optimism-sepolia",
        name: "OP Sepolia",
        family: Family::Evm,
        chain_id: Some(OPTIMISM_SEPOLIA),
        test: true,
        usdc: "0x5fd84259d66cd46123540766be93dfe6d43130d7",
        genesis: None,
    },
    Network {
        key: "solana",
        name: "Solana",
        family: Family::Solana,
        chain_id: None,
        test: false,
        usdc: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        genesis: Some("5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"),
    },
    Network {
        key: "solana-devnet",
        name: "Solana Devnet",
        family: Family::Solana,
        chain_id: None,
        test: true,
        usdc: USDC_SOLANA_DEVNET,
        genesis: Some("EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"),
    },
];

/// The network a config or a request names.
#[must_use]
pub fn network(key: &str) -> Option<&'static Network> {
    NETWORKS.iter().find(|n| n.key == key)
}

/// The chain ENS names are looked up on, for a test or a live edge.
#[must_use]
pub fn ens_chain(test: bool) -> u64 {
    if test {
        SEPOLIA
    } else {
        ETHEREUM
    }
}

/// One network this edge settles on.
#[derive(Debug, Clone)]
pub struct UsdcChain {
    pub network: &'static Network,
    /// Decrypted systemd credential: one URL, read per request.
    pub rpc_file: String,
    /// `LososEscrow` on an EVM chain, the `losos-escrow` program on Solana.
    /// `None` only on a Solana network paid straight to the seller.
    pub pay: Option<String>,
    /// EVM: a lowercase `0x` address. Solana: the owner whose USDC account
    /// takes the fee.
    pub treasury: String,
    /// Solana: the genesis hash the endpoint must report. The network's own,
    /// unless a test network names a local validator.
    pub genesis: Option<String>,
}

impl UsdcChain {
    #[must_use]
    pub fn public(&self) -> ChainPublic {
        ChainPublic {
            network: self.network.key,
            name: self.network.name,
            family: self.network.family,
            chain_id: self.network.chain_id,
            usdc: self.network.usdc,
            pay: self.pay.clone(),
            treasury: self.treasury.clone(),
        }
    }
}

/// What `--market-usdc` and `--market-ens-rpc-file` captured. Every chain is
/// of one mode; the parser refuses a mix.
#[derive(Debug, Clone)]
pub struct UsdcOpts {
    pub chains: Vec<UsdcChain>,
    /// Ethereum or Sepolia, for ENS names only.
    pub ens_rpc_file: Option<String>,
    /// Where the prices of ETH and SOL come from, so a page can compare the
    /// network fee of a payment across networks in dollars.
    pub price_sources: [crate::coins::PriceSource; 2],
    /// The escrow's releaser key, a decrypted credential. Needed whenever an
    /// EVM chain is configured; one key for every EVM chain.
    pub releaser_file: Option<String>,
}

impl UsdcOpts {
    #[must_use]
    pub fn test(&self) -> bool {
        self.chains.first().is_some_and(|c| c.network.test)
    }

    #[must_use]
    pub fn chain(&self, key: &str) -> Option<&UsdcChain> {
        self.chains.iter().find(|c| c.network.key == key)
    }

    #[must_use]
    pub fn has(&self, family: Family) -> bool {
        self.chains.iter().any(|c| c.network.family == family)
    }

    /// Whether a Solana network here holds payments in the escrow program.
    #[must_use]
    pub fn solana_escrowed(&self) -> bool {
        self.chains
            .iter()
            .any(|c| c.network.family == Family::Solana && c.pay.is_some())
    }

    /// The chain id an EVM payout signature names: the first EVM chain's. The
    /// address is the same on every EVM chain, so one signature covers them.
    #[must_use]
    pub fn payout_chain_id(&self) -> Option<u64> {
        self.chains.iter().find_map(|c| c.network.chain_id)
    }

    /// The network key a Solana payout signature names.
    #[must_use]
    pub fn payout_solana(&self) -> Option<&'static str> {
        self.chains
            .iter()
            .find(|c| c.network.family == Family::Solana)
            .map(|c| c.network.key)
    }

    #[must_use]
    pub fn public(&self) -> UsdcPublic {
        UsdcPublic {
            test: self.test(),
            chain_id: self.payout_chain_id(),
            solana: self.payout_solana(),
            chains: self.chains.iter().map(UsdcChain::public).collect(),
            ens_chain_id: self.ens_rpc_file.as_ref().map(|_| ens_chain(self.test())),
            prices: std::collections::BTreeMap::new(),
            onramp: None,
        }
    }
}

/// EVM JSON-RPC methods the registrar will forward. `eth_sendRawTransaction`
/// is the one that publishes anything, and it only forwards a transaction the
/// wallet has already signed: the admin page's CSP lets it talk to this box
/// and nothing else, so a wallet created on the page has no other way to
/// reach the chain. The registrar still has no key.
pub const EVM_METHODS: &[&str] = &[
    "eth_blockNumber",
    "eth_call",
    "eth_chainId",
    "eth_estimateGas",
    "eth_feeHistory",
    "eth_gasPrice",
    "eth_getBalance",
    "eth_getBlockByNumber",
    "eth_getCode",
    // The wallet's receipt wait asks for the transaction while it is not
    // yet in a block; refusing it failed every payment after the money moved.
    "eth_getTransactionByHash",
    "eth_getTransactionCount",
    "eth_getTransactionReceipt",
    "eth_maxPriorityFeePerGas",
    "eth_sendRawTransaction",
    "net_version",
];

const RPC_TIMEOUT: Duration = Duration::from_secs(4);
/// A forwarded reply. Large enough for a receipt, small enough that a
/// hostile endpoint cannot pin the registrar on a huge body.
const MAX_RPC_REPLY: usize = 256 * 1024;

/// Blocks a payment must have, its own included, before it counts. Base
/// makes a block every two seconds, so this is a few seconds of waiting
/// against a payment that a short reorganisation would undo.
pub const CONFIRMATIONS: u64 = 2;

/// What one paid order must look like on chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedPayment {
    /// The escrow contract.
    pub pay: [u8; 20],
    pub seller: [u8; 20],
    pub order_id_hash: [u8; 32],
    pub gross: u128,
    pub fee: u128,
    /// When the payer may take the deposit back alone. The invoice names it,
    /// so the edge knows it has until then to release.
    pub deadline: u64,
    /// The keys the buyer's and the seller's boxes registered
    /// (`crate::escrow_boxes`): two of these and the edge's move the
    /// deposit, so a payment that names others is not counted.
    pub buyer_box: [u8; 20],
    pub seller_box: [u8; 20],
}

/// How long after an order's hold the payer may take an unreleased deposit
/// back: the month the slot runs (`crate::slots`), the day the seller has to
/// ask for the payout, and the two days the edge keeps in hand before the
/// deadline. `LososEscrow` takes deadlines up to 35 days out.
pub const ESCROW_REFUND_AFTER_SECS: u64 = 33 * 24 * 60 * 60;

/// The escrow deadline for an order whose hold ends at `pay_before`.
#[must_use]
pub fn refund_deadline(pay_before: u64) -> u64 {
    pay_before.saturating_add(ESCROW_REFUND_AFTER_SECS)
}

/// A deposit's state in `LososEscrow.deposits`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepositState {
    None,
    Held,
    Released,
    Refunded,
    /// Part refunded, the rest released (`settle`).
    Split,
}

/// Cents of the market's currency, as USDC base units (6 decimals).
#[must_use]
pub fn cents_to_usdc(cents: u64) -> u128 {
    u128::from(cents) * 10_000
}

/// The text a wallet signs to prove it controls the payout address. The
/// chain is in the message so a signature for a test network cannot set a
/// live payout: an EVM chain id, or a Solana network key. An EVM address is
/// lowercase so there is one encoding; a Solana address is base58.
#[must_use]
pub fn payout_message(chain: impl std::fmt::Display, address: &str) -> String {
    format!("LosOS market payout v1\nchain: {chain}\naddress: {address}\n")
}

/// keccak256 of the order id string. That is the `bytes32` the wallet passes
/// to `LososPay.pay`, and the first indexed topic of `Paid`.
#[must_use]
pub fn order_id_hash(order_id: &str) -> [u8; 32] {
    keccak256(order_id.as_bytes())
}

/// The key `LososEscrow` keeps `buyer`'s tap `tap` under:
/// `keccak256(abi.encode(buyer, keccak256(tap)))`, as its `tapKey` works it
/// out. The wallet passes only `keccak256(tap)`; the buyer is whoever sends
/// `openTaps`, so the same id from another wallet is another tap.
#[must_use]
pub fn tap_key(buyer: [u8; 20], tap: &str) -> [u8; 32] {
    let mut data = crate::evm_tx::address_word(buyer).to_vec();
    data.extend_from_slice(&order_id_hash(tap));
    keccak256(&data)
}

#[must_use]
pub fn order_id_hash_hex(order_id: &str) -> String {
    format!("0x{}", to_hex(&order_id_hash(order_id)))
}

/// `0x` + 40 hex, lowercased, or `None`.
#[must_use]
pub fn normalize_address(raw: &str) -> Option<String> {
    let bytes = decode_address(raw)?;
    Some(format!("0x{}", to_hex(&bytes)))
}

/// The 20 bytes of an address `normalize_address` would accept.
#[must_use]
pub fn address_bytes(raw: &str) -> Option<[u8; 20]> {
    decode_address(raw)
}

/// `0x` + 64 hex, lowercased, or `None`.
#[must_use]
pub fn normalize_tx(raw: &str) -> Option<String> {
    let bytes = decode_hex(raw)?;
    if bytes.len() != 32 {
        return None;
    }
    Some(format!("0x{}", to_hex(&bytes)))
}

/// The address that signed `message` with an Ethereum `personal_sign`
/// signature (`0x` + 65 bytes, `v` as 27 or 28). `None` when the signature
/// does not recover.
#[must_use]
pub fn recover_address(message: &[u8], signature_hex: &str) -> Option<String> {
    let sig = decode_hex(signature_hex)?;
    if sig.len() != 65 {
        return None;
    }
    let signature = Signature::from_slice(&sig[..64]).ok()?;
    let recid = match sig[64] {
        27 | 0 => RecoveryId::try_from(0u8).ok()?,
        28 | 1 => RecoveryId::try_from(1u8).ok()?,
        _ => return None,
    };
    let hash = personal_sign_hash(message);
    let key = VerifyingKey::recover_from_prehash(&hash, &signature, recid).ok()?;
    Some(address_of(&key))
}

/// Read the sealed RPC URL. A missing file, a blank file or anything that is
/// not a plain `https://` URL is "not configured": the market answers 503
/// and the rest of the registrar is untouched. The URL is not in the error,
/// because it is the secret.
pub async fn read_rpc_url(path: &Path) -> Result<String, MarketError> {
    let raw = tokio::fs::read_to_string(path)
        .await
        .map_err(|_| MarketError::Unconfigured)?;
    let url = raw.trim();
    if !rpc_url_ok(url) {
        return Err(MarketError::Unconfigured);
    }
    Ok(url.to_string())
}

/// Whether `url` may be used as the chain endpoint. `https`, or plain
/// `http` to a node on this machine's loopback; no userinfo
/// (`https://key@host` would hide the host), no whitespace, and a host. The
/// path and query may carry a provider key; that is why the whole URL is the
/// sealed credential.
#[must_use]
pub fn rpc_url_ok(url: &str) -> bool {
    if url.len() > 2048
        || url
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return false;
    }
    let (rest, tls) = if let Some(rest) = url.strip_prefix("https://") {
        (rest, true)
    } else if let Some(rest) = url.strip_prefix("http://") {
        (rest, false)
    } else {
        return false;
    };
    if rest.is_empty() || rest.starts_with('/') {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return false;
    }
    if tls {
        return authority.contains('.');
    }
    let host = authority
        .rsplit_once(':')
        .filter(|(_, port)| port.bytes().all(|b| b.is_ascii_digit()))
        .map_or(authority, |(host, _)| host);
    matches!(host, "127.0.0.1" | "localhost" | "[::1]")
}

/// A single raw transaction: `0x` plus an even run of hex, long enough to be
/// a signed transaction and short enough that it is not a blob dump.
fn raw_tx_ok(params: &Value) -> bool {
    let Some(raw) = params
        .as_array()
        .and_then(|p| p.first())
        .and_then(Value::as_str)
    else {
        return false;
    };
    let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) else {
        return false;
    };
    (100..=20_000).contains(&hex.len())
        && hex.len() % 2 == 0
        && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

/// What the browser is allowed to know. No RPC URL.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct UsdcPublic {
    pub test: bool,
    /// The chain id an EVM payout signature names, when the edge settles on
    /// an EVM chain.
    pub chain_id: Option<u64>,
    /// The network a Solana payout signature names, when it settles there.
    pub solana: Option<&'static str>,
    pub chains: Vec<ChainPublic>,
    /// Where the page may look ENS names up through the relay, when the
    /// operator gave the edge an endpoint for it.
    pub ens_chain_id: Option<u64>,
    /// The fee coins' prices two sources agree on, micro-dollars per whole
    /// coin as text, keyed `eth` and `sol`. Missing while they disagree or
    /// are out of reach.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub prices: std::collections::BTreeMap<&'static str, String>,
    /// Who sells USDC on Base for euros through this edge (`binance`), when
    /// anyone does (`crate::onramp`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onramp: Option<&'static str>,
}

/// One network as a page sees it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ChainPublic {
    pub network: &'static str,
    pub name: &'static str,
    pub family: Family,
    pub chain_id: Option<u64>,
    pub usdc: &'static str,
    pub pay: Option<String>,
    pub treasury: String,
}

/// One network an invoice may be paid on, and whom it pays there.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct InvoiceChain {
    #[serde(flatten)]
    pub chain: ChainPublic,
    /// The seller's payout address on this network's family.
    pub seller: String,
    /// The buyer's and the seller's box keys on this network's family, the
    /// two arguments of the escrow's `pay` after the deadline. Absent on a
    /// network that pays the seller directly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer_box: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seller_box: Option<String>,
}

/// What the buyer's wallet pays. Amounts are decimal strings of USDC base
/// units (6 decimals), so a page does not have to put a `u128` in a JSON
/// number. `chains` are the networks the order may be paid on: the edge's,
/// less any family the seller has no payout address for.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StablecoinInvoice {
    pub order_id: String,
    pub order_id_hash: String,
    /// Gross, USDC base units, decimal.
    pub gross: String,
    /// Platform fee, USDC base units, decimal. Includes `fee_offset`.
    pub fee: String,
    /// The part of `fee` that goes toward platform fees the seller owes on
    /// earlier coin sales, USDC base units, decimal. Absent when none: the
    /// buyer pays `gross` either way, only the seller's share is smaller.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_offset: Option<String>,
    /// Unix seconds. The order holds its units until then, so a wallet
    /// should not start paying after it: a later payment is honoured only
    /// while the units are still free.
    pub pay_before: u64,
    /// Unix seconds, the `deadline` argument of `LososEscrow.pay`: after it
    /// the payer may take an unreleased deposit back. The edge counts a
    /// payment only with this exact deadline.
    pub deadline: u64,
    pub chains: Vec<InvoiceChain>,
    /// Coins other than USDC the seller takes for this order (`btc`); the
    /// buyer asks the edge for an address in one of them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub coins: Vec<&'static str>,
}

/// The seller's payout addresses an order was invoiced to, one per family.
#[derive(Debug, Clone, Copy, Default)]
pub struct PayTo<'a> {
    pub evm: Option<&'a str>,
    pub solana: Option<&'a str>,
}

impl PayTo<'_> {
    #[must_use]
    pub fn of(&self, family: Family) -> Option<&str> {
        match family {
            Family::Evm => self.evm,
            Family::Solana => self.solana,
        }
    }
}

#[must_use]
pub fn invoice(
    opts: &UsdcOpts,
    order_id: &str,
    pay_to: PayTo<'_>,
    boxes: Option<&crate::escrow_boxes::OrderBoxes>,
    gross_cents: u64,
    fee_cents: u64,
    pay_before: u64,
) -> StablecoinInvoice {
    StablecoinInvoice {
        order_id: order_id.to_string(),
        order_id_hash: order_id_hash_hex(order_id),
        gross: cents_to_usdc(gross_cents).to_string(),
        fee: cents_to_usdc(fee_cents).to_string(),
        fee_offset: None,
        pay_before,
        deadline: refund_deadline(pay_before),
        chains: opts
            .chains
            .iter()
            .filter_map(|c| {
                let seller = pay_to.of(c.network.family)?;
                // An escrowed network needs both box keys; without them the
                // edge could not count the payment.
                let keys = |side: &crate::escrow_boxes::BoxKeys| {
                    side.of(c.network.family).map(str::to_string)
                };
                let (buyer_box, seller_box) = match (c.pay.is_some(), boxes) {
                    (false, _) => (None, None),
                    (true, Some(b)) => (Some(keys(&b.buyer)?), Some(keys(&b.seller)?)),
                    (true, None) => return None,
                };
                Some(InvoiceChain {
                    chain: c.public(),
                    seller: seller.to_string(),
                    buyer_box,
                    seller_box,
                })
            })
            .collect(),
        coins: Vec::new(),
    }
}

/// Which methods a relayed call may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relay {
    Evm,
    Solana,
    /// Name lookups on Ethereum or Sepolia: reads only.
    Ens,
}

/// ENS lookups are contract reads through the universal resolver.
const ENS_METHODS: &[&str] = &["eth_blockNumber", "eth_call", "eth_chainId"];

impl Relay {
    fn allows(self, method: &str) -> bool {
        match self {
            Relay::Evm => EVM_METHODS.contains(&method),
            Relay::Solana => crate::solana::METHODS.contains(&method),
            Relay::Ens => ENS_METHODS.contains(&method),
        }
    }
}

/// Forward one JSON-RPC call to `rpc_url`. The method has to be on the
/// allowlist, and the error text never includes the URL.
pub async fn forward_rpc(
    rpc_url: &str,
    relay: Relay,
    method: &str,
    params: &Value,
    id: &Value,
) -> Result<Value, MarketError> {
    if !relay.allows(method) {
        return Err(MarketError::Invalid("that chain method is not forwarded"));
    }
    if !params.is_array() || params.as_array().is_some_and(|p| p.len() > 8) {
        return Err(MarketError::Invalid("chain params must be a short array"));
    }
    if method == "eth_sendRawTransaction" && !raw_tx_ok(params) {
        return Err(MarketError::Invalid(
            "the signed transaction is not a raw transaction",
        ));
    }
    if method == "sendTransaction" && !crate::solana::wire_tx_ok(params) {
        return Err(MarketError::Invalid(
            "the signed transaction is not a raw transaction",
        ));
    }
    let body = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    let client = reqwest::Client::builder()
        .timeout(RPC_TIMEOUT)
        .build()
        .map_err(|_| MarketError::Unconfigured)?;
    let mut response = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|_| MarketError::Conflict("the chain endpoint did not answer"))?;
    // Read the body a chunk at a time, so the cap holds before the whole
    // reply is in memory rather than after.
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| MarketError::Conflict("the chain endpoint did not answer"))?
    {
        if bytes.len() + chunk.len() > MAX_RPC_REPLY {
            return Err(MarketError::Conflict(
                "the chain endpoint answered too much",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| MarketError::Conflict("the chain endpoint did not answer JSON"))
}

/// The pseudo-method a wallet sends through the ENS relay when a lookup
/// asks for an offchain answer (EIP-3668). Not a node method: the edge
/// answers it from [`ENS_DNS_GATEWAY`].
pub const ENS_CCIP_METHOD: &str = "ens_ccipRead";

/// The one gateway the relay asks: ENS's own, which `OffchainDNSResolver`
/// names for every DNS name (`mattbox.boxes.losos.dasmat.us` and an
/// owner's domain alike). It answers with DNSSEC proofs the contract checks
/// itself, so the gateway is not trusted with the answer.
pub const ENS_DNS_GATEWAY: &str = "https://dnssec-oracle.ens.domains/";

/// The calldata a resolver hands the gateway: a DNS name and a query, a few
/// hundred bytes.
const MAX_CCIP_DATA: usize = 16 * 1024;

fn hex_bytes(s: &str, max: usize) -> bool {
    s.strip_prefix("0x").is_some_and(|h| {
        h.len() % 2 == 0 && h.len() <= max * 2 && h.bytes().all(|b| b.is_ascii_hexdigit())
    })
}

/// Ask `gateway` for one offchain answer: `params` is `[sender, data]`, as
/// the resolver's `OffchainLookup` revert gave them. The reply is shaped as a
/// JSON-RPC object whose result is the gateway's `data`, so a wallet's
/// transport reads it like any relayed call.
///
/// # Errors
/// Params that are not an address and calldata, or a gateway that does not
/// answer hex.
pub async fn forward_ccip(gateway: &str, params: &Value, id: &Value) -> Result<Value, MarketError> {
    let (Some(sender), Some(data)) = (
        params.get(0).and_then(Value::as_str),
        params.get(1).and_then(Value::as_str),
    ) else {
        return Err(MarketError::Invalid(
            "an offchain lookup is a sender and its calldata",
        ));
    };
    if params.as_array().map_or(0, Vec::len) != 2 || !hex_bytes(sender, 20) || sender.len() != 42 {
        return Err(MarketError::Invalid("the sender is not an address"));
    }
    if !hex_bytes(data, MAX_CCIP_DATA) {
        return Err(MarketError::Invalid("the lookup's calldata is not hex"));
    }
    let client = reqwest::Client::builder()
        .timeout(RPC_TIMEOUT * 2)
        .build()
        .map_err(|_| MarketError::Unconfigured)?;
    let mut response = client
        .post(gateway)
        .json(&json!({ "sender": sender.to_ascii_lowercase(), "data": data }))
        .send()
        .await
        .map_err(|_| MarketError::Conflict("the name gateway did not answer"))?;
    if !response.status().is_success() {
        return Err(MarketError::Conflict("the name gateway refused the lookup"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| MarketError::Conflict("the name gateway did not answer"))?
    {
        if bytes.len() + chunk.len() > MAX_RPC_REPLY {
            return Err(MarketError::Conflict("the name gateway answered too much"));
        }
        bytes.extend_from_slice(&chunk);
    }
    let reply: Value = serde_json::from_slice(&bytes)
        .map_err(|_| MarketError::Conflict("the name gateway did not answer JSON"))?;
    match reply.get("data").and_then(Value::as_str) {
        Some(answer) if hex_bytes(answer, MAX_RPC_REPLY) => Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": answer,
        })),
        _ => Err(MarketError::Conflict("the name gateway did not answer hex")),
    }
}

/// The receipt object, or a conflict when it is not mined yet. Errors do not
/// carry the RPC URL.
pub async fn fetch_receipt(rpc_url: &str, tx: &str) -> Result<Value, MarketError> {
    let reply = forward_rpc(
        rpc_url,
        Relay::Evm,
        "eth_getTransactionReceipt",
        &json!([tx]),
        &json!(1),
    )
    .await?;
    match reply.get("result") {
        Some(Value::Object(_)) => Ok(reply["result"].clone()),
        Some(Value::Null) | None => {
            Err(MarketError::Conflict("the transaction is not on chain yet"))
        }
        _ => Err(MarketError::Conflict(
            "the chain endpoint did not answer JSON",
        )),
    }
}

/// The `result` of one chain call, or a conflict naming what failed.
pub(crate) async fn call_result(
    rpc_url: &str,
    relay: Relay,
    method: &str,
    params: Value,
) -> Result<Value, MarketError> {
    let reply = forward_rpc(rpc_url, relay, method, &params, &json!(1)).await?;
    match reply.get("result") {
        Some(v) if !v.is_null() => Ok(v.clone()),
        _ => Err(MarketError::Conflict(
            "the chain endpoint returned an error",
        )),
    }
}

/// Check that the sealed endpoint is the chain this edge settles on, and
/// that the configured `LososEscrow` pulls Circle's USDC, pays the configured
/// treasury and takes orders from this edge's releaser key. Without this a
/// Sepolia endpoint behind a live config would turn test USDC into paid live
/// orders, a contract deployed over a token anyone can mint would pay for
/// orders with nothing, and a contract someone else releases would let them
/// decide where buyers' money goes.
pub async fn check_endpoint(
    rpc_url: &str,
    chain: &UsdcChain,
    releaser: [u8; 20],
) -> Result<(), MarketError> {
    let pay = chain.pay.as_deref().ok_or(MarketError::Unconfigured)?;
    let id = call_result(rpc_url, Relay::Evm, "eth_chainId", json!([])).await?;
    let usdc_word = call_result(
        rpc_url,
        Relay::Evm,
        "eth_call",
        json!([{ "to": pay, "data": selector("usdc()") }, "latest"]),
    )
    .await?;
    let treasury_word = call_result(
        rpc_url,
        Relay::Evm,
        "eth_call",
        json!([{ "to": pay, "data": selector("treasury()") }, "latest"]),
    )
    .await?;
    endpoint_matches(chain, &id, &usdc_word, &treasury_word).map_err(MarketError::Conflict)?;
    let releaser_word = call_result(
        rpc_url,
        Relay::Evm,
        "eth_call",
        json!([{ "to": pay, "data": selector("releaser()") }, "latest"]),
    )
    .await?;
    if topic_address(releaser_word.as_str().unwrap_or("")) != Some(releaser) {
        return Err(MarketError::Conflict(
            "the escrow takes orders from a different releaser key",
        ));
    }
    Ok(())
}

/// The releaser's gas on one network, in wei, and whether `escrow` takes
/// orders from it.
pub async fn releaser_on_chain(
    rpc_url: &str,
    escrow: &str,
    releaser: [u8; 20],
) -> Result<(u128, bool), MarketError> {
    let balance = call_result(
        rpc_url,
        Relay::Evm,
        "eth_getBalance",
        json!([format!("0x{}", to_hex(&releaser)), "latest"]),
    )
    .await?;
    let balance = balance
        .as_str()
        .and_then(|b| b.strip_prefix("0x"))
        .and_then(|b| u128::from_str_radix(if b.is_empty() { "0" } else { b }, 16).ok())
        .ok_or(MarketError::Conflict("the chain endpoint did not answer"))?;
    let word = call_result(
        rpc_url,
        Relay::Evm,
        "eth_call",
        json!([{ "to": escrow, "data": selector("releaser()") }, "latest"]),
    )
    .await?;
    Ok((
        balance,
        topic_address(word.as_str().unwrap_or("")) == Some(releaser),
    ))
}

/// Whether `seller` can receive USDC on this chain, from its code there.
/// The payout signature already proved someone holds the key; this catches
/// an address that is a contract on this chain (the payout signature cannot
/// come from one) and the addresses money must never be released to. An
/// account delegated under EIP-7702 is still a key holder's account.
pub fn seller_can_receive(
    chain: &UsdcChain,
    seller: [u8; 20],
    code: &Value,
) -> Result<(), &'static str> {
    let special = [
        Some([0u8; 20]),
        decode_address(chain.network.usdc),
        chain.pay.as_deref().and_then(decode_address),
        decode_address(&chain.treasury),
    ];
    if special.contains(&Some(seller)) {
        return Err("the seller's payout address is not a wallet");
    }
    let code = code.as_str().ok_or("the chain endpoint did not answer")?;
    let bytes = if code == "0x" || code.is_empty() {
        Vec::new()
    } else {
        decode_hex(code).ok_or("the chain endpoint did not answer")?
    };
    let delegated = bytes.len() == 23 && bytes.starts_with(&[0xef, 0x01, 0x00]);
    if !bytes.is_empty() && !delegated {
        return Err("the seller's payout address is a contract on this network");
    }
    Ok(())
}

/// [`seller_can_receive`] against the chain. The outer error is the
/// endpoint failing; the inner one is the seller's address refused.
pub async fn check_seller(
    rpc_url: &str,
    chain: &UsdcChain,
    seller: [u8; 20],
) -> Result<Result<(), &'static str>, MarketError> {
    let code = call_result(
        rpc_url,
        Relay::Evm,
        "eth_getCode",
        json!([format!("0x{}", to_hex(&seller)), "latest"]),
    )
    .await?;
    if !code.is_string() {
        return Err(MarketError::Conflict("the chain endpoint did not answer"));
    }
    Ok(seller_can_receive(chain, seller, &code))
}

/// A deposit's state as the escrow records it.
pub async fn deposit_state(
    rpc_url: &str,
    escrow: &str,
    deposit: u64,
) -> Result<DepositState, MarketError> {
    let data = format!("0x{}", to_hex(&crate::evm_tx::deposits_data(deposit)));
    let answer = call_result(
        rpc_url,
        Relay::Evm,
        "eth_call",
        json!([{ "to": escrow, "data": data }, "latest"]),
    )
    .await?;
    let bytes = decode_hex(answer.as_str().unwrap_or(""))
        .ok_or(MarketError::Conflict("the chain endpoint did not answer"))?;
    // orderId, payer, seller, gross, fee, deadline, state.
    match bytes.get(6 * 32..7 * 32).and_then(|w| w.last()) {
        Some(0) => Ok(DepositState::None),
        Some(1) => Ok(DepositState::Held),
        Some(2) => Ok(DepositState::Released),
        Some(3) => Ok(DepositState::Refunded),
        Some(4) => Ok(DepositState::Split),
        _ => Err(MarketError::Conflict("the chain endpoint did not answer")),
    }
}

/// A tap as `LososEscrow.taps(bytes32)` answers it: `None` when the escrow
/// has no such tap (a zero buyer).
#[must_use]
pub fn decode_tap(answer: &[u8]) -> Option<Option<crate::meter::ChainTap>> {
    let word = |i: usize| answer.get(i * 32..i * 32 + 32);
    let key = |i: usize| -> Option<[u8; 32]> {
        let w = word(i)?;
        if w[..12].iter().any(|b| *b != 0) {
            return None;
        }
        let mut k = [0u8; 32];
        k.copy_from_slice(w);
        Some(k)
    };
    let small = |i: usize| -> Option<u64> { u64::try_from(word_u128(answer, i)?).ok() };
    let buyer = key(0)?;
    if buyer == [0u8; 32] {
        return Some(None);
    }
    let closed_at = small(5)?;
    Some(Some(crate::meter::ChainTap {
        buyer,
        seller: key(1)?,
        rate: small(2)?,
        fee_bps: u32::try_from(small(3)?).ok()?,
        settled_at: small(4)?,
        closed_at: (closed_at > 0).then_some(closed_at),
    }))
}

async fn escrow_call(rpc_url: &str, escrow: &str, data: &[u8]) -> Result<Vec<u8>, MarketError> {
    let data = format!("0x{}", to_hex(data));
    let answer = call_result(
        rpc_url,
        Relay::Evm,
        "eth_call",
        json!([{ "to": escrow, "data": data }, "latest"]),
    )
    .await?;
    decode_hex(answer.as_str().unwrap_or(""))
        .ok_or(MarketError::Conflict("the chain endpoint did not answer"))
}

/// A tap as the escrow records it, `None` when it has none by that id.
pub async fn tap_state(
    rpc_url: &str,
    escrow: &str,
    tap: [u8; 32],
) -> Result<Option<crate::meter::ChainTap>, MarketError> {
    let bytes = escrow_call(rpc_url, escrow, &crate::evm_tx::taps_data(tap)).await?;
    decode_tap(&bytes).ok_or(MarketError::Conflict("the chain endpoint did not answer"))
}

/// What the escrow holds for `wallet`, and what it asked to withdraw.
pub async fn wallet_view(
    rpc_url: &str,
    escrow: &str,
    wallet: [u8; 20],
) -> Result<crate::meter::WalletView, MarketError> {
    let bad = || MarketError::Conflict("the chain endpoint did not answer");
    let balance = escrow_call(rpc_url, escrow, &crate::evm_tx::balances_data(wallet)).await?;
    let ask = escrow_call(rpc_url, escrow, &crate::evm_tx::withdrawals_data(wallet)).await?;
    let small = |b: &[u8], i| word_u128(b, i).and_then(|v| u64::try_from(v).ok());
    Ok(crate::meter::WalletView {
        balance: small(&balance, 0).ok_or_else(bad)?,
        // The escrow keeps an ask as asked, up to 2^128 - 1, and pays out
        // at most the balance; more than u64 holds reads as all of it.
        asked: word_u128(&ask, 0)
            .map(|v| u64::try_from(v).unwrap_or(u64::MAX))
            .ok_or_else(bad)?,
        not_before: small(&ask, 1).ok_or_else(bad)?,
    })
}

/// Whether an ENS endpoint is on the chain names are looked up on for this
/// mode. A Sepolia endpoint behind a live edge would resolve names to
/// addresses nobody chose for mainnet.
pub async fn check_ens_endpoint(rpc_url: &str, test: bool) -> Result<(), MarketError> {
    let id = call_result(rpc_url, Relay::Ens, "eth_chainId", json!([])).await?;
    if hex_u64(id.as_str().unwrap_or("")) != Some(ens_chain(test)) {
        return Err(MarketError::Conflict(
            "the name endpoint is on a different chain",
        ));
    }
    Ok(())
}

/// The pure half of [`check_endpoint`]: the three answers against the config.
pub fn endpoint_matches(
    chain: &UsdcChain,
    id: &Value,
    usdc_word: &Value,
    treasury_word: &Value,
) -> Result<(), &'static str> {
    if chain.network.chain_id.is_none()
        || hex_u64(id.as_str().unwrap_or("")) != chain.network.chain_id
    {
        return Err("the chain endpoint is on a different chain");
    }
    let want_usdc =
        decode_address(chain.network.usdc).ok_or("the USDC address is not configured")?;
    if topic_address(usdc_word.as_str().unwrap_or("")) != Some(want_usdc) {
        return Err("the pay contract does not pull Circle's USDC");
    }
    let want_treasury =
        decode_address(&chain.treasury).ok_or("the treasury address is not configured")?;
    if topic_address(treasury_word.as_str().unwrap_or("")) != Some(want_treasury) {
        return Err("the pay contract pays a different treasury");
    }
    Ok(())
}

/// Whether the block `receipt` names has [`CONFIRMATIONS`] blocks on top of
/// it, itself included, at `head`.
pub fn confirmed(receipt: &Value, head: &Value) -> Result<(), &'static str> {
    let block = hex_u64(receipt["blockNumber"].as_str().unwrap_or(""))
        .ok_or("the transaction is not in a block yet")?;
    let head = hex_u64(head.as_str().unwrap_or("")).ok_or("the chain endpoint did not answer")?;
    if head.saturating_sub(block) + 1 < CONFIRMATIONS {
        return Err("the payment is waiting for another block");
    }
    Ok(())
}

/// Check that the sealed endpoint is on the chain `chain` names, for a read
/// that involves no escrow: a direct payment of coin fees.
pub async fn check_chain_id(rpc_url: &str, chain: &UsdcChain) -> Result<(), MarketError> {
    let id = call_result(rpc_url, Relay::Evm, "eth_chainId", json!([])).await?;
    if chain.network.chain_id.is_none()
        || hex_u64(id.as_str().unwrap_or("")) != chain.network.chain_id
    {
        return Err(MarketError::Conflict(
            "the chain endpoint is on a different chain",
        ));
    }
    Ok(())
}

/// The latest block number, as the hex quantity the endpoint sent.
pub async fn block_number(rpc_url: &str) -> Result<Value, MarketError> {
    call_result(rpc_url, Relay::Evm, "eth_blockNumber", json!([])).await
}

fn selector(signature: &str) -> String {
    format!("0x{}", to_hex(&keccak256(signature.as_bytes())[..4]))
}

/// The event an ERC-20 transfer emits.
const TRANSFER: &[u8] = b"Transfer(address,address,uint256)";

/// What a successful transaction paid `treasury` in `token` from `from`, in
/// base units: the sum of the token's `Transfer` logs between the two. This
/// is how a seller settles platform fees owed on coin sales: a plain USDC
/// transfer from their payout address to the edge's treasury. A static
/// sentence, safe to show the owner, when it is not one.
pub fn fee_transfer(
    receipt: &Value,
    token: [u8; 20],
    treasury: [u8; 20],
    from: [u8; 20],
) -> Result<u128, &'static str> {
    if hex_u64(receipt["status"].as_str().unwrap_or("")) != Some(1) {
        return Err("the transaction failed");
    }
    let topic0 = to_hex(&keccak256(TRANSFER));
    let logs = receipt["logs"]
        .as_array()
        .ok_or("the receipt has no logs")?;
    let (mut moves_usdc, mut to_treasury, mut total) = (false, false, 0u128);
    for log in logs {
        if decode_address(log["address"].as_str().unwrap_or("")) != Some(token) {
            continue;
        }
        let topics = log["topics"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        let named = topics.first().and_then(Value::as_str).unwrap_or("");
        if topics.len() < 3 || !named.trim_start_matches("0x").eq_ignore_ascii_case(&topic0) {
            continue;
        }
        moves_usdc = true;
        if topic_address(topics[2].as_str().unwrap_or("")) != Some(treasury) {
            continue;
        }
        to_treasury = true;
        if topic_address(topics[1].as_str().unwrap_or("")) != Some(from) {
            continue;
        }
        let data = decode_hex(log["data"].as_str().unwrap_or("")).unwrap_or_default();
        let value = word_u128(&data, 0).ok_or("the transfer log is short")?;
        total = total.saturating_add(value);
    }
    if !moves_usdc {
        return Err("the transaction moves no USDC");
    }
    if !to_treasury {
        return Err("the transaction does not pay the edge's treasury");
    }
    if total == 0 {
        return Err("the payment did not come from the seller's payout address");
    }
    Ok(total)
}

/// The event `LososEscrow.pay` emits.
const HELD: &[u8] = b"Held(uint256,bytes32,address,address,uint256,uint256,uint64,address,address)";

/// Whether `receipt` is a successful `LososEscrow.pay` that holds exactly
/// `expected`. Returns the deposit's id, which `release` and `refund` take.
/// A static sentence on failure, safe to show the owner.
pub fn payment_matches(receipt: &Value, expected: &ExpectedPayment) -> Result<u64, &'static str> {
    if hex_u64(receipt["status"].as_str().unwrap_or("")) != Some(1) {
        return Err("the transaction failed");
    }
    let topic0 = to_hex(&keccak256(HELD));
    let logs = receipt["logs"]
        .as_array()
        .ok_or("the receipt has no logs")?;
    for log in logs {
        let address = match decode_address(log["address"].as_str().unwrap_or("")) {
            Some(a) => a,
            None => continue,
        };
        if address != expected.pay {
            continue;
        }
        let topics = log["topics"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        if topics.len() < 4 {
            continue;
        }
        let got0 = topics[0].as_str().unwrap_or("").trim_start_matches("0x");
        if !got0.eq_ignore_ascii_case(&topic0) {
            continue;
        }
        let Some(order) = decode_word(topics[2].as_str().unwrap_or("")) else {
            continue;
        };
        if order != expected.order_id_hash {
            continue;
        }
        let Some(deposit) = decode_word(topics[1].as_str().unwrap_or(""))
            .filter(|w| w[..24].iter().all(|b| *b == 0))
            .map(|w| u64::from_be_bytes(w[24..].try_into().expect("eight bytes")))
        else {
            return Err("the payment log is short");
        };
        let data = decode_hex(log["data"].as_str().unwrap_or("")).unwrap_or_default();
        if data.len() < 6 * 32 {
            return Err("the payment log is short");
        }
        if data[12..32] != expected.seller || data[..12].iter().any(|b| *b != 0) {
            return Err("the payment went to a different seller");
        }
        let (Some(gross), Some(fee)) = (word_u128(&data, 1), word_u128(&data, 2)) else {
            return Err("the payment amount is not the order's");
        };
        if gross != expected.gross || fee != expected.fee {
            return Err("the payment amount is not the order's");
        }
        if word_u128(&data, 3) != Some(u128::from(expected.deadline)) {
            return Err("the payment's refund deadline is not the invoice's");
        }
        let address_word = |i: usize| -> Option<[u8; 20]> {
            let w = data.get(i * 32..i * 32 + 32)?;
            w[..12]
                .iter()
                .all(|b| *b == 0)
                .then(|| w[12..].try_into().expect("20 bytes"))
        };
        if address_word(4) != Some(expected.buyer_box)
            || address_word(5) != Some(expected.seller_box)
        {
            return Err("the payment names other boxes than the invoice");
        }
        return Ok(deposit);
    }
    Err("the transaction does not pay this order")
}

fn personal_sign_hash(message: &[u8]) -> [u8; 32] {
    let prefix = format!("\x19Ethereum Signed Message:\n{}", message.len());
    let mut data = prefix.into_bytes();
    data.extend_from_slice(message);
    keccak256(&data)
}

pub(crate) fn address_of(key: &VerifyingKey) -> String {
    let point = key.to_sec1_point(false);
    let hash = keccak256(&point.as_bytes()[1..]);
    format!("0x{}", to_hex(&hash[12..]))
}

pub(crate) fn keccak256(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

fn decode_address(raw: &str) -> Option<[u8; 20]> {
    let bytes = decode_hex(raw)?;
    if bytes.len() != 20 {
        return None;
    }
    let mut out = [0u8; 20];
    out.copy_from_slice(&bytes);
    Some(out)
}

fn topic_address(topic: &str) -> Option<[u8; 20]> {
    let bytes = decode_hex(topic)?;
    if bytes.len() != 32 {
        return None;
    }
    let mut out = [0u8; 20];
    out.copy_from_slice(&bytes[12..]);
    Some(out)
}

fn decode_word(topic: &str) -> Option<[u8; 32]> {
    let bytes = decode_hex(topic)?;
    if bytes.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Some(out)
}

fn word_u128(data: &[u8], index: usize) -> Option<u128> {
    let start = index * 32;
    let word = data.get(start..start + 32)?;
    if word[..16].iter().any(|b| *b != 0) {
        return None;
    }
    let mut buf = [0u8; 16];
    buf.copy_from_slice(&word[16..]);
    Some(u128::from_be_bytes(buf))
}

fn hex_u64(raw: &str) -> Option<u64> {
    let bytes = decode_hex(raw)?;
    if bytes.is_empty() || bytes.len() > 8 {
        return None;
    }
    let mut acc = 0u64;
    for b in bytes {
        acc = acc << 8 | u64::from(b);
    }
    Some(acc)
}

/// `0x` hex to bytes, odd lengths padded, as JSON-RPC quantities come.
pub(crate) fn decode_hex(raw: &str) -> Option<Vec<u8>> {
    let hex = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X"))?;
    if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    // JSON-RPC quantities are not padded, so `0x1` is a one-nibble status.
    let padded;
    let hex = if hex.len() % 2 == 1 {
        padded = format!("0{hex}");
        padded.as_str()
    } else {
        hex
    };
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect::<Option<Vec<_>>>()
}

/// Sign `message` the way a wallet's `personal_sign` does. Tests only: the
/// registrar never holds a key, so this is not available outside the suite.
#[cfg(test)]
pub(crate) fn sign_personal(secret: &[u8; 32], message: &[u8]) -> String {
    let key = SigningKey::from_bytes(secret.into()).expect("key");
    let (signature, recid) = key.sign_prehash_recoverable(&personal_sign_hash(message));
    let mut bytes = [0u8; 65];
    bytes[..64].copy_from_slice(&signature.to_bytes());
    bytes[64] = u8::from(recid) + 27;
    format!("0x{}", to_hex(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> [u8; 32] {
        let mut secret = [0u8; 32];
        secret[31] = 7;
        secret
    }

    fn address_of_key() -> String {
        let key = SigningKey::from_bytes((&key()).into()).unwrap();
        address_of(&VerifyingKey::from(&key))
    }

    /// A stand-in for ENS's gateway: it answers `0x` + the sender's last
    /// byte twice, or with `reply` when one is given.
    async fn gateway(reply: Option<Value>) -> String {
        use axum::{routing::post, Json, Router};
        let app = Router::new().route(
            "/",
            post(move |Json(body): Json<Value>| {
                let reply = reply.clone();
                async move {
                    Json(reply.unwrap_or_else(|| {
                        let sender = body["sender"].as_str().unwrap_or_default();
                        let data = body["data"].as_str().unwrap_or_default();
                        json!({ "data": format!("0x{}{}", &sender[40..], &data[2..]) })
                    }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        url
    }

    #[tokio::test]
    async fn an_offchain_lookup_goes_to_the_gateway_and_back_as_rpc() {
        let url = gateway(None).await;
        let sender = "0xF142B308cF687d4358410a4cB885513b30A42025";
        let reply = forward_ccip(&url, &json!([sender, "0xabcd"]), &json!(7))
            .await
            .unwrap();
        // The gateway saw the sender in lowercase and the calldata as sent.
        assert_eq!(
            reply,
            json!({ "jsonrpc": "2.0", "id": 7, "result": "0x25abcd" })
        );
    }

    #[tokio::test]
    async fn an_offchain_lookup_is_an_address_and_calldata() {
        let url = gateway(None).await;
        let sender = "0xf142b308cf687d4358410a4cb885513b30a42025";
        for params in [
            json!([]),
            json!([sender]),
            json!([sender, "0xab", "extra"]),
            json!(["0x1234", "0xab"]),
            json!([sender, "abcd"]),
            json!([sender, "0xabc"]),
            json!([sender, "0xzz"]),
            json!([sender, format!("0x{}", "00".repeat(MAX_CCIP_DATA + 1))]),
            json!({ "sender": sender, "data": "0x" }),
        ] {
            let err = forward_ccip(&url, &params, &json!(1)).await.unwrap_err();
            assert!(matches!(err, MarketError::Invalid(_)), "{params}: {err:?}");
        }
    }

    #[tokio::test]
    async fn a_gateway_answer_that_is_not_hex_is_refused() {
        let sender = "0xf142b308cf687d4358410a4cb885513b30a42025";
        for reply in [json!({}), json!({ "data": "nope" }), json!({ "data": 5 })] {
            let url = gateway(Some(reply.clone())).await;
            let err = forward_ccip(&url, &json!([sender, "0x00"]), &json!(1))
                .await
                .unwrap_err();
            assert!(matches!(err, MarketError::Conflict(_)), "{reply}: {err:?}");
        }
        // Nothing listening.
        let err = forward_ccip("http://127.0.0.1:9/", &json!([sender, "0x00"]), &json!(1))
            .await
            .unwrap_err();
        assert!(matches!(err, MarketError::Conflict(_)));
    }

    /// The calls viem makes to send a transaction and wait for its receipt,
    /// the same list lososd relays.
    #[test]
    fn the_wallet_can_send_and_wait_through_the_relay() {
        for method in [
            "eth_chainId",
            "eth_blockNumber",
            "eth_call",
            "eth_estimateGas",
            "eth_getBlockByNumber",
            "eth_maxPriorityFeePerGas",
            "eth_getTransactionCount",
            "eth_sendRawTransaction",
            "eth_getTransactionByHash",
            "eth_getTransactionReceipt",
        ] {
            assert!(Relay::Evm.allows(method), "{method} is forwarded");
        }
    }

    #[test]
    fn a_payout_signature_recovers_the_signing_address() {
        let address = address_of_key();
        let message = payout_message(BASE_SEPOLIA, &address);
        let signature = sign_personal(&key(), message.as_bytes());
        assert_eq!(
            recover_address(message.as_bytes(), &signature).as_deref(),
            Some(address.as_str())
        );
        // A signature for the other chain is a different message.
        let other = payout_message(BASE, &address);
        assert_ne!(
            recover_address(other.as_bytes(), &signature).as_deref(),
            Some(address.as_str())
        );
    }

    #[test]
    fn one_cent_is_ten_thousand_usdc_base_units() {
        assert_eq!(cents_to_usdc(1), 10_000);
        assert_eq!(cents_to_usdc(50), 500_000);
        assert_eq!(cents_to_usdc(2000), 20_000_000);
    }

    #[test]
    fn the_rpc_url_is_https_or_a_loopback_node() {
        assert!(rpc_url_ok("https://base-sepolia.g.alchemy.com/v2/secret"));
        assert!(!rpc_url_ok("http://example.com/v2/secret"));
        assert!(!rpc_url_ok("https://user:secret@example.com/"));
        assert!(!rpc_url_ok("https://example.com/v2/secret key"));
        assert!(!rpc_url_ok("file:///etc/passwd"));
        // Plain http only reaches a node on this machine.
        assert!(rpc_url_ok("http://127.0.0.1:8545"));
        assert!(rpc_url_ok("http://localhost:8545/"));
        assert!(!rpc_url_ok("http://127.0.0.1.example.com:8545"));
        assert!(!rpc_url_ok("http://10.0.0.5:8545"));
    }

    fn opts() -> UsdcChain {
        UsdcChain {
            network: network("base-sepolia").unwrap(),
            rpc_file: String::new(),
            pay: Some("0x1111111111111111111111111111111111111111".to_string()),
            treasury: "0x4444444444444444444444444444444444444444".to_string(),
            genesis: None,
        }
    }

    fn word(address: &str) -> Value {
        json!(format!("0x000000000000000000000000{}", &address[2..]))
    }

    #[test]
    fn the_endpoint_must_be_this_chain_and_the_contract_this_config() {
        let o = opts();
        let usdc = word(USDC_BASE_SEPOLIA);
        let treasury = word(&o.treasury);
        assert_eq!(
            endpoint_matches(&o, &json!("0x14a34"), &usdc, &treasury),
            Ok(())
        );
        // A Base (mainnet) endpoint behind a Sepolia config, or the reverse.
        assert_eq!(
            endpoint_matches(&o, &json!("0x2105"), &usdc, &treasury),
            Err("the chain endpoint is on a different chain")
        );
        // A LososPay deployed over some other token.
        assert_eq!(
            endpoint_matches(
                &o,
                &json!("0x14a34"),
                &word("0x5555555555555555555555555555555555555555"),
                &treasury
            ),
            Err("the pay contract does not pull Circle's USDC")
        );
        assert_eq!(
            endpoint_matches(&o, &json!("0x14a34"), &usdc, &word(USDC_BASE_SEPOLIA)),
            Err("the pay contract pays a different treasury")
        );
    }

    #[test]
    fn a_payment_needs_a_block_on_top_of_its_own() {
        let receipt = json!({ "blockNumber": "0x64" });
        assert_eq!(
            confirmed(&receipt, &json!("0x64")),
            Err("the payment is waiting for another block")
        );
        assert_eq!(confirmed(&receipt, &json!("0x65")), Ok(()));
        assert_eq!(
            confirmed(&json!({}), &json!("0x65")),
            Err("the transaction is not in a block yet")
        );
    }

    #[test]
    fn selectors_are_the_solidity_ones() {
        assert_eq!(selector("usdc()"), "0x3e413bee");
        assert_eq!(selector("treasury()"), "0x61d027b3");
        assert_eq!(selector("releaser()"), "0x02691bcb");
    }

    const BUYER_BOX: &str = "0x6666666666666666666666666666666666666666";
    const SELLER_BOX: &str = "0x7777777777777777777777777777777777777777";

    fn held_receipt(deposit: u64, order: [u8; 32], seller: &str, gross: u128, fee: u128) -> Value {
        held_receipt_boxes(deposit, order, seller, gross, fee, BUYER_BOX, SELLER_BOX)
    }

    fn held_receipt_boxes(
        deposit: u64,
        order: [u8; 32],
        seller: &str,
        gross: u128,
        fee: u128,
        buyer_box: &str,
        seller_box: &str,
    ) -> Value {
        let topic0 = format!("0x{}", to_hex(&keccak256(HELD)));
        json!({
            "status": "0x1",
            "logs": [{
                "address": "0x1111111111111111111111111111111111111111",
                "topics": [
                    topic0,
                    format!("0x{}", to_hex(&pad_u128(u128::from(deposit)))),
                    format!("0x{}", to_hex(&order)),
                    "0x0000000000000000000000003333333333333333333333333333333333333333",
                ],
                "data": format!(
                    "0x000000000000000000000000{}{}{}{}000000000000000000000000{}000000000000000000000000{}",
                    &seller[2..],
                    to_hex(&pad_u128(gross)),
                    to_hex(&pad_u128(fee)),
                    to_hex(&pad_u128(1_700_086_400)),
                    &buyer_box[2..],
                    &seller_box[2..],
                ),
            }],
        })
    }

    #[test]
    fn a_held_log_matches_only_the_recorded_order() {
        let order = order_id_hash("ord_abc");
        let expected = ExpectedPayment {
            pay: decode_address("0x1111111111111111111111111111111111111111").unwrap(),
            seller: decode_address("0x2222222222222222222222222222222222222222").unwrap(),
            order_id_hash: order,
            gross: 20_000_000,
            fee: 800_000,
            deadline: 1_700_086_400,
            buyer_box: decode_address(BUYER_BOX).unwrap(),
            seller_box: decode_address(SELLER_BOX).unwrap(),
        };
        let seller = "0x2222222222222222222222222222222222222222";
        let receipt = held_receipt(7, order, seller, 20_000_000, 800_000);
        assert_eq!(payment_matches(&receipt, &expected), Ok(7), "{receipt}");
        // A buyer who names a key of their own for the seller's box would
        // hold two of the three; one who swaps the two, the same.
        let stranger = "0x8888888888888888888888888888888888888888";
        for (b, s) in [
            (BUYER_BOX, stranger),
            (stranger, SELLER_BOX),
            (SELLER_BOX, BUYER_BOX),
        ] {
            let other = held_receipt_boxes(7, order, seller, 20_000_000, 800_000, b, s);
            assert_eq!(
                payment_matches(&other, &expected),
                Err("the payment names other boxes than the invoice")
            );
        }

        let short = held_receipt(7, order, seller, 1, 800_000);
        assert_eq!(
            payment_matches(&short, &expected),
            Err("the payment amount is not the order's")
        );
        let elsewhere = held_receipt(
            7,
            order,
            "0x5555555555555555555555555555555555555555",
            20_000_000,
            800_000,
        );
        assert_eq!(
            payment_matches(&elsewhere, &expected),
            Err("the payment went to a different seller")
        );
        let other_order = held_receipt(7, order_id_hash("ord_xyz"), seller, 20_000_000, 800_000);
        assert_eq!(
            payment_matches(&other_order, &expected),
            Err("the transaction does not pay this order")
        );
        let mut later = expected.clone();
        later.deadline += 1;
        assert_eq!(
            payment_matches(&receipt, &later),
            Err("the payment's refund deadline is not the invoice's")
        );
        // A log from any other contract is not the escrow's.
        let mut forged = receipt.clone();
        forged["logs"][0]["address"] = json!("0x9999999999999999999999999999999999999999");
        assert_eq!(
            payment_matches(&forged, &expected),
            Err("the transaction does not pay this order")
        );
        let mut failed = receipt.clone();
        failed["status"] = json!("0x0");
        assert_eq!(
            payment_matches(&failed, &expected),
            Err("the transaction failed")
        );
    }

    #[test]
    fn only_a_wallet_on_this_chain_is_paid_out() {
        let o = opts();
        let seller = decode_address("0x2222222222222222222222222222222222222222").unwrap();
        assert_eq!(seller_can_receive(&o, seller, &json!("0x")), Ok(()));
        // An account delegated under EIP-7702 is still its key holder's.
        let delegated = format!("0xef0100{}", "ab".repeat(20));
        assert_eq!(seller_can_receive(&o, seller, &json!(delegated)), Ok(()));
        assert_eq!(
            seller_can_receive(&o, seller, &json!("0x6080604052")),
            Err("the seller's payout address is a contract on this network")
        );
        for special in [
            "0x0000000000000000000000000000000000000000",
            USDC_BASE_SEPOLIA,
            "0x1111111111111111111111111111111111111111",
            "0x4444444444444444444444444444444444444444",
        ] {
            assert_eq!(
                seller_can_receive(&o, decode_address(special).unwrap(), &json!("0x")),
                Err("the seller's payout address is not a wallet"),
                "{special}"
            );
        }
    }

    #[test]
    fn the_invoice_names_the_refund_deadline() {
        let usdc = UsdcOpts {
            chains: vec![opts()],
            ens_rpc_file: None,
            price_sources: [
                crate::coins::PriceSource::kraken(),
                crate::coins::PriceSource::coingecko(),
            ],
            releaser_file: None,
        };
        let pay_to = PayTo {
            evm: Some("0x2222222222222222222222222222222222222222"),
            solana: None,
        };
        let boxes = crate::escrow_boxes::OrderBoxes {
            buyer: crate::escrow_boxes::BoxKeys::parse(BUYER_BOX, None).unwrap(),
            seller: crate::escrow_boxes::BoxKeys::parse(SELLER_BOX, None).unwrap(),
        };
        let i = invoice(&usdc, "ord_1", pay_to, Some(&boxes), 100, 4, 1_000);
        assert_eq!(i.deadline, 1_000 + ESCROW_REFUND_AFTER_SECS);
        assert_eq!(i.chains[0].buyer_box.as_deref(), Some(BUYER_BOX));
        assert_eq!(i.chains[0].seller_box.as_deref(), Some(SELLER_BOX));
        // Without the boxes' keys an escrowed network cannot be paid.
        assert!(invoice(&usdc, "ord_1", pay_to, None, 100, 4, 1_000)
            .chains
            .is_empty());
    }

    fn pad_u128(n: u128) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[16..].copy_from_slice(&n.to_be_bytes());
        out
    }

    /// A receipt written by `edge/contracts/test-payment.sh` after a real `pay`
    /// on anvil. Unset, the test does nothing, so `cargo test` stays offline.
    /// The script sets `LOSOS_ANVIL_RECEIPT` and fails if this does not pass.
    #[test]
    fn an_anvil_receipt_pays_the_order_it_names() {
        let Ok(path) = std::env::var("LOSOS_ANVIL_RECEIPT") else {
            return;
        };
        let meta: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("receipt file")).unwrap();
        let receipt = meta.get("receipt").unwrap_or(&meta);
        let expected = ExpectedPayment {
            pay: decode_address(meta["pay"].as_str().unwrap()).unwrap(),
            seller: decode_address(meta["seller"].as_str().unwrap()).unwrap(),
            order_id_hash: decode_word(meta["order_id_hash"].as_str().unwrap()).unwrap(),
            gross: meta["gross"].as_str().unwrap().parse().unwrap(),
            fee: meta["fee"].as_str().unwrap().parse().unwrap(),
            deadline: meta["deadline"].as_u64().unwrap(),
            buyer_box: decode_address(meta["buyer_box"].as_str().unwrap()).unwrap(),
            seller_box: decode_address(meta["seller_box"].as_str().unwrap()).unwrap(),
        };
        let deposit = payment_matches(receipt, &expected).unwrap();
        assert_eq!(Some(deposit), meta["deposit"].as_u64());
        if let Some(signature) = meta["payout_signature"].as_str() {
            let message = meta["payout_message"].as_str().expect("payout message");
            let address = meta["payout_address"].as_str().expect("payout address");
            assert_eq!(
                recover_address(message.as_bytes(), signature).as_deref(),
                Some(address)
            );
        }
    }

    fn transfer_log(token: &str, from: &str, to: &str, value: u128) -> Value {
        let word = |a: &str| format!("0x000000000000000000000000{}", &a[2..]);
        serde_json::json!({
            "address": token,
            "topics": [format!("0x{}", to_hex(&keccak256(TRANSFER))), word(from), word(to)],
            "data": format!("0x{}", to_hex(&pad_u128(value))),
        })
    }

    #[test]
    fn a_fee_payment_is_a_usdc_transfer_from_the_payout_to_the_treasury() {
        let usdc = decode_address(USDC_BASE_SEPOLIA).unwrap();
        let treasury = "0x4444444444444444444444444444444444444444";
        let seller = "0x5555555555555555555555555555555555555555";
        let other = "0x6666666666666666666666666666666666666666";
        let check = |logs: Vec<Value>, status: &str| {
            fee_transfer(
                &serde_json::json!({ "status": status, "logs": logs }),
                usdc,
                decode_address(treasury).unwrap(),
                decode_address(seller).unwrap(),
            )
        };
        // Two transfers in one transaction count together.
        assert_eq!(
            check(
                vec![
                    transfer_log(USDC_BASE_SEPOLIA, seller, treasury, 600_000),
                    transfer_log(USDC_BASE_SEPOLIA, seller, treasury, 400_000),
                ],
                "0x1"
            ),
            Ok(1_000_000)
        );
        assert_eq!(
            check(
                vec![transfer_log(USDC_BASE_SEPOLIA, seller, treasury, 1)],
                "0x0"
            ),
            Err("the transaction failed")
        );
        // Another token to the treasury is not USDC.
        assert_eq!(
            check(
                vec![transfer_log(other, seller, treasury, 1_000_000)],
                "0x1"
            ),
            Err("the transaction moves no USDC")
        );
        // USDC, but to someone else.
        assert_eq!(
            check(
                vec![transfer_log(USDC_BASE_SEPOLIA, seller, other, 1_000_000)],
                "0x1"
            ),
            Err("the transaction does not pay the edge's treasury")
        );
        // USDC to the treasury from someone else: not this seller's to claim.
        assert_eq!(
            check(
                vec![transfer_log(USDC_BASE_SEPOLIA, other, treasury, 1_000_000)],
                "0x1"
            ),
            Err("the payment did not come from the seller's payout address")
        );
    }

    /// A USDC transfer to the treasury made by `edge/contracts/test-payment.sh`
    /// on anvil, as `transfer` puts it on chain. Unset, the test does
    /// nothing, so `cargo test` stays offline.
    #[test]
    fn an_anvil_fee_transfer_pays_the_treasury() {
        let Ok(path) = std::env::var("LOSOS_ANVIL_FEE_RECEIPT") else {
            return;
        };
        let meta: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("receipt file")).unwrap();
        let paid = fee_transfer(
            &meta["receipt"],
            decode_address(meta["usdc"].as_str().unwrap()).unwrap(),
            decode_address(meta["treasury"].as_str().unwrap()).unwrap(),
            decode_address(meta["from"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            paid,
            meta["amount"].as_str().unwrap().parse::<u128>().unwrap()
        );
    }
}
