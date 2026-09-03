//! Chain-derived unsigned Dealer LP Open/Close Hot construction.
//!
//! One same-finalized view supplies the common Hot frame, admitted-AOT
//! evidence and the action-specific LP runtime suffix. The builder selects the
//! schema-bound descriptor from the sole Dealer `CapabilityProgramSetV2`,
//! rejoins every descriptor artifact, validates exact Profile2 geometry, and
//! emits one unsigned Trading instruction. It never signs or submits.

use crate::{
    Finality, Observation,
    direct_inline_v3::{CheckedHotOuterReleaseV3, ObservedAccountMetaV3},
};
use dclutch_account_profile_contract::v2::{
    AccountPrestateV2, AccountProfileV2, SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID_V2,
};
use dclutch_capability_program_contract::{
    hot_v3::{
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_ACTIVATION_CACHE_ACCOUNT_V3,
        HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
        HOT_EFFECT_RAW_ACCOUNT_V3, HOT_FAMILY_REQUEST_OFFSET_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LIFECYCLE_RAW_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_PROGRAM_SET_RAW_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_TRADING_PROGRAM_ACCOUNT_V3, HOT_TRANSITION_RAW_ACCOUNT_V3, HotBumpHintsV1,
        HotExecutionEnvelopeV3,
    },
    set_v2::{CapabilityDescriptorReferenceV2, CapabilityProgramSetV2, SelectorWidthV2},
    v4::{
        ArtifactReferenceV4, CapabilityArtifactsV4, CapabilityProgramV4, CapabilityRootAccountV4,
        SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4;
use dclutch_execution_strategy_contract::admitted_v3::{
    ADMITTED_ACCELERATOR_PROGRAM_ACCOUNT_V3, ADMITTED_STRATEGY_EVIDENCE_COUNT_V3,
    ADMITTED_STRATEGY_EVIDENCE_START_V3,
};
use dclutch_execution_strategy_contract::v2::{
    AcceleratorTransportProfileV2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_hot_bump_miner_v1::{
    HotBumpCorpusV1, activated_custody_program_v1, mine_hot_bump_hints_v1,
};
use dclutch_request_profile_contract::SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_ID_V1;
use dclutch_trading_sbf::{
    admitted_composition_v3::admitted_caller_authority_count_v3,
    dealer::{
        v3_lp_artifacts::{
            DEALER_LP_IDENTITY_COUNT_V3, DEALER_LP_SCALAR_COUNT_V3, dealer_lp_account_count_v3,
        },
        v3_operator::{
            DEALER_MULTI_LP_ACTION_SELECTOR_OFFSET_V3, DealerMultiLpRequestV3,
            MultiLpRequestActionV3,
        },
        v3_release::dealer_request_schema_v3,
    },
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::sysvar;

// The admitted CPI frame's evidence suffix is owned by
// `dclutch_execution_strategy_contract::admitted_v3`, which derives every slot
// from `ADMITTED_STRATEGY_EVIDENCE_START_V3` and pins the span's length to its
// last named account. The coordinates below are that table read relative to the
// start of the suffix, because `strategy_accounts` is the suffix, not the whole
// frame -- so they are subtracted from the contract's absolute coordinates
// rather than restated as the numbers they currently evaluate to.
const ADMITTED_AOT_FIXED_EXTRAS_V3: usize = ADMITTED_STRATEGY_EVIDENCE_COUNT_V3;
const ADMITTED_ACCELERATOR_PROGRAM_EXTRA_V3: usize =
    ADMITTED_ACCELERATOR_PROGRAM_ACCOUNT_V3 - ADMITTED_STRATEGY_EVIDENCE_START_V3;
const DEALER_LP_INJECTED_ACCOUNT_COUNT_V4: usize = 5;

/// Same-finalized physical inputs for one Dealer LP lifecycle instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerLpHotStateV4 {
    /// Exact common Hot39 prefix in canonical ABI order.
    pub fixed_accounts: Vec<ObservedAccountMetaV3>,
    /// Eight admitted-AOT records followed by exact caller-authority pages.
    pub strategy_accounts: Vec<ObservedAccountMetaV3>,
    /// AccountProfile suffix after the five injected common coordinates.
    pub runtime_suffix_accounts: Vec<ObservedAccountMetaV3>,
    /// Immutable execution release set selected by the Market.
    pub release_set: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Checked current Trading Hot outer.
    pub hot_outer: Option<CheckedHotOuterReleaseV3>,
}

/// Exact unsigned LP instruction and the facts that selected it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerLpHotReportV4 {
    /// Sole unsigned canonical Trading Hot instruction.
    pub instruction: Instruction,
    /// Same finalized observation selecting every supplied account.
    pub observation: Observation,
    /// Selected Open or Close action.
    pub action: MultiLpRequestActionV3,
    /// Exact descriptor schema/content pair selected from the global set.
    pub selected_descriptor: CapabilityDescriptorReferenceV2,
    /// SHA-256 of the exact family request supplied to Hot.
    pub family_request_digest: [u8; 32],
}

/// Stable refusal from LP Hot construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerLpHotOperatorErrorV4 {
    /// The selected outer deployment lacks checked release evidence.
    HotOuterUnavailable,
    /// Accounts did not belong to one finalized observation.
    ObservationMismatch,
    /// Common frame, Trading program, root, or sysvar geometry differed.
    FixedFrame,
    /// Request bytes or request-bound PDA coordinates differed.
    Request,
    /// ProgramSet, descriptor, root selection, or artifact content differed.
    Artifact,
    /// Admitted-AOT evidence/caller-authority geometry differed.
    StrategyGeometry,
    /// AccountProfile privileges, aliases, data widths, or suffix differed.
    RuntimeGeometry,
    /// Checked count or instruction-data arithmetic overflowed.
    Arithmetic,
}

/// Build one complete unsigned Dealer LP Open or Close Hot instruction.
pub fn build_dealer_lp_hot_instruction_v4(
    state: &DealerLpHotStateV4,
    family_request: &[u8],
) -> Result<DealerLpHotReportV4, DealerLpHotOperatorErrorV4> {
    let outer = state
        .hot_outer
        .ok_or(DealerLpHotOperatorErrorV4::HotOuterUnavailable)?;
    if outer.trading_program == Pubkey::default()
        || outer.artifact_release == [0; 32]
        || outer.checked_manifest_digest == [0; 32]
    {
        return Err(DealerLpHotOperatorErrorV4::HotOuterUnavailable);
    }
    let observation = validate_fixed_frame(state, outer)?;
    let request = DealerMultiLpRequestV3::decode(family_request)
        .map_err(|_| DealerLpHotOperatorErrorV4::Request)?;
    validate_request_coordinates(state, outer, request)?;

    let descriptor_reference = select_lp_descriptor(state, family_request)?;
    let descriptor_bytes = &fixed(state, HOT_DESCRIPTOR_RAW_ACCOUNT_V3)?.account.data;
    let descriptor = CapabilityProgramV4::decode(descriptor_bytes)
        .map_err(|_| DealerLpHotOperatorErrorV4::Artifact)?;
    validate_root_and_artifacts(state, request, descriptor)?;
    validate_strategy_geometry(state, descriptor)?;
    validate_runtime_geometry(state, request.action)?;

    let root = fixed(state, HOT_ROOT_ACCOUNT_V3)?;
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(family_request.len()).map_err(|_| DealerLpHotOperatorErrorV4::Arithmetic)?,
        state.release_set,
        request.market,
        state.generation,
        hash(&root.account.data).to_bytes(),
    )
    .map_err(|_| DealerLpHotOperatorErrorV4::Arithmetic)?
    .with_bump_hints(dealer_lp_hot_bump_hints_v4(state, &outer.trading_program)?);
    let mut data = Vec::with_capacity(
        HOT_FAMILY_REQUEST_OFFSET_V3
            .checked_add(family_request.len())
            .ok_or(DealerLpHotOperatorErrorV4::Arithmetic)?,
    );
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(family_request);
    let mut accounts = Vec::with_capacity(
        state
            .fixed_accounts
            .len()
            .checked_add(state.strategy_accounts.len())
            .and_then(|value| value.checked_add(state.runtime_suffix_accounts.len()))
            .ok_or(DealerLpHotOperatorErrorV4::Arithmetic)?,
    );
    accounts.extend(state.fixed_accounts.iter().map(account_meta));
    accounts.extend(state.strategy_accounts.iter().map(account_meta));
    accounts.extend(state.runtime_suffix_accounts.iter().map(account_meta));
    Ok(DealerLpHotReportV4 {
        instruction: Instruction {
            program_id: outer.trading_program,
            accounts,
            data,
        },
        observation,
        action: request.action,
        selected_descriptor: descriptor_reference,
        family_request_digest: hash(family_request).to_bytes(),
    })
}

/// Mine the bumps this family's readers would otherwise search for on chain.
///
/// Each search is a `find_program_address` at 1,500 CU per rejected candidate,
/// on a depth drawn from the fixture keys, and the Dealer LP route is the one
/// running within a few thousand CU of the ceiling. Run here it costs the
/// caller nothing anybody measures, and each hint is reproduced with
/// `create_program_address` against the account the frame supplies: a wrong
/// byte derives a different address and refuses at an equality that was already
/// there. No conjunct moves.
///
/// The DERIVATION is `dclutch_hot_bump_miner_v1`'s, shared with the Direct
/// builder, the campaign bundle builder and the Rational public outer builders.
/// This function owns the CORPUS -- which coordinate of the LP hot frame is the
/// Market, which is the root, and which account names the Custody deployment.
///
/// # Which slots this corpus reaches, and which it deliberately leaves
///
/// `market`, `root` and Custody's transfer authority are all derivable from the
/// frame this builder already validated. `child_relay[0]` is Custody's replay
/// cursor, whose seeds end in the projected child request's replay context;
/// `child_caller`'s seeds end in a digest over a request projected ON chain;
/// `lifecycle` is the LP lifecycle's created accounts in materialization order.
/// None of the three is projected here, so all three stay zero and search,
/// which is correct and merely slower -- that is the whole contract of the
/// block.
fn dealer_lp_hot_bump_hints_v4(
    state: &DealerLpHotStateV4,
    trading_program: &Pubkey,
) -> Result<HotBumpHintsV1, DealerLpHotOperatorErrorV4> {
    let market = &fixed(state, HOT_MARKET_ACCOUNT_V3)?.account;
    // Custody is not in the hot fixed frame; the Market's activation cache is,
    // and it names the release set's Custody deployment.
    let activation = &fixed(state, HOT_ACTIVATION_CACHE_ACCOUNT_V3)?.account;
    Ok(mine_hot_bump_hints_v1(&HotBumpCorpusV1 {
        market_key: market.key,
        market_data: &market.data,
        root_data: &fixed(state, HOT_ROOT_ACCOUNT_V3)?.account.data,
        core_program: fixed(state, HOT_CORE_PROGRAM_ACCOUNT_V3)?.account.key,
        trading_program: *trading_program,
        custody_program: activated_custody_program_v1(&activation.data),
        release_set: state.release_set,
    }))
}

fn validate_fixed_frame(
    state: &DealerLpHotStateV4,
    outer: CheckedHotOuterReleaseV3,
) -> Result<Observation, DealerLpHotOperatorErrorV4> {
    if state.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3
        || state.release_set == [0; 32]
        || state.generation == 0
    {
        return Err(DealerLpHotOperatorErrorV4::FixedFrame);
    }
    let market = fixed(state, HOT_MARKET_ACCOUNT_V3)?;
    let root = fixed(state, HOT_ROOT_ACCOUNT_V3)?;
    let trading = fixed(state, HOT_TRADING_PROGRAM_ACCOUNT_V3)?;
    let rent = fixed(state, HOT_RENT_SYSVAR_ACCOUNT_V3)?;
    let instructions = fixed(state, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)?;
    if market.account.key == Pubkey::default()
        || root.account.key == Pubkey::default()
        || root.is_signer
        || !root.is_writable
        || trading.account.key != outer.trading_program
        || !trading.account.executable
        || rent.account.key != sysvar::rent::ID
        || instructions.account.key != sysvar::instructions::ID
    {
        return Err(DealerLpHotOperatorErrorV4::FixedFrame);
    }
    let observation = market.account.observation;
    if observation.finality != Finality::Finalized {
        return Err(DealerLpHotOperatorErrorV4::ObservationMismatch);
    }
    for (index, account) in state.fixed_accounts.iter().enumerate() {
        if account.account.observation != observation
            || account.account.observation.finality != Finality::Finalized
            || account.is_signer
            || account.is_writable != (index == HOT_ROOT_ACCOUNT_V3)
        {
            return Err(DealerLpHotOperatorErrorV4::FixedFrame);
        }
    }
    for account in state
        .strategy_accounts
        .iter()
        .chain(&state.runtime_suffix_accounts)
    {
        if account.account.observation != observation
            || account.account.observation.finality != Finality::Finalized
        {
            return Err(DealerLpHotOperatorErrorV4::ObservationMismatch);
        }
    }
    Ok(observation)
}

fn validate_request_coordinates(
    state: &DealerLpHotStateV4,
    outer: CheckedHotOuterReleaseV3,
    request: DealerMultiLpRequestV3,
) -> Result<(), DealerLpHotOperatorErrorV4> {
    let root_observation = fixed(state, HOT_ROOT_ACCOUNT_V3)?;
    let root = root_observation.account.key;
    let market = fixed(state, HOT_MARKET_ACCOUNT_V3)?.account.key;
    let expected_obligation = Pubkey::find_program_address(
        &[
            dclutch_trading_sbf::dealer::v3_obligation::DEALER_OBLIGATION_PDA_DOMAIN_V3,
            root.as_ref(),
        ],
        &outer.trading_program,
    )
    .0;
    let expected_lp = Pubkey::find_program_address(
        &[
            dclutch_trading_sbf::dealer::v3_multi_lp::DEALER_LP_POSITION_PDA_DOMAIN_V3,
            root.as_ref(),
            &request.lp_owner,
        ],
        &outer.trading_program,
    )
    .0;
    if request.release_set != state.release_set
        || request.market != market.to_bytes()
        || request.child_root != root.to_bytes()
        || request.generation != state.generation
        || request.expires_at < root_observation.account.observation.slot
        || request.obligation != expected_obligation.to_bytes()
        || request.lp_position != expected_lp.to_bytes()
    {
        return Err(DealerLpHotOperatorErrorV4::Request);
    }
    Ok(())
}

fn select_lp_descriptor(
    state: &DealerLpHotStateV4,
    family_request: &[u8],
) -> Result<CapabilityDescriptorReferenceV2, DealerLpHotOperatorErrorV4> {
    let set =
        CapabilityProgramSetV2::decode(&fixed(state, HOT_PROGRAM_SET_RAW_ACCOUNT_V3)?.account.data)
            .map_err(|_| DealerLpHotOperatorErrorV4::Artifact)?;
    if set.selector_offset() != DEALER_MULTI_LP_ACTION_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV2::U16
    {
        return Err(DealerLpHotOperatorErrorV4::Artifact);
    }
    let selected = set
        .select_descriptor(family_request)
        .map_err(|_| DealerLpHotOperatorErrorV4::Artifact)?;
    let descriptor = &fixed(state, HOT_DESCRIPTOR_RAW_ACCOUNT_V3)?.account.data;
    if selected.schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_ID_V4
        || selected.program().to_bytes() != hash(descriptor).to_bytes()
    {
        return Err(DealerLpHotOperatorErrorV4::Artifact);
    }
    Ok(selected)
}

fn validate_root_and_artifacts(
    state: &DealerLpHotStateV4,
    request: DealerMultiLpRequestV3,
    descriptor: CapabilityProgramV4,
) -> Result<(), DealerLpHotOperatorErrorV4> {
    let root = CapabilityRootAccountV4::decode(
        &fixed(state, HOT_ROOT_ACCOUNT_V3)?.account.data,
        descriptor,
    )
    .map_err(|_| DealerLpHotOperatorErrorV4::Artifact)?;
    let header = root.header();
    if header.release_set().to_bytes() != state.release_set
        || header.market() != request.market
        || header.generation() != state.generation
        || header.selection().capability_release().to_bytes()
            != hash(&fixed(state, HOT_PROGRAM_SET_RAW_ACCOUNT_V3)?.account.data).to_bytes()
        || header.selection().config().to_bytes()
            != hash(&fixed(state, HOT_CONFIG_RAW_ACCOUNT_V3)?.account.data).to_bytes()
        || descriptor.request_schema()
            != dealer_request_schema_v3(request.action.selector())
                .map_err(|_| DealerLpHotOperatorErrorV4::Artifact)?
    {
        return Err(DealerLpHotOperatorErrorV4::Artifact);
    }
    descriptor
        .validate_persisted_selection(header.selection())
        .map_err(|_| DealerLpHotOperatorErrorV4::Artifact)?;
    let reference = |schema: [u8; 32], index: usize| {
        Ok::<_, DealerLpHotOperatorErrorV4>(ArtifactReferenceV4::new(
            ContentId::new(schema).map_err(|_| DealerLpHotOperatorErrorV4::Artifact)?,
            ContentId::new(hash(&fixed(state, index)?.account.data).to_bytes())
                .map_err(|_| DealerLpHotOperatorErrorV4::Artifact)?,
        ))
    };
    descriptor
        .validate_artifacts(CapabilityArtifactsV4 {
            account_profile: reference(
                ACCOUNT_PROFILE_SCHEMA_ID_V2,
                HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
            )?,
            request_profile: reference(
                REQUEST_PROFILE_SCHEMA_ID_V1,
                HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
            )?,
            lifecycle: reference(
                SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
                HOT_LIFECYCLE_RAW_ACCOUNT_V3,
            )?,
            strategy: reference(
                EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                HOT_STRATEGY_RAW_ACCOUNT_V3,
            )?,
            transition: reference(
                dclutch_transition_vm::v3::SCHEMA_RELEASE_ID,
                HOT_TRANSITION_RAW_ACCOUNT_V3,
            )?,
            effect: reference(EFFECT_SCHEMA_ID_V4, HOT_EFFECT_RAW_ACCOUNT_V3)?,
        })
        .map_err(|_| DealerLpHotOperatorErrorV4::Artifact)
}

fn validate_strategy_geometry(
    state: &DealerLpHotStateV4,
    descriptor: CapabilityProgramV4,
) -> Result<(), DealerLpHotOperatorErrorV4> {
    // The LP route is chunked, and that is READ rather than assumed: the record
    // this function already decodes below is the authority for it, so it is
    // decoded first and its profile is what sizes the caller-authority span.
    let strategy = ExecutionStrategyProgramV2::decode(
        &fixed(state, HOT_STRATEGY_RAW_ACCOUNT_V3)?.account.data,
    )
    .map_err(|_| DealerLpHotOperatorErrorV4::StrategyGeometry)?;
    let callers = admitted_caller_authority_count_v3(
        strategy
            .transport_profile()
            .map_err(|_| DealerLpHotOperatorErrorV4::StrategyGeometry)?,
        u32::from(DEALER_LP_SCALAR_COUNT_V3),
        u32::from(DEALER_LP_IDENTITY_COUNT_V3),
    )
    .map_err(|_| DealerLpHotOperatorErrorV4::StrategyGeometry)?;
    if state.strategy_accounts.len() != ADMITTED_AOT_FIXED_EXTRAS_V3 + callers
        || state
            .strategy_accounts
            .iter()
            .any(|account| account.is_signer || account.is_writable)
        || !state
            .strategy_accounts
            .get(ADMITTED_ACCELERATOR_PROGRAM_EXTRA_V3)
            .ok_or(DealerLpHotOperatorErrorV4::StrategyGeometry)?
            .account
            .executable
    {
        return Err(DealerLpHotOperatorErrorV4::StrategyGeometry);
    }
    if strategy.disposition() != StrategyDispositionV2::AdmittedAot
        || strategy.transport_profile() != Ok(AcceleratorTransportProfileV2::ChunkedBankV2)
    {
        return Err(DealerLpHotOperatorErrorV4::StrategyGeometry);
    }
    descriptor
        .validate_strategy_transition(
            descriptor.strategy(),
            ArtifactReferenceV4::new(strategy.transition_schema(), strategy.transition_program()),
        )
        .map_err(|_| DealerLpHotOperatorErrorV4::StrategyGeometry)
}

fn validate_runtime_geometry(
    state: &DealerLpHotStateV4,
    action: MultiLpRequestActionV3,
) -> Result<(), DealerLpHotOperatorErrorV4> {
    let profile = AccountProfileV2::decode(
        &fixed(state, HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)?
            .account
            .data,
    )
    .map_err(|_| DealerLpHotOperatorErrorV4::RuntimeGeometry)?;
    let expected = usize::from(dealer_lp_account_count_v3(action));
    if profile.fixed_account_count() != dealer_lp_account_count_v3(action)
        || profile.item_account_stride() != 0
        || expected < DEALER_LP_INJECTED_ACCOUNT_COUNT_V4
        || state.runtime_suffix_accounts.len() != expected - DEALER_LP_INJECTED_ACCOUNT_COUNT_V4
        || profile.common_scalar_count() != DEALER_LP_SCALAR_COUNT_V3
        || profile.common_identity_count() != DEALER_LP_IDENTITY_COUNT_V3
        || profile
            .physical_account_count_with_dynamic_spans(0, &[])
            .map_err(|_| DealerLpHotOperatorErrorV4::RuntimeGeometry)?
            != expected
    {
        return Err(DealerLpHotOperatorErrorV4::RuntimeGeometry);
    }
    let injected = [
        HOT_ROOT_ACCOUNT_V3,
        HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
    ];
    for coordinate in 0..expected {
        let account = runtime_account(state, &injected, coordinate)?;
        let rule = profile
            .rule(
                false,
                u16::try_from(coordinate).map_err(|_| DealerLpHotOperatorErrorV4::Arithmetic)?,
            )
            .map_err(|_| DealerLpHotOperatorErrorV4::RuntimeGeometry)?;
        let privileges = rule.privileges();
        let expected_data = usize::try_from(rule.data_length())
            .map_err(|_| DealerLpHotOperatorErrorV4::Arithmetic)?;
        if account.is_signer != (privileges & 1 != 0)
            || account.is_writable != (privileges & 2 != 0)
            || account.account.executable != (privileges & 4 != 0)
            || (account.account.data.len() != expected_data
                && !(rule.prestate() == AccountPrestateV2::LifecycleBound
                    && account.account.data.is_empty()))
        {
            return Err(DealerLpHotOperatorErrorV4::RuntimeGeometry);
        }
        let representative = profile
            .representative(0, coordinate)
            .map_err(|_| DealerLpHotOperatorErrorV4::RuntimeGeometry)?;
        if account.account.key
            != runtime_account(state, &injected, representative)?
                .account
                .key
        {
            return Err(DealerLpHotOperatorErrorV4::RuntimeGeometry);
        }
    }
    Ok(())
}

fn runtime_account<'a>(
    state: &'a DealerLpHotStateV4,
    injected: &[usize; DEALER_LP_INJECTED_ACCOUNT_COUNT_V4],
    coordinate: usize,
) -> Result<&'a ObservedAccountMetaV3, DealerLpHotOperatorErrorV4> {
    if coordinate < injected.len() {
        fixed(
            state,
            *injected
                .get(coordinate)
                .ok_or(DealerLpHotOperatorErrorV4::RuntimeGeometry)?,
        )
    } else {
        state
            .runtime_suffix_accounts
            .get(coordinate - injected.len())
            .ok_or(DealerLpHotOperatorErrorV4::RuntimeGeometry)
    }
}

fn fixed(
    state: &DealerLpHotStateV4,
    index: usize,
) -> Result<&ObservedAccountMetaV3, DealerLpHotOperatorErrorV4> {
    state
        .fixed_accounts
        .get(index)
        .ok_or(DealerLpHotOperatorErrorV4::FixedFrame)
}

fn account_meta(value: &ObservedAccountMetaV3) -> AccountMeta {
    AccountMeta {
        pubkey: value.account.key,
        is_signer: value.is_signer,
        is_writable: value.is_writable,
    }
}

#[cfg(test)]
mod tests {
    use crate::ObservedAccount;
    use dclutch_capability_program_contract::set_v2::{
        CapabilityProgramSetEntryV2, encode_program_set_v2, encoded_program_set_bytes_v2,
    };

    use super::*;

    fn observation() -> Observation {
        Observation {
            slot: 9,
            unix_timestamp: 10,
            finality: Finality::Finalized,
        }
    }

    fn meta(key: Pubkey, data: Vec<u8>) -> ObservedAccountMetaV3 {
        ObservedAccountMetaV3 {
            account: ObservedAccount {
                observation: observation(),
                key,
                owner: Pubkey::new_from_array([0x99; 32]),
                lamports: 1,
                executable: false,
                data,
            },
            is_signer: false,
            is_writable: false,
        }
    }

    fn id(bytes: [u8; 32]) -> ContentId {
        ContentId::new(bytes).expect("nonzero content")
    }

    fn set_bytes(offset: u32, selected_schema: [u8; 32], selected_program: [u8; 32]) -> Vec<u8> {
        let entries = (1_u32..=9)
            .map(|selector| {
                let (schema, program) = if selector == 7 {
                    (selected_schema, selected_program)
                } else {
                    (
                        [0x40 + u8::try_from(selector).expect("selector"); 32],
                        [0x60 + u8::try_from(selector).expect("selector"); 32],
                    )
                };
                CapabilityProgramSetEntryV2::new(
                    selector,
                    CapabilityDescriptorReferenceV2::new(id(schema), id(program)),
                )
            })
            .collect::<Vec<_>>();
        let mut output = vec![0; encoded_program_set_bytes_v2(entries.len()).expect("set width")];
        encode_program_set_v2(offset, SelectorWidthV2::U16, &entries, &mut output)
            .expect("program set");
        output
    }

    fn state(set: Vec<u8>, descriptor: Vec<u8>) -> DealerLpHotStateV4 {
        let mut fixed_accounts = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
            .map(|index| {
                meta(
                    Pubkey::new_from_array(
                        [1 + u8::try_from(index).expect("fixed coordinate"); 32],
                    ),
                    Vec::new(),
                )
            })
            .collect::<Vec<_>>();
        fixed_accounts[HOT_PROGRAM_SET_RAW_ACCOUNT_V3].account.data = set;
        fixed_accounts[HOT_DESCRIPTOR_RAW_ACCOUNT_V3].account.data = descriptor;
        DealerLpHotStateV4 {
            fixed_accounts,
            strategy_accounts: Vec::new(),
            runtime_suffix_accounts: Vec::new(),
            release_set: [1; 32],
            generation: 1,
            hot_outer: None,
        }
    }

    fn open_request() -> [u8; 12] {
        let mut request = [0_u8; 12];
        request[10..12].copy_from_slice(&7_u16.to_le_bytes());
        request
    }

    #[test]
    fn schema_content_selection_refuses_drift_and_descriptor_substitution() {
        let descriptor = vec![0x71; 600];
        let descriptor_id = hash(&descriptor).to_bytes();
        let canonical = state(
            set_bytes(
                DEALER_MULTI_LP_ACTION_SELECTOR_OFFSET_V3,
                CAPABILITY_PROGRAM_SCHEMA_ID_V4,
                descriptor_id,
            ),
            descriptor.clone(),
        );
        assert_eq!(
            select_lp_descriptor(&canonical, &open_request()),
            Ok(CapabilityDescriptorReferenceV2::new(
                id(CAPABILITY_PROGRAM_SCHEMA_ID_V4),
                id(descriptor_id),
            ))
        );

        let wrong_offset = state(
            set_bytes(
                DEALER_MULTI_LP_ACTION_SELECTOR_OFFSET_V3 + 1,
                CAPABILITY_PROGRAM_SCHEMA_ID_V4,
                descriptor_id,
            ),
            descriptor.clone(),
        );
        assert_eq!(
            select_lp_descriptor(&wrong_offset, &open_request()),
            Err(DealerLpHotOperatorErrorV4::Artifact)
        );

        let mut substituted = canonical.clone();
        substituted.fixed_accounts[HOT_DESCRIPTOR_RAW_ACCOUNT_V3]
            .account
            .data[0] ^= 1;
        assert_eq!(
            select_lp_descriptor(&substituted, &open_request()),
            Err(DealerLpHotOperatorErrorV4::Artifact)
        );

        let wrong_schema = state(
            set_bytes(
                DEALER_MULTI_LP_ACTION_SELECTOR_OFFSET_V3,
                [0x55; 32],
                descriptor_id,
            ),
            descriptor,
        );
        assert_eq!(
            select_lp_descriptor(&wrong_schema, &open_request()),
            Err(DealerLpHotOperatorErrorV4::Artifact)
        );
    }
}
