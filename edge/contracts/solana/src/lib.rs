//! `losos-escrow`: the Solana twin of `LososEscrow.sol`.
//!
//! It holds one USDC payment per deposit for as long as what it bought
//! runs. It is then released to the treasury and the seller recorded at
//! payment time, refunded to the payer, or split between the two when the
//! seller stopped delivering partway. Each of those needs two of three keys:
//! the releaser (the edge's), the buyer's box's and the seller's box's, the
//! last two named at payment. A key agrees either by signing the
//! transaction as its authority or through an Ed25519 signature-check
//! instruction in the same transaction over the decision's message
//! ([`decision_message`]), so the edge alone can neither pay a deposit out
//! nor send it back, and the boxes can settle without the edge. Nobody can
//! change where a deposit goes. If no two agree, the payer takes the deposit
//! back after its deadline.
//!
//! One config account per treasury and mint (`["config", treasury, mint]`)
//! names the releaser. Each payment is a deposit account
//! (`["deposit", config, nonce]`, a nonce the wallet picks at random) and a
//! token account (`["vault", deposit]`) that the deposit owns. When a deposit
//! is released, refunded or split, both are closed and their rent goes back
//! to the payer, so a finished deposit no longer exists.
//!
//! Pay as you go uses two more accounts. A buyer tops up a balance
//! (`["balance", config, buyer]`, its USDC in the token account
//! `["vault", balance]`) and opens taps against it (`["tap", config, tap
//! id]`). A tap names one seller, a fee and a rate: the most the releaser may
//! bill per hour of the tap's life. The releaser settles a tap from time to
//! time for what it metered, never more than the rate times the time since
//! the last settle, counted up to seven days. A new tap starts one hour into
//! that time, so the first hour can be billed as soon as it opens; over its
//! whole life a tap pays out at most its rate times the time since it opened
//! plus one hour. The buyer can close a tap at any
//! time and withdraw the balance after asking 48 hours ahead, which leaves
//! the edge time to settle the last of it and needs nothing from the edge.
//!
//! Only the classic SPL token program is taken, the one Circle's USDC lives
//! on. The program keeps no id of its own, so the same build serves any
//! address it is deployed to.

#![cfg_attr(target_os = "solana", no_std)]

use pinocchio::cpi::{invoke_signed, Seed, Signer};
use pinocchio::error::{ProgramError, ProgramResult};
use pinocchio::instruction::{InstructionAccount, InstructionView};
use pinocchio::sysvars::clock::Clock;
use pinocchio::sysvars::instructions::{Instructions, INSTRUCTIONS_ID};
use pinocchio::sysvars::rent::Rent;
use pinocchio::sysvars::Sysvar;
use pinocchio::{AccountView, Address};

pub mod state;

use state::{Balance, Config, Deposit, Tap, TokenAccount};

#[cfg(target_os = "solana")]
mod entrypoint {
    pinocchio::program_entrypoint!(crate::process_instruction);
    pinocchio::no_allocator!();
    pinocchio::nostd_panic_handler!();
}

/// The classic SPL token program, `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`.
pub const TOKEN_PROGRAM: Address = Address::new_from_array([
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
]);
pub const SYSTEM_PROGRAM: Address = Address::new_from_array([0; 32]);
/// The native Ed25519 signature-check program,
/// `Ed25519SigVerify111111111111111111111111111`.
pub const ED25519_PROGRAM: Address = Address::new_from_array([
    3, 125, 70, 214, 124, 147, 251, 190, 18, 249, 66, 143, 131, 141, 64, 255, 5, 112, 116, 73, 39,
    244, 138, 100, 252, 202, 112, 68, 128, 0, 0, 0,
]);
/// What a key signs to agree on a deposit's end: this tag, the deposit's
/// address, and the amount going back to the payer (`u64` little-endian):
/// 0 for a release, the gross for a refund, anything between for a split.
pub const DECISION_TAG: &[u8; 24] = b"losos-escrow decision v1";
pub const DECISION_LEN: usize = 24 + 32 + 8;

/// The bytes a key signs to agree that `deposit` sends `refund` back.
#[must_use]
pub fn decision_message(deposit: &Address, refund: u64) -> [u8; DECISION_LEN] {
    let mut m = [0u8; DECISION_LEN];
    m[..24].copy_from_slice(DECISION_TAG);
    m[24..56].copy_from_slice(deposit.as_ref());
    m[56..].copy_from_slice(&refund.to_le_bytes());
    m
}

/// The same ceiling as `LososEscrow`: half, the fixed split of a machine
/// order.
pub const MAX_FEE_BPS: u128 = 5000;
/// A payer can take a deposit back only after its deadline, and the deadline
/// has to leave the edge time to check the payment: at least an hour. At
/// most 35 days: a month of storage or machines, plus the days the edge has
/// to pay the seller once the month is over.
pub const MIN_HOLD: i64 = 60 * 60;
pub const MAX_HOLD: i64 = 35 * 24 * 60 * 60;

/// A withdrawal waits this long after it is asked for, so the edge can
/// settle what the buyer used up to then: 48 hours.
pub const NOTICE: i64 = 48 * 60 * 60;
/// One settle bills at most this much of a tap's time: 7 days. Time beyond
/// it since the last settle is not billable.
pub const MAX_ACCRUAL: i64 = 7 * 24 * 60 * 60;
pub const HOUR: i64 = 60 * 60;
/// A new tap's billing starts this long before it opened, so the edge can
/// bill the first hour of use at once. In all, a tap can be billed at most
/// its rate times (the time since it opened + 1 hour).
pub const HEADSTART: i64 = 60 * 60;

pub const CONFIG_SEED: &[u8] = b"config";
pub const DEPOSIT_SEED: &[u8] = b"deposit";
pub const VAULT_SEED: &[u8] = b"vault";
pub const BALANCE_SEED: &[u8] = b"balance";
pub const TAP_SEED: &[u8] = b"tap";

/// The first byte of every instruction.
pub mod ix {
    /// `[releaser: 32]`. Accounts: treasury (signer, writable, pays the
    /// rent), config (writable), mint, system program.
    pub const INITIALIZE: u8 = 0;
    /// `[nonce: 32][order: 32][gross: u64][fee: u64][deadline: i64]
    /// [buyer box: 32][seller box: 32]`. Accounts: payer (signer,
    /// writable), payer's token account (writable), config, deposit
    /// (writable), vault (writable), seller, mint, token program, system
    /// program.
    pub const PAY: u8 = 1;
    /// Two of the three keys agree (see the crate's docs). Accounts:
    /// authority (signer), config, deposit (writable), vault (writable),
    /// seller's token account (writable), treasury's token account
    /// (writable), payer (writable), mint, token program, instructions
    /// sysvar.
    pub const RELEASE: u8 = 2;
    /// Two of the three keys agree, or the authority is the payer after the
    /// deadline. Accounts: authority (signer), config, deposit (writable),
    /// vault (writable), payer's token account (writable), payer (writable),
    /// mint, token program, instructions sysvar.
    pub const REFUND: u8 = 3;
    /// `[refund: u64]`, two of the three keys agreeing on it. Accounts:
    /// authority (signer), config, deposit (writable), vault (writable),
    /// payer's token account (writable), seller's token account (writable),
    /// treasury's token account (writable), payer (writable), mint, token
    /// program, instructions sysvar.
    pub const SETTLE: u8 = 4;
    /// `[next: 32]`. Accounts: treasury (signer), config (writable).
    pub const SET_RELEASER: u8 = 5;
    /// `[amount: u64]`. Accounts: buyer (signer, writable, pays the rent on
    /// first use), buyer's token account (writable), config, balance
    /// (writable), vault (writable), mint, token program, system program.
    pub const TOP_UP: u8 = 6;
    /// `[tap id: 32][rate: u64][fee bps: u16]`. Accounts: buyer (signer,
    /// writable, pays the rent), config, balance, tap (writable), seller,
    /// system program.
    pub const OPEN_TAP: u8 = 7;
    /// No data. Accounts: buyer or releaser (signer), config, tap
    /// (writable), buyer (writable, takes the rent if nothing is left to
    /// settle).
    pub const CLOSE_TAP: u8 = 8;
    /// `[amount: u64]`. Accounts: releaser (signer), config, tap (writable),
    /// balance, vault (writable), seller's token account (writable),
    /// treasury's token account (writable), buyer (writable, takes the rent
    /// when a closed tap is fully settled), mint, token program.
    pub const SETTLE_TAP: u8 = 9;
    /// `[amount: u64]`. Accounts: buyer (signer), config, balance (writable).
    pub const ASK_WITHDRAWAL: u8 = 10;
    /// No data. Accounts: buyer (signer), config, balance (writable), vault
    /// (writable), buyer's token account (writable), mint, token program.
    pub const WITHDRAW: u8 = 11;
}

/// Why an instruction was refused, as `Custom(n)`. The numbers are the
/// wire; the edge and the tests read them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum EscrowError {
    ZeroAddress = 1,
    ZeroSeller,
    ZeroAmount,
    FeeTooHigh,
    BadDeadline,
    NotReleaser,
    NotTreasury,
    NotHeld,
    TooEarly,
    BadSplit,
    /// An account is not the one the config or the deposit names.
    WrongAccount,
    WrongMint,
    /// A token account belongs to someone other than whom it pays.
    WrongOwner,
    /// The account is not a tap of this program: never opened, or closed
    /// and fully settled.
    NoTap,
    /// A tap with this id is already open.
    TapExists,
    /// The tap is already closed.
    TapClosed,
    /// More than the tap's rate allows for the time since its last settle.
    OverCap,
    /// More than the buyer's balance holds.
    OverBalance,
    /// The account is not a balance of this program.
    NoBalance,
    /// The buyer has asked for no withdrawal.
    NoWithdrawal,
    /// The two boxes' keys are not two keys other than the releaser's.
    BadBoxes,
    /// Fewer than two of the deposit's three keys agreed.
    NotApproved,
}

impl From<EscrowError> for ProgramError {
    fn from(e: EscrowError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

/// # Errors
/// The instruction is refused; nothing moved.
pub fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    data: &[u8],
) -> ProgramResult {
    let (tag, rest) = data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;
    match *tag {
        ix::INITIALIZE => initialize(program_id, accounts, rest),
        ix::PAY => pay(program_id, accounts, rest),
        ix::RELEASE => release(program_id, accounts),
        ix::REFUND => refund(program_id, accounts),
        ix::SETTLE => settle(program_id, accounts, rest),
        ix::SET_RELEASER => set_releaser(program_id, accounts, rest),
        ix::TOP_UP => top_up(program_id, accounts, rest),
        ix::OPEN_TAP => open_tap(program_id, accounts, rest),
        ix::CLOSE_TAP => close_tap(program_id, accounts),
        ix::SETTLE_TAP => settle_tap(program_id, accounts, rest),
        ix::ASK_WITHDRAWAL => ask_withdrawal(program_id, accounts, rest),
        ix::WITHDRAW => withdraw(program_id, accounts),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn initialize(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [treasury, config, mint, system, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let releaser = address_at(data, 0)?;
    if !treasury.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if releaser == Address::default() {
        return Err(EscrowError::ZeroAddress.into());
    }
    if system.address() != &SYSTEM_PROGRAM {
        return Err(ProgramError::IncorrectProgramId);
    }
    // A mint the token program made: the decimals check on every transfer
    // then holds against it.
    state::mint_decimals(mint)?;
    let (want, bump) = Address::find_program_address(
        &[
            CONFIG_SEED,
            treasury.address().as_ref(),
            mint.address().as_ref(),
        ],
        program_id,
    );
    if config.address() != &want {
        return Err(EscrowError::WrongAccount.into());
    }
    let bump_seed = [bump];
    let seeds = [
        Seed::from(CONFIG_SEED),
        Seed::from(treasury.address().as_ref()),
        Seed::from(mint.address().as_ref()),
        Seed::from(&bump_seed),
    ];
    create_account(treasury, config, Config::LEN, program_id, &seeds)?;
    let mut data = config.try_borrow_mut()?;
    Config {
        bump,
        mint: *mint.address(),
        treasury: *treasury.address(),
        releaser,
    }
    .write(&mut data);
    Ok(())
}

fn pay(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [payer, payer_token, config, deposit, vault, seller, mint, token, system, ..] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let nonce = bytes_at(data, 0)?;
    let order = bytes_at(data, 32)?;
    let gross = u64_at(data, 64)?;
    let fee = u64_at(data, 72)?;
    let deadline = u64_at(data, 80)? as i64;
    let buyer_box = address_at(data, 88)?;
    let seller_box = address_at(data, 120)?;
    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if token.address() != &TOKEN_PROGRAM || system.address() != &SYSTEM_PROGRAM {
        return Err(ProgramError::IncorrectProgramId);
    }
    let cfg = Config::load(program_id, config)?;
    if mint.address() != &cfg.mint {
        return Err(EscrowError::WrongMint.into());
    }
    if buyer_box == Address::default()
        || seller_box == Address::default()
        || buyer_box == seller_box
        || buyer_box == cfg.releaser
        || seller_box == cfg.releaser
    {
        return Err(EscrowError::BadBoxes.into());
    }
    let decimals = state::mint_decimals(mint)?;
    let seller_key = *seller.address();
    if seller_key == Address::default()
        || seller_key == cfg.treasury
        || seller_key == *config.address()
        || seller_key == *deposit.address()
        || seller_key == *vault.address()
    {
        return Err(EscrowError::ZeroSeller.into());
    }
    if gross == 0 {
        return Err(EscrowError::ZeroAmount.into());
    }
    if fee > gross || u128::from(fee) * 10_000 > u128::from(gross) * MAX_FEE_BPS {
        return Err(EscrowError::FeeTooHigh.into());
    }
    let now = Clock::get()?.unix_timestamp;
    if deadline < now.saturating_add(MIN_HOLD) || deadline > now.saturating_add(MAX_HOLD) {
        return Err(EscrowError::BadDeadline.into());
    }

    let (want, bump) = Address::find_program_address(
        &[DEPOSIT_SEED, config.address().as_ref(), &nonce],
        program_id,
    );
    if deposit.address() != &want {
        return Err(EscrowError::WrongAccount.into());
    }
    let (want_vault, vault_bump) =
        Address::find_program_address(&[VAULT_SEED, deposit.address().as_ref()], program_id);
    if vault.address() != &want_vault {
        return Err(EscrowError::WrongAccount.into());
    }

    let bump_seed = [bump];
    let deposit_seeds = [
        Seed::from(DEPOSIT_SEED),
        Seed::from(config.address().as_ref()),
        Seed::from(&nonce),
        Seed::from(&bump_seed),
    ];
    create_account(payer, deposit, Deposit::LEN, program_id, &deposit_seeds)?;
    let vault_bump_seed = [vault_bump];
    let vault_seeds = [
        Seed::from(VAULT_SEED),
        Seed::from(deposit.address().as_ref()),
        Seed::from(&vault_bump_seed),
    ];
    create_account(
        payer,
        vault,
        TokenAccount::LEN,
        &TOKEN_PROGRAM,
        &vault_seeds,
    )?;
    initialize_token_account(vault, mint, deposit.address())?;
    transfer(payer_token, mint, vault, payer, gross, decimals, &[])?;

    let mut data = deposit.try_borrow_mut()?;
    Deposit {
        bump,
        vault_bump,
        config: *config.address(),
        payer: *payer.address(),
        seller: seller_key,
        order,
        nonce,
        gross,
        fee,
        deadline,
        paid_at: now,
        buyer_box,
        seller_box,
    }
    .write(&mut data);
    Ok(())
}

/// The accounts every way out of a deposit shares, checked.
struct Held {
    cfg: Config,
    deposit: Deposit,
    decimals: u8,
    /// What the vault holds: the deposit, plus anything sent to it by plain
    /// transfer, which must go somewhere before the vault can close.
    balance: u64,
}

fn load_held(
    program_id: &Address,
    config: &AccountView,
    deposit: &AccountView,
    vault: &AccountView,
    payer: &AccountView,
    mint: &AccountView,
    token: &AccountView,
) -> Result<Held, ProgramError> {
    if token.address() != &TOKEN_PROGRAM {
        return Err(ProgramError::IncorrectProgramId);
    }
    let cfg = Config::load(program_id, config)?;
    let d = Deposit::load(program_id, deposit)?;
    if d.config != *config.address() {
        return Err(EscrowError::WrongAccount.into());
    }
    let want_vault = Address::derive_address(
        &[VAULT_SEED, deposit.address().as_ref()],
        Some(d.vault_bump),
        program_id,
    );
    if vault.address() != &want_vault {
        return Err(EscrowError::WrongAccount.into());
    }
    if payer.address() != &d.payer {
        return Err(EscrowError::WrongAccount.into());
    }
    if mint.address() != &cfg.mint {
        return Err(EscrowError::WrongMint.into());
    }
    let decimals = state::mint_decimals(mint)?;
    let balance = TokenAccount::load(vault)?.amount;
    Ok(Held {
        cfg,
        deposit: d,
        decimals,
        balance,
    })
}

/// A token account of `mint` owned by `owner`.
fn check_destination(account: &AccountView, mint: &Address, owner: &Address) -> ProgramResult {
    let t = TokenAccount::load(account)?;
    if t.mint != *mint {
        return Err(EscrowError::WrongMint.into());
    }
    if t.owner != *owner {
        return Err(EscrowError::WrongOwner.into());
    }
    Ok(())
}

/// Which of the deposit's three keys `key` is, as a bit: 1 the releaser, 2
/// the buyer's box, 4 the seller's box, 0 none.
fn party(cfg: &Config, d: &Deposit, key: &Address) -> u8 {
    if key == &cfg.releaser {
        1
    } else if key == &d.buyer_box {
        2
    } else if key == &d.seller_box {
        4
    } else {
        0
    }
}

/// The keys that agree on `deposit` sending `refund` back: the authority,
/// if it signed and is one of the three, and every one of the three whose
/// signature over [`decision_message`] an Ed25519 signature-check
/// instruction anywhere in this transaction verified. Only checks whose
/// key, signature and message all sit in their own instruction's data are
/// counted, so one cannot point at bytes elsewhere.
fn agreed(
    cfg: &Config,
    d: &Deposit,
    deposit: &Address,
    authority: &AccountView,
    sysvar: &AccountView,
    refund: u64,
) -> Result<u8, ProgramError> {
    let mut seen = 0;
    if authority.is_signer() {
        seen |= party(cfg, d, authority.address());
    }
    if sysvar.address() != &INSTRUCTIONS_ID {
        return Err(ProgramError::UnsupportedSysvar);
    }
    let want = decision_message(deposit, refund);
    let ixs = Instructions::try_from(sysvar)?;
    for i in 0..ixs.num_instructions() {
        let ix = ixs.load_instruction_at(i)?;
        if ix.get_program_id() != &ED25519_PROGRAM {
            continue;
        }
        let data = ix.get_instruction_data();
        let count = usize::from(*data.first().ok_or(ProgramError::InvalidInstructionData)?);
        for n in 0..count {
            let at = 2 + n * 14;
            let key_at = usize::from(u16_at(data, at + 4)?);
            let message_at = usize::from(u16_at(data, at + 8)?);
            let message_len = usize::from(u16_at(data, at + 10)?);
            let own = [at + 2, at + 6, at + 12]
                .iter()
                .all(|&o| u16_at(data, o) == Ok(u16::MAX));
            if !own || message_len != DECISION_LEN {
                continue;
            }
            if data.get(message_at..message_at + DECISION_LEN) != Some(&want[..]) {
                continue;
            }
            seen |= party(cfg, d, &address_at(data, key_at)?);
        }
    }
    Ok(seen)
}

/// Refuse unless at least two of the deposit's three keys agree.
fn approve(
    h: &Held,
    deposit: &Address,
    authority: &AccountView,
    sysvar: &AccountView,
    refund: u64,
) -> ProgramResult {
    match agreed(&h.cfg, &h.deposit, deposit, authority, sysvar, refund)? {
        3 | 5 | 6 | 7 => Ok(()),
        _ => Err(EscrowError::NotApproved.into()),
    }
}

/// Fee to the treasury, the rest to the seller recorded in `pay`, when two
/// of the three keys agree.
fn release(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
    let [authority, config, deposit, vault, seller_token, treasury_token, payer, mint, token, sysvar, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let h = load_held(program_id, config, deposit, vault, payer, mint, token)?;
    approve(&h, deposit.address(), authority, sysvar, 0)?;
    check_destination(seller_token, &h.cfg.mint, &h.deposit.seller)?;
    check_destination(treasury_token, &h.cfg.mint, &h.cfg.treasury)?;
    let stray = h.balance - h.deposit.gross;
    let d = &h.deposit;
    with_deposit_signer(d, |signer| {
        transfer(
            vault,
            mint,
            treasury_token,
            deposit,
            d.fee + stray,
            h.decimals,
            signer,
        )?;
        transfer(
            vault,
            mint,
            seller_token,
            deposit,
            d.gross - d.fee,
            h.decimals,
            signer,
        )?;
        close_token_account(vault, payer, deposit, signer)
    })?;
    close_account(deposit, payer)
}

/// Back to the payer, whole: at any time when two of the three keys agree
/// (the seller's address failed its check, the order was paid twice,
/// nothing could be delivered), or by the payer alone after the deadline.
fn refund(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
    let [authority, config, deposit, vault, payer_token, payer, mint, token, sysvar, ..] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let h = load_held(program_id, config, deposit, vault, payer, mint, token)?;
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let by_payer = authority.address() == &h.deposit.payer;
    if by_payer && Clock::get()?.unix_timestamp >= h.deposit.deadline {
        // The payer's own way out once nobody agreed in time.
    } else if let Err(e) = approve(&h, deposit.address(), authority, sysvar, h.deposit.gross) {
        // The payer alone is only early, not refused for good.
        return Err(if by_payer {
            EscrowError::TooEarly.into()
        } else {
            e
        });
    }
    check_destination(payer_token, &h.cfg.mint, &h.deposit.payer)?;
    let d = &h.deposit;
    with_deposit_signer(d, |signer| {
        transfer(
            vault,
            mint,
            payer_token,
            deposit,
            h.balance,
            h.decimals,
            signer,
        )?;
        close_token_account(vault, payer, deposit, signer)
    })?;
    close_account(deposit, payer)
}

/// Part back to the payer, the rest released: the seller's box went offline
/// partway through what was bought, so the payer gets back the share that
/// was not delivered and the seller keeps the share that was. The fee
/// shrinks with the seller's share, rounded down. Two of the three keys
/// agree on this exact `refund`. For all or nothing, see `refund` and
/// `release`.
fn settle(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [authority, config, deposit, vault, payer_token, seller_token, treasury_token, payer, mint, token, sysvar, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let back = u64_at(data, 0)?;
    let h = load_held(program_id, config, deposit, vault, payer, mint, token)?;
    let d = &h.deposit;
    if back == 0 || back >= d.gross {
        return Err(EscrowError::BadSplit.into());
    }
    approve(&h, deposit.address(), authority, sysvar, back)?;
    check_destination(payer_token, &h.cfg.mint, &d.payer)?;
    check_destination(seller_token, &h.cfg.mint, &d.seller)?;
    check_destination(treasury_token, &h.cfg.mint, &h.cfg.treasury)?;
    let (fee, to_seller) = split(d.gross, d.fee, back);
    let stray = h.balance - d.gross;
    with_deposit_signer(d, |signer| {
        transfer(vault, mint, payer_token, deposit, back, h.decimals, signer)?;
        transfer(
            vault,
            mint,
            treasury_token,
            deposit,
            fee + stray,
            h.decimals,
            signer,
        )?;
        transfer(
            vault,
            mint,
            seller_token,
            deposit,
            to_seller,
            h.decimals,
            signer,
        )?;
        close_token_account(vault, payer, deposit, signer)
    })?;
    close_account(deposit, payer)
}

/// The fee and the seller's share when `back` of `gross` goes back to the
/// payer: the fee scales with what the seller keeps, rounded down, as
/// `LososEscrow.settle` does.
#[must_use]
pub fn split(gross: u64, fee: u64, back: u64) -> (u64, u64) {
    let kept = gross - back;
    // fee <= gross, so the quotient is at most `kept` and fits.
    let fee = (u128::from(fee) * u128::from(kept) / u128::from(gross)) as u64;
    (fee, kept - fee)
}

/// A new edge key, for a rotation or after a compromise.
fn set_releaser(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [treasury, config, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let next = address_at(data, 0)?;
    let mut cfg = Config::load(program_id, config)?;
    if !treasury.is_signer() || treasury.address() != &cfg.treasury {
        return Err(EscrowError::NotTreasury.into());
    }
    if next == Address::default() {
        return Err(EscrowError::ZeroAddress.into());
    }
    cfg.releaser = next;
    cfg.write(&mut config.try_borrow_mut()?);
    Ok(())
}

/// Money into the buyer's balance. The first top-up makes the balance and
/// its vault, at the buyer's cost.
fn top_up(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [buyer, buyer_token, config, balance, vault, mint, token, system, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let amount = u64_at(data, 0)?;
    if !buyer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if token.address() != &TOKEN_PROGRAM || system.address() != &SYSTEM_PROGRAM {
        return Err(ProgramError::IncorrectProgramId);
    }
    let cfg = Config::load(program_id, config)?;
    if mint.address() != &cfg.mint {
        return Err(EscrowError::WrongMint.into());
    }
    let decimals = state::mint_decimals(mint)?;
    if amount == 0 {
        return Err(EscrowError::ZeroAmount.into());
    }
    if balance.owned_by(program_id) {
        let b = Balance::load(program_id, balance)?;
        if b.config != *config.address() || b.buyer != *buyer.address() {
            return Err(EscrowError::WrongAccount.into());
        }
        check_vault(program_id, balance, &b, vault)?;
    } else {
        let (want, bump) = Address::find_program_address(
            &[
                BALANCE_SEED,
                config.address().as_ref(),
                buyer.address().as_ref(),
            ],
            program_id,
        );
        if balance.address() != &want {
            return Err(EscrowError::WrongAccount.into());
        }
        let (want_vault, vault_bump) =
            Address::find_program_address(&[VAULT_SEED, balance.address().as_ref()], program_id);
        if vault.address() != &want_vault {
            return Err(EscrowError::WrongAccount.into());
        }
        let bump_seed = [bump];
        let balance_seeds = [
            Seed::from(BALANCE_SEED),
            Seed::from(config.address().as_ref()),
            Seed::from(buyer.address().as_ref()),
            Seed::from(&bump_seed),
        ];
        open_account(buyer, balance, Balance::LEN, program_id, &balance_seeds)?;
        let vault_bump_seed = [vault_bump];
        let vault_seeds = [
            Seed::from(VAULT_SEED),
            Seed::from(balance.address().as_ref()),
            Seed::from(&vault_bump_seed),
        ];
        open_account(
            buyer,
            vault,
            TokenAccount::LEN,
            &TOKEN_PROGRAM,
            &vault_seeds,
        )?;
        initialize_token_account(vault, mint, balance.address())?;
        Balance {
            bump,
            vault_bump,
            config: *config.address(),
            buyer: *buyer.address(),
            asked: 0,
            not_before: 0,
        }
        .write(&mut balance.try_borrow_mut()?);
    }
    transfer(buyer_token, mint, vault, buyer, amount, decimals, &[])
}

/// The buyer lets the releaser bill one seller from the balance at up to
/// `rate` an hour, `fee_bps` of each settle to the treasury. Billing counts
/// from `HEADSTART` before now, so the first hour can be settled at once.
fn open_tap(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [buyer, config, balance, tap, seller, system, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let id = bytes_at(data, 0)?;
    let rate = u64_at(data, 32)?;
    let fee_bps = u16_at(data, 40)?;
    if !buyer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if system.address() != &SYSTEM_PROGRAM {
        return Err(ProgramError::IncorrectProgramId);
    }
    let cfg = Config::load(program_id, config)?;
    let b = Balance::load(program_id, balance)?;
    if b.config != *config.address() || b.buyer != *buyer.address() {
        return Err(EscrowError::WrongAccount.into());
    }
    if u128::from(fee_bps) > MAX_FEE_BPS {
        return Err(EscrowError::FeeTooHigh.into());
    }
    if rate == 0 {
        return Err(EscrowError::ZeroAmount.into());
    }
    let vault = vault_of(program_id, balance, &b);
    let seller_key = *seller.address();
    if seller_key == Address::default()
        || seller_key == cfg.treasury
        || seller_key == *config.address()
        || seller_key == *balance.address()
        || seller_key == vault
        || seller_key == *tap.address()
    {
        return Err(EscrowError::ZeroSeller.into());
    }
    if tap.owned_by(program_id) {
        return Err(EscrowError::TapExists.into());
    }
    // The buyer is in the seeds, so the same id opened by another wallet is
    // another tap: nobody can take a buyer's quoted ids first.
    let (want, bump) = Address::find_program_address(
        &[
            TAP_SEED,
            config.address().as_ref(),
            buyer.address().as_ref(),
            &id,
        ],
        program_id,
    );
    if tap.address() != &want {
        return Err(EscrowError::WrongAccount.into());
    }
    let bump_seed = [bump];
    let seeds = [
        Seed::from(TAP_SEED),
        Seed::from(config.address().as_ref()),
        Seed::from(buyer.address().as_ref()),
        Seed::from(&id),
        Seed::from(&bump_seed),
    ];
    open_account(buyer, tap, Tap::LEN, program_id, &seeds)?;
    Tap {
        bump,
        config: *config.address(),
        buyer: *buyer.address(),
        seller: seller_key,
        id,
        rate,
        settled_at: Clock::get()?
            .unix_timestamp
            .saturating_sub(HEADSTART)
            .max(0),
        closed_at: None,
        fee_bps,
    }
    .write(&mut tap.try_borrow_mut()?);
    Ok(())
}

/// The buyer or the releaser ends a tap's billing now. The releaser may
/// still settle it, for time up to now. A tap with nothing left to settle
/// is closed at once and its rent goes back to the buyer.
fn close_tap(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
    let [authority, config, tap, buyer, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let cfg = Config::load(program_id, config)?;
    let mut t = Tap::load(program_id, tap)?;
    if t.config != *config.address() {
        return Err(EscrowError::WrongAccount.into());
    }
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if authority.address() != &cfg.releaser && authority.address() != &t.buyer {
        return Err(EscrowError::NotReleaser.into());
    }
    if t.closed_at.is_some() {
        return Err(EscrowError::TapClosed.into());
    }
    if buyer.address() != &t.buyer {
        return Err(EscrowError::WrongAccount.into());
    }
    // Never before the last settle, so the time left to bill is never
    // negative.
    let closed_at = Clock::get()?.unix_timestamp.max(t.settled_at);
    if closed_at == t.settled_at {
        return close_account(tap, buyer);
    }
    t.closed_at = Some(closed_at);
    t.write(&mut tap.try_borrow_mut()?);
    Ok(())
}

/// The releaser bills `amount` of what it metered on a tap: at most the
/// tap's rate times the time since the last settle (or since `HEADSTART`
/// before the tap opened), up to now or up to when the tap closed, counting
/// at most `MAX_ACCRUAL` of it. The fee
/// (`fee_bps` of the amount, rounded down) goes to the treasury and the rest
/// to the tap's seller. Cap not used is not carried over. A closed tap
/// settled up to its close is finished: its account is closed and its rent
/// goes back to the buyer. On a closed tap `amount` may be 0, which only
/// finishes it.
fn settle_tap(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [releaser, config, tap, balance, vault, seller_token, treasury_token, buyer, mint, token, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let amount = u64_at(data, 0)?;
    if token.address() != &TOKEN_PROGRAM {
        return Err(ProgramError::IncorrectProgramId);
    }
    let cfg = Config::load(program_id, config)?;
    if !releaser.is_signer() || releaser.address() != &cfg.releaser {
        return Err(EscrowError::NotReleaser.into());
    }
    let mut t = Tap::load(program_id, tap)?;
    if t.config != *config.address() {
        return Err(EscrowError::WrongAccount.into());
    }
    let b = Balance::load(program_id, balance)?;
    if b.config != *config.address() || b.buyer != t.buyer {
        return Err(EscrowError::WrongAccount.into());
    }
    check_vault(program_id, balance, &b, vault)?;
    if buyer.address() != &t.buyer {
        return Err(EscrowError::WrongAccount.into());
    }
    if mint.address() != &cfg.mint {
        return Err(EscrowError::WrongMint.into());
    }
    let decimals = state::mint_decimals(mint)?;
    check_destination(seller_token, &cfg.mint, &t.seller)?;
    check_destination(treasury_token, &cfg.mint, &cfg.treasury)?;

    let end = match t.closed_at {
        Some(closed_at) => closed_at,
        None => Clock::get()?.unix_timestamp,
    };
    if end <= t.settled_at {
        return Err(EscrowError::TooEarly.into());
    }
    if amount == 0 && t.closed_at.is_none() {
        return Err(EscrowError::ZeroAmount.into());
    }
    if u128::from(amount) > cap(t.rate, end - t.settled_at) {
        return Err(EscrowError::OverCap.into());
    }
    if amount > TokenAccount::load(vault)?.amount {
        return Err(EscrowError::OverBalance.into());
    }
    let fee = tap_fee(amount, t.fee_bps);
    with_balance_signer(&b, |signer| {
        transfer(vault, mint, treasury_token, balance, fee, decimals, signer)?;
        transfer(
            vault,
            mint,
            seller_token,
            balance,
            amount - fee,
            decimals,
            signer,
        )
    })?;
    t.settled_at = end;
    if t.closed_at.is_some() {
        return close_account(tap, buyer);
    }
    t.write(&mut tap.try_borrow_mut()?);
    Ok(())
}

/// The most a tap at `rate` an hour can bill for `elapsed` seconds:
/// `rate * min(elapsed, MAX_ACCRUAL) / 3600`, rounded down.
#[must_use]
pub fn cap(rate: u64, elapsed: i64) -> u128 {
    let elapsed = elapsed.clamp(0, MAX_ACCRUAL) as u128;
    u128::from(rate) * elapsed / HOUR as u128
}

/// The treasury's part of a settle: `fee_bps` of `amount`, rounded down.
#[must_use]
pub fn tap_fee(amount: u64, fee_bps: u16) -> u64 {
    // A tap's fee_bps is at most MAX_FEE_BPS, so the quotient is at most
    // half the amount and fits.
    (u128::from(amount) * u128::from(fee_bps) / 10_000) as u64
}

/// The buyer asks to take `amount` out of the balance. It can be withdrawn
/// after `NOTICE`; a new ask replaces the old one and waits again.
fn ask_withdrawal(
    program_id: &Address,
    accounts: &mut [AccountView],
    data: &[u8],
) -> ProgramResult {
    let [buyer, config, balance, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let amount = u64_at(data, 0)?;
    if !buyer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Config::load(program_id, config)?;
    let mut b = Balance::load(program_id, balance)?;
    if b.config != *config.address() || b.buyer != *buyer.address() {
        return Err(EscrowError::WrongAccount.into());
    }
    if amount == 0 {
        return Err(EscrowError::ZeroAmount.into());
    }
    b.asked = amount;
    b.not_before = Clock::get()?
        .unix_timestamp
        .checked_add(NOTICE)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    b.write(&mut balance.try_borrow_mut()?);
    Ok(())
}

/// After the notice the buyer takes what was asked, or the whole balance if
/// settles since left less, to their own token account. The ask is then
/// spent.
fn withdraw(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
    let [buyer, config, balance, vault, buyer_token, mint, token, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !buyer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if token.address() != &TOKEN_PROGRAM {
        return Err(ProgramError::IncorrectProgramId);
    }
    let cfg = Config::load(program_id, config)?;
    let mut b = Balance::load(program_id, balance)?;
    if b.config != *config.address() || b.buyer != *buyer.address() {
        return Err(EscrowError::WrongAccount.into());
    }
    check_vault(program_id, balance, &b, vault)?;
    if mint.address() != &cfg.mint {
        return Err(EscrowError::WrongMint.into());
    }
    let decimals = state::mint_decimals(mint)?;
    if b.asked == 0 {
        return Err(EscrowError::NoWithdrawal.into());
    }
    if Clock::get()?.unix_timestamp < b.not_before {
        return Err(EscrowError::TooEarly.into());
    }
    check_destination(buyer_token, &cfg.mint, &b.buyer)?;
    let out = b.asked.min(TokenAccount::load(vault)?.amount);
    b.asked = 0;
    b.not_before = 0;
    b.write(&mut balance.try_borrow_mut()?);
    with_balance_signer(&b, |signer| {
        transfer(vault, mint, buyer_token, balance, out, decimals, signer)
    })
}

/// The vault address of a balance.
fn vault_of(program_id: &Address, balance: &AccountView, b: &Balance) -> Address {
    Address::derive_address(
        &[VAULT_SEED, balance.address().as_ref()],
        Some(b.vault_bump),
        program_id,
    )
}

fn check_vault(
    program_id: &Address,
    balance: &AccountView,
    b: &Balance,
    vault: &AccountView,
) -> ProgramResult {
    if vault.address() != &vault_of(program_id, balance, b) {
        return Err(EscrowError::WrongAccount.into());
    }
    Ok(())
}

fn with_balance_signer(b: &Balance, f: impl FnOnce(&[Signer]) -> ProgramResult) -> ProgramResult {
    let bump = [b.bump];
    let seeds = [
        Seed::from(BALANCE_SEED),
        Seed::from(b.config.as_ref()),
        Seed::from(b.buyer.as_ref()),
        Seed::from(&bump),
    ];
    f(&[Signer::from(&seeds)])
}

fn with_deposit_signer(d: &Deposit, f: impl FnOnce(&[Signer]) -> ProgramResult) -> ProgramResult {
    let bump = [d.bump];
    let seeds = [
        Seed::from(DEPOSIT_SEED),
        Seed::from(d.config.as_ref()),
        Seed::from(&d.nonce),
        Seed::from(&bump),
    ];
    f(&[Signer::from(&seeds)])
}

/// Hand a deposit's or a tap's lamports to the payer and close it, so a
/// finished one no longer exists and cannot be acted on again.
fn close_account(account: &mut AccountView, payer: &mut AccountView) -> ProgramResult {
    let lamports = account.lamports();
    payer.set_lamports(
        payer
            .lamports()
            .checked_add(lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?,
    );
    account.set_lamports(0);
    account.close()
}

fn create_account(
    from: &AccountView,
    to: &AccountView,
    space: usize,
    owner: &Address,
    seeds: &[Seed],
) -> ProgramResult {
    let lamports = Rent::get()?.try_minimum_balance(space)?;
    let mut data = [0u8; 52];
    // SystemInstruction::CreateAccount, a little-endian u32.
    data[4..12].copy_from_slice(&lamports.to_le_bytes());
    data[12..20].copy_from_slice(&(space as u64).to_le_bytes());
    data[20..52].copy_from_slice(owner.as_ref());
    let metas = [
        InstructionAccount::writable_signer(from.address()),
        InstructionAccount::writable_signer(to.address()),
    ];
    invoke_signed(
        &InstructionView {
            program_id: &SYSTEM_PROGRAM,
            data: &data,
            accounts: &metas,
        },
        &[from, to],
        &[Signer::from(seeds)],
    )
}

/// `create_account`, except that an address someone already sent lamports
/// to is topped up to the rent, allocated and assigned instead. A balance's
/// address is known in advance, and a gift of one lamport must not stop its
/// buyer from ever topping up.
fn open_account(
    from: &AccountView,
    to: &AccountView,
    space: usize,
    owner: &Address,
    seeds: &[Seed],
) -> ProgramResult {
    let held = to.lamports();
    if held == 0 {
        return create_account(from, to, space, owner, seeds);
    }
    let need = Rent::get()?
        .try_minimum_balance(space)?
        .saturating_sub(held);
    if need > 0 {
        let mut data = [0u8; 12];
        data[0] = 2; // SystemInstruction::Transfer
        data[4..12].copy_from_slice(&need.to_le_bytes());
        let metas = [
            InstructionAccount::writable_signer(from.address()),
            InstructionAccount::writable(to.address()),
        ];
        invoke_signed(
            &InstructionView {
                program_id: &SYSTEM_PROGRAM,
                data: &data,
                accounts: &metas,
            },
            &[from, to],
            &[],
        )?;
    }
    let metas = [InstructionAccount::writable_signer(to.address())];
    let mut allocate = [0u8; 12];
    allocate[0] = 8; // SystemInstruction::Allocate
    allocate[4..12].copy_from_slice(&(space as u64).to_le_bytes());
    invoke_signed(
        &InstructionView {
            program_id: &SYSTEM_PROGRAM,
            data: &allocate,
            accounts: &metas,
        },
        &[to],
        &[Signer::from(seeds)],
    )?;
    let mut assign = [0u8; 36];
    assign[0] = 1; // SystemInstruction::Assign
    assign[4..36].copy_from_slice(owner.as_ref());
    invoke_signed(
        &InstructionView {
            program_id: &SYSTEM_PROGRAM,
            data: &assign,
            accounts: &metas,
        },
        &[to],
        &[Signer::from(seeds)],
    )
}

fn initialize_token_account(
    account: &AccountView,
    mint: &AccountView,
    owner: &Address,
) -> ProgramResult {
    let mut data = [0u8; 33];
    data[0] = 18; // InitializeAccount3
    data[1..].copy_from_slice(owner.as_ref());
    let metas = [
        InstructionAccount::writable(account.address()),
        InstructionAccount::readonly(mint.address()),
    ];
    invoke_signed(
        &InstructionView {
            program_id: &TOKEN_PROGRAM,
            data: &data,
            accounts: &metas,
        },
        &[account, mint],
        &[],
    )
}

fn transfer(
    from: &AccountView,
    mint: &AccountView,
    to: &AccountView,
    authority: &AccountView,
    amount: u64,
    decimals: u8,
    signers: &[Signer],
) -> ProgramResult {
    if amount == 0 {
        return Ok(());
    }
    let mut data = [0u8; 10];
    data[0] = 12; // TransferChecked
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    data[9] = decimals;
    let metas = [
        InstructionAccount::writable(from.address()),
        InstructionAccount::readonly(mint.address()),
        InstructionAccount::writable(to.address()),
        InstructionAccount::readonly_signer(authority.address()),
    ];
    invoke_signed(
        &InstructionView {
            program_id: &TOKEN_PROGRAM,
            data: &data,
            accounts: &metas,
        },
        &[from, mint, to, authority],
        signers,
    )
}

fn close_token_account(
    account: &AccountView,
    destination: &AccountView,
    authority: &AccountView,
    signers: &[Signer],
) -> ProgramResult {
    let metas = [
        InstructionAccount::writable(account.address()),
        InstructionAccount::writable(destination.address()),
        InstructionAccount::readonly_signer(authority.address()),
    ];
    invoke_signed(
        &InstructionView {
            program_id: &TOKEN_PROGRAM,
            data: &[9], // CloseAccount
            accounts: &metas,
        },
        &[account, destination, authority],
        signers,
    )
}

fn bytes_at(data: &[u8], at: usize) -> Result<[u8; 32], ProgramError> {
    data.get(at..at + 32)
        .and_then(|b| b.try_into().ok())
        .ok_or(ProgramError::InvalidInstructionData)
}

fn address_at(data: &[u8], at: usize) -> Result<Address, ProgramError> {
    bytes_at(data, at).map(Address::new_from_array)
}

fn u16_at(data: &[u8], at: usize) -> Result<u16, ProgramError> {
    data.get(at..at + 2)
        .and_then(|b| b.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(ProgramError::InvalidInstructionData)
}

fn u64_at(data: &[u8], at: usize) -> Result<u64, ProgramError> {
    data.get(at..at + 8)
        .and_then(|b| b.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(ProgramError::InvalidInstructionData)
}

#[cfg(test)]
mod tests {
    use super::state::{Balance, Tap};
    use super::{cap, split, tap_fee, Address, MAX_ACCRUAL};

    /// The bytes the edge's client decodes in its own tests
    /// (`edge/registrar/src/solana_escrow.rs`): change both together.
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
    fn the_layouts_the_edge_reads_are_pinned() {
        let balance = Balance {
            bump: 254,
            vault_bump: 253,
            config: Address::new_from_array([1; 32]),
            buyer: Address::new_from_array([2; 32]),
            asked: 7_000_000,
            not_before: 1_800_172_800,
        };
        let mut data = [0u8; Balance::LEN];
        balance.write(&mut data);
        assert_eq!(data.to_vec(), hex(BALANCE_FIXTURE));
        assert_eq!(Balance::read(&data), Some(balance));

        let closed = Tap {
            bump: 255,
            config: Address::new_from_array([1; 32]),
            buyer: Address::new_from_array([2; 32]),
            seller: Address::new_from_array([3; 32]),
            id: [4; 32],
            rate: 1_000_000,
            settled_at: 1_800_000_000,
            closed_at: Some(1_800_003_600),
            fee_bps: 400,
        };
        let mut data = [0u8; Tap::LEN];
        closed.write(&mut data);
        assert_eq!(data.to_vec(), hex(TAP_CLOSED_FIXTURE));
        assert_eq!(Tap::read(&data), Some(closed));
        let open = Tap {
            closed_at: None,
            ..closed
        };
        open.write(&mut data);
        assert_eq!(data.to_vec(), hex(TAP_OPEN_FIXTURE));
        assert_eq!(Tap::read(&data), Some(open));
        // A closed flag other than 0 or 1 is not a tap.
        data[2] = 2;
        assert_eq!(Tap::read(&data), None);
    }

    #[test]
    fn a_tap_bills_at_most_its_rate_for_at_most_a_week() {
        assert_eq!(cap(3_600, 1), 1);
        assert_eq!(cap(1_000_000, 90 * 60), 1_500_000);
        // Rounded down.
        assert_eq!(cap(1, 3_599), 0);
        assert_eq!(cap(7, 1_800), 3);
        // A week at most, and never for time that has not passed.
        assert_eq!(cap(1_000, 30 * 24 * 3600), cap(1_000, MAX_ACCRUAL));
        assert_eq!(cap(1_000, -5), 0);
        // The largest rate for the longest time still fits.
        assert_eq!(cap(u64::MAX, MAX_ACCRUAL), u128::from(u64::MAX) * 7 * 24);
        assert_eq!(tap_fee(999, 400), 39);
        assert_eq!(tap_fee(24, 400), 0);
        assert_eq!(tap_fee(u64::MAX, 5000), u64::MAX / 2);
    }

    #[test]
    fn a_split_scales_the_fee_with_what_the_seller_keeps() {
        // The registrar's own example: 2000 cents with a 400 fee, 1401 back.
        let (fee, seller) = split(20_000_000, 4_000_000, 14_010_000);
        assert_eq!(fee, 1_198_000);
        assert_eq!(seller, 4_792_000);
        assert_eq!(14_010_000 + fee + seller, 20_000_000);
        // Rounded down, never above the whole fee.
        assert_eq!(split(3, 1, 1), (0, 2));
        assert_eq!(split(u64::MAX, u64::MAX / 2, 1).0, u64::MAX / 2 - 1);
    }
}
