//! Chain-derived physical-account projection for Dealer scenario exact-fill.
//!
//! Selector 9 has one runtime-width semantic path.  This module re-executes
//! that path from the exact SignedDelta-bearing family request, derives the
//! complete candidate register bank, and lets the canonical Profile13 artifact
//! select all seven variable account spans.  Callers cannot supply a Position
//! count, Custody-route bitmap, or packed-account width separately.
//!
//! Common Hot finalized-record and accelerator authentication remains owned by
//! the family-neutral Trading outer.  This module deliberately returns only
//! the derived account metas and semantic report; the one-instruction wrapper
//! consumes the common authenticated context rather than creating a parallel
//! Dealer authority.

use crate::{Finality, Observation, direct_inline_v3::ObservedAccountMetaV3};
use dclutch_account_profile_contract::v2::{AccountProfileV2, PhysicalAccountDataGeometryV2};
use dclutch_capability_program_contract::hot_v3::{
    HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
    HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3,
    HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
};
use dclutch_trading_sbf::{
    admitted_composition_v3::admitted_caller_authority_count_v3,
    dealer::{
        v3_composer::{
            MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3, ScenarioAtomicPlanV3,
            ScenarioCollateralFrameV3, ScenarioComposerContextV3, ScenarioCustodyEffectV3,
        },
        v3_trade::{
            DealerScenarioTradeRequestV3, ScenarioTradeChainProjectionV3, prepare_scenario_trade_v3,
        },
        v3_trade_artifacts::{
            DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4, DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
            project_dealer_scenario_hot_registers_v4,
        },
        v3_trade_profile::{
            DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4, DEALER_SCENARIO_PROFILE_SPANS_V4,
            DealerScenarioAccountProfileInputV4, encode_dealer_scenario_account_profile_v4_atomic,
        },
    },
};
use solana_program::instruction::AccountMeta;

const ADMITTED_AOT_FIXED_EXTRAS_V3: usize = 8;
const ADMITTED_ACCELERATOR_PROGRAM_EXTRA_V3: usize = 6;
const DEALER_HOT_INJECTED_ACCOUNTS_V4: usize = 5;
const DEALER_HOT_INJECTED_PHYSICAL_INDICES_V4: [usize; DEALER_HOT_INJECTED_ACCOUNTS_V4] = [
    HOT_ROOT_ACCOUNT_V3,
    HOT_CONFIG_RAW_ACCOUNT_V3,
    HOT_PRODUCT_RAW_ACCOUNT_V3,
    HOT_PORTFOLIO_RAW_ACCOUNT_V3,
    HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
];

/// Same-finalized physical inputs after common Hot authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioHotMetaStateV4<'a> {
    /// Exact common Hot38 prefix in canonical ABI order.
    pub fixed_accounts: &'a [ObservedAccountMetaV3],
    /// Eight admitted-AOT extras followed by exact caller-authority pages.
    pub strategy_accounts: &'a [ObservedAccountMetaV3],
    /// Packed Profile13 suffix after the five common injected coordinates.
    pub runtime_suffix_accounts: &'a [ObservedAccountMetaV3],
}

/// Semantic inputs authenticated from the same chain observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioSemanticStateV4<'a> {
    /// Current and candidate obligation, Claims Positions, and Market joins.
    pub chain: ScenarioTradeChainProjectionV3<'a>,
    /// Current Registry/Realm/Custody composition coordinates.
    pub context: ScenarioComposerContextV3,
    /// Current canonical collateral accounts and balances.
    pub collateral: ScenarioCollateralFrameV3,
}

/// Exact selector-9 semantic and packed-account projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerScenarioHotMetaReportV4 {
    /// Same finalized observation shared by every physical account.
    pub observation: Observation,
    /// Canonical scenario-solvent plan re-derived from the request.
    pub semantic_plan: ScenarioAtomicPlanV3,
    /// Six `{0,14}` Custody spans followed by the Claims `{1,2}` span in
    /// canonical Profile13 table order.
    pub dynamic_span_counts: [u32; DEALER_SCENARIO_PROFILE_SPANS_V4],
    /// Exact packed physical AccountProfile account count, including the five
    /// common injected accounts.
    pub runtime_physical_account_count: usize,
    /// Exact admitted-AOT caller-authority page count for `97 + N` scalars and
    /// 117 identities.
    pub caller_authority_count: usize,
    /// Canonical transaction metas in `Hot38 || strategy || packed suffix`
    /// order. No signer or submission is performed.
    pub instruction_accounts: Vec<AccountMeta>,
}

/// Stable refusal from selector-9 semantic/meta projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioHotMetaErrorV4 {
    /// The family request or chain-derived semantic composition refused.
    Semantics,
    /// The canonical Profile13 artifact differed.
    AccountProfile,
    /// Common, strategy, or packed runtime account geometry differed.
    AccountGeometry,
    /// Accounts did not share one finalized observation.
    Observation,
    /// A checked count or width overflowed.
    Arithmetic,
}

/// Re-execute selector 9 and derive its complete unsigned account-meta list.
///
/// The returned dynamic spans are derived only from the canonical semantic
/// composer and candidate register projection.  In particular, a caller
/// cannot smuggle a second custody bitmap or Claims Position count into the
/// physical account frame.
pub fn project_dealer_scenario_hot_metas_v4(
    state: DealerScenarioHotMetaStateV4<'_>,
    semantic: DealerScenarioSemanticStateV4<'_>,
    family_request: &[u8],
) -> Result<DealerScenarioHotMetaReportV4, DealerScenarioHotMetaErrorV4> {
    let observation = validate_common_observation(state, semantic)?;
    let request = DealerScenarioTradeRequestV3::decode(family_request)
        .map_err(|_| DealerScenarioHotMetaErrorV4::Semantics)?;
    let width =
        usize::try_from(request.width).map_err(|_| DealerScenarioHotMetaErrorV4::Arithmetic)?;
    let scalar_count = usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)
        .checked_add(width)
        .ok_or(DealerScenarioHotMetaErrorV4::Arithmetic)?;
    let identity_count = usize::from(DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4);

    let mut acquired = vec![0_u64; width];
    let mut delivered = vec![0_u64; width];
    let mut obligations_before = vec![0_u64; width];
    let mut obligations_after = vec![0_u64; width];
    let mut post_inventory = vec![0_u64; width];
    let mut post_counterparty_inventory = vec![0_u64; width];
    let mut post_equity = vec![0_i128; width];
    let mut custody_effects =
        [None::<ScenarioCustodyEffectV3>; MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3];
    let semantic_plan = prepare_scenario_trade_v3(
        request,
        semantic.chain,
        semantic.context,
        semantic.collateral,
        &mut acquired,
        &mut delivered,
        &mut obligations_before,
        &mut obligations_after,
        &mut post_inventory,
        &mut post_counterparty_inventory,
        &mut post_equity,
        &mut custody_effects,
    )
    .map_err(|_| DealerScenarioHotMetaErrorV4::Semantics)?;
    let mut scalars = vec![0_u64; scalar_count];
    let mut identities = vec![[0_u8; 32]; identity_count];
    project_dealer_scenario_hot_registers_v4(
        request,
        &semantic_plan,
        semantic.chain.candidate_obligation,
        &custody_effects,
        semantic.chain.trading_program,
        semantic.chain.now,
        &mut scalars,
        &mut identities,
    )
    .map_err(|_| DealerScenarioHotMetaErrorV4::Semantics)?;

    let profile = authenticate_account_profile(state)?;
    if usize::from(profile.common_scalar_count()) != scalar_count
        || usize::from(profile.common_identity_count()) != identity_count
    {
        return Err(DealerScenarioHotMetaErrorV4::AccountProfile);
    }
    let mut dynamic_span_counts = [0_u32; DEALER_SCENARIO_PROFILE_SPANS_V4];
    profile
        .dynamic_span_widths_from_scalars(&scalars, &mut dynamic_span_counts)
        .map_err(|_| DealerScenarioHotMetaErrorV4::AccountProfile)?;
    let runtime_physical_account_count = validate_runtime_accounts(
        state,
        profile,
        request.width,
        &dynamic_span_counts,
        observation,
    )?;
    let caller_authority_count = admitted_caller_authority_count_v3(
        u32::try_from(scalar_count).map_err(|_| DealerScenarioHotMetaErrorV4::Arithmetic)?,
        u32::try_from(identity_count).map_err(|_| DealerScenarioHotMetaErrorV4::Arithmetic)?,
    )
    .map_err(|_| DealerScenarioHotMetaErrorV4::AccountGeometry)?;
    if state.strategy_accounts.len()
        != ADMITTED_AOT_FIXED_EXTRAS_V3
            .checked_add(caller_authority_count)
            .ok_or(DealerScenarioHotMetaErrorV4::Arithmetic)?
        || state
            .strategy_accounts
            .iter()
            .any(|account| account.is_signer || account.is_writable)
        || !state
            .strategy_accounts
            .get(ADMITTED_ACCELERATOR_PROGRAM_EXTRA_V3)
            .is_some_and(|account| account.account.executable)
    {
        return Err(DealerScenarioHotMetaErrorV4::AccountGeometry);
    }

    let capacity = state
        .fixed_accounts
        .len()
        .checked_add(state.strategy_accounts.len())
        .and_then(|value| value.checked_add(state.runtime_suffix_accounts.len()))
        .ok_or(DealerScenarioHotMetaErrorV4::Arithmetic)?;
    let mut instruction_accounts = Vec::with_capacity(capacity);
    instruction_accounts.extend(state.fixed_accounts.iter().map(account_meta));
    instruction_accounts.extend(state.strategy_accounts.iter().map(account_meta));
    instruction_accounts.extend(state.runtime_suffix_accounts.iter().map(account_meta));
    Ok(DealerScenarioHotMetaReportV4 {
        observation,
        semantic_plan,
        dynamic_span_counts,
        runtime_physical_account_count,
        caller_authority_count,
        instruction_accounts,
    })
}

fn authenticate_account_profile<'a>(
    state: DealerScenarioHotMetaStateV4<'a>,
) -> Result<AccountProfileV2<'a>, DealerScenarioHotMetaErrorV4> {
    let profile_bytes = fixed(state, HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)?
        .account
        .data
        .as_slice();
    let mut common_data_lengths = [0_u32; DEALER_HOT_INJECTED_ACCOUNTS_V4];
    for (destination, index) in common_data_lengths
        .iter_mut()
        .zip(DEALER_HOT_INJECTED_PHYSICAL_INDICES_V4)
    {
        *destination = u32::try_from(fixed(state, index)?.account.data.len())
            .map_err(|_| DealerScenarioHotMetaErrorV4::Arithmetic)?;
    }
    let mut scratch = vec![0_u8; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
    let mut expected = vec![0_u8; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
    encode_dealer_scenario_account_profile_v4_atomic(
        DealerScenarioAccountProfileInputV4 {
            common_data_lengths,
        },
        &mut scratch,
        &mut expected,
    )
    .map_err(|_| DealerScenarioHotMetaErrorV4::AccountProfile)?;
    if profile_bytes != expected {
        return Err(DealerScenarioHotMetaErrorV4::AccountProfile);
    }
    AccountProfileV2::decode(profile_bytes)
        .map_err(|_| DealerScenarioHotMetaErrorV4::AccountProfile)
}

fn validate_common_observation(
    state: DealerScenarioHotMetaStateV4<'_>,
    semantic: DealerScenarioSemanticStateV4<'_>,
) -> Result<Observation, DealerScenarioHotMetaErrorV4> {
    if state.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3 {
        return Err(DealerScenarioHotMetaErrorV4::AccountGeometry);
    }
    let observation = fixed(state, HOT_MARKET_ACCOUNT_V3)?.account.observation;
    if observation.finality != Finality::Finalized
        || semantic.chain.now != observation.slot
        || fixed(state, HOT_MARKET_ACCOUNT_V3)?.account.key.to_bytes() != semantic.chain.market
        || fixed(state, HOT_ROOT_ACCOUNT_V3)?.account.key.to_bytes() != semantic.chain.child_root
        || fixed(state, HOT_TRADING_PROGRAM_ACCOUNT_V3)?
            .account
            .key
            .to_bytes()
            != semantic.chain.trading_program
    {
        return Err(DealerScenarioHotMetaErrorV4::Observation);
    }
    for (index, account) in state.fixed_accounts.iter().enumerate() {
        if account.account.observation != observation
            || account.is_signer
            || account.is_writable != (index == HOT_ROOT_ACCOUNT_V3)
        {
            return Err(DealerScenarioHotMetaErrorV4::AccountGeometry);
        }
    }
    for account in state
        .strategy_accounts
        .iter()
        .chain(state.runtime_suffix_accounts)
    {
        if account.account.observation != observation
            || account.account.observation.finality != Finality::Finalized
        {
            return Err(DealerScenarioHotMetaErrorV4::Observation);
        }
    }
    Ok(observation)
}

fn validate_runtime_accounts(
    state: DealerScenarioHotMetaStateV4<'_>,
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32; DEALER_SCENARIO_PROFILE_SPANS_V4],
    observation: Observation,
) -> Result<usize, DealerScenarioHotMetaErrorV4> {
    let physical_count = profile
        .physical_account_count_with_dynamic_spans(tail_count, span_counts)
        .map_err(|_| DealerScenarioHotMetaErrorV4::AccountProfile)?;
    if physical_count < DEALER_HOT_INJECTED_ACCOUNTS_V4
        || state.runtime_suffix_accounts.len()
            != physical_count
                .checked_sub(DEALER_HOT_INJECTED_ACCOUNTS_V4)
                .ok_or(DealerScenarioHotMetaErrorV4::Arithmetic)?
    {
        return Err(DealerScenarioHotMetaErrorV4::AccountGeometry);
    }
    for physical_ordinal in 0..physical_count {
        let account = runtime_account(state, physical_ordinal)?;
        let geometry = profile
            .physical_account_geometry_with_dynamic_spans(tail_count, span_counts, physical_ordinal)
            .map_err(|_| DealerScenarioHotMetaErrorV4::AccountProfile)?;
        let privileges = geometry.privileges();
        if account.account.observation != observation
            || account.is_signer != privileges.signer()
            || account.is_writable != privileges.writable()
            || account.account.executable != privileges.executable()
            || !data_geometry_matches(geometry.data(), account.account.data.len())
        {
            return Err(DealerScenarioHotMetaErrorV4::AccountGeometry);
        }
    }
    Ok(physical_count)
}

fn data_geometry_matches(geometry: PhysicalAccountDataGeometryV2, actual: usize) -> bool {
    match geometry {
        PhysicalAccountDataGeometryV2::Exact { bytes } => actual == bytes,
        PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes } => {
            actual == 0 || actual == live_bytes
        }
        PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
            actual >= minimum_bytes
        }
        PhysicalAccountDataGeometryV2::Opaque => true,
    }
}

fn runtime_account<'a>(
    state: DealerScenarioHotMetaStateV4<'a>,
    physical_ordinal: usize,
) -> Result<&'a ObservedAccountMetaV3, DealerScenarioHotMetaErrorV4> {
    if physical_ordinal < DEALER_HOT_INJECTED_ACCOUNTS_V4 {
        let index = *DEALER_HOT_INJECTED_PHYSICAL_INDICES_V4
            .get(physical_ordinal)
            .ok_or(DealerScenarioHotMetaErrorV4::AccountGeometry)?;
        fixed(state, index)
    } else {
        state
            .runtime_suffix_accounts
            .get(
                physical_ordinal
                    .checked_sub(DEALER_HOT_INJECTED_ACCOUNTS_V4)
                    .ok_or(DealerScenarioHotMetaErrorV4::Arithmetic)?,
            )
            .ok_or(DealerScenarioHotMetaErrorV4::AccountGeometry)
    }
}

fn fixed<'a>(
    state: DealerScenarioHotMetaStateV4<'a>,
    index: usize,
) -> Result<&'a ObservedAccountMetaV3, DealerScenarioHotMetaErrorV4> {
    state
        .fixed_accounts
        .get(index)
        .ok_or(DealerScenarioHotMetaErrorV4::AccountGeometry)
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
    use dclutch_trading_sbf::dealer::v3_trade_profile::{
        DealerScenarioAccountProfileInputV4, encode_dealer_scenario_account_profile_v4_atomic,
    };
    use solana_program::pubkey::Pubkey;

    fn observation() -> Observation {
        Observation {
            slot: 11,
            unix_timestamp: 12,
            finality: Finality::Finalized,
        }
    }

    fn meta(
        index: usize,
        bytes: usize,
        signer: bool,
        writable: bool,
        executable: bool,
    ) -> ObservedAccountMetaV3 {
        ObservedAccountMetaV3 {
            account: ObservedAccount {
                observation: observation(),
                key: Pubkey::new_from_array([u8::try_from(index + 1).expect("small index"); 32]),
                owner: Pubkey::new_from_array([200; 32]),
                lamports: 1,
                executable,
                data: vec![0; bytes],
            },
            is_signer: signer,
            is_writable: writable,
        }
    }

    fn canonical_profile(common_data_lengths: [u32; 5]) -> Vec<u8> {
        let mut scratch = vec![0; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
        let mut output = vec![0; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
        encode_dealer_scenario_account_profile_v4_atomic(
            DealerScenarioAccountProfileInputV4 {
                common_data_lengths,
            },
            &mut scratch,
            &mut output,
        )
        .expect("profile");
        output
    }

    fn runtime_fixture(
        tail_count: u32,
        span_counts: [u32; DEALER_SCENARIO_PROFILE_SPANS_V4],
    ) -> (Vec<ObservedAccountMetaV3>, Vec<ObservedAccountMetaV3>) {
        let common_lengths = [32_u32, 40, 48, 56, 64];
        let profile_bytes = canonical_profile(common_lengths);
        let profile = AccountProfileV2::decode(&profile_bytes).expect("decode");
        let physical_count = profile
            .physical_account_count_with_dynamic_spans(tail_count, &span_counts)
            .expect("count");
        let mut fixed_accounts = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
            .map(|index| meta(index, 0, false, index == HOT_ROOT_ACCOUNT_V3, false))
            .collect::<Vec<_>>();
        fixed_accounts
            .get_mut(HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)
            .expect("profile account")
            .account
            .data = profile_bytes.clone();
        for (length, index) in common_lengths
            .into_iter()
            .zip(DEALER_HOT_INJECTED_PHYSICAL_INDICES_V4)
        {
            fixed_accounts
                .get_mut(index)
                .expect("injected account")
                .account
                .data = vec![0; usize::try_from(length).expect("length")];
        }
        let mut suffix = Vec::new();
        for ordinal in DEALER_HOT_INJECTED_ACCOUNTS_V4..physical_count {
            let geometry = profile
                .physical_account_geometry_with_dynamic_spans(tail_count, &span_counts, ordinal)
                .expect("geometry");
            let privileges = geometry.privileges();
            let bytes = match geometry.data() {
                PhysicalAccountDataGeometryV2::Exact { bytes }
                | PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes: bytes } => bytes,
                PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
                    minimum_bytes
                }
                PhysicalAccountDataGeometryV2::Opaque => 7,
            };
            suffix.push(meta(
                HOT_FIXED_ACCOUNT_COUNT_V3 + ordinal,
                bytes,
                privileges.signer(),
                privileges.writable(),
                privileges.executable(),
            ));
        }
        (fixed_accounts, suffix)
    }

    #[test]
    fn profile13_packs_sparse_and_dense_selector_nine_frames() {
        for spans in [[0, 0, 0, 0, 1, 0, 0], [14, 14, 14, 14, 2, 14, 14]] {
            let (fixed_accounts, suffix) = runtime_fixture(4, spans);
            let state = DealerScenarioHotMetaStateV4 {
                fixed_accounts: &fixed_accounts,
                strategy_accounts: &[],
                runtime_suffix_accounts: &suffix,
            };
            let profile = authenticate_account_profile(state).expect("canonical profile");
            let observed = validate_runtime_accounts(state, profile, 4, &spans, observation())
                .expect("packed geometry");
            assert_eq!(observed, suffix.len() + DEALER_HOT_INJECTED_ACCOUNTS_V4);
        }
    }

    #[test]
    fn packed_frame_refuses_privilege_and_exact_data_substitution() {
        let spans = [0, 0, 0, 0, 1, 0, 0];
        let (fixed_accounts, mut suffix) = runtime_fixture(4, spans);
        let profile_bytes = canonical_profile([32, 40, 48, 56, 64]);
        let profile = AccountProfileV2::decode(&profile_bytes).expect("canonical profile");
        let state = DealerScenarioHotMetaStateV4 {
            fixed_accounts: &fixed_accounts,
            strategy_accounts: &[],
            runtime_suffix_accounts: &suffix,
        };
        assert!(authenticate_account_profile(state).is_ok());
        assert!(validate_runtime_accounts(state, profile, 4, &spans, observation()).is_ok());

        suffix.last_mut().expect("obligation").is_writable = false;
        let substituted = DealerScenarioHotMetaStateV4 {
            fixed_accounts: &fixed_accounts,
            strategy_accounts: &[],
            runtime_suffix_accounts: &suffix,
        };
        assert_eq!(
            validate_runtime_accounts(substituted, profile, 4, &spans, observation()),
            Err(DealerScenarioHotMetaErrorV4::AccountGeometry)
        );
        suffix.last_mut().expect("obligation").is_writable = true;
        suffix.last_mut().expect("obligation").account.data.pop();
        let substituted = DealerScenarioHotMetaStateV4 {
            fixed_accounts: &fixed_accounts,
            strategy_accounts: &[],
            runtime_suffix_accounts: &suffix,
        };
        assert_eq!(
            validate_runtime_accounts(substituted, profile, 4, &spans, observation()),
            Err(DealerScenarioHotMetaErrorV4::AccountGeometry)
        );
    }

    #[test]
    fn profile_bytes_are_bound_to_observed_common_widths() {
        let spans = [0, 0, 0, 0, 1, 0, 0];
        let (mut fixed_accounts, suffix) = runtime_fixture(4, spans);
        let state = DealerScenarioHotMetaStateV4 {
            fixed_accounts: &fixed_accounts,
            strategy_accounts: &[],
            runtime_suffix_accounts: &suffix,
        };
        assert!(authenticate_account_profile(state).is_ok());
        fixed_accounts
            .get_mut(HOT_CONFIG_RAW_ACCOUNT_V3)
            .expect("config")
            .account
            .data
            .push(0);
        let substituted = DealerScenarioHotMetaStateV4 {
            fixed_accounts: &fixed_accounts,
            strategy_accounts: &[],
            runtime_suffix_accounts: &suffix,
        };
        assert_eq!(
            authenticate_account_profile(substituted),
            Err(DealerScenarioHotMetaErrorV4::AccountProfile)
        );
    }
}
