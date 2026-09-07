//! Owned-loopback Series founder evidence.
//!
//! The Series release constructor accepts authenticated immutable records; it
//! intentionally does not invent a Template, occurrence projection, or Ticket.
//! This small command is the local producer for that boundary.  It derives a
//! two-occurrence projection from the generated wire layouts, binds the first
//! Ticket to the resulting Template and occurrence identities, and publishes
//! only public role identities.  It never loads a keypair and never creates a
//! seeded ProgramTest account.

use std::{fs::OpenOptions, io::Write as _, path::PathBuf};

use dclutch_trading::series::{
    AccountKeyV3, admit_occurrence, admit_ticket, generated, occurrence_content_id,
    template_content_id,
};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use solana_program::hash::hashv;
use solana_sdk::pubkey::Pubkey;

use crate::{Error, Result, plan::hex};

pub(crate) const SERIES_FOUNDER_INPUT_COMMAND_V1: &str =
    "local-private-validator-series-founder-input-v1";
const SERIES_FOUNDER_INPUT_SCHEMA_V1: &str = "dclutch-owned-loopback-series-founder-input-v1";

/// The immutable input a subsequent founder passes to the current-source
/// owner.  Bytes are emitted as hex so this command has no implicit file or
/// account authority.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SeriesFounderInputV1 {
    schema: String,
    template_hex: String,
    occurrence_zero_hex: String,
    occurrence_one_hex: String,
    occurrence_zero_siblings_hex: Vec<String>,
    occurrence_one_siblings_hex: Vec<String>,
    ticket_zero_hex: String,
    roles: SeriesFounderRolesV1,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SeriesFounderRolesV1 {
    founder: String,
    refund_owner: String,
    payer: String,
    collateral_mint: String,
    founder_source: String,
    escrow_vault: String,
    future_hoard_vault: String,
    rent_credit: String,
}

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-series-founder-input-v1 \\\n+     \x20   --seed HEX64 --output ABSOLUTE_NEW_JSON\n"
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let mut seed = None;
    let mut output = None;
    let mut iterator = arguments.into_iter();
    while let Some(flag) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{flag} requires a value; usage: {}", usage())))?;
        let slot = match flag.as_str() {
            "--seed" => &mut seed,
            "--output" => &mut output,
            _ => {
                return Err(Error::new(format!(
                    "unknown Series founder input flag {flag}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{flag} may be supplied only once")));
        }
    }
    let seed = decode_seed(&seed.ok_or_else(|| Error::new("--seed is required"))?)?;
    let output = PathBuf::from(output.ok_or_else(|| Error::new("--output is required"))?);
    if !output.is_absolute() || output.exists() {
        return Err(Error::new(
            "--output must be an absolute path that does not exist",
        ));
    }
    let parent = output
        .parent()
        .ok_or_else(|| Error::new("--output has no parent directory"))?;
    if !parent.is_dir() {
        return Err(Error::new("--output parent must already exist"));
    }
    let input = build_series_founder_input_v1(seed)?;
    let bytes = serde_json::to_vec_pretty(&input)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    let mut stdout = std::io::stdout();
    writeln!(stdout, "{}", output.display())?;
    Ok(())
}

/// Derive the two leaves, their one-sibling proofs, and an admitted first
/// Ticket from one disposable local seed.
pub(crate) fn build_series_founder_input_v1(seed: [u8; 32]) -> Result<SeriesFounderInputV1> {
    let role = |name| role_key(seed, name);
    let roles = SeriesFounderRolesV1 {
        founder: role("founder").to_string(),
        refund_owner: role("refund-owner").to_string(),
        payer: role("payer").to_string(),
        collateral_mint: role("collateral-mint").to_string(),
        founder_source: role("founder-source").to_string(),
        escrow_vault: role("escrow-vault").to_string(),
        future_hoard_vault: role("future-hoard-vault").to_string(),
        rent_credit: role("rent-credit").to_string(),
    };
    let refund = AccountKeyV3::new(role("refund-owner").to_bytes())
        .map_err(|_| Error::new("derived Series refund owner was zero"))?;
    let founder = role("founder").to_bytes();

    let mut occurrence_zero = generated::SERIES_EXAMPLE_OCCURRENCE_V3;
    put(
        &mut occurrence_zero,
        generated::SERIES_OCCURRENCE_INDEX_OFFSET_V3,
        &0_u32.to_le_bytes(),
    )?;
    put(
        &mut occurrence_zero,
        generated::SERIES_OCCURRENCE_SCHEDULED_SLOT_OFFSET_V3,
        &100_u64.to_le_bytes(),
    )?;
    let mut occurrence_one = generated::SERIES_EXAMPLE_OCCURRENCE_V3;
    // The generated example already carries occurrence one at its second
    // schedule.  Still write both fields so the two leaves are visibly
    // derived rather than a copied fixture pair.
    put(
        &mut occurrence_one,
        generated::SERIES_OCCURRENCE_INDEX_OFFSET_V3,
        &1_u32.to_le_bytes(),
    )?;
    put(
        &mut occurrence_one,
        generated::SERIES_OCCURRENCE_SCHEDULED_SLOT_OFFSET_V3,
        &110_u64.to_le_bytes(),
    )?;
    let zero_id = occurrence_content_id(&occurrence_zero)
        .map_err(|_| Error::new("derived occurrence zero refused decode"))?;
    let one_id = occurrence_content_id(&occurrence_one)
        .map_err(|_| Error::new("derived occurrence one refused decode"))?;
    let root = projection_node(zero_id.to_bytes(), one_id.to_bytes());

    let mut template = generated::SERIES_EXAMPLE_TEMPLATE_V3;
    put(
        &mut template,
        generated::SERIES_TEMPLATE_OCCURRENCE_COUNT_OFFSET_V3,
        &2_u32.to_le_bytes(),
    )?;
    put(
        &mut template,
        generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
        &root,
    )?;
    put(
        &mut template,
        generated::SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3,
        &refund.to_bytes(),
    )?;
    let template_id = template_content_id(&template)
        .map_err(|_| Error::new("derived two-occurrence Template refused decode"))?;

    let mut ticket_zero = generated::SERIES_EXAMPLE_TICKET_V3;
    put(
        &mut ticket_zero,
        generated::SERIES_TICKET_INDEX_OFFSET_V3,
        &0_u32.to_le_bytes(),
    )?;
    put(
        &mut ticket_zero,
        generated::SERIES_TICKET_TEMPLATE_OFFSET_V3,
        &template_id.to_bytes(),
    )?;
    put(
        &mut ticket_zero,
        generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
        &zero_id.to_bytes(),
    )?;
    let market: [u8; 32] = occurrence_zero[generated::SERIES_OCCURRENCE_MARKET_OFFSET_V3
        ..generated::SERIES_OCCURRENCE_MARKET_OFFSET_V3 + 32]
        .try_into()
        .map_err(|_| Error::new("Series occurrence Market field truncated"))?;
    put(
        &mut ticket_zero,
        generated::SERIES_TICKET_MARKET_OFFSET_V3,
        &market,
    )?;
    put(
        &mut ticket_zero,
        generated::SERIES_TICKET_FOUNDER_OFFSET_V3,
        &founder,
    )?;
    put(
        &mut ticket_zero,
        generated::SERIES_TICKET_REFUND_OWNER_OFFSET_V3,
        &refund.to_bytes(),
    )?;

    // These calls are controls over the exact same bytes the future source
    // constructor receives.  They prove both sibling directions and the
    // Ticket's Template/occurrence join before any registry publication.
    admit_occurrence(&template, &occurrence_zero, &[one_id.to_bytes()])
        .map_err(|_| Error::new("derived occurrence-zero proof refused"))?;
    admit_occurrence(&template, &occurrence_one, &[zero_id.to_bytes()])
        .map_err(|_| Error::new("derived occurrence-one proof refused"))?;
    admit_ticket(&ticket_zero).map_err(|_| Error::new("derived Ticket refused decode"))?;

    Ok(SeriesFounderInputV1 {
        schema: SERIES_FOUNDER_INPUT_SCHEMA_V1.into(),
        template_hex: hex(&template),
        occurrence_zero_hex: hex(&occurrence_zero),
        occurrence_one_hex: hex(&occurrence_one),
        occurrence_zero_siblings_hex: vec![hex(&one_id.to_bytes())],
        occurrence_one_siblings_hex: vec![hex(&zero_id.to_bytes())],
        ticket_zero_hex: hex(&ticket_zero),
        roles,
    })
}

fn role_key(seed: [u8; 32], label: &str) -> Pubkey {
    let mut hash = Sha256::new();
    hash.update(b"dclutch/owned-loopback/series-founder-role/v1");
    hash.update([0]);
    hash.update(seed);
    hash.update([0]);
    hash.update(label.as_bytes());
    Pubkey::new_from_array(hash.finalize().into())
}

fn projection_node(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    hashv(&[
        &generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
        &[0],
        &left,
        &right,
    ])
    .to_bytes()
}

fn put<const N: usize>(target: &mut [u8], offset: usize, value: &[u8; N]) -> Result<()> {
    target
        .get_mut(offset..offset + N)
        .ok_or_else(|| Error::new("generated Series wire layout was truncated"))?
        .copy_from_slice(value);
    Ok(())
}

fn decode_seed(value: &str) -> Result<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::new("--seed must be 64 hexadecimal characters"));
    }
    let mut seed = [0_u8; 32];
    for (index, target) in seed.iter_mut().enumerate() {
        *target = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| Error::new("--seed must be hexadecimal"))?;
    }
    Ok(seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_leaf_founder_input_admits_both_proof_directions() {
        let input = build_series_founder_input_v1([0x51; 32]).expect("founder input");
        assert_eq!(input.schema, SERIES_FOUNDER_INPUT_SCHEMA_V1);
        assert_eq!(input.occurrence_zero_siblings_hex.len(), 1);
        assert_eq!(input.occurrence_one_siblings_hex.len(), 1);
        assert_ne!(input.roles.founder, input.roles.refund_owner);
    }
}
