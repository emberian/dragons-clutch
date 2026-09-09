//! Structured campaign bridge to the shipped wallet terminal exteriors.
//!
//! This module adds no settlement instruction or state projection. It composes
//! the Claims-role Custody replay creator, wallet-terminal input producer,
//! durable payout exterior, and wallet-authorized Position close in their
//! existing command shapes, then authenticates the remaining aggregate and
//! terminal coordinates directly from chain state. Rational custody remains
//! in aggregate supply until its own terminal redemption runs next.

use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use dclutch_claims::{
    liability_basis_state_v2::{LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2},
    protocol_position_v2::{ProtocolPositionAdmissionSeedsV2, ProtocolPositionSeedsV2},
};
use dclutch_custody::{
    CallerRoleV1, CompartmentV1, CustodyReplaySeedsV1, CustodyReplayV1, CustodyVaultSeedsV1,
};
use dclutch_market::{CoreState, Phase as CorePhase};
use dclutch_operator::wallet_terminal_input::associated_token_account_v1;
use serde::Serialize;
use serde_json::Value;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
};

use crate::{
    Error, Result,
    campaign::CampaignTerminalEvidenceV1,
    model::{SuccessorPlan, TransactionEvidence},
    plan::pubkey,
    rpc::Rpc,
};

/// Files already owned by the Structured campaign and its admitted holder.
pub(crate) struct StructuredNativeSettlementInputV1<'a> {
    pub(crate) rpc: &'a mut Rpc,
    pub(crate) rpc_url: &'a str,
    pub(crate) plan: &'a SuccessorPlan,
    pub(crate) plan_path: &'a Path,
    pub(crate) campaign: &'a CampaignTerminalEvidenceV1,
    pub(crate) campaign_path: &'a Path,
    pub(crate) participant_evidence_path: &'a Path,
    pub(crate) payer: &'a Keypair,
    pub(crate) payer_keypair_path: &'a Path,
    pub(crate) market: Pubkey,
    pub(crate) work_dir: &'a Path,
}

/// Terminal accounts consumed by the Rational redemption continuation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructuredNativeSettlementV1 {
    pub(crate) terminal_certificate: Pubkey,
    pub(crate) custody_replay: Pubkey,
    pub(crate) hoard: Pubkey,
    pub(crate) collateral_recipient: Pubkey,
    pub(crate) claims_aggregate: Pubkey,
    pub(crate) position: Pubkey,
    pub(crate) payouts: Vec<PathBuf>,
    pub(crate) position_close: PathBuf,
    pub(crate) aggregate_revision: u64,
    pub(crate) aggregate_supply: Vec<u64>,
}

/// Discharge every native Claim owned by the campaign actor and close its
/// empty Position through the existing wallet-authorized exteriors.
pub(crate) fn settle_structured_native_wallet_v1(
    input: StructuredNativeSettlementInputV1<'_>,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<StructuredNativeSettlementV1> {
    fs::create_dir_all(input.work_dir)?;
    let claims = pubkey(&input.plan.claims.program_id)?;
    let custody = pubkey(&input.plan.custody.program_id)?;
    let aggregate = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, input.market.as_ref()],
        &claims,
    )
    .0;
    if aggregate != campaign_address_v1(input.campaign, "claims_aggregate")? {
        return Err(Error::new(
            "Structured native settlement aggregate differs from founding report",
        ));
    }
    let aggregate_account = input
        .rpc
        .required_account(aggregate, "Structured Claims aggregate")?;
    let aggregate_view = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Structured Claims aggregate: {error:?}")))?;
    if aggregate_account.owner != claims
        || aggregate_view.logical_market != input.market.to_bytes()
        || aggregate_view.release_set != crate::plan::hex32(&input.plan.release_set_id)?
    {
        return Err(Error::new(
            "Structured native settlement aggregate identity differs",
        ));
    }
    let collateral_mint = campaign_address_v1(input.campaign, "collateral_mint")?;
    let collateral_mint_account = input
        .rpc
        .required_account(collateral_mint, "Structured collateral Mint")?;
    let recipient = associated_token_account_v1(
        input.payer.pubkey(),
        collateral_mint,
        collateral_mint_account.owner,
    );
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            input.market.to_bytes(),
            aggregate_view.release_set,
            CallerRoleV1::Claims,
            aggregate_view.custody_context,
        )
        .as_slices(),
        &custody,
    )
    .0;
    let replay_evidence = input.work_dir.join("claims-custody-replay.json");
    if input.rpc.account(replay)?.is_none() {
        crate::claims_custody_replay::run_owned_loopback_v1(vec![
            "--rpc-url".into(),
            input.rpc_url.into(),
            "--plan".into(),
            input.plan_path.display().to_string(),
            "--evidence".into(),
            input.campaign_path.display().to_string(),
            "--market".into(),
            input.market.to_string(),
            "--fee-payer".into(),
            input.payer.pubkey().to_string(),
            "--output".into(),
            replay_evidence.display().to_string(),
            "--execute".into(),
            "--fee-payer-keypair".into(),
            input.payer_keypair_path.display().to_string(),
        ])?;
    }
    if replay_evidence.exists() {
        harvest_signature_v1(
            input.rpc,
            &replay_evidence,
            &["landed", "signature"],
            transactions,
        )?;
    }
    authenticate_claims_replay_v1(input.rpc, replay, custody, input.market, aggregate_view)?;

    let mut payouts = Vec::new();
    for claim_index in 0..aggregate_view.claim_count {
        let input_path = input
            .work_dir
            .join(format!("native-payout-input-{claim_index}.json"));
        let journal_dir = input
            .work_dir
            .join(format!("native-payout-journal-{claim_index}"));
        fs::create_dir_all(&journal_dir)?;
        let evidence_path = input
            .work_dir
            .join(format!("native-payout-{claim_index}.json"));
        if !input_path.exists() {
            let args = vec![
                "--rpc-url".into(),
                input.rpc_url.into(),
                "--plan".into(),
                input.plan_path.display().to_string(),
                "--evidence".into(),
                input.campaign_path.display().to_string(),
                "--market".into(),
                input.market.to_string(),
                "--owner".into(),
                input.payer.pubkey().to_string(),
                "--recipient".into(),
                recipient.to_string(),
                "--claim-index".into(),
                claim_index.to_string(),
            ];
            let payout_input =
                match crate::terminal_lifecycle::produce_wallet_terminal_input_owned_loopback_v1(
                    args,
                ) {
                    Ok(value) => value,
                    Err(error) if error.to_string().contains("must be within 1..=0 atoms") => {
                        continue;
                    }
                    Err(error) => return Err(error),
                };
            write_exact_json_v1(&input_path, &payout_input)?;
        }
        let payout_args = vec![
            "--rpc-url".into(),
            input.rpc_url.into(),
            "--input".into(),
            input_path.display().to_string(),
            "--fee-payer".into(),
            input.payer.pubkey().to_string(),
            "--fee-payer-keypair".into(),
            input.payer_keypair_path.display().to_string(),
            "--owner-keypair".into(),
            input.payer_keypair_path.display().to_string(),
            "--journal-dir".into(),
            journal_dir.display().to_string(),
            "--evidence".into(),
            evidence_path.display().to_string(),
            "--execute".into(),
        ];
        for pass in 0..16 {
            crate::wallet_terminal_payout_exterior::run(payout_args.clone())?;
            if evidence_path.exists() {
                break;
            }
            if pass == 15 {
                return Err(Error::new(format!(
                    "Structured native payout {claim_index} did not finalize in 16 durable stages"
                )));
            }
        }
        harvest_signature_v1(input.rpc, &evidence_path, &["signature"], transactions)?;
        payouts.push(evidence_path);
    }

    let position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), input.payer.pubkey().to_bytes())
            .map_err(|error| Error::new(format!("Structured owner Position seeds: {error:?}")))?
            .as_slices(),
        &claims,
    )
    .0;
    let admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(
            aggregate.to_bytes(),
            input.payer.pubkey().to_bytes(),
        )
        .map_err(|error| Error::new(format!("Structured owner admission seeds: {error:?}")))?
        .as_slices(),
        &claims,
    )
    .0;
    let close_path = input.work_dir.join("native-position-close.json");
    match (input.rpc.account(position)?, input.rpc.account(admission)?) {
        (Some(_), Some(_)) => {
            crate::user_position_close::run(vec![
                "--rpc-url".into(),
                input.rpc_url.into(),
                "--participant-evidence".into(),
                input.participant_evidence_path.display().to_string(),
                "--fee-payer".into(),
                input.payer.pubkey().to_string(),
                "--evidence".into(),
                close_path.display().to_string(),
                "--execute".into(),
                "--position-owner-keypair".into(),
                input.payer_keypair_path.display().to_string(),
                "--fee-payer-keypair".into(),
                input.payer_keypair_path.display().to_string(),
            ])?;
            harvest_optional_signature_v1(
                input.rpc,
                &close_path,
                &["finalized", "signature"],
                transactions,
            )?;
        }
        (None, None) => {
            if !close_path.exists() {
                crate::user_position_close::run(vec![
                    "--rpc-url".into(),
                    input.rpc_url.into(),
                    "--participant-evidence".into(),
                    input.participant_evidence_path.display().to_string(),
                    "--fee-payer".into(),
                    input.payer.pubkey().to_string(),
                    "--evidence".into(),
                    close_path.display().to_string(),
                    "--execute".into(),
                    "--position-owner-keypair".into(),
                    input.payer_keypair_path.display().to_string(),
                    "--fee-payer-keypair".into(),
                    input.payer_keypair_path.display().to_string(),
                ])?;
            }
            harvest_optional_signature_v1(
                input.rpc,
                &close_path,
                &["finalized", "signature"],
                transactions,
            )?;
        }
        _ => {
            return Err(Error::new(
                "Structured native Position and admission are not an atomic live or closed pair",
            ));
        }
    }
    if input.rpc.account(position)?.is_some() || input.rpc.account(admission)?.is_some() {
        return Err(Error::new(
            "Structured native wallet close left its Position or admission live",
        ));
    }
    let aggregate_after_account = input.rpc.required_account(
        aggregate,
        "Structured Claims aggregate after native settlement",
    )?;
    let aggregate_after = LiabilityBasisMarketViewV2::decode(&aggregate_after_account.data)
        .map_err(|error| Error::new(format!("Structured settled aggregate: {error:?}")))?;
    if aggregate_after_account.owner != claims
        || aggregate_after.logical_market != input.market.to_bytes()
        || aggregate_after.release_set != aggregate_view.release_set
        || aggregate_after.claim_count != aggregate_view.claim_count
    {
        return Err(Error::new(
            "Structured settled aggregate changed owner, Market, release, or width",
        ));
    }
    let supplies = (0..aggregate_after.claim_count)
        .map(|index| {
            aggregate_after
                .supply(&aggregate_after_account.data, index)
                .map_err(|error| {
                    Error::new(format!("Structured aggregate supply {index}: {error:?}"))
                })
        })
        .collect::<Result<Vec<_>>>()?;
    // Supply held by Rational custody is intentionally still live here. The
    // caller redeems every remaining shard next, then requires this aggregate
    // to reach zero before beginning retirement.
    let core_account = input
        .rpc
        .required_account(input.market, "terminal Core Market")?;
    let core = CoreState::decode(&core_account.data)
        .map_err(|error| Error::new(format!("Structured terminal Core Market: {error:?}")))?;
    if core_account.owner != pubkey(&input.plan.core.program_id)?
        || core.identity.market_id.to_bytes() != input.market.to_bytes()
        || core.identity.selected_release_set.to_bytes() != aggregate_after.release_set
        || core.phase != CorePhase::Terminal
    {
        return Err(Error::new(
            "Structured terminal Core Market changed owner, identity, release, or phase",
        ));
    }
    let terminal_certificate = core
        .terminal_receipt
        .ok_or_else(|| Error::new("Structured settlement Core Market is not Terminal"))?;
    let terminal_certificate = Pubkey::new_from_array(terminal_certificate.to_bytes());
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            input.market.to_bytes(),
            aggregate_after.release_set,
            aggregate_after.custody_context,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &custody,
    )
    .0;
    input
        .rpc
        .required_account(terminal_certificate, "terminal certificate")?;
    input.rpc.required_account(hoard, "Structured Hoard")?;
    Ok(StructuredNativeSettlementV1 {
        terminal_certificate,
        custody_replay: replay,
        hoard,
        collateral_recipient: recipient,
        claims_aggregate: aggregate,
        position,
        payouts,
        position_close: close_path,
        aggregate_revision: aggregate_after.revision,
        aggregate_supply: supplies,
    })
}

fn authenticate_claims_replay_v1(
    rpc: &mut Rpc,
    replay: Pubkey,
    custody: Pubkey,
    market: Pubkey,
    aggregate: LiabilityBasisMarketViewV2,
) -> Result<()> {
    let account = rpc.required_account(replay, "Claims-role Custody replay")?;
    let value = CustodyReplayV1::decode(&account.data)
        .map_err(|error| Error::new(format!("Claims-role Custody replay: {error:?}")))?;
    if account.owner != custody
        || value.caller_role != CallerRoleV1::Claims
        || value.market != market.to_bytes()
        || value.release_set != aggregate.release_set
        || value.context != aggregate.custody_context
    {
        return Err(Error::new(
            "Structured Claims-role Custody replay differs from aggregate",
        ));
    }
    Ok(())
}

fn campaign_address_v1(campaign: &CampaignTerminalEvidenceV1, label: &str) -> Result<Pubkey> {
    pubkey(
        &campaign
            .accounts
            .get(label)
            .ok_or_else(|| Error::new(format!("Structured campaign report omitted {label}")))?
            .address,
    )
}

fn write_exact_json_v1(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    if path.exists() {
        if fs::read(path)? != bytes {
            return Err(Error::new(format!(
                "Structured settlement refuses to replace changed {}",
                path.display()
            )));
        }
        return Ok(());
    }
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = File::create(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

fn harvest_signature_v1(
    rpc: &mut Rpc,
    path: &Path,
    fields: &[&str],
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<()> {
    let document: Value = serde_json::from_slice(&fs::read(path)?)?;
    let mut cursor = &document;
    for field in fields {
        cursor = cursor
            .get(*field)
            .ok_or_else(|| Error::new(format!("{} omitted finalized {field}", path.display())))?;
    }
    let signature_text = cursor
        .as_str()
        .ok_or_else(|| Error::new(format!("{} signature is not text", path.display())))?;
    if transactions
        .iter()
        .any(|transaction| transaction.signature == signature_text)
    {
        return Ok(());
    }
    let signature = signature_text
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("{} signature: {error}", path.display())))?;
    let finalized = rpc
        .finalized_signed_packet("Structured native settlement", signature, false)?
        .ok_or_else(|| Error::new(format!("{} signature is not finalized", path.display())))?;
    transactions.push(finalized.evidence);
    Ok(())
}

fn harvest_optional_signature_v1(
    rpc: &mut Rpc,
    path: &Path,
    fields: &[&str],
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<()> {
    let document: Value = serde_json::from_slice(&fs::read(path)?)?;
    let mut cursor = &document;
    for field in fields {
        let Some(value) = cursor.get(*field) else {
            return Ok(());
        };
        cursor = value;
    }
    let signature_text = cursor
        .as_str()
        .ok_or_else(|| Error::new(format!("{} signature is not text", path.display())))?;
    if transactions
        .iter()
        .any(|transaction| transaction.signature == signature_text)
    {
        return Ok(());
    }
    let signature = signature_text
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("{} signature: {error}", path.display())))?;
    let finalized = rpc
        .finalized_signed_packet("Structured native settlement", signature, false)?
        .ok_or_else(|| Error::new(format!("{} signature is not finalized", path.display())))?;
    transactions.push(finalized.evidence);
    Ok(())
}
