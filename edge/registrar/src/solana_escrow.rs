//! USDC held on Solana by `losos-escrow`, the program in
//! [`edge/contracts/solana`](https://github.com/dasmatus/losos/tree/main/edge/contracts/solana).
//!
//! The buyer's wallet sends one `pay` instruction. It makes a deposit
//! account at `["deposit", config, nonce]` and a token account the deposit
//! owns, and moves the order's USDC into it. The edge reads that deposit
//! back: the account itself is the record, so the edge checks the program's
//! own copy of the seller, the amounts and the deadline rather than the
//! transaction's instruction data. Later it releases, refunds or splits the
//! deposit with the releaser key; the program closes both accounts then, so
//! a deposit that no longer exists is one that finished.
//!
//! Pay as you go has two more accounts per buyer and tap: a balance at
//! `["balance", config, buyer]`, whose USDC is in the token account
//! `["vault", balance]`, and a tap at `["tap", config, tap id]` that names
//! the seller, the rate and the fee the buyer agreed to. The wallet tops up
//! and opens taps; the edge reads them back ([`tap_state`],
//! [`wallet_view`]) and bills them with [`settle_tap`] and [`close_tap`].
//! The program refuses any settle above the tap's rate for the time since
//! the last one, so the edge's key can bill no faster than the buyer agreed.
//!
//! The releaser is an ed25519 key derived from the EVM releaser's secret
//! ([`crate::evm_tx::Releaser::solana_seed`]), so one sealed credential
//! covers both families and one rotation replaces both. Its address needs a
//! little SOL for network fees and for the token accounts it opens for a
//! seller or treasury that has none.

use curve25519_dalek::edwards::CompressedEdwardsY;
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde_json::{json, Value};

use crate::market::MarketError;
use crate::meter::{ChainTap, WalletView};
use crate::solana::{base58_decode, base58_encode};
use crate::stablecoin::{call_result, Relay};

pub const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
pub const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

/// The program's instruction tags (`losos_escrow::ix`).
const PAY: u8 = 1;
const RELEASE: u8 = 2;
const REFUND: u8 = 3;
const SETTLE: u8 = 4;
// 6 top_up, 7 open_tap, 10 ask_withdrawal and 11 withdraw are the
// wallet's; the edge sends only these two.
const CLOSE_TAP: u8 = 8;
const SETTLE_TAP: u8 = 9;

const CONFIG_LEN: usize = 98;
const DEPOSIT_LEN: usize = 260;
const BALANCE_LEN: usize = 84;
const TAP_LEN: usize = 158;
const TOKEN_ACCOUNT_LEN: usize = 165;

/// The releaser on Solana. It signs only the escrow calls this module
/// builds: release, refund and split for deposits, settle and close for
/// taps.
pub struct SolanaReleaser {
    pair: Ed25519KeyPair,
    address: [u8; 32],
}

impl std::fmt::Debug for SolanaReleaser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SolanaReleaser")
            .field("address", &self.address_base58())
            .finish()
    }
}

impl SolanaReleaser {
    #[must_use]
    pub fn from_releaser(releaser: &crate::evm_tx::Releaser) -> Self {
        Self::from_seed(&releaser.solana_seed())
    }

    /// # Panics
    /// Never: any 32 bytes are an ed25519 seed.
    #[must_use]
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        let pair = Ed25519KeyPair::from_seed_unchecked(seed).expect("32 bytes");
        let address = pair.public_key().as_ref().try_into().expect("32 bytes");
        Self { pair, address }
    }

    #[must_use]
    pub fn address(&self) -> [u8; 32] {
        self.address
    }

    #[must_use]
    pub fn address_base58(&self) -> String {
        base58_encode(&self.address)
    }

    fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.pair
            .sign(message)
            .as_ref()
            .try_into()
            .expect("64 bytes")
    }
}

/// 32 bytes of a base58 address, or `None`.
#[must_use]
pub fn key(raw: &str) -> Option<[u8; 32]> {
    base58_decode(raw)?.try_into().ok()
}

/// Whether 32 bytes are a point on the curve, which no program address is.
/// The runtime's own test.
#[must_use]
pub fn on_curve(bytes: &[u8; 32]) -> bool {
    CompressedEdwardsY(*bytes).decompress().is_some()
}

/// `Pubkey::find_program_address`.
#[must_use]
pub fn find_program_address(seeds: &[&[u8]], program: &[u8; 32]) -> ([u8; 32], u8) {
    for bump in (0..=255u8).rev() {
        let mut h = ring::digest::Context::new(&ring::digest::SHA256);
        for s in seeds {
            h.update(s);
        }
        h.update(&[bump]);
        h.update(program);
        h.update(b"ProgramDerivedAddress");
        let out: [u8; 32] = h.finish().as_ref().try_into().expect("32 bytes");
        if !on_curve(&out) {
            return (out, bump);
        }
    }
    // Two hundred and fifty-six hashes all on the curve: about 2^-256.
    unreachable!("no program address")
}

#[must_use]
pub fn config_address(program: &[u8; 32], treasury: &[u8; 32], mint: &[u8; 32]) -> [u8; 32] {
    find_program_address(&[b"config", treasury, mint], program).0
}

#[must_use]
pub fn vault_address(program: &[u8; 32], deposit: &[u8; 32]) -> [u8; 32] {
    find_program_address(&[b"vault", deposit], program).0
}

/// `["balance", config, buyer]`: a buyer's prepaid balance. Its vault is
/// [`vault_address`] of it.
#[must_use]
pub fn balance_address(program: &[u8; 32], config: &[u8; 32], buyer: &[u8; 32]) -> [u8; 32] {
    find_program_address(&[b"balance", config, buyer], program).0
}

/// `["tap", config, tap id]`.
#[must_use]
pub fn tap_address(
    program: &[u8; 32],
    config: &[u8; 32],
    buyer: &[u8; 32],
    tap_id: &[u8; 32],
) -> [u8; 32] {
    find_program_address(&[b"tap", config, buyer, tap_id], program).0
}

/// The 32 bytes a tap is known by on Solana: the SHA-256 of the edge's tap
/// id. The program uses them only as a seed, so any 32 bytes would do; the
/// wallet must use the same ones.
#[must_use]
pub fn tap_id_bytes(id: &str) -> [u8; 32] {
    ring::digest::digest(&ring::digest::SHA256, id.as_bytes())
        .as_ref()
        .try_into()
        .expect("32 bytes")
}

#[must_use]
pub fn associated_token_address(owner: &[u8; 32], mint: &[u8; 32]) -> [u8; 32] {
    let token = key(TOKEN_PROGRAM).expect("constant");
    let ata = key(ATA_PROGRAM).expect("constant");
    find_program_address(&[owner, &token, mint], &ata).0
}

/// The config account as the program wrote it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigData {
    pub mint: [u8; 32],
    pub treasury: [u8; 32],
    pub releaser: [u8; 32],
}

impl ConfigData {
    #[must_use]
    pub fn read(data: &[u8]) -> Option<Self> {
        (data.len() == CONFIG_LEN && data[0] == 1).then(|| Self {
            mint: word(data, 2),
            treasury: word(data, 34),
            releaser: word(data, 66),
        })
    }
}

/// A held deposit as the program wrote it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepositData {
    pub config: [u8; 32],
    pub payer: [u8; 32],
    pub seller: [u8; 32],
    pub order: [u8; 32],
    pub gross: u64,
    pub fee: u64,
    pub deadline: i64,
    pub paid_at: i64,
    /// The buyer's and the seller's box keys: two of these and the
    /// releaser move the deposit.
    pub buyer_box: [u8; 32],
    pub seller_box: [u8; 32],
}

impl DepositData {
    #[must_use]
    pub fn read(data: &[u8]) -> Option<Self> {
        (data.len() == DEPOSIT_LEN && data[0] == 2).then(|| Self {
            config: word(data, 4),
            payer: word(data, 36),
            seller: word(data, 68),
            order: word(data, 100),
            gross: le(data, 164),
            fee: le(data, 172),
            deadline: le(data, 180) as i64,
            paid_at: le(data, 188) as i64,
            buyer_box: word(data, 196),
            seller_box: word(data, 228),
        })
    }
}

/// A buyer's balance as the program wrote it (`losos_escrow::state::Balance`).
/// The money itself is the vault's token amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BalanceData {
    pub config: [u8; 32],
    pub buyer: [u8; 32],
    /// What the buyer asked to withdraw, 0 without an ask.
    pub asked: u64,
    /// Unix seconds the ask can be withdrawn from, 0 without an ask.
    pub not_before: i64,
}

impl BalanceData {
    #[must_use]
    pub fn read(data: &[u8]) -> Option<Self> {
        (data.len() == BALANCE_LEN && data[0] == 3).then(|| Self {
            config: word(data, 4),
            buyer: word(data, 36),
            asked: le(data, 68),
            not_before: le(data, 76) as i64,
        })
    }
}

/// A tap as the program wrote it (`losos_escrow::state::Tap`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TapData {
    pub config: [u8; 32],
    pub buyer: [u8; 32],
    pub seller: [u8; 32],
    pub id: [u8; 32],
    /// Base units an hour, at most.
    pub rate: u64,
    pub settled_at: i64,
    pub closed_at: Option<i64>,
    pub fee_bps: u16,
}

impl TapData {
    #[must_use]
    pub fn read(data: &[u8]) -> Option<Self> {
        (data.len() == TAP_LEN && data[0] == 4 && data[2] <= 1).then(|| Self {
            config: word(data, 4),
            buyer: word(data, 36),
            seller: word(data, 68),
            id: word(data, 100),
            rate: le(data, 132),
            settled_at: le(data, 140) as i64,
            closed_at: (data[2] == 1).then(|| le(data, 148) as i64),
            fee_bps: u16::from_le_bytes([data[156], data[157]]),
        })
    }

    #[must_use]
    pub fn chain(&self) -> ChainTap {
        let secs = |t: i64| u64::try_from(t).unwrap_or(0);
        ChainTap {
            buyer: self.buyer,
            seller: self.seller,
            rate: self.rate,
            fee_bps: u32::from(self.fee_bps),
            settled_at: secs(self.settled_at),
            closed_at: self.closed_at.map(secs),
        }
    }
}

/// The amount of a classic token account.
fn token_amount(data: &[u8]) -> Option<u64> {
    (data.len() == TOKEN_ACCOUNT_LEN).then(|| le(data, 64))
}

/// What one paid order's deposit must hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedDeposit {
    /// The escrow program.
    pub program: String,
    pub config: [u8; 32],
    pub seller: [u8; 32],
    pub order_id_hash: [u8; 32],
    pub gross: u128,
    pub fee: u128,
    pub deadline: u64,
    pub buyer_box: [u8; 32],
    pub seller_box: [u8; 32],
}

impl ExpectedDeposit {
    /// What a payment on `chain` (one with the program as `pay`) must have
    /// put in its deposit. `None` when an address does not parse.
    #[must_use]
    pub fn new(
        chain: &crate::stablecoin::UsdcChain,
        seller: &str,
        order_id_hash: [u8; 32],
        gross: u128,
        fee: u128,
        deadline: u64,
        boxes: (&str, &str),
    ) -> Option<Self> {
        let program = chain.pay.as_deref()?;
        let p = key(program)?;
        Some(Self {
            program: program.to_string(),
            config: config_address(&p, &key(&chain.treasury)?, &key(chain.network.usdc)?),
            seller: key(seller)?,
            order_id_hash,
            gross,
            fee,
            deadline,
            buyer_box: key(boxes.0)?,
            seller_box: key(boxes.1)?,
        })
    }
}

/// The deposit `tx` (a `getTransaction` result, `jsonParsed`) made with the
/// escrow's `pay`, by address. The transaction must have succeeded; what it
/// paid is read from the deposit itself.
pub fn deposit_of(tx: &Value, program: &str) -> Result<String, &'static str> {
    let meta = tx
        .get("meta")
        .filter(|m| m.is_object())
        .ok_or("the transaction has no result")?;
    if !meta["err"].is_null() {
        return Err("the transaction failed");
    }
    let instructions = tx["transaction"]["message"]["instructions"]
        .as_array()
        .ok_or("the transaction has no instructions")?;
    let mut found = None;
    for i in instructions {
        if i["programId"].as_str() != Some(program) {
            continue;
        }
        let tag = i["data"]
            .as_str()
            .and_then(base58_decode)
            .and_then(|d| d.first().copied());
        if tag != Some(PAY) {
            continue;
        }
        let deposit = i["accounts"][3]
            .as_str()
            .ok_or("the payment names no deposit")?;
        // One payment per transaction, so one deposit per signature.
        if found.replace(deposit.to_string()).is_some() {
            return Err("the transaction pays more than once");
        }
    }
    found.ok_or("the transaction does not pay the escrow")
}

/// Whether `d` is the deposit `expected` describes. A static sentence on
/// failure, safe to show the owner.
pub fn deposit_matches(d: &DepositData, expected: &ExpectedDeposit) -> Result<(), &'static str> {
    if d.config != expected.config {
        return Err("the payment went to a different escrow");
    }
    if d.order != expected.order_id_hash {
        return Err("the transaction does not pay this order");
    }
    if d.seller != expected.seller {
        return Err("the payment went to a different seller");
    }
    if u128::from(d.gross) != expected.gross || u128::from(d.fee) != expected.fee {
        return Err("the payment amount is not the order's");
    }
    if u64::try_from(d.deadline).ok() != Some(expected.deadline) {
        return Err("the payment's refund deadline is not the order's");
    }
    if d.buyer_box != expected.buyer_box || d.seller_box != expected.seller_box {
        return Err("the payment names other boxes than the invoice");
    }
    Ok(())
}

/// An account's owner and data at `confirmed`, or `None` when it does not
/// exist.
async fn account(rpc_url: &str, address: &str) -> Result<Option<(String, Vec<u8>)>, MarketError> {
    account_head(rpc_url, address, None).await
}

/// As [`account`], with only the first `len` bytes of the data when `len`
/// is set: a program's data account carries the whole program after its
/// header.
async fn account_head(
    rpc_url: &str,
    address: &str,
    len: Option<usize>,
) -> Result<Option<(String, Vec<u8>)>, MarketError> {
    let mut config = json!({ "encoding": "base64", "commitment": "confirmed" });
    if let Some(len) = len {
        config["dataSlice"] = json!({ "offset": 0, "length": len });
    }
    let reply = crate::stablecoin::forward_rpc(
        rpc_url,
        Relay::Solana,
        "getAccountInfo",
        &json!([address, config]),
        &json!(1),
    )
    .await?;
    let value = &reply["result"]["value"];
    if reply.get("result").is_none() {
        return Err(MarketError::Conflict(
            "the chain endpoint returned an error",
        ));
    }
    if value.is_null() {
        return Ok(None);
    }
    let owner = value["owner"].as_str().unwrap_or("").to_string();
    let data = value["data"][0]
        .as_str()
        .and_then(base64_decode)
        .ok_or(MarketError::Conflict("the chain endpoint did not answer"))?;
    Ok(Some((owner, data)))
}

/// The held deposit at `address`, or `None` once it is gone.
pub async fn read_deposit(
    rpc_url: &str,
    program: &str,
    address: &str,
) -> Result<Option<DepositData>, MarketError> {
    match account(rpc_url, address).await? {
        Some((owner, data)) if owner == program => Ok(DepositData::read(&data)),
        _ => Ok(None),
    }
}

/// The tap `tap_id` under `config`, or `None` when it was never opened or
/// has finished: the program closes a tap once it is closed and settled up
/// to its close.
pub async fn tap_state(
    rpc_url: &str,
    program: &str,
    config: &[u8; 32],
    buyer: &[u8; 32],
    tap_id: [u8; 32],
) -> Result<Option<ChainTap>, MarketError> {
    let p = key(program).ok_or(MarketError::Unconfigured)?;
    let address = base58_encode(&tap_address(&p, config, buyer, &tap_id));
    match account(rpc_url, &address).await? {
        Some((owner, data)) if owner == program => Ok(TapData::read(&data)
            .filter(|t| t.config == *config && t.id == tap_id && t.buyer == *buyer)
            .map(|t| t.chain())),
        _ => Ok(None),
    }
}

/// What `buyer` holds in the escrow under `config`: the vault's tokens and
/// the open ask. All zeros when the buyer never topped up.
pub async fn wallet_view(
    rpc_url: &str,
    program: &str,
    config: &[u8; 32],
    buyer: [u8; 32],
) -> Result<WalletView, MarketError> {
    let p = key(program).ok_or(MarketError::Unconfigured)?;
    let balance = balance_address(&p, config, &buyer);
    let b = match account(rpc_url, &base58_encode(&balance)).await? {
        Some((owner, data)) if owner == program => BalanceData::read(&data),
        _ => None,
    };
    let Some(b) = b.filter(|b| b.config == *config && b.buyer == buyer) else {
        return Ok(WalletView::default());
    };
    let vault = base58_encode(&vault_address(&p, &balance));
    let held = match account(rpc_url, &vault).await? {
        Some((owner, data)) if owner == TOKEN_PROGRAM => token_amount(&data).unwrap_or(0),
        _ => 0,
    };
    Ok(WalletView {
        balance: held,
        asked: b.asked,
        not_before: u64::try_from(b.not_before).unwrap_or(0),
    })
}

/// Check that the configured program has this edge's config: Circle's mint,
/// the configured treasury, and the releaser this edge holds. Without it a
/// program someone else releases would let them decide where buyers' money
/// goes, and a config over another mint would take a token anyone can make.
pub async fn check_config(
    rpc_url: &str,
    program: &str,
    treasury: &str,
    mint: &str,
    releaser: &[u8; 32],
) -> Result<(), MarketError> {
    let (Some(p), Some(t), Some(m)) = (key(program), key(treasury), key(mint)) else {
        return Err(MarketError::Unconfigured);
    };
    let config = base58_encode(&config_address(&p, &t, &m));
    let data = match account(rpc_url, &config).await? {
        Some((owner, data)) if owner == program => data,
        _ => {
            return Err(MarketError::Conflict(
                "the escrow has no config for this treasury yet",
            ))
        }
    };
    let c = ConfigData::read(&data).ok_or(MarketError::Conflict(
        "the escrow has no config for this treasury yet",
    ))?;
    if c.mint != m || c.treasury != t {
        return Err(MarketError::Conflict(
            "the escrow pays a different treasury",
        ));
    }
    if c.releaser != *releaser {
        return Err(MarketError::Conflict(
            "the escrow takes orders from a different releaser key",
        ));
    }
    Ok(())
}

/// The loader that lets a program's upgrade authority replace its code.
pub const UPGRADEABLE_LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
/// Loaders whose programs can never change.
const FINAL_LOADERS: &[&str] = &[
    "BPFLoader2111111111111111111111111111111111",
    "BPFLoader1111111111111111111111111111111111",
];

/// The data account of an upgradeable program, from the program account's
/// data: `UpgradeableLoaderState::Program`, tag 2 then its address.
#[must_use]
pub fn programdata_of(program_data: &[u8]) -> Option<[u8; 32]> {
    (program_data.len() >= 36 && program_data[..4] == [2, 0, 0, 0]).then(|| word(program_data, 4))
}

/// Whether nobody can upgrade a program, from the head of its data account:
/// `UpgradeableLoaderState::ProgramData`, tag 3, the slot, then an optional
/// authority. No authority, or the all-zero address nobody can sign for, is
/// final.
#[must_use]
pub fn upgrade_is_final(head: &[u8]) -> bool {
    if head.len() < 13 || head[..4] != [3, 0, 0, 0] {
        return false;
    }
    match head[12] {
        0 => true,
        1 => head.len() >= 45 && head[13..45] == [0; 32],
        _ => false,
    }
}

/// Check that the escrow program can never be changed. Whoever holds an
/// upgrade authority could replace its code and take every deposit, so a
/// live edge counts no payment into a program that still has one.
pub async fn check_final(rpc_url: &str, program: &str) -> Result<(), MarketError> {
    const NOT_DEPLOYED: MarketError = MarketError::Conflict("the escrow program is not deployed");
    match account(rpc_url, program).await? {
        Some((owner, _)) if FINAL_LOADERS.contains(&owner.as_str()) => Ok(()),
        Some((owner, data)) if owner == UPGRADEABLE_LOADER => {
            let programdata = programdata_of(&data).ok_or(NOT_DEPLOYED)?;
            match account_head(rpc_url, &base58_encode(&programdata), Some(45)).await? {
                Some((owner, head)) if owner == UPGRADEABLE_LOADER && upgrade_is_final(&head) => {
                    Ok(())
                }
                Some((owner, _)) if owner == UPGRADEABLE_LOADER => Err(MarketError::Conflict(
                    "the escrow program can still be upgraded; its upgrade authority must be removed first",
                )),
                _ => Err(NOT_DEPLOYED),
            }
        }
        _ => Err(NOT_DEPLOYED),
    }
}

/// The releaser's SOL in lamports, and whether the config takes orders from
/// it. `None` for the second while there is no config.
pub async fn releaser_on_chain(
    rpc_url: &str,
    program: &str,
    treasury: &str,
    mint: &str,
    releaser: &[u8; 32],
) -> Result<(u64, Option<bool>), MarketError> {
    let balance = call_result(
        rpc_url,
        Relay::Solana,
        "getBalance",
        json!([base58_encode(releaser), { "commitment": "confirmed" }]),
    )
    .await?;
    let lamports = balance["value"]
        .as_u64()
        .ok_or(MarketError::Conflict("the chain endpoint did not answer"))?;
    let agrees = match check_config(rpc_url, program, treasury, mint, releaser).await {
        Ok(()) => Some(true),
        Err(MarketError::Conflict("the escrow has no config for this treasury yet")) => None,
        Err(MarketError::Conflict(_)) => Some(false),
        Err(e) => return Err(e),
    };
    Ok((lamports, agrees))
}

/// What the edge does with a deposit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Release,
    Refund,
    /// So much of the gross back to the payer, in USDC base units.
    Split(u64),
}

/// Where a deposit's money may go: the program checks each of these against
/// the deposit and the config, so they only have to be right, not trusted.
#[derive(Debug, Clone)]
pub struct Parties {
    pub program: [u8; 32],
    pub mint: [u8; 32],
    pub treasury: [u8; 32],
    pub deposit: [u8; 32],
    pub held: DepositData,
    /// The box that agrees, and its signature over
    /// [`crate::escrow_boxes::solana_message`] for this move: the second of
    /// the two keys the program wants, the releaser being the first.
    pub agreed: Option<([u8; 32], [u8; 64])>,
}

/// The program that checks ed25519 signatures, and the sysvar through which
/// the escrow program reads that check back.
pub const ED25519_PROGRAM: &str = "Ed25519SigVerify111111111111111111111111111";
pub const INSTRUCTIONS_SYSVAR: &str = "Sysvar1nstructions1111111111111111111111111";

/// An ed25519 signature check whose key, signature and message all sit in
/// its own data, the only shape the escrow program counts.
fn ed25519_check(key_: &[u8; 32], signature: &[u8; 64], message: &[u8]) -> Ix {
    let own = u16::MAX.to_le_bytes();
    let (key_at, sig_at, message_at) = (16u16, 48u16, 112u16);
    let mut data = vec![1u8, 0];
    for part in [
        sig_at.to_le_bytes(),
        own,
        key_at.to_le_bytes(),
        own,
        message_at.to_le_bytes(),
        u16::try_from(message.len()).expect("short").to_le_bytes(),
        own,
    ] {
        data.extend_from_slice(&part);
    }
    data.extend_from_slice(key_);
    data.extend_from_slice(signature);
    data.extend_from_slice(message);
    Ix {
        program: key(ED25519_PROGRAM).expect("constant"),
        accounts: Vec::new(),
        data,
    }
}

struct Meta {
    key: [u8; 32],
    signer: bool,
    writable: bool,
}

struct Ix {
    program: [u8; 32],
    accounts: Vec<Meta>,
    data: Vec<u8>,
}

const fn w(key: [u8; 32]) -> Meta {
    Meta {
        key,
        signer: false,
        writable: true,
    }
}

const fn r(key: [u8; 32]) -> Meta {
    Meta {
        key,
        signer: false,
        writable: false,
    }
}

/// An idempotent creation of `owner`'s USDC account, which the releaser
/// pays for when it is missing, so a seller or treasury that never held
/// USDC can still be paid.
fn create_ata(payer: [u8; 32], owner: [u8; 32], mint: [u8; 32]) -> Ix {
    let k = |s| key(s).expect("constant");
    Ix {
        program: k(ATA_PROGRAM),
        accounts: vec![
            Meta {
                key: payer,
                signer: true,
                writable: true,
            },
            w(associated_token_address(&owner, &mint)),
            r(owner),
            r(mint),
            r(k(SYSTEM_PROGRAM)),
            r(k(TOKEN_PROGRAM)),
        ],
        data: vec![1],
    }
}

/// The one transaction the edge sends for `step`: the token accounts it
/// pays into, created if they are missing, then the escrow call. Signed;
/// returns the base64 wire form and the signature.
#[must_use]
pub fn escrow_transaction(
    releaser: &SolanaReleaser,
    parties: &Parties,
    step: Move,
    blockhash: &[u8; 32],
) -> (String, String) {
    let me = releaser.address();
    let p = parties;
    let token = key(TOKEN_PROGRAM).expect("constant");
    let config = config_address(&p.program, &p.treasury, &p.mint);
    let vault = vault_address(&p.program, &p.deposit);
    let ata = |owner: &[u8; 32]| associated_token_address(owner, &p.mint);
    let mut ixs = Vec::new();
    let mut accounts = vec![
        Meta {
            key: me,
            signer: true,
            writable: true,
        },
        r(config),
        w(p.deposit),
        w(vault),
    ];
    let data = match step {
        Move::Release => {
            ixs.push(create_ata(me, p.held.seller, p.mint));
            ixs.push(create_ata(me, p.treasury, p.mint));
            accounts.push(w(ata(&p.held.seller)));
            accounts.push(w(ata(&p.treasury)));
            vec![RELEASE]
        }
        Move::Refund => {
            ixs.push(create_ata(me, p.held.payer, p.mint));
            accounts.push(w(ata(&p.held.payer)));
            vec![REFUND]
        }
        Move::Split(back) => {
            ixs.push(create_ata(me, p.held.payer, p.mint));
            ixs.push(create_ata(me, p.held.seller, p.mint));
            ixs.push(create_ata(me, p.treasury, p.mint));
            accounts.push(w(ata(&p.held.payer)));
            accounts.push(w(ata(&p.held.seller)));
            accounts.push(w(ata(&p.treasury)));
            let mut d = vec![SETTLE];
            d.extend_from_slice(&back.to_le_bytes());
            d
        }
    };
    accounts.push(w(p.held.payer));
    accounts.push(r(p.mint));
    accounts.push(r(token));
    accounts.push(r(key(INSTRUCTIONS_SYSVAR).expect("constant")));
    if let Some((by, signature)) = &p.agreed {
        let refund = match step {
            Move::Release => 0,
            Move::Refund => p.held.gross,
            Move::Split(back) => back,
        };
        let message = crate::escrow_boxes::solana_message(&p.deposit, refund);
        ixs.push(ed25519_check(by, signature, &message));
    }
    ixs.push(Ix {
        program: p.program,
        accounts,
        data,
    });
    signed(releaser, &ixs, blockhash)
}

/// `ixs` as one transaction the releaser pays for and signs: the base64
/// wire form and the signature.
fn signed(releaser: &SolanaReleaser, ixs: &[Ix], blockhash: &[u8; 32]) -> (String, String) {
    let message = compile(releaser.address(), ixs, blockhash);
    let signature = releaser.sign(&message);
    let mut wire = shortvec(1);
    wire.extend_from_slice(&signature);
    wire.extend_from_slice(&message);
    (base64_encode(&wire), base58_encode(&signature))
}

/// What the edge does with a tap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapStep {
    /// Bill so many base units of what was metered since the last settle.
    /// 0 is taken only on a closed tap, where it finishes one that owes
    /// nothing.
    Settle(u64),
    /// Stop the tap's billing now.
    Close,
}

/// Who a tap's money and rent may go to. The program checks each against
/// the tap and the config, so they only have to be right, not trusted.
#[derive(Debug, Clone)]
pub struct TapParties {
    pub program: [u8; 32],
    pub mint: [u8; 32],
    pub treasury: [u8; 32],
    pub tap_id: [u8; 32],
    /// The tap's buyer: whose balance pays, and who takes the tap's rent.
    pub buyer: [u8; 32],
    pub seller: [u8; 32],
}

impl TapParties {
    /// The parties of `tap`, as [`tap_state`] read it.
    #[must_use]
    pub fn new(
        program: [u8; 32],
        mint: [u8; 32],
        treasury: [u8; 32],
        tap_id: [u8; 32],
        tap: &ChainTap,
    ) -> Self {
        Self {
            program,
            mint,
            treasury,
            tap_id,
            buyer: tap.buyer,
            seller: tap.seller,
        }
    }
}

/// The one transaction the edge sends for a tap. A settle first creates
/// the seller's and the treasury's USDC accounts if they are missing, as a
/// release does. Signed; returns the base64 wire form and the signature.
#[must_use]
pub fn tap_transaction(
    releaser: &SolanaReleaser,
    parties: &TapParties,
    step: TapStep,
    blockhash: &[u8; 32],
) -> (String, String) {
    let me = releaser.address();
    let p = parties;
    let config = config_address(&p.program, &p.treasury, &p.mint);
    let tap = tap_address(&p.program, &config, &p.buyer, &p.tap_id);
    let signer = Meta {
        key: me,
        signer: true,
        writable: true,
    };
    let mut ixs = Vec::new();
    let (accounts, data) = match step {
        TapStep::Settle(amount) => {
            let token = key(TOKEN_PROGRAM).expect("constant");
            let balance = balance_address(&p.program, &config, &p.buyer);
            let ata = |owner: &[u8; 32]| associated_token_address(owner, &p.mint);
            ixs.push(create_ata(me, p.seller, p.mint));
            ixs.push(create_ata(me, p.treasury, p.mint));
            let mut data = vec![SETTLE_TAP];
            data.extend_from_slice(&amount.to_le_bytes());
            (
                vec![
                    signer,
                    r(config),
                    w(tap),
                    r(balance),
                    w(vault_address(&p.program, &balance)),
                    w(ata(&p.seller)),
                    w(ata(&p.treasury)),
                    w(p.buyer),
                    r(p.mint),
                    r(token),
                ],
                data,
            )
        }
        TapStep::Close => (vec![signer, r(config), w(tap), w(p.buyer)], vec![CLOSE_TAP]),
    };
    ixs.push(Ix {
        program: p.program,
        accounts,
        data,
    });
    signed(releaser, &ixs, blockhash)
}

/// A legacy message. The fee payer must be key 0, and the header only works
/// if keys are grouped signer-writable, signer-readonly, writable, readonly.
fn compile(payer: [u8; 32], ixs: &[Ix], blockhash: &[u8; 32]) -> Vec<u8> {
    let mut keys: Vec<Meta> = vec![Meta {
        key: payer,
        signer: true,
        writable: true,
    }];
    let mut merge = |m: &Meta| {
        if let Some(seen) = keys.iter_mut().find(|k| k.key == m.key) {
            seen.signer |= m.signer;
            seen.writable |= m.writable;
        } else {
            keys.push(Meta {
                key: m.key,
                signer: m.signer,
                writable: m.writable,
            });
        }
    };
    for ix in ixs {
        ix.accounts.iter().for_each(&mut merge);
        merge(&r(ix.program));
    }
    let rank = |m: &Meta| match (m.signer, m.writable) {
        (true, true) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    };
    // Stable, so the payer stays first among the signers.
    keys.sort_by_key(rank);
    let index = |k: &[u8; 32]| {
        u8::try_from(keys.iter().position(|m| m.key == *k).expect("merged"))
            .expect("under 256 keys")
    };
    let mut out = vec![
        u8::try_from(keys.iter().filter(|m| m.signer).count()).expect("few"),
        u8::try_from(keys.iter().filter(|m| m.signer && !m.writable).count()).expect("few"),
        u8::try_from(keys.iter().filter(|m| !m.signer && !m.writable).count()).expect("few"),
    ];
    out.extend(shortvec(keys.len()));
    for m in &keys {
        out.extend_from_slice(&m.key);
    }
    out.extend_from_slice(blockhash);
    out.extend(shortvec(ixs.len()));
    for ix in ixs {
        out.push(index(&ix.program));
        out.extend(shortvec(ix.accounts.len()));
        for a in &ix.accounts {
            out.push(index(&a.key));
        }
        out.extend(shortvec(ix.data.len()));
        out.extend_from_slice(&ix.data);
    }
    out
}

/// compact-u16: seven bits per byte, low first, high bit means more follows.
fn shortvec(mut n: usize) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut byte = (n & 0x7f) as u8;
        n >>= 7;
        if n > 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if n == 0 {
            return out;
        }
    }
}

/// Send `step` for the deposit and return its signature.
pub async fn send(
    rpc_url: &str,
    releaser: &SolanaReleaser,
    parties: &Parties,
    step: Move,
) -> Result<String, MarketError> {
    let blockhash = latest_blockhash(rpc_url).await?;
    let (wire, signature) = escrow_transaction(releaser, parties, step, &blockhash);
    submit(rpc_url, wire).await?;
    Ok(signature)
}

/// Send `step` for the tap and return its signature.
pub async fn send_tap(
    rpc_url: &str,
    releaser: &SolanaReleaser,
    parties: &TapParties,
    step: TapStep,
) -> Result<String, MarketError> {
    let blockhash = latest_blockhash(rpc_url).await?;
    let (wire, signature) = tap_transaction(releaser, parties, step, &blockhash);
    submit(rpc_url, wire).await?;
    Ok(signature)
}

/// Bill `amount` base units on the tap and return the signature.
pub async fn settle_tap(
    rpc_url: &str,
    releaser: &SolanaReleaser,
    parties: &TapParties,
    amount: u64,
) -> Result<String, MarketError> {
    send_tap(rpc_url, releaser, parties, TapStep::Settle(amount)).await
}

/// Close the tap and return the signature.
pub async fn close_tap(
    rpc_url: &str,
    releaser: &SolanaReleaser,
    parties: &TapParties,
) -> Result<String, MarketError> {
    send_tap(rpc_url, releaser, parties, TapStep::Close).await
}

async fn latest_blockhash(rpc_url: &str) -> Result<[u8; 32], MarketError> {
    let latest = call_result(
        rpc_url,
        Relay::Solana,
        "getLatestBlockhash",
        json!([{ "commitment": "confirmed" }]),
    )
    .await?;
    latest["value"]["blockhash"]
        .as_str()
        .and_then(key)
        .ok_or(MarketError::Conflict("the chain endpoint did not answer"))
}

async fn submit(rpc_url: &str, wire: String) -> Result<(), MarketError> {
    call_result(
        rpc_url,
        Relay::Solana,
        "sendTransaction",
        json!([wire, { "encoding": "base64", "preflightCommitment": "confirmed" }]),
    )
    .await?;
    Ok(())
}

/// How a transaction the edge sent stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Landed {
    /// At `confirmed` or later, and it succeeded.
    Done,
    /// On chain, and the program refused it.
    Failed,
    /// Not seen (yet, or ever: its blockhash expires after about a minute).
    Unknown,
}

pub async fn status(rpc_url: &str, signature: &str) -> Result<Landed, MarketError> {
    let reply = call_result(
        rpc_url,
        Relay::Solana,
        "getSignatureStatuses",
        json!([[signature], { "searchTransactionHistory": true }]),
    )
    .await?;
    Ok(landed(&reply["value"][0]))
}

#[must_use]
pub fn landed(status: &Value) -> Landed {
    if status.is_null() {
        return Landed::Unknown;
    }
    if !status["err"].is_null() {
        return Landed::Failed;
    }
    match status["confirmationStatus"].as_str() {
        Some("confirmed" | "finalized") => Landed::Done,
        _ => Landed::Unknown,
    }
}

fn word(data: &[u8], at: usize) -> [u8; 32] {
    data[at..at + 32].try_into().expect("in bounds")
}

fn le(data: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(data[at..at + 8].try_into().expect("in bounds"))
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(crate) fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub(crate) fn base64_decode(raw: &str) -> Option<Vec<u8>> {
    let raw = raw.trim_end_matches('=');
    let mut out = Vec::with_capacity(raw.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in raw.bytes() {
        let v = B64.iter().position(|b| *b == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_program_nobody_can_upgrade_is_final() {
        // What solana-test-validator 4.0.3 answers for `--bpf-program` (the
        // zero authority) and `--upgradeable-program` (a real one), and what
        // `set-upgrade-authority --final` leaves (none).
        let mut program = vec![2, 0, 0, 0];
        program.extend([7u8; 32]);
        assert_eq!(programdata_of(&program), Some([7u8; 32]));
        assert_eq!(programdata_of(&program[..35]), None);
        assert_eq!(programdata_of(&[3, 0, 0, 0]), None);

        let head = |authority: Option<[u8; 32]>| {
            let mut d = vec![3, 0, 0, 0];
            d.extend(5u64.to_le_bytes());
            match authority {
                None => d.extend([0u8; 33]),
                Some(a) => {
                    d.push(1);
                    d.extend(a);
                }
            }
            d
        };
        assert!(upgrade_is_final(&head(None)));
        assert!(upgrade_is_final(&head(Some([0; 32]))));
        assert!(!upgrade_is_final(&head(Some([9; 32]))));
        assert!(!upgrade_is_final(&head(Some([9; 32]))[..13]));
        let mut wrong = head(None);
        wrong[0] = 2;
        assert!(!upgrade_is_final(&wrong));
        assert!(!upgrade_is_final(&[]));
    }

    #[test]
    fn program_addresses_match_the_runtime() {
        // `solana find-program-derived-address` gives the same two.
        let owner = key("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM").unwrap();
        let usdc = key("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        assert_eq!(
            base58_encode(&associated_token_address(&owner, &usdc)),
            "FGETo8T8wMcN2wCjav8VK6eh3dLk63evNDPxzLSJra8B"
        );
        let program = key("LosEscrow1111111111111111111111111111111111").unwrap();
        assert_eq!(
            base58_encode(&vault_address(&program, &owner)),
            "92UMAvkSCU1ATw3VWw8b1kXV16DHQS7G3hxjeCSzyjHL"
        );
        assert!(on_curve(&owner));
    }

    #[test]
    fn base64_round_trips() {
        for n in 0..10 {
            let bytes: Vec<u8> = (0..n).map(|i| (i * 37) as u8).collect();
            assert_eq!(base64_decode(&base64_encode(&bytes)).unwrap(), bytes);
        }
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
    }

    #[test]
    fn a_deposit_matches_only_its_order() {
        let d = DepositData {
            config: [1; 32],
            payer: [2; 32],
            seller: [3; 32],
            order: [4; 32],
            gross: 20_000_000,
            fee: 4_000_000,
            deadline: 1_900_000_000,
            paid_at: 1_800_000_000,
            buyer_box: [5; 32],
            seller_box: [6; 32],
        };
        let want = ExpectedDeposit {
            program: String::new(),
            config: [1; 32],
            seller: [3; 32],
            order_id_hash: [4; 32],
            gross: 20_000_000,
            fee: 4_000_000,
            deadline: 1_900_000_000,
            buyer_box: [5; 32],
            seller_box: [6; 32],
        };
        assert_eq!(deposit_matches(&d, &want), Ok(()));
        for (bad, why) in [
            (
                DepositData {
                    config: [9; 32],
                    ..d
                },
                "the payment went to a different escrow",
            ),
            (
                DepositData {
                    order: [9; 32],
                    ..d
                },
                "the transaction does not pay this order",
            ),
            (
                DepositData {
                    seller: [9; 32],
                    ..d
                },
                "the payment went to a different seller",
            ),
            (
                DepositData { gross: 1, ..d },
                "the payment amount is not the order's",
            ),
            (
                DepositData { fee: 0, ..d },
                "the payment amount is not the order's",
            ),
            (
                DepositData { deadline: 1, ..d },
                "the payment's refund deadline is not the order's",
            ),
            // The buyer's own second key as the seller's box would hold two
            // of the three keys.
            (
                DepositData {
                    seller_box: [9; 32],
                    ..d
                },
                "the payment names other boxes than the invoice",
            ),
            (
                DepositData {
                    buyer_box: [6; 32],
                    seller_box: [5; 32],
                    ..d
                },
                "the payment names other boxes than the invoice",
            ),
        ] {
            assert_eq!(deposit_matches(&bad, &want), Err(why));
        }
    }

    #[test]
    fn the_pay_instruction_names_the_deposit() {
        let program = "LosEscrow1111111111111111111111111111111111";
        let tx = |data: &str, err: Value| {
            json!({
                "meta": { "err": err },
                "transaction": { "message": { "instructions": [
                    { "programId": program, "accounts": ["a", "b", "c", "DEP", "v"], "data": data }
                ] } }
            })
        };
        let pay = base58_encode(&[PAY, 0, 0]);
        let release = base58_encode(&[RELEASE]);
        assert_eq!(
            deposit_of(&tx(&pay, Value::Null), program).as_deref(),
            Ok("DEP")
        );
        assert_eq!(
            deposit_of(&tx(&release, Value::Null), program),
            Err("the transaction does not pay the escrow")
        );
        assert_eq!(
            deposit_of(
                &tx(&pay, json!({"InstructionError": [0, {"Custom": 1}]})),
                program
            ),
            Err("the transaction failed")
        );
        assert_eq!(
            deposit_of(
                &tx(&pay, Value::Null),
                "Other111111111111111111111111111111111111"
            ),
            Err("the transaction does not pay the escrow")
        );
    }

    #[test]
    fn a_status_is_done_only_when_confirmed_without_error() {
        assert_eq!(landed(&Value::Null), Landed::Unknown);
        assert_eq!(
            landed(&json!({"err": null, "confirmationStatus": "processed"})),
            Landed::Unknown
        );
        assert_eq!(
            landed(&json!({"err": null, "confirmationStatus": "confirmed"})),
            Landed::Done
        );
        assert_eq!(
            landed(&json!({"err": null, "confirmationStatus": "finalized"})),
            Landed::Done
        );
        assert_eq!(
            landed(
                &json!({"err": {"InstructionError": [2, {"Custom": 8}]}, "confirmationStatus": "confirmed"})
            ),
            Landed::Failed
        );
    }

    /// The program's own bytes for these accounts: the fixtures its
    /// `the_layouts_the_edge_reads_are_pinned` test writes with
    /// `losos_escrow::state`. Change both together.
    const BALANCE_FIXTURE: &str = "03fefd0001010101010101010101010101010101010101010101010101010101010101010202020202020202020202020202020202020202020202020202020202020202c0cf6a000000000000754c6b00000000";
    const TAP_CLOSED_FIXTURE: &str = "04ff0100010101010101010101010101010101010101010101010101010101010101010102020202020202020202020202020202020202020202020202020202020202020303030303030303030303030303030303030303030303030303030303030303040404040404040404040404040404040404040404040404040404040404040440420f000000000000d2496b0000000010e0496b000000009001";
    const TAP_OPEN_FIXTURE: &str = "04ff0000010101010101010101010101010101010101010101010101010101010101010102020202020202020202020202020202020202020202020202020202020202020303030303030303030303030303030303030303030303030303030303030303040404040404040404040404040404040404040404040404040404040404040440420f000000000000d2496b0000000000000000000000009001";

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn balances_and_taps_decode_as_the_program_writes_them() {
        let b = BalanceData::read(&hex(BALANCE_FIXTURE)).unwrap();
        assert_eq!(
            b,
            BalanceData {
                config: [1; 32],
                buyer: [2; 32],
                asked: 7_000_000,
                not_before: 1_800_172_800,
            }
        );
        let closed = TapData::read(&hex(TAP_CLOSED_FIXTURE)).unwrap();
        assert_eq!(
            closed,
            TapData {
                config: [1; 32],
                buyer: [2; 32],
                seller: [3; 32],
                id: [4; 32],
                rate: 1_000_000,
                settled_at: 1_800_000_000,
                closed_at: Some(1_800_003_600),
                fee_bps: 400,
            }
        );
        assert_eq!(
            closed.chain(),
            ChainTap {
                buyer: [2; 32],
                seller: [3; 32],
                rate: 1_000_000,
                fee_bps: 400,
                settled_at: 1_800_000_000,
                closed_at: Some(1_800_003_600),
            }
        );
        let open = TapData::read(&hex(TAP_OPEN_FIXTURE)).unwrap();
        assert_eq!(open.closed_at, None);
        assert_eq!(open.chain().closed_at, None);
        // Not a balance or a tap: the wrong length, tag or closed flag.
        assert_eq!(BalanceData::read(&hex(TAP_OPEN_FIXTURE)), None);
        assert_eq!(TapData::read(&hex(BALANCE_FIXTURE)), None);
        let mut data = hex(TAP_OPEN_FIXTURE);
        data[2] = 2;
        assert_eq!(TapData::read(&data), None);
        assert_eq!(TapData::read(&data[..157]), None);
        let mut token = vec![0u8; 165];
        token[64..72].copy_from_slice(&42u64.to_le_bytes());
        assert_eq!(token_amount(&token), Some(42));
        assert_eq!(token_amount(&token[..164]), None);
    }

    #[test]
    fn tap_addresses_match_the_runtime() {
        // `solana find-program-derived-address LosEscrow1… string:tap
        // pubkey:<config> pubkey:<buyer> hex:04…04` and `string:balance
        // pubkey:<config> pubkey:<buyer>`.
        let program = key("LosEscrow1111111111111111111111111111111111").unwrap();
        let config = key("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM").unwrap();
        let buyer = key("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        assert_eq!(
            base58_encode(&tap_address(&program, &config, &buyer, &[4; 32])),
            "5Gbz1iYxjC5fegQqLZ7y5XxoR49y17gDhNQGjqzXWkWY"
        );
        assert_eq!(
            base58_encode(&balance_address(&program, &config, &buyer)),
            "4p1yaXiRqaKiAUnnaeZStCXtG4krcWCnTnddBvDdXTHx"
        );
        // SHA-256 of the edge's tap id.
        let mut want = [0u8; 32];
        want.copy_from_slice(&hex(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ));
        assert_eq!(tap_id_bytes("abc"), want);
        assert_ne!(tap_id_bytes("tap-1"), tap_id_bytes("tap-2"));
    }

    /// A legacy transaction taken apart: its keys with their signer and
    /// writable flags, and each instruction as (program, accounts, data).
    struct Decoded {
        keys: Vec<([u8; 32], bool, bool)>,
        ixs: Vec<([u8; 32], Vec<usize>, Vec<u8>)>,
    }

    fn decode(wire: &str, signer: &[u8; 32]) -> Decoded {
        let wire = base64_decode(wire).unwrap();
        assert_eq!(wire[0], 1, "one signature");
        let (signature, message) = (&wire[1..65], &wire[65..]);
        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, signer)
            .verify(message, signature)
            .expect("signed by the releaser");
        let (signers, ro_signers, ro) = (
            usize::from(message[0]),
            usize::from(message[1]),
            usize::from(message[2]),
        );
        let n = usize::from(message[3]);
        let mut at = 4;
        let mut keys = Vec::new();
        for i in 0..n {
            let k = word(message, at);
            at += 32;
            let is_signer = i < signers;
            let writable = if is_signer {
                i < signers - ro_signers
            } else {
                i < n - ro
            };
            keys.push((k, is_signer, writable));
        }
        at += 32; // blockhash
        let count = usize::from(message[at]);
        at += 1;
        let mut ixs = Vec::new();
        for _ in 0..count {
            let program = keys[usize::from(message[at])].0;
            let len = usize::from(message[at + 1]);
            let accounts = message[at + 2..at + 2 + len]
                .iter()
                .map(|i| usize::from(*i))
                .collect();
            at += 2 + len;
            let dlen = usize::from(message[at]);
            let data = message[at + 1..at + 1 + dlen].to_vec();
            at += 1 + dlen;
            ixs.push((program, accounts, data));
        }
        assert_eq!(at, message.len());
        Decoded { keys, ixs }
    }

    fn tap_parties() -> TapParties {
        TapParties::new(
            key("LosEscrow1111111111111111111111111111111111").unwrap(),
            key("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            [8; 32],
            tap_id_bytes("tap-1"),
            &ChainTap {
                buyer: [2; 32],
                seller: [3; 32],
                rate: 1_000_000,
                fee_bps: 400,
                settled_at: 1_800_000_000,
                closed_at: None,
            },
        )
    }

    #[test]
    fn a_tap_settle_names_the_programs_accounts_in_order() {
        let releaser = SolanaReleaser::from_seed(&[5; 32]);
        let p = tap_parties();
        let (wire, _) = tap_transaction(&releaser, &p, TapStep::Settle(1_500_000), &[9; 32]);
        let d = decode(&wire, &releaser.address());
        let config = config_address(&p.program, &p.treasury, &p.mint);
        let balance = balance_address(&p.program, &config, &p.buyer);
        let token = key(TOKEN_PROGRAM).unwrap();
        let ata = |o: &[u8; 32]| associated_token_address(o, &p.mint);
        // The seller's and the treasury's USDC accounts, then the settle.
        assert_eq!(d.ixs.len(), 3);
        let ata_program = key(ATA_PROGRAM).unwrap();
        assert_eq!(d.ixs[0].0, ata_program);
        assert_eq!(d.keys[d.ixs[0].1[1]].0, ata(&p.seller));
        assert_eq!(d.ixs[1].0, ata_program);
        assert_eq!(d.keys[d.ixs[1].1[1]].0, ata(&p.treasury));
        let (program, accounts, data) = &d.ixs[2];
        assert_eq!(*program, p.program);
        let mut want = vec![SETTLE_TAP];
        want.extend_from_slice(&1_500_000u64.to_le_bytes());
        assert_eq!(*data, want);
        assert_eq!(data[0], 9);
        // (key, signer, writable), in the program's order.
        let named: Vec<_> = accounts.iter().map(|i| d.keys[*i]).collect();
        assert_eq!(
            named,
            vec![
                (releaser.address(), true, true),
                (config, false, false),
                (
                    tap_address(&p.program, &config, &p.buyer, &p.tap_id),
                    false,
                    true
                ),
                (balance, false, false),
                (vault_address(&p.program, &balance), false, true),
                (ata(&p.seller), false, true),
                (ata(&p.treasury), false, true),
                (p.buyer, false, true),
                (p.mint, false, false),
                (token, false, false),
            ]
        );
    }

    #[test]
    fn a_tap_close_names_the_tap_and_its_buyer() {
        let releaser = SolanaReleaser::from_seed(&[5; 32]);
        let p = tap_parties();
        let (wire, signature) = tap_transaction(&releaser, &p, TapStep::Close, &[9; 32]);
        let d = decode(&wire, &releaser.address());
        let config = config_address(&p.program, &p.treasury, &p.mint);
        assert_eq!(d.ixs.len(), 1);
        let (program, accounts, data) = &d.ixs[0];
        assert_eq!(*program, p.program);
        assert_eq!(*data, vec![CLOSE_TAP]);
        assert_eq!(data[0], 8);
        let named: Vec<_> = accounts.iter().map(|i| d.keys[*i]).collect();
        assert_eq!(
            named,
            vec![
                (releaser.address(), true, true),
                (config, false, false),
                (
                    tap_address(&p.program, &config, &p.buyer, &p.tap_id),
                    false,
                    true
                ),
                (p.buyer, false, true),
            ]
        );
        assert_eq!(
            base58_decode(&signature).unwrap(),
            base64_decode(&wire).unwrap()[1..65].to_vec()
        );
    }

    #[test]
    fn the_releaser_is_a_key_of_its_own_and_never_prints() {
        let r = SolanaReleaser::from_seed(&[5; 32]);
        assert_eq!(
            format!("{r:?}"),
            format!("SolanaReleaser {{ address: {:?} }}", r.address_base58())
        );
        assert!(on_curve(&r.address()));
    }
}
