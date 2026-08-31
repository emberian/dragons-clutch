//! `dclutch ticket` — author one Direct intent ticket, and read one back.
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
        other => Err(Error::new(format!(
            "unknown ticket command `{other}`. Run `dclutch ticket` for the two it knows."
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
    fn the_one_line_status_no_longer_claims_the_command_is_absent() {
        assert!(!ONE_LINE_STATUS_V1.contains("Not in this release"));
        assert!(ONE_LINE_STATUS_V1.contains("Direct intent"));
    }
}
