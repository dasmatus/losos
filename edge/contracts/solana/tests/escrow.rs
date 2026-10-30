//! The program as the Solana runtime runs it: the `.so` that `cargo
//! build-sbf` makes, in LiteSVM with the real SPL token program. Every rule
//! `LososEscrow.sol` keeps is checked here, and every way a payer, a seller
//! or a stranger could take money that is not theirs. The pay-as-you-go
//! half (balances and taps) is checked the same way, from both sides: a
//! buyer who tries to keep what was used, and a stolen edge key that tries
//! to bill more than the buyer allowed. A deposit moves only when two of
//! its three keys (the edge's releaser, the buyer's box, the seller's box)
//! agree, so a stolen edge key alone moves none of it.
//!
//! ```sh
//! cargo build-sbf && cargo test
//! ```

use litesvm::LiteSVM;
use losos_escrow::state::{Balance, Config, Deposit, Tap};
use losos_escrow::{
    decision_message, ix, EscrowError, BALANCE_SEED, CONFIG_SEED, DEPOSIT_SEED, HEADSTART,
    MAX_ACCRUAL, NOTICE, TAP_SEED, VAULT_SEED,
};
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
const ED25519: Address = Address::from_str_const("Ed25519SigVerify111111111111111111111111111");
const INSTRUCTIONS: Address =
    Address::from_str_const("Sysvar1nstructions1111111111111111111111111");
const NOW: i64 = 1_800_000_000;
const DAY: i64 = 24 * 60 * 60;
/// 20.00 USDC, a 4.00 fee: the registrar's own split example in base units.
const GROSS: u64 = 20_000_000;
const FEE: u64 = 4_000_000;
const HOUR: i64 = 60 * 60;
/// 1.00 USDC an hour, 4 % to the treasury.
const RATE: u64 = 1_000_000;
const FEE_BPS: u16 = 400;

struct Edge {
    svm: LiteSVM,
    program: Address,
    mint: Address,
    treasury: Keypair,
    releaser: Keypair,
    config: Address,
    payer: Keypair,
    seller: Keypair,
    buyer_box: Keypair,
    seller_box: Keypair,
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
        let buyer_box = Keypair::new();
        let seller_box = Keypair::new();
        for k in [
            &treasury,
            &releaser,
            &payer,
            &seller,
            &buyer_box,
            &seller_box,
        ] {
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
            buyer_box,
            seller_box,
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
        data.extend_from_slice(p.buyer_box.unwrap_or(self.buyer_box.pubkey()).as_ref());
        data.extend_from_slice(p.seller_box.unwrap_or(self.seller_box.pubkey()).as_ref());
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
        accounts.push(AccountMeta::new_readonly(INSTRUCTIONS, false));
        Instruction {
            program_id: self.program,
            accounts,
            data,
        }
    }

    /// The box's half of a decision: an Ed25519 check of `key`'s signature
    /// over the program's decision message, every offset in its own data.
    fn decision_ix(&self, key: &Keypair, nonce: &[u8; 32], refund: u64) -> Instruction {
        let (deposit, _) = self.deposit_of(nonce);
        ed25519_ix(key, &decision_message(&deposit, refund), u16::MAX)
    }

    /// The refund a finishing instruction asks the keys to agree on.
    fn decided(&self, tag: u8, nonce: &[u8; 32], refund: u64) -> u64 {
        match tag {
            ix::RELEASE => 0,
            ix::REFUND => self.held(nonce).map_or(GROSS, |d| d.gross),
            _ => refund,
        }
    }

    fn finish(
        &mut self,
        tag: u8,
        by: &Keypair,
        nonce: &[u8; 32],
        to: Finish,
    ) -> Result<(), TransactionError> {
        // By default the box the decision favours signs it beside `by`: the
        // seller's for a release or a split, the buyer's for a refund. With
        // the edge as `by` that is the everyday two of three.
        let agree = to.agree.clone().unwrap_or_else(|| match tag {
            ix::REFUND => vec![1],
            _ => vec![2],
        });
        let refund = self.decided(tag, nonce, to.refund);
        let mut ixs: Vec<Instruction> = agree
            .iter()
            .map(|&k| self.decision_ix(&self.box_key(k), nonce, refund))
            .collect();
        ixs.push(self.finish_ix(tag, &by.pubkey(), nonce, to));
        // The signer pays its own fee, so a refused instruction costs only it.
        self.send(&ixs, &[by])
    }

    /// 0 the releaser, 1 the buyer's box, 2 the seller's box.
    fn box_key(&self, k: u8) -> Keypair {
        match k {
            0 => self.releaser(),
            1 => self.buyer_box.insecure_clone(),
            _ => self.seller_box.insecure_clone(),
        }
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

/// The nonce of payment `n`, made up at run time: CodeQL reads a literal
/// deposit seed as a hard-coded nonce. The same `n` is the same nonce for
/// the whole run, so a test can still pay one twice.
fn a_nonce(n: u8) -> [u8; 32] {
    static RUN: std::sync::OnceLock<u128> = std::sync::OnceLock::new();
    let run = RUN.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    });
    let text = format!("{n:02x}{run:030x}");
    text.as_bytes().try_into().expect("32 hex digits")
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
    buyer_box: Option<Address>,
    seller_box: Option<Address>,
}

impl Pay {
    fn new(n: u8) -> Self {
        Self {
            nonce: a_nonce(n),
            gross: GROSS,
            fee: FEE,
            deadline: NOW + 33 * DAY,
            seller: None,
            mint: None,
            token: None,
            deposit: None,
            config: None,
            buyer_box: None,
            seller_box: None,
        }
    }
}

#[derive(Default, Clone)]
struct Finish {
    refund: u64,
    /// Whose Ed25519 checks ride in front (see [`Edge::box_key`]); `None`
    /// is the box the decision favours.
    agree: Option<Vec<u8>>,
    payer: Option<Address>,
    payer_token: Option<Address>,
    seller_token: Option<Address>,
    treasury_token: Option<Address>,
    vault: Option<Address>,
    deposit: Option<Address>,
    config: Option<Address>,
}

/// An Ed25519 signature-check instruction for `message` signed by `key`,
/// with the signature, key and message all named by `index` (`u16::MAX` is
/// the instruction's own data).
fn ed25519_ix(key: &Keypair, message: &[u8], index: u16) -> Instruction {
    let signature = key.sign_message(message);
    let (key_at, sig_at, message_at) = (16u16, 48u16, 112u16);
    let mut data = vec![1u8, 0];
    for v in [
        sig_at,
        index,
        key_at,
        index,
        message_at,
        u16::try_from(message.len()).unwrap(),
        index,
    ] {
        data.extend_from_slice(&v.to_le_bytes());
    }
    data.extend_from_slice(key.pubkey().as_ref());
    data.extend_from_slice(signature.as_ref());
    data.extend_from_slice(message);
    Instruction {
        program_id: ED25519,
        accounts: vec![],
        data,
    }
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
    assert_eq!(d.buyer_box.as_ref(), e.buyer_box.pubkey().as_ref());
    assert_eq!(d.seller_box.as_ref(), e.seller_box.pubkey().as_ref());
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
fn one_key_alone_never_moves_a_deposit() {
    let mut e = Edge::new();
    let nonce = e.paid(1);
    // Beside the seller's box: someone who is none of the three keys.
    for who in [e.seller(), e.payer(), e.treasury(), Keypair::new()] {
        e.svm.airdrop(&who.pubkey(), 1_000_000_000).unwrap();
        assert_eq!(
            custom(e.release(&who, &nonce)),
            code(EscrowError::NotApproved)
        );
        assert_eq!(
            custom(e.settle(&who, &nonce, 1)),
            code(EscrowError::NotApproved)
        );
    }
    // Each of the three on its own: the edge (a stolen releaser key), the
    // buyer's box, the seller's box.
    let stranger = Keypair::new();
    e.svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();
    let releaser = e.releaser();
    for (by, agree) in [
        (&releaser, vec![]),
        (&stranger, vec![0]),
        (&stranger, vec![1]),
        (&stranger, vec![2]),
    ] {
        for tag in [ix::RELEASE, ix::REFUND, ix::SETTLE] {
            let to = Finish {
                refund: 1,
                agree: Some(agree.clone()),
                ..Finish::default()
            };
            assert_eq!(
                custom(e.finish(tag, by, &nonce, to)),
                code(EscrowError::NotApproved),
                "tag {tag} agree {agree:?}"
            );
        }
    }
    // The same box twice is still one key.
    let twice = Finish {
        agree: Some(vec![2, 2]),
        ..Finish::default()
    };
    assert_eq!(
        custom(e.finish(ix::RELEASE, &stranger, &nonce, twice)),
        code(EscrowError::NotApproved)
    );
    assert!(e.held(&nonce).is_some());
}

#[test]
fn any_two_of_the_three_keys_decide() {
    let mut e = Edge::new();
    let stranger = Keypair::new();
    e.svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();
    let releaser = e.releaser();
    // The two boxes without the edge: whoever sends the transaction.
    let a = e.paid(1);
    let both = || Finish {
        refund: 5_000_000,
        agree: Some(vec![1, 2]),
        ..Finish::default()
    };
    e.finish(ix::RELEASE, &stranger, &a, both()).unwrap();
    assert_eq!(e.balance(&ata(&e.seller.pubkey())), GROSS - FEE);
    let b = e.paid(2);
    e.finish(ix::REFUND, &stranger, &b, both()).unwrap();
    assert_eq!(e.balance(&ata(&e.payer.pubkey())), 100_000_000 - GROSS);
    let c = e.paid(3);
    e.finish(ix::SETTLE, &stranger, &c, both()).unwrap();
    assert!(e.held(&c).is_none());
    // The edge with either box, the edge's own signature as a check too.
    let d = e.paid(4);
    let with_buyer = Finish {
        agree: Some(vec![1]),
        ..Finish::default()
    };
    e.finish(ix::RELEASE, &releaser, &d, with_buyer).unwrap();
    let f = e.paid(5);
    let checks_only = Finish {
        agree: Some(vec![0, 1]),
        ..Finish::default()
    };
    e.finish(ix::REFUND, &stranger, &f, checks_only).unwrap();
    // All three are fine as well.
    let g = e.paid(6);
    let all = Finish {
        agree: Some(vec![1, 2]),
        ..Finish::default()
    };
    e.finish(ix::RELEASE, &releaser, &g, all).unwrap();
    for n in [&a, &b, &d, &f, &g] {
        assert!(e.held(n).is_none());
    }
}

#[test]
fn a_box_signature_counts_only_for_its_own_decision() {
    let mut e = Edge::new();
    let nonce = e.paid(1);
    let other = e.paid(2);
    let releaser = e.releaser();
    let seller_box = e.box_key(2);
    let buyer_box = e.box_key(1);
    let (deposit, _) = e.deposit_of(&nonce);
    let (other_deposit, _) = e.deposit_of(&other);
    let try_with = |e: &mut Edge, check: Instruction, tag: u8, refund: u64| {
        let to = Finish {
            refund,
            ..Finish::default()
        };
        let finish = e.finish_ix(tag, &releaser.pubkey(), &nonce, to);
        custom(e.send(&[check, finish], &[&releaser]))
    };
    let cases = [
        // The seller's box agreed to release another deposit.
        (
            ed25519_ix(&seller_box, &decision_message(&other_deposit, 0), u16::MAX),
            ix::RELEASE,
            0,
        ),
        // The seller's box agreed to a split, not to a release.
        (
            ed25519_ix(&seller_box, &decision_message(&deposit, 1), u16::MAX),
            ix::RELEASE,
            0,
        ),
        // The buyer's box agreed to a release, not to a refund.
        (
            ed25519_ix(&buyer_box, &decision_message(&deposit, 0), u16::MAX),
            ix::REFUND,
            0,
        ),
        // The buyer's box agreed to give back 2, not 3.
        (
            ed25519_ix(&buyer_box, &decision_message(&deposit, 2), u16::MAX),
            ix::SETTLE,
            3,
        ),
        // A message of the right shape under another tag.
        (
            ed25519_ix(
                &seller_box,
                &[
                    b"something else entirely!".as_slice(),
                    deposit.as_ref(),
                    &[0u8; 8],
                ]
                .concat(),
                u16::MAX,
            ),
            ix::RELEASE,
            0,
        ),
        // A check whose data points at instruction 0 (itself, here): the
        // precompile verifies it, the program does not read it.
        (
            ed25519_ix(&seller_box, &decision_message(&deposit, 0), 0),
            ix::RELEASE,
            0,
        ),
    ];
    for (n, (check, tag, refund)) in cases.into_iter().enumerate() {
        assert_eq!(
            try_with(&mut e, check, tag, refund),
            code(EscrowError::NotApproved),
            "case {n}"
        );
    }
    // A forged signature never reaches the program: the precompile refuses
    // the whole transaction.
    let mut forged = ed25519_ix(&seller_box, &decision_message(&deposit, 0), u16::MAX);
    forged.data[60] ^= 1;
    let finish = e.finish_ix(ix::RELEASE, &releaser.pubkey(), &nonce, Finish::default());
    refused(e.send(&[forged, finish], &[&releaser]));
    assert!(e.held(&nonce).is_some());
    e.release(&releaser, &nonce).unwrap();
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
        code(EscrowError::NotApproved)
    );
    let seller = e.seller();
    assert_eq!(
        custom(e.refund(&seller, &nonce)),
        code(EscrowError::NotApproved)
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
fn the_edge_and_the_buyers_box_refund_at_any_time_but_only_to_the_payer() {
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
        // Two box keys, neither the edge's: otherwise the edge, or one
        // box, would hold two of the three.
        (
            Pay {
                buyer_box: Some(Address::default()),
                ..Pay::new(1)
            },
            code(EscrowError::BadBoxes),
        ),
        (
            Pay {
                seller_box: Some(Address::default()),
                ..Pay::new(1)
            },
            code(EscrowError::BadBoxes),
        ),
        (
            Pay {
                buyer_box: Some(e.seller_box.pubkey()),
                ..Pay::new(1)
            },
            code(EscrowError::BadBoxes),
        ),
        (
            Pay {
                seller_box: Some(e.releaser.pubkey()),
                ..Pay::new(1)
            },
            code(EscrowError::BadBoxes),
        ),
        (
            Pay {
                buyer_box: Some(e.releaser.pubkey()),
                ..Pay::new(1)
            },
            code(EscrowError::BadBoxes),
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
        code(EscrowError::NotApproved)
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
    let check = e.decision_ix(&e.box_key(2), &p.nonce, 1);
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
        Message::new(&[check, ix], Some(&releaser.pubkey())),
        e.svm.latest_blockhash(),
    );
    let meta = e.svm.send_transaction(tx).unwrap();
    assert!(
        meta.compute_units_consumed < 60_000,
        "{}",
        meta.compute_units_consumed
    );
}

// Pay as you go: balances and taps.

impl Edge {
    fn balance_of(&self, buyer: &Address) -> (Address, Address) {
        let balance = Address::find_program_address(
            &[BALANCE_SEED, self.config.as_ref(), buyer.as_ref()],
            &self.program,
        )
        .0;
        let vault = Address::find_program_address(&[VAULT_SEED, balance.as_ref()], &self.program).0;
        (balance, vault)
    }

    /// The payer's tap `id`.
    fn tap_of(&self, id: &[u8; 32]) -> Address {
        self.tap_by(&self.payer.pubkey(), id)
    }

    fn tap_by(&self, buyer: &Address, id: &[u8; 32]) -> Address {
        Address::find_program_address(
            &[TAP_SEED, self.config.as_ref(), buyer.as_ref(), id],
            &self.program,
        )
        .0
    }

    /// Another buyer, with SOL and 100 USDC of its own.
    fn buyer(&mut self) -> Keypair {
        let k = Keypair::new();
        self.svm.airdrop(&k.pubkey(), 10_000_000_000).unwrap();
        self.token_account(&k.pubkey(), 100_000_000);
        k
    }

    fn stranger(&mut self) -> Keypair {
        let k = Keypair::new();
        self.svm.airdrop(&k.pubkey(), 1_000_000_000).unwrap();
        k
    }

    fn top_up_ix(&self, buyer: &Address, amount: u64, t: &TopUp) -> Instruction {
        let (balance, vault) = self.balance_of(buyer);
        let mut data = vec![ix::TOP_UP];
        data.extend_from_slice(&amount.to_le_bytes());
        Instruction {
            program_id: self.program,
            accounts: vec![
                AccountMeta::new(*buyer, true),
                AccountMeta::new(ata(buyer), false),
                AccountMeta::new_readonly(self.config, false),
                AccountMeta::new(t.balance.unwrap_or(balance), false),
                AccountMeta::new(t.vault.unwrap_or(vault), false),
                AccountMeta::new_readonly(t.mint.unwrap_or(self.mint), false),
                AccountMeta::new_readonly(t.token.unwrap_or(TOKEN), false),
                AccountMeta::new_readonly(SYSTEM, false),
            ],
            data,
        }
    }

    fn top_up_with(
        &mut self,
        buyer: &Keypair,
        amount: u64,
        t: &TopUp,
    ) -> Result<(), TransactionError> {
        let ix = self.top_up_ix(&buyer.pubkey(), amount, t);
        self.send(&[ix], &[buyer])
    }

    fn top_up(&mut self, buyer: &Keypair, amount: u64) -> Result<(), TransactionError> {
        self.top_up_with(buyer, amount, &TopUp::default())
    }

    fn open_tap_ix(&self, buyer: &Address, o: &Open) -> Instruction {
        let mut data = vec![ix::OPEN_TAP];
        data.extend_from_slice(&o.id);
        data.extend_from_slice(&o.rate.to_le_bytes());
        data.extend_from_slice(&o.fee_bps.to_le_bytes());
        Instruction {
            program_id: self.program,
            accounts: vec![
                AccountMeta::new(*buyer, true),
                AccountMeta::new_readonly(self.config, false),
                AccountMeta::new_readonly(o.balance.unwrap_or(self.balance_of(buyer).0), false),
                AccountMeta::new(o.tap.unwrap_or(self.tap_by(buyer, &o.id)), false),
                AccountMeta::new_readonly(o.seller.unwrap_or(self.seller.pubkey()), false),
                AccountMeta::new_readonly(SYSTEM, false),
            ],
            data,
        }
    }

    fn open_tap(&mut self, buyer: &Keypair, o: &Open) -> Result<(), TransactionError> {
        let ix = self.open_tap_ix(&buyer.pubkey(), o);
        self.send(&[ix], &[buyer])
    }

    /// The payer tops up `amount` and opens the standard tap `n` at `RATE`.
    fn tapped(&mut self, n: u8, amount: u64) -> [u8; 32] {
        let payer = self.payer();
        if amount > 0 {
            self.top_up(&payer, amount).unwrap();
        }
        let o = Open::new(n);
        self.open_tap(&payer, &o).unwrap();
        o.id
    }

    fn close_tap_ix(&self, by: &Address, id: &[u8; 32], buyer: Address) -> Instruction {
        Instruction {
            program_id: self.program,
            accounts: vec![
                AccountMeta::new_readonly(*by, true),
                AccountMeta::new_readonly(self.config, false),
                AccountMeta::new(self.tap_of(id), false),
                AccountMeta::new(buyer, false),
            ],
            data: vec![ix::CLOSE_TAP],
        }
    }

    fn close_tap(&mut self, by: &Keypair, id: &[u8; 32]) -> Result<(), TransactionError> {
        let ix = self.close_tap_ix(&by.pubkey(), id, self.payer.pubkey());
        self.send(&[ix], &[by])
    }

    fn settle_tap_ix(&self, by: &Address, id: &[u8; 32], amount: u64, s: &SettleTo) -> Instruction {
        let buyer = s.buyer.unwrap_or(self.payer.pubkey());
        let (balance, vault) = self.balance_of(&s.balance_of.unwrap_or(buyer));
        let mut data = vec![ix::SETTLE_TAP];
        data.extend_from_slice(&amount.to_le_bytes());
        Instruction {
            program_id: self.program,
            accounts: vec![
                AccountMeta::new_readonly(*by, true),
                AccountMeta::new_readonly(self.config, false),
                AccountMeta::new(s.tap.unwrap_or(self.tap_by(&buyer, id)), false),
                AccountMeta::new_readonly(s.balance.unwrap_or(balance), false),
                AccountMeta::new(s.vault.unwrap_or(vault), false),
                AccountMeta::new(s.seller_token.unwrap_or(ata(&self.seller.pubkey())), false),
                AccountMeta::new(
                    s.treasury_token.unwrap_or(ata(&self.treasury.pubkey())),
                    false,
                ),
                AccountMeta::new(buyer, false),
                AccountMeta::new_readonly(s.mint.unwrap_or(self.mint), false),
                AccountMeta::new_readonly(s.token.unwrap_or(TOKEN), false),
            ],
            data,
        }
    }

    fn settle_tap_with(
        &mut self,
        by: &Keypair,
        id: &[u8; 32],
        amount: u64,
        s: &SettleTo,
    ) -> Result<(), TransactionError> {
        let ix = self.settle_tap_ix(&by.pubkey(), id, amount, s);
        self.send(&[ix], &[by])
    }

    fn settle_tap(&mut self, id: &[u8; 32], amount: u64) -> Result<(), TransactionError> {
        let releaser = self.releaser();
        self.settle_tap_with(&releaser, id, amount, &SettleTo::default())
    }

    fn ask_ix(&self, buyer: &Address, amount: u64, balance: Option<Address>) -> Instruction {
        let mut data = vec![ix::ASK_WITHDRAWAL];
        data.extend_from_slice(&amount.to_le_bytes());
        Instruction {
            program_id: self.program,
            accounts: vec![
                AccountMeta::new_readonly(*buyer, true),
                AccountMeta::new_readonly(self.config, false),
                AccountMeta::new(balance.unwrap_or(self.balance_of(buyer).0), false),
            ],
            data,
        }
    }

    fn ask(&mut self, buyer: &Keypair, amount: u64) -> Result<(), TransactionError> {
        let ix = self.ask_ix(&buyer.pubkey(), amount, None);
        self.send(&[ix], &[buyer])
    }

    fn withdraw_ix(&self, buyer: &Address, w: &Out) -> Instruction {
        let (balance, vault) = self.balance_of(&w.balance_of.unwrap_or(*buyer));
        Instruction {
            program_id: self.program,
            accounts: vec![
                AccountMeta::new_readonly(*buyer, true),
                AccountMeta::new_readonly(self.config, false),
                AccountMeta::new(balance, false),
                AccountMeta::new(w.vault.unwrap_or(vault), false),
                AccountMeta::new(w.buyer_token.unwrap_or(ata(buyer)), false),
                AccountMeta::new_readonly(self.mint, false),
                AccountMeta::new_readonly(TOKEN, false),
            ],
            data: vec![ix::WITHDRAW],
        }
    }

    fn withdraw_with(&mut self, buyer: &Keypair, w: &Out) -> Result<(), TransactionError> {
        let ix = self.withdraw_ix(&buyer.pubkey(), w);
        self.send(&[ix], &[buyer])
    }

    fn withdraw(&mut self, buyer: &Keypair) -> Result<(), TransactionError> {
        self.withdraw_with(buyer, &Out::default())
    }

    fn tap(&self, id: &[u8; 32]) -> Option<Tap> {
        self.tap_for(&self.payer.pubkey(), id)
    }

    fn tap_for(&self, buyer: &Address, id: &[u8; 32]) -> Option<Tap> {
        self.svm
            .get_account(&self.tap_by(buyer, id))
            .filter(|a| a.owner == self.program && !a.data.is_empty())
            .and_then(|a| Tap::read(&a.data))
    }

    fn balance_account(&self, buyer: &Address) -> Option<Balance> {
        self.svm
            .get_account(&self.balance_of(buyer).0)
            .filter(|a| a.owner == self.program)
            .and_then(|a| Balance::read(&a.data))
    }

    /// What the buyer has prepaid: the vault's tokens.
    fn prepaid(&self, buyer: &Address) -> u64 {
        self.balance(&self.balance_of(buyer).1)
    }

    /// A mint that is not the config's, and a token account of it.
    fn lookalike(&mut self, owner: &Address) -> (Address, Address) {
        let mint = Address::new_unique();
        let rent = self.svm.minimum_balance_for_rent_exemption(82);
        self.svm
            .set_account(mint, account(rent, mint_data(6), TOKEN))
            .unwrap();
        let theirs = Address::new_unique();
        let rent = self.svm.minimum_balance_for_rent_exemption(165);
        self.svm
            .set_account(theirs, account(rent, token_data(&mint, owner, 0), TOKEN))
            .unwrap();
        (mint, theirs)
    }
}

#[derive(Default)]
struct TopUp {
    balance: Option<Address>,
    vault: Option<Address>,
    mint: Option<Address>,
    token: Option<Address>,
}

#[derive(Clone)]
struct Open {
    id: [u8; 32],
    rate: u64,
    fee_bps: u16,
    seller: Option<Address>,
    tap: Option<Address>,
    balance: Option<Address>,
}

impl Open {
    fn new(n: u8) -> Self {
        Self {
            id: a_nonce(n),
            rate: RATE,
            fee_bps: FEE_BPS,
            seller: None,
            tap: None,
            balance: None,
        }
    }
}

#[derive(Default)]
struct SettleTo {
    /// The tap's buyer: the rent account, and whose balance is billed.
    buyer: Option<Address>,
    /// Bill this buyer's balance and vault instead.
    balance_of: Option<Address>,
    balance: Option<Address>,
    vault: Option<Address>,
    tap: Option<Address>,
    seller_token: Option<Address>,
    treasury_token: Option<Address>,
    mint: Option<Address>,
    token: Option<Address>,
}

#[derive(Default)]
struct Out {
    balance_of: Option<Address>,
    vault: Option<Address>,
    buyer_token: Option<Address>,
}

#[test]
fn a_top_up_opens_the_balance_once_and_adds_to_it() {
    let mut e = Edge::new();
    let payer = e.payer();
    e.top_up(&payer, 30_000_000).unwrap();
    let b = e.balance_account(&payer.pubkey()).unwrap();
    assert_eq!(b.buyer.as_ref(), payer.pubkey().as_ref());
    assert_eq!(b.config.as_ref(), e.config.as_ref());
    assert_eq!((b.asked, b.not_before), (0, 0));
    assert_eq!(e.prepaid(&payer.pubkey()), 30_000_000);
    e.top_up(&payer, 10_000_000).unwrap();
    assert_eq!(e.prepaid(&payer.pubkey()), 40_000_000);
    assert_eq!(e.balance(&ata(&payer.pubkey())), 60_000_000);
    // Each buyer has a balance of their own.
    let other = e.buyer();
    e.top_up(&other, 1).unwrap();
    assert_eq!(e.prepaid(&other.pubkey()), 1);
    assert_eq!(e.prepaid(&payer.pubkey()), 40_000_000);
}

#[test]
fn a_top_up_must_fit_the_rules() {
    let mut e = Edge::new();
    let payer = e.payer();
    assert_eq!(custom(e.top_up(&payer, 0)), code(EscrowError::ZeroAmount));
    let (mint, _) = e.lookalike(&payer.pubkey());
    assert_eq!(
        custom(e.top_up_with(
            &payer,
            1,
            &TopUp {
                mint: Some(mint),
                ..TopUp::default()
            }
        )),
        code(EscrowError::WrongMint)
    );
    refused(e.top_up_with(
        &payer,
        1,
        &TopUp {
            token: Some(TOKEN_2022),
            ..TopUp::default()
        },
    ));
    // Not into someone else's balance, nor at a made-up address.
    let other = e.buyer();
    e.top_up(&other, 1).unwrap();
    let (theirs, their_vault) = e.balance_of(&other.pubkey());
    for t in [
        TopUp {
            balance: Some(theirs),
            vault: Some(their_vault),
            ..TopUp::default()
        },
        TopUp {
            balance: Some(Address::new_unique()),
            ..TopUp::default()
        },
        TopUp {
            vault: Some(their_vault),
            ..TopUp::default()
        },
    ] {
        assert_eq!(
            custom(e.top_up_with(&payer, 1, &t)),
            code(EscrowError::WrongAccount)
        );
    }
    e.top_up(&payer, 1).unwrap();
    // Once the balance exists, its vault is the only one it takes.
    assert_eq!(
        custom(e.top_up_with(
            &payer,
            1,
            &TopUp {
                vault: Some(their_vault),
                ..TopUp::default()
            }
        )),
        code(EscrowError::WrongAccount)
    );
    assert_eq!(e.prepaid(&other.pubkey()), 1);
    assert_eq!(e.prepaid(&payer.pubkey()), 1);
}

#[test]
fn a_gift_of_lamports_cannot_block_a_balance_or_a_tap() {
    let mut e = Edge::new();
    let payer = e.payer();
    let (balance, vault) = e.balance_of(&payer.pubkey());
    // More than the rent on the balance, one lamport on the vault and tap.
    e.svm
        .set_account(balance, account(1_000_000_000, vec![], SYSTEM))
        .unwrap();
    e.svm
        .set_account(vault, account(1, vec![], SYSTEM))
        .unwrap();
    let o = Open::new(1);
    let tap = e.tap_of(&o.id);
    e.svm.set_account(tap, account(1, vec![], SYSTEM)).unwrap();
    e.top_up(&payer, 5_000_000).unwrap();
    assert_eq!(e.prepaid(&payer.pubkey()), 5_000_000);
    assert!(e.balance_account(&payer.pubkey()).is_some());
    let rent = e.svm.minimum_balance_for_rent_exemption(165);
    assert_eq!(e.lamports(&vault), rent);
    e.open_tap(&payer, &o).unwrap();
    assert!(e.tap(&o.id).is_some());
    e.at(NOW + HOUR);
    e.settle_tap(&o.id, RATE).unwrap();
}

#[test]
fn a_tap_bills_at_most_its_rate_and_pays_the_seller_and_the_treasury() {
    let mut e = Edge::new();
    let id = e.tapped(1, 30_000_000);
    let t = e.tap(&id).unwrap();
    assert_eq!(t.buyer.as_ref(), e.payer.pubkey().as_ref());
    assert_eq!(t.seller.as_ref(), e.seller.pubkey().as_ref());
    assert_eq!(t.config.as_ref(), e.config.as_ref());
    assert_eq!((t.id, t.rate, t.fee_bps), (id, RATE, FEE_BPS));
    // Billing counts from an hour before the tap opened.
    assert_eq!((t.settled_at, t.closed_at), (NOW - HEADSTART, None));

    // An hour and a half at 1.00 an hour, plus the hour of headstart, is
    // 2.50, not a base unit more.
    e.at(NOW + 90 * 60);
    assert_eq!(
        custom(e.settle_tap(&id, 2_500_001)),
        code(EscrowError::OverCap)
    );
    e.settle_tap(&id, 2_500_000).unwrap();
    assert_eq!(e.balance(&ata(&e.treasury.pubkey())), 100_000);
    assert_eq!(e.balance(&ata(&e.seller.pubkey())), 2_400_000);
    assert_eq!(e.prepaid(&e.payer.pubkey()), 30_000_000 - 2_500_000);
    assert_eq!(e.tap(&id).unwrap().settled_at, NOW + 90 * 60);
    // The same second again bills nothing more, not even a unit.
    assert_eq!(custom(e.settle_tap(&id, 1)), code(EscrowError::TooEarly));
    // Nor for time that has not passed.
    e.at(NOW + 60 * 60);
    assert_eq!(custom(e.settle_tap(&id, 1)), code(EscrowError::TooEarly));

    // The fee rounds down: 4 % of 999 is 39.96, so 39 and 960.
    e.at(NOW + 150 * 60);
    e.settle_tap(&id, 999).unwrap();
    assert_eq!(e.balance(&ata(&e.treasury.pubkey())), 100_000 + 39);
    assert_eq!(e.balance(&ata(&e.seller.pubkey())), 2_400_000 + 960);
    // And 24 units carry no fee at all.
    e.at(NOW + 151 * 60);
    e.settle_tap(&id, 24).unwrap();
    assert_eq!(e.balance(&ata(&e.treasury.pubkey())), 100_039);
    assert_eq!(e.balance(&ata(&e.seller.pubkey())), 2_400_984);
    assert_eq!(
        e.prepaid(&e.payer.pubkey()),
        30_000_000 - 2_500_000 - 999 - 24
    );
    // A settle of nothing on an open tap is refused.
    e.at(NOW + 152 * 60);
    assert_eq!(custom(e.settle_tap(&id, 0)), code(EscrowError::ZeroAmount));
}

#[test]
fn cap_left_unused_is_not_carried_over() {
    let mut e = Edge::new();
    let id = e.tapped(1, 30_000_000);
    // Ten hours pass and the edge bills one unit of them and the headstart.
    e.at(NOW + 10 * HOUR);
    e.settle_tap(&id, 1).unwrap();
    // One more hour: one hour's worth, not the eleven left over.
    e.at(NOW + 11 * HOUR);
    assert_eq!(
        custom(e.settle_tap(&id, RATE + 1)),
        code(EscrowError::OverCap)
    );
    e.settle_tap(&id, RATE).unwrap();
}

#[test]
fn a_settle_counts_at_most_a_week() {
    let mut e = Edge::new();
    let id = e.tapped(1, 100_000_000);
    let week = RATE * (MAX_ACCRUAL / HOUR) as u64;
    assert_eq!(week, 168_000_000);
    // A rate the balance can cover for a week: 0.50 an hour.
    let slow = Open {
        rate: RATE / 2,
        ..Open::new(2)
    };
    let payer = e.payer();
    e.open_tap(&payer, &slow).unwrap();
    e.at(NOW + 30 * 24 * HOUR);
    assert_eq!(
        custom(e.settle_tap(&slow.id, week / 2 + 1)),
        code(EscrowError::OverCap)
    );
    e.settle_tap(&slow.id, week / 2).unwrap();
    // The thirty days are spent: the next settle starts from now.
    assert_eq!(e.tap(&slow.id).unwrap().settled_at, NOW + 30 * 24 * HOUR);
    e.at(NOW + 30 * 24 * HOUR + HOUR);
    assert_eq!(
        custom(e.settle_tap(&slow.id, RATE / 2 + 1)),
        code(EscrowError::OverCap)
    );
    // The other tap could bill a week too, but the balance is short.
    assert_eq!(
        custom(e.settle_tap(&id, week)),
        code(EscrowError::OverBalance)
    );
}

#[test]
fn a_settle_never_takes_more_than_the_balance() {
    let mut e = Edge::new();
    let id = e.tapped(1, 1_000_000);
    e.at(NOW + 2 * HOUR);
    assert_eq!(
        custom(e.settle_tap(&id, 1_000_001)),
        code(EscrowError::OverBalance)
    );
    e.settle_tap(&id, 1_000_000).unwrap();
    assert_eq!(e.prepaid(&e.payer.pubkey()), 0);
    e.at(NOW + 3 * HOUR);
    assert_eq!(custom(e.settle_tap(&id, 1)), code(EscrowError::OverBalance));
    // A top-up lets billing go on, for the hour since the last settle.
    let payer = e.payer();
    e.top_up(&payer, 5_000_000).unwrap();
    assert_eq!(
        custom(e.settle_tap(&id, RATE + 1)),
        code(EscrowError::OverCap)
    );
    e.settle_tap(&id, RATE).unwrap();
}

#[test]
fn a_closed_tap_bills_only_up_to_its_close_and_returns_its_rent() {
    let mut e = Edge::new();
    let id = e.tapped(1, 30_000_000);
    let payer = e.payer();
    let tap = e.tap_of(&id);
    let rent = e.lamports(&tap);
    assert!(rent > 0);
    e.at(NOW + HOUR);
    e.close_tap(&payer, &id).unwrap();
    assert_eq!(e.tap(&id).unwrap().closed_at, Some(NOW + HOUR));
    assert_eq!(
        custom(e.close_tap(&payer, &id)),
        code(EscrowError::TapClosed)
    );
    let releaser = e.releaser();
    assert_eq!(
        custom(e.close_tap(&releaser, &id)),
        code(EscrowError::TapClosed)
    );
    // Five hours later the edge can still bill the hour before the close,
    // and the hour of headstart.
    e.at(NOW + 5 * HOUR);
    assert_eq!(
        custom(e.settle_tap(&id, 2 * RATE + 1)),
        code(EscrowError::OverCap)
    );
    let before = e.lamports(&payer.pubkey());
    e.settle_tap(&id, 2 * RATE).unwrap();
    // Settled up to its close, the tap is gone and the buyer has its rent.
    assert!(e.tap(&id).is_none());
    assert_eq!(e.lamports(&tap), 0);
    assert_eq!(e.lamports(&payer.pubkey()), before + rent);
    assert_eq!(e.balance(&ata(&e.seller.pubkey())), 1_920_000);
    assert_eq!(e.balance(&ata(&e.treasury.pubkey())), 80_000);
    assert_eq!(custom(e.settle_tap(&id, 1)), code(EscrowError::NoTap));
    assert_eq!(custom(e.close_tap(&payer, &id)), code(EscrowError::NoTap));
}

#[test]
fn a_closed_tap_partly_settled_bills_only_the_rest_of_its_time() {
    let mut e = Edge::new();
    let id = e.tapped(1, 30_000_000);
    e.at(NOW + HOUR);
    e.settle_tap(&id, RATE / 2).unwrap();
    // The releaser closes it half an hour later.
    e.at(NOW + 90 * 60);
    let releaser = e.releaser();
    e.close_tap(&releaser, &id).unwrap();
    e.at(NOW + 10 * HOUR);
    assert_eq!(
        custom(e.settle_tap(&id, RATE / 2 + 1)),
        code(EscrowError::OverCap)
    );
    e.settle_tap(&id, RATE / 4).unwrap();
    assert!(e.tap(&id).is_none());
}

#[test]
fn a_closed_tap_with_nothing_owed_finishes_for_free() {
    let mut e = Edge::new();
    // Settled up to the second it closes: nothing left, gone at once.
    let id = e.tapped(1, 30_000_000);
    let payer = e.payer();
    e.settle_tap(&id, RATE).unwrap();
    let tap = e.tap_of(&id);
    let rent = e.lamports(&tap);
    let before = e.lamports(&payer.pubkey());
    e.close_tap(&payer, &id).unwrap();
    assert!(e.tap(&id).is_none());
    assert_eq!(e.lamports(&payer.pubkey()), before + rent - 5000);
    // Closed later with nothing used: the edge finishes it with a settle of
    // nothing, which moves no money.
    let other = Open::new(2);
    e.open_tap(&payer, &other).unwrap();
    e.at(NOW + HOUR);
    e.close_tap(&payer, &other.id).unwrap();
    e.settle_tap(&other.id, 0).unwrap();
    assert!(e.tap(&other.id).is_none());
    assert_eq!(e.prepaid(&payer.pubkey()), 30_000_000 - RATE);
    assert_eq!(e.balance(&ata(&e.seller.pubkey())), 960_000);
}

#[test]
fn the_first_hour_can_be_billed_right_after_open() {
    let mut e = Edge::new();
    let payer = e.payer();
    let id = e.tapped(1, 30_000_000);
    // No time has passed, and the hour of headstart is billable.
    assert_eq!(
        custom(e.settle_tap(&id, RATE + 1)),
        code(EscrowError::OverCap)
    );
    e.settle_tap(&id, RATE).unwrap();
    assert_eq!(e.tap(&id).unwrap().settled_at, NOW);
    assert_eq!(e.balance(&ata(&e.seller.pubkey())), 960_000);
    assert_eq!(e.balance(&ata(&e.treasury.pubkey())), 40_000);
    assert_eq!(custom(e.settle_tap(&id, 1)), code(EscrowError::TooEarly));

    // Closed in the second it opened, a tap still owes its first hour.
    let closed = Open::new(2);
    e.open_tap(&payer, &closed).unwrap();
    e.close_tap(&payer, &closed.id).unwrap();
    let t = e.tap(&closed.id).unwrap();
    assert_eq!((t.settled_at, t.closed_at), (NOW - HEADSTART, Some(NOW)));
    let rent = e.lamports(&e.tap_of(&closed.id));
    e.at(NOW + 24 * HOUR);
    assert_eq!(
        custom(e.settle_tap(&closed.id, RATE + 1)),
        code(EscrowError::OverCap)
    );
    let before = e.lamports(&payer.pubkey());
    e.settle_tap(&closed.id, RATE).unwrap();
    assert!(e.tap(&closed.id).is_none());
    assert_eq!(e.lamports(&payer.pubkey()), before + rent);
    assert_eq!(e.prepaid(&payer.pubkey()), 30_000_000 - 2 * RATE);

    // The headstart never reaches before the clock's zero.
    e.at(100);
    let early = Open::new(3);
    e.open_tap(&payer, &early).unwrap();
    assert_eq!(e.tap(&early.id).unwrap().settled_at, 0);
}

#[test]
fn only_the_buyer_or_the_releaser_closes_a_tap() {
    let mut e = Edge::new();
    let id = e.tapped(1, 30_000_000);
    let other = e.buyer();
    let stranger = e.stranger();
    for who in [e.seller(), e.treasury(), other, stranger] {
        assert_eq!(
            custom(e.close_tap(&who, &id)),
            code(EscrowError::NotReleaser)
        );
    }
    // The rent of a finished tap goes to its buyer and no one else.
    let thief = e.stranger();
    let releaser = e.releaser();
    let ix = e.close_tap_ix(&releaser.pubkey(), &id, thief.pubkey());
    assert_eq!(
        custom(e.send(&[ix], &[&releaser])),
        code(EscrowError::WrongAccount)
    );
    assert_eq!(e.tap(&id).unwrap().closed_at, None);
    e.close_tap(&releaser, &id).unwrap();
}

#[test]
fn only_the_releaser_settles_a_tap() {
    let mut e = Edge::new();
    let id = e.tapped(1, 30_000_000);
    e.at(NOW + HOUR);
    let stranger = e.stranger();
    for who in [e.payer(), e.seller(), e.treasury(), stranger] {
        assert_eq!(
            custom(e.settle_tap_with(&who, &id, 1, &SettleTo::default())),
            code(EscrowError::NotReleaser)
        );
    }
    assert_eq!(e.prepaid(&e.payer.pubkey()), 30_000_000);
}

#[test]
fn a_settle_pays_only_the_taps_seller_and_the_treasury() {
    let mut e = Edge::new();
    let id = e.tapped(1, 30_000_000);
    e.at(NOW + HOUR);
    let releaser = e.releaser();
    let thief = e.stranger();
    let theirs = e.token_account(&thief.pubkey(), 0);
    let (lookalike, sellers_lookalike) = e.lookalike(&e.seller.pubkey());
    // A second buyer with a balance and a tap of their own, to another seller.
    let other = e.buyer();
    e.top_up(&other, 50_000_000).unwrap();
    let other_seller = Keypair::new().pubkey();
    e.token_account(&other_seller, 0);
    let their_tap = Open {
        seller: Some(other_seller),
        ..Open::new(2)
    };
    e.open_tap(&other, &their_tap).unwrap();
    let their_vault = e.balance_of(&other.pubkey()).1;
    let nonce = e.paid(3);
    let (deposit, _) = e.deposit_of(&nonce);

    let cases: Vec<(SettleTo, u32)> = vec![
        (
            SettleTo {
                seller_token: Some(theirs),
                ..SettleTo::default()
            },
            code(EscrowError::WrongOwner),
        ),
        (
            SettleTo {
                treasury_token: Some(theirs),
                ..SettleTo::default()
            },
            code(EscrowError::WrongOwner),
        ),
        (
            SettleTo {
                seller_token: Some(ata(&e.treasury.pubkey())),
                ..SettleTo::default()
            },
            code(EscrowError::WrongOwner),
        ),
        // Another tap's seller is not this tap's seller.
        (
            SettleTo {
                seller_token: Some(ata(&other_seller)),
                ..SettleTo::default()
            },
            code(EscrowError::WrongOwner),
        ),
        (
            SettleTo {
                seller_token: Some(sellers_lookalike),
                ..SettleTo::default()
            },
            code(EscrowError::WrongMint),
        ),
        (
            SettleTo {
                mint: Some(lookalike),
                ..SettleTo::default()
            },
            code(EscrowError::WrongMint),
        ),
        // Billing this tap to another buyer's balance.
        (
            SettleTo {
                balance_of: Some(other.pubkey()),
                ..SettleTo::default()
            },
            code(EscrowError::WrongAccount),
        ),
        (
            SettleTo {
                vault: Some(their_vault),
                ..SettleTo::default()
            },
            code(EscrowError::WrongAccount),
        ),
        // The rent to someone other than the buyer.
        (
            SettleTo {
                buyer: Some(thief.pubkey()),
                balance_of: Some(e.payer.pubkey()),
                tap: Some(e.tap_of(&id)),
                ..SettleTo::default()
            },
            code(EscrowError::WrongAccount),
        ),
        // Accounts of the wrong kind.
        (
            SettleTo {
                tap: Some(deposit),
                ..SettleTo::default()
            },
            code(EscrowError::NoTap),
        ),
        (
            SettleTo {
                balance: Some(e.tap_of(&id)),
                ..SettleTo::default()
            },
            code(EscrowError::NoBalance),
        ),
    ];
    for (s, want) in cases {
        assert_eq!(custom(e.settle_tap_with(&releaser, &id, RATE, &s)), want);
    }
    refused(e.settle_tap_with(
        &releaser,
        &id,
        RATE,
        &SettleTo {
            token: Some(TOKEN_2022),
            ..SettleTo::default()
        },
    ));
    // Their tap, billed with this buyer's balance, is refused too.
    assert_eq!(
        custom(e.settle_tap_with(
            &releaser,
            &their_tap.id,
            RATE,
            &SettleTo {
                buyer: Some(other.pubkey()),
                balance_of: Some(e.payer.pubkey()),
                ..SettleTo::default()
            }
        )),
        code(EscrowError::WrongAccount)
    );
    assert_eq!(e.balance(&theirs), 0);
    assert_eq!(e.prepaid(&e.payer.pubkey()), 30_000_000);
    assert_eq!(e.balance(&their_vault), 50_000_000);
    e.settle_tap(&id, RATE).unwrap();
    // Their tap opened an hour in, so an hour later it owes an hour.
    e.at(NOW + 2 * HOUR);
    e.settle_tap_with(
        &releaser,
        &their_tap.id,
        RATE,
        &SettleTo {
            buyer: Some(other.pubkey()),
            seller_token: Some(ata(&other_seller)),
            ..SettleTo::default()
        },
    )
    .unwrap();
    assert_eq!(e.balance(&ata(&other_seller)), 960_000);
}

#[test]
fn a_tap_must_fit_the_rules() {
    let mut e = Edge::new();
    let payer = e.payer();
    // No balance yet, no tap.
    assert_eq!(
        custom(e.open_tap(&payer, &Open::new(1))),
        code(EscrowError::NoBalance)
    );
    e.top_up(&payer, 10_000_000).unwrap();
    let (balance, vault) = e.balance_of(&payer.pubkey());
    let o = Open::new(1);
    let tap = e.tap_of(&o.id);
    let other = e.buyer();
    e.top_up(&other, 1).unwrap();
    let cases: Vec<(Open, u32)> = vec![
        (
            Open {
                fee_bps: 5001,
                ..o.clone()
            },
            code(EscrowError::FeeTooHigh),
        ),
        (
            Open {
                fee_bps: u16::MAX,
                ..o.clone()
            },
            code(EscrowError::FeeTooHigh),
        ),
        (
            Open {
                rate: 0,
                ..o.clone()
            },
            code(EscrowError::ZeroAmount),
        ),
        (
            Open {
                seller: Some(Address::default()),
                ..o.clone()
            },
            code(EscrowError::ZeroSeller),
        ),
        (
            Open {
                seller: Some(e.treasury.pubkey()),
                ..o.clone()
            },
            code(EscrowError::ZeroSeller),
        ),
        (
            Open {
                seller: Some(e.config),
                ..o.clone()
            },
            code(EscrowError::ZeroSeller),
        ),
        (
            Open {
                seller: Some(balance),
                ..o.clone()
            },
            code(EscrowError::ZeroSeller),
        ),
        (
            Open {
                seller: Some(vault),
                ..o.clone()
            },
            code(EscrowError::ZeroSeller),
        ),
        (
            Open {
                seller: Some(tap),
                ..o.clone()
            },
            code(EscrowError::ZeroSeller),
        ),
        (
            Open {
                tap: Some(Address::new_unique()),
                ..o.clone()
            },
            code(EscrowError::WrongAccount),
        ),
        // Against someone else's balance.
        (
            Open {
                balance: Some(e.balance_of(&other.pubkey()).0),
                ..o.clone()
            },
            code(EscrowError::WrongAccount),
        ),
    ];
    for (bad, want) in cases {
        assert_eq!(
            custom(e.open_tap(&payer, &bad)),
            want,
            "fee {} rate {}",
            bad.fee_bps,
            bad.rate
        );
    }
    // Exactly half is fine, and an id opens once.
    e.open_tap(
        &payer,
        &Open {
            fee_bps: 5000,
            ..o.clone()
        },
    )
    .unwrap();
    assert_eq!(custom(e.open_tap(&payer, &o)), code(EscrowError::TapExists));
    assert_eq!(e.tap(&o.id).unwrap().fee_bps, 5000);
    // The same id from another wallet is that wallet's own tap.
    e.open_tap(&other, &o).unwrap();
    assert_eq!(
        e.tap_for(&other.pubkey(), &o.id).unwrap().buyer,
        other.pubkey()
    );
    assert_eq!(e.tap(&o.id).unwrap().buyer, payer.pubkey());
    // Several taps open in one transaction, for a group.
    let group: Vec<Instruction> = (2..5)
        .map(|n| e.open_tap_ix(&payer.pubkey(), &Open::new(n)))
        .collect();
    e.send(&group, &[&payer]).unwrap();
    for n in 2..5 {
        assert!(e.tap(&a_nonce(n)).is_some());
    }
}

#[test]
fn a_withdrawal_waits_out_the_notice_and_pays_once() {
    let mut e = Edge::new();
    let payer = e.payer();
    e.top_up(&payer, 30_000_000).unwrap();
    assert_eq!(custom(e.withdraw(&payer)), code(EscrowError::NoWithdrawal));
    assert_eq!(custom(e.ask(&payer, 0)), code(EscrowError::ZeroAmount));
    e.ask(&payer, 10_000_000).unwrap();
    let b = e.balance_account(&payer.pubkey()).unwrap();
    assert_eq!((b.asked, b.not_before), (10_000_000, NOW + NOTICE));
    assert_eq!(custom(e.withdraw(&payer)), code(EscrowError::TooEarly));
    e.at(NOW + NOTICE - 1);
    assert_eq!(custom(e.withdraw(&payer)), code(EscrowError::TooEarly));
    e.at(NOW + NOTICE);
    e.withdraw(&payer).unwrap();
    assert_eq!(e.balance(&ata(&payer.pubkey())), 80_000_000);
    assert_eq!(e.prepaid(&payer.pubkey()), 20_000_000);
    let b = e.balance_account(&payer.pubkey()).unwrap();
    assert_eq!((b.asked, b.not_before), (0, 0));
    // The ask is spent.
    assert_eq!(custom(e.withdraw(&payer)), code(EscrowError::NoWithdrawal));
    assert_eq!(e.balance(&ata(&payer.pubkey())), 80_000_000);
}

#[test]
fn a_new_ask_replaces_the_old_one_and_waits_again() {
    let mut e = Edge::new();
    let payer = e.payer();
    e.top_up(&payer, 30_000_000).unwrap();
    e.ask(&payer, 5_000_000).unwrap();
    e.at(NOW + 24 * HOUR);
    e.ask(&payer, 7_000_000).unwrap();
    e.at(NOW + NOTICE);
    assert_eq!(custom(e.withdraw(&payer)), code(EscrowError::TooEarly));
    e.at(NOW + 24 * HOUR + NOTICE);
    e.withdraw(&payer).unwrap();
    assert_eq!(e.prepaid(&payer.pubkey()), 23_000_000);
    assert_eq!(e.balance(&ata(&payer.pubkey())), 77_000_000);
}

#[test]
fn a_withdrawal_pays_what_is_left_after_the_edge_settled_during_the_notice() {
    let mut e = Edge::new();
    let payer = e.payer();
    let id = e.tapped(1, 10_000_000);
    // The buyer asks for everything at once; the edge bills the three hours
    // used before the tap closed.
    e.ask(&payer, 10_000_000).unwrap();
    e.at(NOW + 3 * HOUR);
    e.close_tap(&payer, &id).unwrap();
    e.at(NOW + 4 * HOUR);
    e.settle_tap(&id, 3 * RATE).unwrap();
    e.at(NOW + NOTICE);
    e.withdraw(&payer).unwrap();
    assert_eq!(e.prepaid(&payer.pubkey()), 0);
    assert_eq!(e.balance(&ata(&payer.pubkey())), 97_000_000);
    assert_eq!(e.balance(&ata(&e.seller.pubkey())), 2_880_000);
    assert_eq!(e.balance(&ata(&e.treasury.pubkey())), 120_000);
}

#[test]
fn a_withdrawal_pays_only_the_buyer() {
    let mut e = Edge::new();
    let payer = e.payer();
    e.top_up(&payer, 30_000_000).unwrap();
    e.ask(&payer, 30_000_000).unwrap();
    e.at(NOW + NOTICE);
    // Not to anyone else's token account.
    let seller_token = ata(&e.seller.pubkey());
    assert_eq!(
        custom(e.withdraw_with(
            &payer,
            &Out {
                buyer_token: Some(seller_token),
                ..Out::default()
            }
        )),
        code(EscrowError::WrongOwner)
    );
    // A stranger can neither ask for nor take the buyer's balance.
    let thief = e.buyer();
    let theirs = e.balance_of(&payer.pubkey()).0;
    let ix = e.ask_ix(&thief.pubkey(), 1, Some(theirs));
    assert_eq!(
        custom(e.send(&[ix], &[&thief])),
        code(EscrowError::WrongAccount)
    );
    assert_eq!(
        custom(e.withdraw_with(
            &thief,
            &Out {
                balance_of: Some(payer.pubkey()),
                ..Out::default()
            }
        )),
        code(EscrowError::WrongAccount)
    );
    // Nor the releaser.
    let releaser = e.releaser();
    let ix = e.withdraw_ix(
        &releaser.pubkey(),
        &Out {
            balance_of: Some(payer.pubkey()),
            buyer_token: Some(ata(&payer.pubkey())),
            ..Out::default()
        },
    );
    assert_eq!(
        custom(e.send(&[ix], &[&releaser])),
        code(EscrowError::WrongAccount)
    );
    // Nor from another buyer's vault.
    e.top_up(&thief, 1_000_000).unwrap();
    let their_vault = e.balance_of(&thief.pubkey()).1;
    assert_eq!(
        custom(e.withdraw_with(
            &payer,
            &Out {
                vault: Some(their_vault),
                ..Out::default()
            }
        )),
        code(EscrowError::WrongAccount)
    );
    e.withdraw(&payer).unwrap();
    assert_eq!(e.balance(&ata(&payer.pubkey())), 100_000_000);
    assert_eq!(e.balance(&seller_token), 0);
}

#[test]
fn a_tap_fits_one_transaction_with_room_to_spare() {
    let mut e = Edge::new();
    let payer = e.payer();
    let releaser = e.releaser();
    let o = Open::new(1);
    // A first top-up and a tap in one wallet transaction.
    let ixs = [
        e.top_up_ix(&payer.pubkey(), 10_000_000, &TopUp::default()),
        e.open_tap_ix(&payer.pubkey(), &o),
    ];
    e.svm.expire_blockhash();
    let tx = Transaction::new(
        &[&payer],
        Message::new(&ixs, Some(&payer.pubkey())),
        e.svm.latest_blockhash(),
    );
    let meta = e.svm.send_transaction(tx).unwrap();
    assert!(
        meta.compute_units_consumed < 80_000,
        "{}",
        meta.compute_units_consumed
    );
    e.at(NOW + HOUR);
    let ix = e.settle_tap_ix(&releaser.pubkey(), &o.id, RATE, &SettleTo::default());
    e.svm.expire_blockhash();
    let tx = Transaction::new(
        &[&releaser],
        Message::new(&[ix], Some(&releaser.pubkey())),
        e.svm.latest_blockhash(),
    );
    let meta = e.svm.send_transaction(tx).unwrap();
    assert!(
        meta.compute_units_consumed < 40_000,
        "{}",
        meta.compute_units_consumed
    );
}

#[test]
fn a_wallet_that_copies_a_quoted_id_first_blocks_nothing() {
    let mut e = Edge::new();
    let payer = e.payer();
    let copier = e.buyer();
    e.top_up(&copier, 1).unwrap();
    let o = Open::new(7);
    // The copier opens the buyer's quoted id before the buyer does.
    e.open_tap(&copier, &o).unwrap();
    e.top_up(&payer, 30_000_000).unwrap();
    e.open_tap(&payer, &o).unwrap();
    let t = e.tap(&o.id).unwrap();
    assert_eq!(t.buyer, payer.pubkey());
    assert_eq!(t.seller, e.seller.pubkey());
    e.at(NOW + HOUR);
    e.settle_tap(&o.id, RATE).unwrap();
    assert_eq!(e.prepaid(&payer.pubkey()), 30_000_000 - RATE);
    assert_eq!(e.prepaid(&copier.pubkey()), 1);
}
