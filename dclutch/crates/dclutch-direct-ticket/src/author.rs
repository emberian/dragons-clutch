//! Authoring: the only half of this crate that touches key material.
//!
//! Split behind the `author` feature so a consumer that merely READS tickets --
//! or that only wants the [`SignedDirectIntentV3`](crate::SignedDirectIntentV3)
//! type -- links no signer at all and can go on truthfully saying it never
//! signs.
//!
//! WHAT THIS MODULE REFUSES TO DO: read chain state, guess a nonce, guess a
//! slot window, take the maker on faith, or submit. Every field the signature
//! binds is a required argument, and `--maker` states which identity the caller
//! BELIEVES the key file holds -- a keypair that expands to anything else is
//! refused before the file's contents reach a signature. The producer re-checks
//! every one of these fields against finalized chain state and refuses on any
//! mismatch, so a wrong argument here is a refused trade, never a different
//! trade.

use std::{
    env, fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use dclutch_direct_codec::intent_v2::CompactIntentV2;
use serde::{Deserialize, Serialize};
use solana_keypair::Keypair;
use solana_program::pubkey::Pubkey;
use solana_signer::Signer as _;

use crate::{
    Error, Result,
    envelope::{
        SignedDirectIntentV3, canonical_ticket_pubkey_v1, canonical_ticket_u64_v1,
        encode_portable_direct_ticket_v1, parse_portable_direct_ticket_v1, sha256_hex_v1,
    },
    refusal,
};

/// The name this author has carried since it was a subcommand of the operator
/// binary, kept so a reader who followed the old refusal still lands here.
pub const DIRECT_TICKET_AUTHOR_COMMAND_V1: &str = "direct-intent-ticket-author-v1";

const DIRECT_TICKET_AUTHOR_RECEIPT_SCHEMA_V1: &str =
    "dclutch-direct-intent-ticket-author-receipt-v1";

/// Every argument the author needs, all of them checked before a key is opened.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectTicketAuthorArgumentsV1 {
    keypair_env: String,
    maker: Pubkey,
    market: Pubkey,
    collateral_account: Pubkey,
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
    out: PathBuf,
}

/// What the author tells the operator, and nothing more.
///
/// `ticket_sha256` is here because it is exactly the next argument:
/// `--expected-seller-ticket-sha256` / `--expected-buyer-ticket-sha256`. There
/// is no keypair path field, deliberately.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DirectTicketAuthorReceiptV1 {
    /// Always `dclutch-direct-intent-ticket-author-receipt-v1`.
    pub schema: String,
    /// Absolute path of the ticket that was written.
    pub ticket: String,
    /// SHA-256 of the ticket bytes, as the producer will want to be told them.
    pub ticket_sha256: String,
    /// Width of the ticket on disk.
    pub ticket_bytes: usize,
    /// Base58 maker, as the key file actually expanded to it.
    pub maker: String,
    /// Base58 Market.
    pub market: String,
    /// `sell` or `buy`.
    pub side: String,
    /// `fok` or `ioc`.
    pub lifecycle: String,
    /// Runtime outcome coordinate.
    pub outcome: u32,
    /// Market generation the intent is bound to.
    pub generation: u64,
    /// Maker nonce.
    pub nonce: u64,
    /// First valid slot.
    pub valid_from: u64,
    /// Last valid slot.
    pub valid_through: u64,
    /// Maximum fill in collateral atoms.
    pub maximum_fill: u64,
    /// Scaled limit price.
    pub limit_price: u64,
    /// Fee in basis points.
    pub fee_basis_points: u16,
    /// Base58 collateral account.
    pub collateral_account: String,
    /// Width of the message that was actually signed.
    pub signed_preimage_bytes: usize,
    /// The domain string that message begins with.
    pub signature_domain: String,
}

/// The usage screen, under whatever invocation the calling binary offers.
///
/// The invocation is a parameter because two binaries carry this author --
/// `dclutch ticket author` and the operator's `direct-intent-ticket-author-v1`
/// subcommand -- and a usage screen naming the wrong one sends the reader to a
/// command that does not exist.
#[must_use]
pub fn usage_v1(invocation: &str) -> String {
    format!(
        "{invocation} \
         --keypair-env ENVIRONMENT_VARIABLE_HOLDING_THE_ABSOLUTE_KEYPAIR_PATH \
         --maker PUBKEY --market PUBKEY --side sell|buy --lifecycle fok|ioc \
         --outcome U32 --generation U64 --nonce U64 --valid-from SLOT \
         --valid-through SLOT --maximum-fill ATOMS --limit-price SCALED \
         --fee-basis-points BPS --collateral-account PUBKEY \
         --out ABSOLUTE_JSON_THAT_DOES_NOT_EXIST"
    )
}

/// Author one ticket and print the receipt as JSON on stdout.
///
/// # Errors
///
/// Every refusal in this module, including an unset `--keypair-env` variable
/// and a key file that does not expand to the stated `--maker`.
pub fn run_v1(invocation: &str, arguments: Vec<String>) -> Result<()> {
    let receipt = author_direct_intent_ticket_v1(parse_arguments_v1(invocation, arguments)?)?;
    let mut stdout = std::io::stdout();
    serde_json::to_writer_pretty(&mut stdout, &receipt)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

/// Resolve the key path out of the environment, then author.
///
/// # Errors
///
/// If the named variable is unset, empty, or does not hold an absolute path,
/// and every refusal [`author_with_keypair_path_v1`] can raise.
pub fn author_direct_intent_ticket_v1(
    arguments: DirectTicketAuthorArgumentsV1,
) -> Result<DirectTicketAuthorReceiptV1> {
    let keypair_path = keypair_path_from_environment_v1(&arguments.keypair_env)?;
    author_with_keypair_path_v1(arguments, &keypair_path)
}

/// The author with the key path already resolved.
///
/// Split out from [`author_direct_intent_ticket_v1`] so tests can exercise the
/// whole authoring path without an environment variable: `env::set_var` is
/// `unsafe` under edition 2024 and this crate forbids `unsafe_code`, so a test
/// that wanted one would have had to weaken the crate to get it. A test that
/// wants the environment path too spawns a real process and sets the variable
/// on the CHILD, which needs no unsafe anywhere.
///
/// # Errors
///
/// If the output path already exists, the key file is unreadable or damaged,
/// the key does not expand to the stated maker, or the authored ticket does not
/// reopen as the intent that was signed.
pub fn author_with_keypair_path_v1(
    arguments: DirectTicketAuthorArgumentsV1,
    keypair_path: &Path,
) -> Result<DirectTicketAuthorReceiptV1> {
    let intent = CompactIntentV2 {
        side: arguments.side,
        lifecycle: arguments.lifecycle,
        outcome: arguments.outcome,
        market: arguments.market.to_bytes(),
        generation: arguments.generation,
        nonce: arguments.nonce,
        valid_from: arguments.valid_from,
        valid_through: arguments.valid_through,
        maximum_fill: arguments.maximum_fill,
        limit_price: arguments.limit_price,
        fee_basis_points: arguments.fee_basis_points,
        collateral_account: arguments.collateral_account.to_bytes(),
    };
    if arguments.out.exists() {
        return Err(refusal(format!(
            "ticket output already exists: {}",
            arguments.out.display()
        )));
    }

    // No secret-bearing file is opened above this line.
    let keypair = Keypair::new_from_array(keypair_seed_from_file_v1(
        keypair_path,
        "Direct ticket maker",
    )?);
    if keypair.pubkey() != arguments.maker {
        // Naming neither the path nor the identity the file actually holds:
        // the caller stated a maker, and the only useful fact is that the file
        // behind their own environment variable is not that maker.
        return Err(refusal(format!(
            "the keypair named by ${} does not expand to the stated --maker {}",
            arguments.keypair_env, arguments.maker
        )));
    }
    let signed = sign_direct_intent_v1(&keypair, intent)?;
    drop(keypair);

    let text = encode_portable_direct_ticket_v1(&signed)?;
    // Read our own emission back through the hostile reader the producer uses.
    // If these two ever disagree the ticket is refused here, where the operator
    // can still fix it, rather than at the producer with a wasted slot window.
    if parse_portable_direct_ticket_v1(text.as_bytes(), "authored")? != signed {
        return Err(refusal(
            "the authored ticket did not reopen as the intent that was signed",
        ));
    }
    write_create_new_v1(&arguments.out, text.as_bytes())?;
    Ok(DirectTicketAuthorReceiptV1 {
        schema: DIRECT_TICKET_AUTHOR_RECEIPT_SCHEMA_V1.into(),
        ticket: arguments.out.display().to_string(),
        ticket_sha256: sha256_hex_v1(text.as_bytes()),
        ticket_bytes: text.len(),
        maker: signed.maker.to_string(),
        market: arguments.market.to_string(),
        side: if arguments.side == 0 { "sell" } else { "buy" }.into(),
        lifecycle: if arguments.lifecycle == 0 {
            "fok"
        } else {
            "ioc"
        }
        .into(),
        outcome: arguments.outcome,
        generation: arguments.generation,
        nonce: arguments.nonce,
        valid_from: arguments.valid_from,
        valid_through: arguments.valid_through,
        maximum_fill: arguments.maximum_fill,
        limit_price: arguments.limit_price,
        fee_basis_points: arguments.fee_basis_points,
        collateral_account: arguments.collateral_account.to_string(),
        signed_preimage_bytes:
            dclutch_direct_codec::intent_v2::COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2,
        signature_domain: String::from_utf8(
            dclutch_direct_codec::intent_v2::COMPACT_INTENT_SIGNATURE_DOMAIN_PREIMAGE_V2.to_vec(),
        )
        .map_err(|_| refusal("the Direct signature domain preimage is not UTF-8"))?,
    })
}

/// Sign one exact intent with the standard Solana Ed25519 signer.
///
/// The freshly produced signature is verified against the same preimage before
/// it is handed back, so a signer that silently produced the wrong bytes is a
/// refusal here and not a refused transaction someone debugs later.
///
/// # Errors
///
/// If the intent cannot produce a preimage, or the fresh signature does not
/// verify against the key that just made it.
pub fn sign_direct_intent_v1(
    keypair: &Keypair,
    intent: CompactIntentV2,
) -> Result<SignedDirectIntentV3> {
    let preimage = intent
        .signed_preimage()
        .map_err(|error| Error::new(format!("Direct intent preimage: {error:?}")))?;
    let signature = keypair.sign_message(&preimage);
    if !signature.verify(keypair.pubkey().as_ref(), &preimage) {
        return Err(refusal("fresh Direct signature did not verify"));
    }
    Ok(SignedDirectIntentV3 {
        maker: keypair.pubkey(),
        signature: signature.as_ref().try_into().map_err(|_| {
            refusal("fresh Direct Ed25519 signature did not have the exact 64-byte width")
        })?,
        intent,
    })
}

/// Read one 64-byte Solana CLI keypair file and return the 32-byte seed.
///
/// The declared public half is checked against what the secret half expands to,
/// so a damaged file is a refusal here rather than an address nobody controls.
///
/// # Errors
///
/// If the path is relative, unreadable, not a JSON byte array, not 64 bytes, or
/// internally inconsistent.
pub fn keypair_seed_from_file_v1(path: &Path, label: &str) -> Result<[u8; 32]> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} keypair path must be absolute")));
    }
    let bytes: Vec<u8> = serde_json::from_slice(&fs::read(path).map_err(|error| {
        Error::new(format!("read {label} keypair {}: {error}", path.display()))
    })?)
    .map_err(|error| {
        Error::new(format!(
            "{label} keypair {} is not a JSON byte array: {error}",
            path.display()
        ))
    })?;
    if bytes.len() != 64 {
        return Err(Error::new(format!(
            "{label} keypair {} holds {} bytes; a Solana CLI keypair file is 64 (32-byte secret \
             seed then its 32-byte public key)",
            path.display(),
            bytes.len()
        )));
    }
    // Split by value rather than by `copy_from_slice` on a `get(..).unwrap_or`:
    // the width was checked above, but a slice copy whose panic is prevented by
    // a check thirty lines away is a panic waiting for someone to move the
    // check. `try_into` carries its own proof.
    let (secret, declared): ([u8; 32], [u8; 32]) = match (bytes.get(..32), bytes.get(32..)) {
        (Some(secret), Some(declared)) => (
            secret
                .try_into()
                .map_err(|_| Error::new("keypair secret half was not 32 bytes"))?,
            declared
                .try_into()
                .map_err(|_| Error::new("keypair public half was not 32 bytes"))?,
        ),
        _ => return Err(Error::new("keypair file could not be split into halves")),
    };
    let derived = Keypair::new_from_array(secret);
    if derived.pubkey().to_bytes() != declared {
        return Err(Error::new(format!(
            "{label} keypair {} is inconsistent: the public key it declares is not the one its \
             secret seed expands to. This file is damaged; do not fund the address it prints.",
            path.display()
        )));
    }
    Ok(secret)
}

/// Resolve the keypair path out of the caller's environment.
///
/// The variable NAME is an argument; the path is not, and neither ever appears
/// in a receipt. A refusal here names the variable the caller chose, because
/// that is the only thing they can act on that is already theirs.
///
/// Public because this discipline is not the ticket author's private habit. It
/// is what the project does with any key whose exposure is not recoverable, and
/// the successor-declaration caller needs it for the retained Loader deployer --
/// the one key that can strand every market in the protocol. A second copy of
/// these four refusals would be a second place for them to drift.
///
/// # Errors
///
/// If `variable` is not an uppercase environment-variable name, or the variable
/// is unset, empty, or does not hold an absolute path.
pub fn keypair_path_from_environment_v1(variable: &str) -> Result<PathBuf> {
    if !is_environment_variable_name_v1(variable) {
        return Err(refusal(
            "--keypair-env must name one uppercase environment variable, not a path",
        ));
    }
    let value = env::var(variable)
        .map_err(|_| refusal(format!("${variable} is unset or not valid Unicode")))?;
    if value.is_empty() {
        return Err(refusal(format!("${variable} is empty")));
    }
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(refusal(format!(
            "${variable} does not hold an absolute keypair path"
        )));
    }
    Ok(path)
}

fn is_environment_variable_name_v1(variable: &str) -> bool {
    !variable.is_empty()
        && variable
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Parse the author's arguments, refusing every key-material flag at parse.
///
/// # Errors
///
/// On an unknown flag, a repeated flag, a missing required flag, a
/// non-canonical value, an empty slot interval, a zero fill or price, the
/// default address in an identity, a relative `--out`, or any flag that would
/// have carried a key path.
pub fn parse_arguments_v1(
    invocation: &str,
    arguments: Vec<String>,
) -> Result<DirectTicketAuthorArgumentsV1> {
    let mut keypair_env = None;
    let mut maker = None;
    let mut market = None;
    let mut collateral_account = None;
    let mut side = None;
    let mut lifecycle = None;
    let mut outcome = None;
    let mut generation = None;
    let mut nonce = None;
    let mut valid_from = None;
    let mut valid_through = None;
    let mut maximum_fill = None;
    let mut limit_price = None;
    let mut fee_basis_points = None;
    let mut out = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--keypair-env" => &mut keypair_env,
            "--maker" => &mut maker,
            "--market" => &mut market,
            "--collateral-account" => &mut collateral_account,
            "--side" => &mut side,
            "--lifecycle" => &mut lifecycle,
            "--outcome" => &mut outcome,
            "--generation" => &mut generation,
            "--nonce" => &mut nonce,
            "--valid-from" => &mut valid_from,
            "--valid-through" => &mut valid_through,
            "--maximum-fill" => &mut maximum_fill,
            "--limit-price" => &mut limit_price,
            "--fee-basis-points" => &mut fee_basis_points,
            "--out" => &mut out,
            "--keypair" | "--keypair-path" | "--secret-key" => {
                return Err(refusal(format!(
                    "{argument} is refused: pass --keypair-env NAME so the path never reaches the \
                     command line or the process table"
                )));
            }
            _ => {
                return Err(Error::new(format!(
                    "unknown ticket author argument: {argument}\n{}",
                    usage_v1(invocation)
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} was given twice")));
        }
    }
    let required = |value: Option<String>, flag: &str| -> Result<String> {
        value.ok_or_else(|| Error::new(format!("{flag} is required\n{}", usage_v1(invocation))))
    };
    // Checked here rather than at read time so a caller who typed a path into
    // the flag learns it at parse, before the value has travelled any further.
    let keypair_env = required(keypair_env, "--keypair-env")?;
    if !is_environment_variable_name_v1(&keypair_env) {
        return Err(refusal(
            "--keypair-env must name one uppercase environment variable, not a path",
        ));
    }
    let side = match required(side, "--side")?.as_str() {
        "sell" => 0_u8,
        "buy" => 1_u8,
        other => {
            return Err(refusal(format!(
                "--side must be sell or buy, not {other}; sell is the 0 the intent encodes"
            )));
        }
    };
    let lifecycle = match required(lifecycle, "--lifecycle")?.as_str() {
        "fok" => 0_u8,
        "ioc" => 1_u8,
        other => {
            return Err(refusal(format!(
                "--lifecycle must be fok or ioc, not {other}; the inline route admits no other"
            )));
        }
    };
    let maker = canonical_ticket_pubkey_v1(&required(maker, "--maker")?, "--maker")?;
    let market = canonical_ticket_pubkey_v1(&required(market, "--market")?, "--market")?;
    let collateral_account = canonical_ticket_pubkey_v1(
        &required(collateral_account, "--collateral-account")?,
        "--collateral-account",
    )?;
    let outcome = canonical_ticket_u64_v1(&required(outcome, "--outcome")?, "--outcome")?;
    let outcome = u32::try_from(outcome)
        .map_err(|_| refusal("--outcome is outside the runtime u32 coordinate"))?;
    let fee_basis_points = canonical_ticket_u64_v1(
        &required(fee_basis_points, "--fee-basis-points")?,
        "--fee-basis-points",
    )?;
    let fee_basis_points = u16::try_from(fee_basis_points)
        .ok()
        .filter(|value| *value <= 10_000)
        .ok_or_else(|| refusal("--fee-basis-points is outside 0..10000"))?;
    let valid_from =
        canonical_ticket_u64_v1(&required(valid_from, "--valid-from")?, "--valid-from")?;
    let valid_through = canonical_ticket_u64_v1(
        &required(valid_through, "--valid-through")?,
        "--valid-through",
    )?;
    let maximum_fill =
        canonical_ticket_u64_v1(&required(maximum_fill, "--maximum-fill")?, "--maximum-fill")?;
    let limit_price =
        canonical_ticket_u64_v1(&required(limit_price, "--limit-price")?, "--limit-price")?;
    if valid_from > valid_through {
        return Err(refusal(
            "--valid-from is after --valid-through; the signed slot interval would be empty",
        ));
    }
    if maximum_fill == 0 || limit_price == 0 {
        return Err(refusal(
            "--maximum-fill and --limit-price must both be positive; a zero of either is not a \
             tradeable intent",
        ));
    }
    if maker == Pubkey::default()
        || market == Pubkey::default()
        || collateral_account == Pubkey::default()
    {
        return Err(refusal(
            "--maker, --market, and --collateral-account must be real identities, not the default \
             address",
        ));
    }
    let out = PathBuf::from(required(out, "--out")?);
    if !out.is_absolute() {
        return Err(refusal("--out must be an absolute path"));
    }
    Ok(DirectTicketAuthorArgumentsV1 {
        keypair_env,
        maker,
        market,
        collateral_account,
        side,
        lifecycle,
        outcome,
        generation: canonical_ticket_u64_v1(
            &required(generation, "--generation")?,
            "--generation",
        )?,
        nonce: canonical_ticket_u64_v1(&required(nonce, "--nonce")?, "--nonce")?,
        valid_from,
        valid_through,
        maximum_fill,
        limit_price,
        fee_basis_points,
        out,
    })
}

fn write_create_new_v1(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
