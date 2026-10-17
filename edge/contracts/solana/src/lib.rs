//! `losos-escrow`: the Solana twin of `LososEscrow.sol`.
//!
//! It holds one USDC payment per deposit for as long as what it bought
//! runs. The edge then releases it to the treasury and the seller recorded at
//! payment time, refunds it to the payer, or splits it between the two when
//! the seller stopped delivering partway. The edge's key can do nothing
//! else: it cannot change where a deposit goes. If the edge never answers,
//! the payer takes the deposit back after its deadline.
//!
//! One config account per treasury and mint (`["config", treasury, mint]`)
//! names the releaser. Each payment is a deposit account
//! (`["deposit", config, nonce]`, a nonce the wallet picks at random) and a
//! token account (`["vault", deposit]`) that the deposit owns. When a deposit
//! is released, refunded or split, both are closed and their rent goes back
//! to the payer, so a finished deposit no longer exists.
//!
//! Only the classic SPL token program is taken, the one Circle's USDC lives
//! on. The program keeps no id of its own, so the same build serves any
//! address it is deployed to.

#![cfg_attr(target_os = "solana", no_std)]

use pinocchio::cpi::{invoke_signed, Seed, Signer};
use pinocchio::error::{ProgramError, ProgramResult};
use pinocchio::instruction::{InstructionAccount, InstructionView};
use pinocchio::sysvars::clock::Clock;
use pinocchio::sysvars::rent::Rent;
use pinocchio::sysvars::Sysvar;
use pinocchio::{AccountView, Address};

pub mod state;

use state::{Config, Deposit, TokenAccount};

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

/// The same ceiling as `LososEscrow`: half, the fixed split of a machine
/// order.
pub const MAX_FEE_BPS: u128 = 5000;
/// A payer can take a deposit back only after its deadline, and the deadline
/// has to leave the edge time to check the payment: at least an hour. At
/// most 35 days: a month of storage or machines, plus the days the edge has
/// to pay the seller once the month is over.
pub const MIN_HOLD: i64 = 60 * 60;
pub const MAX_HOLD: i64 = 35 * 24 * 60 * 60;

pub const CONFIG_SEED: &[u8] = b"config";
pub const DEPOSIT_SEED: &[u8] = b"deposit";
pub const VAULT_SEED: &[u8] = b"vault";

/// The first byte of every instruction.
pub mod ix {
    /// `[releaser: 32]`. Accounts: treasury (signer, writable, pays the
    /// rent), config (writable), mint, system program.
    pub const INITIALIZE: u8 = 0;
    /// `[nonce: 32][order: 32][gross: u64][fee: u64][deadline: i64]`.
    /// Accounts: payer (signer, writable), payer's token account (writable),
    /// config, deposit (writable), vault (writable), seller, mint, token
    /// program, system program.
    pub const PAY: u8 = 1;
    /// Accounts: releaser (signer), config, deposit (writable), vault
    /// (writable), seller's token account (writable), treasury's token
    /// account (writable), payer (writable), mint, token program.
    pub const RELEASE: u8 = 2;
    /// Accounts: releaser, or the payer after the deadline (signer), config,
    /// deposit (writable), vault (writable), payer's token account
    /// (writable), payer (writable), mint, token program.
    pub const REFUND: u8 = 3;
    /// `[refund: u64]`. Accounts: releaser (signer), config, deposit
    /// (writable), vault (writable), payer's token account (writable),
    /// seller's token account (writable), treasury's token account
    /// (writable), payer (writable), mint, token program.
    pub const SETTLE: u8 = 4;
    /// `[next: 32]`. Accounts: treasury (signer), config (writable).
    pub const SET_RELEASER: u8 = 5;
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

/// The edge has checked the order and the seller's address: fee to the
/// treasury, the rest to the seller recorded in `pay`.
fn release(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
    let [releaser, config, deposit, vault, seller_token, treasury_token, payer, mint, token, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let h = load_held(program_id, config, deposit, vault, payer, mint, token)?;
    if !releaser.is_signer() || releaser.address() != &h.cfg.releaser {
        return Err(EscrowError::NotReleaser.into());
    }
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
    close_deposit(deposit, payer)
}

/// Back to the payer, whole. The edge may refund at any time (the seller's
/// address failed its check, the order was paid twice, nothing could be
/// delivered). The payer may refund only after the deadline.
fn refund(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
    let [authority, config, deposit, vault, payer_token, payer, mint, token, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let h = load_held(program_id, config, deposit, vault, payer, mint, token)?;
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if authority.address() != &h.cfg.releaser {
        if authority.address() != &h.deposit.payer {
            return Err(EscrowError::NotReleaser.into());
        }
        if Clock::get()?.unix_timestamp < h.deposit.deadline {
            return Err(EscrowError::TooEarly.into());
        }
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
    close_deposit(deposit, payer)
}

/// Part back to the payer, the rest released: the seller's box went offline
/// partway through what was bought, so the payer gets back the share that
/// was not delivered and the seller keeps the share that was. The fee
/// shrinks with the seller's share, rounded down. For all or nothing, the
/// edge calls `refund` or `release`.
fn settle(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [releaser, config, deposit, vault, payer_token, seller_token, treasury_token, payer, mint, token, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let back = u64_at(data, 0)?;
    let h = load_held(program_id, config, deposit, vault, payer, mint, token)?;
    if !releaser.is_signer() || releaser.address() != &h.cfg.releaser {
        return Err(EscrowError::NotReleaser.into());
    }
    let d = &h.deposit;
    if back == 0 || back >= d.gross {
        return Err(EscrowError::BadSplit.into());
    }
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
    close_deposit(deposit, payer)
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

/// Hand the deposit account's lamports to the payer and close it, so a
/// finished deposit no longer exists and cannot be acted on again.
fn close_deposit(deposit: &mut AccountView, payer: &mut AccountView) -> ProgramResult {
    let lamports = deposit.lamports();
    payer.set_lamports(
        payer
            .lamports()
            .checked_add(lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?,
    );
    deposit.set_lamports(0);
    deposit.close()
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

fn u64_at(data: &[u8], at: usize) -> Result<u64, ProgramError> {
    data.get(at..at + 8)
        .and_then(|b| b.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(ProgramError::InvalidInstructionData)
}

#[cfg(test)]
mod tests {
    use super::split;

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
