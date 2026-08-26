#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Chain-derived unsigned v0 construction for exact fractional claims.
//!
//! The operator authenticates finalized artifact Records, a Product Runtime
//! graph, and the selected TokenBehaviorV2 before using the pure Fractional
//! kernel. It never signs, submits, invents a shard Mint, or persists a supply
//! projection. Parsed Claims/Token balances are an explicitly named adapter
//! input and are rechecked by the onchain child route.

mod artifacts;
mod claims;
mod composition;
mod exposure_action_v2;
mod records;
mod token2022;

use dclutch_fractional_claim_contract::{
    FractionalActionV1, FractionalArtifactBundleV1, FractionalFamilyRequestInputV1,
    FractionalFamilyRequestV1, NO_TERMINAL_OUTCOME_V1,
};
use dclutch_fractional_claim_kernel::{
    FractionalPhaseV1, FractionalProjectionV1, FractionalTermsV1, OutcomeReserveV1,
    TransferObservationV1, encode_fractional_projection_v1, fractional_projection_bytes_v1,
    prepare_open_unwrap_v1, prepare_retire_v1, prepare_terminal_redeem_v1,
    prepare_terminal_zero_burn_v1, prepare_terminalize_v1, prepare_transfer_v1, prepare_wrap_v1,
};
use dclutch_resolution_core_v3_operator::{Observation, ObservedAccount};
use dclutch_versioned_message_operator::{
    VersionedMessagePlanV0, compile_v0_message_with_optional_tables,
};
use solana_hash::Hash;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

pub use artifacts::{
    FRACTIONAL_COMMON_IDENTITIES_V1, FRACTIONAL_COMMON_SCALARS_V1,
    FractionalArtifactCompilerErrorV1, FractionalClaimsAccountRuleV1,
    FractionalFinalizedArtifactBundleV1, build_fractional_composed_artifact_bundle_v1,
    build_fractional_finalized_artifact_bundle_v1,
};
pub use claims::{
    FractionalClaimsPositionSnapshotV1, FractionalSignedDeltaChainObservationV1,
    FractionalSignedDeltaChainPlanV1, build_fractional_signed_delta_instruction_v1,
    lower_fractional_action_to_signed_delta_v1, validate_fractional_signed_delta_chain_result_v1,
};
pub use composition::{
    CheckedFractionalCompositionV1, CheckedFractionalExposureV3,
    check_fractional_composition_bundle_v1, decode_and_check_fractional_composition_v1,
    decode_and_check_fractional_exposure_v3,
};
pub use exposure_action_v2::{
    CheckedFractionalTokenBehaviorV2, FractionalExposureMintSnapshotV2,
    FractionalExposureRentCloseObservationV2, FractionalExposureRentClosePlanV2,
    FractionalExposureRetirementContextV2, FractionalExposureRetirementPlanV2,
    FractionalExposureTerminalCandidateV2, FractionalExposureTerminalInputV2,
    FractionalExposureTokenEffectV2, FractionalExposureTokenObservationV2,
    FractionalExposureTokenPlanV2, FractionalTokenBehaviorRecordAdmissionV2,
    authenticate_fractional_token_behavior_v2, fractional_exposure_record_admission_v2,
    plan_fractional_exposure_rent_close_v2, plan_fractional_exposure_retirement_v2,
    plan_fractional_exposure_terminal_candidate_v2, plan_fractional_exposure_token_effect_v2,
};
pub use records::{
    CheckedFractionalReleaseInputV1, CheckedFractionalReleaseV1, FinalizedArtifactRecordV1,
    FractionalArtifactRecordSnapshotV1, FractionalChainArtifactSnapshotV1,
    FractionalPreparedChainArtifactsV1, authenticate_fractional_chain_artifacts_v1,
    prepare_fractional_chain_artifacts_v1,
};
pub use token2022::{
    FractionalDenominatorExecutionV1, FractionalLifecycleRentClosePlanV2, FractionalMintSnapshotV1,
    FractionalPhysicalTokenEffectsV1, FractionalPhysicalTokenObservationV1,
    FractionalPhysicalUnsignedV0PlanV1, FractionalRetirementTokenPlanV1,
    FractionalTokenAccountSnapshotV1, FractionalTokenActionSnapshotV1, FractionalTokenEffectPlanV1,
    FractionalTokenEffectV1, build_fractional_physical_unsigned_v0_from_chain_v1,
    plan_fractional_lifecycle_rent_close_v2, plan_fractional_retirement_token_effects_v1,
    plan_fractional_token_effect_v1,
};

/// Exact semantic coordinates obtained from authenticated chain state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalRequestContextV1 {
    /// Current execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Finalized Product graph-root digest.
    pub product_record: [u8; 32],
    /// Product-owned ResultDomain digest and ordering.
    pub result_domain: [u8; 32],
    /// Exact Fractional terms digest.
    pub terms: [u8; 32],
    /// Finalized TokenBehaviorV2 digest.
    pub token_behavior: [u8; 32],
}

/// Wallet intent; every authority and amount observation remains chain-derived.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalIntentV1 {
    /// Exact action.
    pub action: FractionalActionV1,
    /// Product-owned selected outcome, or `u32::MAX` for retirement.
    pub outcome: u32,
    /// Native claims or raw shard atoms according to the action.
    pub quantity: u64,
}

/// Adapter-parsed Claims and Token observations from one finalized snapshot.
///
/// This type is ephemeral and never persisted. The adapter must derive its
/// reserve rows from canonical Claims Positions and exact Token Mint supplies;
/// the kernel then validates every phase-dependent invariant again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalActionObservationV1<'a> {
    /// Common finalized observation shared with all artifact reads.
    pub observation: Observation,
    /// Current wrapper revision from the minimal root.
    pub revision: u64,
    /// Open, authenticated terminal winner, or retired phase.
    pub phase: FractionalPhaseV1,
    /// Finalized terminal coordinate digest, including Open-to-Terminal evidence.
    pub terminal_digest: [u8; 32],
    /// Authenticated winning outcome, or the canonical absent sentinel.
    ///
    /// Terminalization intentionally carries a winner while `phase` remains
    /// [`FractionalPhaseV1::Open`], because the kernel prepares the Open to
    /// Terminal transition rather than accepting a pre-transition shadow.
    pub terminal_outcome: u32,
    /// Exact Claims-native reserve and Token Mint supply rows in Product order.
    pub reserves: &'a [OutcomeReserveV1],
    /// Exact actor identity; zero only for permissionless terminalize/retire.
    pub owner: Pubkey,
    /// Exact selected source Token account, or zero when inactive.
    pub source_token_account: Pubkey,
    /// Exact selected destination Token account, or zero when inactive.
    pub destination_token_account: Pubkey,
    /// Actor's selected native Claims balance.
    pub actor_native_claims: u64,
    /// Actor/source raw selected shard balance.
    pub source_shards: u64,
    /// Destination raw selected shard balance.
    pub destination_shards: u64,
}

/// Owned exact action result derived from the pure kernel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalActionPlanV1 {
    /// Canonical family request.
    pub request: FractionalFamilyRequestV1,
    /// Terms-selected shard Mint; zero for terminalize/retire.
    pub shard_mint: [u8; 32],
    /// Whole native claims locked, returned, or terminally consumed.
    pub native_claims: u64,
    /// Exact same-Mint shard multiple minted, transferred, or burned.
    pub consumed_shards: u64,
    /// Explicit same-Mint change retained in the source Token account.
    pub change_shards: u64,
    /// Exact terminal collateral payout; zero for losing burns and nonterminal actions.
    pub collateral_atoms: u64,
    /// Selected reserve row after the action, when applicable.
    pub post_reserve: Option<OutcomeReserveV1>,
    /// Actor/source raw shard balance after the action.
    pub post_source_shards: u64,
    /// Destination raw shard balance after transfer/wrap.
    pub post_destination_shards: u64,
    /// Required wrapper revision after the action.
    pub post_revision: u64,
    /// Outcome-ordered zero-payout native claims burned during retirement.
    pub retirement_native_burns: Vec<u64>,
}

/// Complete unsigned v0 output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalUnsignedV0PlanV1 {
    /// Exact kernel-derived action effects.
    pub action: FractionalActionPlanV1,
    /// Unsigned packet-safe v0 message.
    pub message: VersionedMessagePlanV0,
    /// Checked release-manifest digest used by the operator.
    pub checked_manifest_digest: [u8; 32],
    /// Common finalized observation.
    pub observation: Observation,
}

/// Stable operator refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Finalized artifact or Product observation refused.
    ChainArtifacts,
    /// Terms/projection bytes or runtime-width reserve rows refused.
    Projection,
    /// Intent/action observations were inconsistent.
    Action,
    /// Pure kernel refused the requested exact transition.
    Kernel,
    /// Artifact bundle did not select the exact generated request.
    Bundle,
    /// Trading account frame omitted or substituted a selected program/signer.
    AccountFrame,
    /// Unsigned v0 compilation or packet sizing refused.
    Message,
    /// Composition DAG, Product basis, or exact retranslation differed.
    Composition,
    /// Canonical Claims SignedDelta lowering, frame, receipt, or post-state refused.
    Claims,
    /// Selected TokenBehaviorV2, Token-owned state, or exact Token-2022 effect refused.
    Token,
    /// Canonical producer-subtree retirement or lifecycle RentV2 closure refused.
    Rent,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, Error>;

/// Derive one exact action request and postcondition from chain observations.
pub fn plan_fractional_action_v1(
    terms: FractionalTermsV1<'_>,
    context: FractionalRequestContextV1,
    intent: FractionalIntentV1,
    observed: FractionalActionObservationV1<'_>,
) -> Result<FractionalActionPlanV1> {
    validate_context(terms, context, intent, observed)?;
    let projection_bytes =
        fractional_projection_bytes_v1(terms.outcome_count()).map_err(|_| Error::Projection)?;
    let mut scratch = vec![0_u8; projection_bytes];
    let mut encoded = vec![0_u8; projection_bytes];
    encode_fractional_projection_v1(
        terms,
        observed.phase,
        observed.revision,
        observed.reserves,
        &mut scratch,
        &mut encoded,
    )
    .map_err(|_| Error::Projection)?;
    let projection =
        FractionalProjectionV1::decode(&encoded, terms).map_err(|_| Error::Projection)?;
    let request = FractionalFamilyRequestV1::new(
        intent.action,
        FractionalFamilyRequestInputV1 {
            release_set: context.release_set,
            market: context.market,
            product_record: context.product_record,
            result_domain: context.result_domain,
            terms: context.terms,
            token_behavior: context.token_behavior,
            owner: observed.owner.to_bytes(),
            source_token_account: observed.source_token_account.to_bytes(),
            destination_token_account: observed.destination_token_account.to_bytes(),
            terminal_digest: observed.terminal_digest,
            expected_revision: observed.revision,
            quantity: intent.quantity,
            outcome: intent.outcome,
            terminal_outcome: observed.terminal_outcome,
        },
    )
    .map_err(|_| Error::Action)?;
    let empty = [0; 32];
    match intent.action {
        FractionalActionV1::Wrap => {
            let plan = prepare_wrap_v1(
                terms,
                projection,
                intent.outcome,
                intent.quantity,
                observed.actor_native_claims,
                observed.destination_shards,
            )
            .map_err(|_| Error::Kernel)?;
            Ok(FractionalActionPlanV1 {
                request,
                shard_mint: plan.shards_to_mint.shard_mint,
                native_claims: plan.native_claims_to_lock,
                consumed_shards: plan.shards_to_mint.shard_atoms,
                change_shards: 0,
                collateral_atoms: 0,
                post_reserve: Some(plan.post_reserve),
                post_source_shards: 0,
                post_destination_shards: plan.post_actor_shards,
                post_revision: plan.next_revision,
                retirement_native_burns: Vec::new(),
            })
        }
        FractionalActionV1::Transfer => {
            let plan = prepare_transfer_v1(
                terms,
                projection,
                intent.outcome,
                intent.quantity,
                TransferObservationV1 {
                    source_account: observed.source_token_account.to_bytes(),
                    destination_account: observed.destination_token_account.to_bytes(),
                    source_shards: observed.source_shards,
                    destination_shards: observed.destination_shards,
                },
            )
            .map_err(|_| Error::Kernel)?;
            Ok(FractionalActionPlanV1 {
                request,
                shard_mint: plan.shards_to_transfer.shard_mint,
                native_claims: 0,
                consumed_shards: plan.shards_to_transfer.shard_atoms,
                change_shards: 0,
                collateral_atoms: 0,
                post_reserve: None,
                post_source_shards: plan.post_source_shards,
                post_destination_shards: plan.post_destination_shards,
                post_revision: plan.unchanged_revision,
                retirement_native_burns: Vec::new(),
            })
        }
        FractionalActionV1::WholeUnwrap | FractionalActionV1::WinningRedeem => {
            let plan = if intent.action == FractionalActionV1::WholeUnwrap {
                prepare_open_unwrap_v1(
                    terms,
                    projection,
                    intent.outcome,
                    intent.quantity,
                    observed.source_shards,
                )
            } else {
                prepare_terminal_redeem_v1(
                    terms,
                    projection,
                    intent.outcome,
                    intent.quantity,
                    observed.source_shards,
                )
            }
            .map_err(|_| Error::Kernel)?;
            Ok(FractionalActionPlanV1 {
                request,
                shard_mint: plan.division.input_shards.shard_mint,
                native_claims: plan.division.whole_native_claims,
                consumed_shards: plan.division.consumed_shards.shard_atoms,
                change_shards: plan.division.change_shards.shard_atoms,
                collateral_atoms: plan.collateral_atoms_to_actor,
                post_reserve: Some(plan.post_reserve),
                post_source_shards: plan.post_actor_shards,
                post_destination_shards: 0,
                post_revision: plan.next_revision,
                retirement_native_burns: Vec::new(),
            })
        }
        FractionalActionV1::LosingZeroBurn => {
            let plan = prepare_terminal_zero_burn_v1(
                terms,
                projection,
                intent.outcome,
                intent.quantity,
                observed.source_shards,
            )
            .map_err(|_| Error::Kernel)?;
            Ok(FractionalActionPlanV1 {
                request,
                shard_mint: plan.shards_to_burn.shard_mint,
                native_claims: 0,
                consumed_shards: plan.shards_to_burn.shard_atoms,
                change_shards: 0,
                collateral_atoms: 0,
                post_reserve: Some(plan.post_reserve),
                post_source_shards: plan.post_actor_shards,
                post_destination_shards: 0,
                post_revision: plan.next_revision,
                retirement_native_burns: Vec::new(),
            })
        }
        FractionalActionV1::Terminalize => {
            let plan = prepare_terminalize_v1(projection, observed.terminal_outcome)
                .map_err(|_| Error::Kernel)?;
            Ok(FractionalActionPlanV1 {
                request,
                shard_mint: empty,
                native_claims: 0,
                consumed_shards: 0,
                change_shards: 0,
                collateral_atoms: 0,
                post_reserve: None,
                post_source_shards: 0,
                post_destination_shards: 0,
                post_revision: plan.next_revision,
                retirement_native_burns: Vec::new(),
            })
        }
        FractionalActionV1::ZeroSupplyRetire => {
            let plan = prepare_retire_v1(terms, projection).map_err(|_| Error::Kernel)?;
            let mut burns = Vec::with_capacity(
                usize::try_from(terms.outcome_count()).map_err(|_| Error::Projection)?,
            );
            let mut outcome = 0_u32;
            while outcome < terms.outcome_count() {
                burns.push(
                    plan.zero_payout_native_claims_to_burn(outcome)
                        .map_err(|_| Error::Kernel)?,
                );
                outcome = outcome.checked_add(1).ok_or(Error::Projection)?;
            }
            Ok(FractionalActionPlanV1 {
                request,
                shard_mint: empty,
                native_claims: 0,
                consumed_shards: 0,
                change_shards: 0,
                collateral_atoms: 0,
                post_reserve: None,
                post_source_shards: 0,
                post_destination_shards: 0,
                post_revision: plan.next_revision(),
                retirement_native_burns: burns,
            })
        }
    }
}

/// Authenticate chain artifacts, derive an exact action, and compile one
/// unsigned packet-safe v0 message without signing or submitting it.
#[allow(clippy::too_many_arguments)]
pub fn build_fractional_unsigned_v0_from_chain_v1(
    prepared: FractionalPreparedChainArtifactsV1<'_>,
    intent: FractionalIntentV1,
    observed: FractionalActionObservationV1<'_>,
    payer: Pubkey,
    recent_blockhash: Hash,
    accounts: &[AccountMeta],
    lookup_tables: &[ObservedAccount],
) -> Result<FractionalUnsignedV0PlanV1> {
    if observed.observation != prepared.observation()
        || observed.revision != prepared.root().input().revision
    {
        return Err(Error::ChainArtifacts);
    }
    let action = plan_fractional_action_v1(
        prepared.terms(),
        prepared.request_context(),
        intent,
        observed,
    )?;
    let request_bytes = action.request.to_bytes();
    let bundle = authenticate_fractional_chain_artifacts_v1(prepared, &request_bytes)?;
    build_fractional_unsigned_v0_v1(
        prepared.checked_release(),
        bundle,
        action,
        observed.observation,
        payer,
        recent_blockhash,
        accounts,
        lookup_tables,
    )
}

/// Compile one unsigned packet-safe v0 message after exact artifact admission.
#[allow(clippy::too_many_arguments)]
pub fn build_fractional_unsigned_v0_v1(
    checked: CheckedFractionalReleaseV1,
    bundle: FractionalArtifactBundleV1<'_>,
    action: FractionalActionPlanV1,
    observation: Observation,
    payer: Pubkey,
    recent_blockhash: Hash,
    accounts: &[AccountMeta],
    lookup_tables: &[ObservedAccount],
) -> Result<FractionalUnsignedV0PlanV1> {
    if bundle.family_request != action.request
        || checked.trading_program() == Pubkey::default()
        || observation.finality != dclutch_resolution_core_v3_operator::Finality::Finalized
    {
        return Err(Error::Bundle);
    }
    validate_account_frame(checked, action.request, payer, accounts)?;
    let instruction = Instruction {
        program_id: checked.trading_program(),
        accounts: accounts.to_vec(),
        data: action.request.to_bytes().to_vec(),
    };
    let message = compile_v0_message_with_optional_tables(
        payer,
        &[instruction],
        recent_blockhash,
        observation,
        lookup_tables,
    )
    .map_err(|_| Error::Message)?;
    Ok(FractionalUnsignedV0PlanV1 {
        action,
        message,
        checked_manifest_digest: checked.checked_manifest_digest(),
        observation,
    })
}

fn validate_context(
    terms: FractionalTermsV1<'_>,
    context: FractionalRequestContextV1,
    intent: FractionalIntentV1,
    observed: FractionalActionObservationV1<'_>,
) -> Result<()> {
    if observed.observation.finality != dclutch_resolution_core_v3_operator::Finality::Finalized
        || context.market != terms.market_id()
        || context.result_domain != terms.result_domain_id()
        || context.release_set != terms.release_set_id()
        || context.terms != terms.terms_id()
        || context.token_behavior != terms.token_behavior_selection_id()
        || context.product_record == [0; 32]
        || observed.reserves.len()
            != usize::try_from(terms.outcome_count()).map_err(|_| Error::Projection)?
        || (intent.action != FractionalActionV1::ZeroSupplyRetire
            && intent.outcome >= terms.outcome_count())
    {
        return Err(Error::Action);
    }
    let terminal = match observed.phase {
        FractionalPhaseV1::Open => {
            if intent.action == FractionalActionV1::Terminalize {
                observed.terminal_digest != [0; 32]
                    && observed.terminal_outcome < terms.outcome_count()
            } else {
                observed.terminal_digest == [0; 32]
                    && observed.terminal_outcome == NO_TERMINAL_OUTCOME_V1
            }
        }
        FractionalPhaseV1::Terminal { winning_outcome } => {
            winning_outcome < terms.outcome_count()
                && observed.terminal_digest != [0; 32]
                && observed.terminal_outcome == winning_outcome
        }
        FractionalPhaseV1::Retired => false,
    };
    if !terminal {
        return Err(Error::Action);
    }
    Ok(())
}

fn validate_account_frame(
    checked: CheckedFractionalReleaseV1,
    request: FractionalFamilyRequestV1,
    payer: Pubkey,
    accounts: &[AccountMeta],
) -> Result<()> {
    if payer == Pubkey::default()
        || accounts.is_empty()
        || !contains_program(accounts, checked.claims_program())
        || !contains_program(accounts, checked.custody_program())
        || !contains_program(accounts, checked.token_program())
    {
        return Err(Error::AccountFrame);
    }
    let owner = Pubkey::new_from_array(request.input().owner);
    if owner != Pubkey::default()
        && !accounts
            .iter()
            .any(|account| account.pubkey == owner && account.is_signer)
    {
        return Err(Error::AccountFrame);
    }
    Ok(())
}

fn contains_program(accounts: &[AccountMeta], program: Pubkey) -> bool {
    accounts
        .iter()
        .any(|account| account.pubkey == program && !account.is_signer && !account.is_writable)
}
