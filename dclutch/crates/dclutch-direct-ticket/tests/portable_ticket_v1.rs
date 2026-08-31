//! The ticket author, exercised the way a second binary has to exercise it.
//!
//! These are INTEGRATION tests on purpose. A unit test inside the crate would
//! pass just as well against a `pub(crate)` author, which is exactly the state
//! this crate exists to end: everything below reaches the author through the
//! published surface, so a regression that re-privatises it fails here rather
//! than at the next binary that tries to call it.

#![cfg(feature = "author")]

use std::path::{Path, PathBuf};

use dclutch_direct_ticket::{
    DIRECT_TICKET_AUTHOR_COMMAND_V1, DirectTicketAuthorReceiptV1, author_direct_intent_ticket_v1,
    author_with_keypair_path_v1, encode_portable_direct_ticket_v1, keypair_seed_from_file_v1,
    parse_arguments_v1, parse_portable_direct_ticket_v1, sha256_hex_v1, usage_v1,
};
use serde::Deserialize;
use solana_program::pubkey::Pubkey;

/// The invocation these tests author under. Any string would do; the point is
/// that it is a PARAMETER, because two binaries carry this author now.
const INVOCATION_V1: &str = "dclutch ticket author";

/// The two-sided ticket vector.
///
/// Emitted by `packages/dclutch-sdk/scripts/generate-direct-intent-ticket-vector.mjs`
/// through the SAME `encodeDirectIntentTicketV1` the browser trade panel calls,
/// and reproduced here by the Rust author. TypeScript is the incumbent producer
/// -- the browser has been the only ticket writer -- so TypeScript emits and
/// Rust matches. If this test goes red, the Rust author has drifted off the
/// wire the panel already puts on chain.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TicketVectorV1 {
    format: String,
    #[allow(dead_code)]
    note: String,
    maker_seed_fill: u8,
    #[allow(dead_code)]
    market_seed_fill: u8,
    #[allow(dead_code)]
    collateral_seed_fill: u8,
    side: u8,
    lifecycle: u8,
    outcome: u32,
    generation: u64,
    nonce: u64,
    valid_from: u64,
    valid_through: u64,
    maximum_fill: u64,
    limit_price: u64,
    fee_basis_points: u16,
    maker: String,
    market: String,
    collateral_account: String,
    signature_hex: String,
    ticket_text: String,
    ticket_sha256: String,
}

fn vector() -> TicketVectorV1 {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/dclutch-sdk/fixtures/direct-intent-ticket.json");
    serde_json::from_slice(&std::fs::read(&path).expect("ticket vector fixture"))
        .expect("ticket vector shape")
}

fn scratch_directory_v1(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "dclutch-direct-ticket-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("scratch directory");
    path
}

/// One 64-byte Solana CLI keypair file: the 32-byte seed then the 32-byte
/// public key it expands to, exactly as `solana-keygen` writes it.
///
/// The public half is taken from the base58 address the caller states rather
/// than recomputed, so this helper needs no signer of its own -- and so a file
/// whose halves DISAGREE is one line away, which is what the damaged-file test
/// wants.
fn write_keypair_file_v1(directory: &Path, name: &str, seed_fill: u8, declares: &str) -> PathBuf {
    let declared: Pubkey = declares.parse().expect("base58 public half");
    let mut bytes = vec![seed_fill; 32];
    bytes.extend_from_slice(&declared.to_bytes());
    let path = directory.join(name);
    std::fs::write(&path, serde_json::to_vec(&bytes).expect("keypair json")).expect("write");
    path
}

fn author_arguments_v1(pairs: &[(&str, &str)]) -> Vec<String> {
    pairs
        .iter()
        .flat_map(|(flag, value)| [(*flag).to_string(), (*value).to_string()])
        .collect()
}

/// Every argument that reproduces the vector's ticket, with `--out` pointed at
/// `out` and any single field overridden.
fn vector_arguments_v1(
    vector: &TicketVectorV1,
    out: &Path,
    override_flag: Option<(&str, &str)>,
) -> Vec<String> {
    let owned: Vec<(String, String)> = [
        ("--keypair-env", "DCLUTCH_TICKET_KEYPAIR".to_string()),
        ("--maker", vector.maker.clone()),
        ("--market", vector.market.clone()),
        ("--collateral-account", vector.collateral_account.clone()),
        (
            "--side",
            if vector.side == 0 { "sell" } else { "buy" }.to_string(),
        ),
        (
            "--lifecycle",
            if vector.lifecycle == 0 { "fok" } else { "ioc" }.to_string(),
        ),
        ("--outcome", vector.outcome.to_string()),
        ("--generation", vector.generation.to_string()),
        ("--nonce", vector.nonce.to_string()),
        ("--valid-from", vector.valid_from.to_string()),
        ("--valid-through", vector.valid_through.to_string()),
        ("--maximum-fill", vector.maximum_fill.to_string()),
        ("--limit-price", vector.limit_price.to_string()),
        ("--fee-basis-points", vector.fee_basis_points.to_string()),
        ("--out", out.display().to_string()),
    ]
    .into_iter()
    .map(|(flag, value)| (flag.to_string(), value))
    .collect();
    owned
        .into_iter()
        .flat_map(|(flag, value)| {
            let value = match override_flag {
                Some((overridden, replacement)) if overridden == flag => replacement.to_string(),
                _ => value,
            };
            [flag, value]
        })
        .collect()
}

/// THE POINT OF THIS CRATE: the ticket a second binary authors is the panel's
/// ticket, byte for byte, INCLUDING the signature -- which is the same
/// assertion as "both languages built the same 172-byte signing message".
#[test]
fn an_authored_ticket_is_byte_identical_to_the_browser_panel_wire() {
    let vector = vector();
    assert_eq!(vector.format, "dclutch/direct-intent-ticket-vector/v1");
    let scratch = scratch_directory_v1("vector");
    let key = write_keypair_file_v1(
        &scratch,
        "maker.json",
        vector.maker_seed_fill,
        &vector.maker,
    );
    let out = scratch.join("vector-ticket.json");

    let arguments = parse_arguments_v1(INVOCATION_V1, vector_arguments_v1(&vector, &out, None))
        .expect("the vector's arguments parse");
    let receipt = author_with_keypair_path_v1(arguments, &key).expect("the vector authors");

    let written = std::fs::read_to_string(&out).expect("the ticket was written");
    assert_eq!(
        written, vector.ticket_text,
        "the authored ticket is not the panel's bytes"
    );
    assert!(
        !written.ends_with('\n'),
        "the panel emits no trailing newline"
    );
    assert_eq!(receipt.ticket_sha256, vector.ticket_sha256);
    assert_eq!(receipt.ticket_bytes, vector.ticket_text.len());
    assert_eq!(receipt.maker, vector.maker);
    assert_eq!(receipt.signed_preimage_bytes, 172);
    assert_eq!(
        receipt.signature_domain,
        "dclutch/signature/direct-compact-intent-v2"
    );

    // And the reader agrees with the writer, on the same bytes.
    let signed = parse_portable_direct_ticket_v1(written.as_bytes(), "vector")
        .expect("the vector ticket reopens");
    assert_eq!(
        dclutch_direct_ticket::hex_lower_v1(&signed.signature),
        vector.signature_hex
    );
    assert_eq!(signed.maker.to_string(), vector.maker);
    assert_eq!(
        encode_portable_direct_ticket_v1(&signed).expect("re-encode"),
        vector.ticket_text
    );
    let _ = std::fs::remove_dir_all(&scratch);
}

/// The receipt hands the operator exactly the next argument and no path to
/// anything secret.
#[test]
fn the_receipt_names_the_ticket_and_never_the_key() {
    let vector = vector();
    let scratch = scratch_directory_v1("receipt");
    let key = write_keypair_file_v1(
        &scratch,
        "maker.json",
        vector.maker_seed_fill,
        &vector.maker,
    );
    let out = scratch.join("receipt-ticket.json");
    let arguments =
        parse_arguments_v1(INVOCATION_V1, vector_arguments_v1(&vector, &out, None)).expect("parse");
    let receipt: DirectTicketAuthorReceiptV1 =
        author_with_keypair_path_v1(arguments, &key).expect("author");

    let rendered = serde_json::to_string(&receipt).expect("receipt json");
    assert!(
        !rendered.contains("keypair"),
        "receipt named a key: {rendered}"
    );
    // The ticket the caller asked for is the ONLY path in the receipt. Anything
    // else here would be a filesystem fact the operator did not request, and
    // the key file lives in the same directory.
    assert_eq!(
        rendered
            .matches(scratch.to_str().expect("utf8 scratch"))
            .count(),
        1,
        "receipt carried a path beyond the ticket it wrote: {rendered}"
    );
    assert_eq!(receipt.ticket_sha256.len(), 64);
    assert_eq!(
        sha256_hex_v1(&std::fs::read(&out).expect("read back")),
        receipt.ticket_sha256,
        "the receipt digest is not the digest of the file on disk"
    );

    // A second write to the same path is refused, so a rerun cannot silently
    // replace a ticket an operator already quoted a digest for.
    let again =
        parse_arguments_v1(INVOCATION_V1, vector_arguments_v1(&vector, &out, None)).expect("parse");
    let error = author_with_keypair_path_v1(again, &key)
        .expect_err("a second author to the same path must be refused");
    assert!(
        format!("{error}").contains("already exists"),
        "unexpected refusal: {error}"
    );
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn a_key_that_is_not_the_stated_maker_never_signs() {
    let vector = vector();
    let scratch = scratch_directory_v1("wrong-maker");
    // The file is the vector's maker; the caller states the MARKET's address.
    let key = write_keypair_file_v1(
        &scratch,
        "someone-else.json",
        vector.maker_seed_fill,
        &vector.maker,
    );
    let out = scratch.join("refused.json");
    let arguments = parse_arguments_v1(
        INVOCATION_V1,
        vector_arguments_v1(&vector, &out, Some(("--maker", &vector.market))),
    )
    .expect("parse");
    let error = author_with_keypair_path_v1(arguments, &key)
        .expect_err("a key that is not the stated maker must not sign");
    let rendered = format!("{error}");
    assert!(
        rendered.contains("--maker"),
        "unexpected refusal: {rendered}"
    );
    assert!(
        !rendered.contains("someone-else.json"),
        "the refusal echoed the key path: {rendered}"
    );
    assert!(!out.exists(), "a refused author still wrote a ticket");
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn a_damaged_keypair_file_is_refused_before_it_is_ever_signed_with() {
    let vector = vector();
    let scratch = scratch_directory_v1("damaged");
    // Seed and declared public half disagree: the seed is the maker's, the
    // declared half is the collateral account's.
    let key = write_keypair_file_v1(
        &scratch,
        "damaged.json",
        vector.maker_seed_fill,
        &vector.collateral_account,
    );
    let error = keypair_seed_from_file_v1(&key, "Direct ticket maker")
        .expect_err("a file whose halves disagree must be refused");
    assert!(
        format!("{error}").contains("do not fund the address it prints"),
        "unexpected refusal: {error}"
    );
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn an_unset_variable_is_a_refusal_that_names_only_the_variable() {
    let vector = vector();
    let scratch = scratch_directory_v1("unset");
    let out = scratch.join("never-written.json");
    let arguments = parse_arguments_v1(
        INVOCATION_V1,
        vector_arguments_v1(
            &vector,
            &out,
            Some(("--keypair-env", "DCLUTCH_TICKET_KEYPAIR_ABSENT_V1")),
        ),
    )
    .expect("parse");
    let error =
        author_direct_intent_ticket_v1(arguments).expect_err("an unset variable must be refused");
    let rendered = format!("{error}");
    assert!(rendered.starts_with("REFUSED:"), "{rendered}");
    assert!(
        rendered.contains("DCLUTCH_TICKET_KEYPAIR_ABSENT_V1"),
        "{rendered}"
    );
    assert!(
        !rendered.contains('/'),
        "the refusal invented a path: {rendered}"
    );
    assert!(!out.exists(), "a refused author still wrote a ticket");
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn the_key_path_is_never_an_argument() {
    let usage = usage_v1(INVOCATION_V1);
    assert!(usage.starts_with(INVOCATION_V1));
    assert!(usage.contains("--keypair-env"));
    for forbidden in ["--keypair ", "--keypair-path", "--secret-key", "--seed"] {
        assert!(!usage.contains(forbidden), "usage exposed {forbidden}");
    }
    for refused in ["--keypair", "--keypair-path", "--secret-key"] {
        let error = parse_arguments_v1(
            INVOCATION_V1,
            author_arguments_v1(&[(refused, "/tmp/anything.json")]),
        )
        .expect_err("a path-bearing key flag must be refused");
        assert!(
            format!("{error}").contains("--keypair-env"),
            "refusal did not redirect to the environment variable: {error}"
        );
        assert!(
            !format!("{error}").contains("/tmp/anything.json"),
            "refusal echoed the path it was given: {error}"
        );
    }
    // The name the old refusal sent readers to still resolves to this author.
    assert_eq!(
        DIRECT_TICKET_AUTHOR_COMMAND_V1,
        "direct-intent-ticket-author-v1"
    );
}

#[test]
fn noncanonical_arguments_are_refused_before_a_key_is_opened() {
    let vector = vector();
    let out = PathBuf::from("/tmp/dclutch-direct-ticket-argument-test.json");
    parse_arguments_v1(INVOCATION_V1, vector_arguments_v1(&vector, &out, None))
        .expect("the canonical shape parses");
    for (flag, value) in [
        ("--side", "SELL"),
        ("--lifecycle", "gtc"),
        ("--fee-basis-points", "10001"),
        ("--maximum-fill", "0"),
        ("--limit-price", "0"),
        ("--valid-from", "4294967296"),
        ("--nonce", "007"),
        ("--outcome", "4294967296"),
        ("--maker", "11111111111111111111111111111111"),
        ("--out", "relative/path.json"),
        ("--keypair-env", "/Users/somebody/keys/founder.json"),
    ] {
        let error = parse_arguments_v1(
            INVOCATION_V1,
            vector_arguments_v1(&vector, &out, Some((flag, value))),
        )
        .expect_err(&format!("{flag}={value} must be refused"));
        assert!(
            format!("{error}").starts_with("REFUSED:"),
            "{flag}={value} produced a non-refusal: {error}"
        );
    }
}

#[test]
fn one_flipped_field_dies_at_the_signature_and_not_at_the_chain() {
    let vector = vector();
    let tampered = vector.ticket_text.replace(
        "\"maximumFill\": \"100000000\"",
        "\"maximumFill\": \"100000001\"",
    );
    assert_ne!(tampered, vector.ticket_text, "the tamper did not apply");
    let error = parse_portable_direct_ticket_v1(tampered.as_bytes(), "tampered")
        .expect_err("a changed field must not reopen");
    assert!(
        format!("{error}").contains("signature did not verify"),
        "unexpected refusal: {error}"
    );
}

#[test]
fn a_duplicate_key_cannot_smuggle_a_second_value_past_the_reader() {
    let vector = vector();
    let smuggled = vector.ticket_text.replace(
        "\"maximumFill\": \"100000000\"",
        "\"maximumFill\": \"100000000\",\n    \"maximumFill\": \"1\"",
    );
    assert_ne!(smuggled, vector.ticket_text, "the smuggle did not apply");
    let error = parse_portable_direct_ticket_v1(smuggled.as_bytes(), "smuggled")
        .expect_err("a duplicate key must not resolve to one of its values");
    assert!(
        format!("{error}").contains("duplicate JSON object key"),
        "unexpected refusal: {error}"
    );
}

#[test]
fn the_widest_legal_fields_survive_the_hostile_reader() {
    let vector = vector();
    let scratch = scratch_directory_v1("wide");
    let key = write_keypair_file_v1(
        &scratch,
        "maker.json",
        vector.maker_seed_fill,
        &vector.maker,
    );
    let out = scratch.join("wide-ticket.json");
    let arguments = parse_arguments_v1(
        INVOCATION_V1,
        author_arguments_v1(&[
            ("--keypair-env", "DCLUTCH_TICKET_KEYPAIR"),
            ("--maker", &vector.maker),
            ("--market", &vector.market),
            ("--collateral-account", &vector.collateral_account),
            ("--side", "buy"),
            ("--lifecycle", "ioc"),
            ("--outcome", "4294967295"),
            ("--generation", "18446744073709551615"),
            ("--nonce", "0"),
            ("--valid-from", "0"),
            ("--valid-through", "18446744073709551615"),
            ("--maximum-fill", "18446744073709551615"),
            ("--limit-price", "18446744073709551615"),
            ("--fee-basis-points", "10000"),
            ("--out", &out.display().to_string()),
        ]),
    )
    .expect("the widest legal shape parses");
    author_with_keypair_path_v1(arguments, &key).expect("the widest legal shape authors");
    let written = std::fs::read(&out).expect("written");
    let signed =
        parse_portable_direct_ticket_v1(&written, "wide").expect("the widest ticket reopens");
    assert_eq!(signed.intent.maximum_fill, u64::MAX);
    assert_eq!(signed.intent.fee_basis_points, 10_000);
    assert_eq!(signed.intent.outcome, u32::MAX);
    let _ = std::fs::remove_dir_all(&scratch);
}
