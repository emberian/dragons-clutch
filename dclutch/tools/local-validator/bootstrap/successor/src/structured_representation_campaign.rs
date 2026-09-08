//! Executed holder campaign for the selected Structured representation release.
//!
//! Receipt and coordinate activation create the protocol-owned resources. This
//! module starts where that lifecycle ends: it acquires the real founding
//! holder, materializes only the holder-local ATA/replay inputs, constructs the
//! four open representation actions from one finalized snapshot per action,
//! submits them through the selected Hot route, and reads their exact
//! poststates back. The fee payer and holder remain separate actors.

use std::collections::{BTreeMap, BTreeSet};

use dclutch_claims::{
    composition::{
        COMPOSITION_EXPOSURE_SCHEMA_ID_V3, CompositionExposureBundleV3, RecordAdmissionV3,
    },
    liability_basis_state_v2::{LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2},
    protocol_position_v2::{ProtocolPositionClaimsCapabilitySeedsV2, ProtocolPositionSeedsV2},
    rational::{
        CallerRoleV2, RATIONAL_REPLAY_BYTES_V2, RATIONAL_REPLAY_SEED_V2,
        RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, RATIONAL_SHARD_MINT_SEED_V2,
        RATIONAL_STRUCTURED_CUSTODY_SEED_V2, RationalReplayV2, RepresentationActionV2,
        TokenBehaviorRecordAdmissionV2, authenticate_token_behavior_v2,
    },
    rational_kernel::{
        DescriptorAdmissionV2, REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
        RepresentationDescriptorV2,
    },
};
use dclutch_custody::{
    CustodyReplayV1,
    token_svm::{MINT_BYTES, Mint, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, TokenAccount},
};
use dclutch_market::{
    CoreState,
    capability_program::hot_v3::{
        DIRECT_HOT_HEAP_FRAME_BYTES_V1, HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
        HOT_CAPABILITY_SEAL_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
        HOT_EFFECT_RAW_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3, HOT_LIFECYCLE_RAW_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
        HOT_STRATEGY_RAW_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3, HOT_TRANSITION_RAW_ACCOUNT_V3,
    },
    capability_program::v4::{
        CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
    },
    realm::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1},
};
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    bearer::{
        CheckedRationalHotOuterReleaseV3, RationalOpenSelectedHotBundleV3,
        RationalOpenStructuredHotBundleV3, RationalTerminalHotBundleV3, RationalTerminalHotStateV3,
        build_rational_open_selected_hot_instruction_v3,
        build_rational_open_structured_hot_instruction_v3,
        build_rational_terminal_hot_instruction_v3, construct_chain_hot_denominate_v3,
        construct_chain_hot_issue_structured_v3, construct_chain_hot_reconstitute_v3,
        construct_chain_hot_redeem_terminal_v3, construct_chain_hot_unwrap_structured_v3,
    },
    capability_seal_v1::{CapabilitySealInstructionInputV1, capability_seal_instruction_v1},
    observation::decode_rent,
    rational_lifecycle_hot::CheckedRationalLifecycleHotOuterV3,
    rational_representation::{
        AssetObservationV2, FinalizedRecordObservationV2, ObservedAccountV2,
        ProductEvidenceObservationV2, RationalObservationV2, ReplayObservationV2,
        SelectedActionInputV2, StructuredActionInputV2, TerminalObservationV2,
    },
    structured_selected_release_v1::StructuredSelectedReleaseV1,
    wallet_terminal_input::{associated_token_account_program_v1, associated_token_account_v1},
};
use dclutch_registry::{
    ActivatedExecutionReleaseSetV1,
    record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId},
    release_set::ExecutionRoleV1,
};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signer},
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::transfer;

use crate::{
    Error, Result,
    campaign::CampaignTerminalEvidenceV1,
    model::{SuccessorPlan, TransactionEvidence},
    plan::pubkey,
    rpc::{Rpc, RpcAccount},
    selected_capability_activation::SelectedActivationRecordPairV1,
};

/// Authenticated values retained by the receipt activation caller.
pub(crate) struct StructuredRepresentationCampaignInputV1<'a> {
    pub(crate) rpc: &'a mut Rpc,
    pub(crate) fee_payer: &'a Keypair,
    pub(crate) actor: &'a Keypair,
    pub(crate) plan: &'a SuccessorPlan,
    pub(crate) campaign: &'a CampaignTerminalEvidenceV1,
    pub(crate) selected_release: &'a StructuredSelectedReleaseV1,
    pub(crate) market: Pubkey,
    pub(crate) root: Pubkey,
    pub(crate) representation_descriptor: SelectedActivationRecordPairV1,
    pub(crate) representation_descriptor_body: &'a [u8],
    pub(crate) composition_exposure: SelectedActivationRecordPairV1,
    pub(crate) composition_exposure_body: &'a [u8],
    pub(crate) hot_fixed: &'a [Pubkey],
    pub(crate) lifecycle_hot_outer: CheckedRationalLifecycleHotOuterV3,
    pub(crate) minimum_finalized_slot: u64,
}

/// One exact decoded representation state, before or after an action.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructuredRepresentationStateEvidenceV1 {
    pub(crate) slot: u64,
    pub(crate) replay_revision: u64,
    pub(crate) aggregate_revision: u64,
    pub(crate) actor_position_revision: u64,
    pub(crate) actor_native_claims: Vec<u64>,
    pub(crate) custody_position_revisions: Vec<u64>,
    pub(crate) custody_native_claims: Vec<Vec<u64>>,
    pub(crate) shard_supplies: Vec<u64>,
    pub(crate) actor_shards: Vec<u64>,
    pub(crate) structured_shards: Vec<u64>,
    pub(crate) receipt_supply: u64,
    pub(crate) actor_receipts: u64,
    pub(crate) account_state_sha256: Vec<String>,
}

/// A finalized Hot action plus both sides of its readback.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructuredRepresentationActionEvidenceV1 {
    pub(crate) action: &'static str,
    pub(crate) outcome: Option<u32>,
    pub(crate) quantity: u64,
    pub(crate) family_digest: String,
    pub(crate) checked_manifest_digest: String,
    pub(crate) signature: String,
    pub(crate) slot: u64,
    pub(crate) before: StructuredRepresentationStateEvidenceV1,
    pub(crate) after: StructuredRepresentationStateEvidenceV1,
}

/// The executable open lifecycle. Terminal redemption is a later continuation
/// because it requires a genuinely resolved Core Market and certificate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructuredRepresentationCampaignEvidenceV1 {
    pub(crate) actor: String,
    pub(crate) fee_payer: String,
    pub(crate) market: String,
    pub(crate) descriptor: String,
    pub(crate) actions: Vec<StructuredRepresentationActionEvidenceV1>,
}

/// Chain coordinates that exist only after Core has accepted a genuine
/// terminal result. The terminal builder authenticates every supplied account
/// against the selected Realm, Core receipt, and Claims/Custody derivations.
pub(crate) struct StructuredRepresentationTerminalInputV1<'a> {
    pub(crate) common: StructuredRepresentationCampaignInputV1<'a>,
    pub(crate) outcome: u32,
    pub(crate) quantity: u64,
    pub(crate) realm: SelectedActivationRecordPairV1,
    pub(crate) realm_body: &'a [u8],
    pub(crate) terminal_certificate: Pubkey,
    pub(crate) custody_replay: Pubkey,
    pub(crate) hoard: Pubkey,
}

/// Exact terminal balances and replay cursors read from one finalized slot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructuredRepresentationTerminalStateEvidenceV1 {
    pub(crate) slot: u64,
    pub(crate) representation_replay_revision: u64,
    pub(crate) custody_replay_revision: u64,
    pub(crate) aggregate_revision: u64,
    pub(crate) custody_position_revision: u64,
    pub(crate) custody_native_claims: Vec<u64>,
    pub(crate) shard_supply: u64,
    pub(crate) actor_shards: u64,
    pub(crate) structured_shards: u64,
    pub(crate) hoard_collateral: u64,
    pub(crate) recipient_collateral: u64,
    pub(crate) account_state_sha256: Vec<String>,
}

/// One accepted terminal redemption and its exact finalized poststate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructuredRepresentationTerminalEvidenceV1 {
    pub(crate) action: &'static str,
    pub(crate) actor: String,
    pub(crate) fee_payer: String,
    pub(crate) market: String,
    pub(crate) outcome: u32,
    pub(crate) quantity: u64,
    pub(crate) family_digest: String,
    pub(crate) checked_manifest_digest: String,
    pub(crate) signature: String,
    pub(crate) slot: u64,
    pub(crate) before: StructuredRepresentationTerminalStateEvidenceV1,
    pub(crate) after: StructuredRepresentationTerminalStateEvidenceV1,
}

#[derive(Clone, Copy)]
enum OpenActionV1 {
    Denominate { outcome: u32, quantity: u64 },
    Reconstitute { outcome: u32, quantity: u64 },
    Issue { quantity: u64 },
    Unwrap { quantity: u64 },
}

impl OpenActionV1 {
    fn action(self) -> RepresentationActionV2 {
        match self {
            Self::Denominate { .. } => RepresentationActionV2::Denominate,
            Self::Reconstitute { .. } => RepresentationActionV2::Reconstitute,
            Self::Issue { .. } => RepresentationActionV2::IssueStructured,
            Self::Unwrap { .. } => RepresentationActionV2::UnwrapStructured,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Denominate { .. } => "denominate",
            Self::Reconstitute { .. } => "reconstitute",
            Self::Issue { .. } => "issueStructured",
            Self::Unwrap { .. } => "unwrapStructured",
        }
    }

    fn outcome(self) -> Option<u32> {
        match self {
            Self::Denominate { outcome, .. } | Self::Reconstitute { outcome, .. } => Some(outcome),
            Self::Issue { .. } | Self::Unwrap { .. } => None,
        }
    }

    fn quantity(self) -> u64 {
        match self {
            Self::Denominate { quantity, .. }
            | Self::Reconstitute { quantity, .. }
            | Self::Issue { quantity }
            | Self::Unwrap { quantity } => quantity,
        }
    }
}

#[derive(Clone, Copy)]
struct RepresentationIdentitiesV1 {
    claims: Pubkey,
    registry: Pubkey,
    aggregate: Pubkey,
    actor_position: Pubkey,
    replay: Pubkey,
    receipt_mint: Pubkey,
    actor_receipt: Pubkey,
    actor: Pubkey,
}

#[derive(Clone, Copy)]
struct AssetIdentitiesV1 {
    outcome: u32,
    custody_position: Pubkey,
    shard_mint: Pubkey,
    actor_shard: Pubkey,
    structured_custody: Pubkey,
}

struct FinalizedSnapshotV1 {
    slot: u64,
    block_time: i64,
    accounts: BTreeMap<Pubkey, Option<RpcAccount>>,
}

impl FinalizedSnapshotV1 {
    fn observed(&self, key: Pubkey) -> Result<ObservedAccountV2<'_>> {
        match self.accounts.get(&key) {
            Some(Some(account)) => Ok(ObservedAccountV2 {
                key,
                owner: account.owner,
                lamports: account.lamports,
                executable: account.executable,
                data: &account.data,
            }),
            Some(None) => Ok(ObservedAccountV2 {
                key,
                owner: system_program::ID,
                lamports: 0,
                executable: false,
                data: &[],
            }),
            None => Err(Error::new(format!(
                "Structured representation snapshot omitted {key}"
            ))),
        }
    }

    fn required(&self, key: Pubkey, label: &str) -> Result<&RpcAccount> {
        self.accounts
            .get(&key)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                Error::new(format!(
                    "Structured representation snapshot omitted {label}"
                ))
            })
    }

    fn rent(&self) -> Result<Rent> {
        let account = self.required(sysvar::rent::ID, "Rent sysvar")?;
        decode_rent(&ObservedAccount {
            observation: Observation {
                slot: self.slot,
                unix_timestamp: self.block_time,
                finality: Finality::Finalized,
            },
            key: sysvar::rent::ID,
            owner: account.owner,
            lamports: account.lamports,
            executable: account.executable,
            data: account.data.clone(),
        })
        .map_err(|error| Error::new(format!("Structured representation Rent: {error:?}")))
    }
}

#[derive(Clone, Copy)]
struct ArtifactBodiesV1<'a> {
    descriptor: &'a [u8],
    account_profile: &'a [u8],
    request_profile: &'a [u8],
    lifecycle: &'a [u8],
    strategy: &'a [u8],
    transition: &'a [u8],
    effect: &'a [u8],
}

impl<'a> ArtifactBodiesV1<'a> {
    fn selected(bundle: &'a RationalOpenSelectedHotBundleV3) -> Self {
        Self {
            descriptor: &bundle.descriptor,
            account_profile: &bundle.account_profile,
            request_profile: &bundle.request_profile,
            lifecycle: &bundle.lifecycle_policy,
            strategy: &bundle.strategy,
            transition: &bundle.transition,
            effect: &bundle.effect,
        }
    }

    fn structured(bundle: &'a RationalOpenStructuredHotBundleV3) -> Self {
        Self {
            descriptor: &bundle.descriptor,
            account_profile: &bundle.account_profile,
            request_profile: &bundle.request_profile,
            lifecycle: &bundle.lifecycle_policy,
            strategy: &bundle.strategy,
            transition: &bundle.transition,
            effect: &bundle.effect,
        }
    }

    fn terminal(bundle: &'a RationalTerminalHotBundleV3) -> Self {
        Self {
            descriptor: &bundle.descriptor,
            account_profile: &bundle.account_profile,
            request_profile: &bundle.request_profile,
            lifecycle: &bundle.lifecycle_policy,
            strategy: &bundle.strategy,
            transition: &bundle.transition,
            effect: &bundle.effect,
        }
    }
}

/// Execute all four open actions against the selected release.
pub(crate) fn run_structured_representation_campaign_v1(
    mut input: StructuredRepresentationCampaignInputV1<'_>,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<StructuredRepresentationCampaignEvidenceV1> {
    let registry = pubkey(&input.plan.registry.program_id)?;
    let claims = pubkey(&input.plan.claims.program_id)?;
    let trading = pubkey(&input.plan.trading.program_id)?;
    if input.hot_fixed.len() != HOT_FIXED_ACCOUNT_COUNT_V3
        || input.market == Pubkey::default()
        || input.root == Pubkey::default()
        || input.minimum_finalized_slot == 0
    {
        return Err(Error::new(
            "Structured representation requires a complete Hot38 frame, fee payer, and holder",
        ));
    }
    if input.lifecycle_hot_outer.trading_program != trading {
        return Err(Error::new(
            "Structured representation checked Hot release names another Trading program",
        ));
    }
    require_pair_matches_body_v1(
        registry,
        input.representation_descriptor,
        REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
        input.representation_descriptor_body,
        "representation descriptor",
    )?;
    require_pair_matches_body_v1(
        registry,
        input.composition_exposure,
        COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
        input.composition_exposure_body,
        "composition exposure",
    )?;
    let descriptor_id: [u8; 32] = Sha256::digest(input.representation_descriptor_body).into();
    let representation_authority = Pubkey::find_program_address(
        &[RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, &descriptor_id],
        &claims,
    )
    .0;
    let descriptor = RepresentationDescriptorV2::decode(
        input.representation_descriptor_body,
        DescriptorAdmissionV2 {
            selected_descriptor_id: descriptor_id,
            finalized_descriptor_id: descriptor_id,
            recomputed_descriptor_digest: descriptor_id,
            finalized_descriptor_digest: descriptor_id,
            record_authenticated: true,
            derived_representation_authority: representation_authority.to_bytes(),
            authority_derivation_authenticated: true,
        },
    )
    .map_err(|error| Error::new(format!("Structured representation descriptor: {error:?}")))?;
    if descriptor.market_id() != input.market.to_bytes()
        || descriptor.release_set_id() != input.selected_release.publication.release_set
        || descriptor.outcome_count() != input.selected_release.publication.outcome_count
    {
        return Err(Error::new(
            "Structured representation descriptor differs from Market-selected release",
        ));
    }
    let exposure_id: [u8; 32] = Sha256::digest(input.composition_exposure_body).into();
    let exposure = CompositionExposureBundleV3::decode(
        input.composition_exposure_body,
        RecordAdmissionV3 {
            selected_id: descriptor.graph_id(),
            finalized_id: exposure_id,
            recomputed_digest: exposure_id,
            finalized_digest: exposure_id,
            record_authenticated: true,
        },
    )
    .map_err(|error| Error::new(format!("Structured representation exposure: {error:?}")))?;
    descriptor
        .authenticate_exposure(exposure)
        .map_err(|error| {
            Error::new(format!(
                "Structured representation descriptor/exposure: {error:?}"
            ))
        })?;

    let aggregate = campaign_address_v1(input.campaign, "claims_aggregate")?;
    let actor_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), input.actor.pubkey().to_bytes())
            .map_err(|error| Error::new(format!("Structured actor Position seeds: {error:?}")))?
            .as_slices(),
        &claims,
    )
    .0;
    // The actor may be the founding holder or a later participant. The
    // Position address is derived from the report-authenticated aggregate and
    // the signing key; its body is authenticated below. No report-projected
    // owner is allowed to replace the wallet identity.
    let replay = Pubkey::find_program_address(
        &[
            RATIONAL_REPLAY_SEED_V2,
            &descriptor_id,
            input.actor.pubkey().as_ref(),
        ],
        &claims,
    )
    .0;
    let receipt_mint = Pubkey::new_from_array(descriptor.receipt_mint());
    let token_program = Pubkey::new_from_array(descriptor.token_program());
    let actor_receipt =
        associated_token_account_v1(input.actor.pubkey(), receipt_mint, token_program);
    let identities = RepresentationIdentitiesV1 {
        claims,
        registry,
        aggregate,
        actor_position,
        replay,
        receipt_mint,
        actor_receipt,
        actor: input.actor.pubkey(),
    };
    let assets = derive_assets_v1(descriptor, identities, input.actor.pubkey())?;
    materialize_holder_inputs_v1(
        input.rpc,
        input.fee_payer,
        descriptor,
        identities,
        &assets,
        input.minimum_finalized_slot,
        transactions,
    )?;

    let initial = snapshot_state_v1(
        input.rpc,
        descriptor,
        identities,
        &assets,
        input.minimum_finalized_slot,
    )?;
    if initial
        .actor_native_claims
        .iter()
        .all(|balance| *balance == 0)
    {
        return Err(Error::new(
            "Structured representation founder Position has no native claim to denominate",
        ));
    }
    let (probe_outcome, probe_quantity) = first_supported_outcome_v1(exposure, &initial)?;
    let mut actions = Vec::new();
    for action in [
        OpenActionV1::Denominate {
            outcome: probe_outcome,
            quantity: probe_quantity,
        },
        OpenActionV1::Reconstitute {
            outcome: probe_outcome,
            quantity: probe_quantity,
        },
    ] {
        actions.push(execute_open_action_v1(
            &mut input,
            descriptor,
            identities,
            &assets,
            action,
            transactions,
        )?);
    }
    for outcome in 0..descriptor.outcome_count() {
        let coefficient = descriptor.coefficient(outcome).map_err(|error| {
            Error::new(format!(
                "Structured representation coefficient {outcome}: {error:?}"
            ))
        })?;
        if coefficient == 0 {
            continue;
        }
        let quantum = denomination_quantum_v1(exposure, outcome)?;
        let shards_per_quantum = descriptor
            .denominator()
            .checked_mul(quantum)
            .ok_or_else(|| Error::new("Structured denomination quantum overflows"))?;
        let multiples = coefficient
            .checked_add(shards_per_quantum - 1)
            .and_then(|value| value.checked_div(shards_per_quantum))
            .ok_or_else(|| {
                Error::new("Structured representation denomination quantity overflow")
            })?;
        let quantity = multiples.checked_mul(quantum).ok_or_else(|| {
            Error::new("Structured representation denomination quantity overflow")
        })?;
        actions.push(execute_open_action_v1(
            &mut input,
            descriptor,
            identities,
            &assets,
            OpenActionV1::Denominate { outcome, quantity },
            transactions,
        )?);
    }
    for action in [
        OpenActionV1::Issue { quantity: 1 },
        OpenActionV1::Unwrap { quantity: 1 },
    ] {
        actions.push(execute_open_action_v1(
            &mut input,
            descriptor,
            identities,
            &assets,
            action,
            transactions,
        )?);
    }
    Ok(StructuredRepresentationCampaignEvidenceV1 {
        actor: input.actor.pubkey().to_string(),
        fee_payer: input.fee_payer.pubkey().to_string(),
        market: input.market.to_string(),
        descriptor: hex32_v1(descriptor_id),
        actions,
    })
}

/// Redeem one actor-owned shard coordinate after Core reaches Terminal.
///
/// This continuation deliberately takes the terminal certificate and
/// Claims-role Custody coordinates from the preceding resolution producer.
/// The public operator authenticates them from one finalized snapshot; report
/// JSON never supplies token amounts, revisions, payout, or account bodies.
pub(crate) fn run_structured_representation_terminal_v1(
    mut input: StructuredRepresentationTerminalInputV1<'_>,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<StructuredRepresentationTerminalEvidenceV1> {
    let trading = pubkey(&input.common.plan.trading.program_id)?;
    if input.quantity == 0
        || input.common.hot_fixed.len() != HOT_FIXED_ACCOUNT_COUNT_V3
        || input.common.market == Pubkey::default()
        || input.common.root == Pubkey::default()
        || input.common.minimum_finalized_slot == 0
    {
        return Err(Error::new(
            "Structured terminal redemption requires a complete input and nonzero quantity",
        ));
    }
    if input.common.lifecycle_hot_outer.trading_program != trading {
        return Err(Error::new(
            "Structured terminal checked Hot release names another Trading program",
        ));
    }
    let registry = pubkey(&input.common.plan.registry.program_id)?;
    let claims = pubkey(&input.common.plan.claims.program_id)?;
    require_pair_matches_body_v1(
        registry,
        input.common.representation_descriptor,
        REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
        input.common.representation_descriptor_body,
        "representation descriptor",
    )?;
    require_pair_matches_body_v1(
        registry,
        input.common.composition_exposure,
        COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
        input.common.composition_exposure_body,
        "composition exposure",
    )?;
    require_pair_matches_body_v1(
        registry,
        input.realm,
        REALM_SCHEMA_RELEASE_ID_V1,
        input.realm_body,
        "Realm",
    )?;
    let descriptor_id: [u8; 32] =
        Sha256::digest(input.common.representation_descriptor_body).into();
    let representation_authority = Pubkey::find_program_address(
        &[RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, &descriptor_id],
        &claims,
    )
    .0;
    let descriptor = RepresentationDescriptorV2::decode(
        input.common.representation_descriptor_body,
        DescriptorAdmissionV2 {
            selected_descriptor_id: descriptor_id,
            finalized_descriptor_id: descriptor_id,
            recomputed_descriptor_digest: descriptor_id,
            finalized_descriptor_digest: descriptor_id,
            record_authenticated: true,
            derived_representation_authority: representation_authority.to_bytes(),
            authority_derivation_authenticated: true,
        },
    )
    .map_err(|error| Error::new(format!("Structured terminal descriptor: {error:?}")))?;
    if descriptor.market_id() != input.common.market.to_bytes()
        || descriptor.release_set_id() != input.common.selected_release.publication.release_set
        || descriptor.outcome_count() != input.common.selected_release.publication.outcome_count
        || input.outcome >= descriptor.outcome_count()
    {
        return Err(Error::new(
            "Structured terminal descriptor, Market, release, or outcome differs",
        ));
    }
    let realm_digest: [u8; 32] = Sha256::digest(input.realm_body).into();
    if realm_digest != input.common.selected_release.publication.realm {
        return Err(Error::new(
            "Structured terminal Realm differs from selected release",
        ));
    }
    let realm = RealmV1::decode(input.realm_body)
        .map_err(|error| Error::new(format!("Structured terminal Realm: {error:?}")))?;
    let collateral_mint = Pubkey::new_from_array(*realm.collateral_mint());
    let collateral_token_program = Pubkey::new_from_array(*realm.token_program());
    let collateral_recipient = associated_token_account_v1(
        input.common.actor.pubkey(),
        collateral_mint,
        collateral_token_program,
    );
    materialize_terminal_recipient_v1(
        input.common.rpc,
        input.common.fee_payer,
        input.common.actor.pubkey(),
        collateral_mint,
        collateral_token_program,
        collateral_recipient,
        input.common.minimum_finalized_slot,
        transactions,
    )?;

    let aggregate = campaign_address_v1(input.common.campaign, "claims_aggregate")?;
    let actor_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), input.common.actor.pubkey().to_bytes())
            .map_err(|error| Error::new(format!("Structured terminal actor seeds: {error:?}")))?
            .as_slices(),
        &claims,
    )
    .0;
    let replay = Pubkey::find_program_address(
        &[
            RATIONAL_REPLAY_SEED_V2,
            &descriptor_id,
            input.common.actor.pubkey().as_ref(),
        ],
        &claims,
    )
    .0;
    let receipt_mint = Pubkey::new_from_array(descriptor.receipt_mint());
    let identities = RepresentationIdentitiesV1 {
        claims,
        registry,
        aggregate,
        actor_position,
        replay,
        receipt_mint,
        actor_receipt: associated_token_account_v1(
            input.common.actor.pubkey(),
            receipt_mint,
            Pubkey::new_from_array(descriptor.token_program()),
        ),
        actor: input.common.actor.pubkey(),
    };
    let assets = derive_assets_v1(descriptor, identities, identities.actor)?;
    let selected_asset = *assets
        .get(usize::try_from(input.outcome).unwrap_or(usize::MAX))
        .ok_or_else(|| Error::new("Structured terminal selected asset is absent"))?;
    let terminal_bundle = input.common.selected_release.terminal.clone();
    let fixed = action_fixed_v1(
        &mut input.common,
        ArtifactBodiesV1::terminal(&terminal_bundle),
        RepresentationActionV2::RedeemTerminal,
    )?;
    let addresses = terminal_snapshot_addresses_v1(
        &fixed,
        input.common.representation_descriptor,
        input.common.composition_exposure,
        input.realm,
        identities,
        selected_asset,
        input.terminal_certificate,
        input.custody_replay,
        collateral_mint,
        input.hoard,
        collateral_recipient,
    );
    let snapshot = finalized_snapshot_v1(
        input.common.rpc,
        &addresses,
        input.common.minimum_finalized_slot,
    )?;
    let before = terminal_state_evidence_v1(
        &snapshot,
        descriptor,
        identities,
        selected_asset,
        input.custody_replay,
        input.hoard,
        collateral_recipient,
    )?;
    let core = CoreState::decode(&snapshot.required(input.common.market, "Core Market")?.data)
        .map_err(|error| Error::new(format!("Structured terminal Market: {error:?}")))?;
    let program_set = input
        .common
        .selected_release
        .open_action_program_set()
        .map_err(|error| Error::new(format!("Structured terminal ProgramSet: {error:?}")))?;
    let selection_digest: [u8; 32] = Sha256::digest(&input.common.selected_release.config).into();
    let authenticated_token_behavior = authenticate_token_behavior_v2(
        descriptor,
        core.identity.realm_id.to_bytes(),
        &input.common.selected_release.config,
        TokenBehaviorRecordAdmissionV2 {
            selected_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            finalized_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            selected_content_digest: selection_digest,
            finalized_content_digest: selection_digest,
            recomputed_content_digest: selection_digest,
            record_authenticated: true,
            market_realm_authenticated: true,
        },
    )
    .map_err(|error| Error::new(format!("Structured terminal token behavior: {error:?}")))?;
    let rent = snapshot.rent()?;
    let observed_assets = [asset_observation_v1(&snapshot, selected_asset)?];
    let observation = RationalObservationV2 {
        caller_role: CallerRoleV2::Trading,
        registry_program: identities.registry,
        activation_cache: snapshot.observed(pubkey(&input.common.plan.activation)?)?,
        descriptor: finalized_record_v1(
            &snapshot,
            input.common.representation_descriptor,
            identities.registry,
            "representation descriptor",
        )?,
        graph: finalized_record_v1(
            &snapshot,
            input.common.composition_exposure,
            identities.registry,
            "composition exposure",
        )?,
        product_evidence: ProductEvidenceObservationV2 {
            linked_basis: record_from_fixed_v1(
                &snapshot,
                &fixed,
                HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
                dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            )?,
            product: record_from_fixed_v1(
                &snapshot,
                &fixed,
                HOT_PRODUCT_RAW_ACCOUNT_V3,
                dclutch_product::admission::PRODUCT_RECORD_SCHEMA_ID_V2,
            )?,
            result_domain: record_from_fixed_v1(
                &snapshot,
                &fixed,
                HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3,
                dclutch_product::admission::RESULT_DOMAIN_SCHEMA_ID_V2,
            )?,
            portfolio: record_from_fixed_v1(
                &snapshot,
                &fixed,
                HOT_PORTFOLIO_RAW_ACCOUNT_V3,
                dclutch_product::admission::PORTFOLIO_SCHEMA_ID_V2,
            )?,
        },
        core_market: snapshot.observed(input.common.market)?,
        claims_aggregate: snapshot.observed(identities.aggregate)?,
        replay: ReplayObservationV2 {
            account: snapshot.observed(identities.replay)?,
        },
        receipt_mint: snapshot.observed(identities.receipt_mint)?,
        actor_receipt_account: None,
        actor_claims_position: None,
        assets: &observed_assets,
        actor: identities.actor,
        parent_context: [0; 32],
        rent: &rent,
    };
    let planned = construct_chain_hot_redeem_terminal_v3(
        observation,
        TerminalObservationV2 {
            outcome: input.outcome,
            quantity: input.quantity,
            realm: finalized_record_v1(&snapshot, input.realm, identities.registry, "Realm")?,
            terminal_certificate: snapshot.observed(input.terminal_certificate)?,
            custody_replay: snapshot.observed(input.custody_replay)?,
            collateral_mint: snapshot.observed(collateral_mint)?,
            hoard: snapshot.observed(input.hoard)?,
            collateral_recipient: snapshot.observed(collateral_recipient)?,
        },
    )
    .map_err(|error| Error::new(format!("Structured terminal plan: {error:?}")))?;
    let metas = fixed
        .iter()
        .enumerate()
        .map(|(index, key)| {
            if index == HOT_ROOT_ACCOUNT_V3 {
                AccountMeta::new(*key, false)
            } else {
                AccountMeta::new_readonly(*key, false)
            }
        })
        .collect::<Vec<_>>();
    let state = RationalTerminalHotStateV3 {
        fixed_accounts: &metas,
        strategy_accounts: &[],
        root_data: &snapshot
            .required(input.common.root, "capability root")?
            .data,
        market_data: &snapshot.required(input.common.market, "Core Market")?.data,
        activation_cache_data: &snapshot
            .required(pubkey(&input.common.plan.activation)?, "activation cache")?
            .data,
        release_set: core.identity.selected_release_set.to_bytes(),
        market: input.common.market,
        generation: core.identity.generation,
        finalized_slot: snapshot.slot,
        hot_outer: Some(CheckedRationalHotOuterReleaseV3 {
            trading_program: input.common.lifecycle_hot_outer.trading_program,
            artifact_release: input.common.lifecycle_hot_outer.artifact_release,
            checked_manifest_digest: input.common.lifecycle_hot_outer.checked_manifest_digest,
        }),
    };
    let built = build_rational_terminal_hot_instruction_v3(
        &state,
        &planned,
        &terminal_bundle,
        &program_set,
        authenticated_token_behavior,
    )
    .map_err(|error| Error::new(format!("Structured terminal Hot build: {error:?}")))?;
    require_actor_signer_v1(&built.required_wallet_signers, identities.actor)?;
    materialize_action_seal_v1(
        &mut input.common,
        &fixed,
        RepresentationActionV2::RedeemTerminal,
        &built.instruction,
        transactions,
    )?;
    let routing = instruction_addresses_v1(&built.instruction);
    let (observation_slot, tables) = crate::market::publish_routing_table_over_v1(
        input.common.rpc,
        input.common.fee_payer,
        "STRUCTURED-REPRESENTATION-redeem-terminal",
        &routing,
        transactions,
    )?;
    let actor_signers = (input.common.fee_payer.pubkey() != input.common.actor.pubkey())
        .then_some(input.common.actor)
        .into_iter()
        .collect::<Vec<_>>();
    let sent = input.common.rpc.send_v0_on_founding_heap_with_signers(
        "Structured representation terminal redemption",
        std::slice::from_ref(&built.instruction),
        input.common.fee_payer,
        &actor_signers,
        observation_slot,
        &tables,
    )?;
    if let Some(error) = sent.error.as_ref() {
        return Err(Error::new(format!(
            "Structured terminal redemption refused on chain: {error}"
        )));
    }
    let sent_slot = sent.slot;
    let signature = sent.signature.clone();
    transactions.push(sent);
    let after_snapshot = finalized_snapshot_v1(input.common.rpc, &addresses, sent_slot)?;
    let after = terminal_state_evidence_v1(
        &after_snapshot,
        descriptor,
        identities,
        selected_asset,
        input.custody_replay,
        input.hoard,
        collateral_recipient,
    )?;
    verify_terminal_poststate_v1(
        descriptor.denominator(),
        input.outcome,
        input.quantity,
        &before,
        &after,
    )?;
    Ok(StructuredRepresentationTerminalEvidenceV1 {
        action: "redeemTerminal",
        actor: identities.actor.to_string(),
        fee_payer: input.common.fee_payer.pubkey().to_string(),
        market: input.common.market.to_string(),
        outcome: input.outcome,
        quantity: input.quantity,
        family_digest: hex32_v1(built.family_digest),
        checked_manifest_digest: hex32_v1(built.checked_manifest_digest),
        signature,
        slot: sent_slot,
        before,
        after,
    })
}

#[allow(clippy::too_many_arguments)]
fn terminal_snapshot_addresses_v1(
    fixed: &[Pubkey],
    representation_descriptor: SelectedActivationRecordPairV1,
    composition_exposure: SelectedActivationRecordPairV1,
    realm: SelectedActivationRecordPairV1,
    identities: RepresentationIdentitiesV1,
    asset: AssetIdentitiesV1,
    terminal_certificate: Pubkey,
    custody_replay: Pubkey,
    collateral_mint: Pubkey,
    hoard: Pubkey,
    recipient: Pubkey,
) -> Vec<Pubkey> {
    let mut addresses = fixed.to_vec();
    addresses.extend([
        representation_descriptor.raw,
        representation_descriptor.staging,
        composition_exposure.raw,
        composition_exposure.staging,
        realm.raw,
        realm.staging,
        identities.aggregate,
        identities.replay,
        identities.receipt_mint,
        asset.custody_position,
        asset.shard_mint,
        asset.actor_shard,
        asset.structured_custody,
        terminal_certificate,
        custody_replay,
        collateral_mint,
        hoard,
        recipient,
        sysvar::rent::ID,
    ]);
    addresses
}

#[allow(clippy::too_many_arguments)]
fn materialize_terminal_recipient_v1(
    rpc: &mut Rpc,
    fee_payer: &Keypair,
    actor: Pubkey,
    collateral_mint: Pubkey,
    token_program: Pubkey,
    recipient: Pubkey,
    minimum_slot: u64,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<()> {
    let snapshot = finalized_snapshot_v1(rpc, &[recipient, collateral_mint], minimum_slot)?;
    if let Some(account) = snapshot.accounts.get(&recipient).and_then(Option::as_ref) {
        let token = parse_token_v1(account, "terminal collateral recipient")?;
        if account.owner != token_program
            || account.executable
            || token.mint != collateral_mint.to_bytes()
            || token.owner != actor.to_bytes()
        {
            return Err(Error::new(
                "Structured terminal collateral recipient is not the actor's canonical ATA",
            ));
        }
        return Ok(());
    }
    let instruction = create_associated_token_account_idempotent_v1(
        fee_payer.pubkey(),
        actor,
        collateral_mint,
        recipient,
        token_program,
    );
    let sent = rpc.send_with_signers(
        "materialize Structured terminal collateral recipient",
        std::slice::from_ref(&instruction),
        fee_payer,
        &[],
    )?;
    if let Some(error) = sent.error.as_ref() {
        return Err(Error::new(format!(
            "Structured terminal recipient materialization refused on chain: {error}"
        )));
    }
    let sent_slot = sent.slot;
    transactions.push(sent);
    let after = finalized_snapshot_v1(rpc, &[recipient], sent_slot)?;
    let account = after.required(recipient, "terminal collateral recipient")?;
    let token = parse_token_v1(account, "terminal collateral recipient")?;
    if account.owner != token_program
        || account.executable
        || token.mint != collateral_mint.to_bytes()
        || token.owner != actor.to_bytes()
    {
        return Err(Error::new(
            "Structured terminal collateral recipient materialization poststate differs",
        ));
    }
    Ok(())
}

fn terminal_state_evidence_v1(
    snapshot: &FinalizedSnapshotV1,
    descriptor: RepresentationDescriptorV2<'_>,
    identities: RepresentationIdentitiesV1,
    asset: AssetIdentitiesV1,
    custody_replay: Pubkey,
    hoard: Pubkey,
    recipient: Pubkey,
) -> Result<StructuredRepresentationTerminalStateEvidenceV1> {
    let replay_account = snapshot.required(identities.replay, "representation replay")?;
    let replay = RationalReplayV2::decode(&replay_account.data)
        .and_then(|value| {
            value.authenticate(descriptor.descriptor_id(), identities.actor.to_bytes())
        })
        .map_err(|error| Error::new(format!("Structured terminal replay: {error:?}")))?;
    if replay_account.owner != identities.claims || replay_account.executable {
        return Err(Error::new("Structured terminal replay owner differs"));
    }
    let aggregate_account = snapshot.required(identities.aggregate, "Claims aggregate")?;
    let aggregate = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Structured terminal aggregate: {error:?}")))?;
    let custody_account = snapshot.required(asset.custody_position, "custody Position")?;
    let custody = LiabilityBasisPositionViewV2::decode(&custody_account.data)
        .map_err(|error| Error::new(format!("Structured terminal custody Position: {error:?}")))?;
    if aggregate_account.owner != identities.claims
        || aggregate.logical_market != descriptor.market_id()
        || custody_account.owner != identities.claims
        || custody.market_account != identities.aggregate.to_bytes()
        || custody.claim_count != aggregate.claim_count
    {
        return Err(Error::new(
            "Structured terminal Claims state differs from descriptor",
        ));
    }
    let custody_native_claims = (0..custody.claim_count)
        .map(|outcome| {
            custody
                .balance(&custody_account.data, outcome)
                .map_err(|error| {
                    Error::new(format!(
                        "Structured terminal custody claim {outcome}: {error:?}"
                    ))
                })
        })
        .collect::<Result<Vec<_>>>()?;
    let mint_account = snapshot.required(asset.shard_mint, "terminal shard Mint")?;
    let mint = parse_mint_base_v1(mint_account, "terminal shard Mint")?;
    let actor_account = snapshot.required(asset.actor_shard, "terminal actor shard ATA")?;
    let actor = parse_token_v1(actor_account, "terminal actor shard ATA")?;
    let structured_account = snapshot.required(asset.structured_custody, "Structured custody")?;
    let structured = parse_token_v1(structured_account, "Structured custody")?;
    if actor.mint != asset.shard_mint.to_bytes()
        || actor.owner != identities.actor.to_bytes()
        || structured.mint != asset.shard_mint.to_bytes()
    {
        return Err(Error::new(
            "Structured terminal selected shard accounts differ",
        ));
    }
    let custody_replay_account = snapshot.required(custody_replay, "Claims Custody replay")?;
    let custody_cursor = CustodyReplayV1::decode(&custody_replay_account.data)
        .map_err(|error| Error::new(format!("Structured terminal Custody replay: {error:?}")))?;
    let hoard_token = parse_token_v1(snapshot.required(hoard, "Hoard")?, "Hoard")?;
    let recipient_token = parse_token_v1(
        snapshot.required(recipient, "terminal collateral recipient")?,
        "terminal collateral recipient",
    )?;
    let keys = [
        identities.aggregate,
        identities.replay,
        asset.custody_position,
        asset.shard_mint,
        asset.actor_shard,
        asset.structured_custody,
        custody_replay,
        hoard,
        recipient,
    ];
    let account_state_sha256 = keys
        .into_iter()
        .map(|key| {
            snapshot
                .required(key, "terminal state hash account")
                .map(|account| account_state_sha256_v1(key, account))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(StructuredRepresentationTerminalStateEvidenceV1 {
        slot: snapshot.slot,
        representation_replay_revision: replay.revision(),
        custody_replay_revision: custody_cursor.next_revision,
        aggregate_revision: aggregate.revision,
        custody_position_revision: custody.revision,
        custody_native_claims,
        shard_supply: mint.supply,
        actor_shards: actor.amount,
        structured_shards: structured.amount,
        hoard_collateral: hoard_token.amount,
        recipient_collateral: recipient_token.amount,
        account_state_sha256,
    })
}

fn verify_terminal_poststate_v1(
    denominator: u64,
    outcome: u32,
    quantity: u64,
    before: &StructuredRepresentationTerminalStateEvidenceV1,
    after: &StructuredRepresentationTerminalStateEvidenceV1,
) -> Result<()> {
    let selected =
        usize::try_from(outcome).map_err(|_| Error::new("Structured terminal outcome overflow"))?;
    let token_delta = denominator
        .checked_mul(quantity)
        .ok_or_else(|| Error::new("Structured terminal shard delta overflow"))?;
    let payout = before
        .hoard_collateral
        .checked_sub(after.hoard_collateral)
        .ok_or_else(|| Error::new("Structured terminal Hoard increased"))?;
    if after.slot < before.slot
        || after.representation_replay_revision
            != before
                .representation_replay_revision
                .checked_add(1)
                .ok_or_else(|| Error::new("Structured terminal replay overflow"))?
        || after.aggregate_revision
            != before
                .aggregate_revision
                .checked_add(1)
                .ok_or_else(|| Error::new("Structured terminal aggregate revision overflow"))?
        || after.custody_position_revision
            != before
                .custody_position_revision
                .checked_add(1)
                .ok_or_else(|| Error::new("Structured terminal Position revision overflow"))?
        || after.shard_supply
            != before
                .shard_supply
                .checked_sub(token_delta)
                .unwrap_or(u64::MAX)
        || after.actor_shards
            != before
                .actor_shards
                .checked_sub(token_delta)
                .unwrap_or(u64::MAX)
        || after.structured_shards != before.structured_shards
        || after.recipient_collateral
            != before
                .recipient_collateral
                .checked_add(payout)
                .unwrap_or(u64::MAX)
        || after.custody_replay_revision
            != before
                .custody_replay_revision
                .checked_add(u64::from(payout != 0))
                .unwrap_or(u64::MAX)
        || before.custody_native_claims.len() != after.custody_native_claims.len()
    {
        return Err(Error::new(
            "Structured terminal finalized readback differs from the exact burn and payout",
        ));
    }
    for index in 0..before.custody_native_claims.len() {
        let expected = if index == selected {
            before.custody_native_claims[index].checked_sub(quantity)
        } else {
            Some(before.custody_native_claims[index])
        };
        if expected != after.custody_native_claims.get(index).copied() {
            return Err(Error::new(format!(
                "Structured terminal native Claim poststate differs at outcome {index}"
            )));
        }
    }
    Ok(())
}

fn derive_assets_v1(
    descriptor: RepresentationDescriptorV2<'_>,
    identities: RepresentationIdentitiesV1,
    actor: Pubkey,
) -> Result<Vec<AssetIdentitiesV1>> {
    (0..descriptor.outcome_count())
        .map(|outcome| {
            let outcome_bytes = outcome.to_le_bytes();
            let custody_owner = Pubkey::find_program_address(
                &ProtocolPositionClaimsCapabilitySeedsV2::new(descriptor.descriptor_id(), outcome)
                    .map_err(|error| {
                        Error::new(format!("Structured custody owner seeds: {error:?}"))
                    })?
                    .as_slices(),
                &identities.claims,
            )
            .0;
            let custody_position = Pubkey::find_program_address(
                &ProtocolPositionSeedsV2::new(
                    identities.aggregate.to_bytes(),
                    custody_owner.to_bytes(),
                )
                .map_err(|error| {
                    Error::new(format!("Structured custody Position seeds: {error:?}"))
                })?
                .as_slices(),
                &identities.claims,
            )
            .0;
            let shard_mint = Pubkey::find_program_address(
                &[
                    RATIONAL_SHARD_MINT_SEED_V2,
                    &descriptor.descriptor_id(),
                    &outcome_bytes,
                ],
                &identities.claims,
            )
            .0;
            Ok(AssetIdentitiesV1 {
                outcome,
                custody_position,
                shard_mint,
                actor_shard: associated_token_account_v1(
                    actor,
                    shard_mint,
                    Pubkey::new_from_array(descriptor.token_program()),
                ),
                structured_custody: Pubkey::find_program_address(
                    &[
                        RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
                        &descriptor.descriptor_id(),
                        &outcome_bytes,
                    ],
                    &identities.claims,
                )
                .0,
            })
        })
        .collect()
}

fn materialize_holder_inputs_v1(
    rpc: &mut Rpc,
    payer: &Keypair,
    descriptor: RepresentationDescriptorV2<'_>,
    identities: RepresentationIdentitiesV1,
    assets: &[AssetIdentitiesV1],
    minimum_slot: u64,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<()> {
    let mut addresses = vec![identities.replay, sysvar::rent::ID];
    addresses.extend(assets.iter().map(|asset| asset.actor_shard));
    addresses.push(identities.actor_receipt);
    let snapshot = finalized_snapshot_v1(rpc, &addresses, minimum_slot)?;
    let rent = snapshot.rent()?;
    let replay = snapshot
        .accounts
        .get(&identities.replay)
        .ok_or_else(|| Error::new("Structured holder setup omitted replay"))?;
    let replay_lamports = replay.as_ref().map_or(0, |account| account.lamports);
    if replay.as_ref().is_some_and(|account| {
        account.owner != system_program::ID || account.executable || !account.data.is_empty()
    }) {
        return Err(Error::new(
            "Structured holder replay exists in a non-vacant prestate",
        ));
    }
    let mut instructions = Vec::new();
    let replay_rent = rent.minimum_balance(RATIONAL_REPLAY_BYTES_V2);
    if replay_lamports < replay_rent {
        instructions.push(transfer(
            &payer.pubkey(),
            &identities.replay,
            replay_rent - replay_lamports,
        ));
    }
    for (account, mint) in assets
        .iter()
        .map(|asset| (asset.actor_shard, asset.shard_mint))
        .chain(std::iter::once((
            identities.actor_receipt,
            identities.receipt_mint,
        )))
    {
        if snapshot.accounts.get(&account).is_some_and(Option::is_none) {
            instructions.push(create_associated_token_account_idempotent_v1(
                payer.pubkey(),
                identities.actor,
                mint,
                account,
                Pubkey::new_from_array(descriptor.token_program()),
            ));
        }
    }
    if !instructions.is_empty() {
        let sent = rpc.send_with_signers(
            "materialize Structured holder replay and token accounts",
            &instructions,
            payer,
            &[],
        )?;
        if let Some(error) = sent.error.as_ref() {
            return Err(Error::new(format!(
                "Structured holder input materialization refused on chain: {error}"
            )));
        }
        let sent_slot = sent.slot;
        transactions.push(sent);
        let after = finalized_snapshot_v1(rpc, &addresses, sent_slot)?;
        let replay = after.required(identities.replay, "funded replay")?;
        if replay.owner != system_program::ID
            || replay.executable
            || !replay.data.is_empty()
            || replay.lamports < replay_rent
        {
            return Err(Error::new(
                "Structured holder replay materialization poststate differs",
            ));
        }
        for (account, mint) in assets
            .iter()
            .map(|asset| (asset.actor_shard, asset.shard_mint))
            .chain(std::iter::once((
                identities.actor_receipt,
                identities.receipt_mint,
            )))
        {
            let token = after.required(account, "holder ATA")?;
            let decoded =
                TokenAccount::parse_base_or_immutable_owner(&token.data).map_err(|error| {
                    Error::new(format!("Structured holder ATA poststate: {error:?}"))
                })?;
            if token.owner != Pubkey::new_from_array(descriptor.token_program())
                || decoded.mint != mint.to_bytes()
                || decoded.owner != identities.actor.to_bytes()
            {
                return Err(Error::new(
                    "Structured holder ATA materialization poststate differs",
                ));
            }
        }
    }
    Ok(())
}

fn create_associated_token_account_idempotent_v1(
    payer: Pubkey,
    owner: Pubkey,
    mint: Pubkey,
    associated: Pubkey,
    token_program: Pubkey,
) -> Instruction {
    Instruction {
        program_id: associated_token_account_program_v1(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(associated, false),
            AccountMeta::new_readonly(owner, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(token_program, false),
        ],
        // SPL Associated Token Account `CreateIdempotent`.
        data: vec![1],
    }
}

fn execute_open_action_v1(
    input: &mut StructuredRepresentationCampaignInputV1<'_>,
    descriptor: RepresentationDescriptorV2<'_>,
    identities: RepresentationIdentitiesV1,
    assets: &[AssetIdentitiesV1],
    action: OpenActionV1,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<StructuredRepresentationActionEvidenceV1> {
    let (bundle_bodies, selected_bundle, structured_bundle) =
        match action.action() {
            RepresentationActionV2::Denominate => {
                let bundle =
                    input.selected_release.selected.first().ok_or_else(|| {
                        Error::new("Structured release omitted Denominate bundle")
                    })?;
                (ArtifactBodiesV1::selected(bundle), Some(bundle), None)
            }
            RepresentationActionV2::Reconstitute => {
                let bundle =
                    input.selected_release.selected.get(1).ok_or_else(|| {
                        Error::new("Structured release omitted Reconstitute bundle")
                    })?;
                (ArtifactBodiesV1::selected(bundle), Some(bundle), None)
            }
            RepresentationActionV2::IssueStructured => {
                let bundle = input.selected_release.structured.first().ok_or_else(|| {
                    Error::new("Structured release omitted IssueStructured bundle")
                })?;
                (ArtifactBodiesV1::structured(bundle), None, Some(bundle))
            }
            RepresentationActionV2::UnwrapStructured => {
                let bundle = input.selected_release.structured.get(1).ok_or_else(|| {
                    Error::new("Structured release omitted UnwrapStructured bundle")
                })?;
                (ArtifactBodiesV1::structured(bundle), None, Some(bundle))
            }
            RepresentationActionV2::RedeemTerminal => {
                return Err(Error::new("terminal action reached open campaign"));
            }
        };
    let fixed = action_fixed_v1(input, bundle_bodies, action.action())?;
    let snapshot = action_snapshot_v1(
        input.rpc,
        &fixed,
        input.representation_descriptor,
        input.composition_exposure,
        identities,
        assets,
        action,
        input.minimum_finalized_slot,
    )?;
    let before = state_evidence_from_snapshot_v1(&snapshot, descriptor, identities, assets)?;
    let core = CoreState::decode(&snapshot.required(input.market, "Core Market")?.data)
        .map_err(|error| Error::new(format!("Structured representation Market: {error:?}")))?;
    let program_set = input
        .selected_release
        .open_action_program_set()
        .map_err(|error| Error::new(format!("Structured open ProgramSet: {error:?}")))?;
    let selection_digest: [u8; 32] = Sha256::digest(&input.selected_release.config).into();
    let authenticated_token_behavior = authenticate_token_behavior_v2(
        descriptor,
        core.identity.realm_id.to_bytes(),
        &input.selected_release.config,
        TokenBehaviorRecordAdmissionV2 {
            selected_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            finalized_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            selected_content_digest: selection_digest,
            finalized_content_digest: selection_digest,
            recomputed_content_digest: selection_digest,
            record_authenticated: true,
            market_realm_authenticated: true,
        },
    )
    .map_err(|error| Error::new(format!("Structured token behavior: {error:?}")))?;
    let rent = snapshot.rent()?;
    let selected_asset = action
        .outcome()
        .map(|outcome| {
            assets
                .get(usize::try_from(outcome).unwrap_or(usize::MAX))
                .copied()
                .ok_or_else(|| Error::new("Structured selected outcome is outside descriptor"))
        })
        .transpose()?;
    let observed_assets = match selected_asset {
        Some(asset) => vec![asset_observation_v1(&snapshot, asset)?],
        None => assets
            .iter()
            .copied()
            .map(|asset| asset_observation_v1(&snapshot, asset))
            .collect::<Result<Vec<_>>>()?,
    };
    let observation = RationalObservationV2 {
        caller_role: CallerRoleV2::Trading,
        registry_program: identities.registry,
        activation_cache: snapshot.observed(pubkey(&input.plan.activation)?)?,
        descriptor: finalized_record_v1(
            &snapshot,
            input.representation_descriptor,
            identities.registry,
            "representation descriptor",
        )?,
        graph: finalized_record_v1(
            &snapshot,
            input.composition_exposure,
            identities.registry,
            "composition exposure",
        )?,
        product_evidence: ProductEvidenceObservationV2 {
            linked_basis: record_from_fixed_v1(
                &snapshot,
                &fixed,
                HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
                dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            )?,
            product: record_from_fixed_v1(
                &snapshot,
                &fixed,
                HOT_PRODUCT_RAW_ACCOUNT_V3,
                dclutch_product::admission::PRODUCT_RECORD_SCHEMA_ID_V2,
            )?,
            result_domain: record_from_fixed_v1(
                &snapshot,
                &fixed,
                HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3,
                dclutch_product::admission::RESULT_DOMAIN_SCHEMA_ID_V2,
            )?,
            portfolio: record_from_fixed_v1(
                &snapshot,
                &fixed,
                HOT_PORTFOLIO_RAW_ACCOUNT_V3,
                dclutch_product::admission::PORTFOLIO_SCHEMA_ID_V2,
            )?,
        },
        core_market: snapshot.observed(input.market)?,
        claims_aggregate: snapshot.observed(identities.aggregate)?,
        replay: ReplayObservationV2 {
            account: snapshot.observed(identities.replay)?,
        },
        receipt_mint: snapshot.observed(identities.receipt_mint)?,
        actor_receipt_account: matches!(
            action.action(),
            RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
        )
        .then_some(snapshot.observed(identities.actor_receipt)?),
        actor_claims_position: matches!(
            action.action(),
            RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute
        )
        .then_some(snapshot.observed(identities.actor_position)?),
        assets: &observed_assets,
        actor: identities.actor,
        parent_context: [0; 32],
        rent: &rent,
    };
    let metas = fixed
        .iter()
        .enumerate()
        .map(|(index, key)| {
            if index == HOT_ROOT_ACCOUNT_V3 {
                AccountMeta::new(*key, false)
            } else {
                AccountMeta::new_readonly(*key, false)
            }
        })
        .collect::<Vec<_>>();
    let state = RationalTerminalHotStateV3 {
        fixed_accounts: &metas,
        strategy_accounts: &[],
        root_data: &snapshot.required(input.root, "capability root")?.data,
        market_data: &snapshot.required(input.market, "Core Market")?.data,
        activation_cache_data: &snapshot
            .required(pubkey(&input.plan.activation)?, "activation cache")?
            .data,
        release_set: core.identity.selected_release_set.to_bytes(),
        market: input.market,
        generation: core.identity.generation,
        finalized_slot: snapshot.slot,
        hot_outer: Some(CheckedRationalHotOuterReleaseV3 {
            trading_program: input.lifecycle_hot_outer.trading_program,
            artifact_release: input.lifecycle_hot_outer.artifact_release,
            checked_manifest_digest: input.lifecycle_hot_outer.checked_manifest_digest,
        }),
    };
    let (hot, family_digest, checked_manifest_digest) = match action {
        OpenActionV1::Denominate { outcome, quantity } => {
            let planned = construct_chain_hot_denominate_v3(
                observation,
                SelectedActionInputV2 { outcome, quantity },
            )
            .map_err(|error| Error::new(format!("Structured Denominate plan: {error:?}")))?;
            let built = build_rational_open_selected_hot_instruction_v3(
                &state,
                &planned,
                selected_bundle.ok_or_else(|| Error::new("Denominate bundle disappeared"))?,
                &program_set,
                authenticated_token_behavior,
            )
            .map_err(|error| Error::new(format!("Structured Denominate Hot build: {error:?}")))?;
            require_actor_signer_v1(&built.required_wallet_signers, identities.actor)?;
            (
                built.instruction,
                built.family_digest,
                built.checked_manifest_digest,
            )
        }
        OpenActionV1::Reconstitute { outcome, quantity } => {
            let planned = construct_chain_hot_reconstitute_v3(
                observation,
                SelectedActionInputV2 { outcome, quantity },
            )
            .map_err(|error| Error::new(format!("Structured Reconstitute plan: {error:?}")))?;
            let built = build_rational_open_selected_hot_instruction_v3(
                &state,
                &planned,
                selected_bundle.ok_or_else(|| Error::new("Reconstitute bundle disappeared"))?,
                &program_set,
                authenticated_token_behavior,
            )
            .map_err(|error| Error::new(format!("Structured Reconstitute Hot build: {error:?}")))?;
            require_actor_signer_v1(&built.required_wallet_signers, identities.actor)?;
            (
                built.instruction,
                built.family_digest,
                built.checked_manifest_digest,
            )
        }
        OpenActionV1::Issue { quantity } => {
            let planned = construct_chain_hot_issue_structured_v3(
                observation,
                StructuredActionInputV2 { quantity },
            )
            .map_err(|error| Error::new(format!("Structured Issue plan: {error:?}")))?;
            let built = build_rational_open_structured_hot_instruction_v3(
                &state,
                &planned,
                structured_bundle.ok_or_else(|| Error::new("Issue bundle disappeared"))?,
                &program_set,
                authenticated_token_behavior,
            )
            .map_err(|error| Error::new(format!("Structured Issue Hot build: {error:?}")))?;
            require_actor_signer_v1(&built.required_wallet_signers, identities.actor)?;
            (
                built.instruction,
                built.family_digest,
                built.checked_manifest_digest,
            )
        }
        OpenActionV1::Unwrap { quantity } => {
            let planned = construct_chain_hot_unwrap_structured_v3(
                observation,
                StructuredActionInputV2 { quantity },
            )
            .map_err(|error| Error::new(format!("Structured Unwrap plan: {error:?}")))?;
            let built = build_rational_open_structured_hot_instruction_v3(
                &state,
                &planned,
                structured_bundle.ok_or_else(|| Error::new("Unwrap bundle disappeared"))?,
                &program_set,
                authenticated_token_behavior,
            )
            .map_err(|error| Error::new(format!("Structured Unwrap Hot build: {error:?}")))?;
            require_actor_signer_v1(&built.required_wallet_signers, identities.actor)?;
            (
                built.instruction,
                built.family_digest,
                built.checked_manifest_digest,
            )
        }
    };

    materialize_action_seal_v1(input, &fixed, action.action(), &hot, transactions)?;
    let routing = instruction_addresses_v1(&hot);
    let (observation_slot, tables) = crate::market::publish_routing_table_over_v1(
        input.rpc,
        input.fee_payer,
        &format!("STRUCTURED-REPRESENTATION-{}", action.label()),
        &routing,
        transactions,
    )?;
    let actor_signers = (input.fee_payer.pubkey() != input.actor.pubkey())
        .then_some(input.actor)
        .into_iter()
        .collect::<Vec<_>>();
    let sent = input.rpc.send_v0_on_founding_heap_with_signers(
        &format!("Structured representation {}", action.label()),
        std::slice::from_ref(&hot),
        input.fee_payer,
        &actor_signers,
        observation_slot,
        &tables,
    )?;
    if let Some(error) = sent.error.as_ref() {
        return Err(Error::new(format!(
            "Structured representation {} refused on chain: {error}",
            action.label()
        )));
    }
    let sent_slot = sent.slot;
    let signature = sent.signature.clone();
    transactions.push(sent);
    let after = snapshot_state_v1(input.rpc, descriptor, identities, assets, sent_slot)?;
    verify_action_poststate_v1(descriptor, action, &before, &after)?;
    Ok(StructuredRepresentationActionEvidenceV1 {
        action: action.label(),
        outcome: action.outcome(),
        quantity: action.quantity(),
        family_digest: hex32_v1(family_digest),
        checked_manifest_digest: hex32_v1(checked_manifest_digest),
        signature,
        slot: sent_slot,
        before,
        after,
    })
}

fn action_snapshot_v1(
    rpc: &mut Rpc,
    fixed: &[Pubkey],
    representation_descriptor: SelectedActivationRecordPairV1,
    composition_exposure: SelectedActivationRecordPairV1,
    identities: RepresentationIdentitiesV1,
    assets: &[AssetIdentitiesV1],
    action: OpenActionV1,
    minimum_finalized_slot: u64,
) -> Result<FinalizedSnapshotV1> {
    let mut addresses = fixed.to_vec();
    addresses.extend([
        representation_descriptor.raw,
        representation_descriptor.staging,
        composition_exposure.raw,
        composition_exposure.staging,
        identities.aggregate,
        identities.replay,
        identities.receipt_mint,
        identities.actor_position,
        identities.actor_receipt,
        sysvar::rent::ID,
    ]);
    let action_assets: Vec<_> = match action.outcome() {
        Some(outcome) => vec![
            *assets
                .get(usize::try_from(outcome).unwrap_or(usize::MAX))
                .ok_or_else(|| Error::new("Structured selected action omitted asset"))?,
        ],
        None => assets.to_vec(),
    };
    for asset in action_assets {
        addresses.extend([
            asset.custody_position,
            asset.shard_mint,
            asset.actor_shard,
            asset.structured_custody,
        ]);
    }
    finalized_snapshot_v1(rpc, &addresses, minimum_finalized_slot)
}

fn asset_observation_v1(
    snapshot: &FinalizedSnapshotV1,
    asset: AssetIdentitiesV1,
) -> Result<AssetObservationV2<'_>> {
    Ok(AssetObservationV2 {
        outcome: asset.outcome,
        claims_custody_position: snapshot.observed(asset.custody_position)?,
        shard_mint: snapshot.observed(asset.shard_mint)?,
        actor_shard_account: snapshot.observed(asset.actor_shard)?,
        structured_custody_account: snapshot.observed(asset.structured_custody)?,
    })
}

fn finalized_record_v1<'a>(
    snapshot: &'a FinalizedSnapshotV1,
    pair: SelectedActivationRecordPairV1,
    registry: Pubkey,
    label: &str,
) -> Result<FinalizedRecordObservationV2<'a>> {
    let raw = snapshot.observed(pair.raw)?;
    let staging = snapshot.observed(pair.staging)?;
    if raw.owner != registry
        || raw.executable
        || raw.data.is_empty()
        || Sha256::digest(raw.data).as_slice() != pair.content
        || staging.owner != system_program::ID
        || staging.executable
        || !staging.data.is_empty()
    {
        return Err(Error::new(format!(
            "Structured representation {label} is not a finalized canonical record"
        )));
    }
    Ok(FinalizedRecordObservationV2 {
        schema_id: pair.schema,
        raw,
        staging,
    })
}

fn record_from_fixed_v1<'a>(
    snapshot: &'a FinalizedSnapshotV1,
    fixed: &[Pubkey],
    raw_index: usize,
    schema_id: [u8; 32],
) -> Result<FinalizedRecordObservationV2<'a>> {
    Ok(FinalizedRecordObservationV2 {
        schema_id,
        raw: snapshot.observed(*fixed.get(raw_index).ok_or_else(|| {
            Error::new("Structured representation fixed record index is absent")
        })?)?,
        staging: snapshot.observed(*fixed.get(raw_index + 1).ok_or_else(|| {
            Error::new("Structured representation fixed staging index is absent")
        })?)?,
    })
}

fn action_fixed_v1(
    input: &mut StructuredRepresentationCampaignInputV1<'_>,
    bodies: ArtifactBodiesV1<'_>,
    action: RepresentationActionV2,
) -> Result<Vec<Pubkey>> {
    let registry = pubkey(&input.plan.registry.program_id)?;
    let mut fixed = input.hot_fixed.to_vec();
    if fixed.len() != HOT_FIXED_ACCOUNT_COUNT_V3 {
        return Err(Error::new(
            "Structured representation Hot38 frame width differs",
        ));
    }
    let descriptor = CapabilityProgramV4::decode(bodies.descriptor).map_err(|error| {
        Error::new(format!(
            "Structured representation capability descriptor: {error:?}"
        ))
    })?;
    let artifacts = descriptor.artifacts();
    let pairs = [
        selected_pair_v1(registry, CAPABILITY_PROGRAM_SCHEMA_ID_V4, bodies.descriptor)?,
        selected_pair_v1(
            registry,
            artifacts.account_profile.schema().to_bytes(),
            bodies.account_profile,
        )?,
        selected_pair_v1(
            registry,
            artifacts.request_profile.schema().to_bytes(),
            bodies.request_profile,
        )?,
        selected_pair_v1(
            registry,
            artifacts.lifecycle.schema().to_bytes(),
            bodies.lifecycle,
        )?,
        selected_pair_v1(
            registry,
            artifacts.strategy.schema().to_bytes(),
            bodies.strategy,
        )?,
        selected_pair_v1(
            registry,
            artifacts.transition.schema().to_bytes(),
            bodies.transition,
        )?,
        selected_pair_v1(
            registry,
            artifacts.effect.schema().to_bytes(),
            bodies.effect,
        )?,
    ];
    for (index, pair) in [
        HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
        HOT_LIFECYCLE_RAW_ACCOUNT_V3,
        HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_TRANSITION_RAW_ACCOUNT_V3,
        HOT_EFFECT_RAW_ACCOUNT_V3,
    ]
    .into_iter()
    .zip(pairs)
    {
        fixed[index] = pair.raw;
        fixed[index + 1] = pair.staging;
    }
    let program_set = selected_pair_v1(
        registry,
        dclutch_market::capability_program::set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        &input.selected_release.program_set,
    )?;
    fixed[HOT_PROGRAM_SET_RAW_ACCOUNT_V3] = program_set.raw;
    fixed[HOT_PROGRAM_SET_RAW_ACCOUNT_V3 + 1] = program_set.staging;
    let config = selected_pair_v1(
        registry,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        &input.selected_release.config,
    )?;
    fixed[HOT_CONFIG_RAW_ACCOUNT_V3] = config.raw;
    fixed[HOT_CONFIG_RAW_ACCOUNT_V3 + 1] = config.staging;

    let activation = input
        .rpc
        .account(pubkey(&input.plan.activation)?)?
        .ok_or_else(|| Error::new("Structured representation activation cache is absent"))?;
    let activated = ActivatedExecutionReleaseSetV1::decode(&activation.data).map_err(|error| {
        Error::new(format!(
            "Structured representation activation cache: {error:?}"
        ))
    })?;
    let seal_key = dclutch_vm::capability_seal::CapabilitySealKeyV1::new(
        CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        Sha256::digest(bodies.descriptor).into(),
        u32::from(action as u8),
        activated
            .role(ExecutionRoleV1::Trading)
            .release()
            .semantic_release_id()
            .to_bytes(),
        registry.to_bytes(),
    )
    .map_err(|error| Error::new(format!("Structured representation seal key: {error:?}")))?;
    fixed[HOT_CAPABILITY_SEAL_ACCOUNT_V3] = Pubkey::find_program_address(
        &seal_key.seeds().as_slices(),
        &input.lifecycle_hot_outer.trading_program,
    )
    .0;
    if fixed[HOT_MARKET_ACCOUNT_V3] != input.market
        || fixed[HOT_ROOT_ACCOUNT_V3] != input.root
        || fixed[HOT_TRADING_PROGRAM_ACCOUNT_V3] != input.lifecycle_hot_outer.trading_program
        || fixed.iter().any(|key| *key == Pubkey::default())
    {
        return Err(Error::new(
            "Structured representation action frame changed a common Hot coordinate",
        ));
    }
    Ok(fixed)
}

fn materialize_action_seal_v1(
    input: &mut StructuredRepresentationCampaignInputV1<'_>,
    fixed: &[Pubkey],
    action: RepresentationActionV2,
    hot: &Instruction,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<()> {
    let seal = *fixed
        .get(HOT_CAPABILITY_SEAL_ACCOUNT_V3)
        .ok_or_else(|| Error::new("Structured representation frame omitted seal"))?;
    if input.rpc.account(seal)?.is_some() {
        return Ok(());
    }
    let registry = pubkey(&input.plan.registry.program_id)?;
    let activated_account = input
        .rpc
        .account(pubkey(&input.plan.activation)?)?
        .ok_or_else(|| Error::new("Structured representation activation cache vanished"))?;
    let activated =
        ActivatedExecutionReleaseSetV1::decode(&activated_account.data).map_err(|error| {
            Error::new(format!(
                "Structured representation activation cache: {error:?}"
            ))
        })?;
    let descriptor_raw = *fixed
        .get(HOT_DESCRIPTOR_RAW_ACCOUNT_V3)
        .ok_or_else(|| Error::new("Structured representation descriptor coordinate vanished"))?;
    let descriptor_account = input
        .rpc
        .account(descriptor_raw)?
        .ok_or_else(|| Error::new("Structured representation descriptor record vanished"))?;
    let instruction = capability_seal_instruction_v1(CapabilitySealInstructionInputV1 {
        trading_program: input.lifecycle_hot_outer.trading_program,
        registry_program: registry,
        trading_semantic_release: activated
            .role(ExecutionRoleV1::Trading)
            .release()
            .semantic_release_id()
            .to_bytes(),
        descriptor_digest: Sha256::digest(&descriptor_account.data).into(),
        action: u32::from(action as u8),
        fixed_frame: fixed,
        payer: input.fee_payer.pubkey(),
    })
    .map_err(|error| Error::new(format!("Structured representation seal builder: {error:?}")))?
    .instruction;
    let mut routing = instruction_addresses_v1(&instruction);
    routing.extend(instruction_addresses_v1(hot));
    routing.sort_unstable();
    routing.dedup();
    let (observation, tables) = crate::market::publish_routing_table_over_v1(
        input.rpc,
        input.fee_payer,
        &format!("STRUCTURED-REPRESENTATION-SEAL-{}", action as u8),
        &routing,
        transactions,
    )?;
    let sent = input.rpc.send_v0_on_heap(
        "materialize Structured representation capability seal",
        std::slice::from_ref(&instruction),
        input.fee_payer,
        observation,
        &tables,
        DIRECT_HOT_HEAP_FRAME_BYTES_V1,
    )?;
    if let Some(error) = sent.error.as_ref() {
        return Err(Error::new(format!(
            "Structured representation capability seal refused on chain: {error}"
        )));
    }
    transactions.push(sent);
    let live = input
        .rpc
        .account(seal)?
        .ok_or_else(|| Error::new("Structured representation seal transaction left no seal"))?;
    if live.owner != input.lifecycle_hot_outer.trading_program
        || live.executable
        || live.data.is_empty()
    {
        return Err(Error::new(
            "Structured representation capability seal poststate differs",
        ));
    }
    Ok(())
}

fn selected_pair_v1(
    registry: Pubkey,
    schema: [u8; 32],
    body: &[u8],
) -> Result<SelectedActivationRecordPairV1> {
    let content: [u8; 32] = Sha256::digest(body).into();
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema)
            .map_err(|error| Error::new(format!("Structured representation schema: {error:?}")))?,
        ContentDigest::new(content)
            .map_err(|error| Error::new(format!("Structured representation content: {error:?}")))?,
    );
    let derive = |seeds: RecordPdaSeedsV1| {
        Pubkey::find_program_address(
            &[
                seeds.domain(),
                seeds.schema_release_id().as_bytes(),
                seeds.expected_digest().as_bytes(),
            ],
            &registry,
        )
    };
    let (raw, raw_bump) = derive(key.raw_record_pda_seeds());
    let (staging, staging_bump) = derive(key.staging_cursor_pda_seeds());
    Ok(SelectedActivationRecordPairV1 {
        raw,
        staging,
        schema,
        content,
        bumps: [raw_bump, staging_bump],
    })
}

fn require_pair_matches_body_v1(
    registry: Pubkey,
    pair: SelectedActivationRecordPairV1,
    schema: [u8; 32],
    body: &[u8],
    label: &str,
) -> Result<()> {
    if pair != selected_pair_v1(registry, schema, body)? {
        return Err(Error::new(format!(
            "Structured representation {label} coordinates differ from its exact body"
        )));
    }
    Ok(())
}

fn finalized_snapshot_v1(
    rpc: &mut Rpc,
    addresses: &[Pubkey],
    minimum_slot: u64,
) -> Result<FinalizedSnapshotV1> {
    let unique = addresses
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let (slot, accounts) = rpc.finalized_accounts(&unique, minimum_slot)?;
    if slot == 0 || slot < minimum_slot || accounts.len() != unique.len() {
        return Err(Error::new(
            "Structured representation RPC did not return one complete finalized snapshot",
        ));
    }
    Ok(FinalizedSnapshotV1 {
        slot,
        block_time: rpc.block_time(slot)?,
        accounts: unique.into_iter().zip(accounts).collect(),
    })
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

fn instruction_addresses_v1(instruction: &Instruction) -> Vec<Pubkey> {
    let mut addresses = BTreeSet::from([instruction.program_id]);
    addresses.extend(instruction.accounts.iter().map(|account| account.pubkey));
    addresses.into_iter().collect()
}

fn require_actor_signer_v1(required: &[Pubkey], actor: Pubkey) -> Result<()> {
    if required != [actor] {
        return Err(Error::new(format!(
            "Structured Hot builder required wallet signers {required:?}, expected only holder {actor}"
        )));
    }
    Ok(())
}

fn hex32_v1(value: [u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn snapshot_state_v1(
    rpc: &mut Rpc,
    descriptor: RepresentationDescriptorV2<'_>,
    identities: RepresentationIdentitiesV1,
    assets: &[AssetIdentitiesV1],
    minimum_slot: u64,
) -> Result<StructuredRepresentationStateEvidenceV1> {
    let mut addresses = vec![
        identities.aggregate,
        identities.actor_position,
        identities.replay,
        identities.receipt_mint,
        identities.actor_receipt,
    ];
    for asset in assets {
        addresses.extend([
            asset.custody_position,
            asset.shard_mint,
            asset.actor_shard,
            asset.structured_custody,
        ]);
    }
    let snapshot = finalized_snapshot_v1(rpc, &addresses, minimum_slot)?;
    state_evidence_from_snapshot_v1(&snapshot, descriptor, identities, assets)
}

fn state_evidence_from_snapshot_v1(
    snapshot: &FinalizedSnapshotV1,
    descriptor: RepresentationDescriptorV2<'_>,
    identities: RepresentationIdentitiesV1,
    assets: &[AssetIdentitiesV1],
) -> Result<StructuredRepresentationStateEvidenceV1> {
    let aggregate_account = snapshot.required(identities.aggregate, "Claims aggregate")?;
    let aggregate = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Structured Claims aggregate: {error:?}")))?;
    if aggregate.logical_market != descriptor.market_id()
        || aggregate.release_set != descriptor.release_set_id()
        || aggregate.claim_count == 0
    {
        return Err(Error::new(
            "Structured Claims aggregate differs from representation descriptor",
        ));
    }
    let actor_account = snapshot.required(identities.actor_position, "actor Position")?;
    let actor_position = LiabilityBasisPositionViewV2::decode(&actor_account.data)
        .map_err(|error| Error::new(format!("Structured actor Position: {error:?}")))?;
    if actor_account.owner != identities.claims
        || actor_account.executable
        || actor_position.market_account != identities.aggregate.to_bytes()
        || actor_position.owner != identities.actor.to_bytes()
        || actor_position.claim_count != aggregate.claim_count
    {
        return Err(Error::new(
            "Structured actor Position differs from its canonical owner or aggregate",
        ));
    }
    let actor_native_claims = (0..actor_position.claim_count)
        .map(|outcome| {
            actor_position
                .balance(&actor_account.data, outcome)
                .map_err(|error| Error::new(format!("Structured actor claim {outcome}: {error:?}")))
        })
        .collect::<Result<Vec<_>>>()?;

    let replay_account = snapshot.required(identities.replay, "representation replay")?;
    let replay = RationalReplayV2::decode(&replay_account.data)
        .and_then(|value| {
            value.authenticate(descriptor.descriptor_id(), identities.actor.to_bytes())
        })
        .map_err(|error| Error::new(format!("Structured representation replay: {error:?}")))?;
    if replay_account.owner != identities.claims || replay_account.executable {
        return Err(Error::new(
            "Structured representation replay has the wrong owner or executable flag",
        ));
    }
    let receipt_account = snapshot.required(identities.receipt_mint, "receipt Mint")?;
    let receipt = parse_mint_base_v1(receipt_account, "receipt Mint")?;
    let actor_receipt_account = snapshot.required(identities.actor_receipt, "actor receipt ATA")?;
    let actor_receipt = parse_token_v1(actor_receipt_account, "actor receipt ATA")?;
    if receipt_account.owner != Pubkey::new_from_array(descriptor.token_program())
        || actor_receipt_account.owner != Pubkey::new_from_array(descriptor.token_program())
        || actor_receipt.mint != identities.receipt_mint.to_bytes()
        || actor_receipt.owner != identities.actor.to_bytes()
    {
        return Err(Error::new(
            "Structured receipt state differs from descriptor or actor",
        ));
    }

    let mut custody_position_revisions = Vec::with_capacity(assets.len());
    let mut custody_native_claims = Vec::with_capacity(assets.len());
    let mut shard_supplies = Vec::with_capacity(assets.len());
    let mut actor_shards = Vec::with_capacity(assets.len());
    let mut structured_shards = Vec::with_capacity(assets.len());
    for asset in assets {
        let custody_account = snapshot.required(asset.custody_position, "custody Position")?;
        let custody =
            LiabilityBasisPositionViewV2::decode(&custody_account.data).map_err(|error| {
                Error::new(format!(
                    "Structured custody Position {}: {error:?}",
                    asset.outcome
                ))
            })?;
        if custody_account.owner != identities.claims
            || custody_account.executable
            || custody.market_account != identities.aggregate.to_bytes()
            || custody.claim_count != aggregate.claim_count
        {
            return Err(Error::new(format!(
                "Structured custody Position {} differs from aggregate",
                asset.outcome
            )));
        }
        custody_position_revisions.push(custody.revision);
        custody_native_claims.push(
            (0..custody.claim_count)
                .map(|outcome| {
                    custody
                        .balance(&custody_account.data, outcome)
                        .map_err(|error| {
                            Error::new(format!(
                                "Structured custody {} claim {outcome}: {error:?}",
                                asset.outcome
                            ))
                        })
                })
                .collect::<Result<Vec<_>>>()?,
        );
        let mint_account = snapshot.required(asset.shard_mint, "shard Mint")?;
        let mint = parse_mint_base_v1(mint_account, "shard Mint")?;
        let actor_account = snapshot.required(asset.actor_shard, "actor shard ATA")?;
        let actor = parse_token_v1(actor_account, "actor shard ATA")?;
        let structured_account =
            snapshot.required(asset.structured_custody, "Structured custody")?;
        let structured = parse_token_v1(structured_account, "Structured custody")?;
        if mint_account.owner != Pubkey::new_from_array(descriptor.token_program())
            || actor_account.owner != Pubkey::new_from_array(descriptor.token_program())
            || structured_account.owner != Pubkey::new_from_array(descriptor.token_program())
            || actor.mint != asset.shard_mint.to_bytes()
            || actor.owner != identities.actor.to_bytes()
            || structured.mint != asset.shard_mint.to_bytes()
            || structured.owner
                != Pubkey::find_program_address(
                    &[
                        RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
                        &descriptor.descriptor_id(),
                    ],
                    &identities.claims,
                )
                .0
                .to_bytes()
        {
            return Err(Error::new(format!(
                "Structured token coordinate {} differs from descriptor",
                asset.outcome
            )));
        }
        shard_supplies.push(mint.supply);
        actor_shards.push(actor.amount);
        structured_shards.push(structured.amount);
    }
    let account_state_sha256 = addresses_for_state_v1(identities, assets)
        .into_iter()
        .map(|key| {
            snapshot
                .required(key, "state hash account")
                .map(|account| account_state_sha256_v1(key, account))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(StructuredRepresentationStateEvidenceV1 {
        slot: snapshot.slot,
        replay_revision: replay.revision(),
        aggregate_revision: aggregate.revision,
        actor_position_revision: actor_position.revision,
        actor_native_claims,
        custody_position_revisions,
        custody_native_claims,
        shard_supplies,
        actor_shards,
        structured_shards,
        receipt_supply: receipt.supply,
        actor_receipts: actor_receipt.amount,
        account_state_sha256,
    })
}

fn addresses_for_state_v1(
    identities: RepresentationIdentitiesV1,
    assets: &[AssetIdentitiesV1],
) -> Vec<Pubkey> {
    let mut addresses = vec![
        identities.aggregate,
        identities.actor_position,
        identities.replay,
        identities.receipt_mint,
        identities.actor_receipt,
    ];
    for asset in assets {
        addresses.extend([
            asset.custody_position,
            asset.shard_mint,
            asset.actor_shard,
            asset.structured_custody,
        ]);
    }
    addresses
}

fn parse_mint_base_v1(account: &RpcAccount, label: &str) -> Result<Mint> {
    Mint::parse(
        account
            .data
            .get(..MINT_BYTES)
            .ok_or_else(|| Error::new(format!("Structured {label} is truncated")))?,
    )
    .map_err(|error| Error::new(format!("Structured {label}: {error:?}")))
}

fn parse_token_v1(account: &RpcAccount, label: &str) -> Result<TokenAccount> {
    TokenAccount::parse_base_or_immutable_owner(&account.data)
        .map_err(|error| Error::new(format!("Structured {label}: {error:?}")))
}

fn account_state_sha256_v1(key: Pubkey, account: &RpcAccount) -> String {
    let mut digest = Sha256::new();
    digest.update(key.to_bytes());
    digest.update(account.owner.to_bytes());
    digest.update(account.lamports.to_le_bytes());
    digest.update([u8::from(account.executable)]);
    digest.update((account.data.len() as u64).to_le_bytes());
    digest.update(&account.data);
    hex32_v1(digest.finalize().into())
}

fn first_supported_outcome_v1(
    exposure: CompositionExposureBundleV3<'_>,
    state: &StructuredRepresentationStateEvidenceV1,
) -> Result<(u32, u64)> {
    for outcome in 0..exposure.representation_width() {
        let quantity = denomination_quantum_v1(exposure, outcome)?;
        if actor_can_denominate_v1(exposure, state, outcome, quantity)? {
            return Ok((outcome, quantity));
        }
    }
    Err(Error::new(
        "Structured representation holder lacks one exact native exposure row",
    ))
}

fn actor_can_denominate_v1(
    exposure: CompositionExposureBundleV3<'_>,
    state: &StructuredRepresentationStateEvidenceV1,
    outcome: u32,
    quantity: u64,
) -> Result<bool> {
    let row = exposure
        .row(outcome)
        .map_err(|error| Error::new(format!("Structured exposure row {outcome}: {error:?}")))?;
    for index in 0..row.term_count() {
        let term = exposure.row_term(row, index).map_err(|error| {
            Error::new(format!(
                "Structured exposure term {outcome}/{index}: {error:?}"
            ))
        })?;
        let numerator = term
            .numerator
            .checked_mul(quantity)
            .ok_or_else(|| Error::new("Structured exposure quantity overflow"))?;
        if !numerator.is_multiple_of(row.denominator()) {
            return Ok(false);
        }
        let required = numerator / row.denominator();
        if state
            .actor_native_claims
            .get(usize::try_from(term.product_coordinate).unwrap_or(usize::MAX))
            .is_none_or(|balance| *balance < required)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn denomination_quantum_v1(exposure: CompositionExposureBundleV3<'_>, outcome: u32) -> Result<u64> {
    let row = exposure
        .row(outcome)
        .map_err(|error| Error::new(format!("Structured exposure row {outcome}: {error:?}")))?;
    let mut quantum = 1_u64;
    for index in 0..row.term_count() {
        let term = exposure.row_term(row, index).map_err(|error| {
            Error::new(format!(
                "Structured exposure term {outcome}/{index}: {error:?}"
            ))
        })?;
        let term_quantum = row.denominator() / gcd_u64_v1(row.denominator(), term.numerator);
        quantum = lcm_u64_v1(quantum, term_quantum)?;
    }
    Ok(quantum)
}

fn gcd_u64_v1(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn lcm_u64_v1(left: u64, right: u64) -> Result<u64> {
    left.checked_div(gcd_u64_v1(left, right))
        .and_then(|value| value.checked_mul(right))
        .ok_or_else(|| Error::new("Structured exposure denomination quantum overflow"))
}

fn verify_action_poststate_v1(
    descriptor: RepresentationDescriptorV2<'_>,
    action: OpenActionV1,
    before: &StructuredRepresentationStateEvidenceV1,
    after: &StructuredRepresentationStateEvidenceV1,
) -> Result<()> {
    if after.slot == 0
        || after.slot < before.slot
        || after.replay_revision
            != before
                .replay_revision
                .checked_add(1)
                .ok_or_else(|| Error::new("Structured representation replay revision overflow"))?
        || before.actor_native_claims.len() != after.actor_native_claims.len()
        || before.custody_native_claims.len() != after.custody_native_claims.len()
        || before.shard_supplies.len() != after.shard_supplies.len()
        || before.actor_shards.len() != after.actor_shards.len()
        || before.structured_shards.len() != after.structured_shards.len()
    {
        return Err(Error::new(
            "Structured representation finalized readback has inconsistent geometry or replay",
        ));
    }
    match action {
        OpenActionV1::Denominate { outcome, quantity }
        | OpenActionV1::Reconstitute { outcome, quantity } => {
            let selected = usize::try_from(outcome)
                .map_err(|_| Error::new("Structured selected outcome overflow"))?;
            let token_delta = descriptor
                .denominator()
                .checked_mul(quantity)
                .ok_or_else(|| Error::new("Structured selected token delta overflow"))?;
            let aggregate_after = before
                .aggregate_revision
                .checked_add(1)
                .ok_or_else(|| Error::new("Structured aggregate revision overflow"))?;
            let actor_after = before
                .actor_position_revision
                .checked_add(1)
                .ok_or_else(|| Error::new("Structured actor Position revision overflow"))?;
            let custody_after = before
                .custody_position_revisions
                .get(selected)
                .copied()
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| Error::new("Structured custody Position revision overflow"))?;
            if after.aggregate_revision != aggregate_after
                || after.actor_position_revision != actor_after
                || after.custody_position_revisions.get(selected).copied() != Some(custody_after)
                || before.receipt_supply != after.receipt_supply
                || before.actor_receipts != after.actor_receipts
                || before.structured_shards != after.structured_shards
            {
                return Err(Error::new(
                    "Structured selected action did not advance exact Claims and receipt state",
                ));
            }
            for index in 0..before.shard_supplies.len() {
                let expected_supply = if index == selected {
                    match action {
                        OpenActionV1::Denominate { .. } => {
                            before.shard_supplies[index].checked_add(token_delta)
                        }
                        OpenActionV1::Reconstitute { .. } => {
                            before.shard_supplies[index].checked_sub(token_delta)
                        }
                        _ => None,
                    }
                } else {
                    Some(before.shard_supplies[index])
                };
                let expected_actor = if index == selected {
                    match action {
                        OpenActionV1::Denominate { .. } => {
                            before.actor_shards[index].checked_add(token_delta)
                        }
                        OpenActionV1::Reconstitute { .. } => {
                            before.actor_shards[index].checked_sub(token_delta)
                        }
                        _ => None,
                    }
                } else {
                    Some(before.actor_shards[index])
                };
                if expected_supply != after.shard_supplies.get(index).copied()
                    || expected_actor != after.actor_shards.get(index).copied()
                {
                    return Err(Error::new(format!(
                        "Structured selected action token poststate differs at outcome {index}"
                    )));
                }
                if index != selected
                    && (before.custody_position_revisions[index]
                        != after.custody_position_revisions[index]
                        || before.custody_native_claims[index]
                            != after.custody_native_claims[index])
                {
                    return Err(Error::new(format!(
                        "Structured selected action changed unrelated custody outcome {index}"
                    )));
                }
            }
            if before.actor_native_claims == after.actor_native_claims
                || before.custody_native_claims[selected] == after.custody_native_claims[selected]
            {
                return Err(Error::new(
                    "Structured selected action advanced revisions without moving native claims",
                ));
            }
        }
        OpenActionV1::Issue { quantity } | OpenActionV1::Unwrap { quantity } => {
            if before.aggregate_revision != after.aggregate_revision
                || before.actor_position_revision != after.actor_position_revision
                || before.actor_native_claims != after.actor_native_claims
                || before.custody_position_revisions != after.custody_position_revisions
                || before.custody_native_claims != after.custody_native_claims
                || before.shard_supplies != after.shard_supplies
            {
                return Err(Error::new(
                    "Structured receipt action changed native Claims state or shard supply",
                ));
            }
            let (receipt_supply, actor_receipts) = match action {
                OpenActionV1::Issue { .. } => (
                    before.receipt_supply.checked_add(quantity),
                    before.actor_receipts.checked_add(quantity),
                ),
                OpenActionV1::Unwrap { .. } => (
                    before.receipt_supply.checked_sub(quantity),
                    before.actor_receipts.checked_sub(quantity),
                ),
                _ => unreachable!(),
            };
            if receipt_supply != Some(after.receipt_supply)
                || actor_receipts != Some(after.actor_receipts)
            {
                return Err(Error::new(
                    "Structured receipt action Mint or holder balance differs",
                ));
            }
            for outcome in 0..descriptor.outcome_count() {
                let index = usize::try_from(outcome)
                    .map_err(|_| Error::new("Structured receipt outcome overflow"))?;
                let delta = descriptor
                    .coefficient(outcome)
                    .map_err(|error| Error::new(format!("Structured coefficient: {error:?}")))?
                    .checked_mul(quantity)
                    .ok_or_else(|| Error::new("Structured receipt shard delta overflow"))?;
                let (actor, custody) = match action {
                    OpenActionV1::Issue { .. } => (
                        before.actor_shards[index].checked_sub(delta),
                        before.structured_shards[index].checked_add(delta),
                    ),
                    OpenActionV1::Unwrap { .. } => (
                        before.actor_shards[index].checked_add(delta),
                        before.structured_shards[index].checked_sub(delta),
                    ),
                    _ => unreachable!(),
                };
                if actor != after.actor_shards.get(index).copied()
                    || custody != after.structured_shards.get(index).copied()
                {
                    return Err(Error::new(format!(
                        "Structured receipt action shard poststate differs at outcome {outcome}"
                    )));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{StructuredRepresentationTerminalStateEvidenceV1, verify_terminal_poststate_v1};

    fn terminal_state() -> StructuredRepresentationTerminalStateEvidenceV1 {
        StructuredRepresentationTerminalStateEvidenceV1 {
            slot: 40,
            representation_replay_revision: 3,
            custody_replay_revision: 6,
            aggregate_revision: 7,
            custody_position_revision: 4,
            custody_native_claims: vec![5, 8, 3],
            shard_supply: 100,
            actor_shards: 40,
            structured_shards: 10,
            hoard_collateral: 50,
            recipient_collateral: 1,
            account_state_sha256: vec!["before".to_owned()],
        }
    }

    #[test]
    fn terminal_readback_accepts_exact_burn_and_positive_payout() {
        let before = terminal_state();
        let mut after = before.clone();
        after.slot = 41;
        after.representation_replay_revision = 4;
        after.custody_replay_revision = 7;
        after.aggregate_revision = 8;
        after.custody_position_revision = 5;
        after.custody_native_claims[1] = 6;
        after.shard_supply = 80;
        after.actor_shards = 20;
        after.hoard_collateral = 48;
        after.recipient_collateral = 3;
        after.account_state_sha256 = vec!["after".to_owned()];
        verify_terminal_poststate_v1(10, 1, 2, &before, &after)
            .expect("the exact terminal burn and payout must verify");
    }

    #[test]
    fn terminal_readback_accepts_losing_zero_payout_without_custody_advance() {
        let before = terminal_state();
        let mut after = before.clone();
        after.slot = 41;
        after.representation_replay_revision = 4;
        after.aggregate_revision = 8;
        after.custody_position_revision = 5;
        after.custody_native_claims[2] = 2;
        after.shard_supply = 90;
        after.actor_shards = 30;
        after.account_state_sha256 = vec!["after".to_owned()];
        verify_terminal_poststate_v1(10, 2, 1, &before, &after)
            .expect("a losing shard burn has an exact zero payout");
    }

    #[test]
    fn terminal_readback_names_an_unrelated_claim_mutation() {
        let before = terminal_state();
        let mut after = before.clone();
        after.slot = 41;
        after.representation_replay_revision = 4;
        after.custody_replay_revision = 7;
        after.aggregate_revision = 8;
        after.custody_position_revision = 5;
        after.custody_native_claims = vec![4, 6, 3];
        after.shard_supply = 80;
        after.actor_shards = 20;
        after.hoard_collateral = 48;
        after.recipient_collateral = 3;
        let error = verify_terminal_poststate_v1(10, 1, 2, &before, &after)
            .expect_err("an unrelated coordinate mutation must be named");
        assert_eq!(
            error.to_string(),
            "Structured terminal native Claim poststate differs at outcome 0"
        );
    }
}
