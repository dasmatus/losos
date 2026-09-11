//! The appliance recovery code: minted once, stable forever after, and never
//! in a place it can leak from.
//!
//! Exercised through the crate's public surface, so these tests see exactly
//! what `lososd` sees: a `#[path]` include would compile a second, private copy
//! of the module and could keep passing after the real one stopped being
//! reachable.
use losos_ctl::fake::FakeLosos;
use losos_ctl::losos::cmd_recovery;
use losos_ctl::recovery::{
    code_file_from, ensure_code, format_uuid_v4, is_well_formed, plan_ensure, CodeStore, Ensure,
    MemoryStore, MintReason, Recovery, DEFAULT_RECOVERY_FILE, UUID_LEN,
};

/// A well-formed code, written by hand rather than generated, so a bug in the
/// generator cannot make the validator's tests pass.
const GOOD: &str = "8f3a1c2e-5b7d-4a19-9e06-2c4f8b1d3a57";

// ── Minting once ────────────────────────────────────────────────────────────

#[test]
fn a_fresh_appliance_mints_one_code_and_says_so() {
    let mut store = MemoryStore::empty();
    let got = ensure_code(&mut store).expect("minting on an empty store");

    assert!(
        got.minted,
        "the first call is the one that creates the code"
    );
    assert!(
        is_well_formed(&got.code),
        "minted a code the validator rejects: {}",
        got.code
    );
    assert_eq!(store.writes, 1, "one mint is one write");
}

#[test]
fn the_code_is_stable_across_reads_and_is_never_re_minted() {
    // This is the entire contract. A code that changed between two calls would
    // invalidate whatever the owner wrote down, which is the failure the module
    // exists to prevent — so it is asserted over repeated calls, not once.
    let mut store = MemoryStore::empty();
    let first = ensure_code(&mut store).expect("first call").code;

    for call in 2..=10 {
        let again = ensure_code(&mut store).expect("later call");
        assert_eq!(again.code, first, "the code changed on call {call}");
        assert!(
            !again.minted,
            "call {call} claimed to have minted a code it only read"
        );
    }
    assert_eq!(
        store.writes, 1,
        "the file was rewritten after the first mint"
    );
}

#[test]
fn an_existing_code_is_returned_untouched() {
    // Trailing newline included: that is how the file is written, and how any
    // editor would leave it. Mistaking it for corruption would re-mint.
    let mut store = MemoryStore::with_file(&format!("{GOOD}\n"));
    let got = ensure_code(&mut store).expect("reading an existing code");

    assert_eq!(got.code, GOOD);
    assert!(!got.minted);
    assert_eq!(store.writes, 0, "reading must not write");
}

#[test]
fn two_appliances_do_not_get_the_same_code() {
    let a = ensure_code(&mut MemoryStore::empty()).expect("box a").code;
    let mut second = MemoryStore::empty();
    // Drain one draw so the two stores are not at the same counter — the point
    // is that nothing in the mint path is a constant.
    let _ = second.fresh_bytes().expect("drawing bytes");
    let b = ensure_code(&mut second).expect("box b").code;

    assert_ne!(a, b);
}

#[test]
fn a_read_error_is_propagated_rather_than_minted_over() {
    // The dangerous silent failure: an unreadable file treated as a missing one
    // overwrites a code the owner still holds. `read_code` returning `None`
    // must mean "no file", never "could not tell".
    struct Unreadable;
    impl CodeStore for Unreadable {
        fn read_code(&mut self) -> anyhow::Result<Option<String>> {
            Err(anyhow::anyhow!("EIO reading the code file"))
        }
        fn write_code(&mut self, _code: &str) -> anyhow::Result<()> {
            panic!("wrote a new code over a file it could not read");
        }
        fn fresh_bytes(&mut self) -> anyhow::Result<[u8; 16]> {
            panic!("generated a code for a file it could not read");
        }
    }

    let err = ensure_code(&mut Unreadable).expect_err("an unreadable file must fail");
    assert!(err.to_string().contains("EIO"), "lost the cause: {err}");
}

// ── The planner ─────────────────────────────────────────────────────────────

#[test]
fn plan_ensure_keeps_anything_well_formed_and_mints_over_nothing_else() {
    assert_eq!(plan_ensure(Some(GOOD)), Ensure::Keep(GOOD.to_string()));
    assert_eq!(
        plan_ensure(Some(&format!("  {GOOD}\n"))),
        Ensure::Keep(GOOD.to_string()),
        "surrounding whitespace is not corruption"
    );

    assert_eq!(plan_ensure(None), Ensure::Mint(MintReason::Absent));
    assert_eq!(
        plan_ensure(Some("   \n")),
        Ensure::Mint(MintReason::Absent),
        "an empty file is a write that never landed, not a tampered one"
    );
    assert_eq!(
        plan_ensure(Some("hunter2")),
        Ensure::Mint(MintReason::Malformed)
    );
}

// ── Format ──────────────────────────────────────────────────────────────────

#[test]
fn the_version_and_variant_nibbles_are_forced_whatever_the_randomness_says() {
    // All-zero and all-ones input: everything except the six overwritten bits
    // passes straight through, so these two vectors pin the layout exactly.
    assert_eq!(
        format_uuid_v4([0x00; 16]),
        "00000000-0000-4000-8000-000000000000"
    );
    assert_eq!(
        format_uuid_v4([0xff; 16]),
        "ffffffff-ffff-4fff-bfff-ffffffffffff"
    );
}

#[test]
fn the_hex_is_lowercase_and_the_hyphens_are_where_uuids_put_them() {
    let mut bytes = [0u8; 16];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = (i as u8) * 17; // 0x00, 0x11, 0x22, ... — every nibble pair legible
    }
    let uuid = format_uuid_v4(bytes);

    assert_eq!(uuid, "00112233-4455-4677-8899-aabbccddeeff");
    assert_eq!(uuid.len(), UUID_LEN);
    assert!(
        !uuid.chars().any(|c| c.is_ascii_uppercase()),
        "one code must have exactly one spelling: {uuid}"
    );
}

#[test]
fn everything_the_generator_produces_survives_the_validator() {
    // The round trip, over a wide spread of inputs rather than one. A generator
    // and a validator that disagree would mint a fresh code on every single
    // read, which is the worst available bug here and the least visible.
    let mut store = MemoryStore::empty();
    for draw in 0..512 {
        let uuid = format_uuid_v4(store.fresh_bytes().expect("drawing bytes"));
        assert!(is_well_formed(&uuid), "draw {draw} rejected: {uuid}");
    }
}

// ── Validation ──────────────────────────────────────────────────────────────

#[test]
fn the_validator_rejects_everything_that_is_not_a_canonical_v4_uuid() {
    for (why, bad) in [
        ("empty", ""),
        ("too short", "8f3a1c2e-5b7d-4a19-9e06-2c4f8b1d3a5"),
        ("too long", "8f3a1c2e-5b7d-4a19-9e06-2c4f8b1d3a577"),
        ("uppercase", "8F3A1C2E-5B7D-4A19-9E06-2C4F8B1D3A57"),
        ("unhyphenated", "8f3a1c2e5b7d4a199e062c4f8b1d3a5700ff"),
        (
            "hyphens in the wrong places",
            "8f3a1c2e5-b7d-4a19-9e06-2c4f8b1d3a57",
        ),
        ("not hex", "8f3a1c2e-5b7d-4a19-9e06-2c4f8b1d3azz"),
        ("version 1, not 4", "8f3a1c2e-5b7d-1a19-9e06-2c4f8b1d3a57"),
        (
            "no RFC 4122 variant",
            "8f3a1c2e-5b7d-4a19-1e06-2c4f8b1d3a57",
        ),
    ] {
        assert!(!is_well_formed(bad), "accepted a {why} code: {bad:?}");
    }

    assert!(is_well_formed(GOOD), "rejected a valid code: {GOOD}");
    // Every RFC 4122 variant nibble, since those four are easy to get wrong.
    for v in ['8', '9', 'a', 'b'] {
        let code: String = GOOD
            .char_indices()
            .map(|(i, c)| if i == 19 { v } else { c })
            .collect();
        assert!(is_well_formed(&code), "rejected variant {v}: {code}");
    }
}

// ── Leaks ───────────────────────────────────────────────────────────────────

#[test]
fn the_code_does_not_appear_in_debug_output() {
    // `tracing` takes `?value`. A derived Debug would put the code in the
    // journal the first time someone added a field to a log line, and that
    // would look exactly like every other log line in the crate.
    let r = Recovery {
        code: GOOD.to_string(),
        minted: true,
    };
    let rendered = format!("{r:?}");

    assert!(
        !rendered.contains(GOOD),
        "Debug leaked the code: {rendered}"
    );
    assert!(rendered.contains("redacted"));
    assert!(rendered.contains("minted: true"), "lost the useful half");
}

// ── Where the file lives ────────────────────────────────────────────────────

#[test]
fn the_path_falls_back_to_var_secrets_when_the_unit_says_nothing() {
    assert_eq!(
        code_file_from(None),
        std::path::Path::new(DEFAULT_RECOVERY_FILE)
    );
    assert_eq!(
        code_file_from(Some("   ")),
        std::path::Path::new(DEFAULT_RECOVERY_FILE),
        "an empty LOSOS_RECOVERY_FILE means the unit left it unset"
    );
    assert_eq!(
        code_file_from(Some("/var/secrets/elsewhere")),
        std::path::Path::new("/var/secrets/elsewhere")
    );
    assert!(
        DEFAULT_RECOVERY_FILE.starts_with("/var/"),
        "the default must be under /var or impermanence drops it on reboot"
    );
}

// ── The command layer, against the fake backend ─────────────────────────────
//
// Everything above tests the module in isolation. These drive `cmd_recovery`
// through the `Losos` trait, which is the path `lososd` actually takes, so a
// trait method wired to the wrong store fails here and nowhere else.

#[test]
fn the_command_mints_on_a_fresh_appliance_and_repeats_itself_after() {
    let mut f = FakeLosos::new();

    let first = cmd_recovery(&mut f).expect("first read on a fresh appliance");
    assert_eq!(first["minted"], true, "the first call is the minting one");
    let code = first["code"].as_str().expect("code must be a string");
    assert!(is_well_formed(code), "minted a malformed code: {code}");

    let second = cmd_recovery(&mut f).expect("second read");
    assert_eq!(
        second["code"], first["code"],
        "the code changed between calls; the owner's written copy is now wrong"
    );
    assert_eq!(
        second["minted"], false,
        "a re-read must not claim to have minted anything"
    );
    assert_eq!(
        f.recovery.writes, 1,
        "the code file was written more than once"
    );
}

#[test]
fn a_code_already_on_disk_is_returned_rather_than_replaced() {
    let mut f = FakeLosos::new();
    f.recovery = MemoryStore::with_file(&format!("{GOOD}\n"));

    let out = cmd_recovery(&mut f).expect("reading an existing code");
    assert_eq!(
        out["code"], GOOD,
        "returned something other than the stored code"
    );
    assert_eq!(out["minted"], false);
    assert_eq!(
        f.recovery.writes, 0,
        "an existing code must never be rewritten"
    );
}

#[test]
fn the_response_carries_the_code_and_nothing_else_about_the_box() {
    let mut f = FakeLosos::new();
    let out = cmd_recovery(&mut f).expect("reading the code");

    let obj = out.as_object().expect("the response is a JSON object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    // schema.json's recoveryResponse sets additionalProperties: false, so a new
    // field here is a wire-contract change and has to be made deliberately.
    assert_eq!(
        keys,
        ["code", "minted"],
        "recoveryResponse gained or lost a field"
    );
}
