//! Exterior for one wallet-authorized User Position close on an owned validator.
//!
//! The participant journal supplies only routing coordinates and is reopened
//! against finalized transaction history. The operator then authenticates one
//! current finalized snapshot and predicts the exact Claims receipt from live
//! balances. No close semantics are implemented here.

use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr as _,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_claims_svm::{
    liability_basis_state_v2::LiabilityBasisMarketViewV2,
    protocol_position_v2::{
        ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2,
        ProtocolPositionCloseReceiptV2,
    },
};
use dclutch_operator::{
    ObservedAccount,
    user_position_close_v1::{
        UserPositionClosePlanV1, UserPositionCloseSnapshotV1, plan_user_position_close_v1,
    },
};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_relay_contract::SOLANA_DEVNET_GENESIS_HASH_V1;
use dclutch_release_set_contract::ExecutionRoleV1;
use serde_json::json;
use sha2::{Digest as _, Sha256};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Error, Result,
    campaign::{parse_campaign_terminal_evidence_with_expected_cluster_v1, read_keypair_file},
    cluster::{ClusterOriginV1, ExpectedClusterV1},
    direct_trade::{
        authenticate_devnet_terminal_evidence_v1, authenticate_owned_loopback_terminal_evidence_v1,
    },
    rpc::{Rpc, WritePolicyV1},
    terminal_lifecycle::{authenticate_plan_source, finalized_snapshot},
    user_position_admission::parse_finalized_direct_participant_evidence_for_cluster_v1,
};

/// Owned-loopback command name.
pub(crate) const COMMAND_V1: &str = "local-private-validator-user-position-close-v1";
/// Public-devnet command name. Its origin, evidence, and history domains are
/// distinct from [`COMMAND_V1`].
pub(crate) const COMMAND_DEVNET_V1: &str = "devnet-user-position-close-v1";

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-user-position-close-v1 --rpc-url http://127.0.0.1:PORT (--participant-evidence ABSOLUTE_JSON | --direct-evidence ABSOLUTE_JSON --plan ABSOLUTE_JSON --market-input ABSOLUTE_JSON --campaign-evidence ABSOLUTE_JSON --position-owner PUBKEY) --fee-payer FEE_PAYER --evidence ABSOLUTE_NEW_JSON [--execute --position-owner-keypair ABSOLUTE_JSON --fee-payer-keypair ABSOLUTE_JSON]\n\
     dclutch-local-successor-bootstrap devnet-user-position-close-v1 --rpc-url URL --i-mean-devnet GENESIS_HASH (--participant-evidence ABSOLUTE_JSON | --direct-evidence ABSOLUTE_JSON --plan ABSOLUTE_JSON --market-input ABSOLUTE_JSON --campaign-evidence ABSOLUTE_JSON --position-owner PUBKEY) --fee-payer FEE_PAYER --evidence ABSOLUTE_NEW_JSON [--execute --position-owner-keypair ABSOLUTE_JSON --fee-payer-keypair ABSOLUTE_JSON]\n\
     \nCloses one terminal zero-vector wallet Position and its admission record through the wallet-authorized Trading outer and the canonical Claims Close action. The participant arm reopens finalized admission/collateral history. The Direct terminal arm reopens the exact signed manifest, durable mutation journals, and finalized transaction history after payouts, then admits only an authenticated seller or buyer Position. Every current program, immutable admission baseline, live balance, and RentCredit fact is read from one finalized snapshot. A donated lamport cannot veto this close: the predicted and observed receipt must conserve the complete live balances into the Market's RentCredit. Without --execute this is a dry run that opens no key and sends nothing."
}

#[derive(Debug)]
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    expected_cluster: ExpectedClusterV1,
    source: SourceArgumentsV1,
    fee_payer: Pubkey,
    position_owner_keypair: Option<PathBuf>,
    fee_payer_keypair: Option<PathBuf>,
    evidence: PathBuf,
    execute: bool,
}

#[derive(Debug)]
enum SourceArgumentsV1 {
    Participant {
        evidence: PathBuf,
    },
    DirectTerminal {
        evidence: PathBuf,
        plan: PathBuf,
        market_input: PathBuf,
        campaign_evidence: PathBuf,
        owner: Pubkey,
    },
}

struct AuthenticatedCloseSourceV1 {
    kind: &'static str,
    sha256: String,
    market: Pubkey,
    claims_market: Pubkey,
    position: Pubkey,
    owner: Pubkey,
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    run_for_cluster_v1(arguments, ExpectedClusterV1::OwnedLoopback)
}

pub(crate) fn run_devnet_v1(arguments: Vec<String>) -> Result<()> {
    run_for_cluster_v1(arguments, ExpectedClusterV1::Devnet)
}

fn run_for_cluster_v1(arguments: Vec<String>, expected_cluster: ExpectedClusterV1) -> Result<()> {
    let arguments = parse_for_cluster_v1(arguments, expected_cluster)?;
    let mut rpc = Rpc::connect_cluster(
        &arguments.origin,
        if arguments.execute {
            WritePolicyV1::Writes
        } else {
            WritePolicyV1::ReadsOnly
        },
    )?;
    let source =
        authenticate_close_source_v1(&mut rpc, &arguments.source, arguments.expected_cluster)?;
    let identity = derive_position_identity(&mut rpc, source.claims_market, source.owner)?;
    if source.market != identity.market || source.position != identity.position {
        return Err(Error::new(format!(
            "{} source names Market {}/Position {}, but the current Claims aggregate derives {}/{}",
            source.kind, source.market, source.position, identity.market, identity.position
        )));
    }

    let standing_position = rpc.account(identity.position)?;
    let standing_admission = rpc.account(identity.admission)?;
    match (&standing_position, &standing_admission) {
        (None, None) => {
            write_already_closed(&arguments, &identity, &source)?;
            println!("Position             {}", identity.position);
            println!("admission            {}", identity.admission);
            println!("state                already closed; nothing to do");
            return Ok(());
        }
        (Some(_), Some(_)) => {}
        _ => {
            return Err(Error::new(
                "only one of the canonical Position/admission pair exists; their close is atomic",
            ));
        }
    }

    let coordinates = derive_live_coordinates(&mut rpc, identity)?;
    let (snapshot, payer_before) = gather(&mut rpc, &coordinates, arguments.fee_payer)?;
    let plan = plan_user_position_close_v1(&snapshot)
        .map_err(|error| Error::new(format!("User Position close plan refused: {error:?}")))?;
    report_plan(&plan);
    if !arguments.execute {
        write_planned(&arguments, &coordinates, &source, &plan)?;
        println!("dry run; no key was opened and nothing was sent");
        return Ok(());
    }

    let owner_path = arguments
        .position_owner_keypair
        .as_deref()
        .ok_or_else(|| Error::new("--execute requires --position-owner-keypair"))?;
    let payer_path = arguments
        .fee_payer_keypair
        .as_deref()
        .ok_or_else(|| Error::new("--execute requires --fee-payer-keypair"))?;
    let owner = Keypair::new_from_array(read_keypair_file(owner_path, "Position owner")?);
    let payer = Keypair::new_from_array(read_keypair_file(payer_path, "Position close fee payer")?);
    if owner.pubkey() != plan.required_signer {
        return Err(Error::new(format!(
            "Position owner key is {}, but the authenticated Position owner is {}",
            owner.pubkey(),
            plan.required_signer
        )));
    }
    if payer.pubkey() != arguments.fee_payer {
        return Err(Error::new(format!(
            "fee-payer key is {}, not --fee-payer {}",
            payer.pubkey(),
            arguments.fee_payer
        )));
    }

    let evidence = if payer.pubkey() == owner.pubkey() {
        rpc.send_with_signers(
            "User Position close",
            &[plan.instruction.clone()],
            &payer,
            &[],
        )?
    } else {
        rpc.send_with_signers(
            "User Position close",
            &[plan.instruction.clone()],
            &payer,
            &[&owner],
        )?
    };
    if let Some(error) = &evidence.error {
        return Err(Error::new(format!(
            "User Position close refused on chain: {error}"
        )));
    }
    let signature = Signature::from_str(&evidence.signature)
        .map_err(|error| Error::new(format!("close signature: {error}")))?;
    let history = rpc
        .finalized_signed_packet("User Position close", signature, false)?
        .ok_or_else(|| Error::new("finalized close history disappeared after confirmation"))?;
    let returned = history
        .return_data
        .ok_or_else(|| Error::new("finalized close omitted Claims return data"))?;
    if returned.program != plan.expected_receipt_producer
        || returned.data != plan.expected_receipt_body
    {
        return Err(Error::new(
            "finalized close return data differed from the live-balance receipt predicted before signing",
        ));
    }
    let receipt = ProtocolPositionCloseReceiptV2::decode(&returned.data)
        .map_err(|error| Error::new(format!("finalized close receipt: {error:?}")))?;
    receipt
        .validate_request(
            plan.claims_request,
            plan.claims_request_digest,
            plan.expected_receipt_producer.to_bytes(),
        )
        .map_err(|error| Error::new(format!("finalized close receipt binding: {error:?}")))?;

    let post = finalized_snapshot(
        &mut rpc,
        &[
            coordinates.position,
            coordinates.admission,
            coordinates.rent_credit,
            coordinates.owner,
            arguments.fee_payer,
        ],
    )?;
    let position_after = post.account(coordinates.position)?;
    let admission_after = post.account(coordinates.admission)?;
    let rent_after = post.required(coordinates.rent_credit, "lifecycle RentCredit")?;
    let owner_after = post.required(coordinates.owner, "Position owner")?;
    let payer_after = post.required(arguments.fee_payer, "fee payer")?;
    if position_after.lamports != 0
        || admission_after.lamports != 0
        || !position_after.data.is_empty()
        || !admission_after.data.is_empty()
        || position_after.owner != system_program::ID
        || admission_after.owner != system_program::ID
        || rent_after.lamports != plan.rent_credit_after_lamports
        || rent_after.data != snapshot.rent_credit.data
        || rent_after.owner != snapshot.rent_credit.owner
    {
        return Err(Error::new(
            "finalized close did not delete both accounts or credit the exact conserved live balance",
        ));
    }
    let fee = evidence
        .fee_lamports
        .ok_or_else(|| Error::new("finalized close omitted its transaction fee"))?;
    authenticate_wallet_poststate(
        &snapshot.owner,
        payer_before.as_ref(),
        owner_after,
        payer_after,
        arguments.fee_payer,
        fee,
    )?;

    write_finalized(
        &arguments,
        &coordinates,
        &source,
        &plan,
        &evidence,
        &returned.data,
        owner_after,
        payer_after,
    )?;
    println!("signature            {}", evidence.signature);
    println!("slot                 {}", evidence.slot);
    println!("Position after       gone (read back from chain)");
    println!("admission after      gone (read back from chain)");
    println!(
        "RentCredit after     {} (exact live-balance conservation)",
        rent_after.lamports
    );
    Ok(())
}

fn authenticate_close_source_v1(
    rpc: &mut Rpc,
    arguments: &SourceArgumentsV1,
    expected_cluster: ExpectedClusterV1,
) -> Result<AuthenticatedCloseSourceV1> {
    match arguments {
        SourceArgumentsV1::Participant { evidence } => {
            let bytes = read_source_v1(evidence, "participant evidence")?;
            let participant = parse_finalized_direct_participant_evidence_for_cluster_v1(
                &bytes,
                rpc,
                expected_cluster,
            )?;
            Ok(AuthenticatedCloseSourceV1 {
                kind: "participant",
                sha256: hex(&Sha256::digest(&bytes)),
                market: participant.market,
                claims_market: participant.claims_market,
                position: participant.position,
                owner: participant.owner,
            })
        }
        SourceArgumentsV1::DirectTerminal {
            evidence,
            plan,
            market_input,
            campaign_evidence,
            owner,
        } => {
            let evidence_bytes = read_source_v1(evidence, "Direct finalized evidence")?;
            let plan_bytes = read_source_v1(plan, "successor plan")?;
            let market_input_bytes = read_source_v1(market_input, "Market input")?;
            let campaign_bytes = read_source_v1(campaign_evidence, "campaign evidence")?;
            let campaign = parse_campaign_terminal_evidence_with_expected_cluster_v1(
                &campaign_bytes,
                expected_cluster,
            )?;
            authenticate_plan_source(&plan_bytes, &campaign.plan_sha256)?;
            if hex(&Sha256::digest(&market_input_bytes)) != campaign.market_sha256 {
                return Err(Error::new(
                    "Direct terminal close Market input changed from founding evidence",
                ));
            }
            let campaign_key = |name: &str| -> Result<Pubkey> {
                campaign
                    .accounts
                    .get(name)
                    .ok_or_else(|| Error::new(format!("campaign evidence omitted {name}")))?
                    .address
                    .parse::<Pubkey>()
                    .map_err(|error| Error::new(format!("campaign {name}: {error}")))
            };
            let market = campaign_key("founding_market")?;
            let claims_market = campaign_key("claims_aggregate")?;
            let terminal = match expected_cluster {
                ExpectedClusterV1::Devnet => authenticate_devnet_terminal_evidence_v1(
                    rpc,
                    evidence,
                    market,
                    &hex(&Sha256::digest(&plan_bytes)),
                    &hex(&Sha256::digest(&market_input_bytes)),
                )?,
                ExpectedClusterV1::OwnedLoopback => {
                    authenticate_owned_loopback_terminal_evidence_v1(
                        rpc,
                        evidence,
                        market,
                        &hex(&Sha256::digest(&plan_bytes)),
                        &hex(&Sha256::digest(&market_input_bytes)),
                    )?
                }
            };
            if read_source_v1(evidence, "Direct finalized evidence")? != evidence_bytes {
                return Err(Error::new(
                    "Direct finalized evidence changed while its terminal history was authenticated",
                ));
            }
            if terminal.direct.market != market || terminal.claims_market != claims_market {
                return Err(Error::new(
                    "Direct terminal history and founding evidence named different Market/Claims roots",
                ));
            }
            let position = if *owner == terminal.direct.seller_owner {
                terminal.direct.seller_position
            } else if *owner == terminal.direct.buyer_owner {
                terminal.direct.buyer_position
            } else {
                return Err(Error::new(
                    "--position-owner is neither authenticated Direct seller nor buyer",
                ));
            };
            Ok(AuthenticatedCloseSourceV1 {
                kind: "direct-terminal",
                sha256: hex(&Sha256::digest(&evidence_bytes)),
                market,
                claims_market,
                position,
                owner: *owner,
            })
        }
    }
}

fn read_source_v1(path: &Path, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| Error::new(format!("{label} {}: {error}", path.display())))?;
    if !path.is_absolute()
        || metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > 16 * 1024 * 1024
        || fs::canonicalize(path)? != path
    {
        return Err(Error::new(format!(
            "{label} must be one absolute regular file within 1..16777216 bytes"
        )));
    }
    fs::read(path).map_err(Into::into)
}

#[derive(Clone, Copy)]
struct PositionIdentityV1 {
    market: Pubkey,
    claims_market: Pubkey,
    position: Pubkey,
    admission: Pubkey,
    owner: Pubkey,
    claims_program: Pubkey,
    release_set: [u8; 32],
    registry_program: Pubkey,
}

#[derive(Clone, Copy)]
struct CoordinatesV1 {
    market: Pubkey,
    claims_market: Pubkey,
    position: Pubkey,
    admission: Pubkey,
    owner: Pubkey,
    rent_credit: Pubkey,
    rent_program: Pubkey,
    activation_cache: Pubkey,
    registry_program: Pubkey,
    trading_program: Pubkey,
    trading_programdata: Pubkey,
    claims_program: Pubkey,
    claims_programdata: Pubkey,
}

fn derive_position_identity(
    rpc: &mut Rpc,
    claims_market: Pubkey,
    owner: Pubkey,
) -> Result<PositionIdentityV1> {
    let aggregate_account = rpc.required_account(claims_market, "Claims aggregate")?;
    let aggregate = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
    let claims_program = aggregate_account.owner;
    let admission_seeds =
        ProtocolPositionAdmissionSeedsV2::new(claims_market.to_bytes(), owner.to_bytes())
            .map_err(|error| Error::new(format!("admission seeds: {error:?}")))?;
    let admission = Pubkey::find_program_address(&admission_seeds.as_slices(), &claims_program).0;
    let position_seeds = dclutch_claims_svm::protocol_position_v2::ProtocolPositionSeedsV2::new(
        claims_market.to_bytes(),
        owner.to_bytes(),
    )
    .map_err(|error| Error::new(format!("Position seeds: {error:?}")))?;
    let position = Pubkey::find_program_address(&position_seeds.as_slices(), &claims_program).0;
    Ok(PositionIdentityV1 {
        market: Pubkey::new_from_array(aggregate.logical_market),
        claims_market,
        position,
        admission,
        owner,
        claims_program,
        release_set: aggregate.release_set,
        registry_program: Pubkey::new_from_array(aggregate.registry_program),
    })
}

fn derive_live_coordinates(rpc: &mut Rpc, identity: PositionIdentityV1) -> Result<CoordinatesV1> {
    let PositionIdentityV1 {
        market,
        claims_market,
        position,
        admission,
        owner,
        claims_program,
        release_set,
        registry_program,
    } = identity;
    let admission_account = rpc.required_account(admission, "Claims admission")?;
    let admitted = ProtocolPositionAdmissionV2::decode(&admission_account.data)
        .map_err(|error| Error::new(format!("Claims admission: {error:?}")))?;
    let trading_program = Pubkey::new_from_array(admitted.trading_program());
    if admitted.claims_program() != claims_program.to_bytes()
        || admitted.market() != market.to_bytes()
        || admitted.release_set() != release_set
        || admitted.position_owner() != owner.to_bytes()
    {
        return Err(Error::new(
            "Claims admission did not join the live aggregate, Claims program, or owner",
        ));
    }
    let activation_cache =
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set], &registry_program)
            .0;
    let cache = rpc.required_account(activation_cache, "Registry activation cache")?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache.data)
        .map_err(|error| Error::new(format!("activation cache: {error:?}")))?;
    let selected = |role: ExecutionRoleV1| -> Result<(Pubkey, Pubkey)> {
        let release = activated
            .role(role)
            .map_err(|error| Error::new(format!("{role:?} release: {error:?}")))?
            .release();
        Ok((
            Pubkey::new_from_array(release.program().to_bytes()),
            Pubkey::new_from_array(release.programdata()),
        ))
    };
    let (selected_trading, trading_programdata) = selected(ExecutionRoleV1::Trading)?;
    let (selected_claims, claims_programdata) = selected(ExecutionRoleV1::Claims)?;
    if selected_trading != trading_program || selected_claims != claims_program {
        return Err(Error::new(
            "admission programs differed from the aggregate's activated release",
        ));
    }
    Ok(CoordinatesV1 {
        market,
        claims_market,
        position,
        admission,
        owner,
        rent_credit: Pubkey::new_from_array(admitted.rent_credit()),
        rent_program: Pubkey::new_from_array(admitted.rent_program()),
        activation_cache,
        registry_program,
        trading_program,
        trading_programdata,
        claims_program,
        claims_programdata,
    })
}

fn gather(
    rpc: &mut Rpc,
    coordinates: &CoordinatesV1,
    fee_payer: Pubkey,
) -> Result<(UserPositionCloseSnapshotV1, Option<ObservedAccount>)> {
    let keys = [
        coordinates.claims_market,
        coordinates.position,
        coordinates.admission,
        sysvar::rent::ID,
        system_program::ID,
        coordinates.activation_cache,
        coordinates.registry_program,
        coordinates.trading_program,
        coordinates.trading_programdata,
        coordinates.claims_program,
        coordinates.claims_programdata,
        coordinates.owner,
        coordinates.rent_credit,
        coordinates.rent_program,
        fee_payer,
    ];
    let finalized = finalized_snapshot(rpc, &keys)?;
    let get = |key: Pubkey, label: &str| -> Result<ObservedAccount> {
        Ok(finalized.required(key, label)?.clone())
    };
    let snapshot = UserPositionCloseSnapshotV1 {
        // The existing operator uses this constant as its public-cluster
        // capability label. The owned-loopback transport boundary above is the
        // authority that prevents public submission from this command.
        genesis_hash: SOLANA_DEVNET_GENESIS_HASH_V1,
        claims_market: get(coordinates.claims_market, "Claims aggregate")?,
        position: get(coordinates.position, "Claims Position")?,
        admission: get(coordinates.admission, "Claims admission")?,
        rent_sysvar: get(sysvar::rent::ID, "Rent sysvar")?,
        system_program: get(system_program::ID, "System Program")?,
        activation_cache: get(coordinates.activation_cache, "activation cache")?,
        registry_program: get(coordinates.registry_program, "Registry program")?,
        trading_program: get(coordinates.trading_program, "Trading program")?,
        trading_programdata: get(coordinates.trading_programdata, "Trading ProgramData")?,
        claims_program: get(coordinates.claims_program, "Claims program")?,
        claims_programdata: get(coordinates.claims_programdata, "Claims ProgramData")?,
        owner: get(coordinates.owner, "Position owner")?,
        rent_credit: get(coordinates.rent_credit, "lifecycle RentCredit")?,
        rent_program: get(coordinates.rent_program, "Rent program")?,
    };
    let payer = if fee_payer == coordinates.owner {
        None
    } else {
        Some(get(fee_payer, "fee payer")?)
    };
    Ok((snapshot, payer))
}

fn authenticate_wallet_poststate(
    owner_before: &ObservedAccount,
    payer_before: Option<&ObservedAccount>,
    owner_after: &ObservedAccount,
    payer_after: &ObservedAccount,
    fee_payer: Pubkey,
    fee: u64,
) -> Result<()> {
    let same_except_lamports = |before: &ObservedAccount, after: &ObservedAccount| {
        before.key == after.key
            && before.owner == after.owner
            && before.executable == after.executable
            && before.data == after.data
    };
    if fee_payer == owner_before.key {
        if !same_except_lamports(owner_before, owner_after)
            || payer_after != owner_after
            || owner_before.lamports.checked_sub(fee) != Some(owner_after.lamports)
        {
            return Err(Error::new(
                "combined Position owner/fee payer changed by more than the exact transaction fee",
            ));
        }
    } else {
        let payer_before = payer_before.ok_or_else(|| Error::new("payer prestate omitted"))?;
        if !same_except_lamports(owner_before, owner_after)
            || owner_before.lamports != owner_after.lamports
            || !same_except_lamports(payer_before, payer_after)
            || payer_before.lamports.checked_sub(fee) != Some(payer_after.lamports)
        {
            return Err(Error::new(
                "Position owner or fee payer changed outside the exact fee debit",
            ));
        }
    }
    Ok(())
}

fn report_plan(plan: &UserPositionClosePlanV1) {
    println!("== wallet Position close, read off chain ==");
    println!("Position             {}", plan.position);
    println!("admission            {}", plan.admission);
    println!("owner signer         {}", plan.required_signer);
    println!("Position live        {} lamports", plan.position_lamports);
    println!("admission live       {} lamports", plan.admission_lamports);
    println!(
        "RentCredit           {} -> {}",
        plan.rent_credit_before_lamports, plan.rent_credit_after_lamports
    );
    println!(
        "total conserved      {} lamports",
        plan.total_credit_lamports
    );
    println!("accounts             {}", plan.instruction.accounts.len());
}

fn evidence_plan(
    coordinates: &CoordinatesV1,
    source: &AuthenticatedCloseSourceV1,
    plan: &UserPositionClosePlanV1,
) -> serde_json::Value {
    json!({
        "sourceKind": source.kind,
        "sourceSha256": source.sha256,
        "sourceMarket": source.market.to_string(),
        "market": coordinates.market.to_string(),
        "claimsMarket": coordinates.claims_market.to_string(),
        "position": coordinates.position.to_string(),
        "admission": coordinates.admission.to_string(),
        "owner": coordinates.owner.to_string(),
        "rentCredit": coordinates.rent_credit.to_string(),
        "requestSha256": hex(&plan.claims_request_digest),
        "expectedReceiptBase64": BASE64.encode(plan.expected_receipt_body),
        "positionLamports": plan.position_lamports,
        "admissionLamports": plan.admission_lamports,
        "totalCreditLamports": plan.total_credit_lamports,
        "rentCreditBeforeLamports": plan.rent_credit_before_lamports,
        "rentCreditAfterLamports": plan.rent_credit_after_lamports,
        "accountCount": plan.instruction.accounts.len(),
        "requiredSigner": plan.required_signer.to_string(),
    })
}

fn write_planned(
    arguments: &ArgumentsV1,
    coordinates: &CoordinatesV1,
    source: &AuthenticatedCloseSourceV1,
    plan: &UserPositionClosePlanV1,
) -> Result<()> {
    write_document(
        &arguments.evidence,
        json!({
            "schema": "dclutch-user-position-close-evidence-v1",
            "cluster": arguments.expected_cluster.evidence_label(),
            "phase": "planned",
            "authorizedMutation": false,
            "plan": evidence_plan(coordinates, source, plan),
        }),
    )
}

fn write_finalized(
    arguments: &ArgumentsV1,
    coordinates: &CoordinatesV1,
    source: &AuthenticatedCloseSourceV1,
    plan: &UserPositionClosePlanV1,
    transaction: &crate::model::TransactionEvidence,
    receipt: &[u8],
    owner_after: &ObservedAccount,
    payer_after: &ObservedAccount,
) -> Result<()> {
    write_document(
        &arguments.evidence,
        json!({
            "schema": "dclutch-user-position-close-evidence-v1",
            "cluster": arguments.expected_cluster.evidence_label(),
            "phase": "finalized",
            "authorizedMutation": true,
            "plan": evidence_plan(coordinates, source, plan),
            "finalized": {
                "signature": transaction.signature,
                "slot": transaction.slot,
                "feeLamports": transaction.fee_lamports,
                "computeUnitsConsumed": transaction.compute_units_consumed,
                "receiptBase64": BASE64.encode(receipt),
                "receiptSha256": hex(&Sha256::digest(receipt)),
                "positionClosed": true,
                "admissionClosed": true,
                "rentCreditAfterLamports": plan.rent_credit_after_lamports,
                "ownerAfterLamports": owner_after.lamports,
                "feePayerAfterLamports": payer_after.lamports,
            }
        }),
    )
}

fn write_already_closed(
    arguments: &ArgumentsV1,
    identity: &PositionIdentityV1,
    source: &AuthenticatedCloseSourceV1,
) -> Result<()> {
    write_document(
        &arguments.evidence,
        json!({
            "schema": "dclutch-user-position-close-evidence-v1",
            "cluster": arguments.expected_cluster.evidence_label(),
            "phase": "already-closed",
            "authorizedMutation": false,
            "sourceKind": source.kind,
            "sourceSha256": source.sha256,
            "sourceMarket": source.market.to_string(),
            "market": identity.market.to_string(),
            "claimsMarket": identity.claims_market.to_string(),
            "position": identity.position.to_string(),
            "admission": identity.admission.to_string(),
            "owner": identity.owner.to_string(),
        }),
    )
}

fn write_document(path: &PathBuf, document: serde_json::Value) -> Result<()> {
    fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(&document)?),
    )
    .map_err(|error| Error::new(format!("{}: {error}", path.display())))?;
    println!("evidence             {}", path.display());
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse(arguments: Vec<String>) -> Result<ArgumentsV1> {
    parse_for_cluster_v1(arguments, ExpectedClusterV1::OwnedLoopback)
}

fn parse_for_cluster_v1(
    arguments: Vec<String>,
    expected_cluster: ExpectedClusterV1,
) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut participant_evidence = None;
    let mut direct_evidence = None;
    let mut plan = None;
    let mut market_input = None;
    let mut campaign_evidence = None;
    let mut position_owner = None;
    let mut fee_payer = None;
    let mut position_owner_keypair = None;
    let mut fee_payer_keypair = None;
    let mut evidence = None;
    let mut execute = false;
    let mut cursor = arguments.into_iter();
    while let Some(flag) = cursor.next() {
        let mut value = || {
            cursor
                .next()
                .ok_or_else(|| Error::new(format!("{flag} requires a value")))
        };
        match flag.as_str() {
            "--rpc-url" => rpc_url = Some(value()?),
            "--i-mean-devnet" => acknowledgment = Some(value()?),
            "--participant-evidence" => participant_evidence = Some(PathBuf::from(value()?)),
            "--direct-evidence" => direct_evidence = Some(PathBuf::from(value()?)),
            "--plan" => plan = Some(PathBuf::from(value()?)),
            "--market-input" => market_input = Some(PathBuf::from(value()?)),
            "--campaign-evidence" => campaign_evidence = Some(PathBuf::from(value()?)),
            "--position-owner" => {
                position_owner = Some(value()?.parse::<Pubkey>().map_err(|error| {
                    Error::new(format!("--position-owner must be a public key: {error}"))
                })?)
            }
            "--fee-payer" => {
                fee_payer = Some(value()?.parse::<Pubkey>().map_err(|error| {
                    Error::new(format!("--fee-payer must be a public key: {error}"))
                })?)
            }
            "--position-owner-keypair" => position_owner_keypair = Some(PathBuf::from(value()?)),
            "--fee-payer-keypair" => fee_payer_keypair = Some(PathBuf::from(value()?)),
            "--evidence" => evidence = Some(PathBuf::from(value()?)),
            "--execute" => execute = true,
            _ => return Err(Error::new(format!("unknown argument {flag}"))),
        }
    }
    let direct_width = [
        direct_evidence.is_some(),
        plan.is_some(),
        market_input.is_some(),
        campaign_evidence.is_some(),
        position_owner.is_some(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    let source = match (participant_evidence, direct_width) {
        (Some(evidence), 0) => SourceArgumentsV1::Participant { evidence },
        (None, 5) => SourceArgumentsV1::DirectTerminal {
            evidence: direct_evidence.expect("complete Direct source"),
            plan: plan.expect("complete Direct source"),
            market_input: market_input.expect("complete Direct source"),
            campaign_evidence: campaign_evidence.expect("complete Direct source"),
            owner: position_owner.expect("complete Direct source"),
        },
        (Some(_), _) => {
            return Err(Error::new(format!(
                "--participant-evidence cannot be combined with the Direct terminal source\n{}",
                usage()
            )));
        }
        (None, _) => {
            return Err(Error::new(format!(
                "supply either --participant-evidence or all five Direct terminal source flags\n{}",
                usage()
            )));
        }
    };
    let rpc_url =
        rpc_url.ok_or_else(|| Error::new(format!("--rpc-url is required\n{}", usage())))?;
    let origin = ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?;
    expected_cluster.authenticate(&origin)?;
    Ok(ArgumentsV1 {
        origin,
        expected_cluster,
        source,
        fee_payer: fee_payer
            .ok_or_else(|| Error::new(format!("--fee-payer is required\n{}", usage())))?,
        position_owner_keypair,
        fee_payer_keypair,
        evidence: evidence
            .ok_or_else(|| Error::new(format!("--evidence is required\n{}", usage())))?,
        execute,
    })
}

#[cfg(test)]
mod tests {
    use solana_sdk::pubkey::Pubkey;

    use super::{SourceArgumentsV1, parse, parse_for_cluster_v1};
    use crate::cluster::{DEVNET_GENESIS_HASH, ExpectedClusterV1};

    fn common() -> Vec<String> {
        vec![
            "--rpc-url".into(),
            "http://127.0.0.1:8899".into(),
            "--fee-payer".into(),
            Pubkey::new_unique().to_string(),
            "--evidence".into(),
            "/tmp/close.json".into(),
        ]
    }

    #[test]
    fn exact_direct_terminal_source_is_one_indivisible_argument_arm() {
        let owner = Pubkey::new_unique();
        let mut arguments = common();
        arguments.extend([
            "--direct-evidence".into(),
            "/tmp/direct.json".into(),
            "--plan".into(),
            "/tmp/plan.json".into(),
            "--market-input".into(),
            "/tmp/market.json".into(),
            "--campaign-evidence".into(),
            "/tmp/campaign.json".into(),
            "--position-owner".into(),
            owner.to_string(),
        ]);
        let parsed = parse(arguments).expect("complete Direct terminal arm");
        assert!(matches!(
            parsed.source,
            SourceArgumentsV1::DirectTerminal { owner: parsed, .. } if parsed == owner
        ));
    }

    #[test]
    fn partial_or_ambiguous_terminal_sources_refuse_before_rpc() {
        let mut partial = common();
        partial.extend([
            "--direct-evidence".into(),
            "/tmp/direct.json".into(),
            "--plan".into(),
            "/tmp/plan.json".into(),
        ]);
        assert!(parse(partial).is_err());

        let mut ambiguous = common();
        ambiguous.extend([
            "--participant-evidence".into(),
            "/tmp/participant.json".into(),
            "--direct-evidence".into(),
            "/tmp/direct.json".into(),
            "--plan".into(),
            "/tmp/plan.json".into(),
            "--market-input".into(),
            "/tmp/market.json".into(),
            "--campaign-evidence".into(),
            "/tmp/campaign.json".into(),
            "--position-owner".into(),
            Pubkey::new_unique().to_string(),
        ]);
        assert!(parse(ambiguous).is_err());
    }

    #[test]
    fn devnet_arm_requires_its_exact_origin_and_acknowledgment() {
        let mut exact = common();
        exact[1] = "https://api.devnet.solana.com:443/".into();
        exact.extend([
            "--i-mean-devnet".into(),
            DEVNET_GENESIS_HASH.into(),
            "--participant-evidence".into(),
            "/tmp/participant.json".into(),
        ]);
        let parsed = parse_for_cluster_v1(exact.clone(), ExpectedClusterV1::Devnet)
            .expect("exact public-devnet close arm");
        assert_eq!(parsed.expected_cluster, ExpectedClusterV1::Devnet);

        let mut missing_ack = exact.clone();
        let acknowledgement = missing_ack
            .iter()
            .position(|value| value == "--i-mean-devnet")
            .unwrap();
        missing_ack.drain(acknowledgement..=acknowledgement + 1);
        assert!(
            parse_for_cluster_v1(missing_ack, ExpectedClusterV1::Devnet).is_err(),
            "public devnet is never inferred from a URL"
        );

        assert!(
            parse_for_cluster_v1(exact, ExpectedClusterV1::OwnedLoopback).is_err(),
            "the owned-loopback command cannot accept a devnet identity"
        );
    }
}
