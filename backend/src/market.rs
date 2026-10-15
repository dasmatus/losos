//! The market, as the admin UI reaches it.
//!
//! The market itself lives on the edge (`backend-registrar/src/market.rs`),
//! which is the Stripe platform and the only place money is accounted for.
//! This appliance is one of its tenants. The admin pages are served under
//! `connect-src 'self'`, so the browser cannot call the edge, and the
//! registrar wants the appliance's own proxy token, which the browser must
//! never see — so lososd relays: it takes a small, validated [`Op`], adds the
//! appliance's credentials, and forwards it to the registrar's `/market/*`.
//!
//! This module is the pure half: the operations, their validation and wire
//! shape, and how a registrar answer is read. The `curl` that carries them is
//! in `io_backend.rs`, behind [`crate::losos::Losos::market_request`], for the
//! reasons in `catalogue.rs`.
//!
//! # What "unavailable" means
//!
//! A registrar that answers 503 (the edge runs no market) or 403 (this box was
//! not cleared for it) is not an error: it is the common case, and the screen
//! should say "not offered here" rather than show a failure. Both become
//! [`Outcome::Unavailable`]. So does an appliance with no registrar configured
//! at all (`losos.proxy.enable = false`).
//!
//! # What reaches the owner
//!
//! A refusal the owner can act on — "finish Stripe onboarding before listing",
//! "not enough units available" — is passed through, with the registrar's own
//! words, as [`Refused`]. Anything else is a failure whose detail goes to the
//! journal and not the browser.

use std::fmt;

use serde_json::{json, Value};

/// Longest registrar message relayed to the browser.
const MAX_MESSAGE: usize = 200;
/// Longest quantity, in units, one order may ask for. The registrar's own
/// ceiling is authoritative; this only stops obvious nonsense leaving the box.
const MAX_QUANTITY: u64 = 1_000_000;
const MAX_UNIT_PRICE: u64 = 1_000_000;
const MAX_CAPACITY: u64 = 1_000_000_000;
/// A registrar listing or order id: `lst_`/`ord_` and lowercase hex.
const MAX_ID_LEN: usize = 64;

/// EVM chain methods the box will relay. The registrar allowlists the same
/// set.
const RPC_METHODS: &[&str] = &[
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

/// Solana methods the box will relay, the registrar's list
/// (`backend-registrar/src/solana.rs`).
const SOLANA_METHODS: &[&str] = &[
    "getAccountInfo",
    "getBalance",
    "getBlockHeight",
    "getFeeForMessage",
    "getGenesisHash",
    "getLatestBlockhash",
    "getMinimumBalanceForRentExemption",
    "getSignatureStatuses",
    "getSlot",
    "getTokenAccountBalance",
    "getTokenAccountsByOwner",
    "getTransaction",
    "isBlockhashValid",
    "sendTransaction",
    "simulateTransaction",
];

/// A network key the edge names (`base-sepolia`, `solana-devnet`), or `ens`.
/// The edge decides which exist; the box only keeps the shape sane.
fn network_key(raw: &str) -> bool {
    (1..=32).contains(&raw.len())
        && raw
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn is_solana(network: Option<&str>) -> bool {
    network.is_some_and(|n| n.starts_with("solana"))
}

/// Base58 of `bytes` bytes: 32 for a key, 64 for a signature.
fn base58(raw: &str, bytes: usize) -> bool {
    let len = match bytes {
        32 => 32..=44,
        _ => 64..=88,
    };
    len.contains(&raw.len())
        && raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() && !matches!(b, b'0' | b'O' | b'I' | b'l'))
}

fn eth_hex(raw: &str, hex_len: usize) -> bool {
    let Some(hex) = raw.strip_prefix("0x") else {
        return false;
    };
    hex.len() == hex_len && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

fn eth_address(raw: &str) -> bool {
    eth_hex(raw, 40)
}

fn eth_hash(raw: &str) -> bool {
    eth_hex(raw, 64)
}

fn eth_signature(raw: &str) -> bool {
    eth_hex(raw, 130)
}

/// Where the registrar is and who this appliance is to it. `None` from
/// [`Config::from_env`] means the box has no master proxy configured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// `https://register.<domain>`, no trailing slash.
    pub registrar_url: String,
    pub appliance_id: String,
    pub token_file: String,
}

impl Config {
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        Self::from_parts(
            &get("LOSOS_REGISTRAR_URL")?,
            &get("LOSOS_APPLIANCE_ID")?,
            &get("LOSOS_PROXY_TOKEN_FILE")?,
        )
    }

    /// Only an `https://` registrar is accepted: the body carries the token.
    #[must_use]
    pub fn from_parts(url: &str, appliance_id: &str, token_file: &str) -> Option<Self> {
        let url = url.trim().trim_end_matches('/');
        if !url.starts_with("https://") || url.len() <= "https://".len() {
            return None;
        }
        Some(Self {
            registrar_url: url.to_string(),
            appliance_id: appliance_id.trim().to_string(),
            token_file: token_file.trim().to_string(),
        })
    }
}

/// One thing the owner can ask the market to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// What can be bought. Anonymous.
    Browse,
    /// This appliance's listings, purchases, sales and entitlements.
    Account,
    /// Start or resume Stripe onboarding. `box_uuid` is the box's derived
    /// public UUID (see `boxid.rs`), written onto the Stripe account; the HTTP
    /// layer leaves it `None` and [`crate::losos::cmd_market_op`] fills it in,
    /// so no caller can name a UUID that is not this box's.
    Onboard {
        box_uuid: Option<String>,
    },
    List {
        kind: String,
        unit_price: u64,
        capacity: u64,
    },
    Close {
        listing_id: String,
    },
    Order {
        listing_id: String,
        quantity: u64,
    },
    /// Set this box's USDC payout address for one family of networks.
    /// `signature` is the wallet's signature of the payout message the edge
    /// checks: `personal_sign` on an EVM chain, ed25519 on Solana.
    Payout {
        address: String,
        signature: String,
        /// `evm` or `solana`; `evm` when absent.
        family: Option<String>,
    },
    /// The watch-only key this box is paid to in a coin other than USDC:
    /// Bitcoin's extended public key or ranged descriptor, Monero's primary
    /// address with its private view key, or Zcash's unified viewing key.
    /// Never a spending key.
    CoinKey {
        coin: String,
        key: String,
        view_key: Option<String>,
    },
    /// An address and amount to pay `order_id` in `coin`, or what reached it.
    CoinPay {
        order_id: String,
        coin: String,
    },
    /// Tell the edge a USDC transaction on `network` paid `order_id`.
    Confirm {
        order_id: String,
        tx: String,
        network: Option<String>,
    },
    /// Tell the edge a USDC transfer on the EVM `network` paid this seller's
    /// platform fees on coin sales into the edge's treasury.
    FeePay {
        tx: String,
        network: Option<String>,
    },
    /// One allowlisted chain call to `network`, or to the name endpoint for
    /// `ens`. `params` is a JSON array. The answer is the JSON-RPC object,
    /// not a market view.
    Rpc {
        method: String,
        params: String,
        network: Option<String>,
    },
    /// This box's custom domains on the edge, and whether it may add one
    /// (`backend-registrar/src/domains.rs`). Not the market, but the same
    /// relay: the edge wants the proxy token, and only an official edge is
    /// asked.
    Domains,
    DomainAdd {
        domain: String,
    },
    DomainRemove {
        domain: String,
    },
    /// Take this box off the edge's registry (`/deregister`), so the edge
    /// stops routing to it. What a full erase does last, after the domains
    /// and listings are gone (`crate::erase`).
    Deregister,
    /// What the edge's operator sells (boxes, gateways). Anonymous.
    Hardware,
    /// A Stripe Checkout for hardware: `(sku, quantity)` lines. The edge's
    /// gate prices them from its own catalogue.
    HardwareCheckout {
        items: Vec<(String, u64)>,
    },
    /// The widget builder (`backend-registrar/src/builder.rs`): this box's
    /// balance, the price, and its recent builds.
    BuilderAccount,
    /// A Stripe Checkout for one of the edge's credit packs.
    BuilderCredit {
        amount: u64,
    },
    /// Start a build from the owner's description, or a change to `base`,
    /// the widget's current files. `lang` is the admin page's language.
    BuilderStart {
        prompt: String,
        base: Option<Vec<crate::look::WidgetFile>>,
        lang: Option<String>,
    },
    /// One build, with its source once it is done.
    BuilderBuild {
        build_id: String,
    },
    /// The machine images the edge offers, the replica's shape and the
    /// split (`backend-registrar/src/vms.rs`). Anonymous.
    VmImages,
    /// What each replica of this box's machines is doing.
    VmStatus,
    /// Rent `quantity` replicas of `image` from a machine listing.
    VmOrder {
        listing_id: String,
        quantity: u64,
        image: String,
        name: String,
        user_data: Option<String>,
    },
    /// Reserve an upload of the owner's own QCOW2; the answer carries the
    /// single-use path the file is then streamed to.
    VmTicket {
        name: String,
        efi: bool,
    },
    /// Forget one of this box's uploaded images.
    VmRemove {
        upload_id: String,
    },
}

/// Most replicas in one machine order; the edge's own limit.
pub const MAX_VM_REPLICAS: u64 = 10;
/// Longest cloud-init user data relayed; the edge's own limit.
pub const MAX_USER_DATA: usize = 16 * 1024;

/// A catalogue image id (`ubuntu-24.04`) or one of the owner's uploads
/// (`upl_<hex>`).
pub(crate) fn valid_image(id: &str) -> bool {
    if let Some(hex) = id.strip_prefix("upl_") {
        return valid_upload_id_hex(hex);
    }
    let mut bytes = id.bytes();
    id.len() <= 40
        && bytes
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
}

fn valid_upload_id_hex(hex: &str) -> bool {
    !hex.is_empty() && hex.len() <= 48 && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A machine's or an image's name: 1 to 40 characters, none of them a
/// control character.
pub(crate) fn valid_display_name(name: &str) -> bool {
    let n = name.trim().chars().count();
    (1..=40).contains(&n) && !name.chars().any(char::is_control)
}

/// Cloud-init user data the edge will take: a `#cloud-config` document or a
/// script, at most [`MAX_USER_DATA`] bytes.
pub(crate) fn valid_user_data(text: &str) -> bool {
    text.len() <= MAX_USER_DATA
        && (text.starts_with("#cloud-config") || text.starts_with("#!"))
        && !text.contains('\0')
}

/// Longest description of a widget; the edge's limit is the same.
pub const MAX_BUILDER_PROMPT_CHARS: usize = 2000;
/// The languages the builder writes its notes in; the admin UI's own.
pub const BUILDER_LANGS: [&str; 3] = ["en", "sk", "de"];
/// The range a credit pack may be in, in minor units; the edge's packs are
/// the authority.
const MIN_CREDIT: u64 = 100;
const MAX_CREDIT: u64 = 50_000;

/// Most lines and most of one item in a hardware order; the edge's own
/// limits are the same.
pub const MAX_HARDWARE_LINES: usize = 8;
pub const MAX_HARDWARE_QUANTITY: u64 = 20;

fn valid_sku(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Longest domain name relayed. The registrar's own check is the
/// authoritative one; this only stops nonsense leaving the box.
const MAX_DOMAIN_LEN: usize = 253;

/// A plausible domain name, lowercase: letters, digits, hyphens, dots. The
/// edge refuses the rest with a sentence of its own.
pub(crate) fn valid_domain(d: &str) -> bool {
    !d.is_empty()
        && d.len() <= MAX_DOMAIN_LEN
        && d.contains('.')
        && d.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
}

/// Coins other than USDC the market may take for orders.
const COINS: &[&str] = &["btc", "xmr", "zec"];

/// A watch-only key as a wallet exports it: an extended public key, or an
/// output descriptor. Printable, no spaces, bounded; the edge's node checks
/// the rest. Anything naming a private key is refused here already, so one
/// pasted by mistake never leaves the box.
fn coin_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 1024
        && key.chars().all(|c| c.is_ascii_graphic())
        && !key.contains("prv")
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

impl Op {
    /// Whether this is one of the widget builder's operations.
    #[must_use]
    pub fn is_builder(&self) -> bool {
        matches!(
            self,
            Op::BuilderAccount
                | Op::BuilderCredit { .. }
                | Op::BuilderStart { .. }
                | Op::BuilderBuild { .. }
        )
    }

    /// Refuse an operation before it leaves the box.
    ///
    /// # Errors
    /// A sentence naming what was wrong, suitable for a 400.
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Op::Browse
            | Op::Account
            | Op::Domains
            | Op::Hardware
            | Op::Deregister
            | Op::VmImages
            | Op::VmStatus
            | Op::BuilderAccount => Ok(()),
            Op::BuilderCredit { amount } => {
                if (MIN_CREDIT..=MAX_CREDIT).contains(amount) {
                    Ok(())
                } else {
                    Err("the amount is not one of the edge's credit packs")
                }
            }
            Op::BuilderStart { prompt, base, lang } => {
                let prompt = prompt.trim();
                if prompt.is_empty() {
                    Err("describe the widget first")
                } else if prompt.chars().count() > MAX_BUILDER_PROMPT_CHARS {
                    Err("the description is longer than 2000 characters")
                } else if prompt
                    .chars()
                    .any(|c| c.is_control() && c != '\n' && c != '\t')
                {
                    Err("the description has control characters in it")
                } else if base
                    .as_ref()
                    .is_some_and(|b| crate::look::validate_files(b).is_err())
                {
                    Err("the widget to change is not one this box would keep")
                } else if lang.as_deref().is_some_and(|l| !BUILDER_LANGS.contains(&l)) {
                    Err("lang must be en, sk or de")
                } else {
                    Ok(())
                }
            }
            Op::BuilderBuild { build_id } => {
                if valid_id(build_id) {
                    Ok(())
                } else {
                    Err("build_id is not valid")
                }
            }
            Op::VmOrder {
                listing_id,
                quantity,
                image,
                name,
                user_data,
            } => {
                if !valid_id(listing_id) {
                    Err("listing_id is not valid")
                } else if *quantity == 0 || *quantity > MAX_VM_REPLICAS {
                    Err("a machine order has 1 to 10 replicas")
                } else if !valid_image(image) {
                    Err("image is not valid")
                } else if !valid_display_name(name) {
                    Err("a machine's name is 1 to 40 characters")
                } else if user_data.as_deref().is_some_and(|u| !valid_user_data(u)) {
                    Err("cloud-init starts with #cloud-config or #! and is at most 16 KiB")
                } else {
                    Ok(())
                }
            }
            Op::VmTicket { name, .. } => {
                if valid_display_name(name) {
                    Ok(())
                } else {
                    Err("an image's name is 1 to 40 characters")
                }
            }
            Op::VmRemove { upload_id } => {
                if upload_id
                    .strip_prefix("upl_")
                    .is_some_and(valid_upload_id_hex)
                {
                    Ok(())
                } else {
                    Err("upload_id is not valid")
                }
            }
            Op::HardwareCheckout { items } => {
                if items.is_empty() || items.len() > MAX_HARDWARE_LINES {
                    Err("the order needs 1 to 8 lines")
                } else if items.iter().any(|(sku, _)| !valid_sku(sku)) {
                    Err("an item in the order is not valid")
                } else if items
                    .iter()
                    .any(|(_, q)| *q == 0 || *q > MAX_HARDWARE_QUANTITY)
                {
                    Err("a quantity is out of range (1 to 20)")
                } else {
                    Ok(())
                }
            }
            Op::DomainAdd { domain } | Op::DomainRemove { domain } => {
                if valid_domain(domain) {
                    Ok(())
                } else {
                    Err("a domain name has only letters, digits, hyphens and dots, such as cloud.example.org")
                }
            }
            Op::Onboard { box_uuid } => match box_uuid {
                Some(u) if !crate::recovery::is_well_formed(u) => Err("box_uuid is not a UUID"),
                _ => Ok(()),
            },
            Op::List {
                kind,
                unit_price,
                capacity,
            } => {
                if !matches!(kind.as_str(), "storage" | "compute" | "vm") {
                    Err("kind must be 'storage', 'compute' or 'vm'")
                } else if *unit_price == 0 || *unit_price > MAX_UNIT_PRICE {
                    Err("unit_price is out of range")
                } else if *capacity == 0 || *capacity > MAX_CAPACITY {
                    Err("capacity is out of range")
                } else {
                    Ok(())
                }
            }
            Op::Close { listing_id } => {
                if valid_id(listing_id) {
                    Ok(())
                } else {
                    Err("listing_id is not valid")
                }
            }
            Op::Order {
                listing_id,
                quantity,
            } => {
                if !valid_id(listing_id) {
                    Err("listing_id is not valid")
                } else if *quantity == 0 || *quantity > MAX_QUANTITY {
                    Err("quantity is out of range")
                } else {
                    Ok(())
                }
            }
            Op::Payout {
                address,
                signature,
                family,
            } => match family.as_deref() {
                None | Some("evm") => {
                    if !eth_address(address) {
                        Err("address is not an 0x address")
                    } else if !eth_signature(signature) {
                        Err("signature is not a wallet signature")
                    } else {
                        Ok(())
                    }
                }
                Some("solana") => {
                    if !base58(address, 32) {
                        Err("address is not a Solana address")
                    } else if !base58(signature, 64) {
                        Err("signature is not a wallet signature")
                    } else {
                        Ok(())
                    }
                }
                Some(_) => Err("family is evm or solana"),
            },
            Op::CoinKey {
                coin,
                key,
                view_key,
            } => match (coin.as_str(), view_key) {
                ("xmr", Some(view_key)) => crate::monero_key::check(key, view_key),
                ("xmr", None) => Err("view_key is required for Monero"),
                (_, Some(_)) => Err("view_key is only for Monero"),
                ("zec", None) => crate::zcash_key::check(key),
                (c, None) if !COINS.contains(&c) => Err("coin is not one the market takes"),
                (_, None) if !coin_key(key) => {
                    Err("key is not an extended public key or a descriptor")
                }
                _ => Ok(()),
            },
            Op::CoinPay { order_id, coin } => {
                if !valid_id(order_id) {
                    Err("order_id is not valid")
                } else if !COINS.contains(&coin.as_str()) {
                    Err("coin is not one the market takes")
                } else {
                    Ok(())
                }
            }
            Op::Confirm {
                order_id,
                tx,
                network,
            } => {
                let tx_ok = if is_solana(network.as_deref()) {
                    base58(tx, 64)
                } else {
                    eth_hash(tx)
                };
                if !valid_id(order_id) {
                    Err("order_id is not valid")
                } else if network.as_deref().is_some_and(|n| !network_key(n)) {
                    Err("network is not a network name")
                } else if !tx_ok {
                    Err("tx is not a transaction hash")
                } else {
                    Ok(())
                }
            }
            Op::FeePay { tx, network } => {
                if network.as_deref().is_some_and(|n| !network_key(n)) {
                    Err("network is not a network name")
                } else if is_solana(network.as_deref()) {
                    Err("coin fees are paid in USDC on an EVM network")
                } else if !eth_hash(tx) {
                    Err("tx is not a transaction hash")
                } else {
                    Ok(())
                }
            }
            Op::Rpc {
                method,
                params,
                network,
            } => {
                let methods = if is_solana(network.as_deref()) {
                    SOLANA_METHODS
                } else {
                    RPC_METHODS
                };
                if network.as_deref().is_some_and(|n| !network_key(n)) {
                    Err("network is not a network name")
                } else if !methods.contains(&method.as_str()) {
                    Err("that chain method is not forwarded")
                } else if !params.starts_with('[') || params.len() > 8_000 {
                    Err("chain params must be a short array")
                } else {
                    Ok(())
                }
            }
        }
    }

    /// `(method, registrar path)`.
    #[must_use]
    pub fn route(&self) -> (&'static str, &'static str) {
        match self {
            Op::Browse => ("GET", "/market/listings"),
            Op::Account => ("POST", "/market/account"),
            Op::Onboard { .. } => ("POST", "/market/seller/onboard"),
            Op::List { .. } => ("POST", "/market/listings"),
            Op::Close { .. } => ("POST", "/market/listings/close"),
            Op::Order { .. } => ("POST", "/market/orders"),
            Op::Payout { .. } => ("POST", "/market/payout"),
            Op::Confirm { .. } => ("POST", "/market/confirm"),
            Op::FeePay { .. } => ("POST", "/market/fees/pay"),
            Op::CoinKey { .. } => ("POST", "/market/coins/key"),
            Op::CoinPay { .. } => ("POST", "/market/coins/pay"),
            Op::Rpc { .. } => ("POST", "/market/rpc"),
            Op::Domains => ("POST", "/domains/list"),
            Op::DomainAdd { .. } => ("POST", "/domains/add"),
            Op::DomainRemove { .. } => ("POST", "/domains/remove"),
            Op::Hardware => ("GET", "/market/hardware"),
            Op::HardwareCheckout { .. } => ("POST", "/market/hardware/checkout"),
            Op::Deregister => ("POST", "/deregister"),
            Op::BuilderAccount => ("POST", "/builder/account"),
            Op::BuilderCredit { .. } => ("POST", "/builder/credits"),
            Op::BuilderStart { .. } => ("POST", "/builder/builds"),
            Op::BuilderBuild { .. } => ("POST", "/builder/build"),
            Op::VmImages => ("GET", "/market/vm-images"),
            Op::VmStatus => ("POST", "/market/vms/status"),
            Op::VmOrder { .. } => ("POST", "/market/orders"),
            Op::VmTicket { .. } => ("POST", "/market/vm-images/ticket"),
            Op::VmRemove { .. } => ("POST", "/market/vm-images/remove"),
        }
    }

    /// The JSON body, credentials included, or `None` for the anonymous GET.
    ///
    /// The token is in the body and the body goes to curl on stdin, so it is
    /// never in an argument vector, where `ps` would show it.
    #[must_use]
    pub fn body(&self, appliance_id: &str, token: &str) -> Option<String> {
        let mut doc = json!({ "appliance_id": appliance_id, "token": token });
        let extra = match self {
            Op::Browse | Op::Hardware | Op::VmImages => return None,
            Op::VmStatus => json!({}),
            Op::VmOrder {
                listing_id,
                quantity,
                image,
                name,
                user_data,
            } => {
                let mut order = json!({
                    "listing_id": listing_id,
                    "quantity": quantity,
                    "image": image,
                    "name": name.trim(),
                });
                if let Some(u) = user_data {
                    order["user_data"] = json!(u);
                }
                order
            }
            Op::VmTicket { name, efi } => json!({ "name": name.trim(), "efi": efi }),
            Op::VmRemove { upload_id } => json!({ "upload_id": upload_id }),
            Op::HardwareCheckout { items } => json!({
                "items": items
                    .iter()
                    .map(|(sku, quantity)| json!({ "sku": sku, "quantity": quantity }))
                    .collect::<Vec<_>>(),
            }),
            Op::Account | Op::Domains | Op::Deregister | Op::BuilderAccount => json!({}),
            Op::BuilderCredit { amount } => json!({ "amount": amount }),
            Op::BuilderStart { prompt, base, lang } => {
                let mut extra = json!({ "prompt": prompt.trim() });
                if let Some(files) = base {
                    extra["base_files"] = json!(files);
                    // An edge from before widgets had files reads `base`,
                    // the entry, and ignores the rest.
                    if let Some(entry) = files.iter().find(|f| f.name == crate::look::ENTRY_FILE) {
                        extra["base"] = json!(entry.content);
                    }
                }
                if let Some(lang) = lang {
                    extra["lang"] = json!(lang);
                }
                extra
            }
            Op::BuilderBuild { build_id } => json!({ "build_id": build_id }),
            Op::DomainAdd { domain } | Op::DomainRemove { domain } => json!({ "domain": domain }),
            Op::Onboard { box_uuid } => match box_uuid {
                Some(u) => json!({ "box_uuid": u }),
                None => json!({}),
            },
            Op::List {
                kind,
                unit_price,
                capacity,
            } => json!({ "kind": kind, "unit_price": unit_price, "capacity": capacity }),
            Op::Close { listing_id } => json!({ "listing_id": listing_id }),
            Op::Order {
                listing_id,
                quantity,
            } => json!({ "listing_id": listing_id, "quantity": quantity }),
            Op::Payout {
                address,
                signature,
                family,
            } => {
                let mut doc = json!({ "address": address, "signature": signature });
                if let Some(f) = family {
                    doc["family"] = json!(f);
                }
                doc
            }
            Op::CoinKey {
                coin,
                key,
                view_key: None,
            } => json!({ "coin": coin, "key": key }),
            Op::CoinKey {
                coin,
                key,
                view_key: Some(view_key),
            } => json!({ "coin": coin, "key": key, "view_key": view_key }),
            Op::CoinPay { order_id, coin } => json!({ "order_id": order_id, "coin": coin }),
            Op::Confirm {
                order_id,
                tx,
                network,
            } => {
                let mut doc = json!({ "order_id": order_id, "tx": tx });
                if let Some(n) = network {
                    doc["network"] = json!(n);
                }
                doc
            }
            Op::FeePay { tx, network } => {
                let mut doc = json!({ "tx": tx });
                if let Some(n) = network {
                    doc["network"] = json!(n);
                }
                doc
            }
            Op::Rpc {
                method,
                params,
                network,
            } => {
                let parsed = serde_json::from_str::<Value>(params).unwrap_or_else(|_| json!([]));
                let mut doc = json!({ "method": method, "params": parsed });
                if let Some(n) = network {
                    doc["network"] = json!(n);
                }
                doc
            }
        };
        if let (Some(d), Some(e)) = (doc.as_object_mut(), extra.as_object()) {
            d.extend(e.clone());
        }
        Some(doc.to_string())
    }
}

/// An answer the owner can act on, passed through to the browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// 400 or 409; never 404, which the SPA reads as "route not served".
    pub status: u16,
    pub message: String,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Refused {}

/// What the registrar said.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The market is not offered to this box, or this box has no registrar.
    Unavailable,
    Reply(Value),
}

/// Read a registrar response.
///
/// # Errors
/// [`Refused`] for an owner-actionable 4xx; any other `anyhow` error for
/// everything that is a fault rather than a refusal (a rejected token, a 5xx,
/// a body that is not JSON), whose text is for the journal.
pub fn classify(status: u16, body: &str) -> anyhow::Result<Outcome> {
    match status {
        200..=299 => {
            if body.trim().is_empty() {
                return Ok(Outcome::Reply(json!({})));
            }
            let v = serde_json::from_str(body)
                .map_err(|_| anyhow::anyhow!("the registrar answered {status} with non-JSON"))?;
            Ok(Outcome::Reply(v))
        }
        // 503: the edge runs no market. 403: it does, but not for this box.
        403 | 503 => Ok(Outcome::Unavailable),
        400 | 404 | 409 | 422 => Err(Refused {
            status: if status == 409 { 409 } else { 400 },
            message: public_message(body),
        }
        .into()),
        401 => anyhow::bail!("the registrar rejected this appliance's proxy token"),
        _ => anyhow::bail!("the registrar answered {status}"),
    }
}

/// The registrar's own sentence, if it is a short printable one.
pub(crate) fn public_message(body: &str) -> String {
    let text = body.trim();
    let text = serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| text.to_string());
    if text.is_empty() || text.chars().count() > MAX_MESSAGE || text.chars().any(char::is_control) {
        "the market refused the request".to_string()
    } else {
        text
    }
}

/// The only hosts the owner's browser is ever sent to: Stripe-hosted Checkout
/// and Connect onboarding (the account link). Custom Checkout domains are not
/// supported.
pub const STRIPE_HOSTS: [&str; 2] = ["checkout.stripe.com", "connect.stripe.com"];

/// Whether a Checkout or onboarding URL is one the owner's browser should be
/// sent to.
///
/// A plain `https://` test is not enough: the registrar is exactly the party
/// the Stripe gate is built to distrust, and a compromised one could hand back
/// any HTTPS page dressed up as a card form. So the host must be one of
/// [`STRIPE_HOSTS`], exactly: no port, no userinfo (`https://checkout.stripe.com@evil/`
/// names the host `evil`), and no lookalike suffix.
#[must_use]
pub fn stripe_hosted_url(url: &str) -> bool {
    if url
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return false;
    }
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    STRIPE_HOSTS.contains(&authority)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registrar_must_be_https_because_the_token_travels_in_the_body() {
        assert!(Config::from_parts("https://register.losos.cfd/", "box", "/t").is_some());
        assert_eq!(
            Config::from_parts("https://register.losos.cfd/", "box", "/t")
                .unwrap()
                .registrar_url,
            "https://register.losos.cfd"
        );
        assert!(Config::from_parts("http://register.losos.cfd", "box", "/t").is_none());
        assert!(Config::from_parts("https://", "box", "/t").is_none());
    }

    /// Every call viem makes to send a transaction and wait for its receipt.
    /// One missing method fails the payment after the money has moved.
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
            let op = Op::Rpc {
                method: method.to_string(),
                params: "[]".to_string(),
                network: None,
            };
            assert!(op.validate().is_ok(), "{method} is relayed");
        }
    }

    /// What the Solana payer calls: a blockhash, the accounts it may have to
    /// create, the send, and the wait.
    #[test]
    fn a_solana_wallet_can_send_and_wait_through_the_relay() {
        let rpc = |method: &str, network: Option<&str>| Op::Rpc {
            method: method.to_string(),
            params: "[]".to_string(),
            network: network.map(str::to_string),
        };
        for method in [
            "getLatestBlockhash",
            "getTokenAccountBalance",
            "getBalance",
            "sendTransaction",
            "getSignatureStatuses",
        ] {
            assert!(
                rpc(method, Some("solana-devnet")).validate().is_ok(),
                "{method}"
            );
            // Not to an EVM chain.
            assert!(
                rpc(method, Some("base-sepolia")).validate().is_err(),
                "{method}"
            );
        }
        assert!(rpc("eth_sendRawTransaction", Some("solana-devnet"))
            .validate()
            .is_err());
        assert!(rpc("eth_call", Some("ens")).validate().is_ok());
        assert!(rpc("eth_call", Some("../x")).validate().is_err());
        let body = rpc("getSlot", Some("solana-devnet"))
            .body("appliance-1", "tok")
            .unwrap();
        let doc: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(doc["network"], "solana-devnet");
        assert_eq!(doc["method"], "getSlot");

        let sig = "5".repeat(87);
        let confirm = |tx: &str, network: Option<&str>| Op::Confirm {
            order_id: "ord_ab12".to_string(),
            tx: tx.to_string(),
            network: network.map(str::to_string),
        };
        assert!(confirm(&sig, Some("solana-devnet")).validate().is_ok());
        assert!(confirm(&sig, None).validate().is_err());
        let hash = format!("0x{}", "ab".repeat(32));
        assert!(confirm(&hash, Some("arbitrum-sepolia")).validate().is_ok());
        assert!(confirm(&hash, Some("solana-devnet")).validate().is_err());

        let fees = |tx: &str, network: Option<&str>| Op::FeePay {
            tx: tx.to_string(),
            network: network.map(str::to_string),
        };
        assert!(fees(&hash, None).validate().is_ok());
        assert!(fees(&hash, Some("base-sepolia")).validate().is_ok());
        assert_eq!(
            fees(&sig, Some("solana-devnet")).validate(),
            Err("coin fees are paid in USDC on an EVM network")
        );
        assert!(fees(&sig, None).validate().is_err());
        assert!(fees(&hash, Some("../x")).validate().is_err());
        assert_eq!(fees(&hash, None).route(), ("POST", "/market/fees/pay"));
        let doc: Value = serde_json::from_str(
            &fees(&hash, Some("base"))
                .body("appliance-1", "tok")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(doc["tx"], hash);
        assert_eq!(doc["network"], "base");
        assert_eq!(doc["token"], "tok");
        let doc: Value =
            serde_json::from_str(&fees(&hash, None).body("appliance-1", "tok").unwrap()).unwrap();
        assert!(doc.get("network").is_none());

        let payout = |address: &str, signature: &str, family: Option<&str>| Op::Payout {
            address: address.to_string(),
            signature: signature.to_string(),
            family: family.map(str::to_string),
        };
        let key = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
        assert!(payout(key, &sig, Some("solana")).validate().is_ok());
        assert!(payout(key, &sig, None).validate().is_err());
        assert!(payout("0OIl".repeat(10).as_str(), &sig, Some("solana"))
            .validate()
            .is_err());
        assert!(payout(key, &sig, Some("bitcoin")).validate().is_err());
    }

    #[test]
    fn operations_are_validated_before_they_leave_the_box() {
        let list = |kind: &str, p, c| Op::List {
            kind: kind.to_string(),
            unit_price: p,
            capacity: c,
        };
        assert!(list("storage", 100, 10).validate().is_ok());
        assert!(list("compute", 100, 10).validate().is_ok());
        assert!(list("bandwidth", 100, 10).validate().is_err());
        assert!(list("storage", 0, 10).validate().is_err());
        assert!(list("storage", 100, 0).validate().is_err());
        let order = |id: &str, q| Op::Order {
            listing_id: id.to_string(),
            quantity: q,
        };
        assert!(order("lst_ab12", 3).validate().is_ok());
        assert!(order("lst_ab12", 0).validate().is_err());
        assert!(order("../x", 1).validate().is_err());
        assert!(order("", 1).validate().is_err());
        assert!(Op::Close {
            listing_id: "LST".to_string()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn domain_operations_relay_to_the_domain_routes_with_the_name_in_the_body() {
        let add = Op::DomainAdd {
            domain: "cloud.example.org".to_string(),
        };
        assert!(add.validate().is_ok());
        assert_eq!(add.route(), ("POST", "/domains/add"));
        let body: Value = serde_json::from_str(&add.body("box", "tok").unwrap()).unwrap();
        assert_eq!(
            body,
            json!({ "appliance_id": "box", "token": "tok", "domain": "cloud.example.org" })
        );
        assert_eq!(Op::Domains.route(), ("POST", "/domains/list"));
        for bad in [
            "",
            "localhost",
            "Cloud.example.org",
            "a b.org",
            "x.org/../y",
            "é.org",
        ] {
            assert!(
                Op::DomainRemove {
                    domain: bad.to_string()
                }
                .validate()
                .is_err(),
                "{bad:?}"
            );
        }
        let long = format!("{}.org", "a".repeat(250));
        assert!(Op::DomainAdd { domain: long }.validate().is_err());
    }

    #[test]
    fn credentials_ride_in_the_body_and_browse_is_anonymous() {
        assert_eq!(Op::Browse.body("box", "tok"), None);
        let body: Value = serde_json::from_str(&Op::Account.body("box", "tok").unwrap()).unwrap();
        assert_eq!(body, json!({ "appliance_id": "box", "token": "tok" }));
        let body: Value = serde_json::from_str(
            &Op::Order {
                listing_id: "lst_1".to_string(),
                quantity: 4,
            }
            .body("box", "tok")
            .unwrap(),
        )
        .unwrap();
        assert_eq!(body["listing_id"], "lst_1");
        assert_eq!(body["quantity"], 4);
        assert_eq!(body["token"], "tok");
    }

    #[test]
    fn the_onboard_body_carries_the_box_uuid_only_when_set() {
        let none: Value =
            serde_json::from_str(&Op::Onboard { box_uuid: None }.body("a", "t").unwrap()).unwrap();
        assert!(none.get("box_uuid").is_none());
        let id = "3f2b8c1e-7a4d-4e9b-9c15-0d6a2b7e4f31";
        let some: Value = serde_json::from_str(
            &Op::Onboard {
                box_uuid: Some(id.to_string()),
            }
            .body("a", "t")
            .unwrap(),
        )
        .unwrap();
        assert_eq!(some["box_uuid"], id);
        assert!(Op::Onboard {
            box_uuid: Some("nope".to_string())
        }
        .validate()
        .is_err());
    }

    #[test]
    fn a_hardware_order_carries_skus_and_quantities_only() {
        let op = Op::HardwareCheckout {
            items: vec![("box".to_string(), 3), ("gateway".to_string(), 1)],
        };
        assert!(op.validate().is_ok());
        assert_eq!(op.route(), ("POST", "/market/hardware/checkout"));
        let body: Value = serde_json::from_str(&op.body("box", "tok").unwrap()).unwrap();
        assert_eq!(
            body,
            json!({ "appliance_id": "box", "token": "tok", "items": [
                { "sku": "box", "quantity": 3 }, { "sku": "gateway", "quantity": 1 } ] })
        );
        assert_eq!(Op::Hardware.route(), ("GET", "/market/hardware"));
        assert_eq!(Op::Hardware.body("box", "tok"), None);
        let bad = |items: Vec<(&str, u64)>| {
            Op::HardwareCheckout {
                items: items.into_iter().map(|(s, q)| (s.to_string(), q)).collect(),
            }
            .validate()
            .is_err()
        };
        assert!(bad(vec![]));
        assert!(bad(vec![("box", 0)]));
        assert!(bad(vec![("box", 21)]));
        assert!(bad(vec![("Box", 1)]));
        assert!(bad(vec![("../x", 1)]));
        assert!(bad(vec![("box", 1); 9]));
    }

    #[test]
    fn builder_operations_carry_only_what_the_edge_needs() {
        use crate::look::WidgetFile;
        let start = |prompt: &str, base: Option<&str>, lang: Option<&str>| Op::BuilderStart {
            prompt: prompt.to_string(),
            base: base.map(|b| {
                vec![
                    WidgetFile::entry(b),
                    WidgetFile {
                        name: "app.js".to_string(),
                        content: "1".to_string(),
                    },
                ]
            }),
            lang: lang.map(str::to_string),
        };
        let op = start("  a clock\nwith seconds ", Some("<div></div>"), Some("sk"));
        assert!(op.validate().is_ok());
        assert_eq!(op.route(), ("POST", "/builder/builds"));
        let body: Value = serde_json::from_str(&op.body("box", "tok").unwrap()).unwrap();
        assert_eq!(
            body,
            json!({ "appliance_id": "box", "token": "tok", "prompt": "a clock\nwith seconds",
                    "base_files": [
                        { "name": "index.html", "content": "<div></div>" },
                        { "name": "app.js", "content": "1" },
                    ],
                    "base": "<div></div>", "lang": "sk" })
        );
        // No base is a new widget.
        let body: Value =
            serde_json::from_str(&start("x", None, None).body("b", "t").unwrap()).unwrap();
        assert!(body.get("base").is_none() && body.get("lang").is_none());
        // A base the box would not keep is not sent.
        assert!(start("x", Some("  "), None).validate().is_err());

        assert!(start(" ", None, None).validate().is_err());
        assert!(start(&"x".repeat(2001), None, None).validate().is_err());
        assert!(start(&"é".repeat(2000), None, None).validate().is_ok());
        assert!(start("a\u{1b}[2J", None, None).validate().is_err());
        assert!(start("x", None, Some("fr")).validate().is_err());
        let big = "x".repeat(crate::look::MAX_WIDGET_BYTES + 1);
        assert!(start("x", Some(&big), None).validate().is_err());

        let credit = |amount| Op::BuilderCredit { amount };
        assert!(credit(500).validate().is_ok());
        assert!(credit(0).validate().is_err());
        assert!(credit(1_000_000).validate().is_err());
        let body: Value = serde_json::from_str(&credit(500).body("b", "t").unwrap()).unwrap();
        assert_eq!(body["amount"], 500);
        assert_eq!(credit(500).route(), ("POST", "/builder/credits"));

        let build = |id: &str| Op::BuilderBuild {
            build_id: id.to_string(),
        };
        assert!(build("bld_0a1b2c").validate().is_ok());
        assert!(build("../x").validate().is_err());
        assert_eq!(build("bld_1").route(), ("POST", "/builder/build"));
        assert_eq!(Op::BuilderAccount.route(), ("POST", "/builder/account"));
    }

    #[test]
    fn routes_match_the_registrar() {
        assert_eq!(Op::Browse.route(), ("GET", "/market/listings"));
        assert_eq!(
            Op::Onboard { box_uuid: None }.route(),
            ("POST", "/market/seller/onboard")
        );
        assert_eq!(
            Op::Close {
                listing_id: "l".to_string()
            }
            .route(),
            ("POST", "/market/listings/close")
        );
        assert_eq!(Op::Deregister.route(), ("POST", "/deregister"));
        let pay = Op::CoinPay {
            order_id: "ord_1".to_string(),
            coin: "btc".to_string(),
        };
        assert_eq!(pay.route(), ("POST", "/market/coins/pay"));
    }

    #[test]
    fn coin_keys_are_public_keys_only() {
        let key = |coin: &str, key: &str| Op::CoinKey {
            coin: coin.to_string(),
            key: key.to_string(),
            view_key: None,
        };
        let zpub = "zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH1r1ADqtfSdVCToUG868RvUUkgDKf31mGDtKsAYz2oz2AGutZYs";
        assert!(key("btc", zpub).validate().is_ok());
        assert!(key(
            "btc",
            "wpkh([73c5da0a/84h/1h/0h]tpubD6NzVbkrYhZ4X/0/*)#abcdefgh"
        )
        .validate()
        .is_ok());
        assert!(key("doge", zpub).validate().is_err());
        assert!(key(
            "btc",
            "xprvNotARealKeyOnlyTheShapeOfOneNotARealKeyOnlyTheShapeOfOneNotARealKeyOnlyTheShapeOfOneNotARealKeyOnlyTheShapeOfOne"
        )
        .validate()
        .is_err());
        assert!(key("btc", "wpkh(a b)").validate().is_err());
        assert!(key("btc", "").validate().is_err());
        // Monero needs its view key, and nothing else takes one.
        assert!(key("xmr", "4").validate().is_err());
        let with_view = Op::CoinKey {
            coin: "btc".to_string(),
            key: zpub.to_string(),
            view_key: Some("00".repeat(32)),
        };
        assert!(with_view.validate().is_err());
        // Zcash takes a unified viewing key alone; a spending key never
        // leaves the box.
        assert!(key("zec", "uview1qqqqqqqqqqqqqqqqqqqqqqqq")
            .validate()
            .is_ok());
        assert!(key("zec", "secret-orchard-sk-main1qqqqqqqqqqqqqqqq")
            .validate()
            .unwrap_err()
            .contains("spending key"));
        assert!(key("zec", zpub).validate().is_err());
        let zec_with_view = Op::CoinKey {
            coin: "zec".to_string(),
            key: "uview1qqqqqqqqqqqqqqqqqqqqqqqq".to_string(),
            view_key: Some("00".repeat(32)),
        };
        assert!(zec_with_view.validate().is_err());
        let pay = |order: &str, coin: &str| Op::CoinPay {
            order_id: order.to_string(),
            coin: coin.to_string(),
        };
        assert!(pay("ord_1", "btc").validate().is_ok());
        assert!(pay("../x", "btc").validate().is_err());
    }

    #[test]
    fn a_market_that_is_off_or_not_for_this_box_is_unavailable_not_an_error() {
        assert_eq!(
            classify(503, "market not configured").unwrap(),
            Outcome::Unavailable
        );
        assert_eq!(classify(403, "").unwrap(), Outcome::Unavailable);
    }

    #[test]
    fn success_parses_json_and_tolerates_an_empty_body() {
        assert_eq!(
            classify(200, r#"{"a":1}"#).unwrap(),
            Outcome::Reply(json!({ "a": 1 }))
        );
        assert_eq!(classify(204, "").unwrap(), Outcome::Reply(json!({})));
        assert!(classify(200, "<html>").is_err());
    }

    #[test]
    fn owner_actionable_refusals_keep_the_registrars_words_and_never_become_404() {
        let e = classify(409, "not enough units available").unwrap_err();
        let r = e.downcast_ref::<Refused>().expect("a refusal");
        assert_eq!(
            (r.status, r.message.as_str()),
            (409, "not enough units available")
        );
        let e = classify(404, "no such listing or order").unwrap_err();
        assert_eq!(e.downcast_ref::<Refused>().unwrap().status, 400);
        let e = classify(400, "x\u{7}y").unwrap_err();
        assert_eq!(
            e.downcast_ref::<Refused>().unwrap().message,
            "the market refused the request"
        );
    }

    #[test]
    fn faults_are_not_refusals() {
        for status in [401, 500, 502] {
            let e = classify(status, "boom").unwrap_err();
            assert!(e.downcast_ref::<Refused>().is_none(), "{status}");
        }
    }

    #[test]
    fn only_stripe_hosted_pages_are_followed() {
        assert!(stripe_hosted_url("https://checkout.stripe.com/c/pay/cs_1"));
        assert!(stripe_hosted_url(
            "https://checkout.stripe.com/c/pay/cs_1#fragment"
        ));
        assert!(stripe_hosted_url(
            "https://connect.stripe.com/setup/e/acct_1/abc"
        ));
        for bad in [
            "http://checkout.stripe.com/x",
            "javascript:alert(1)",
            "https://a b",
            "https://example.com/pay",
            "https://checkout.stripe.com.evil.example/x",
            "https://checkout.stripe.com@evil.example/x",
            "https://user@checkout.stripe.com/x",
            "https://evil.example/https://checkout.stripe.com/",
            "https://checkout.stripe.com:8443/x",
            "https://CHECKOUT.stripe.com/x",
            "https://checkout.stripe.com\\@evil.example/",
            "https://",
        ] {
            assert!(!stripe_hosted_url(bad), "{bad}");
        }
    }
}
