//! The treasury's two calls, as a signed transaction to send yourself:
//! `initialize` makes the config for a treasury and mint and names the
//! releaser, `set-releaser` names another. The treasury pays the fee and,
//! for `initialize`, the config's rent.
//!
//! ```sh
//! cargo run -q --example config -- initialize   PROGRAM TREASURY_KEYPAIR MINT RELEASER BLOCKHASH
//! cargo run -q --example config -- set-releaser PROGRAM TREASURY_KEYPAIR MINT RELEASER BLOCKHASH
//! ```
//!
//! `TREASURY_KEYPAIR` is a keypair file as `solana-keygen` writes it (a JSON
//! array of 64 numbers). It prints the transaction in base64 for
//! `sendTransaction`; README.md shows the whole round trip.

use std::str::FromStr;

use losos_escrow::{ix, CONFIG_SEED};
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [verb, program, keypair, mint, releaser, blockhash] = args.as_slice() else {
        eprintln!(
            "usage: config (initialize|set-releaser) PROGRAM TREASURY_KEYPAIR MINT RELEASER BLOCKHASH"
        );
        std::process::exit(2);
    };
    let address = |what: &str, raw: &str| {
        Address::from_str(raw).unwrap_or_else(|_| {
            eprintln!("{what} is not a base58 address: {raw}");
            std::process::exit(2);
        })
    };
    let program = address("PROGRAM", program);
    let mint = address("MINT", mint);
    let releaser = address("RELEASER", releaser);
    let blockhash = address("BLOCKHASH", blockhash);
    let treasury = read_keypair(keypair);
    let me = treasury.pubkey();
    let (config, _) =
        Address::find_program_address(&[CONFIG_SEED, me.as_ref(), mint.as_ref()], &program);

    // Keys in the order a legacy message wants them: the signing fee payer,
    // the writable config, then the read-only rest.
    let (tag, keys, readonly, accounts): (u8, Vec<Address>, u8, Vec<u8>) = match verb.as_str() {
        "initialize" => (
            ix::INITIALIZE,
            vec![me, config, mint, Address::default(), program],
            3,
            vec![0, 1, 2, 3],
        ),
        "set-releaser" => (ix::SET_RELEASER, vec![me, config, program], 1, vec![0, 1]),
        _ => {
            eprintln!("unknown call {verb}: initialize or set-releaser");
            std::process::exit(2);
        }
    };
    let mut message = vec![1, 0, readonly, keys.len() as u8];
    for k in &keys {
        message.extend_from_slice(k.as_ref());
    }
    message.extend_from_slice(blockhash.as_ref());
    message.push(1);
    message.push((keys.len() - 1) as u8);
    message.push(accounts.len() as u8);
    message.extend_from_slice(&accounts);
    message.push(33);
    message.push(tag);
    message.extend_from_slice(releaser.as_ref());

    let signature = treasury.sign_message(&message);
    let mut wire = vec![1u8];
    wire.extend_from_slice(signature.as_ref());
    wire.extend_from_slice(&message);
    eprintln!("config {config}, treasury {me}, signature {signature}");
    println!("{}", base64(&wire));
}

fn read_keypair(path: &str) -> Keypair {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("{path}: {e}");
        std::process::exit(2);
    });
    let bytes: Result<Vec<u8>, _> = text
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|n| n.trim().parse::<u8>())
        .collect();
    bytes
        .ok()
        .and_then(|b| Keypair::try_from(b.as_slice()).ok())
        .unwrap_or_else(|| {
            eprintln!("{path} is not a solana-keygen keypair file");
            std::process::exit(2);
        })
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                T[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}
