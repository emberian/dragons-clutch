//! Physical account authentication for Dealer selector 9's admitted accelerator.
//!
//! The common Hot helper authenticates the release, finalized execution
//! artifacts, Product graph, exact family request, Profile13 expansion, and
//! input register bank. This module consumes that read-only view and rejoins
//! every family semantic account before invoking the sole Dealer evaluator.
//! It owns no state, CPI, or write authority.

extern crate alloc;

use alloc::{boxed::Box, vec, vec::Vec};

use dclutch_claims_svm::{
    frame_spec_v1::{
        ClaimsFrameRoleV1, SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3, SignedDeltaFrameSpecV3,
    },
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
    },
    protocol_position_v2::ProtocolPositionSeedsV2,
};
use dclutch_core_contract::{MarketRoot, Phase};
use dclutch_custody_contract::{
    CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1, CustodyAuthoritySeedsV1,
    CustodyReplaySeedsV1, CustodyReplayV1, CustodyVaultSeedsV1,
};
use dclutch_dealer_codec::{
    config_v3::{DEALER_CONFIG_SCHEMA_PREIMAGE_V3, DealerConfigV3},
    scenario::ClaimsInventoryObservation,
};
use dclutch_realm_contract::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_token_svm::{ACCOUNT_BYTES, AccountState, COption, TokenAccount};
use solana_program::{account_info::AccountInfo, hash::hash, pubkey::Pubkey};
use solana_sdk_ids::system_program;

use crate::hot_v3::AuthenticatedAcceleratorInvocationV4;

use super::{
    v3_admitted::{
        DealerScenarioAdmittedBuffersV4, DealerScenarioAdmittedErrorV4,
        evaluate_dealer_scenario_admitted_v4,
    },
    v3_composer::{
        MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3, ScenarioAtomicPlanV3, ScenarioCollateralFrameV3,
        ScenarioComposerContextV3, ScenarioCustodyEffectV3,
    },
    v3_hot_artifact::DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3,
    v3_obligation::{
        DealerObligationProjectionV3, ObligationAccountObservationV3, ObligationExpectationV3,
        obligation_account_bytes_v3,
    },
    v3_trade::{DEALER_SCENARIO_TRADE_ACTION_V3, DealerScenarioTradeRequestV3},
    v3_trade_artifacts::{
        DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4, DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
        DEALER_SCENARIO_CURRENT_SLOT_SCALAR_V4,
    },
    v3_trade_profile::{
        DEALER_SCENARIO_PROFILE_SPANS_V4, DealerScenarioLogicalFrameV4,
        dealer_scenario_logical_frame_v4,
    },
};

/// Stable refusal at selector 9's physical accelerator boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioAcceleratorErrorV4 {
    /// The authenticated common view did not select selector 9 or its exact widths.
    Invocation,
    /// Immutable Dealer config or Core/Claims joins differed.
    SemanticJoin,
    /// Claims aggregate, Position, or obligation state refused.
    Claims,
    /// Realm, replay, vault, external token account, or physical span refused.
    Custody,
    /// The sole scenario evaluator refused the authenticated state.
    Evaluation(DealerScenarioAdmittedErrorV4),
    /// Checked count or byte arithmetic overflowed.
    Arithmetic,
}

impl From<DealerScenarioAdmittedErrorV4> for DealerScenarioAcceleratorErrorV4 {
    fn from(value: DealerScenarioAdmittedErrorV4) -> Self {
        Self::Evaluation(value)
    }
}

#[derive(Clone, Copy)]
struct TokenObservationV4 {
    key: [u8; 32],
    token: TokenAccount,
}

#[derive(Clone, Copy)]
struct CollateralObservationV4 {
    principal: TokenObservationV4,
    fee: TokenObservationV4,
    hoard: TokenObservationV4,
    counterparty: TokenObservationV4,
    realm: RealmV1,
    replay: CustodyReplayV1,
}

/// Authenticate every selector-9 family account and evaluate one candidate bank.
///
/// `candidate_bank` is commit-last: it remains byte-for-byte unchanged on every
/// refusal. All runtime-width scratch is ephemeral and derived from the Product
/// width authenticated by common Hot.
pub fn evaluate_authenticated_dealer_scenario_v4(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    candidate_bank: &mut [u8],
) -> Result<ScenarioAtomicPlanV3, DealerScenarioAcceleratorErrorV4> {
    let request = DealerScenarioTradeRequestV3::decode(invocation.family_request())
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Invocation)?;
    let width =
        usize::try_from(request.width).map_err(|_| DealerScenarioAcceleratorErrorV4::Arithmetic)?;
    let runtime = invocation.runtime_accounts();
    let spans: [u32; DEALER_SCENARIO_PROFILE_SPANS_V4] = invocation
        .span_widths()
        .try_into()
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Invocation)?;
    let frame = dealer_scenario_logical_frame_v4(spans)
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Invocation)?;
    if invocation.selected_action() != u32::from(DEALER_SCENARIO_TRADE_ACTION_V3)
        || invocation.context().tail_count != request.width
        || invocation.product_runtime().runtime.outcome_count != request.width
        || frame.logical_account_count
            != u32::try_from(runtime.len())
                .map_err(|_| DealerScenarioAcceleratorErrorV4::Arithmetic)?
        || spans.get(4).copied() != Some(u32::from(request.claims_position_count))
        || spans.get(7).copied() != Some(u32::from(request.evidence_span_count))
        || invocation.input_bank().len() != candidate_bank.len()
        || invocation.request().scalar_count()
            != u32::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)
                .checked_add(request.width)
                .ok_or(DealerScenarioAcceleratorErrorV4::Arithmetic)?
        || invocation.request().identity_count()
            != u32::from(DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4)
        || runtime
            .iter()
            .any(|account| account.is_signer || account.is_writable)
    {
        return Err(DealerScenarioAcceleratorErrorV4::Invocation);
    }

    authenticate_and_evaluate_dealer_scenario_v4(
        invocation,
        request,
        width,
        runtime,
        spans,
        frame,
        candidate_bank,
    )
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn authenticate_and_evaluate_dealer_scenario_v4(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerScenarioTradeRequestV3<'_>,
    width: usize,
    runtime: &[AccountInfo<'_>],
    spans: [u32; DEALER_SCENARIO_PROFILE_SPANS_V4],
    frame: DealerScenarioLogicalFrameV4,
    candidate_bank: &mut [u8],
) -> Result<ScenarioAtomicPlanV3, DealerScenarioAcceleratorErrorV4> {
    let config = authenticate_config(invocation, request, runtime)?;
    let claims = Box::new(authenticate_claims(
        invocation, request, frame, runtime, width, config,
    )?);
    let current_obligation = DealerObligationProjectionV3::decode(&claims.obligation_bytes)
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?;
    let collateral = Box::new(authenticate_collateral(
        invocation,
        request,
        frame,
        spans,
        runtime,
        config,
        claims.core_market,
    )?);
    let current_slot = invocation
        .scalars()
        .get(usize::from(DEALER_SCENARIO_CURRENT_SLOT_SCALAR_V4))
        .copied()
        .ok_or(DealerScenarioAcceleratorErrorV4::Invocation)?;

    evaluate_authenticated_candidate_v4(
        invocation,
        request,
        width,
        config,
        &claims,
        current_obligation,
        &collateral,
        current_slot,
        candidate_bank,
    )
}

struct DealerScenarioEvaluationScratchV4 {
    acquired: Vec<u64>,
    delivered: Vec<u64>,
    obligations_before: Vec<u64>,
    obligations_after: Vec<u64>,
    candidate_obligation_state: Vec<u8>,
    post_inventory: Vec<u64>,
    post_counterparty_inventory: Vec<u64>,
    post_equity: Vec<i128>,
    custody_effects: [Option<ScenarioCustodyEffectV3>; MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3],
    candidate_scalars: Vec<u64>,
    candidate_identities: Vec<[u8; 32]>,
    bank_scratch: Vec<u8>,
}

impl DealerScenarioEvaluationScratchV4 {
    fn new(
        width: usize,
        request_width: u32,
        scalar_count: usize,
        identity_count: usize,
        bank_bytes: usize,
    ) -> Result<Box<Self>, DealerScenarioAcceleratorErrorV4> {
        Ok(Box::new(Self {
            acquired: vec![0_u64; width],
            delivered: vec![0_u64; width],
            obligations_before: vec![0_u64; width],
            obligations_after: vec![0_u64; width],
            candidate_obligation_state: vec![
                0_u8;
                obligation_account_bytes_v3(request_width).map_err(
                    |_| DealerScenarioAcceleratorErrorV4::Arithmetic
                )?
            ],
            post_inventory: vec![0_u64; width],
            post_counterparty_inventory: vec![0_u64; width],
            post_equity: vec![0_i128; width],
            custody_effects: [None; MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3],
            candidate_scalars: vec![0_u64; scalar_count],
            candidate_identities: vec![[0_u8; 32]; identity_count],
            bank_scratch: vec![0_u8; bank_bytes],
        }))
    }
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn evaluate_authenticated_candidate_v4(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerScenarioTradeRequestV3<'_>,
    width: usize,
    config: DealerConfigV3,
    claims: &ClaimsObservationV4,
    current_obligation: DealerObligationProjectionV3<'_>,
    collateral: &CollateralObservationV4,
    current_slot: u64,
    candidate_bank: &mut [u8],
) -> Result<ScenarioAtomicPlanV3, DealerScenarioAcceleratorErrorV4> {
    let scalar_count = usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)
        .checked_add(width)
        .ok_or(DealerScenarioAcceleratorErrorV4::Arithmetic)?;
    let identity_count = usize::from(DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4);
    let mut scratch = DealerScenarioEvaluationScratchV4::new(
        width,
        request.width,
        scalar_count,
        identity_count,
        candidate_bank.len(),
    )?;

    let context = invocation.context();
    evaluate_dealer_scenario_admitted_v4(
        invocation.family_request(),
        invocation.input_bank(),
        super::v3_trade::ScenarioTradeChainProjectionV3 {
            trading_program: context.trading_program.to_bytes(),
            release_set: request.release_set,
            market: request.market,
            child_root: request.child_root,
            obligation_address: request.obligation,
            current_obligation,
            dealer_position: ClaimsInventoryObservation {
                market_id: request.market,
                product_id: invocation.product_runtime().runtime.product_id.to_bytes(),
                liability_basis_id: invocation.product_runtime().semantic_basis_id.to_bytes(),
                position_owner: request.dealer_owner,
                revision: claims.dealer_revision,
                inventory: &claims.dealer_inventory,
            },
            counterparty_position: ClaimsInventoryObservation {
                market_id: request.market,
                product_id: invocation.product_runtime().runtime.product_id.to_bytes(),
                liability_basis_id: invocation.product_runtime().semantic_basis_id.to_bytes(),
                position_owner: request.counterparty_owner,
                revision: claims.counterparty_revision,
                inventory: &claims.counterparty_inventory,
            },
            product_record_digest: invocation
                .product_runtime()
                .runtime
                .product_record
                .content_digest
                .to_bytes(),
            linked_basis_record_digest: invocation.linked_basis_record().content_digest.to_bytes(),
            counterparty_account: request.counterparty_account,
            principal_balance: collateral.principal.token.amount,
            locked_capital_floor: config.locked_capital_floor(),
            claims_revision: claims.claims_revision,
            generation: request.generation,
            now: current_slot,
            expires_at: request.expires_at,
            terminal: false,
        },
        ScenarioComposerContextV3 {
            trading_program: context.trading_program.to_bytes(),
            custody_program: invocation.custody_program().to_bytes(),
            release_set: request.release_set,
            market: request.market,
            realm: config.realm(),
            child_root: request.child_root,
            obligation_account: request.obligation,
            mint: *collateral.realm.collateral_mint(),
            token_program: *collateral.realm.token_program(),
            parent_request_digest: hash(request.bytes()).to_bytes(),
            generation: request.generation,
            custody_replay_revision: collateral.replay.next_revision,
            locked_capital_floor: config.locked_capital_floor(),
        },
        ScenarioCollateralFrameV3 {
            principal_vault: collateral.principal.key,
            principal_balance: collateral.principal.token.amount,
            fee_vault: collateral.fee.key,
            fee_balance: collateral.fee.token.amount,
            hoard_vault: collateral.hoard.key,
            hoard_balance: collateral.hoard.token.amount,
            counterparty_account: collateral.counterparty.key,
            counterparty_owner: request.counterparty_owner,
            counterparty_external_delegate: collateral
                .counterparty
                .token
                .delegate
                .as_ref()
                .copied()
                .unwrap_or([0; 32]),
            counterparty_external_delegated_amount: collateral.counterparty.token.delegated_amount,
            counterparty_balance: collateral.counterparty.token.amount,
        },
        current_slot,
        DealerScenarioAdmittedBuffersV4 {
            acquired: &mut scratch.acquired,
            delivered: &mut scratch.delivered,
            obligations_before: &mut scratch.obligations_before,
            obligations_after: &mut scratch.obligations_after,
            candidate_obligation_state: &mut scratch.candidate_obligation_state,
            post_inventory: &mut scratch.post_inventory,
            post_counterparty_inventory: &mut scratch.post_counterparty_inventory,
            post_equity: &mut scratch.post_equity,
            custody_effects: &mut scratch.custody_effects,
            candidate_scalars: &mut scratch.candidate_scalars,
            candidate_identities: &mut scratch.candidate_identities,
            bank_scratch: &mut scratch.bank_scratch,
            candidate_bank,
        },
    )
    .map_err(Into::into)
}

struct ClaimsObservationV4 {
    obligation_bytes: Vec<u8>,
    claims_revision: u64,
    dealer_revision: u64,
    counterparty_revision: u64,
    dealer_inventory: Vec<u64>,
    counterparty_inventory: Vec<u64>,
    core_market: [u8; 32],
}

fn authenticate_config(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerScenarioTradeRequestV3<'_>,
    runtime: &[AccountInfo<'_>],
) -> Result<DealerConfigV3, DealerScenarioAcceleratorErrorV4> {
    let descriptor = invocation.descriptor();
    if descriptor.config_schema().to_bytes() != hash(DEALER_CONFIG_SCHEMA_PREIMAGE_V3).to_bytes() {
        return Err(DealerScenarioAcceleratorErrorV4::SemanticJoin);
    }
    let account = runtime
        .get(1)
        .ok_or(DealerScenarioAcceleratorErrorV4::Invocation)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| DealerScenarioAcceleratorErrorV4::SemanticJoin)?;
    if hash(&data).to_bytes() != invocation.context().config.to_bytes() {
        return Err(DealerScenarioAcceleratorErrorV4::SemanticJoin);
    }
    let config = DealerConfigV3::decode(&data)
        .map_err(|_| DealerScenarioAcceleratorErrorV4::SemanticJoin)?;
    if config.release_set() != request.release_set
        || config.market() != request.market
        || config.position_owner() != request.dealer_owner
        || request.release_set != invocation.context().release_set.to_bytes()
        || request.market != invocation.context().market.to_bytes()
        || request.child_root != invocation.context().root.to_bytes()
    {
        return Err(DealerScenarioAcceleratorErrorV4::SemanticJoin);
    }
    Ok(config)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_claims(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerScenarioTradeRequestV3<'_>,
    frame: DealerScenarioLogicalFrameV4,
    runtime: &[AccountInfo<'_>],
    width: usize,
    config: DealerConfigV3,
) -> Result<ClaimsObservationV4, DealerScenarioAcceleratorErrorV4> {
    let claims_start = usize::try_from(frame.claims_fixed_start)
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Arithmetic)?;
    let claims_program = invocation.claims_program().to_bytes();
    let spec = SignedDeltaFrameSpecV3::new(u32::from(request.claims_position_count))
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?;
    let claims_market_index = claims_role_index(spec, ClaimsFrameRoleV1::ClaimsMarket)?;
    let core_market_index = claims_role_index(spec, ClaimsFrameRoleV1::CoreMarket)?;
    let core_program_index = claims_role_index(spec, ClaimsFrameRoleV1::CoreProgram)?;
    let aggregate = relative_account(runtime, claims_start, claims_market_index)?;
    let core_market = relative_account(runtime, claims_start, core_market_index)?;
    let core_program = relative_account(runtime, claims_start, core_program_index)?;
    if aggregate.owner.to_bytes() != claims_program
        || core_market.key.to_bytes() != request.market
        || core_market.owner != core_program.key
        || !core_program.executable
    {
        return Err(DealerScenarioAcceleratorErrorV4::Claims);
    }
    let expected_aggregate = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, &request.market],
        &Pubkey::new_from_array(claims_program),
    )
    .0;
    if aggregate.key != &expected_aggregate {
        return Err(DealerScenarioAcceleratorErrorV4::Claims);
    }
    let aggregate_data = aggregate
        .try_borrow_data()
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?;
    let market = LiabilityBasisMarketViewV2::decode(&aggregate_data)
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?;
    if usize::try_from(market.claim_count).ok() != Some(width)
        || market.revision != request.claims_revision
        || market.logical_market != request.market
        || market.release_set != request.release_set
        || market.registry_program != invocation.context().registry_program.to_bytes()
        || market.product_instance_id
            != invocation
                .product_runtime()
                .runtime
                .product_record
                .content_digest
                .to_bytes()
        || market.basis_id != invocation.product_runtime().semantic_basis_id.to_bytes()
        || market.realm_id != config.realm()
        || market.custody_context != request.child_root
        || market.generation != request.generation
    {
        return Err(DealerScenarioAcceleratorErrorV4::Claims);
    }
    drop(aggregate_data);

    let core_data = core_market
        .try_borrow_data()
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?;
    let core =
        MarketRoot::decode(&core_data).map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?;
    let identity = core.identity();
    if core.phase() != Phase::Open
        || identity.generation() != request.generation
        || identity.realm_id().to_bytes() != config.realm()
        || identity.product_instance_id().to_bytes() != market.product_instance_id
        || identity.claim_basis_id().to_bytes() != market.basis_id
    {
        return Err(DealerScenarioAcceleratorErrorV4::Claims);
    }
    drop(core_data);

    let plan = request
        .claims_plan()
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?;
    let positions_start = usize::try_from(frame.claims_positions_start)
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Arithmetic)?;
    let mut dealer_inventory = vec![0_u64; width];
    let mut counterparty_inventory = vec![0_u64; width];
    let mut dealer_revision = None;
    let mut counterparty_revision = None;
    for index in 0..plan.position_count() {
        let expected = plan
            .position(index)
            .map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?;
        let account = runtime
            .get(
                positions_start
                    .checked_add(
                        usize::try_from(index)
                            .map_err(|_| DealerScenarioAcceleratorErrorV4::Arithmetic)?,
                    )
                    .ok_or(DealerScenarioAcceleratorErrorV4::Arithmetic)?,
            )
            .ok_or(DealerScenarioAcceleratorErrorV4::Claims)?;
        if expected.owner() == request.dealer_owner {
            authenticate_position(
                account,
                claims_program,
                aggregate.key.to_bytes(),
                request.dealer_owner,
                expected.expected_revision(),
                market.basis_id,
                &mut dealer_inventory,
            )?;
            dealer_revision = Some(expected.expected_revision());
        } else if expected.owner() == request.counterparty_owner {
            authenticate_position(
                account,
                claims_program,
                aggregate.key.to_bytes(),
                request.counterparty_owner,
                expected.expected_revision(),
                market.basis_id,
                &mut counterparty_inventory,
            )?;
            counterparty_revision = Some(expected.expected_revision());
        } else {
            return Err(DealerScenarioAcceleratorErrorV4::Claims);
        }
    }
    if dealer_revision.is_none() {
        let dealer_account = runtime
            .get(
                usize::try_from(frame.evidence_start)
                    .map_err(|_| DealerScenarioAcceleratorErrorV4::Arithmetic)?
                    .checked_add(
                        usize::from(request.evidence_span_count)
                            .checked_sub(1)
                            .ok_or(DealerScenarioAcceleratorErrorV4::Claims)?,
                    )
                    .ok_or(DealerScenarioAcceleratorErrorV4::Arithmetic)?,
            )
            .ok_or(DealerScenarioAcceleratorErrorV4::Claims)?;
        authenticate_position(
            dealer_account,
            claims_program,
            aggregate.key.to_bytes(),
            request.dealer_owner,
            request.dealer_position_revision,
            market.basis_id,
            &mut dealer_inventory,
        )?;
        dealer_revision = Some(request.dealer_position_revision);
    }
    let dealer_revision = dealer_revision.ok_or(DealerScenarioAcceleratorErrorV4::Claims)?;
    let counterparty_revision =
        counterparty_revision.ok_or(DealerScenarioAcceleratorErrorV4::Claims)?;
    if dealer_revision != request.dealer_position_revision
        || counterparty_revision != request.counterparty_position_revision
    {
        return Err(DealerScenarioAcceleratorErrorV4::Claims);
    }

    let obligation_account = runtime
        .get(
            usize::try_from(frame.obligation)
                .map_err(|_| DealerScenarioAcceleratorErrorV4::Arithmetic)?,
        )
        .ok_or(DealerScenarioAcceleratorErrorV4::Claims)?;
    let obligation_data = obligation_account
        .try_borrow_data()
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?;
    let obligation_bytes = Vec::from(&obligation_data[..]);
    drop(obligation_data);
    DealerObligationProjectionV3::authenticate(
        invocation.context().trading_program.to_bytes(),
        ObligationAccountObservationV3 {
            address: obligation_account.key.to_bytes(),
            owner: obligation_account.owner.to_bytes(),
            data: &obligation_bytes,
        },
        ObligationExpectationV3 {
            market: request.market,
            product: invocation.product_runtime().runtime.product_id.to_bytes(),
            liability_basis: market.basis_id,
            position_owner: request.dealer_owner,
            child_root: request.child_root,
            revision: request.current_obligation_revision,
            width: request.width,
            state_digest: request.current_obligation_digest,
        },
    )
    .map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?;

    Ok(ClaimsObservationV4 {
        obligation_bytes,
        claims_revision: market.revision,
        dealer_revision,
        counterparty_revision,
        dealer_inventory,
        counterparty_inventory,
        core_market: core_market.key.to_bytes(),
    })
}

fn authenticate_position(
    account: &AccountInfo<'_>,
    claims_program: [u8; 32],
    aggregate: [u8; 32],
    owner: [u8; 32],
    revision: u64,
    basis: [u8; 32],
    output: &mut [u64],
) -> Result<(), DealerScenarioAcceleratorErrorV4> {
    let seeds = ProtocolPositionSeedsV2::new(aggregate, owner)
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?;
    let expected =
        Pubkey::find_program_address(&seeds.as_slices(), &Pubkey::new_from_array(claims_program)).0;
    if account.key != &expected || account.owner.to_bytes() != claims_program {
        return Err(DealerScenarioAcceleratorErrorV4::Claims);
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?;
    let position = LiabilityBasisPositionViewV2::decode(&data)
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?;
    if usize::try_from(position.claim_count).ok() != Some(output.len())
        || position.market_account != aggregate
        || position.owner != owner
        || position.basis_id != basis
        || position.revision != revision
    {
        return Err(DealerScenarioAcceleratorErrorV4::Claims);
    }
    for (index, destination) in output.iter_mut().enumerate() {
        *destination = position
            .balance(
                &data,
                u32::try_from(index).map_err(|_| DealerScenarioAcceleratorErrorV4::Arithmetic)?,
            )
            .map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_collateral(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerScenarioTradeRequestV3<'_>,
    frame: DealerScenarioLogicalFrameV4,
    spans: [u32; DEALER_SCENARIO_PROFILE_SPANS_V4],
    runtime: &[AccountInfo<'_>],
    config: DealerConfigV3,
    core_market: [u8; 32],
) -> Result<CollateralObservationV4, DealerScenarioAcceleratorErrorV4> {
    if core_market != request.market {
        return Err(DealerScenarioAcceleratorErrorV4::Custody);
    }
    let active = scenario_custody_span_widths(spans);
    if scenario_collateral_evidence_count(active, request.dealer_evidence_count)?
        != request.evidence_span_count
    {
        return Err(DealerScenarioAcceleratorErrorV4::Custody);
    }
    let first_slot = active
        .iter()
        .position(|count| *count != 0)
        .ok_or(DealerScenarioAcceleratorErrorV4::Custody)?;
    let first_start = *frame
        .custody_starts
        .get(first_slot)
        .ok_or(DealerScenarioAcceleratorErrorV4::Custody)?;
    let first = custody_frame(runtime, first_start)?;
    let realm = authenticate_realm(
        invocation,
        config,
        first
            .get(6)
            .ok_or(DealerScenarioAcceleratorErrorV4::Custody)?,
        first
            .get(7)
            .ok_or(DealerScenarioAcceleratorErrorV4::Custody)?,
    )?;
    let custody_program = invocation.custody_program().to_bytes();
    let authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::from_request(custody_shape_request(
            request,
            config,
            invocation.context().trading_program.to_bytes(),
            *realm.collateral_mint(),
            *realm.token_program(),
        ))
        .as_slices(),
        &Pubkey::new_from_array(custody_program),
    )
    .0
    .to_bytes();
    let replay_seeds = CustodyReplaySeedsV1::from_request(custody_shape_request(
        request,
        config,
        invocation.context().trading_program.to_bytes(),
        *realm.collateral_mint(),
        *realm.token_program(),
    ));
    let replay_key = Pubkey::find_program_address(
        &replay_seeds.as_slices(),
        &Pubkey::new_from_array(custody_program),
    )
    .0;
    let replay_account = first
        .get(8)
        .ok_or(DealerScenarioAcceleratorErrorV4::Custody)?;
    if replay_account.key != &replay_key
        || replay_account.owner.to_bytes() != custody_program
        || replay_account.data_len() != CUSTODY_REPLAY_BYTES_V1
    {
        return Err(DealerScenarioAcceleratorErrorV4::Custody);
    }
    let replay_data = replay_account
        .try_borrow_data()
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Custody)?;
    let replay = CustodyReplayV1::decode(&replay_data)
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Custody)?;
    if replay.caller_role != CallerRoleV1::Trading
        || replay.release_set != request.release_set
        || replay.market != request.market
        || replay.realm != config.realm()
        || replay.context != request.child_root
        || replay.caller_program != invocation.context().trading_program.to_bytes()
        || replay.generation != request.generation
    {
        return Err(DealerScenarioAcceleratorErrorV4::Custody);
    }
    drop(replay_data);

    let principal_key = vault_key(
        custody_program,
        request,
        request.child_root,
        CompartmentV1::TradingPrincipal,
    );
    let fee_key = vault_key(
        custody_program,
        request,
        request.child_root,
        CompartmentV1::FeeVault,
    );
    let hoard_key = vault_key(
        custody_program,
        request,
        request.market,
        CompartmentV1::HoardPrincipal,
    );
    let mint = *realm.collateral_mint();
    let token_program = *realm.token_program();
    let mut principal = None;
    let mut fee = None;
    let mut hoard = None;
    let mut counterparty = None;
    for slot in 0..6 {
        let count = *active
            .get(slot)
            .ok_or(DealerScenarioAcceleratorErrorV4::Custody)?;
        if count == 0 {
            continue;
        }
        if count != u32::from(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3) {
            return Err(DealerScenarioAcceleratorErrorV4::Custody);
        }
        let route = custody_frame(
            runtime,
            *frame
                .custody_starts
                .get(slot)
                .ok_or(DealerScenarioAcceleratorErrorV4::Custody)?,
        )?;
        authenticate_custody_common(
            invocation,
            request,
            config,
            route,
            first,
            replay_key,
            authority,
            mint,
            token_program,
        )?;
        let source = observe_token(
            route
                .get(10)
                .ok_or(DealerScenarioAcceleratorErrorV4::Custody)?,
            mint,
            token_program,
        )?;
        let destination = observe_token(
            route
                .get(11)
                .ok_or(DealerScenarioAcceleratorErrorV4::Custody)?,
            mint,
            token_program,
        )?;
        match slot {
            0 => {
                set_observation(&mut counterparty, source, request.counterparty_account)?;
                set_observation(&mut principal, destination, principal_key)?;
            }
            1 => {
                set_observation(&mut counterparty, source, request.counterparty_account)?;
                set_observation(&mut fee, destination, fee_key)?;
            }
            2 => {
                set_observation(&mut principal, source, principal_key)?;
                set_observation(&mut fee, destination, fee_key)?;
            }
            3 => {
                set_observation(&mut principal, source, principal_key)?;
                set_observation(&mut hoard, destination, hoard_key)?;
            }
            4 => {
                set_observation(&mut hoard, source, hoard_key)?;
                set_observation(&mut principal, destination, principal_key)?;
            }
            5 => {
                set_observation(&mut principal, source, principal_key)?;
                set_observation(&mut counterparty, destination, request.counterparty_account)?;
            }
            _ => return Err(DealerScenarioAcceleratorErrorV4::Custody),
        }
    }

    let mut evidence_cursor = usize::try_from(frame.evidence_start)
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Arithmetic)?;
    if active.get(1) == Some(&0) && active.get(2) == Some(&0) {
        let account = runtime
            .get(evidence_cursor)
            .ok_or(DealerScenarioAcceleratorErrorV4::Custody)?;
        set_observation(
            &mut fee,
            observe_token(account, mint, token_program)?,
            fee_key,
        )?;
        evidence_cursor = evidence_cursor
            .checked_add(1)
            .ok_or(DealerScenarioAcceleratorErrorV4::Arithmetic)?;
    }
    if active.get(3) == Some(&0) && active.get(4) == Some(&0) {
        let account = runtime
            .get(evidence_cursor)
            .ok_or(DealerScenarioAcceleratorErrorV4::Custody)?;
        set_observation(
            &mut hoard,
            observe_token(account, mint, token_program)?,
            hoard_key,
        )?;
        evidence_cursor = evidence_cursor
            .checked_add(1)
            .ok_or(DealerScenarioAcceleratorErrorV4::Arithmetic)?;
    }
    evidence_cursor = evidence_cursor
        .checked_add(usize::from(request.dealer_evidence_count))
        .ok_or(DealerScenarioAcceleratorErrorV4::Arithmetic)?;
    if evidence_cursor
        != usize::try_from(frame.logical_account_count)
            .map_err(|_| DealerScenarioAcceleratorErrorV4::Arithmetic)?
    {
        return Err(DealerScenarioAcceleratorErrorV4::Custody);
    }
    let principal = principal.ok_or(DealerScenarioAcceleratorErrorV4::Custody)?;
    let fee = fee.ok_or(DealerScenarioAcceleratorErrorV4::Custody)?;
    let hoard = hoard.ok_or(DealerScenarioAcceleratorErrorV4::Custody)?;
    let counterparty = counterparty.ok_or(DealerScenarioAcceleratorErrorV4::Custody)?;
    if !is_canonical_internal_vault(principal, authority)
        || !is_canonical_internal_vault(fee, authority)
        || !is_canonical_internal_vault(hoard, authority)
        || counterparty.token.owner != request.counterparty_owner
        || [principal.key, fee.key, hoard.key].contains(&counterparty.key)
    {
        return Err(DealerScenarioAcceleratorErrorV4::Custody);
    }
    Ok(CollateralObservationV4 {
        principal,
        fee,
        hoard,
        counterparty,
        realm,
        replay,
    })
}

fn scenario_custody_span_widths(spans: [u32; DEALER_SCENARIO_PROFILE_SPANS_V4]) -> [u32; 6] {
    [spans[0], spans[1], spans[2], spans[3], spans[5], spans[6]]
}

fn scenario_collateral_evidence_count(
    custody: [u32; 6],
    dealer_evidence_count: u8,
) -> Result<u8, DealerScenarioAcceleratorErrorV4> {
    if dealer_evidence_count > 1 {
        return Err(DealerScenarioAcceleratorErrorV4::Custody);
    }
    u8::from(custody[1] == 0 && custody[2] == 0)
        .checked_add(u8::from(custody[3] == 0 && custody[4] == 0))
        .and_then(|value| value.checked_add(dealer_evidence_count))
        .filter(|value| *value <= 3)
        .ok_or(DealerScenarioAcceleratorErrorV4::Arithmetic)
}

fn is_canonical_internal_vault(observed: TokenObservationV4, authority: [u8; 32]) -> bool {
    observed.token.owner == authority
        && observed.token.delegate.is_none()
        && observed.token.delegated_amount == 0
        && observed.token.close_authority.is_none()
}

fn authenticate_realm(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    config: DealerConfigV3,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
) -> Result<RealmV1, DealerScenarioAcceleratorErrorV4> {
    let registry = invocation.context().registry_program.to_bytes();
    let raw_key = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &config.realm(),
        ],
        &Pubkey::new_from_array(registry),
    )
    .0;
    let staging_key = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &config.realm(),
        ],
        &Pubkey::new_from_array(registry),
    )
    .0;
    if raw.key != &raw_key
        || raw.owner.to_bytes() != registry
        || staging.key != &staging_key
        || staging.owner != &system_program::ID
        || !staging.data_is_empty()
    {
        return Err(DealerScenarioAcceleratorErrorV4::Custody);
    }
    let data = raw
        .try_borrow_data()
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Custody)?;
    if hash(&data).to_bytes() != config.realm() {
        return Err(DealerScenarioAcceleratorErrorV4::Custody);
    }
    RealmV1::decode(&data).map_err(|_| DealerScenarioAcceleratorErrorV4::Custody)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_custody_common(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerScenarioTradeRequestV3<'_>,
    config: DealerConfigV3,
    route: &[AccountInfo<'_>],
    representative: &[AccountInfo<'_>],
    replay: Pubkey,
    authority: [u8; 32],
    mint: [u8; 32],
    token_program: [u8; 32],
) -> Result<(), DealerScenarioAcceleratorErrorV4> {
    for index in [1_usize, 2, 5, 6, 7, 8, 9, 12, 13] {
        if route.get(index).map(|value| value.key)
            != representative.get(index).map(|value| value.key)
        {
            return Err(DealerScenarioAcceleratorErrorV4::Custody);
        }
    }
    if route.get(1).map(|value| value.key.to_bytes()) != Some(request.market)
        || route.get(3).map(|value| value.key.to_bytes())
            != Some(invocation.context().registry_program.to_bytes())
        || route.get(4).map(|value| value.key.to_bytes())
            != Some(invocation.context().trading_program.to_bytes())
        || route.get(8).map(|value| *value.key) != Some(replay)
        || route.get(9).map(|value| value.key.to_bytes()) != Some(mint)
        || route.get(12).map(|value| value.key.to_bytes()) != Some(authority)
        || route.get(13).map(|value| value.key.to_bytes()) != Some(token_program)
        || route.get(13).is_none_or(|value| !value.executable)
        || config.realm() == [0; 32]
    {
        return Err(DealerScenarioAcceleratorErrorV4::Custody);
    }
    Ok(())
}

fn observe_token(
    account: &AccountInfo<'_>,
    mint: [u8; 32],
    token_program: [u8; 32],
) -> Result<TokenObservationV4, DealerScenarioAcceleratorErrorV4> {
    if account.owner.to_bytes() != token_program || account.data_len() != ACCOUNT_BYTES {
        return Err(DealerScenarioAcceleratorErrorV4::Custody);
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| DealerScenarioAcceleratorErrorV4::Custody)?;
    let token =
        TokenAccount::parse(&data).map_err(|_| DealerScenarioAcceleratorErrorV4::Custody)?;
    if token.mint != mint
        || token.state != AccountState::Initialized
        || !matches!(token.native_reserve, COption::None)
    {
        return Err(DealerScenarioAcceleratorErrorV4::Custody);
    }
    Ok(TokenObservationV4 {
        key: account.key.to_bytes(),
        token,
    })
}

fn set_observation(
    destination: &mut Option<TokenObservationV4>,
    observed: TokenObservationV4,
    expected_key: [u8; 32],
) -> Result<(), DealerScenarioAcceleratorErrorV4> {
    if observed.key != expected_key || destination.is_some_and(|prior| prior.key != observed.key) {
        return Err(DealerScenarioAcceleratorErrorV4::Custody);
    }
    *destination = Some(observed);
    Ok(())
}

fn vault_key(
    custody_program: [u8; 32],
    request: DealerScenarioTradeRequestV3<'_>,
    context: [u8; 32],
    compartment: CompartmentV1,
) -> [u8; 32] {
    Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(request.market, request.release_set, context, compartment)
            .as_slices(),
        &Pubkey::new_from_array(custody_program),
    )
    .0
    .to_bytes()
}

fn custody_shape_request(
    request: DealerScenarioTradeRequestV3<'_>,
    config: DealerConfigV3,
    trading_program: [u8; 32],
    mint: [u8; 32],
    token_program: [u8; 32],
) -> dclutch_custody_contract::CustodyRequestV1 {
    dclutch_custody_contract::CustodyRequestV1 {
        operation: dclutch_custody_contract::OperationV1::Transfer,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::TradingPrincipal,
        destination_compartment: CompartmentV1::FeeVault,
        release_set: request.release_set,
        market: request.market,
        realm: config.realm(),
        context: request.child_root,
        caller_program: trading_program,
        semantic: dclutch_custody_contract::ContextV1 {
            candidate: request.obligation,
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order: request.counterparty_owner,
            parent_request_digest: hash(request.bytes()).to_bytes(),
            order_nonce: 0,
            generation: request.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: [0; 32],
        destination: [0; 32],
        source_vault_context: request.child_root,
        destination_vault_context: request.child_root,
        mint,
        token_program,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: 1,
        resulting_revision: 2,
        amount: 1,
        rent_lamports: 0,
    }
}

fn custody_frame<'accounts, 'info>(
    runtime: &'accounts [AccountInfo<'info>],
    start: u32,
) -> Result<&'accounts [AccountInfo<'info>], DealerScenarioAcceleratorErrorV4> {
    let start = usize::try_from(start).map_err(|_| DealerScenarioAcceleratorErrorV4::Arithmetic)?;
    let end = start
        .checked_add(usize::from(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3))
        .ok_or(DealerScenarioAcceleratorErrorV4::Arithmetic)?;
    runtime
        .get(start..end)
        .ok_or(DealerScenarioAcceleratorErrorV4::Custody)
}

fn claims_role_index(
    spec: SignedDeltaFrameSpecV3,
    role: ClaimsFrameRoleV1,
) -> Result<usize, DealerScenarioAcceleratorErrorV4> {
    for index in 0..SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 {
        if spec
            .account(index)
            .map_err(|_| DealerScenarioAcceleratorErrorV4::Claims)?
            .role()
            == role
        {
            return Ok(usize::from(index));
        }
    }
    Err(DealerScenarioAcceleratorErrorV4::Claims)
}

fn relative_account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    start: usize,
    relative: usize,
) -> Result<&'a AccountInfo<'info>, DealerScenarioAcceleratorErrorV4> {
    accounts
        .get(
            start
                .checked_add(relative)
                .ok_or(DealerScenarioAcceleratorErrorV4::Arithmetic)?,
        )
        .ok_or(DealerScenarioAcceleratorErrorV4::Claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_span_never_becomes_a_custody_slot() {
        let spans = [14, 0, 14, 0, 2, 0, 14, 3];
        assert_eq!(scenario_custody_span_widths(spans), [14, 0, 14, 0, 0, 14]);
    }

    #[test]
    fn evidence_width_is_exactly_absent_fee_hoard_and_dealer_position() {
        assert_eq!(
            scenario_collateral_evidence_count([14, 14, 0, 14, 0, 0], 0),
            Ok(0)
        );
        assert_eq!(
            scenario_collateral_evidence_count([0, 0, 0, 0, 0, 14], 1),
            Ok(3)
        );
        assert_eq!(
            scenario_collateral_evidence_count([14, 14, 0, 0, 14, 0], 2),
            Err(DealerScenarioAcceleratorErrorV4::Custody)
        );
    }
}
