//! Exact Token-2022 allowance renewal for an already-admitted Direct participant.

use std::{fs, path::PathBuf};

use dclutch_custody::token_svm::{
    COption, Mint, TokenAccount,
    instruction::{InstructionSpec, approve_checked},
};
use serde_json::json;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_sdk::signature::{Keypair, Signer as _};

use crate::{
    Error, Result, campaign::read_keypair_file, rpc::Rpc,
    user_position_admission::parse_finalized_direct_participant_evidence_v1,
};

pub(crate) const COMMAND_V1: &str = "local-private-validator-direct-collateral-reapprove-v1";

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-direct-collateral-reapprove-v1 --rpc-url http://127.0.0.1:PORT/ --participant-report ABSOLUTE_JSON --fill-atoms POSITIVE_U64 --fee-payer-keypair ABSOLUTE_JSON --owner-keypair ABSOLUTE_JSON --evidence ABSOLUTE_JSON [--execute]"
}

struct ArgumentsV1 {
    rpc_url: String,
    participant_report: PathBuf,
    fill_atoms: u64,
    fee_payer_keypair: PathBuf,
    owner_keypair: PathBuf,
    evidence: PathBuf,
    execute: bool,
}

pub(crate) fn run_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse(arguments)?;
    let bytes = fs::read(&arguments.participant_report)?;
    let mut rpc = Rpc::connect(&arguments.rpc_url)?;
    let participant = parse_finalized_direct_participant_evidence_v1(&bytes, &mut rpc)?;
    let terms = crate::direct_trade_producer::owned_loopback_default_terms_v1(0);
    let terms = crate::direct_trade_producer::DirectTradeTermsV1 {
        fill: arguments.fill_atoms,
        ..terms
    };
    let allowance_atoms = crate::direct_trade_producer::required_buyer_collateral_v1(
        &terms,
        crate::direct_trade_producer::EXPECTED_PRICE_SCALE_V1,
    )?;
    let token_bytes = rpc
        .required_account(
            participant.collateral_account,
            "Direct participant collateral",
        )?
        .data;
    let token = TokenAccount::parse(&token_bytes)
        .map_err(|error| Error::new(format!("Direct participant collateral: {error:?}")))?;
    let mint_bytes = rpc
        .required_account(participant.mint, "Direct collateral Mint")?
        .data;
    let mint = Mint::parse(&mint_bytes)
        .map_err(|error| Error::new(format!("Direct collateral Mint: {error:?}")))?;
    if token.owner != participant.owner.to_bytes()
        || token.mint != participant.mint.to_bytes()
        || token.amount < allowance_atoms
    {
        return Err(Error::new(
            "authenticated participant collateral owner, Mint, or available balance cannot fund the requested allowance",
        ));
    }
    let instruction = token_instruction(
        approve_checked(
            participant.token_program.to_bytes(),
            participant.collateral_account.to_bytes(),
            participant.mint.to_bytes(),
            participant.custody_authority.to_bytes(),
            participant.owner.to_bytes(),
            allowance_atoms,
            mint.decimals,
        )
        .map_err(|error| Error::new(format!("ApproveChecked: {error:?}")))?,
    );

    if !arguments.execute {
        write_evidence(
            &arguments,
            allowance_atoms,
            &participant,
            &token,
            None,
            None,
        )?;
        println!("preflight only; no key was opened and nothing was sent");
        return Ok(());
    }
    let payer = Keypair::new_from_array(read_keypair_file(
        &arguments.fee_payer_keypair,
        "Direct collateral reapproval fee payer",
    )?);
    let owner = Keypair::new_from_array(read_keypair_file(
        &arguments.owner_keypair,
        "Direct collateral reapproval owner",
    )?);
    if owner.pubkey() != participant.owner {
        return Err(Error::new(
            "owner keypair does not match authenticated participant evidence",
        ));
    }
    let landed = rpc.send_with_signers(
        "Direct collateral reapproval",
        &[instruction],
        &payer,
        &[&owner],
    )?;
    if landed.error.is_some() {
        return Err(Error::new("Direct collateral reapproval refused on chain"));
    }
    let post = TokenAccount::parse(
        &rpc.required_account(participant.collateral_account, "reapproved collateral")?
            .data,
    )
    .map_err(|error| Error::new(format!("reapproved collateral: {error:?}")))?;
    if post.delegate != COption::Some(participant.custody_authority.to_bytes())
        || post.delegated_amount != allowance_atoms
        || post.amount != token.amount
    {
        return Err(Error::new(
            "finalized Token-2022 account did not carry the exact custody delegate and allowance",
        ));
    }
    write_evidence(
        &arguments,
        allowance_atoms,
        &participant,
        &token,
        Some(&post),
        Some(&landed),
    )?;
    Ok(())
}

fn write_evidence(
    arguments: &ArgumentsV1,
    allowance_atoms: u64,
    participant: &crate::user_position_admission::FinalizedDirectParticipantEvidenceV1,
    pre: &TokenAccount,
    post: Option<&TokenAccount>,
    landed: Option<&crate::model::TransactionEvidence>,
) -> Result<()> {
    let document = json!({
        "schema": "dclutch-direct-collateral-reapproval-evidence-v1",
        "cluster": "owned-loopback",
        "participant": participant.owner.to_string(),
        "participantReport": arguments.participant_report,
        "collateralAccount": participant.collateral_account.to_string(),
        "mint": participant.mint.to_string(),
        "custodyAuthority": participant.custody_authority.to_string(),
        "fillAtoms": arguments.fill_atoms,
        "allowanceAtoms": allowance_atoms,
        "preAmount": pre.amount,
        "preDelegatedAmount": pre.delegated_amount,
        "postAmount": post.map(|account| account.amount),
        "postDelegatedAmount": post.map(|account| account.delegated_amount),
        "landed": landed.map(|transaction| json!({
            "signature": transaction.signature,
            "slot": transaction.slot,
            "computeUnitsConsumed": transaction.compute_units_consumed,
            "feeLamports": transaction.fee_lamports,
        })),
    });
    fs::write(
        &arguments.evidence,
        format!("{}\n", serde_json::to_string_pretty(&document)?),
    )?;
    Ok(())
}

fn token_instruction<const ACCOUNTS: usize, const DATA: usize>(
    spec: InstructionSpec<ACCOUNTS, DATA>,
) -> Instruction {
    Instruction {
        program_id: solana_program::pubkey::Pubkey::new_from_array(*spec.program_id()),
        accounts: spec
            .accounts()
            .iter()
            .map(|account| AccountMeta {
                pubkey: solana_program::pubkey::Pubkey::new_from_array(*account.address()),
                is_signer: account.is_signer(),
                is_writable: account.is_writable(),
            })
            .collect(),
        data: spec.data().to_vec(),
    }
}

fn parse(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut values = std::collections::BTreeMap::new();
    let mut execute = false;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        if argument == "--execute" {
            if execute {
                return Err(Error::new("--execute may be supplied only once"));
            }
            execute = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        if !matches!(
            argument.as_str(),
            "--rpc-url"
                | "--participant-report"
                | "--fill-atoms"
                | "--fee-payer-keypair"
                | "--owner-keypair"
                | "--evidence"
        ) {
            return Err(Error::new(format!(
                "unknown Direct collateral reapproval argument: {argument}"
            )));
        }
        if values.insert(argument.clone(), value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let mut take = |name: &str| {
        values
            .remove(name)
            .ok_or_else(|| Error::new(format!("{name} is required")))
    };
    let fill_atoms = take("--fill-atoms")?
        .parse::<u64>()
        .map_err(|_| Error::new("--fill-atoms must be a positive u64"))?;
    if fill_atoms == 0 {
        return Err(Error::new("--fill-atoms must be positive"));
    }
    Ok(ArgumentsV1 {
        rpc_url: take("--rpc-url")?,
        participant_report: PathBuf::from(take("--participant-report")?),
        fill_atoms,
        fee_payer_keypair: PathBuf::from(take("--fee-payer-keypair")?),
        owner_keypair: PathBuf::from(take("--owner-keypair")?),
        evidence: PathBuf::from(take("--evidence")?),
        execute,
    })
}
