//! Chain-derived unsigned Dealer junior-equity Hot construction.
//!
//! This host adapter accepts one same-finalized Hot38 frame and the selected
//! admitted-AOT/runtime suffix.  It re-decodes the canonical Dealer request,
//! selects its `CapabilityProgram`, joins the three Dealer artifacts, and
//! emits exactly one unsigned Trading instruction.  It neither signs nor
//! submits and it does not introduce a Dealer-specific child instruction wire.

use crate::{
    Finality, Observation,
    direct_inline_v3::{CheckedHotOuterReleaseV3, ObservedAccountMetaV3},
};
use dclutch_account_profile_contract::v2::{AccountPrestateV2, AccountProfileV2};
use dclutch_capability_program_contract::{
    hot_v3::{
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_EFFECT_RAW_ACCOUNT_V3,
        HOT_FAMILY_REQUEST_OFFSET_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
        HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_TRADING_PROGRAM_ACCOUNT_V3, HOT_TRANSITION_RAW_ACCOUNT_V3, HotExecutionEnvelopeV3,
    },
    set_v1::{CapabilityProgramSetV1, SelectorWidthV1},
};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    AcceleratorTransportProfileV2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_trading_sbf::{
    admitted_composition_v3::admitted_caller_authority_count_v3,
    dealer::{
        v3_artifacts::authenticate_dealer_equity_artifacts_v3,
        v3_equity_operator::{
            DEALER_EQUITY_SELECTOR_OFFSET_V3, DealerEquityRequestV3, EquityRequestActionV3,
        },
        v3_hot_artifact::{
            DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3, dealer_current_slot_scalar_register_v3,
            dealer_equity_identity_count_v3, dealer_equity_scalar_count_v3,
        },
        v3_multi_lp::MultiLpActionV3,
    },
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::sysvar;

const ADMITTED_AOT_FIXED_EXTRAS_V3: usize = 8;
const ADMITTED_ACCELERATOR_PROGRAM_EXTRA_V3: usize = 6;

/// Same-finalized physical inputs for one Dealer junior-equity Hot action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerEquityHotStateV3 {
    /// Exact common Hot38 prefix in canonical ABI order.
    pub fixed_accounts: Vec<ObservedAccountMetaV3>,
    /// Eight admitted-AOT extras followed by its exact caller-authority pages.
    pub strategy_accounts: Vec<ObservedAccountMetaV3>,
    /// AccountProfile suffix after the five common logical coordinates.
    pub runtime_suffix_accounts: Vec<ObservedAccountMetaV3>,
    /// Immutable execution release set selected by the Market.
    pub release_set: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Checked current Trading Hot outer.
    pub hot_outer: Option<CheckedHotOuterReleaseV3>,
}

/// Exact unsigned Dealer instruction and the facts that selected it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerEquityHotReportV3 {
    /// Sole unsigned canonical Trading Hot instruction.
    pub instruction: Instruction,
    /// Same finalized observation selecting every supplied account.
    pub observation: Observation,
    /// Selected action from the exact Dealer request.
    pub action: EquityRequestActionV3,
    /// Selected sparse Claims Position-table cardinality, P in P0/P1/P2.
    pub signed_position_count: u32,
    /// CapabilityProgram content identity selected from the canonical set.
    pub selected_program: ContentId,
    /// SHA-256 of the exact family request supplied to the Hot envelope.
    pub family_request_digest: [u8; 32],
}

/// Stable refusal from Dealer Hot construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerEquityHotOperatorErrorV3 {
    /// The selected outer deployment lacks checked release evidence.
    HotOuterUnavailable,
    /// An account was not part of one finalized observation.
    ObservationMismatch,
    /// The common Hot38 physical ABI, root, program, or sysvar frame differed.
    FixedFrame,
    /// The request bytes, selector, or request-bound PDA differed.
    Request,
    /// The selected ProgramSet/descriptor or Dealer artifact join refused.
    Artifact,
    /// Admitted-AOT transport account geometry differed.
    StrategyGeometry,
    /// AccountProfile account width, privileges, aliases, or data widths differed.
    RuntimeGeometry,
    /// Checked arithmetic or envelope encoding failed.
    Arithmetic,
}

/// Build a complete unsigned Dealer contribution or redemption Hot instruction.
///
/// The caller supplies the exact bytes returned by the canonical Dealer request
/// constructor.  All six `Contribute/Redeem × P0/P1/P2` shapes are inferred
/// from those bytes and the selected onchain artifacts; no action table lives
/// in this client adapter.
pub fn build_dealer_equity_hot_instruction_v3(
    state: &DealerEquityHotStateV3,
    family_request: &[u8],
) -> Result<DealerEquityHotReportV3, DealerEquityHotOperatorErrorV3> {
    let outer = state
        .hot_outer
        .ok_or(DealerEquityHotOperatorErrorV3::HotOuterUnavailable)?;
    if outer.trading_program == Pubkey::default()
        || outer.artifact_release == [0; 32]
        || outer.checked_manifest_digest == [0; 32]
    {
        return Err(DealerEquityHotOperatorErrorV3::HotOuterUnavailable);
    }
    let observation = validate_fixed_frame(state, outer)?;
    let request = DealerEquityRequestV3::decode(family_request)
        .map_err(|_| DealerEquityHotOperatorErrorV3::Request)?;
    let action = action(request.action());
    let position_count = request
        .claims_plan()
        .map_err(|_| DealerEquityHotOperatorErrorV3::Request)?
        .map_or(0, |plan| plan.position_count());
    if position_count > 2 {
        return Err(DealerEquityHotOperatorErrorV3::Request);
    }
    validate_request_coordinates(state, outer, request)?;

    let set =
        CapabilityProgramSetV1::decode(&fixed(state, HOT_PROGRAM_SET_RAW_ACCOUNT_V3)?.account.data)
            .map_err(|_| DealerEquityHotOperatorErrorV3::Artifact)?;
    if set.selector_offset() != DEALER_EQUITY_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV1::U16
    {
        return Err(DealerEquityHotOperatorErrorV3::Artifact);
    }
    let selected_program = set
        .select(family_request)
        .map_err(|_| DealerEquityHotOperatorErrorV3::Artifact)?;
    if selected_program.to_bytes() != hash(&fixed(state, 6)?.account.data).to_bytes() {
        return Err(DealerEquityHotOperatorErrorV3::Artifact);
    }

    let scalar_count = dealer_equity_scalar_count_v3(action)
        .map_err(|_| DealerEquityHotOperatorErrorV3::Artifact)?;
    let identity_count = dealer_equity_identity_count_v3(action)
        .map_err(|_| DealerEquityHotOperatorErrorV3::Artifact)?;
    let mut scalar_scratch = vec![0_u64; scalar_count];
    let mut identity_scratch = vec![[0_u8; 32]; identity_count];
    let bundle = authenticate_dealer_equity_artifacts_v3(
        action,
        position_count,
        &fixed(state, 12)?.account.data,
        &fixed(state, HOT_TRANSITION_RAW_ACCOUNT_V3)?.account.data,
        &fixed(state, HOT_EFFECT_RAW_ACCOUNT_V3)?.account.data,
        &mut scalar_scratch,
        &mut identity_scratch,
    )
    .map_err(|_| DealerEquityHotOperatorErrorV3::Artifact)?;
    validate_strategy_geometry(state, scalar_count, identity_count)?;
    validate_runtime_geometry(state, bundle, action, position_count)?;

    let root = fixed(state, HOT_ROOT_ACCOUNT_V3)?;
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(family_request.len())
            .map_err(|_| DealerEquityHotOperatorErrorV3::Arithmetic)?,
        state.release_set,
        request.market,
        state.generation,
        hash(&root.account.data).to_bytes(),
    )
    .map_err(|_| DealerEquityHotOperatorErrorV3::Arithmetic)?;
    let mut data = Vec::with_capacity(
        HOT_FAMILY_REQUEST_OFFSET_V3
            .checked_add(family_request.len())
            .ok_or(DealerEquityHotOperatorErrorV3::Arithmetic)?,
    );
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(family_request);
    let mut accounts = Vec::with_capacity(
        state
            .fixed_accounts
            .len()
            .checked_add(state.strategy_accounts.len())
            .and_then(|count| count.checked_add(state.runtime_suffix_accounts.len()))
            .ok_or(DealerEquityHotOperatorErrorV3::Arithmetic)?,
    );
    accounts.extend(state.fixed_accounts.iter().map(account_meta));
    accounts.extend(state.strategy_accounts.iter().map(account_meta));
    accounts.extend(state.runtime_suffix_accounts.iter().map(account_meta));
    Ok(DealerEquityHotReportV3 {
        instruction: Instruction {
            program_id: outer.trading_program,
            accounts,
            data,
        },
        observation,
        action: request.action(),
        signed_position_count: position_count,
        selected_program,
        family_request_digest: hash(family_request).to_bytes(),
    })
}

fn action(action: EquityRequestActionV3) -> MultiLpActionV3 {
    match action {
        EquityRequestActionV3::Contribute => MultiLpActionV3::Add,
        EquityRequestActionV3::Redeem => MultiLpActionV3::Remove,
    }
}

fn validate_fixed_frame(
    state: &DealerEquityHotStateV3,
    outer: CheckedHotOuterReleaseV3,
) -> Result<Observation, DealerEquityHotOperatorErrorV3> {
    if state.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3
        || state.release_set == [0; 32]
        || state.generation == 0
    {
        return Err(DealerEquityHotOperatorErrorV3::FixedFrame);
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
        return Err(DealerEquityHotOperatorErrorV3::FixedFrame);
    }
    let observation = market.account.observation;
    if observation.finality != Finality::Finalized {
        return Err(DealerEquityHotOperatorErrorV3::ObservationMismatch);
    }
    for (index, account) in state.fixed_accounts.iter().enumerate() {
        if account.account.observation != observation
            || account.account.observation.finality != Finality::Finalized
            || account.is_signer
            || account.is_writable != (index == HOT_ROOT_ACCOUNT_V3)
        {
            return Err(DealerEquityHotOperatorErrorV3::FixedFrame);
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
            return Err(DealerEquityHotOperatorErrorV3::ObservationMismatch);
        }
    }
    Ok(observation)
}

fn validate_request_coordinates(
    state: &DealerEquityHotStateV3,
    outer: CheckedHotOuterReleaseV3,
    request: DealerEquityRequestV3<'_>,
) -> Result<(), DealerEquityHotOperatorErrorV3> {
    let root = fixed(state, HOT_ROOT_ACCOUNT_V3)?.account.key;
    let market = fixed(state, HOT_MARKET_ACCOUNT_V3)?.account.key;
    if request.release_set != state.release_set
        || request.market != market.to_bytes()
        || request.child_root != root.to_bytes()
        || request.generation != state.generation
        || request.expires_at < fixed(state, HOT_ROOT_ACCOUNT_V3)?.account.observation.slot
    {
        return Err(DealerEquityHotOperatorErrorV3::Request);
    }
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
            request.lp_owner.as_ref(),
        ],
        &outer.trading_program,
    )
    .0;
    if request.obligation != expected_obligation.to_bytes()
        || request.lp_position != expected_lp.to_bytes()
    {
        return Err(DealerEquityHotOperatorErrorV3::Request);
    }
    Ok(())
}

fn validate_strategy_geometry(
    state: &DealerEquityHotStateV3,
    scalars: usize,
    identities: usize,
) -> Result<(), DealerEquityHotOperatorErrorV3> {
    let callers = admitted_caller_authority_count_v3(
        u32::try_from(scalars).map_err(|_| DealerEquityHotOperatorErrorV3::Arithmetic)?,
        u32::try_from(identities).map_err(|_| DealerEquityHotOperatorErrorV3::Arithmetic)?,
    )
    .map_err(|_| DealerEquityHotOperatorErrorV3::StrategyGeometry)?;
    if state.strategy_accounts.len() != ADMITTED_AOT_FIXED_EXTRAS_V3 + callers
        || state
            .strategy_accounts
            .iter()
            .any(|account| account.is_signer || account.is_writable)
        || !state
            .strategy_accounts
            .get(ADMITTED_ACCELERATOR_PROGRAM_EXTRA_V3)
            .ok_or(DealerEquityHotOperatorErrorV3::StrategyGeometry)?
            .account
            .executable
    {
        return Err(DealerEquityHotOperatorErrorV3::StrategyGeometry);
    }
    let strategy = ExecutionStrategyProgramV2::decode(
        &fixed(state, HOT_STRATEGY_RAW_ACCOUNT_V3)?.account.data,
    )
    .map_err(|_| DealerEquityHotOperatorErrorV3::StrategyGeometry)?;
    if strategy.disposition() != StrategyDispositionV2::AdmittedAot
        || strategy.transport_profile() != Ok(AcceleratorTransportProfileV2::ChunkedBankV2)
    {
        return Err(DealerEquityHotOperatorErrorV3::StrategyGeometry);
    }
    Ok(())
}

fn validate_runtime_geometry(
    state: &DealerEquityHotStateV3,
    bundle: dclutch_trading_sbf::dealer::v3_artifacts::DealerEquityArtifactBundleV3<'_>,
    action: MultiLpActionV3,
    position_count: u32,
) -> Result<(), DealerEquityHotOperatorErrorV3> {
    let profile = AccountProfileV2::decode(
        &fixed(state, HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)?
            .account
            .data,
    )
    .map_err(|_| DealerEquityHotOperatorErrorV3::RuntimeGeometry)?;
    let expected_runtime = usize::from(profile.fixed_account_count());
    let expected_effect = usize::from(bundle.effect.fixed_account_count());
    let expected_from_frames = dealer_runtime_account_count(action, position_count)?;
    if profile.item_account_stride() != 0
        || expected_runtime != expected_effect
        || expected_runtime != expected_from_frames
        || expected_runtime < usize::from(DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3)
        || state.runtime_suffix_accounts.len()
            != expected_runtime - usize::from(DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3)
        || profile.common_scalar_count() as usize
            != dealer_equity_scalar_count_v3(action)
                .map_err(|_| DealerEquityHotOperatorErrorV3::RuntimeGeometry)?
        || profile.common_identity_count() as usize
            != dealer_equity_identity_count_v3(action)
                .map_err(|_| DealerEquityHotOperatorErrorV3::RuntimeGeometry)?
        || profile.trusted_current_slot_scalar() != dealer_current_slot_scalar_register_v3(action)
    {
        return Err(DealerEquityHotOperatorErrorV3::RuntimeGeometry);
    }
    let injected = [
        HOT_ROOT_ACCOUNT_V3,
        HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
    ];
    for coordinate in 0..expected_runtime {
        let account = if coordinate < injected.len() {
            fixed(
                state,
                *injected
                    .get(coordinate)
                    .ok_or(DealerEquityHotOperatorErrorV3::RuntimeGeometry)?,
            )?
        } else {
            state
                .runtime_suffix_accounts
                .get(coordinate - injected.len())
                .ok_or(DealerEquityHotOperatorErrorV3::RuntimeGeometry)?
        };
        let rule = profile
            .rule(
                false,
                u16::try_from(coordinate)
                    .map_err(|_| DealerEquityHotOperatorErrorV3::Arithmetic)?,
            )
            .map_err(|_| DealerEquityHotOperatorErrorV3::RuntimeGeometry)?;
        let privileges = rule.privileges();
        let expected_data = usize::try_from(rule.data_length())
            .map_err(|_| DealerEquityHotOperatorErrorV3::Arithmetic)?;
        if account.is_signer != (privileges & 1 != 0)
            || account.is_writable != (privileges & 2 != 0)
            || account.account.executable != (privileges & 4 != 0)
            || (account.account.data.len() != expected_data
                && !(rule.prestate() == AccountPrestateV2::LifecycleBound
                    && account.account.data.is_empty()))
        {
            return Err(DealerEquityHotOperatorErrorV3::RuntimeGeometry);
        }
        let representative = profile
            .representative(0, coordinate)
            .map_err(|_| DealerEquityHotOperatorErrorV3::RuntimeGeometry)?;
        let canonical = if representative < injected.len() {
            fixed(
                state,
                *injected
                    .get(representative)
                    .ok_or(DealerEquityHotOperatorErrorV3::RuntimeGeometry)?,
            )?
        } else {
            state
                .runtime_suffix_accounts
                .get(representative - injected.len())
                .ok_or(DealerEquityHotOperatorErrorV3::RuntimeGeometry)?
        };
        if account.account.key != canonical.account.key {
            return Err(DealerEquityHotOperatorErrorV3::RuntimeGeometry);
        }
    }
    Ok(())
}

fn dealer_runtime_account_count(
    action: MultiLpActionV3,
    position_count: u32,
) -> Result<usize, DealerEquityHotOperatorErrorV3> {
    if position_count > 2 {
        return Err(DealerEquityHotOperatorErrorV3::RuntimeGeometry);
    }
    let custody_routes = match action {
        MultiLpActionV3::Add => 2_usize,
        MultiLpActionV3::Remove => 3_usize,
    };
    usize::from(DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3)
        .checked_add(
            custody_routes
                .checked_mul(14)
                .ok_or(DealerEquityHotOperatorErrorV3::Arithmetic)?,
        )
        .and_then(|count| count.checked_add(20 + usize::try_from(position_count).ok()?))
        .and_then(|count| count.checked_add(2))
        .ok_or(DealerEquityHotOperatorErrorV3::Arithmetic)
}

fn fixed(
    state: &DealerEquityHotStateV3,
    index: usize,
) -> Result<&ObservedAccountMetaV3, DealerEquityHotOperatorErrorV3> {
    state
        .fixed_accounts
        .get(index)
        .ok_or(DealerEquityHotOperatorErrorV3::FixedFrame)
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
    use super::*;
    use crate::ObservedAccount;

    fn observation() -> Observation {
        Observation {
            slot: 9,
            unix_timestamp: 10,
            finality: Finality::Finalized,
        }
    }

    fn meta(key: Pubkey, executable: bool) -> ObservedAccountMetaV3 {
        ObservedAccountMetaV3 {
            account: ObservedAccount {
                observation: observation(),
                key,
                owner: Pubkey::new_from_array([99; 32]),
                lamports: 1,
                executable,
                data: Vec::new(),
            },
            is_signer: false,
            is_writable: false,
        }
    }

    fn fixed_state(program: Pubkey, root: Pubkey, market: Pubkey) -> DealerEquityHotStateV3 {
        let mut fixed_accounts = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
            .map(|index| meta(Pubkey::new_from_array([index as u8 + 1; 32]), false))
            .collect::<Vec<_>>();
        fixed_accounts[HOT_MARKET_ACCOUNT_V3] = meta(market, false);
        fixed_accounts[HOT_ROOT_ACCOUNT_V3] = ObservedAccountMetaV3 {
            is_writable: true,
            ..meta(root, false)
        };
        fixed_accounts[HOT_TRADING_PROGRAM_ACCOUNT_V3] = meta(program, true);
        fixed_accounts[HOT_RENT_SYSVAR_ACCOUNT_V3] = meta(sysvar::rent::ID, false);
        fixed_accounts[HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3] = meta(sysvar::instructions::ID, false);
        DealerEquityHotStateV3 {
            fixed_accounts,
            strategy_accounts: Vec::new(),
            runtime_suffix_accounts: Vec::new(),
            release_set: [7; 32],
            generation: 1,
            hot_outer: Some(CheckedHotOuterReleaseV3 {
                trading_program: program,
                artifact_release: [8; 32],
                checked_manifest_digest: [9; 32],
            }),
        }
    }

    fn p0_request(program: Pubkey, root: Pubkey, market: Pubkey, owner: Pubkey) -> [u8; 480] {
        let obligation = Pubkey::find_program_address(
            &[
                dclutch_trading_sbf::dealer::v3_obligation::DEALER_OBLIGATION_PDA_DOMAIN_V3,
                root.as_ref(),
            ],
            &program,
        )
        .0;
        let lp = Pubkey::find_program_address(
            &[
                dclutch_trading_sbf::dealer::v3_multi_lp::DEALER_LP_POSITION_PDA_DOMAIN_V3,
                root.as_ref(),
                owner.as_ref(),
            ],
            &program,
        )
        .0;
        let mut request = [0_u8; 480];
        request[..8].copy_from_slice(b"DCLMEQ03");
        request[8..10].copy_from_slice(&2_u16.to_le_bytes());
        request[10..12].copy_from_slice(&1_u16.to_le_bytes());
        request[12..16].copy_from_slice(&1_u32.to_le_bytes());
        for (offset, key) in [
            (16, [7; 32]),
            (48, market.to_bytes()),
            (80, root.to_bytes()),
            (112, lp.to_bytes()),
            (144, owner.to_bytes()),
            (176, obligation.to_bytes()),
            (208, [10; 32]),
            (240, [11; 32]),
            (272, [12; 32]),
            (304, [13; 32]),
            (336, [14; 32]),
            (368, [15; 32]),
        ] {
            request[offset..offset + 32].copy_from_slice(&key);
        }
        for (offset, value) in [
            (400, 1_u64),
            (408, 1),
            (416, 1),
            (424, 1),
            (432, 1),
            (440, 9),
            (448, 0),
            (456, 1),
            (464, 1),
        ] {
            request[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        request
    }

    #[test]
    fn all_six_shapes_have_exact_claims_and_custody_geometry() {
        assert_eq!(
            dealer_runtime_account_count(MultiLpActionV3::Add, 0),
            Ok(55)
        );
        assert_eq!(
            dealer_runtime_account_count(MultiLpActionV3::Add, 1),
            Ok(56)
        );
        assert_eq!(
            dealer_runtime_account_count(MultiLpActionV3::Add, 2),
            Ok(57)
        );
        assert_eq!(
            dealer_runtime_account_count(MultiLpActionV3::Remove, 0),
            Ok(69)
        );
        assert_eq!(
            dealer_runtime_account_count(MultiLpActionV3::Remove, 1),
            Ok(70)
        );
        assert_eq!(
            dealer_runtime_account_count(MultiLpActionV3::Remove, 2),
            Ok(71)
        );
        assert_eq!(
            dealer_runtime_account_count(MultiLpActionV3::Add, 3),
            Err(DealerEquityHotOperatorErrorV3::RuntimeGeometry)
        );
    }

    #[test]
    fn dealer_pdas_are_bound_to_root_and_lp_owner() {
        let program = Pubkey::new_from_array([1; 32]);
        let root = Pubkey::new_from_array([2; 32]);
        let owner = Pubkey::new_from_array([3; 32]);
        let obligation = Pubkey::find_program_address(
            &[
                dclutch_trading_sbf::dealer::v3_obligation::DEALER_OBLIGATION_PDA_DOMAIN_V3,
                root.as_ref(),
            ],
            &program,
        )
        .0;
        let lp = Pubkey::find_program_address(
            &[
                dclutch_trading_sbf::dealer::v3_multi_lp::DEALER_LP_POSITION_PDA_DOMAIN_V3,
                root.as_ref(),
                owner.as_ref(),
            ],
            &program,
        )
        .0;
        assert_ne!(obligation, lp);
        let other_owner = Pubkey::new_from_array([4; 32]);
        let other_lp = Pubkey::find_program_address(
            &[
                dclutch_trading_sbf::dealer::v3_multi_lp::DEALER_LP_POSITION_PDA_DOMAIN_V3,
                root.as_ref(),
                other_owner.as_ref(),
            ],
            &program,
        )
        .0;
        assert_ne!(lp, other_lp);
    }

    #[test]
    fn fixed_frame_and_request_pdas_refuse_substitution() {
        let program = Pubkey::new_from_array([1; 32]);
        let root = Pubkey::new_from_array([2; 32]);
        let market = Pubkey::new_from_array([3; 32]);
        let owner = Pubkey::new_from_array([4; 32]);
        let state = fixed_state(program, root, market);
        let outer = state.hot_outer.expect("checked outer");
        assert_eq!(validate_fixed_frame(&state, outer), Ok(observation()));
        let request = p0_request(program, root, market, owner);
        let decoded = DealerEquityRequestV3::decode(&request).expect("P0 request");
        assert_eq!(validate_request_coordinates(&state, outer, decoded), Ok(()));

        let mut substituted_state = state.clone();
        substituted_state.fixed_accounts[HOT_TRADING_PROGRAM_ACCOUNT_V3]
            .account
            .key = Pubkey::new_from_array([5; 32]);
        assert_eq!(
            validate_fixed_frame(&substituted_state, outer),
            Err(DealerEquityHotOperatorErrorV3::FixedFrame)
        );

        let mut substituted_request = request;
        substituted_request[112] ^= 1;
        let decoded =
            DealerEquityRequestV3::decode(&substituted_request).expect("well-formed request");
        assert_eq!(
            validate_request_coordinates(&state, outer, decoded),
            Err(DealerEquityHotOperatorErrorV3::Request)
        );
    }
}
