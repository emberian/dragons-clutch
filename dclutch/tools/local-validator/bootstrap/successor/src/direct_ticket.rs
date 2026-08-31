//! The portable Direct intent ticket: one author, one reader, one shape.
//!
//! A Direct inline fill settles two independently signed intents. Until this
//! module existed the only code in the repository that could WRITE one was the
//! browser trade panel (`apps/dclutch-web/components/MarketTradePanel.tsx`
//! through `encodeDirectIntentTicketV1`), and every tool could only read one --
//! so `devnet-direct-trade-produce-v1` demanded `--seller-ticket` and
//! `--buyer-ticket` as inputs nothing under `tools/` could produce.
//!
//! WHO OWNS WHAT, so nothing here is an inferred layout:
//!
//! - The SIGNED MESSAGE is owned by `dclutch_direct_codec::intent_v2`, emitted
//!   from `formal/dclutch-semantics/EmitDirectIntentV2Rust.lean`. This module
//!   calls `CompactIntentV2::signed_preimage()`; it does not lay out a byte.
//!   The TypeScript side reads the same emitted constants through
//!   `packages/dclutch-sdk/lib/generated/directInlineV3.ts`.
//! - The JSON ENVELOPE is owned by [`PortableDirectTicketV1`] below. Its serde
//!   field order IS the wire order, and `encode_portable_direct_ticket_v1` and
//!   `parse_portable_direct_ticket_v1` are the only writer and the only reader
//!   in this workspace. The panel's `encodeDirectIntentTicketV1` is the other
//!   producer, in the other language, and the two are pinned byte-for-byte
//!   against `packages/dclutch-sdk/fixtures/direct-intent-ticket.json` by the
//!   test at the bottom of this file and by `directTicket.test.ts` on both
//!   TypeScript sides.
//!
//! WHAT THIS MODULE REFUSES TO DO: read chain state, guess a nonce, guess a
//! slot window, or take the maker on faith. Every field the signature binds is
//! a required argument, and `--maker` states which identity the caller BELIEVES
//! the key file holds -- a keypair that expands to anything else is refused
//! before the file's contents reach a signature. The producer re-checks every
//! one of these fields against finalized chain state and refuses on any
//! mismatch, so a wrong argument here is a refused trade, never a different
//! trade.
//!
//! THE KEY PATH IS NEVER AN ARGUMENT. `--keypair-env` names an ENVIRONMENT
//! VARIABLE that holds the absolute path. Nothing about the path or the key
//! reaches the command line, the process table, the receipt, or a refusal
//! message. This is the credential lesson of 2026-08-30 written into a
//! signature.

use std::{
    env, fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use dclutch_direct_codec::intent_v2::CompactIntentV2;
use dclutch_operator::direct_inline_v3::SignedDirectIntentV3;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer as _},
};

use crate::{
    Error, Result, campaign, direct_trade_producer::decode_hex_v1, plan::pubkey,
    rpc::parse_json_without_duplicate_keys_v1,
};

pub(crate) const PORTABLE_DIRECT_TICKET_KIND_V1: &str = "dclutch/direct-intent-ticket/v1";
pub(crate) const DIRECT_TICKET_AUTHOR_COMMAND_V1: &str = "direct-intent-ticket-author-v1";
const DIRECT_TICKET_AUTHOR_RECEIPT_SCHEMA_V1: &str =
    "dclutch-direct-intent-ticket-author-receipt-v1";
/// The same explicit bound `decodeDirectIntentTicketV1` and the producer hold.
const MAXIMUM_TICKET_BYTES_V1: usize = 4_096;

/// The intent half of the portable ticket.
///
/// Field ORDER is the wire: `serde_json::to_string_pretty` emits declaration
/// order and `JSON.stringify(value, null, 2)` emits insertion order, so this
/// declaration and the object literal in `packages/dclutch-sdk/lib/directTicket.ts`
/// have to agree line for line. The vector test proves they do.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PortableDirectTicketIntentV1 {
    pub(crate) side: u8,
    pub(crate) lifecycle: u8,
    pub(crate) outcome: u32,
    pub(crate) market: String,
    pub(crate) generation: String,
    pub(crate) nonce: String,
    pub(crate) valid_from: String,
    pub(crate) valid_through: String,
    pub(crate) maximum_fill: String,
    pub(crate) limit_price: String,
    pub(crate) fee_basis_points: u16,
    pub(crate) collateral_account: String,
}

/// The portable ticket: the maker, their detached signature, and every field
/// that signature covers. Nothing else -- a ticket carries no claim the
/// signature does not already bind.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableDirectTicketV1 {
    pub(crate) kind: String,
    pub(crate) maker: String,
    pub(crate) signature: String,
    pub(crate) intent: PortableDirectTicketIntentV1,
}

/// Emit the exact bytes `encodeDirectIntentTicketV1` emits for the same signed
/// intent -- two-space pretty JSON, no trailing newline, declaration order.
///
/// The trailing newline is deliberately absent. `JSON.stringify(value, null, 2)`
/// does not emit one, and a ticket that differs from the browser's by a single
/// byte is not the thing this module claims to produce.
pub(crate) fn encode_portable_direct_ticket_v1(signed: &SignedDirectIntentV3) -> Result<String> {
    if signed.signature.iter().all(|byte| *byte == 0) {
        return Err(refusal(
            "a ticket requires one nonzero 64-byte detached signature",
        ));
    }
    let ticket = PortableDirectTicketV1 {
        kind: PORTABLE_DIRECT_TICKET_KIND_V1.into(),
        maker: signed.maker.to_string(),
        signature: hex_lower_v1(&signed.signature),
        intent: PortableDirectTicketIntentV1 {
            side: signed.intent.side,
            lifecycle: signed.intent.lifecycle,
            outcome: signed.intent.outcome,
            market: Pubkey::new_from_array(signed.intent.market).to_string(),
            generation: signed.intent.generation.to_string(),
            nonce: signed.intent.nonce.to_string(),
            valid_from: signed.intent.valid_from.to_string(),
            valid_through: signed.intent.valid_through.to_string(),
            maximum_fill: signed.intent.maximum_fill.to_string(),
            limit_price: signed.intent.limit_price.to_string(),
            fee_basis_points: signed.intent.fee_basis_points,
            collateral_account: Pubkey::new_from_array(signed.intent.collateral_account)
                .to_string(),
        },
    };
    let text = serde_json::to_string_pretty(&ticket)?;
    if text.len() > MAXIMUM_TICKET_BYTES_V1 {
        return Err(refusal(
            "authored ticket exceeded the explicit 4096-byte portable bound",
        ));
    }
    Ok(text)
}

/// Hostile-read one portable ticket back into the signed intent it carries.
///
/// The signature is verified here against the preimage this reader rebuilds,
/// so a tampered field is a refusal at parse time rather than a refusal at the
/// Ed25519 program much later.
pub(crate) fn parse_portable_direct_ticket_v1(
    bytes: &[u8],
    label: &str,
) -> Result<SignedDirectIntentV3> {
    if bytes.is_empty() || bytes.len() > MAXIMUM_TICKET_BYTES_V1 {
        return Err(refusal(format!(
            "{label} Direct ticket is outside the 1..4096 byte bound"
        )));
    }
    let value = parse_json_without_duplicate_keys_v1(bytes)
        .map_err(|error| Error::new(format!("{label} Direct ticket {error}")))?;
    let ticket: PortableDirectTicketV1 = serde_json::from_value(value.clone())
        .map_err(|error| Error::new(format!("{label} Direct ticket shape: {error}")))?;
    if serde_json::to_value(&ticket)? != value || ticket.kind != PORTABLE_DIRECT_TICKET_KIND_V1 {
        return Err(refusal(format!(
            "{label} Direct ticket kind, field set, or canonical JSON values changed"
        )));
    }
    let maker = canonical_ticket_pubkey_v1(&ticket.maker, &format!("{label} maker"))?;
    let market = canonical_ticket_pubkey_v1(&ticket.intent.market, &format!("{label} Market"))?;
    let collateral = canonical_ticket_pubkey_v1(
        &ticket.intent.collateral_account,
        &format!("{label} collateral account"),
    )?;
    if maker == Pubkey::default()
        || market == Pubkey::default()
        || collateral == Pubkey::default()
        || ticket.intent.side > 1
        || ticket.intent.lifecycle > 1
        || ticket.intent.fee_basis_points > 10_000
    {
        return Err(refusal(format!(
            "{label} Direct ticket has an invalid identity, enum, or fee width"
        )));
    }
    let signature_bytes = decode_hex_v1(&ticket.signature, &format!("{label} signature"))?;
    let signature: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| refusal(format!("{label} signature is not exactly 64 bytes")))?;
    if signature.iter().all(|byte| *byte == 0) {
        return Err(refusal(format!("{label} signature is all zero")));
    }
    let intent = CompactIntentV2 {
        side: ticket.intent.side,
        lifecycle: ticket.intent.lifecycle,
        outcome: ticket.intent.outcome,
        market: market.to_bytes(),
        generation: canonical_ticket_u64_v1(&ticket.intent.generation, "generation")?,
        nonce: canonical_ticket_u64_v1(&ticket.intent.nonce, "nonce")?,
        valid_from: canonical_ticket_u64_v1(&ticket.intent.valid_from, "validFrom")?,
        valid_through: canonical_ticket_u64_v1(&ticket.intent.valid_through, "validThrough")?,
        maximum_fill: canonical_ticket_u64_v1(&ticket.intent.maximum_fill, "maximumFill")?,
        limit_price: canonical_ticket_u64_v1(&ticket.intent.limit_price, "limitPrice")?,
        fee_basis_points: ticket.intent.fee_basis_points,
        collateral_account: collateral.to_bytes(),
    };
    let encoded = intent
        .encode()
        .map_err(|error| Error::new(format!("{label} Direct intent encode: {error:?}")))?;
    if CompactIntentV2::decode(&encoded)
        .map_err(|error| Error::new(format!("{label} Direct intent decode: {error:?}")))?
        != intent
    {
        return Err(refusal(format!(
            "{label} Direct ticket intent failed canonical codec roundtrip"
        )));
    }
    let preimage = intent
        .signed_preimage()
        .map_err(|error| Error::new(format!("{label} Direct signed preimage: {error:?}")))?;
    if !Signature::from(signature).verify(maker.as_ref(), &preimage) {
        return Err(refusal(format!(
            "{label} Direct ticket detached signature did not verify"
        )));
    }
    Ok(SignedDirectIntentV3 {
        maker,
        signature,
        intent,
    })
}

/// Sign one exact intent with the standard `solana-sdk` Ed25519 signer.
///
/// The freshly produced signature is verified against the same preimage before
/// it is handed back, so a signer that silently produced the wrong bytes is a
/// refusal here and not a refused transaction someone debugs later.
pub(crate) fn sign_direct_intent_v1(
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct DirectTicketAuthorArgumentsV1 {
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
pub(crate) struct DirectTicketAuthorReceiptV1 {
    pub(crate) schema: String,
    pub(crate) ticket: String,
    pub(crate) ticket_sha256: String,
    pub(crate) ticket_bytes: usize,
    pub(crate) maker: String,
    pub(crate) market: String,
    pub(crate) side: String,
    pub(crate) lifecycle: String,
    pub(crate) outcome: u32,
    pub(crate) generation: u64,
    pub(crate) nonce: u64,
    pub(crate) valid_from: u64,
    pub(crate) valid_through: u64,
    pub(crate) maximum_fill: u64,
    pub(crate) limit_price: u64,
    pub(crate) fee_basis_points: u16,
    pub(crate) collateral_account: String,
    pub(crate) signed_preimage_bytes: usize,
    pub(crate) signature_domain: String,
}

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap direct-intent-ticket-author-v1 \
     --keypair-env ENVIRONMENT_VARIABLE_HOLDING_THE_ABSOLUTE_KEYPAIR_PATH \
     --maker PUBKEY --market PUBKEY --side sell|buy --lifecycle fok|ioc \
     --outcome U32 --generation U64 --nonce U64 --valid-from SLOT \
     --valid-through SLOT --maximum-fill ATOMS --limit-price SCALED \
     --fee-basis-points BPS --collateral-account PUBKEY \
     --out ABSOLUTE_JSON_THAT_DOES_NOT_EXIST"
}

/// CLI integration hook. `main.rs` only needs to dispatch arguments here.
pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let receipt = author_direct_intent_ticket_v1(parse_arguments_v1(arguments)?)?;
    let mut stdout = std::io::stdout();
    serde_json::to_writer_pretty(&mut stdout, &receipt)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn author_direct_intent_ticket_v1(
    arguments: DirectTicketAuthorArgumentsV1,
) -> Result<DirectTicketAuthorReceiptV1> {
    let keypair_path = keypair_path_from_environment_v1(&arguments.keypair_env)?;
    author_with_keypair_path_v1(arguments, &keypair_path)
}

/// The author with the key path already resolved.
///
/// Split out from [`author_direct_intent_ticket_v1`] so the tests can exercise
/// the whole authoring path without an environment variable: `env::set_var` is
/// `unsafe` under edition 2024 and this crate forbids `unsafe_code`, so a test
/// that wanted one would have had to weaken the crate to get it.
fn author_with_keypair_path_v1(
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
    let keypair = Keypair::new_from_array(campaign::read_keypair_file(
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
        ticket_sha256: sha256_hex(text.as_bytes()),
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

/// Resolve the keypair path out of the caller's environment.
///
/// The variable NAME is an argument; the path is not, and neither ever appears
/// in a receipt. A refusal here names the variable the caller chose, because
/// that is the only thing they can act on that is already theirs.
fn keypair_path_from_environment_v1(variable: &str) -> Result<PathBuf> {
    if variable.is_empty()
        || !variable
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
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

#[allow(clippy::too_many_lines)]
fn parse_arguments_v1(arguments: Vec<String>) -> Result<DirectTicketAuthorArgumentsV1> {
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
                    usage()
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} was given twice")));
        }
    }
    let required = |value: Option<String>, flag: &str| -> Result<String> {
        value.ok_or_else(|| Error::new(format!("{flag} is required\n{}", usage())))
    };
    // Checked here rather than at read time so a caller who typed a path into
    // the flag learns it at parse, before the value has travelled any further.
    let keypair_env = required(keypair_env, "--keypair-env")?;
    if keypair_env.is_empty()
        || !keypair_env
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
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

pub(crate) fn canonical_ticket_pubkey_v1(value: &str, label: &str) -> Result<Pubkey> {
    let key = pubkey(value)?;
    if key.to_string() != value {
        return Err(refusal(format!("{label} is not canonical base58 text")));
    }
    Ok(key)
}

pub(crate) fn canonical_ticket_u64_v1(value: &str, label: &str) -> Result<u64> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| refusal(format!("Direct ticket {label} is not a u64")))?;
    if parsed.to_string() != value {
        return Err(refusal(format!(
            "Direct ticket {label} is not canonical decimal text"
        )));
    }
    Ok(parsed)
}

fn hex_lower_v1(bytes: &[u8]) -> String {
    use core::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_lower_v1(&Sha256::digest(bytes))
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

fn refusal(reason: impl AsRef<str>) -> Error {
    Error::new(format!("REFUSED: {}", reason.as_ref()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use dclutch_direct_codec::intent_v2::CompactIntentV2;
    use serde::Deserialize;
    use solana_sdk::{
        pubkey::Pubkey,
        signature::{Keypair, Signer as _},
    };

    use super::{
        DIRECT_TICKET_AUTHOR_COMMAND_V1, encode_portable_direct_ticket_v1, parse_arguments_v1,
        parse_portable_direct_ticket_v1, sign_direct_intent_v1, usage,
    };

    /// The two-sided ticket vector.
    ///
    /// Emitted by `packages/dclutch-sdk/scripts/generate-direct-intent-ticket-vector.mjs`
    /// through the SAME `encodeDirectIntentTicketV1` the browser trade panel
    /// calls, and reproduced here by the Rust author. TypeScript is the
    /// incumbent producer -- the browser has been the only ticket writer -- so
    /// TypeScript emits and Rust matches. If this test goes red, the Rust
    /// author has drifted off the wire the panel already puts on chain.
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct TicketVectorV1 {
        format: String,
        #[allow(dead_code)]
        note: String,
        maker_seed_fill: u8,
        market_seed_fill: u8,
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
            .join("../../../../packages/dclutch-sdk/fixtures/direct-intent-ticket.json");
        serde_json::from_slice(&std::fs::read(&path).expect("ticket vector fixture"))
            .expect("ticket vector shape")
    }

    /// The whole point of this module: the CLI-authored ticket is the panel's
    /// ticket, byte for byte, including the signature -- which is the same
    /// assertion as "both languages built the same 172-byte signing message".
    #[test]
    fn authored_ticket_is_byte_identical_to_the_browser_panel_wire() -> crate::Result<()> {
        let vector = vector();
        assert_eq!(vector.format, "dclutch/direct-intent-ticket-vector/v1");
        let maker_key = Keypair::new_from_array([vector.maker_seed_fill; 32]);
        let market = Pubkey::new_from_array(
            Keypair::new_from_array([vector.market_seed_fill; 32])
                .pubkey()
                .to_bytes(),
        );
        let collateral = Keypair::new_from_array([vector.collateral_seed_fill; 32]).pubkey();
        assert_eq!(maker_key.pubkey().to_string(), vector.maker);
        assert_eq!(market.to_string(), vector.market);
        assert_eq!(collateral.to_string(), vector.collateral_account);
        let signed = sign_direct_intent_v1(
            &maker_key,
            CompactIntentV2 {
                side: vector.side,
                lifecycle: vector.lifecycle,
                outcome: vector.outcome,
                market: market.to_bytes(),
                generation: vector.generation,
                nonce: vector.nonce,
                valid_from: vector.valid_from,
                valid_through: vector.valid_through,
                maximum_fill: vector.maximum_fill,
                limit_price: vector.limit_price,
                fee_basis_points: vector.fee_basis_points,
                collateral_account: collateral.to_bytes(),
            },
        )?;
        assert_eq!(super::hex_lower_v1(&signed.signature), vector.signature_hex);
        let text = encode_portable_direct_ticket_v1(&signed)?;
        assert_eq!(text, vector.ticket_text);
        assert_eq!(super::sha256_hex(text.as_bytes()), vector.ticket_sha256);
        assert!(!text.ends_with('\n'), "the panel emits no trailing newline");
        assert_eq!(
            parse_portable_direct_ticket_v1(text.as_bytes(), "vector")?,
            signed
        );
        Ok(())
    }

    #[test]
    fn every_authored_field_survives_the_hostile_reader() -> crate::Result<()> {
        let key = Keypair::new();
        let signed = sign_direct_intent_v1(
            &key,
            CompactIntentV2 {
                side: 1,
                lifecycle: 1,
                outcome: 4_294_967_294,
                market: Pubkey::new_unique().to_bytes(),
                generation: u64::MAX - 3,
                nonce: 0,
                valid_from: 1,
                valid_through: u64::MAX,
                maximum_fill: u64::MAX,
                limit_price: 1,
                fee_basis_points: 10_000,
                collateral_account: Pubkey::new_unique().to_bytes(),
            },
        )?;
        let text = encode_portable_direct_ticket_v1(&signed)?;
        assert_eq!(
            parse_portable_direct_ticket_v1(text.as_bytes(), "wide")?,
            signed
        );
        Ok(())
    }

    #[test]
    fn one_flipped_field_dies_at_the_signature_and_not_at_the_chain() -> crate::Result<()> {
        let key = Keypair::new();
        let signed = sign_direct_intent_v1(
            &key,
            CompactIntentV2 {
                side: 0,
                lifecycle: 0,
                outcome: 2,
                market: Pubkey::new_unique().to_bytes(),
                generation: 9,
                nonce: 5,
                valid_from: 100,
                valid_through: 200,
                maximum_fill: 1_000,
                limit_price: 500_000,
                fee_basis_points: 50,
                collateral_account: Pubkey::new_unique().to_bytes(),
            },
        )?;
        let text = encode_portable_direct_ticket_v1(&signed)?;
        let tampered = text.replace("\"maximumFill\": \"1000\"", "\"maximumFill\": \"1001\"");
        assert_ne!(tampered, text, "the tamper did not apply");
        let error = parse_portable_direct_ticket_v1(tampered.as_bytes(), "tampered")
            .expect_err("a changed field must not reopen");
        assert!(
            format!("{error}").contains("signature did not verify"),
            "unexpected refusal: {error}"
        );
        Ok(())
    }

    /// One 64-byte Solana CLI keypair file: the 32-byte seed then the 32-byte
    /// public key it expands to, exactly as `solana-keygen` writes it.
    fn write_keypair_file_v1(directory: &std::path::Path, name: &str, seed: [u8; 32]) -> PathBuf {
        let keypair = Keypair::new_from_array(seed);
        let mut bytes = seed.to_vec();
        bytes.extend_from_slice(&keypair.pubkey().to_bytes());
        let path = directory.join(name);
        std::fs::write(&path, serde_json::to_vec(&bytes).expect("keypair json")).expect("write");
        path
    }

    fn scratch_directory_v1(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("dclutch-ticketcli-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch directory");
        path
    }

    /// THE POINT OF THE SUBCOMMAND, end to end and offline.
    ///
    /// Two keypair files on disk, two authoring runs through the real argument
    /// parser, and the resulting pair is then put through the EXACT two
    /// functions `devnet-direct-trade-produce-v1` runs on `--seller-ticket` and
    /// `--buyer-ticket` before it opens a socket: `parse_portable_direct_ticket_v1`
    /// and `exact_ticket_pair_terms_v1`. What that proves is the ticket
    /// admission of the produce path, not the fill -- the fill needs a chain,
    /// a founded market, and an activated Direct root.
    #[test]
    fn a_cli_authored_pair_passes_the_producer_ticket_admission() -> crate::Result<()> {
        let scratch = scratch_directory_v1("pair");
        let market = Keypair::new_from_array([0x21; 32]).pubkey();
        let seller_key = write_keypair_file_v1(&scratch, "seller.json", [0x31; 32]);
        let buyer_key = write_keypair_file_v1(&scratch, "buyer.json", [0x32; 32]);
        let seller_maker = Keypair::new_from_array([0x31; 32]).pubkey();
        let buyer_maker = Keypair::new_from_array([0x32; 32]).pubkey();
        let seller_collateral = Keypair::new_from_array([0x41; 32]).pubkey();
        let buyer_collateral = Keypair::new_from_array([0x42; 32]).pubkey();

        let author = |name: &str,
                      key: &PathBuf,
                      maker: Pubkey,
                      side: &str,
                      nonce: &str,
                      collateral: Pubkey|
         -> crate::Result<(PathBuf, super::DirectTicketAuthorReceiptV1)> {
            let out = scratch.join(name);
            let arguments = parse_arguments_v1(
                [
                    ("--keypair-env", "DCLUTCH_TICKET_KEYPAIR"),
                    ("--maker", &maker.to_string()),
                    ("--market", &market.to_string()),
                    ("--collateral-account", &collateral.to_string()),
                    ("--side", side),
                    ("--lifecycle", "fok"),
                    ("--outcome", "3"),
                    ("--generation", "2"),
                    ("--nonce", nonce),
                    ("--valid-from", "11"),
                    ("--valid-through", "432011"),
                    ("--maximum-fill", "100000000"),
                    ("--limit-price", "500000"),
                    ("--fee-basis-points", "50"),
                    ("--out", &out.display().to_string()),
                ]
                .into_iter()
                .flat_map(|(flag, value)| [flag.to_string(), value.to_string()])
                .collect(),
            )?;
            let receipt = super::author_with_keypair_path_v1(arguments, key)?;
            Ok((out, receipt))
        };

        let (seller_path, seller_receipt) = author(
            "seller-ticket.json",
            &seller_key,
            seller_maker,
            "sell",
            "0",
            seller_collateral,
        )?;
        let (buyer_path, buyer_receipt) = author(
            "buyer-ticket.json",
            &buyer_key,
            buyer_maker,
            "buy",
            "0",
            buyer_collateral,
        )?;

        // The receipt hands the operator exactly the next two arguments and no
        // path to anything secret.
        for receipt in [&seller_receipt, &buyer_receipt] {
            assert_eq!(receipt.ticket_sha256.len(), 64);
            assert_eq!(receipt.signed_preimage_bytes, 172);
            assert_eq!(
                receipt.signature_domain,
                "dclutch/signature/direct-compact-intent-v2"
            );
            let rendered = serde_json::to_string(receipt).expect("receipt json");
            assert!(
                !rendered.contains("keypair"),
                "receipt named a key: {rendered}"
            );
            // The ticket the caller asked for is the ONLY path in the receipt.
            // Anything else here would be a filesystem fact the operator did
            // not request, and the key files live in the same directory.
            assert_eq!(
                rendered
                    .matches(scratch.to_str().expect("utf8 scratch"))
                    .count(),
                1,
                "receipt carried a path beyond the ticket it wrote: {rendered}"
            );
            assert!(receipt.ticket.ends_with("-ticket.json"));
        }
        assert_eq!(seller_receipt.side, "sell");
        assert_eq!(buyer_receipt.side, "buy");

        let seller_bytes = std::fs::read(&seller_path)?;
        let buyer_bytes = std::fs::read(&buyer_path)?;
        assert_eq!(
            super::sha256_hex(&seller_bytes),
            seller_receipt.ticket_sha256,
            "the receipt digest is not the digest of the file on disk"
        );

        // The producer's own two gates, unmodified.
        let seller = parse_portable_direct_ticket_v1(&seller_bytes, "seller")?;
        let buyer = parse_portable_direct_ticket_v1(&buyer_bytes, "buyer")?;
        let terms = crate::direct_trade_producer::exact_ticket_pair_terms_v1(&seller, &buyer)?;
        assert_eq!(terms.outcome, 3);
        assert_eq!(terms.fill, 100_000_000);
        assert_eq!(terms.execution_price, 500_000);
        assert_eq!(terms.fee_basis_points, 50);

        // A second write to the same path is refused, so a rerun cannot
        // silently replace a ticket an operator already quoted a digest for.
        assert!(
            author(
                "seller-ticket.json",
                &seller_key,
                seller_maker,
                "sell",
                "0",
                seller_collateral
            )
            .is_err()
        );
        let _ = std::fs::remove_dir_all(&scratch);
        Ok(())
    }

    #[test]
    fn a_key_that_is_not_the_stated_maker_never_signs() -> crate::Result<()> {
        let scratch = scratch_directory_v1("wrong-maker");
        let key = write_keypair_file_v1(&scratch, "someone-else.json", [0x51; 32]);
        let stated = Keypair::new_from_array([0x52; 32]).pubkey();
        let market = Keypair::new_from_array([0x53; 32]).pubkey();
        let collateral = Keypair::new_from_array([0x54; 32]).pubkey();
        let out = scratch.join("refused.json");
        let arguments = parse_arguments_v1(
            [
                ("--keypair-env", "DCLUTCH_TICKET_KEYPAIR"),
                ("--maker", &stated.to_string()),
                ("--market", &market.to_string()),
                ("--collateral-account", &collateral.to_string()),
                ("--side", "sell"),
                ("--lifecycle", "fok"),
                ("--outcome", "0"),
                ("--generation", "1"),
                ("--nonce", "0"),
                ("--valid-from", "1"),
                ("--valid-through", "2"),
                ("--maximum-fill", "1"),
                ("--limit-price", "1"),
                ("--fee-basis-points", "0"),
                ("--out", &out.display().to_string()),
            ]
            .into_iter()
            .flat_map(|(flag, value)| [flag.to_string(), value.to_string()])
            .collect(),
        )?;
        let error = super::author_with_keypair_path_v1(arguments, &key)
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
        Ok(())
    }

    #[test]
    fn an_unset_variable_is_a_refusal_that_names_only_the_variable() {
        let error = super::keypair_path_from_environment_v1("DCLUTCH_TICKET_KEYPAIR_ABSENT_V1")
            .expect_err("an unset variable must be refused");
        let rendered = format!("{error}");
        assert!(rendered.starts_with("REFUSED:"));
        assert!(rendered.contains("DCLUTCH_TICKET_KEYPAIR_ABSENT_V1"));
        assert!(
            !rendered.contains('/'),
            "the refusal invented a path: {rendered}"
        );
    }

    #[test]
    fn the_key_path_is_never_an_argument() {
        let usage = usage();
        assert!(usage.starts_with(&format!(
            "dclutch-local-successor-bootstrap {DIRECT_TICKET_AUTHOR_COMMAND_V1} "
        )));
        assert!(usage.contains("--keypair-env"));
        for forbidden in ["--keypair ", "--keypair-path", "--secret-key", "--seed"] {
            assert!(!usage.contains(forbidden), "usage exposed {forbidden}");
        }
        for refused in ["--keypair", "--keypair-path", "--secret-key"] {
            let error = parse_arguments_v1(vec![refused.into(), "/tmp/anything.json".into()])
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
    }

    #[test]
    fn noncanonical_arguments_are_refused_before_a_key_is_opened() {
        let base: Vec<(&str, &str)> = vec![
            ("--keypair-env", "DCLUTCH_TICKET_KEYPAIR"),
            ("--maker", "11111111111111111111111111111112"),
            ("--market", "11111111111111111111111111111113"),
            ("--collateral-account", "11111111111111111111111111111114"),
            ("--side", "sell"),
            ("--lifecycle", "fok"),
            ("--outcome", "0"),
            ("--generation", "1"),
            ("--nonce", "0"),
            ("--valid-from", "10"),
            ("--valid-through", "20"),
            ("--maximum-fill", "100"),
            ("--limit-price", "500000"),
            ("--fee-basis-points", "50"),
            ("--out", "/tmp/dclutch-ticket-argument-test.json"),
        ];
        let with = |flag: &str, value: &str| -> Vec<String> {
            base.iter()
                .map(|(name, existing)| {
                    (
                        (*name).to_string(),
                        if *name == flag { value } else { *existing }.to_string(),
                    )
                })
                .flat_map(|(name, value)| [name, value])
                .collect()
        };
        parse_arguments_v1(with("--side", "sell")).expect("the canonical shape parses");
        for (flag, value) in [
            ("--side", "SELL"),
            ("--lifecycle", "gtc"),
            ("--fee-basis-points", "10001"),
            ("--maximum-fill", "0"),
            ("--limit-price", "0"),
            ("--valid-from", "21"),
            ("--nonce", "007"),
            ("--outcome", "4294967296"),
            ("--maker", "11111111111111111111111111111111"),
            ("--out", "relative/path.json"),
            ("--keypair-env", "/Users/somebody/keys/founder.json"),
        ] {
            let error = parse_arguments_v1(with(flag, value))
                .expect_err(&format!("{flag}={value} must be refused"));
            assert!(
                format!("{error}").starts_with("REFUSED:"),
                "{flag}={value} produced a non-refusal: {error}"
            );
        }
    }
}
