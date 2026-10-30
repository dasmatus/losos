//! The market, as the admin UI reaches it.
//!
//! The market itself lives on the edge (`edge/registrar/src/market.rs`),
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

/// The registrar's `stablecoin::ENS_CCIP_METHOD`: an ENS lookup's offchain
/// step (EIP-3668), relayed to ENS's DNS gateway by the edge.
const ENS_CCIP_METHOD: &str = "ens_ccipRead";

/// Solana methods the box will relay, the registrar's list
/// (`edge/registrar/src/solana.rs`).
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
#[derive(Debug, Clone, PartialEq)]
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
    /// Set the key of the backup door of storage this box bought
    /// (`crate::backup::door_key`). The secret it hashes never leaves.
    StorageDoor {
        order_id: String,
        door: String,
    },
    /// Pay this box the sales whose slots are over, without waiting for the
    /// edge to do it a day later. The edge refuses (409) while none are.
    Release,
    /// Tell the edge a USDC transfer on the EVM `network` paid this seller's
    /// platform fees on coin sales into the edge's treasury.
    FeePay {
        tx: String,
        network: Option<String>,
    },
    /// Tell the edge this box's escrow keys and read the decisions on
    /// escrowed deposits it waits on this box to sign. Never from the
    /// browser: lososd's own signing pass sends it.
    EscrowDecisions {
        evm: String,
        solana: String,
    },
    /// This box's signature over one decision, the body
    /// [`crate::escrow_key::Keys::sign`] made.
    EscrowSign(Value),
    /// One allowlisted chain call to `network`, or to the name endpoint for
    /// `ens`. `params` is a JSON array. The answer is the JSON-RPC object,
    /// not a market view.
    Rpc {
        method: String,
        params: String,
        network: Option<String>,
    },
    /// This box's custom domains on the edge, and whether it may add one
    /// (`edge/registrar/src/domains.rs`). Not the market, but the same
    /// relay: the edge wants the proxy token, and only an official edge is
    /// asked.
    Domains,
    DomainAdd {
        domain: String,
    },
    DomainRemove {
        domain: String,
    },
    /// Publish this box's wallet at its names in the edge's zone, with the
    /// wallet's `personal_sign` of the binding message the edge checks
    /// (`edge/registrar/src/domains.rs`, `wallet_message`); both `None`
    /// takes it off.
    DomainWallet {
        address: Option<String>,
        signature: Option<String>,
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
    /// The machine images the edge offers, the replica's shape and the
    /// split (`edge/registrar/src/vms.rs`). Anonymous.
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
    /// Buy `euros` of USDC on Base at Binance, delivered to `address`, this
    /// box's wallet (`edge/registrar/src/onramp.rs`). The answer carries
    /// Binance's checkout link.
    Onramp {
        address: String,
        euros: String,
    },
    /// Where one of this box's Binance orders is.
    OnrampStatus {
        order_id: String,
    },
    /// The AI models the edge runs on lent memory, priced for buyers paying
    /// on `network`, with this box's meters, escrow balances and lending
    /// (`edge/registrar/src/server/llm.rs`).
    LlmModels {
        network: Option<String>,
    },
    /// Quote a meter for `model`: one tap per lending box, which the wallet
    /// then opens in the escrow. `spend_per_hour` is the most the meter may
    /// bill in an hour, USDC base units.
    TapQuote {
        model: String,
        network: Option<String>,
        spend_per_hour: u64,
    },
    /// `wallet` opened the meter's taps; the edge reads them back from the
    /// chain, where they are kept under that wallet, and opens the meter.
    TapConfirm {
        meter: String,
        wallet: String,
    },
    /// Stop a meter. The edge closes its taps itself; the wallet signs
    /// nothing. `wallet` names the wallet that began opening a quote's taps
    /// and failed partway, so the edge closes the ones it opened.
    TapStop {
        meter: String,
        wallet: Option<String>,
    },
    /// One request to a running model, billed per token on `meter`.
    LlmChat(ChatAsk),
}

/// A buyer's request to a model, as it leaves the box. The edge checks it
/// again against the model's context length (`crate::inference::chat_ask`
/// there); these are its limits that do not depend on the model.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatAsk {
    pub meter: String,
    /// `(role, content)`.
    pub messages: Vec<(String, String)>,
    pub max_tokens: Option<u64>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub stop: Vec<String>,
}

/// The edge's limits on one request to a model.
pub const MAX_CHAT_MESSAGES: usize = 256;
pub const MAX_PROMPT_BYTES: usize = 256 * 1024;
const MAX_STOP_STRINGS: usize = 4;
const MAX_STOP_LEN: usize = 64;
/// No model has a context this long; the edge holds a request to its own.
const MAX_ANSWER_TOKENS: u64 = 1 << 20;
/// The edge's bounds on a meter's hourly limit (`MIN_SPEND_PER_HOUR` and
/// `MAX_SPEND_PER_HOUR` in edge/registrar/src/meter.rs): a cent to a
/// hundred dollars, in USDC base units.
pub const MIN_SPEND_PER_HOUR: u64 = 10_000;
pub const MAX_SPEND_PER_HOUR: u64 = 100_000_000;

/// How long the box waits for most registrar answers. Above the registrar's
/// own budget for the routes that wait on Stripe (`STRIPE_ROUTE_TIMEOUT` in
/// edge/registrar/src/server.rs, 22 s: four gate calls of 5 s plus
/// persistence), so a slow but successful onboarding is not dropped here
/// after the account was already made.
const TIMEOUT_SECS: u64 = 25;
/// A confirm reads every tap back from the chain, which the edge allows
/// about 20 s for.
const CONFIRM_TIMEOUT_SECS: u64 = 45;
/// An answer from a large model split across boxes can take many minutes;
/// the edge gives up after 16 (`ANSWER_SECS` plus a minute in
/// edge/registrar/src/server/llm.rs), so the box waits a little longer and
/// hears the edge's own reason rather than its own timeout.
pub const CHAT_TIMEOUT_SECS: u64 = 17 * 60;
/// A listing page is small; a registrar that sends more is wrong.
const MAX_REPLY_BYTES: usize = 1024 * 1024;
/// A model's answer is bounded by the edge's 4 MiB read of the head's reply.
const MAX_CHAT_REPLY_BYTES: usize = 5 * 1024 * 1024;

/// A model id in the edge's catalogue: `[a-z0-9-]`, 1 to 64 characters.
fn valid_model(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A meter id the edge made: `mtr_` and lowercase hex.
fn valid_meter(id: &str) -> bool {
    id.strip_prefix("mtr_").is_some_and(|hex| {
        !hex.is_empty()
            && hex.len() <= 48
            && hex
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    })
}

fn valid_network(network: Option<&str>) -> bool {
    network.is_none_or(network_key)
}

impl ChatAsk {
    fn validate(&self) -> Result<(), &'static str> {
        let unit =
            |v: Option<f64>, hi: f64| v.is_none_or(|x| x.is_finite() && (0.0..=hi).contains(&x));
        if !valid_meter(&self.meter) {
            Err("meter is not valid")
        } else if self.messages.is_empty() || self.messages.len() > MAX_CHAT_MESSAGES {
            Err("send one to 256 messages")
        } else if self
            .messages
            .iter()
            .any(|(role, _)| !matches!(role.as_str(), "system" | "user" | "assistant"))
        {
            Err("a message's role is system, user or assistant")
        } else if self.messages.iter().map(|(_, c)| c.len()).sum::<usize>() > MAX_PROMPT_BYTES {
            Err("the messages are too long")
        } else if self
            .max_tokens
            .is_some_and(|n| n == 0 || n > MAX_ANSWER_TOKENS)
        {
            Err("maxTokens is from one to the model's context length")
        } else if !unit(self.temperature, 2.0) || !unit(self.top_p, 1.0) {
            Err("a sampling setting is out of range")
        } else if self.stop.len() > MAX_STOP_STRINGS
            || self
                .stop
                .iter()
                .any(|s| s.is_empty() || s.len() > MAX_STOP_LEN)
        {
            Err("at most four stop strings, each short")
        } else {
            Ok(())
        }
    }

    fn body(&self) -> Value {
        let mut doc = json!({
            "meter": self.meter,
            "messages": self
                .messages
                .iter()
                .map(|(role, content)| json!({ "role": role, "content": content }))
                .collect::<Vec<_>>(),
        });
        if let Some(n) = self.max_tokens {
            doc["maxTokens"] = json!(n);
        }
        if let Some(t) = self.temperature {
            doc["temperature"] = json!(t);
        }
        if let Some(p) = self.top_p {
            doc["topP"] = json!(p);
        }
        if !self.stop.is_empty() {
            doc["stop"] = json!(self.stop);
        }
        doc
    }
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
/// A euro amount: up to six digits, then at most two decimals. The edge
/// checks the range.
fn valid_euros(raw: &str) -> bool {
    let (whole, cents) = raw.split_once('.').unwrap_or((raw, "0"));
    (1..=6).contains(&whole.len())
        && (1..=2).contains(&cents.len())
        && whole
            .bytes()
            .chain(cents.bytes())
            .all(|b| b.is_ascii_digit())
}

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
    /// Refuse an operation before it leaves the box.
    ///
    /// # Errors
    /// A sentence naming what was wrong, suitable for a 400.
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Op::EscrowDecisions { evm, solana } => {
                if eth_address(evm) && base58(solana, 32) {
                    Ok(())
                } else {
                    Err("the escrow keys are not keys")
                }
            }
            Op::EscrowSign(body) => {
                if body.is_object() && body["signature"].is_string() {
                    Ok(())
                } else {
                    Err("the escrow signature is not one")
                }
            }
            Op::Browse
            | Op::Account
            | Op::Domains
            | Op::Hardware
            | Op::Deregister
            | Op::Release
            | Op::VmImages
            | Op::VmStatus => Ok(()),
            Op::StorageDoor { order_id, door } => {
                if !crate::backup::order_ok(order_id) {
                    Err("order_id is not valid")
                } else if door.len() != 64
                    || !door
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    Err("door is not a SHA-256 in hex")
                } else {
                    Ok(())
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
            Op::LlmModels { network } => {
                if valid_network(network.as_deref()) {
                    Ok(())
                } else {
                    Err("network is not a network name")
                }
            }
            Op::TapQuote {
                model,
                network,
                spend_per_hour,
            } => {
                if !valid_model(model) {
                    Err("model is not a model id")
                } else if !valid_network(network.as_deref()) {
                    Err("network is not a network name")
                } else if !(MIN_SPEND_PER_HOUR..=MAX_SPEND_PER_HOUR).contains(spend_per_hour) {
                    Err("allow between one cent and a hundred dollars per hour")
                } else {
                    Ok(())
                }
            }
            Op::TapConfirm { meter, wallet } => {
                if !valid_meter(meter) {
                    Err("meter is not valid")
                } else if eth_address(wallet) || base58(wallet, 32) {
                    Ok(())
                } else {
                    Err("wallet is not a wallet address")
                }
            }
            Op::TapStop { meter, wallet } => {
                if !valid_meter(meter) {
                    Err("meter is not valid")
                } else if wallet
                    .as_deref()
                    .is_none_or(|w| eth_address(w) || base58(w, 32))
                {
                    Ok(())
                } else {
                    Err("wallet is not a wallet address")
                }
            }
            Op::LlmChat(ask) => ask.validate(),
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
            Op::Onramp { address, euros } => {
                if !eth_address(address) {
                    Err("address is not an 0x address")
                } else if !valid_euros(euros) {
                    Err("euros is an amount such as 50 or 25.50")
                } else {
                    Ok(())
                }
            }
            Op::OnrampStatus { order_id } => {
                if order_id.len() == 32
                    && order_id
                        .bytes()
                        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
                {
                    Ok(())
                } else {
                    Err("order_id is not valid")
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
            Op::DomainWallet { address, signature } => match (address, signature) {
                (None, None) => Ok(()),
                (Some(a), Some(sig)) => {
                    if !eth_address(a) {
                        Err("address is not an 0x address")
                    } else if !eth_signature(sig) {
                        Err("signature is not a wallet signature")
                    } else {
                        Ok(())
                    }
                }
                _ => Err("send the address with its signature, or neither to unbind"),
            },
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
                // An offchain name lookup goes to ENS's gateway through the
                // edge's ENS endpoint, never to a payment network.
                let ccip = method == ENS_CCIP_METHOD && network.as_deref() == Some("ens");
                if network.as_deref().is_some_and(|n| !network_key(n)) {
                    Err("network is not a network name")
                } else if !ccip && !methods.contains(&method.as_str()) {
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
            Op::Release => ("POST", "/market/sales/release"),
            Op::StorageDoor { .. } => ("POST", "/market/storage-door"),
            Op::CoinKey { .. } => ("POST", "/market/coins/key"),
            Op::CoinPay { .. } => ("POST", "/market/coins/pay"),
            Op::Rpc { .. } => ("POST", "/market/rpc"),
            Op::EscrowDecisions { .. } => ("POST", "/market/escrow/decisions"),
            Op::EscrowSign(_) => ("POST", "/market/escrow/sign"),
            Op::Domains => ("POST", "/domains/list"),
            Op::DomainAdd { .. } => ("POST", "/domains/add"),
            Op::DomainRemove { .. } => ("POST", "/domains/remove"),
            Op::DomainWallet { .. } => ("POST", "/domains/wallet"),
            Op::Hardware => ("GET", "/market/hardware"),
            Op::HardwareCheckout { .. } => ("POST", "/market/hardware/checkout"),
            Op::Deregister => ("POST", "/deregister"),
            Op::VmImages => ("GET", "/market/vm-images"),
            Op::VmStatus => ("POST", "/market/vms/status"),
            Op::VmOrder { .. } => ("POST", "/market/orders"),
            Op::VmTicket { .. } => ("POST", "/market/vm-images/ticket"),
            Op::VmRemove { .. } => ("POST", "/market/vm-images/remove"),
            Op::Onramp { .. } => ("POST", "/market/onramp"),
            Op::OnrampStatus { .. } => ("POST", "/market/onramp/status"),
            Op::LlmModels { .. } => ("POST", "/market/llm/models"),
            Op::TapQuote { .. } => ("POST", "/market/taps"),
            Op::TapConfirm { .. } => ("POST", "/market/taps/confirm"),
            Op::TapStop { .. } => ("POST", "/market/taps/stop"),
            Op::LlmChat(_) => ("POST", "/market/llm/chat"),
        }
    }

    /// Whether this is one of the pay-as-you-go AI routes, whose 503 means
    /// "not right now" (a model loading, a box restarting) rather than "no
    /// market here".
    #[must_use]
    pub fn is_llm(&self) -> bool {
        matches!(
            self,
            Op::LlmModels { .. }
                | Op::TapQuote { .. }
                | Op::TapConfirm { .. }
                | Op::TapStop { .. }
                | Op::LlmChat(_)
        )
    }

    /// How long the box waits for the registrar's answer, seconds.
    #[must_use]
    pub fn timeout_secs(&self) -> u64 {
        match self {
            Op::LlmChat(_) => CHAT_TIMEOUT_SECS,
            Op::TapConfirm { .. }
            | Op::TapStop {
                wallet: Some(_), ..
            } => CONFIRM_TIMEOUT_SECS,
            _ => TIMEOUT_SECS,
        }
    }

    /// The most the box reads of the registrar's answer.
    #[must_use]
    pub fn max_reply_bytes(&self) -> usize {
        match self {
            Op::LlmChat(_) => MAX_CHAT_REPLY_BYTES,
            _ => MAX_REPLY_BYTES,
        }
    }

    /// Read the registrar's answer to this operation; see [`classify`].
    ///
    /// # Errors
    /// As [`classify`]. On an AI route a 503 that names a reason, such as a
    /// model still loading on its boxes, is a [`Refused`] the owner can wait
    /// out, not the market being absent.
    pub fn classify(&self, status: u16, body: &str) -> anyhow::Result<Outcome> {
        if status == 503 && self.is_llm() {
            let message = public_message(body);
            if message != UNCONFIGURED && message != "the market refused the request" {
                return Err(Refused { status, message }.into());
            }
        }
        classify(status, body)
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
            Op::LlmModels { network } => match network {
                Some(n) => json!({ "network": n }),
                None => json!({}),
            },
            Op::TapQuote {
                model,
                network,
                spend_per_hour,
            } => {
                let mut doc = json!({ "model": model, "spendPerHour": spend_per_hour });
                if let Some(n) = network {
                    doc["network"] = json!(n);
                }
                doc
            }
            Op::TapConfirm { meter, wallet } => json!({ "meter": meter, "wallet": wallet }),
            Op::TapStop {
                meter,
                wallet: None,
            } => json!({ "meter": meter }),
            Op::TapStop {
                meter,
                wallet: Some(wallet),
            } => json!({ "meter": meter, "wallet": wallet }),
            Op::LlmChat(ask) => ask.body(),
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
            Op::StorageDoor { order_id, door } => json!({ "order_id": order_id, "door": door }),
            Op::Onramp { address, euros } => json!({ "address": address, "euros": euros }),
            Op::OnrampStatus { order_id } => json!({ "order_id": order_id }),
            Op::HardwareCheckout { items } => json!({
                "items": items
                    .iter()
                    .map(|(sku, quantity)| json!({ "sku": sku, "quantity": quantity }))
                    .collect::<Vec<_>>(),
            }),
            Op::Account | Op::Domains | Op::Deregister | Op::Release => json!({}),
            Op::DomainAdd { domain } | Op::DomainRemove { domain } => json!({ "domain": domain }),
            Op::DomainWallet {
                address: Some(address),
                signature: Some(signature),
            } => json!({ "address": address, "signature": signature }),
            Op::DomainWallet { .. } => json!({}),
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
            Op::EscrowDecisions { evm, solana } => json!({ "evm": evm, "solana": solana }),
            Op::EscrowSign(body) => body.clone(),
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

/// The registrar's sentence for an edge that runs no market, or no models
/// (`MarketError::Unconfigured` in edge/registrar/src/market.rs).
const UNCONFIGURED: &str = "market not configured on this edge";

/// An answer the owner can act on, passed through to the browser.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct Refused {
    /// 400 or 409; 402 or 429 from the AI routes (the balance or the hourly
    /// limit cannot cover a request, a request already runs on the meter),
    /// and 503 while a model loads. Never 404, which the SPA reads as
    /// "route not served".
    pub status: u16,
    pub message: String,
}

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
        400 | 402 | 404 | 409 | 422 | 429 => Err(Refused {
            status: match status {
                402 | 409 | 429 => status,
                _ => 400,
            },
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

/// Whether a checkout link is Binance's: https on binance.com or one of its
/// subdomains, no port, no userinfo. The edge checks the same; the box does
/// not take the edge's word for where the owner's browser goes.
#[must_use]
pub fn binance_hosted_url(url: &str) -> bool {
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
    !authority.contains(['@', ':'])
        && (authority == "binance.com"
            || authority.strip_suffix(".binance.com").is_some_and(|sub| {
                !sub.is_empty()
                    && sub
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
            }))
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
        // The offchain step of a name lookup, for ENS only.
        assert!(rpc("ens_ccipRead", Some("ens")).validate().is_ok());
        assert!(rpc("ens_ccipRead", Some("base")).validate().is_err());
        assert!(rpc("ens_ccipRead", None).validate().is_err());
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
    fn a_wallet_binding_carries_address_and_signature_or_nothing() {
        let address = format!("0x{}", "ab".repeat(20));
        let signature = format!("0x{}", "cd".repeat(65));
        let bind = Op::DomainWallet {
            address: Some(address.clone()),
            signature: Some(signature.clone()),
        };
        assert!(bind.validate().is_ok());
        assert_eq!(bind.route(), ("POST", "/domains/wallet"));
        let body: Value = serde_json::from_str(&bind.body("box", "tok").unwrap()).unwrap();
        assert_eq!(
            body,
            json!({ "appliance_id": "box", "token": "tok", "address": address, "signature": signature })
        );
        let unbind = Op::DomainWallet {
            address: None,
            signature: None,
        };
        assert!(unbind.validate().is_ok());
        let body: Value = serde_json::from_str(&unbind.body("box", "tok").unwrap()).unwrap();
        assert_eq!(body, json!({ "appliance_id": "box", "token": "tok" }));
        for (a, sig) in [
            (Some(address.clone()), None),
            (None, Some(signature.clone())),
            (Some("mattbox.eth".to_string()), Some(signature.clone())),
            (Some(address.clone()), Some("0x00".to_string())),
        ] {
            assert!(Op::DomainWallet {
                address: a,
                signature: sig
            }
            .validate()
            .is_err());
        }
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
        assert_eq!(Op::Release.route(), ("POST", "/market/sales/release"));
        assert!(Op::Release.validate().is_ok());
        let doc: Value = serde_json::from_str(&Op::Release.body("box", "tok").unwrap()).unwrap();
        assert_eq!(doc, json!({ "appliance_id": "box", "token": "tok" }));
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

    fn ask(meter: &str, messages: &[(&str, &str)]) -> ChatAsk {
        ChatAsk {
            meter: meter.to_string(),
            messages: messages
                .iter()
                .map(|(r, c)| ((*r).to_string(), (*c).to_string()))
                .collect(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: Vec::new(),
        }
    }

    const METER: &str = "mtr_0a1b2c3d4e5f60718293a4b5";
    const WALLET: &str = "0x00000000000000000000000000000000000000b1";

    #[test]
    fn the_ai_routes_match_the_registrar() {
        let quote = Op::TapQuote {
            model: "llama-70b".to_string(),
            network: Some("base".to_string()),
            spend_per_hour: 500_000,
        };
        assert_eq!(
            Op::LlmModels { network: None }.route(),
            ("POST", "/market/llm/models")
        );
        assert_eq!(quote.route(), ("POST", "/market/taps"));
        let meter = METER.to_string();
        assert_eq!(
            Op::TapConfirm {
                meter: meter.clone(),
                wallet: WALLET.to_string(),
            }
            .route(),
            ("POST", "/market/taps/confirm")
        );
        assert_eq!(
            Op::TapStop {
                meter,
                wallet: None
            }
            .route(),
            ("POST", "/market/taps/stop")
        );
        assert_eq!(
            Op::LlmChat(ask(METER, &[("user", "hi")])).route(),
            ("POST", "/market/llm/chat")
        );
    }

    #[test]
    fn ai_bodies_carry_the_credentials_in_snake_case_and_the_rest_as_the_edge_names_it() {
        let quote = Op::TapQuote {
            model: "llama-70b".to_string(),
            network: Some("solana".to_string()),
            spend_per_hour: 500_000,
        };
        let doc: Value = serde_json::from_str(&quote.body("box", "tok").unwrap()).unwrap();
        assert_eq!(
            doc,
            json!({ "appliance_id": "box", "token": "tok", "model": "llama-70b",
                    "network": "solana", "spendPerHour": 500_000 })
        );
        let doc: Value =
            serde_json::from_str(&Op::LlmModels { network: None }.body("box", "tok").unwrap())
                .unwrap();
        assert_eq!(doc, json!({ "appliance_id": "box", "token": "tok" }));
        let doc: Value = serde_json::from_str(
            &Op::TapStop {
                meter: METER.to_string(),
                wallet: None,
            }
            .body("box", "tok")
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            doc,
            json!({ "appliance_id": "box", "token": "tok", "meter": METER })
        );
        let doc: Value = serde_json::from_str(
            &Op::TapStop {
                meter: METER.to_string(),
                wallet: Some(WALLET.to_string()),
            }
            .body("box", "tok")
            .unwrap(),
        )
        .unwrap();
        assert_eq!(doc["wallet"], WALLET);

        let mut full = ask(METER, &[("system", "Be brief."), ("user", "Hello")]);
        full.max_tokens = Some(64);
        full.temperature = Some(0.5);
        full.top_p = Some(0.9);
        full.stop = vec!["\n\n".to_string()];
        let doc: Value =
            serde_json::from_str(&Op::LlmChat(full).body("box", "tok").unwrap()).unwrap();
        assert_eq!(
            doc,
            json!({ "appliance_id": "box", "token": "tok", "meter": METER,
                    "messages": [{ "role": "system", "content": "Be brief." },
                                 { "role": "user", "content": "Hello" }],
                    "maxTokens": 64, "temperature": 0.5, "topP": 0.9, "stop": ["\n\n"] })
        );
        // Unset settings stay unset, so the edge's defaults apply.
        let doc: Value = serde_json::from_str(
            &Op::LlmChat(ask(METER, &[("user", "hi")]))
                .body("box", "tok")
                .unwrap(),
        )
        .unwrap();
        for key in ["maxTokens", "temperature", "topP", "stop"] {
            assert!(doc.get(key).is_none(), "{key}");
        }
    }

    #[test]
    fn a_quote_names_a_model_and_an_hourly_limit_the_edge_takes() {
        let quote = |model: &str, network: Option<&str>, spend| Op::TapQuote {
            model: model.to_string(),
            network: network.map(str::to_string),
            spend_per_hour: spend,
        };
        assert!(quote("llama-70b", None, 500_000).validate().is_ok());
        assert!(quote("qwen3-235b", Some("base"), MIN_SPEND_PER_HOUR)
            .validate()
            .is_ok());
        assert!(quote("m", Some("solana"), MAX_SPEND_PER_HOUR)
            .validate()
            .is_ok());
        for bad in ["", "Llama", "llama_70b", "../x", "a b", &"a".repeat(65)] {
            assert_eq!(
                quote(bad, None, 500_000).validate(),
                Err("model is not a model id"),
                "{bad:?}"
            );
        }
        assert!(quote("m", None, MIN_SPEND_PER_HOUR - 1).validate().is_err());
        assert!(quote("m", None, MAX_SPEND_PER_HOUR + 1).validate().is_err());
        assert!(quote("m", Some("../x"), 500_000).validate().is_err());
        assert!(Op::LlmModels {
            network: Some("Base".to_string())
        }
        .validate()
        .is_err());
        assert!(Op::LlmModels { network: None }.validate().is_ok());
    }

    #[test]
    fn a_meter_is_one_the_edge_made() {
        let stop = |m: &str| Op::TapStop {
            meter: m.to_string(),
            wallet: None,
        };
        assert!(stop(METER).validate().is_ok());
        let stop_with = |wallet: &str| Op::TapStop {
            meter: METER.to_string(),
            wallet: Some(wallet.to_string()),
        };
        assert!(stop_with(WALLET).validate().is_ok());
        assert_eq!(
            stop_with("wallet").validate(),
            Err("wallet is not a wallet address")
        );
        let confirm = |wallet: &str| Op::TapConfirm {
            meter: METER.to_string(),
            wallet: wallet.to_string(),
        };
        assert!(confirm(WALLET).validate().is_ok());
        assert!(confirm("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
            .validate()
            .is_ok());
        for bad in [
            "",
            "0x12",
            "wallet",
            "0xZZ00000000000000000000000000000000000000",
        ] {
            assert_eq!(
                confirm(bad).validate(),
                Err("wallet is not a wallet address"),
                "{bad:?}"
            );
        }
        let sent: Value =
            serde_json::from_str(&confirm(WALLET).body("appliance-1", "tok").unwrap()).unwrap();
        assert_eq!(sent["meter"], METER);
        assert_eq!(sent["wallet"], WALLET);
        for bad in [
            "",
            "mtr_",
            "ord_ab12",
            "mtr_AB12",
            "mtr_../x",
            "mtr_ab 12",
            &format!("mtr_{}", "a".repeat(49)),
        ] {
            assert!(stop(bad).validate().is_err(), "{bad:?}");
            assert!(Op::TapConfirm {
                meter: bad.to_string(),
                wallet: WALLET.to_string(),
            }
            .validate()
            .is_err());
            assert!(Op::LlmChat(ask(bad, &[("user", "hi")])).validate().is_err());
        }
    }

    #[test]
    fn a_chat_is_held_to_the_edges_limits_before_it_leaves() {
        let ok = |a: ChatAsk| Op::LlmChat(a).validate();
        assert!(ok(ask(
            METER,
            &[("system", "s"), ("user", "u"), ("assistant", "a")]
        ))
        .is_ok());
        assert_eq!(ok(ask(METER, &[])), Err("send one to 256 messages"));
        let many: Vec<(&str, &str)> = vec![("user", "x"); MAX_CHAT_MESSAGES + 1];
        assert!(ok(ask(METER, &many)).is_err());
        let most: Vec<(&str, &str)> = vec![("user", "x"); MAX_CHAT_MESSAGES];
        assert!(ok(ask(METER, &most)).is_ok());
        assert_eq!(
            ok(ask(METER, &[("tool", "x")])),
            Err("a message's role is system, user or assistant")
        );
        let long = "x".repeat(MAX_PROMPT_BYTES / 2 + 1);
        assert_eq!(
            ok(ask(METER, &[("user", &long), ("user", &long)])),
            Err("the messages are too long")
        );
        let exact = "x".repeat(MAX_PROMPT_BYTES);
        assert!(ok(ask(METER, &[("user", &exact)])).is_ok());

        let with = |f: &dyn Fn(&mut ChatAsk)| {
            let mut a = ask(METER, &[("user", "hi")]);
            f(&mut a);
            ok(a)
        };
        assert!(with(&|a| a.max_tokens = Some(1)).is_ok());
        assert!(with(&|a| a.max_tokens = Some(0)).is_err());
        assert!(with(&|a| a.max_tokens = Some(MAX_ANSWER_TOKENS + 1)).is_err());
        assert!(with(&|a| a.temperature = Some(2.0)).is_ok());
        assert!(with(&|a| a.temperature = Some(2.1)).is_err());
        assert!(with(&|a| a.temperature = Some(-0.1)).is_err());
        assert!(with(&|a| a.temperature = Some(f64::NAN)).is_err());
        assert!(with(&|a| a.top_p = Some(1.0)).is_ok());
        assert!(with(&|a| a.top_p = Some(1.5)).is_err());
        assert!(with(&|a| a.stop = vec!["a".to_string(); 4]).is_ok());
        assert!(with(&|a| a.stop = vec!["a".to_string(); 5]).is_err());
        assert!(with(&|a| a.stop = vec![String::new()]).is_err());
        assert!(with(&|a| a.stop = vec!["a".repeat(65)]).is_err());
    }

    #[test]
    fn a_model_answer_waits_minutes_and_everything_else_seconds() {
        let chat = Op::LlmChat(ask(METER, &[("user", "hi")]));
        // Longer than the edge's own 16 minutes, so its reason arrives.
        assert!(chat.timeout_secs() > 16 * 60);
        assert!(chat.max_reply_bytes() > 4 * 1024 * 1024);
        assert_eq!(Op::Account.timeout_secs(), 25);
        assert!(
            Op::TapConfirm {
                meter: METER.to_string(),
                wallet: WALLET.to_string(),
            }
            .timeout_secs()
                > 20
        );
        assert_eq!(Op::Account.max_reply_bytes(), 1024 * 1024);
    }

    #[test]
    fn the_ai_refusals_reach_the_owner_with_their_status() {
        let chat = Op::LlmChat(ask(METER, &[("user", "hi")]));
        let refused = |op: &Op, status, body: &str| {
            let e = op.classify(status, body).unwrap_err();
            let r = e.downcast_ref::<Refused>().expect("a refusal").clone();
            (r.status, r.message)
        };
        assert_eq!(
            refused(&chat, 402, "the balance cannot cover this request"),
            (402, "the balance cannot cover this request".to_string())
        );
        assert_eq!(
            refused(&chat, 429, "a request is already running on this meter"),
            (
                429,
                "a request is already running on this meter".to_string()
            )
        );
        // A model loading is "not yet", not "no market here".
        assert_eq!(
            refused(&chat, 503, "the model is still loading on its boxes"),
            (503, "the model is still loading on its boxes".to_string())
        );
        assert_eq!(refused(&chat, 409, "this meter is not open").0, 409);
        // An edge with no models, or no market, is still unavailable.
        let models = Op::LlmModels { network: None };
        assert_eq!(
            models
                .classify(503, "market not configured on this edge")
                .unwrap(),
            Outcome::Unavailable
        );
        assert_eq!(models.classify(403, "").unwrap(), Outcome::Unavailable);
        // The other routes keep reading a 503 as no market.
        assert_eq!(
            Op::Account
                .classify(503, "the model is still loading")
                .unwrap(),
            Outcome::Unavailable
        );
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
    fn a_purchase_names_a_wallet_and_euros_and_only_binance_pages_are_followed() {
        let buy = |address: &str, euros: &str| Op::Onramp {
            address: address.to_string(),
            euros: euros.to_string(),
        };
        let wallet = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
        assert_eq!(buy(wallet, "50").route(), ("POST", "/market/onramp"));
        assert!(buy(wallet, "50").validate().is_ok());
        assert!(buy(wallet, "25.5").validate().is_ok());
        assert!(buy(wallet, "1234567").validate().is_err());
        assert!(buy(wallet, "25.").validate().is_err());
        assert!(buy(wallet, "25,50").validate().is_err());
        assert!(buy("0x1234", "50").validate().is_err());
        let body: Value =
            serde_json::from_str(&buy(wallet, "50").body("mattbox", "t").unwrap()).unwrap();
        assert_eq!(body["euros"], "50");
        assert_eq!(body["address"], wallet);
        let status = Op::OnrampStatus {
            order_id: "0123456789abcdef0123456789abcdef".into(),
        };
        assert!(status.validate().is_ok());
        assert_eq!(status.route(), ("POST", "/market/onramp/status"));
        assert!(Op::OnrampStatus {
            order_id: "../x".into()
        }
        .validate()
        .is_err());

        assert!(binance_hosted_url(
            "https://app.binance.com/uni-qr/qccnt?x=1"
        ));
        assert!(binance_hosted_url("https://binance.com/en"));
        for bad in [
            "http://app.binance.com/x",
            "https://app.binance.com.evil.example/x",
            "https://evilbinance.com/",
            "https://app.binance.com@evil.example/",
            "https://app.binance.com:444/",
            "https://.binance.com/",
        ] {
            assert!(!binance_hosted_url(bad), "{bad}");
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
