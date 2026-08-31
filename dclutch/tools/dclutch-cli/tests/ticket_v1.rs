//! `dclutch ticket`, exercised by RUNNING THE BINARY, the way a user does.
//!
//! WHY A PROCESS AND NOT A FUNCTION CALL. Two things can only be proven by
//! spawning the real executable. First, `--keypair-env` names an environment
//! variable, and setting one in-process is `unsafe` under edition 2024, which
//! this crate forbids — but setting one on a CHILD needs no unsafe at all, so
//! the environment path gets covered here instead of being quietly untested.
//! Second, the claim this whole lane exists to make is about the bytes a user
//! gets from the released binary, and a unit test on an inner function does not
//! make that claim.
//!
//! The vector these tests measure against is emitted by TypeScript — by the
//! SAME `encodeDirectIntentTicketV1` the browser trade panel calls — so a green
//! run here means the CLI and the panel put the same bytes, and the same
//! signature, on the wire.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

use solana_program::pubkey::Pubkey;

/// The variable the tests hand the child. Its NAME is an argument; its VALUE,
/// the key path, never appears on a command line.
const KEYPAIR_ENV_V1: &str = "DCLUTCH_TICKET_KEYPAIR";

fn vector() -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/dclutch-sdk/fixtures/direct-intent-ticket.json");
    serde_json::from_slice(&std::fs::read(&path).expect("ticket vector fixture"))
        .expect("ticket vector shape")
}

fn text_field(vector: &serde_json::Value, name: &str) -> String {
    vector
        .get(name)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("vector field {name}"))
        .to_string()
}

fn number_field(vector: &serde_json::Value, name: &str) -> String {
    vector
        .get(name)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| panic!("vector field {name}"))
        .to_string()
}

fn scratch_directory_v1(name: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("dclutch-cli-ticket-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("scratch directory");
    path
}

/// One 64-byte Solana CLI keypair file: the 32-byte seed then the 32-byte
/// public key it expands to, exactly as `solana-keygen` writes it. The public
/// half comes from the vector's stated address, so this helper needs no signer.
fn write_keypair_file_v1(directory: &Path, seed_fill: u8, declares: &str) -> PathBuf {
    let declared: Pubkey = declares.parse().expect("base58 public half");
    let mut bytes = vec![seed_fill; 32];
    bytes.extend_from_slice(&declared.to_bytes());
    let path = directory.join("maker.json");
    std::fs::write(&path, serde_json::to_vec(&bytes).expect("keypair json")).expect("write");
    path
}

/// Run the released binary, with the key path reaching it ONLY through the
/// child's environment.
fn dclutch_v1(arguments: &[String], keypair_path: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dclutch"));
    command.args(arguments);
    command.env_remove(KEYPAIR_ENV_V1);
    if let Some(path) = keypair_path {
        command.env(KEYPAIR_ENV_V1, path);
    }
    command.output().expect("the dclutch binary runs")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

/// Every argument that reproduces the vector's ticket, with one field
/// optionally overridden.
fn author_arguments_v1(
    vector: &serde_json::Value,
    out: &Path,
    override_flag: Option<(&str, &str)>,
) -> Vec<String> {
    let side = if vector.get("side").and_then(serde_json::Value::as_u64) == Some(0) {
        "sell"
    } else {
        "buy"
    };
    let lifecycle = if vector.get("lifecycle").and_then(serde_json::Value::as_u64) == Some(0) {
        "fok"
    } else {
        "ioc"
    };
    let pairs: Vec<(String, String)> = vec![
        ("--keypair-env", KEYPAIR_ENV_V1.to_string()),
        ("--maker", text_field(vector, "maker")),
        ("--market", text_field(vector, "market")),
        (
            "--collateral-account",
            text_field(vector, "collateralAccount"),
        ),
        ("--side", side.to_string()),
        ("--lifecycle", lifecycle.to_string()),
        ("--outcome", number_field(vector, "outcome")),
        ("--generation", number_field(vector, "generation")),
        ("--nonce", number_field(vector, "nonce")),
        ("--valid-from", number_field(vector, "validFrom")),
        ("--valid-through", number_field(vector, "validThrough")),
        ("--maximum-fill", number_field(vector, "maximumFill")),
        ("--limit-price", number_field(vector, "limitPrice")),
        ("--fee-basis-points", number_field(vector, "feeBasisPoints")),
        ("--out", out.display().to_string()),
    ]
    .into_iter()
    .map(|(flag, value)| (flag.to_string(), value))
    .collect();

    let mut arguments = vec!["ticket".to_string(), "author".to_string()];
    for (flag, value) in pairs {
        let value = match override_flag {
            Some((overridden, replacement)) if overridden == flag => replacement.to_string(),
            _ => value,
        };
        arguments.push(flag);
        arguments.push(value);
    }
    arguments
}

/// THE CLAIM OF THIS LANE: `dclutch ticket` authors the browser panel's exact
/// bytes, signature included, through the released binary's own argument path.
#[test]
fn the_cli_authors_the_browser_panels_exact_bytes() {
    let vector = vector();
    let scratch = scratch_directory_v1("vector");
    let seed_fill = u8::try_from(
        vector
            .get("makerSeedFill")
            .and_then(serde_json::Value::as_u64)
            .expect("makerSeedFill"),
    )
    .expect("seed fill is a byte");
    let maker = text_field(&vector, "maker");
    let key = write_keypair_file_v1(&scratch, seed_fill, &maker);
    let out = scratch.join("vector-ticket.json");

    let output = dclutch_v1(&author_arguments_v1(&vector, &out, None), Some(&key));
    assert!(
        output.status.success(),
        "authoring failed: {}",
        stderr_of(&output)
    );

    let written = std::fs::read_to_string(&out).expect("the ticket was written");
    assert_eq!(
        written,
        text_field(&vector, "ticketText"),
        "the CLI-authored ticket is not the browser panel's bytes"
    );
    assert!(
        !written.ends_with('\n'),
        "the panel emits no trailing newline"
    );

    // The receipt hands the operator the digest the producer asks for next.
    let receipt: serde_json::Value =
        serde_json::from_str(&stdout_of(&output)).expect("the receipt is JSON");
    assert_eq!(
        receipt
            .get("ticketSha256")
            .and_then(serde_json::Value::as_str),
        Some(text_field(&vector, "ticketSha256").as_str())
    );
    assert_eq!(
        receipt.get("maker").and_then(serde_json::Value::as_str),
        Some(maker.as_str())
    );
    assert_eq!(
        receipt
            .get("signedPreimageBytes")
            .and_then(serde_json::Value::as_u64),
        Some(172)
    );

    // Nothing about the key reached the operator's screen.
    let rendered = format!("{}{}", stdout_of(&output), stderr_of(&output));
    assert!(
        !rendered.contains(key.to_str().expect("utf8 key path")),
        "the key path was echoed: {rendered}"
    );
    assert!(!rendered.contains("maker.json"), "{rendered}");

    // And the binary reads back what it just wrote, with no key in sight.
    let verified = dclutch_v1(
        &[
            "ticket".to_string(),
            "verify".to_string(),
            out.display().to_string(),
        ],
        None,
    );
    assert!(
        verified.status.success(),
        "verify failed: {}",
        stderr_of(&verified)
    );
    let report = stdout_of(&verified);
    assert!(report.contains("signature        VERIFIED"), "{report}");
    assert!(report.contains(&maker), "{report}");
    assert!(
        report.contains(&text_field(&vector, "ticketSha256")),
        "{report}"
    );
    assert!(report.contains("Authoring is not submitting") || report.contains("sends none"));

    let _ = std::fs::remove_dir_all(&scratch);
}

/// A key-material FLAG is refused at parse, and the refusal never repeats the
/// path it was handed.
#[test]
fn a_key_path_flag_is_refused_at_parse_and_never_echoed() {
    for flag in ["--keypair", "--keypair-path", "--secret-key"] {
        let output = dclutch_v1(
            &[
                "ticket".to_string(),
                "author".to_string(),
                flag.to_string(),
                "/Users/somebody/keys/founder.json".to_string(),
            ],
            None,
        );
        assert!(!output.status.success(), "{flag} was accepted");
        let rendered = stderr_of(&output);
        assert!(rendered.contains("REFUSED:"), "{rendered}");
        assert!(rendered.contains("--keypair-env"), "{rendered}");
        assert!(
            !rendered.contains("/Users/somebody/keys/founder.json"),
            "the refusal echoed the path it was given: {rendered}"
        );
    }
}

/// A path typed into `--keypair-env` itself is refused as a path, not read.
#[test]
fn a_path_in_the_environment_flag_is_refused_as_a_path() {
    let vector = vector();
    let out = scratch_directory_v1("envflag").join("never.json");
    let output = dclutch_v1(
        &author_arguments_v1(
            &vector,
            &out,
            Some(("--keypair-env", "/Users/somebody/keys/founder.json")),
        ),
        None,
    );
    assert!(!output.status.success());
    let rendered = stderr_of(&output);
    assert!(
        rendered.contains("must name one uppercase environment variable, not a path"),
        "{rendered}"
    );
    assert!(!out.exists(), "a refused author still wrote a ticket");
}

/// An unset variable refuses by NAME, and invents no path.
#[test]
fn an_unset_variable_refuses_and_names_only_the_variable() {
    let vector = vector();
    let scratch = scratch_directory_v1("unset");
    let out = scratch.join("never.json");
    let output = dclutch_v1(&author_arguments_v1(&vector, &out, None), None);
    assert!(
        !output.status.success(),
        "authoring without a key succeeded"
    );
    let rendered = stderr_of(&output);
    assert!(rendered.contains("REFUSED:"), "{rendered}");
    assert!(rendered.contains(KEYPAIR_ENV_V1), "{rendered}");
    assert!(
        !rendered.contains('/'),
        "the refusal invented a path: {rendered}"
    );
    assert!(!out.exists(), "a refused author still wrote a ticket");
    let _ = std::fs::remove_dir_all(&scratch);
}

/// A key that is not the stated maker never signs, and the refusal names
/// neither the file nor the identity the file actually holds.
#[test]
fn a_key_that_is_not_the_stated_maker_never_signs() {
    let vector = vector();
    let scratch = scratch_directory_v1("wrong-maker");
    let seed_fill = u8::try_from(
        vector
            .get("makerSeedFill")
            .and_then(serde_json::Value::as_u64)
            .expect("makerSeedFill"),
    )
    .expect("seed fill is a byte");
    let key = write_keypair_file_v1(&scratch, seed_fill, &text_field(&vector, "maker"));
    let out = scratch.join("refused.json");
    let market = text_field(&vector, "market");
    let output = dclutch_v1(
        &author_arguments_v1(&vector, &out, Some(("--maker", &market))),
        Some(&key),
    );
    assert!(!output.status.success(), "a wrong maker was accepted");
    let rendered = stderr_of(&output);
    assert!(rendered.contains("--maker"), "{rendered}");
    assert!(!rendered.contains("maker.json"), "{rendered}");
    assert!(!out.exists(), "a refused author still wrote a ticket");
    let _ = std::fs::remove_dir_all(&scratch);
}

/// One flipped field dies at the signature, in the binary, before any chain.
#[test]
fn a_tampered_ticket_is_refused_by_verify() {
    let vector = vector();
    let scratch = scratch_directory_v1("tampered");
    let tampered = text_field(&vector, "ticketText").replace(
        "\"maximumFill\": \"100000000\"",
        "\"maximumFill\": \"100000001\"",
    );
    let path = scratch.join("tampered.json");
    std::fs::write(&path, &tampered).expect("write tampered ticket");
    let output = dclutch_v1(
        &[
            "ticket".to_string(),
            "verify".to_string(),
            path.display().to_string(),
        ],
        None,
    );
    assert!(!output.status.success(), "a tampered ticket verified");
    let rendered = stderr_of(&output);
    assert!(
        rendered.contains("signature did not verify"),
        "unexpected refusal: {rendered}"
    );
    let _ = std::fs::remove_dir_all(&scratch);
}

/// The help screens tell a reader where the ticket goes next, because this
/// binary is not what takes it there.
#[test]
fn the_help_screens_separate_authoring_from_submitting() {
    let ticket_help = stdout_of(&dclutch_v1(&["ticket".to_string()], None));
    assert!(
        ticket_help.contains("Authoring is not submitting"),
        "{ticket_help}"
    );
    assert!(
        ticket_help.contains("devnet-direct-trade-produce-v1"),
        "{ticket_help}"
    );
    for forbidden in ["--keypair ", "--keypair-path", "--secret-key"] {
        assert!(
            !ticket_help.contains(forbidden),
            "the help screen offered {forbidden}"
        );
    }

    let top_help = stdout_of(&dclutch_v1(&["--help".to_string()], None));
    assert!(top_help.contains("dclutch ticket author"), "{top_help}");
    assert!(
        top_help.contains("never submits a transaction"),
        "{top_help}"
    );
}
