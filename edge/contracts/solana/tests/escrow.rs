//! The program as the Solana runtime runs it: the `.so` that `cargo
//! build-sbf` makes, in LiteSVM with the real SPL token program. Every rule
//! `LososEscrow.sol` keeps is checked here, and every way a payer, a seller
//! or a stranger could take money that is not theirs.
//!
//! ```sh
//! cargo build-sbf && cargo test
//! ```

use litesvm::LiteSVM;
use losos_escrow::state::{Config, Deposit};
use losos_escrow::{ix, EscrowError, CONFIG_SEED, DEPOSIT_SEED, VAULT_SEED};
use solana_account::Account;
use solana_clock::Clock;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_signer::Signer;
use solana_transaction::Transaction;
use solana_transaction_error::TransactionError;

type Address = solana_address::Address;

const TOKEN: Address = Address::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const TOKEN_2022: Address = Address::from_str_const("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
const SYSTEM: Address = Address::new_from_array([0; 32]);
const NOW: i64 = 1_800_000_000;
const DAY: i64 = 24 * 60 * 60;
/// 20.00 USDC, a 4.00 fee: the registrar's own split example in base units.
const GROSS: u64 = 20_000_000;
const FEE: u64 = 4_000_000;

struct Edge {
    svm: LiteSVM,
    program: Address,
    mint: Address,
    treasury: Keypair,
    releaser: Keypair,
    config: Address,
    payer: Keypair,
    seller: Keypair,
}

fn program_so() -> Vec<u8> {
    let path = std::env::var("LOSOS_ESCROW_SO").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/target/deploy/losos_escrow.so").to_string()
    });
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}; run `cargo build-sbf` first"))
}

fn mint_data(decimals: u8) -> Vec<u8> {
    let mut d = vec![0u8; 82];
    d[44] = decimals;
    d[45] = 1;
    d
}

fn token_data(mint: &Address, owner: &Address, amount: u64) -> Vec<u8> {
    let mut d = vec![0u8; 165];
    d[0..32].copy_from_slice(mint.as_ref());
    d[32..64].copy_from_slice(owner.as_ref());
    d[64..72].copy_from_slice(&amount.to_le_bytes());
    d[108] = 1;
    d
}

impl Edge {
    fn new() -> Self {
        let mut svm = LiteSVM::new();
        let program = Address::new_unique();
        svm.add_program(program, &program_so()).unwrap();
        let mint = Address::new_unique();
        let rent = svm.minimum_balance_for_rent_exemption(82);
        svm.set_account(mint, account(rent, mint_data(6), TOKEN))
            .unwrap();
        let treasury = Keypair::new();
        let releaser = Keypair::new();
        let payer = Keypair::new();
        let seller = Keypair::new();
        for k in [&treasury, &releaser, &payer, &seller] {
            svm.airdrop(&k.pubkey(), 10_000_000_000).unwrap();
        }
        let config = Address::find_program_address(
            &[CONFIG_SEED, treasury.pubkey().as_ref(), mint.as_ref()],
            &program,
        )
        .0;
        let mut edge = Self {
            svm,
            program,
            mint,
            treasury,
            releaser,
            config,
            payer,
            seller,
        };
        edge.at(NOW);
        let init = edge.initialize_ix(&edge.treasury.pubkey(), edge.config, edge.releaser.pubkey());
        edge.send(&[init], &[&edge.treasury.insecure_clone()])
            .unwrap();
        // Every party already has a USDC account; the payer holds 100.
        let payer = edge.payer.pubkey();
        edge.token_account(&payer, 100_000_000);
        edge.token_account(&edge.seller.pubkey(), 0);
        edge.token_account(&edge.treasury.pubkey(), 0);
        edge
    }

    fn at(&mut self, unix: i64) {
        let mut clock: Clock = self.svm.get_sysvar();
        clock.unix_timestamp = unix;
        self.svm.set_sysvar(&clock);
    }

    /// A token account of the mint at an address derived from its owner, so
    /// the tests can name it again.
    fn token_account(&mut self, owner: &Address, amount: u64) -> Address {
        let address = ata(owner);
        let rent = self.svm.minimum_balance_for_rent_exemption(165);
        self.svm
            .set_account(
                address,
                account(rent, token_data(&self.mint, owner, amount), TOKEN),
            )
            .unwrap();
        address
    }

    fn balance(&self, token_account: &Address) -> u64 {
        let a = self.svm.get_account(token_account).unwrap();
        u64::from_le_bytes(a.data[64..72].try_into().unwrap())
    }

    fn lamports(&self, address: &Address) -> u64 {
        self.svm.get_account(address).map_or(0, |a| a.lamports)
    }

    fn send(&mut self, ixs: &[Instruction], signers: &[&Keypair]) -> Result<(), TransactionError> {
        self.svm.expire_blockhash();
        let message = Message::new(ixs, Some(&signers[0].pubkey()));
        let tx = Transaction::new(signers, message, self.svm.latest_blockhash());
        self.svm.send_transaction(tx).map(|_| ()).map_err(|f| f.err)
    }

    fn initialize_ix(&self, treasury: &Address, config: Address, releaser: Address) -> Instruction {
        let mut data = vec![ix::INITIALIZE];
        data.extend_from_slice(releaser.as_ref());
        Instruction {
            program_id: self.program,
            accounts: vec![
                AccountMeta::new(*treasury, true),
                AccountMeta::new(config, false),
                AccountMeta::new_readonly(self.mint, false),
                AccountMeta::new_readonly(SYSTEM, false),
            ],
            data,
        }
    }

    fn deposit_of(&self, nonce: &[u8; 32]) -> (Address, Address) {
        let deposit = Address::find_program_address(
            &[DEPOSIT_SEED, self.config.as_ref(), nonce],
            &self.program,
        )
        .0;
        let vault = Address::find_program_address(&[VAULT_SEED, deposit.as_ref()], &self.program).0;
        (deposit, vault)
    }

    fn pay_ix(&self, p: &Pay) -> Instruction {
        let (deposit, vault) = self.deposit_of(&p.nonce);
        let mut data = vec![ix::PAY];
        data.extend_from_slice(&p.nonce);
        data.extend_from_slice(&[7u8; 32]);
        data.extend_from_slice(&p.gross.to_le_bytes());
        data.extend_from_slice(&p.fee.to_le_bytes());
        data.extend_from_slice(&p.deadline.to_le_bytes());
        Instruction {
            program_id: self.program,
            accounts: vec![
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new(ata(&self.payer.pubkey()), false),
                AccountMeta::new_readonly(p.config.unwrap_or(self.config), false),
                AccountMeta::new(p.deposit.unwrap_or(deposit), false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(p.seller.unwrap_or(self.seller.pubkey()), false),
                AccountMeta::new_readonly(p.mint.unwrap_or(self.mint), false),
                AccountMeta::new_readonly(p.token.unwrap_or(TOKEN), false),
                AccountMeta::new_readonly(SYSTEM, false),
            ],
            data,
        }
    }

    fn pay(&mut self, p: &Pay) -> Result<(), TransactionError> {
        let ix = self.pay_ix(p);
        let payer = self.payer.insecure_clone();
        self.send(&[ix], &[&payer])
    }

    /// Pay the standard order and return its nonce.
    fn paid(&mut self, n: u8) -> [u8; 32] {
        let p = Pay::new(n);
        self.pay(&p).unwrap();
        p.nonce
    }

    fn finish_ix(&self, tag: u8, signer: &Address, nonce: &[u8; 32], to: Finish) -> Instruction {
        let (deposit, vault) = self.deposit_of(nonce);
        let payer = to.payer.unwrap_or(self.payer.pubkey());
        let payer_token = to.payer_token.unwrap_or(ata(&self.payer.pubkey()));
        let seller_token = to.seller_token.unwrap_or(ata(&self.seller.pubkey()));
        let treasury_token = to.treasury_token.unwrap_or(ata(&self.treasury.pubkey()));
        let vault = to.vault.unwrap_or(vault);
        let deposit = to.deposit.unwrap_or(deposit);
        let config = to.config.unwrap_or(self.config);
        let mut data = vec![tag];
        let mut accounts = vec![
            AccountMeta::new_readonly(*signer, true),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(deposit, false),
            AccountMeta::new(vault, false),
        ];
        match tag {
            ix::RELEASE => {
                accounts.push(AccountMeta::new(seller_token, false));
                accounts.push(AccountMeta::new(treasury_token, false));
            }
            ix::REFUND => accounts.push(AccountMeta::new(payer_token, false)),
            _ => {
                data.extend_from_slice(&to.refund.to_le_bytes());
                accounts.push(AccountMeta::new(payer_token, false));
                accounts.push(AccountMeta::new(seller_token, false));
                accounts.push(AccountMeta::new(treasury_token, false));
            }
        }
        accounts.push(AccountMeta::new(payer, false));
        accounts.push(AccountMeta::new_readonly(self.mint, false));
        accounts.push(AccountMeta::new_readonly(TOKEN, false));
        Instruction {
            program_id: self.program,
            accounts,
            data,
        }
    }

    fn finish(
        &mut self,
        tag: u8,
        by: &Keypair,
        nonce: &[u8; 32],
        to: Finish,
    ) -> Result<(), TransactionError> {
        let ix = self.finish_ix(tag, &by.pubkey(), nonce, to);
        // The signer pays its own fee, so a refused instruction costs only it.
        self.send(&[ix], &[by])
    }

    fn release(&mut self, by: &Keypair, nonce: &[u8; 32]) -> Result<(), TransactionError> {
        self.finish(ix::RELEASE, by, nonce, Finish::default())
    }

    fn refund(&mut self, by: &Keypair, nonce: &[u8; 32]) -> Result<(), TransactionError> {
        self.finish(ix::REFUND, by, nonce, Finish::default())
    }

    fn settle(
        &mut self,
        by: &Keypair,
        nonce: &[u8; 32],
        refund: u64,
    ) -> Result<(), TransactionError> {
        self.finish(
            ix::SETTLE,
            by,
            nonce,
            Finish {
                refund,
                ..Finish::default()
            },
        )
    }

    fn releaser(&self) -> Keypair {
        self.releaser.insecure_clone()
    }

    fn payer(&self) -> Keypair {
        self.payer.insecure_clone()
    }

    fn seller(&self) -> Keypair {
        self.seller.insecure_clone()
    }

    fn treasury(&self) -> Keypair {
        self.treasury.insecure_clone()
    }

    fn held(&self, nonce: &[u8; 32]) -> Option<Deposit> {
        let (deposit, _) = self.deposit_of(nonce);
        self.svm
            .get_account(&deposit)
            .filter(|a| a.owner == self.program && !a.data.is_empty())
            .and_then(|a| Deposit::read(&a.data))
    }
}

#[derive(Clone)]
struct Pay {
    nonce: [u8; 32],
    gross: u64,
    fee: u64,
    deadline: i64,
    seller: Option<Address>,
    mint: Option<Address>,
    token: Option<Address>,
    deposit: Option<Address>,
    config: Option<Address>,
}

impl Pay {
    fn new(n: u8) -> Self {
        Self {
            nonce: [n; 32],
            gross: GROSS,
            fee: FEE,
            deadline: NOW + 33 * DAY,
            seller: None,
            mint: None,
            token: None,
            deposit: None,
            config: None,
        }
    }
}

#[derive(Default)]
struct Finish {
    refund: u64,
    payer: Option<Address>,
    payer_token: Option<Address>,
    seller_token: Option<Address>,
    treasury_token: Option<Address>,
    vault: Option<Address>,
    deposit: Option<Address>,
    config: Option<Address>,
}

fn account(lamports: u64, data: Vec<u8>, owner: Address) -> Account {
    Account {
        lamports,
        data,
        owner,
        executable: false,
        rent_epoch: 0,
    }
}

/// Not the real associated token address: a stable address per owner, which
/// is all the program asks of a token account (it reads the owner field).
fn ata(owner: &Address) -> Address {
    Address::find_program_address(&[b"usdc", owner.as_ref()], &TOKEN).0
}

fn custom(r: Result<(), TransactionError>) -> u32 {
    match r {
        Err(TransactionError::InstructionError(
            _,
            solana_instruction::error::InstructionError::Custom(n),
        )) => n,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn refused(r: Result<(), TransactionError>) {
    assert!(r.is_err(), "expected a refusal");
}

const fn code(e: EscrowError) -> u32 {
    e as u32
}

#[test]
fn a_payment_is_held_and_released_to_the_seller_and_the_treasury() {
    let mut e = Edge::new();
    let payer_lamports = e.lamports(&e.payer.pubkey());
    let nonce = e.paid(1);
    let (deposit, vault) = e.deposit_of(&nonce);
    assert_eq!(e.balance(&ata(&e.payer.pubkey())), 100_000_000 - GROSS);
    assert_eq!(e.balance(&vault), GROSS);
    let d = e.held(&nonce).unwrap();
    assert_eq!(d.payer.as_ref(), e.payer.pubkey().as_ref());
    assert_eq!(d.seller.as_ref(), e.seller.pubkey().as_ref());
    assert_eq!(d.config.as_ref(), e.config.as_ref());
    assert_eq!((d.gross, d.fee, d.order), (GROSS, FEE, [7u8; 32]));
    assert_eq!((d.deadline, d.paid_at), (NOW + 33 * DAY, NOW));
    let rent = e.lamports(&deposit) + e.lamports(&vault);
    assert!(rent > 0);

    let releaser = e.releaser();
    e.release(&releaser, &nonce).unwrap();
    assert_eq!(e.balance(&ata(&e.seller.pubkey())), GROSS - FEE);
    assert_eq!(e.balance(&ata(&e.treasury.pubkey())), FEE);
    // Both accounts are gone and their rent is the payer's again: the payer
    // is down only the network fee of its own transaction.
    assert!(e.held(&nonce).is_none());
    assert_eq!(e.lamports(&vault), 0);
    assert_eq!(e.lamports(&deposit), 0);
    assert_eq!(e.lamports(&e.payer.pubkey()), payer_lamports - 5000);
    // Nothing is left to act on.
    assert_eq!(
        custom(e.release(&releaser, &nonce)),
        code(EscrowError::NotHeld)
    );
    assert_eq!(
        custom(e.refund(&releaser, &nonce)),
        code(EscrowError::NotHeld)
    );
}

#[test]
fn only_the_releaser_releases_or_splits() {
    let mut e = Edge::new();
    let nonce = e.paid(1);
    for who in [e.seller(), e.payer(), e.treasury(), Keypair::new()] {
        e.svm.airdrop(&who.pubkey(), 1_000_000_000).unwrap();
        assert_eq!(
            custom(e.release(&who, &nonce)),
            code(EscrowError::NotReleaser)
        );
        assert_eq!(
            custom(e.settle(&who, &nonce, 1)),
            code(EscrowError::NotReleaser)
        );
    }
    assert!(e.held(&nonce).is_some());
}

#[test]
fn a_release_pays_only_the_seller_and_treasury_the_deposit_names() {
    let mut e = Edge::new();
    let nonce = e.paid(1);
    let releaser = e.releaser();
    let thief = Keypair::new().pubkey();
    let theirs = e.token_account(&thief, 0);
    for to in [
        Finish {
            seller_token: Some(theirs),
            ..Finish::default()
        },
        Finish {
            treasury_token: Some(theirs),
            ..Finish::default()
        },
        Finish {
            seller_token: Some(ata(&e.treasury.pubkey())),
            ..Finish::default()
        },
    ] {
        assert_eq!(
            custom(e.finish(ix::RELEASE, &releaser, &nonce, to)),
            code(EscrowError::WrongOwner)
        );
    }
    // Nor the rent to anyone but the payer.
    assert_eq!(
        custom(e.finish(
            ix::RELEASE,
            &releaser,
            &nonce,
            Finish {
                payer: Some(thief),
                ..Finish::default()
            }
        )),
        code(EscrowError::WrongAccount)
    );
    // Nor from a vault that is not the deposit's.
    let other = e.paid(2);
    let (_, other_vault) = e.deposit_of(&other);
    assert_eq!(
        custom(e.finish(
            ix::RELEASE,
            &releaser,
            &nonce,
            Finish {
                vault: Some(other_vault),
                ..Finish::default()
            }
        )),
        code(EscrowError::WrongAccount)
    );
    assert_eq!(e.balance(&theirs), 0);
    e.release(&releaser, &nonce).unwrap();
}

#[test]
fn the_payer_takes_it_back_only_after_the_deadline() {
    let mut e = Edge::new();
    let nonce = e.paid(1);
    let payer = e.payer();
    assert_eq!(
        custom(e.refund(&payer, &nonce)),
        code(EscrowError::TooEarly)
    );
    e.at(NOW + 33 * DAY - 1);
    assert_eq!(
        custom(e.refund(&payer, &nonce)),
        code(EscrowError::TooEarly)
    );
    // A stranger never can, deadline or not.
    let stranger = Keypair::new();
    e.svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();
    e.at(NOW + 40 * DAY);
    assert_eq!(
        custom(e.refund(&stranger, &nonce)),
        code(EscrowError::NotReleaser)
    );
    let seller = e.seller();
    assert_eq!(
        custom(e.refund(&seller, &nonce)),
        code(EscrowError::NotReleaser)
    );
    e.at(NOW + 33 * DAY);
    e.refund(&payer, &nonce).unwrap();
    assert_eq!(e.balance(&ata(&e.payer.pubkey())), 100_000_000);
    assert!(e.held(&nonce).is_none());
    // And once it is back, the edge cannot release it.
    let releaser = e.releaser();
    assert_eq!(
        custom(e.release(&releaser, &nonce)),
        code(EscrowError::NotHeld)
    );
}

#[test]
fn the_releaser_refunds_at_any_time_but_only_to_the_payer() {
    let mut e = Edge::new();
    let nonce = e.paid(1);
    let releaser = e.releaser();
    let seller_token = ata(&e.seller.pubkey());
    assert_eq!(
        custom(e.finish(
            ix::REFUND,
            &releaser,
            &nonce,
            Finish {
                payer_token: Some(seller_token),
                ..Finish::default()
            }
        )),
        code(EscrowError::WrongOwner)
    );
    e.refund(&releaser, &nonce).unwrap();
    assert_eq!(e.balance(&ata(&e.payer.pubkey())), 100_000_000);
    assert_eq!(e.balance(&seller_token), 0);
}

#[test]
fn a_split_refunds_the_part_not_delivered_and_scales_the_fee() {
    let mut e = Edge::new();
    let nonce = e.paid(1);
    let releaser = e.releaser();
    assert_eq!(
        custom(e.settle(&releaser, &nonce, 0)),
        code(EscrowError::BadSplit)
    );
    assert_eq!(
        custom(e.settle(&releaser, &nonce, GROSS)),
        code(EscrowError::BadSplit)
    );
    assert_eq!(
        custom(e.settle(&releaser, &nonce, GROSS + 1)),
        code(EscrowError::BadSplit)
    );
    // 14.01 of 20.00 back: the registrar's own example.
    e.settle(&releaser, &nonce, 14_010_000).unwrap();
    assert_eq!(
        e.balance(&ata(&e.payer.pubkey())),
        100_000_000 - GROSS + 14_010_000
    );
    assert_eq!(e.balance(&ata(&e.treasury.pubkey())), 1_198_000);
    assert_eq!(e.balance(&ata(&e.seller.pubkey())), 4_792_000);
    assert!(e.held(&nonce).is_none());
}

#[test]
fn tokens_sent_to_a_vault_cannot_keep_it_open() {
    let mut e = Edge::new();
    let releaser = e.releaser();
    // Someone pads each vault by plain transfer; the program sweeps it.
    let a = e.paid(1);
    let b = e.paid(2);
    let c = e.paid(3);
    for nonce in [&a, &b, &c] {
        let (_, vault) = e.deposit_of(nonce);
        let mut acct = e.svm.get_account(&vault).unwrap();
        acct.data[64..72].copy_from_slice(&(GROSS + 1).to_le_bytes());
        e.svm.set_account(vault, acct).unwrap();
    }
    e.release(&releaser, &a).unwrap();
    assert_eq!(e.balance(&ata(&e.seller.pubkey())), GROSS - FEE);
    assert_eq!(e.balance(&ata(&e.treasury.pubkey())), FEE + 1);
    e.settle(&releaser, &b, 1).unwrap();
    e.refund(&releaser, &c).unwrap();
    for nonce in [&a, &b, &c] {
        let (_, vault) = e.deposit_of(nonce);
        assert_eq!(e.lamports(&vault), 0);
    }
}

#[test]
fn a_payment_must_fit_the_rules_the_evm_escrow_keeps() {
    let mut e = Edge::new();
    let cases: Vec<(Pay, u32)> = vec![
        (
            Pay {
                gross: 0,
                fee: 0,
                ..Pay::new(1)
            },
            code(EscrowError::ZeroAmount),
        ),
        // More than half is never a fee.
        (
            Pay {
                fee: GROSS / 2 + 1,
                ..Pay::new(1)
            },
            code(EscrowError::FeeTooHigh),
        ),
        (
            Pay {
                gross: 3,
                fee: 2,
                ..Pay::new(1)
            },
            code(EscrowError::FeeTooHigh),
        ),
        (
            Pay {
                deadline: NOW + 60 * 60 - 1,
                ..Pay::new(1)
            },
            code(EscrowError::BadDeadline),
        ),
        (
            Pay {
                deadline: NOW + 35 * DAY + 1,
                ..Pay::new(1)
            },
            code(EscrowError::BadDeadline),
        ),
        (
            Pay {
                seller: Some(Address::default()),
                ..Pay::new(1)
            },
            code(EscrowError::ZeroSeller),
        ),
        (
            Pay {
                seller: Some(e.treasury.pubkey()),
                ..Pay::new(1)
            },
            code(EscrowError::ZeroSeller),
        ),
        (
            Pay {
                seller: Some(e.config),
                ..Pay::new(1)
            },
            code(EscrowError::ZeroSeller),
        ),
        (
            Pay {
                deposit: Some(Address::new_unique()),
                ..Pay::new(1)
            },
            code(EscrowError::WrongAccount),
        ),
    ];
    for (p, want) in cases {
        assert_eq!(
            custom(e.pay(&p)),
            want,
            "deadline {} fee {}",
            p.deadline,
            p.fee
        );
    }
    // Exactly half, and the two deadline bounds, are fine.
    e.pay(&Pay {
        fee: GROSS / 2,
        ..Pay::new(2)
    })
    .unwrap();
    e.pay(&Pay {
        deadline: NOW + 60 * 60,
        ..Pay::new(3)
    })
    .unwrap();
    e.pay(&Pay {
        deadline: NOW + 35 * DAY,
        ..Pay::new(4)
    })
    .unwrap();
}

#[test]
fn only_circles_mint_and_the_classic_token_program_are_taken() {
    let mut e = Edge::new();
    let lookalike = Address::new_unique();
    let rent = e.svm.minimum_balance_for_rent_exemption(82);
    e.svm
        .set_account(lookalike, account(rent, mint_data(6), TOKEN))
        .unwrap();
    assert_eq!(
        custom(e.pay(&Pay {
            mint: Some(lookalike),
            ..Pay::new(1)
        })),
        code(EscrowError::WrongMint)
    );
    refused(e.pay(&Pay {
        token: Some(TOKEN_2022),
        ..Pay::new(1)
    }));
    e.paid(1);
}

#[test]
fn a_nonce_pays_once() {
    let mut e = Edge::new();
    e.paid(1);
    refused(e.pay(&Pay::new(1)));
    assert_eq!(e.balance(&ata(&e.payer.pubkey())), 100_000_000 - GROSS);
}

#[test]
fn a_deposit_or_config_of_another_program_or_treasury_is_not_held() {
    let mut e = Edge::new();
    let nonce = e.paid(1);
    let releaser = e.releaser();
    // A look-alike deposit written by someone else's program.
    let fake = Address::new_unique();
    let (real, _) = e.deposit_of(&nonce);
    let mut data = e.svm.get_account(&real).unwrap();
    data.owner = Address::new_unique();
    e.svm.set_account(fake, data).unwrap();
    assert_eq!(
        custom(e.finish(
            ix::RELEASE,
            &releaser,
            &nonce,
            Finish {
                deposit: Some(fake),
                ..Finish::default()
            }
        )),
        code(EscrowError::NotHeld)
    );
    // A second treasury's config, whose releaser is a stranger, cannot move
    // this treasury's deposits.
    let other_treasury = Keypair::new();
    let stranger = Keypair::new();
    for k in [&other_treasury, &stranger] {
        e.svm.airdrop(&k.pubkey(), 1_000_000_000).unwrap();
    }
    let other_config = Address::find_program_address(
        &[
            CONFIG_SEED,
            other_treasury.pubkey().as_ref(),
            e.mint.as_ref(),
        ],
        &e.program,
    )
    .0;
    let init = e.initialize_ix(&other_treasury.pubkey(), other_config, stranger.pubkey());
    e.send(&[init], &[&other_treasury]).unwrap();
    assert_eq!(
        custom(e.finish(
            ix::RELEASE,
            &stranger,
            &nonce,
            Finish {
                config: Some(other_config),
                ..Finish::default()
            }
        )),
        code(EscrowError::WrongAccount)
    );
    e.release(&releaser, &nonce).unwrap();
}

#[test]
fn a_config_is_made_once_by_its_treasury() {
    let mut e = Edge::new();
    let treasury = e.treasury();
    let again = e.initialize_ix(&treasury.pubkey(), e.config, Keypair::new().pubkey());
    refused(e.send(&[again], &[&treasury]));
    // Someone else cannot make a config in another treasury's name: the
    // address is the treasury's own and the treasury must sign.
    let mallory = Keypair::new();
    e.svm.airdrop(&mallory.pubkey(), 1_000_000_000).unwrap();
    let victim = Keypair::new().pubkey();
    let victim_config =
        Address::find_program_address(&[CONFIG_SEED, victim.as_ref(), e.mint.as_ref()], &e.program)
            .0;
    let mut forged = e.initialize_ix(&victim, victim_config, mallory.pubkey());
    forged.accounts[0].is_signer = false;
    refused(e.send(&[forged], &[&mallory]));
    let wrong_seat = e.initialize_ix(&mallory.pubkey(), victim_config, mallory.pubkey());
    assert_eq!(
        custom(e.send(&[wrong_seat], &[&mallory])),
        code(EscrowError::WrongAccount)
    );
    let cfg = Config::read(&e.svm.get_account(&e.config).unwrap().data).unwrap();
    assert_eq!(cfg.releaser.as_ref(), e.releaser.pubkey().as_ref());
}

#[test]
fn the_treasury_rotates_the_releaser_and_the_old_key_stops() {
    let mut e = Edge::new();
    let nonce = e.paid(1);
    let next = Keypair::new();
    e.svm.airdrop(&next.pubkey(), 1_000_000_000).unwrap();
    let rotate = |by: &Address| {
        let mut data = vec![ix::SET_RELEASER];
        data.extend_from_slice(next.pubkey().as_ref());
        Instruction {
            program_id: e.program,
            accounts: vec![
                AccountMeta::new_readonly(*by, true),
                AccountMeta::new(e.config, false),
            ],
            data,
        }
    };
    let releaser = e.releaser();
    let by_releaser = rotate(&releaser.pubkey());
    let by_treasury = rotate(&e.treasury.pubkey());
    assert_eq!(
        custom(e.send(&[by_releaser], &[&releaser])),
        code(EscrowError::NotTreasury)
    );
    let treasury = e.treasury();
    e.send(&[by_treasury], &[&treasury]).unwrap();
    assert_eq!(
        custom(e.release(&releaser, &nonce)),
        code(EscrowError::NotReleaser)
    );
    e.release(&next, &nonce).unwrap();
}

#[test]
fn a_payment_fits_one_transaction_with_room_to_spare() {
    // The wallet sends `pay` alone; the edge sends a release with two
    // account creations in front. Both must fit the default compute limit.
    let mut e = Edge::new();
    let p = Pay::new(9);
    let ix = e.pay_ix(&p);
    let payer = e.payer();
    e.svm.expire_blockhash();
    let tx = Transaction::new(
        &[&payer],
        Message::new(&[ix], Some(&payer.pubkey())),
        e.svm.latest_blockhash(),
    );
    let meta = e.svm.send_transaction(tx).unwrap();
    assert!(
        meta.compute_units_consumed < 60_000,
        "{}",
        meta.compute_units_consumed
    );
    let releaser = e.releaser();
    let ix = e.finish_ix(
        ix::SETTLE,
        &releaser.pubkey(),
        &p.nonce,
        Finish {
            refund: 1,
            ..Finish::default()
        },
    );
    e.svm.expire_blockhash();
    let tx = Transaction::new(
        &[&releaser],
        Message::new(&[ix], Some(&releaser.pubkey())),
        e.svm.latest_blockhash(),
    );
    let meta = e.svm.send_transaction(tx).unwrap();
    assert!(
        meta.compute_units_consumed < 60_000,
        "{}",
        meta.compute_units_consumed
    );
}
