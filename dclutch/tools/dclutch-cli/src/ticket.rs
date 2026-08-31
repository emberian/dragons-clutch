//! `dclutch ticket` — author one Direct intent ticket, read one back, and
//! publish one to a board.
//!
//! WHAT THIS USED TO BE. Until the author became callable this module was a
//! REFUSAL: a Direct inline fill settles two independently signed intents, the
//! signed message is owned by `dclutch_direct_codec::intent_v2::CompactIntentV2`
//! emitted from `formal/dclutch-semantics/EmitDirectIntentV2Rust.lean`, and
//! there is exactly one author of a ticket per language because a second
//! implementation of a signing preimage is a signature that verifies nowhere,
//! discovered at the refused trade. The Rust author existed, but it was
//! `pub(crate)` inside the operator binary `dclutch-local-successor-bootstrap`,
//! which has no `[lib]`, so this binary could not call it.
//!
//! WHAT IT IS NOW. The author moved down into `crates/dclutch-direct-ticket`
//! and both binaries call THAT one. This module contributes no byte of the
//! ticket and no argument check of its own: it names the invocation, hands the
//! arguments over, and converts the shared crate's refusal into this binary's.
//! The tickets it writes are byte-identical to the browser trade panel's,
//! signature included, and `tests/ticket_v1.rs` proves it by running this
//! binary against the same cross-language vector the TypeScript side emits.
//!
//! AUTHORING IS NOT SUBMITTING, and this binary still submits nothing. See
//! [`where_the_ticket_goes_next_v1`].
//!
//! NEITHER IS PUBLISHING. [`post`] sends a ticket to an offer BOARD, which is a
//! relay and not a cluster: it holds bearer-signed data and hands it on. No
//! transaction is built and no lamport moves. A board can lose an offer or hide
//! it; it cannot change one, because every field is covered by the maker's
//! signature and a tampered field dies at the Ed25519 program.

use crate::{Error, Result};

/// The one line the top-level usage screen prints for this command.
pub const ONE_LINE_STATUS_V1: &str =
    "Sign a Direct intent into a portable ticket, or read one back.";

/// The operator subcommand that settles a PAIR of tickets on chain.
pub const PRODUCER_COMMAND_V1: &str = "devnet-direct-trade-produce-v1";

/// The binary that carries the producer. Not this one, on purpose.
pub const PRODUCER_BINARY_V1: &str = "dclutch-local-successor-bootstrap";

/// How the shared author names itself when this binary invokes it.
const AUTHOR_INVOCATION_V1: &str = "dclutch ticket author";

/// How the shared author names itself when `post` authors on the way through.
const POST_AUTHOR_INVOCATION_V1: &str = "dclutch ticket post";

/// The environment variable naming a board when `--board` is absent.
///
/// A board URL is not a credential — a board holds no keys and takes no
/// custody, so there is nothing in it to leak. It is an environment variable
/// anyway because a maker posts repeatedly to the same board, and because every
/// refusal in this file prints [`crate::rpc::origin`] rather than the URL, which
/// costs nothing and means a redirected `--board` cannot smuggle a secret into
/// a shell transcript either.
pub const BOARD_URL_ENV_V1: &str = "DCLUTCH_TICKET_BOARD_URL";

/// Lift a shared-crate refusal into this binary's error, text unchanged.
fn lift(error: dclutch_direct_ticket::Error) -> Error {
    Error::new(error.to_string())
}

/// Dispatch `author`, `verify`, or the usage screen.
pub fn run(arguments: Vec<String>) -> Result<()> {
    let (head, rest) = match arguments.split_first() {
        None => {
            print!("{}", usage());
            return Ok(());
        }
        Some((head, rest)) => (head.as_str(), rest.to_vec()),
    };
    match head {
        "help" | "-h" | "--help" => {
            print!("{}", usage());
            Ok(())
        }
        "author" => dclutch_direct_ticket::run_v1(AUTHOR_INVOCATION_V1, rest).map_err(lift),
        "verify" => verify(rest),
        "post" => post(rest),
        other => Err(Error::new(format!(
            "unknown ticket command `{other}`. Run `dclutch ticket` for the three it knows."
        ))),
    }
}

/// Re-read one ticket off disk and check its signature. No key, no network.
///
/// This is the same reader the producer runs on `--seller-ticket` and
/// `--buyer-ticket` before it opens a socket, so a ticket this command accepts
/// is a ticket that will clear the producer's admission — the producer then
/// re-checks every field against finalized chain state, which this command
/// cannot do and does not pretend to.
fn verify(arguments: Vec<String>) -> Result<()> {
    let path = match arguments.split_first() {
        Some((path, rest)) if rest.is_empty() && !path.starts_with('-') => path.clone(),
        _ => {
            return Err(Error::new(
                "usage: dclutch ticket verify <PATH>\n\nExactly one path, and no options.",
            ));
        }
    };
    let bytes = std::fs::read(&path)
        .map_err(|error| Error::new(format!("could not read the ticket at {path}: {error}")))?;
    let signed =
        dclutch_direct_ticket::parse_portable_direct_ticket_v1(&bytes, "this").map_err(lift)?;

    println!("ticket           {path}");
    println!(
        "sha256           {}",
        dclutch_direct_ticket::sha256_hex_v1(&bytes)
    );
    println!("bytes            {}", bytes.len());
    println!("signature        VERIFIED against the maker below");
    println!("maker            {}", signed.maker);
    println!("market           {}", crate::address(signed.intent.market));
    println!(
        "side             {}",
        if signed.intent.side == 0 {
            "sell"
        } else {
            "buy"
        }
    );
    println!(
        "lifecycle        {}",
        if signed.intent.lifecycle == 0 {
            "fok (fill or kill)"
        } else {
            "ioc (immediate or cancel)"
        }
    );
    println!("outcome          {}", signed.intent.outcome);
    println!("generation       {}", signed.intent.generation);
    println!("nonce            {}", signed.intent.nonce);
    println!(
        "valid slots      {}..={}",
        signed.intent.valid_from, signed.intent.valid_through
    );
    println!("maximum fill     {} atoms", signed.intent.maximum_fill);
    println!("limit price      {} scaled", signed.intent.limit_price);
    println!("fee              {} bps", signed.intent.fee_basis_points);
    println!(
        "collateral       {}",
        crate::address(signed.intent.collateral_account)
    );
    println!(
        "\nThe signature covers every field above and nothing else. A ticket is\n\
         one HALF of a trade; {where_next}",
        where_next = where_the_ticket_goes_next_v1()
    );
    Ok(())
}

/// Publish one ticket to a board — authoring it first if asked to.
///
/// POSTING IS NOT SUBMITTING, and the distinction is the whole reason this
/// command is allowed to exist in a binary whose header says it submits
/// nothing. A board is a relay: it accepts bearer-signed data, holds it, and
/// hands it to whoever asks. Nothing here builds a transaction, nothing here
/// reaches a cluster, and nothing here moves a lamport. The trade still happens
/// later, somewhere else, when a taker crosses this ticket with their own and
/// the chain verifies both signatures natively.
///
/// What a board can do to a ticket you post is LOSE it or HIDE it. What it
/// cannot do is change it: every field is covered by the signature written
/// here, and a tampered field dies at the Ed25519 program. That asymmetry is
/// why publishing to a stranger's relay is a safe act rather than a trusting
/// one.
///
/// Two shapes, because a maker has two situations:
///
/// - `post --board URL <PATH>` — a ticket already exists on disk; send it.
/// - `post --board URL <AUTHOR ARGUMENTS>` — author one and send it in the same
///   breath. The author's arguments are unchanged and unvalidated by this
///   function; they go to the same shared author `dclutch ticket author` runs.
fn post(arguments: Vec<String>) -> Result<()> {
    let mut board: Option<String> = None;
    let mut slot: Option<u64> = None;
    let mut rest: Vec<String> = Vec::new();

    let mut supplied = arguments.into_iter();
    while let Some(argument) = supplied.next() {
        match argument.as_str() {
            "--board" | "--slot" => {
                let Some(value) = supplied.next() else {
                    return Err(Error::new(format!("`{argument}` needs a value")));
                };
                if argument == "--board" {
                    board = Some(value);
                } else {
                    slot = Some(value.parse::<u64>().map_err(|_| {
                        Error::new("`--slot` is not one unsigned 64-bit slot number")
                    })?);
                }
            }
            _ => rest.push(argument),
        }
    }

    let board = match board.or_else(|| std::env::var(BOARD_URL_ENV_V1).ok()) {
        Some(url) => url,
        None => {
            return Err(Error::new(format!(
                "`dclutch ticket post` needs a board. Pass `--board URL` or set \
                 {BOARD_URL_ENV_V1}. There is no default: a board is one deployment's \
                 relay, not a property of the protocol, and guessing one would publish \
                 your offer somewhere you did not choose."
            )));
        }
    };

    // Which shape? Exactly one bare path is "send this file"; anything else is
    // author arguments, which the shared author validates in full.
    let path = match rest.as_slice() {
        [] => {
            return Err(Error::new(
                "usage: dclutch ticket post --board URL <PATH>\n   \
                 or: dclutch ticket post --board URL <AUTHOR ARGUMENTS, including --out>\n\n\
                 Run `dclutch ticket` for the author's arguments.",
            ));
        }
        [only] if !only.starts_with('-') => std::path::PathBuf::from(only),
        _ => {
            dclutch_direct_ticket::run_v1(POST_AUTHOR_INVOCATION_V1, rest.clone()).map_err(lift)?;
            authored_path_v1(&rest)?
        }
    };

    let bytes = std::fs::read(&path).map_err(|error| {
        Error::new(format!(
            "could not read the ticket at {}: {error}",
            path.display()
        ))
    })?;

    // Refuse HERE, before the network, with this binary's own reader. A maker
    // should learn that their ticket is malformed from the tool in front of
    // them, not from a stranger's service — and the board runs this identical
    // reader, so a ticket that clears this line clears its admission too.
    let signed =
        dclutch_direct_ticket::parse_portable_direct_ticket_v1(&bytes, "this").map_err(lift)?;

    let accepted = send_to_board_v1(&board, &bytes, slot)?;

    println!("ticket           {}", path.display());
    println!(
        "sha256           {}",
        dclutch_direct_ticket::sha256_hex_v1(&bytes)
    );
    println!("board            {}", crate::rpc::origin(&board));
    println!(
        "posted           {}",
        if accepted.duplicate {
            "already held (the board had this exact ticket)"
        } else {
            "accepted"
        }
    );
    println!("digest           {}", accepted.digest);
    println!("maker            {}", signed.maker);
    println!("market           {}", crate::address(signed.intent.market));
    println!("outcome          {}", signed.intent.outcome);
    println!(
        "valid slots      {}..={}",
        signed.intent.valid_from, signed.intent.valid_through
    );
    println!(
        "\nThe board holds this offer; it does not execute it. A relay can hide an\n\
         offer from a taker, but it cannot change one — every field above is covered\n\
         by your signature, and the chain re-derives the signing message and checks\n\
         it natively when someone crosses this ticket. Nothing was submitted."
    );
    Ok(())
}

/// The `--out` path the author was told to write, read back out of its own
/// arguments.
///
/// The author owns `--out`'s validation (absolute, must not already exist), so
/// this only has to find it; a missing one means the author would have refused
/// before reaching here.
fn authored_path_v1(arguments: &[String]) -> Result<std::path::PathBuf> {
    let mut walk = arguments.iter();
    while let Some(argument) = walk.next() {
        if argument == "--out" {
            if let Some(value) = walk.next() {
                return Ok(std::path::PathBuf::from(value));
            }
            break;
        }
    }
    Err(Error::new(
        "`dclutch ticket post` authored a ticket but cannot tell where: pass `--out PATH`.",
    ))
}

/// What a board said when it took a ticket.
struct AcceptedOfferV1 {
    digest: String,
    duplicate: bool,
}

/// POST one ticket's exact bytes to a board.
///
/// The bytes go up verbatim. A ticket's encoding is canonical and re-encoding
/// it here would make this command a SECOND writer of a shape that has exactly
/// one — the failure mode being a digest nobody else computes.
fn send_to_board_v1(url: &str, bytes: &[u8], slot: Option<u64>) -> Result<AcceptedOfferV1> {
    let endpoint = match slot {
        Some(slot) => format!("{}/tickets?slot={slot}", url.trim_end_matches('/')),
        None => format!("{}/tickets", url.trim_end_matches('/')),
    };
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent(concat!("dclutch/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| Error::new(format!("cannot build an HTTP client: {error}")))?;

    let response = client
        .post(&endpoint)
        .header("content-type", "application/json")
        .body(bytes.to_vec())
        .send()
        .map_err(|error| {
            Error::new(format!(
                "{} did not answer: {}",
                crate::rpc::origin(url),
                crate::rpc::redact(&error.to_string(), url)
            ))
        })?;

    let status = response.status();
    let body: serde_json::Value = response.json().map_err(|error| {
        Error::new(format!(
            "{} answered {status} with something that is not JSON: {}",
            crate::rpc::origin(url),
            crate::rpc::redact(&error.to_string(), url)
        ))
    })?;

    if !status.is_success() {
        // The board names every refusal. Carry BOTH halves: the name is what a
        // script branches on, the sentence is what a person acts on.
        let named = body
            .get("refusal")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("UNNAMED");
        let reason = body
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("and named no reason");
        return Err(Error::new(format!(
            "{} refused the offer ({named}): {reason}",
            crate::rpc::origin(url)
        )));
    }

    let digest = body
        .get("digest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            Error::new(format!(
                "{} accepted the offer without naming its digest",
                crate::rpc::origin(url)
            ))
        })?;
    Ok(AcceptedOfferV1 {
        digest: digest.to_owned(),
        duplicate: body
            .get("duplicate")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

/// The sentence that keeps authoring and submitting separate.
///
/// A reader who has just been handed a signed ticket will ask what to do with
/// it, and the honest answer is that this binary is not what does it.
#[must_use]
pub fn where_the_ticket_goes_next_v1() -> String {
    format!(
        "settling one needs the OTHER side's ticket and a transaction, \
         and this binary sends none. Two tickets — one sell, one buy — are settled by \
         `{PRODUCER_COMMAND_V1}` in the operator binary `{PRODUCER_BINARY_V1}`, which \
         re-checks every signed field against finalized chain state and refuses on any \
         mismatch. The browser panel at https://clutch.dregg.pro does the same trade with \
         a wallet and no key file at all."
    )
}

/// The whole `ticket` surface, in the order a reader needs it.
#[must_use]
pub fn usage() -> String {
    format!(
        "dclutch ticket — sign a Direct intent into a portable ticket.\n\
         \n\
         A Direct inline fill settles two independently signed intents. A ticket\n\
         is one of them: the maker, their detached Ed25519 signature, and every\n\
         field that signature covers. These bytes are byte-identical to the ones\n\
         the browser trade panel writes, signature included — one author per\n\
         language, and this is the Rust one.\n\
         \n\
         COMMANDS\n\
         \n\
         \x20 dclutch ticket author [ARGUMENTS BELOW]\n\
         \x20     Sign one intent and write the ticket to --out. Prints a receipt\n\
         \x20     on stdout carrying the ticket's SHA-256, which is exactly what\n\
         \x20     the producer wants told to it next.\n\
         \n\
         \x20 dclutch ticket verify <PATH>\n\
         \x20     Re-read a ticket, check its signature, and print every field it\n\
         \x20     binds. Takes no key and no network.\n\
         \n\
         \x20 dclutch ticket post --board URL [--slot SLOT] <PATH>\n\
         \x20 dclutch ticket post --board URL [--slot SLOT] [AUTHOR ARGUMENTS]\n\
         \x20     Publish a ticket to an offer board so a taker can find it —\n\
         \x20     either one already on disk, or one authored on the way through\n\
         \x20     with the same arguments `author` takes. `--slot` is your own\n\
         \x20     current slot and only lets the board refuse an offer that has\n\
         \x20     already expired. The board URL may come from {BOARD_URL_ENV_V1}\n\
         \x20     instead; there is no default, because a board is one\n\
         \x20     deployment's relay and guessing one would publish your offer\n\
         \x20     somewhere you did not choose.\n\
         \n\
         \x20     POSTING IS NOT SUBMITTING. A board is a relay: it holds\n\
         \x20     bearer-signed data and hands it on. This builds no transaction\n\
         \x20     and reaches no cluster. A board can LOSE your offer or HIDE it;\n\
         \x20     it cannot change one, because every field is covered by your\n\
         \x20     signature and a tampered field dies at the Ed25519 program.\n\
         \n\
         THE KEY NEVER APPEARS ON THE COMMAND LINE\n\
         \n\
         \x20 --keypair-env NAME   NAME is an ENVIRONMENT VARIABLE holding the\n\
         \x20                      absolute path of a Solana CLI keypair file.\n\
         \x20                      This is the only way in. Any flag that would\n\
         \x20                      carry a key, or the path to one, is refused at\n\
         \x20                      parse and named in the refusal — because a path\n\
         \x20                      on the command line is a path in the process\n\
         \x20                      table and in the shell history. Nothing about\n\
         \x20                      the key reaches the receipt or any refusal\n\
         \x20                      message either.\n\
         \n\
         \x20 --maker PUBKEY       The identity you BELIEVE that file holds. A key\n\
         \x20                      that expands to anything else is refused before\n\
         \x20                      its contents reach a signature.\n\
         \n\
         EVERY FIELD THE SIGNATURE BINDS, all required, none guessed\n\
         \n\
         \x20 --market PUBKEY              --collateral-account PUBKEY\n\
         \x20 --side sell|buy              --lifecycle fok|ioc\n\
         \x20 --outcome U32                --generation U64\n\
         \x20 --nonce U64                  --fee-basis-points BPS\n\
         \x20 --valid-from SLOT            --valid-through SLOT\n\
         \x20 --maximum-fill ATOMS         --limit-price SCALED\n\
         \x20 --out ABSOLUTE_JSON_THAT_DOES_NOT_EXIST\n\
         \n\
         \x20 Nothing here is read off a cluster: this command guesses no nonce,\n\
         \x20 no generation and no slot window, because a guessed field is a\n\
         \x20 signature over something you did not mean. Read them with\n\
         \x20 `dclutch market show` and pass them.\n\
         \n\
         WHERE THE TICKET GOES NEXT\n\
         \n\
         \x20 Authoring is not submitting. {where_next}\n",
        where_next = where_the_ticket_goes_next_v1()
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ONE_LINE_STATUS_V1, PRODUCER_BINARY_V1, PRODUCER_COMMAND_V1, run, usage,
        where_the_ticket_goes_next_v1,
    };

    #[test]
    fn the_usage_screen_never_offers_a_key_path_flag() {
        let text = usage();
        for forbidden in ["--keypair ", "--keypair-path", "--secret-key", "--seed"] {
            assert!(!text.contains(forbidden), "usage offered {forbidden}");
        }
        assert!(text.contains("--keypair-env"));
    }

    #[test]
    fn the_usage_screen_says_where_the_ticket_goes_next() {
        let text = usage();
        assert!(text.contains("Authoring is not submitting"), "{text}");
        assert!(text.contains(PRODUCER_COMMAND_V1), "{text}");
        assert!(text.contains(PRODUCER_BINARY_V1), "{text}");
        assert!(where_the_ticket_goes_next_v1().contains("this binary sends none"));
    }

    #[test]
    fn an_unknown_ticket_command_is_refused_and_not_guessed() {
        let error = run(vec!["submit".into()]).expect_err("`ticket submit` must not exist");
        assert!(
            error
                .to_string()
                .contains("unknown ticket command `submit`"),
            "{error}"
        );
    }

    #[test]
    fn verify_takes_exactly_one_path_and_no_options() {
        for arguments in [
            vec![],
            vec!["a".to_string(), "b".to_string()],
            vec!["--file".to_string(), "a".to_string()],
        ] {
            let mut call = vec!["verify".to_string()];
            call.extend(arguments);
            let error = run(call).expect_err("verify must be strict about its one argument");
            assert!(error.to_string().contains("Exactly one path"), "{error}");
        }
    }

    #[test]
    fn the_usage_screen_documents_post_and_keeps_it_apart_from_submitting() {
        let text = usage();
        assert!(text.contains("dclutch ticket post"), "{text}");
        assert!(text.contains("--board URL"), "{text}");
        assert!(text.contains(super::BOARD_URL_ENV_V1), "{text}");
        // The distinction the whole command rests on: a relay is not a cluster.
        assert!(text.contains("POSTING IS NOT SUBMITTING"), "{text}");
        assert!(text.contains("it cannot change one"), "{text}");
    }

    #[test]
    fn post_with_a_board_but_nothing_to_send_names_both_of_its_shapes() {
        let error = run(vec![
            "post".into(),
            "--board".into(),
            "http://127.0.0.1:8787".into(),
        ])
        .expect_err("post needs a ticket or the arguments to author one");
        let text = error.to_string();
        assert!(
            text.contains("dclutch ticket post --board URL <PATH>"),
            "{text}"
        );
        assert!(text.contains("AUTHOR ARGUMENTS, including --out"), "{text}");
    }

    #[test]
    fn a_flag_without_its_value_is_refused_before_any_socket_opens() {
        for flag in ["--board", "--slot"] {
            let error =
                run(vec!["post".into(), flag.into()]).expect_err("a dangling flag must be refused");
            assert!(
                error
                    .to_string()
                    .contains(&format!("`{flag}` needs a value")),
                "{error}"
            );
        }
    }

    #[test]
    fn a_slot_that_is_not_a_slot_is_refused_by_name() {
        let error = run(vec![
            "post".into(),
            "--board".into(),
            "http://127.0.0.1:8787".into(),
            "--slot".into(),
            "soon".into(),
        ])
        .expect_err("`--slot` takes a slot number");
        assert!(
            error
                .to_string()
                .contains("`--slot` is not one unsigned 64-bit slot number"),
            "{error}"
        );
    }

    #[test]
    fn the_authored_path_is_read_back_out_of_the_authors_own_arguments() {
        let found = super::authored_path_v1(&[
            "--maker".into(),
            "M".into(),
            "--out".into(),
            "/tmp/offer.json".into(),
        ])
        .expect("`--out` is right there");
        assert_eq!(found, std::path::PathBuf::from("/tmp/offer.json"));

        let error = super::authored_path_v1(&["--maker".into(), "M".into()])
            .expect_err("without --out there is nothing to post");
        assert!(error.to_string().contains("pass `--out PATH`"), "{error}");
    }

    #[test]
    fn the_one_line_status_no_longer_claims_the_command_is_absent() {
        assert!(!ONE_LINE_STATUS_V1.contains("Not in this release"));
        assert!(ONE_LINE_STATUS_V1.contains("Direct intent"));
    }
}
