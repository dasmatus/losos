//! The two accounts this program writes, and the two token-program accounts
//! it reads, as fixed byte layouts. Every integer is little-endian.

use pinocchio::error::ProgramError;
use pinocchio::{AccountView, Address};

use crate::{EscrowError, TOKEN_PROGRAM};

const CONFIG_TAG: u8 = 1;
const DEPOSIT_TAG: u8 = 2;

/// `["config", treasury, mint]`: who is paid the fee and who releases.
///
/// | offset | field    |
/// | ------ | -------- |
/// | 0      | tag (1)  |
/// | 1      | bump     |
/// | 2      | mint     |
/// | 34     | treasury |
/// | 66     | releaser |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub bump: u8,
    pub mint: Address,
    pub treasury: Address,
    pub releaser: Address,
}

impl Config {
    pub const LEN: usize = 98;

    /// # Errors
    /// The account is not a config this program wrote.
    pub fn load(program_id: &Address, account: &AccountView) -> Result<Self, ProgramError> {
        if !account.owned_by(program_id) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        let data = account.try_borrow()?;
        Self::read(&data).ok_or(EscrowError::WrongAccount.into())
    }

    #[must_use]
    pub fn read(data: &[u8]) -> Option<Self> {
        if data.len() != Self::LEN || data[0] != CONFIG_TAG {
            return None;
        }
        Some(Self {
            bump: data[1],
            mint: address(data, 2),
            treasury: address(data, 34),
            releaser: address(data, 66),
        })
    }

    pub fn write(&self, data: &mut [u8]) {
        data[0] = CONFIG_TAG;
        data[1] = self.bump;
        data[2..34].copy_from_slice(self.mint.as_ref());
        data[34..66].copy_from_slice(self.treasury.as_ref());
        data[66..98].copy_from_slice(self.releaser.as_ref());
    }
}

/// `["deposit", config, nonce]`: one held payment. It exists only while the
/// payment is held.
///
/// | offset | field      |
/// | ------ | ---------- |
/// | 0      | tag (2)    |
/// | 1      | bump       |
/// | 2      | vault bump |
/// | 3      | zero       |
/// | 4      | config     |
/// | 36     | payer      |
/// | 68     | seller     |
/// | 100    | order      |
/// | 132    | nonce      |
/// | 164    | gross      |
/// | 172    | fee        |
/// | 180    | deadline   |
/// | 188    | paid at    |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deposit {
    pub bump: u8,
    pub vault_bump: u8,
    pub config: Address,
    pub payer: Address,
    pub seller: Address,
    /// keccak-256 of the edge's order id: the invoice's `order_id_hash`.
    pub order: [u8; 32],
    pub nonce: [u8; 32],
    pub gross: u64,
    pub fee: u64,
    /// Unix seconds; after it the payer may refund alone.
    pub deadline: i64,
    pub paid_at: i64,
}

impl Deposit {
    pub const LEN: usize = 196;

    /// # Errors
    /// The account is not a held deposit of this program.
    pub fn load(program_id: &Address, account: &AccountView) -> Result<Self, ProgramError> {
        if !account.owned_by(program_id) {
            return Err(EscrowError::NotHeld.into());
        }
        let data = account.try_borrow()?;
        Self::read(&data).ok_or(EscrowError::NotHeld.into())
    }

    #[must_use]
    pub fn read(data: &[u8]) -> Option<Self> {
        if data.len() != Self::LEN || data[0] != DEPOSIT_TAG {
            return None;
        }
        Some(Self {
            bump: data[1],
            vault_bump: data[2],
            config: address(data, 4),
            payer: address(data, 36),
            seller: address(data, 68),
            order: bytes(data, 100),
            nonce: bytes(data, 132),
            gross: u64_le(data, 164),
            fee: u64_le(data, 172),
            deadline: u64_le(data, 180) as i64,
            paid_at: u64_le(data, 188) as i64,
        })
    }

    pub fn write(&self, data: &mut [u8]) {
        data[0] = DEPOSIT_TAG;
        data[1] = self.bump;
        data[2] = self.vault_bump;
        data[3] = 0;
        data[4..36].copy_from_slice(self.config.as_ref());
        data[36..68].copy_from_slice(self.payer.as_ref());
        data[68..100].copy_from_slice(self.seller.as_ref());
        data[100..132].copy_from_slice(&self.order);
        data[132..164].copy_from_slice(&self.nonce);
        data[164..172].copy_from_slice(&self.gross.to_le_bytes());
        data[172..180].copy_from_slice(&self.fee.to_le_bytes());
        data[180..188].copy_from_slice(&self.deadline.to_le_bytes());
        data[188..196].copy_from_slice(&self.paid_at.to_le_bytes());
    }
}

/// The parts of an SPL token account this program reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenAccount {
    pub mint: Address,
    pub owner: Address,
    pub amount: u64,
}

impl TokenAccount {
    pub const LEN: usize = 165;

    /// # Errors
    /// Not an initialized token account of the classic token program.
    pub fn load(account: &AccountView) -> Result<Self, ProgramError> {
        if !account.owned_by(&TOKEN_PROGRAM) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        let data = account.try_borrow()?;
        // State at 108: 1 initialized, 2 frozen. A frozen account cannot
        // send or take, and the transfer would fail anyway.
        if data.len() != Self::LEN || data[108] != 1 {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(Self {
            mint: address(&data, 0),
            owner: address(&data, 32),
            amount: u64_le(&data, 64),
        })
    }
}

/// The decimals of an initialized mint of the classic token program.
///
/// # Errors
/// Anything else.
pub fn mint_decimals(account: &AccountView) -> Result<u8, ProgramError> {
    if !account.owned_by(&TOKEN_PROGRAM) {
        return Err(EscrowError::WrongMint.into());
    }
    let data = account.try_borrow()?;
    // Mint: authority option (36), supply (8), decimals, initialized, ...
    if data.len() != 82 || data[45] != 1 {
        return Err(EscrowError::WrongMint.into());
    }
    Ok(data[44])
}

fn bytes(data: &[u8], at: usize) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&data[at..at + 32]);
    out
}

fn address(data: &[u8], at: usize) -> Address {
    Address::new_from_array(bytes(data, at))
}

fn u64_le(data: &[u8], at: usize) -> u64 {
    let mut out = [0u8; 8];
    out.copy_from_slice(&data[at..at + 8]);
    u64::from_le_bytes(out)
}
