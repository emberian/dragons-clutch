//! Physical selector-1..=6 authentication for admitted Dealer equity.
//!
//! Common Hot authenticates the release, finalized artifacts, request bytes,
//! logical account expansion, and input register bank. This adapter rejoins
//! those facts to the live Dealer root, obligation, LP Position, complete
//! Dealer/LP Claims inventory evidence, Realm-selected collateral accounts,
//! and Custody replay before calling the canonical V3 equity planner. It owns
//! no alternate equity arithmetic, state mutation, or child dispatch.

extern crate alloc;

use alloc::{boxed::Box, vec, vec::Vec};

use dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1;
use dclutch_claims_svm::{
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_BUMP_OFFSET_V2, LIABILITY_BASIS_MARKET_SEED_V2,
        LIABILITY_BASIS_POSITION_BUMP_OFFSET_V2, LiabilityBasisMarketViewV2,
        LiabilityBasisPositionViewV2,
    },
    protocol_position_v2::ProtocolPositionSeedsV2,
};
use dclutch_custody_contract::{
    CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1, CustodyAuthoritySeedsV1,
    CustodyReplaySeedsV1, CustodyReplayV1, CustodyVaultSeedsV1,
};
use dclutch_dealer_codec::{
    Phase,
    config_v4::{DEALER_CONFIG_SCHEMA_PREIMAGE_V4, DealerConfigV4},
    root_tail::{ROOT_TAIL_BYTES, RootTail},
    scenario::ClaimsInventoryObservation,
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, STATE_BYTES};
use dclutch_realm_contract::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_token_svm::{ACCOUNT_BYTES, AccountState, COption, TokenAccount};
use solana_program::{account_info::AccountInfo, hash::hash, pubkey::Pubkey};
use solana_sdk_ids::system_program;

use crate::hot_v3::AuthenticatedAcceleratorInvocationV4;

use super::{
    v3_equity_operator::{
        DealerEquityBumpBankV3, DealerEquityRequestV3, EquityPoolChainProjectionV3,
        EquityRequestActionV3, claims_projection_digest_v3, collateral_projection_digest_v3,
        prepare_equity_request_v3,
    },
    v3_hot_artifact::{
        DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3, DEALER_EQUITY_LP_REVISION_SCALAR_V3,
        DEALER_EQUITY_LP_SHARES_SCALAR_V3, DEALER_EQUITY_OBLIGATION_REVISION_SCALAR_V3,
        DEALER_EQUITY_PARENT_REQUEST_DIGEST_IDENTITY_V3,
        DEALER_EQUITY_POSITION_EVIDENCE_ACCOUNT_COUNT_V3, DEALER_EQUITY_TOTAL_SHARES_SCALAR_V3,
        DEALER_EQUITY_WITNESS_BYTES_SCALAR_V3, DEALER_EQUITY_WITNESS_OFFSET_SCALAR_V3,
        DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3, DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
        DealerCustodyIdentityFieldV3, DealerCustodyScalarFieldV3,
        dealer_current_slot_scalar_register_v3, dealer_custody_identity_register_v3,
        dealer_custody_scalar_register_v3, dealer_equity_evidence_owner_identity_register_v3,
        dealer_equity_identity_count_v3, dealer_equity_scalar_count_v3,
        dealer_equity_witness_offset_v3, dealer_expiry_scalar_register_v3,
        dealer_external_delegate_identity_register_v3,
    },
    v3_multi_lp::{
        DEALER_LP_POSITION_BYTES_V3, DEALER_LP_POSITION_PDA_DOMAIN_V3, DealerLpPositionV3,
        MAX_MULTI_LP_CUSTODY_EFFECTS_V3, MultiLpActionV3, MultiLpBumpHintsV3,
        MultiLpCollateralFrameV3, MultiLpContextV3, MultiLpCustodyEffectV3,
        MultiLpCustodyRequestV3, MultiLpPlanV3, multi_lp_custody_digest_v3,
    },
    v3_obligation::{
        DealerObligationProjectionV3, ObligationAccountObservationV3, ObligationExpectationV3,
    },
    v3_profile::dealer_equity_logical_account_count_v3,
};
use crate::market_admission_v1::TRADING_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1;

const CLAIMS_MARKET_RELATIVE_V4: usize = 1;
const CLAIMS_BASIS_RECORD_RELATIVE_V4: usize = 2;
const CLAIMS_BASIS_STAGING_RELATIVE_V4: usize = 3;
const CLAIMS_PRODUCT_RECORD_RELATIVE_V4: usize = 4;
const CLAIMS_PRODUCT_STAGING_RELATIVE_V4: usize = 5;
const CLAIMS_DOMAIN_RECORD_RELATIVE_V4: usize = 6;
const CLAIMS_DOMAIN_STAGING_RELATIVE_V4: usize = 7;
const CLAIMS_PORTFOLIO_RECORD_RELATIVE_V4: usize = 8;
const CLAIMS_PORTFOLIO_STAGING_RELATIVE_V4: usize = 9;
const CLAIMS_CORE_MARKET_RELATIVE_V4: usize = 11;
const CLAIMS_REGISTRY_RELATIVE_V4: usize = 13;
const CLAIMS_CALLER_PROGRAM_RELATIVE_V4: usize = 14;
const CLAIMS_PROGRAM_RELATIVE_V4: usize = 16;
const CLAIMS_CORE_PROGRAM_RELATIVE_V4: usize = 18;

const CUSTODY_CORE_MARKET_RELATIVE_V4: usize = 1;
const CUSTODY_ACTIVATION_RELATIVE_V4: usize = 2;
const CUSTODY_REGISTRY_RELATIVE_V4: usize = 3;
const CUSTODY_CALLER_PROGRAM_RELATIVE_V4: usize = 4;
const CUSTODY_CALLER_PROGRAMDATA_RELATIVE_V4: usize = 5;
const CUSTODY_REALM_RAW_RELATIVE_V4: usize = 6;
const CUSTODY_REALM_STAGING_RELATIVE_V4: usize = 7;
const CUSTODY_REPLAY_RELATIVE_V4: usize = 8;
const CUSTODY_MINT_RELATIVE_V4: usize = 9;
const CUSTODY_SOURCE_RELATIVE_V4: usize = 10;
const CUSTODY_DESTINATION_RELATIVE_V4: usize = 11;
const CUSTODY_AUTHORITY_RELATIVE_V4: usize = 12;
const CUSTODY_TOKEN_PROGRAM_RELATIVE_V4: usize = 13;

/// Stable refusal at selectors 1..=6's physical accelerator boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerEquityAcceleratorErrorV4 {
    /// Selector, register, request, span, or logical account geometry differed.
    Invocation,
    /// Release, root, config, Product, Core Market, or immutable joins differed.
    SemanticJoin,
    /// Claims aggregate or complete Dealer/LP Position evidence differed.
    Claims,
    /// Trading obligation or LP Position PDA/prestate differed.
    PoolState,
    /// Realm, replay, vault, token, or Custody frame differed.
    Custody,
    /// Canonical equity planning or register projection refused.
    Transition,
    /// Checked account, register, or byte arithmetic overflowed.
    Arithmetic,
}

impl DealerEquityAcceleratorErrorV4 {
    /// This refusal's own word, for the one log line a reader greps.
    ///
    /// The accelerator boundary decides ACCEPTED or REFUSED and has no wire
    /// field for which conjunct refused, so `.is_ok()` was throwing this away
    /// at the one line that held it. Exhaustive on purpose: a new variant does
    /// not compile until its author writes the word a validator log will carry.
    #[must_use]
    pub const fn refusal_name(self) -> &'static str {
        match self {
            Self::Invocation => "equity:Invocation",
            Self::SemanticJoin => "equity:SemanticJoin",
            Self::Claims => "equity:Claims",
            Self::PoolState => "equity:PoolState",
            Self::Custody => "equity:Custody",
            Self::Transition => "equity:Transition",
            Self::Arithmetic => "equity:Arithmetic",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EquityFrameV4 {
    action: MultiLpActionV3,
    signed_position_count: u32,
    claims_start: usize,
    custody_starts: [usize; MAX_MULTI_LP_CUSTODY_EFFECTS_V3],
    custody_route_count: usize,
    obligation: usize,
    lp_position: usize,
    custody_program: usize,
    evidence_start: usize,
    logical_account_count: usize,
}

struct ClaimsObservationV4 {
    market_revision: u64,
    dealer_inventory: Vec<u64>,
    lp_inventory: Vec<u64>,
}

struct PoolStateObservationV4 {
    obligation_bytes: Vec<u8>,
    lp_bytes: Vec<u8>,
    lp: DealerLpPositionV3,
}

#[derive(Clone, Copy)]
struct TokenObservationV4 {
    key: [u8; 32],
    token: TokenAccount,
}

#[derive(Clone, Copy)]
struct CollateralObservationV4 {
    frame: MultiLpCollateralFrameV3,
    realm: RealmV1,
    replay: CustodyReplayV1,
}

/// Authenticate one selector-1..=6 invocation and evaluate its exact bank.
///
/// `candidate_bank` is commit-last and remains byte-for-byte unchanged on
/// every refusal. The returned plan is the canonical V3 physical plan whose
/// values were projected into that bank.
pub fn evaluate_authenticated_dealer_equity_v4(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    candidate_bank: &mut [u8],
) -> Result<MultiLpPlanV3, DealerEquityAcceleratorErrorV4> {
    let request = DealerEquityRequestV3::decode(invocation.family_request())
        .map_err(|_| DealerEquityAcceleratorErrorV4::Invocation)?;
    let signed_position_count = request
        .claims_plan()
        .map_err(|_| DealerEquityAcceleratorErrorV4::Invocation)?
        .map_or(0, |plan| plan.position_count());
    let frame = equity_frame(request.action(), signed_position_count)?;
    let runtime = invocation.runtime_accounts();
    authenticate_invocation_geometry(invocation, request, frame, runtime, candidate_bank.len())?;
    let config = authenticate_context(invocation, request, runtime)?;
    let claims = authenticate_claims(invocation, request, frame, runtime, config)?;
    let pool = authenticate_pool_state(invocation, request, frame, runtime)?;
    let collateral = authenticate_collateral(invocation, request, frame, runtime, config)?;
    evaluate_equity_commit_last(
        invocation,
        &request,
        &config,
        &claims,
        &pool,
        &collateral,
        candidate_bank,
    )
}

fn equity_frame(
    request_action: EquityRequestActionV3,
    signed_position_count: u32,
) -> Result<EquityFrameV4, DealerEquityAcceleratorErrorV4> {
    let action = action_for_request(request_action);
    let signed_positions = usize::try_from(signed_position_count)
        .map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?;
    if signed_position_count > u32::from(DEALER_EQUITY_POSITION_EVIDENCE_ACCOUNT_COUNT_V3) {
        return Err(DealerEquityAcceleratorErrorV4::Invocation);
    }
    let claims_start = usize::from(DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3)
        .checked_add(usize::from(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3))
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let claims_count = usize::from(DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3)
        .checked_add(signed_positions)
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let later_start = claims_start
        .checked_add(claims_count)
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let custody_route_count = match action {
        MultiLpActionV3::Add => 2,
        MultiLpActionV3::Remove => 3,
    };
    let custody_width = usize::from(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3);
    let mut custody_starts = [usize::MAX; MAX_MULTI_LP_CUSTODY_EFFECTS_V3];
    custody_starts[0] = usize::from(DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3);
    for route in 1..custody_route_count {
        custody_starts[route] = later_start
            .checked_add(
                route
                    .checked_sub(1)
                    .and_then(|value| value.checked_mul(custody_width))
                    .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?,
            )
            .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    }
    let obligation = later_start
        .checked_add(
            custody_route_count
                .checked_sub(1)
                .and_then(|value| value.checked_mul(custody_width))
                .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?,
        )
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let lp_position = obligation
        .checked_add(1)
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let custody_program = lp_position
        .checked_add(1)
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let evidence_start = custody_program
        .checked_add(1)
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let logical_account_count = usize::from(
        dealer_equity_logical_account_count_v3(action, signed_position_count)
            .map_err(|_| DealerEquityAcceleratorErrorV4::Invocation)?,
    );
    if evidence_start.checked_add(usize::from(
        DEALER_EQUITY_POSITION_EVIDENCE_ACCOUNT_COUNT_V3,
    )) != Some(logical_account_count)
    {
        return Err(DealerEquityAcceleratorErrorV4::Invocation);
    }
    Ok(EquityFrameV4 {
        action,
        signed_position_count,
        claims_start,
        custody_starts,
        custody_route_count,
        obligation,
        lp_position,
        custody_program,
        evidence_start,
        logical_account_count,
    })
}

fn authenticate_invocation_geometry(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerEquityRequestV3<'_>,
    frame: EquityFrameV4,
    runtime: &[AccountInfo<'_>],
    candidate_bytes: usize,
) -> Result<(), DealerEquityAcceleratorErrorV4> {
    let expected_scalars = dealer_equity_scalar_count_v3(frame.action)
        .map_err(|_| DealerEquityAcceleratorErrorV4::Invocation)?;
    let expected_identities = dealer_equity_identity_count_v3(frame.action)
        .map_err(|_| DealerEquityAcceleratorErrorV4::Invocation)?;
    let bank_bytes = expected_scalars
        .checked_mul(8)
        .and_then(|value| value.checked_add(expected_identities.checked_mul(32)?))
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    // `context().tail_count != request.width` USED TO BE THE FIRST CONJUNCT HERE
    // AND IT COULD NOT BE SATISFIED. The context's `tail_count` is the width the
    // EXECUTOR carved the account frame at -- `project_tail_count`'s answer --
    // and the equity AccountProfile is a fixed topology that projects no tail,
    // so it is zero. `request.width` is the CLAIMS width, which is the product's
    // outcome count and is at least two. Two different quantities, compared for
    // equality, on a route where they can never be equal: the same shape
    // `bfc8383f` repaired in `require_tail_count_agreement_v3` and the same one
    // `require_accelerator_tail_count_v4` repaired one frame up.
    //
    // Nothing is weakened by its absence, and the chain that replaces it is
    // exact. `require_accelerator_tail_count_v4` binds the request's tail to
    // `project_tail_count(profile, product_outcome_count)`;
    // `authenticate_accelerator_context_v4` puts that same value in the context
    // and refuses unless the context digest reproduces the request's; and the
    // conjunct below binds the product's outcome count to `request.width`. On a
    // profile that DOES project a tail those three compose to exactly the
    // equality this line stated. On one that does not, they compose to the
    // truth instead of to a contradiction.
    if invocation.selected_action() != u32::from(request.selector())
        || invocation.product_runtime().runtime.outcome_count != request.width
        || runtime.len() != frame.logical_account_count
        || invocation.request().scalar_count()
            != u32::try_from(expected_scalars)
                .map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?
        || invocation.request().identity_count()
            != u32::try_from(expected_identities)
                .map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?
        || invocation.scalars().len() != expected_scalars
        || invocation.identities().len() != expected_identities
        || invocation.input_bank().len() != bank_bytes
        || candidate_bytes != bank_bytes
        || runtime
            .iter()
            .any(|account| account.is_signer || account.is_writable)
    {
        return Err(DealerEquityAcceleratorErrorV4::Invocation);
    }
    let current = dealer_current_slot_scalar_register_v3(frame.action)
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let expiry = dealer_expiry_scalar_register_v3(frame.action)
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let evidence_owner = dealer_equity_evidence_owner_identity_register_v3(frame.action)
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let trading =
        dealer_custody_identity_register_v3(0, DealerCustodyIdentityFieldV3::CallerProgram)
            .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    if scalar(invocation.scalars(), current)? > request.expires_at
        || scalar(invocation.scalars(), expiry)? != request.expires_at
        || scalar(invocation.scalars(), DEALER_EQUITY_WITNESS_BYTES_SCALAR_V3)?
            != u64::try_from(request.claims_packet().len())
                .map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?
        || identity(invocation.identities(), evidence_owner)?
            != invocation.claims_program().to_bytes()
        || identity(invocation.identities(), trading)?
            != invocation.context().trading_program.to_bytes()
    {
        return Err(DealerEquityAcceleratorErrorV4::Invocation);
    }
    Ok(())
}

fn authenticate_context(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerEquityRequestV3<'_>,
    runtime: &[AccountInfo<'_>],
) -> Result<DealerConfigV4, DealerEquityAcceleratorErrorV4> {
    let context = invocation.context();
    if request.release_set != context.release_set.to_bytes()
        || request.market != context.market.to_bytes()
        || request.child_root != context.root.to_bytes()
        || request.generation != invocation.envelope().generation()
        || invocation.descriptor().config_schema().to_bytes()
            != hash(DEALER_CONFIG_SCHEMA_PREIMAGE_V4).to_bytes()
    {
        return Err(DealerEquityAcceleratorErrorV4::SemanticJoin);
    }
    let root = account(runtime, 0, DealerEquityAcceleratorErrorV4::SemanticJoin)?;
    let config_account = account(runtime, 1, DealerEquityAcceleratorErrorV4::SemanticJoin)?;
    if root.key.to_bytes() != request.child_root
        || root.owner.to_bytes() != context.trading_program.to_bytes()
        || root.executable
        || config_account.executable
    {
        return Err(DealerEquityAcceleratorErrorV4::SemanticJoin);
    }
    let root_data = root
        .try_borrow_data()
        .map_err(|_| DealerEquityAcceleratorErrorV4::SemanticJoin)?;
    let tail_end = CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(ROOT_TAIL_BYTES)
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    if root_data.len() != tail_end {
        return Err(DealerEquityAcceleratorErrorV4::SemanticJoin);
    }
    let tail = RootTail::decode(
        root_data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..tail_end)
            .ok_or(DealerEquityAcceleratorErrorV4::SemanticJoin)?,
    )
    .map_err(|_| DealerEquityAcceleratorErrorV4::SemanticJoin)?;
    if tail.phase != Phase::Open {
        return Err(DealerEquityAcceleratorErrorV4::SemanticJoin);
    }
    drop(root_data);

    let config_data = config_account
        .try_borrow_data()
        .map_err(|_| DealerEquityAcceleratorErrorV4::SemanticJoin)?;
    if hash(&config_data).to_bytes() != context.config.to_bytes() {
        return Err(DealerEquityAcceleratorErrorV4::SemanticJoin);
    }
    let config = DealerConfigV4::decode(&config_data)
        .map_err(|_| DealerEquityAcceleratorErrorV4::SemanticJoin)?;
    if config.release_set() != request.release_set
        || config.position_owner() != request.dealer_position_owner
        || config.locked_capital_floor() != request.locked_capital_floor
    {
        return Err(DealerEquityAcceleratorErrorV4::SemanticJoin);
    }
    Ok(config)
}

#[inline(never)]
fn authenticate_claims(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerEquityRequestV3<'_>,
    frame: EquityFrameV4,
    runtime: &[AccountInfo<'_>],
    config: DealerConfigV4,
) -> Result<ClaimsObservationV4, DealerEquityAcceleratorErrorV4> {
    let width =
        usize::try_from(request.width).map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let claims_program = invocation.claims_program().to_bytes();
    let aggregate = relative_account(
        runtime,
        frame.claims_start,
        CLAIMS_MARKET_RELATIVE_V4,
        DealerEquityAcceleratorErrorV4::Claims,
    )?;
    let core_market = relative_account(
        runtime,
        frame.claims_start,
        CLAIMS_CORE_MARKET_RELATIVE_V4,
        DealerEquityAcceleratorErrorV4::Claims,
    )?;
    let core_program = relative_account(
        runtime,
        frame.claims_start,
        CLAIMS_CORE_PROGRAM_RELATIVE_V4,
        DealerEquityAcceleratorErrorV4::Claims,
    )?;
    authenticate_claims_fixed_joins(invocation, frame, runtime, core_market, core_program)?;
    let aggregate_data = aggregate
        .try_borrow_data()
        .map_err(|_| DealerEquityAcceleratorErrorV4::Claims)?;
    // The aggregate carries the bump Claims recorded when it founded the
    // account, so this walk reproduces the address instead of searching for it.
    let expected_aggregate = derive_hinted(
        &[LIABILITY_BASIS_MARKET_SEED_V2, &request.market],
        &Pubkey::new_from_array(claims_program),
        recorded_bump(&aggregate_data, LIABILITY_BASIS_MARKET_BUMP_OFFSET_V2),
    )
    .0;
    if aggregate.key != &expected_aggregate || aggregate.owner.to_bytes() != claims_program {
        return Err(DealerEquityAcceleratorErrorV4::Claims);
    }
    let market = LiabilityBasisMarketViewV2::decode(&aggregate_data)
        .map_err(|_| DealerEquityAcceleratorErrorV4::Claims)?;
    if usize::try_from(market.claim_count).ok() != Some(width)
        || market.logical_market != request.market
        || market.release_set != request.release_set
        || market.registry_program != invocation.context().registry_program.to_bytes()
        || market.product_instance_id != invocation.product_runtime().runtime.product_id.to_bytes()
        || market.basis_id != invocation.product_runtime().semantic_basis_id.to_bytes()
        || market.realm_id != config.realm()
        || market.custody_context != request.child_root
        || market.generation != request.generation
    {
        return Err(DealerEquityAcceleratorErrorV4::Claims);
    }
    drop(aggregate_data);
    authenticate_core_market(
        invocation,
        request,
        config,
        core_market,
        core_program,
        market,
    )?;

    let signed = request
        .claims_plan()
        .map_err(|_| DealerEquityAcceleratorErrorV4::Claims)?;
    if signed.map_or(0, |plan| plan.position_count()) != frame.signed_position_count {
        return Err(DealerEquityAcceleratorErrorV4::Claims);
    }
    let mut dealer_inventory = vec![0_u64; width];
    let mut lp_inventory = vec![0_u64; width];
    let mut dealer_seen = false;
    let mut lp_seen = false;
    for evidence_offset in 0..usize::from(DEALER_EQUITY_POSITION_EVIDENCE_ACCOUNT_COUNT_V3) {
        let evidence = account(
            runtime,
            frame
                .evidence_start
                .checked_add(evidence_offset)
                .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?,
            DealerEquityAcceleratorErrorV4::Claims,
        )?;
        if evidence_offset
            < usize::try_from(frame.signed_position_count)
                .map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?
        {
            let signed_account = account(
                runtime,
                frame
                    .claims_start
                    .checked_add(usize::from(DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3))
                    .and_then(|value| value.checked_add(evidence_offset))
                    .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?,
                DealerEquityAcceleratorErrorV4::Claims,
            )?;
            if evidence.key != signed_account.key {
                return Err(DealerEquityAcceleratorErrorV4::Claims);
            }
        }
        let data = evidence
            .try_borrow_data()
            .map_err(|_| DealerEquityAcceleratorErrorV4::Claims)?;
        let position = LiabilityBasisPositionViewV2::decode(&data)
            .map_err(|_| DealerEquityAcceleratorErrorV4::Claims)?;
        let expected_revision = if position.owner == request.dealer_position_owner {
            if dealer_seen {
                return Err(DealerEquityAcceleratorErrorV4::Claims);
            }
            dealer_seen = true;
            request.dealer_claims_revision
        } else if position.owner == request.lp_owner {
            if lp_seen {
                return Err(DealerEquityAcceleratorErrorV4::Claims);
            }
            lp_seen = true;
            request.lp_claims_revision
        } else {
            return Err(DealerEquityAcceleratorErrorV4::Claims);
        };
        if let Some(plan) = signed {
            if evidence_offset
                < usize::try_from(plan.position_count())
                    .map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?
            {
                let expected = plan
                    .position(
                        u32::try_from(evidence_offset)
                            .map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?,
                    )
                    .map_err(|_| DealerEquityAcceleratorErrorV4::Claims)?;
                if expected.owner() != position.owner
                    || expected.expected_revision() != expected_revision
                {
                    return Err(DealerEquityAcceleratorErrorV4::Claims);
                }
            }
        }
        let seeds = ProtocolPositionSeedsV2::new(aggregate.key.to_bytes(), position.owner)
            .map_err(|_| DealerEquityAcceleratorErrorV4::Claims)?;
        let expected_key = derive_hinted(
            &seeds.as_slices(),
            &Pubkey::new_from_array(claims_program),
            recorded_bump(&data, LIABILITY_BASIS_POSITION_BUMP_OFFSET_V2),
        )
        .0;
        if evidence.key != &expected_key
            || evidence.owner.to_bytes() != claims_program
            || usize::try_from(position.claim_count).ok() != Some(width)
            || position.market_account != aggregate.key.to_bytes()
            || position.basis_id != market.basis_id
            || position.revision != expected_revision
        {
            return Err(DealerEquityAcceleratorErrorV4::Claims);
        }
        let destination = if position.owner == request.dealer_position_owner {
            &mut dealer_inventory
        } else {
            &mut lp_inventory
        };
        for (index, value) in destination.iter_mut().enumerate() {
            *value = position
                .balance(
                    &data,
                    u32::try_from(index).map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?,
                )
                .map_err(|_| DealerEquityAcceleratorErrorV4::Claims)?;
        }
    }
    if !dealer_seen || !lp_seen {
        return Err(DealerEquityAcceleratorErrorV4::Claims);
    }
    let dealer = ClaimsInventoryObservation {
        market_id: request.market,
        product_id: invocation.product_runtime().runtime.product_id.to_bytes(),
        liability_basis_id: market.basis_id,
        position_owner: request.dealer_position_owner,
        revision: request.dealer_claims_revision,
        inventory: &dealer_inventory,
    };
    let lp = ClaimsInventoryObservation {
        market_id: request.market,
        product_id: invocation.product_runtime().runtime.product_id.to_bytes(),
        liability_basis_id: market.basis_id,
        position_owner: request.lp_owner,
        revision: request.lp_claims_revision,
        inventory: &lp_inventory,
    };
    if claims_projection_digest_v3(dealer) != request.dealer_claims_digest
        || claims_projection_digest_v3(lp) != request.lp_claims_digest
    {
        return Err(DealerEquityAcceleratorErrorV4::Claims);
    }
    Ok(ClaimsObservationV4 {
        market_revision: market.revision,
        dealer_inventory,
        lp_inventory,
    })
}

fn authenticate_claims_fixed_joins<'info>(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    frame: EquityFrameV4,
    runtime: &[AccountInfo<'info>],
    core_market: &AccountInfo<'info>,
    core_program: &AccountInfo<'info>,
) -> Result<(), DealerEquityAcceleratorErrorV4> {
    let product = invocation.product_runtime();
    let expected = [
        (
            CLAIMS_BASIS_RECORD_RELATIVE_V4,
            product.linked_basis_record.raw_account,
        ),
        (
            CLAIMS_BASIS_STAGING_RELATIVE_V4,
            product.linked_basis_record.staging_account,
        ),
        (
            CLAIMS_PRODUCT_RECORD_RELATIVE_V4,
            product.runtime.product_record.raw_account,
        ),
        (
            CLAIMS_PRODUCT_STAGING_RELATIVE_V4,
            product.runtime.product_record.staging_account,
        ),
        (
            CLAIMS_DOMAIN_RECORD_RELATIVE_V4,
            product.runtime.result_domain_record.raw_account,
        ),
        (
            CLAIMS_DOMAIN_STAGING_RELATIVE_V4,
            product.runtime.result_domain_record.staging_account,
        ),
        (
            CLAIMS_PORTFOLIO_RECORD_RELATIVE_V4,
            product.runtime.portfolio_record.raw_account,
        ),
        (
            CLAIMS_PORTFOLIO_STAGING_RELATIVE_V4,
            product.runtime.portfolio_record.staging_account,
        ),
    ];
    for (relative, key) in expected {
        if relative_account(
            runtime,
            frame.claims_start,
            relative,
            DealerEquityAcceleratorErrorV4::Claims,
        )?
        .key != &key
        {
            return Err(DealerEquityAcceleratorErrorV4::Claims);
        }
    }
    let registry = relative_account(
        runtime,
        frame.claims_start,
        CLAIMS_REGISTRY_RELATIVE_V4,
        DealerEquityAcceleratorErrorV4::Claims,
    )?;
    let caller = relative_account(
        runtime,
        frame.claims_start,
        CLAIMS_CALLER_PROGRAM_RELATIVE_V4,
        DealerEquityAcceleratorErrorV4::Claims,
    )?;
    let claims = relative_account(
        runtime,
        frame.claims_start,
        CLAIMS_PROGRAM_RELATIVE_V4,
        DealerEquityAcceleratorErrorV4::Claims,
    )?;
    if registry.key.to_bytes() != invocation.context().registry_program.to_bytes()
        || caller.key.to_bytes() != invocation.context().trading_program.to_bytes()
        || claims.key.to_bytes() != invocation.claims_program().to_bytes()
        || core_market.key.to_bytes() != invocation.context().market.to_bytes()
        || core_market.owner != core_program.key
        || core_market.data_len() != STATE_BYTES
        || [registry, caller, claims, core_program]
            .iter()
            .any(|account| !account.executable)
    {
        return Err(DealerEquityAcceleratorErrorV4::Claims);
    }
    Ok(())
}

fn authenticate_core_market(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerEquityRequestV3<'_>,
    config: DealerConfigV4,
    core_market: &AccountInfo<'_>,
    core_program: &AccountInfo<'_>,
    aggregate: LiabilityBasisMarketViewV2,
) -> Result<(), DealerEquityAcceleratorErrorV4> {
    let data = core_market
        .try_borrow_data()
        .map_err(|_| DealerEquityAcceleratorErrorV4::Claims)?;
    let core = CoreState::decode(&data).map_err(|_| DealerEquityAcceleratorErrorV4::Claims)?;
    let core_seeds = MarketCoreStateSeedsV2::new(core.identity);
    let derived = derive_hinted(
        &core_seeds.as_slices(),
        core_program.key,
        request.bumps().core_market(),
    )
    .0;
    if derived != *core_market.key
        || !TRADING_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(core.phase)
        || core.identity.market_id.to_bytes() != request.market
        || core.identity.generation != request.generation
        || core.identity.realm_id.to_bytes() != config.realm()
        || core.identity.product_id.to_bytes() != aggregate.product_instance_id
        || core.identity.product_record.to_bytes()
            != invocation
                .product_runtime()
                .runtime
                .product_record
                .content_digest
                .to_bytes()
        || core.identity.selected_release_set.to_bytes() != request.release_set
        || core.identity.registry_program.to_bytes()
            != invocation.context().registry_program.to_bytes()
    {
        return Err(DealerEquityAcceleratorErrorV4::Claims);
    }
    Ok(())
}

#[inline(never)]
fn authenticate_pool_state(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerEquityRequestV3<'_>,
    frame: EquityFrameV4,
    runtime: &[AccountInfo<'_>],
) -> Result<PoolStateObservationV4, DealerEquityAcceleratorErrorV4> {
    let trading = invocation.context().trading_program.to_bytes();
    let obligation_account = account(
        runtime,
        frame.obligation,
        DealerEquityAcceleratorErrorV4::PoolState,
    )?;
    let obligation_data = obligation_account
        .try_borrow_data()
        .map_err(|_| DealerEquityAcceleratorErrorV4::PoolState)?;
    let obligation_bytes = Vec::from(&obligation_data[..]);
    drop(obligation_data);
    DealerObligationProjectionV3::authenticate(
        trading,
        ObligationAccountObservationV3 {
            address: obligation_account.key.to_bytes(),
            owner: obligation_account.owner.to_bytes(),
            data: &obligation_bytes,
        },
        ObligationExpectationV3 {
            market: request.market,
            product: invocation.product_runtime().runtime.product_id.to_bytes(),
            liability_basis: invocation.product_runtime().semantic_basis_id.to_bytes(),
            position_owner: request.dealer_position_owner,
            child_root: request.child_root,
            revision: request.obligation_revision,
            width: request.width,
            state_digest: request.obligation_digest,
        },
    )
    .map_err(|_| DealerEquityAcceleratorErrorV4::PoolState)?;

    let lp_account = account(
        runtime,
        frame.lp_position,
        DealerEquityAcceleratorErrorV4::PoolState,
    )?;
    if lp_account.key.to_bytes() != request.lp_position
        || lp_account.owner.to_bytes() != trading
        || lp_account.data_len() != DEALER_LP_POSITION_BYTES_V3
    {
        return Err(DealerEquityAcceleratorErrorV4::PoolState);
    }
    let lp_data = lp_account
        .try_borrow_data()
        .map_err(|_| DealerEquityAcceleratorErrorV4::PoolState)?;
    if hash(&lp_data).to_bytes() != request.lp_digest {
        return Err(DealerEquityAcceleratorErrorV4::PoolState);
    }
    let lp = DealerLpPositionV3::decode(&lp_data)
        .map_err(|_| DealerEquityAcceleratorErrorV4::PoolState)?;
    // The Position carries the bump Trading recorded when it created the
    // account, and this route already required that byte to be canonical, so
    // the search it fed is the one derivation the body could always replace.
    let expected = derive_hinted(
        &[
            DEALER_LP_POSITION_PDA_DOMAIN_V3,
            &request.child_root,
            &request.lp_owner,
        ],
        &Pubkey::new_from_array(trading),
        u8::try_from(lp.pda_bump).unwrap_or(0),
    );
    if lp_account.key != &expected.0 {
        return Err(DealerEquityAcceleratorErrorV4::PoolState);
    }
    if lp.revision != request.lp_revision
        || lp.release_set != request.release_set
        || lp.market != request.market
        || lp.child_root != request.child_root
        || lp.lp_owner != request.lp_owner
        || lp.obligation_account != request.obligation
        || lp.generation != request.generation
        || lp.pda_bump != u16::from(expected.1)
        || lp_account.lamports() < lp.rent_principal
    {
        return Err(DealerEquityAcceleratorErrorV4::PoolState);
    }
    let lp_bytes = Vec::from(&lp_data[..]);
    Ok(PoolStateObservationV4 {
        obligation_bytes,
        lp_bytes,
        lp,
    })
}

#[inline(never)]
fn authenticate_collateral(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerEquityRequestV3<'_>,
    frame: EquityFrameV4,
    runtime: &[AccountInfo<'_>],
    config: DealerConfigV4,
) -> Result<CollateralObservationV4, DealerEquityAcceleratorErrorV4> {
    let first = custody_frame(runtime, frame.custody_starts[0])?;
    let realm = authenticate_realm(
        invocation,
        config,
        request.bumps(),
        first
            .get(CUSTODY_REALM_RAW_RELATIVE_V4)
            .ok_or(DealerEquityAcceleratorErrorV4::Custody)?,
        first
            .get(CUSTODY_REALM_STAGING_RELATIVE_V4)
            .ok_or(DealerEquityAcceleratorErrorV4::Custody)?,
    )?;
    let custody_program = invocation.custody_program().to_bytes();
    let bumps = request.bumps();
    let authority = derive_hinted(
        &CustodyAuthoritySeedsV1::new(request.market, request.release_set).as_slices(),
        &Pubkey::new_from_array(custody_program),
        bumps.custody_authority(),
    )
    .0
    .to_bytes();
    let replay_key = derive_hinted(
        &CustodyReplaySeedsV1::new(
            request.market,
            request.release_set,
            CallerRoleV1::Trading,
            request.child_root,
        )
        .as_slices(),
        &Pubkey::new_from_array(custody_program),
        bumps.custody_replay(),
    )
    .0;
    let replay_account = first
        .get(CUSTODY_REPLAY_RELATIVE_V4)
        .ok_or(DealerEquityAcceleratorErrorV4::Custody)?;
    if replay_account.key != &replay_key
        || replay_account.owner.to_bytes() != custody_program
        || replay_account.data_len() != CUSTODY_REPLAY_BYTES_V1
    {
        return Err(DealerEquityAcceleratorErrorV4::Custody);
    }
    let replay_data = replay_account
        .try_borrow_data()
        .map_err(|_| DealerEquityAcceleratorErrorV4::Custody)?;
    let replay = CustodyReplayV1::decode(&replay_data)
        .map_err(|_| DealerEquityAcceleratorErrorV4::Custody)?;
    if replay.caller_role != CallerRoleV1::Trading
        || replay.release_set != request.release_set
        || replay.market != request.market
        || replay.realm != config.realm()
        || replay.context != request.child_root
        || replay.caller_program != invocation.context().trading_program.to_bytes()
        || replay.generation != request.generation
    {
        return Err(DealerEquityAcceleratorErrorV4::Custody);
    }
    drop(replay_data);

    let principal_key = vault_key(
        custody_program,
        request.market,
        request.release_set,
        request.child_root,
        CompartmentV1::TradingPrincipal,
        bumps.principal_vault(),
    );
    let hoard_key = vault_key(
        custody_program,
        request.market,
        request.release_set,
        request.market,
        CompartmentV1::HoardPrincipal,
        bumps.hoard_vault(),
    );
    let mint = *realm.collateral_mint();
    let token_program = *realm.token_program();
    let callee = account(
        runtime,
        frame.custody_program,
        DealerEquityAcceleratorErrorV4::Custody,
    )?;
    if callee.key.to_bytes() != custody_program || !callee.executable {
        return Err(DealerEquityAcceleratorErrorV4::Custody);
    }
    for route_index in 0..frame.custody_route_count {
        let route = custody_frame(runtime, frame.custody_starts[route_index])?;
        authenticate_custody_common(
            invocation,
            request,
            route,
            first,
            replay_key,
            authority,
            mint,
            token_program,
        )?;
    }
    let (external_account, principal_account, hoard_account) = match frame.action {
        MultiLpActionV3::Add => {
            let first_source = frame_account(first, CUSTODY_SOURCE_RELATIVE_V4)?;
            let first_destination = frame_account(first, CUSTODY_DESTINATION_RELATIVE_V4)?;
            let merge = custody_frame(runtime, frame.custody_starts[1])?;
            let merge_source = frame_account(merge, CUSTODY_SOURCE_RELATIVE_V4)?;
            let merge_destination = frame_account(merge, CUSTODY_DESTINATION_RELATIVE_V4)?;
            if first_destination.key.to_bytes() != principal_key
                || merge_source.key.to_bytes() != hoard_key
                || merge_destination.key.to_bytes() != principal_key
            {
                return Err(DealerEquityAcceleratorErrorV4::Custody);
            }
            (first_source, first_destination, merge_source)
        }
        MultiLpActionV3::Remove => {
            let split = first;
            let payout = custody_frame(runtime, frame.custody_starts[1])?;
            let merge = custody_frame(runtime, frame.custody_starts[2])?;
            let principal = frame_account(split, CUSTODY_SOURCE_RELATIVE_V4)?;
            let hoard = frame_account(split, CUSTODY_DESTINATION_RELATIVE_V4)?;
            let payout_source = frame_account(payout, CUSTODY_SOURCE_RELATIVE_V4)?;
            let external = frame_account(payout, CUSTODY_DESTINATION_RELATIVE_V4)?;
            let merge_source = frame_account(merge, CUSTODY_SOURCE_RELATIVE_V4)?;
            let merge_destination = frame_account(merge, CUSTODY_DESTINATION_RELATIVE_V4)?;
            if principal.key.to_bytes() != principal_key
                || hoard.key.to_bytes() != hoard_key
                || payout_source.key.to_bytes() != principal_key
                || merge_source.key.to_bytes() != hoard_key
                || merge_destination.key.to_bytes() != principal_key
            {
                return Err(DealerEquityAcceleratorErrorV4::Custody);
            }
            (external, principal, hoard)
        }
    };
    let external = observe_token(external_account, mint, token_program)?;
    let principal = observe_token(principal_account, mint, token_program)?;
    let hoard = observe_token(hoard_account, mint, token_program)?;
    if external.token.owner != request.lp_owner
        || external.key == principal.key
        || external.key == hoard.key
        || principal.key == hoard.key
        || !is_canonical_internal_vault(principal, authority)
        || !is_canonical_internal_vault(hoard, authority)
    {
        return Err(DealerEquityAcceleratorErrorV4::Custody);
    }
    let collateral = MultiLpCollateralFrameV3 {
        lp_external_account: external.key,
        lp_owner: request.lp_owner,
        lp_external_balance: external.token.amount,
        lp_external_delegate: external.token.delegate.as_ref().copied().unwrap_or([0; 32]),
        lp_external_delegated_amount: external.token.delegated_amount,
        principal_vault: principal.key,
        principal_balance: principal.token.amount,
        hoard_vault: hoard.key,
        hoard_balance: hoard.token.amount,
    };
    if collateral_projection_digest_v3(collateral) != request.collateral_digest {
        return Err(DealerEquityAcceleratorErrorV4::Custody);
    }
    Ok(CollateralObservationV4 {
        frame: collateral,
        realm,
        replay,
    })
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn evaluate_equity_commit_last(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: &DealerEquityRequestV3<'_>,
    config: &DealerConfigV4,
    claims: &ClaimsObservationV4,
    pool: &PoolStateObservationV4,
    collateral: &CollateralObservationV4,
    candidate_bank: &mut [u8],
) -> Result<MultiLpPlanV3, DealerEquityAcceleratorErrorV4> {
    let width =
        usize::try_from(request.width).map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let current_slot_register =
        dealer_current_slot_scalar_register_v3(action_for_request(request.action()))
            .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let current_slot = scalar(invocation.scalars(), current_slot_register)?;
    let trading_program = invocation.context().trading_program.to_bytes();
    let chain = EquityPoolChainProjectionV3 {
        trading_program,
        // The bank rides with the projection so a host that rebuilds this
        // request from it reproduces the same signed header byte for byte.
        bumps: request.bumps(),
        release_set: request.release_set,
        market: request.market,
        child_root: request.child_root,
        obligation_address: request.obligation,
        obligation: DealerObligationProjectionV3::decode(&pool.obligation_bytes)
            .map_err(|_| DealerEquityAcceleratorErrorV4::PoolState)?,
        lp_position_address: request.lp_position,
        lp_position: pool.lp,
        lp_position_bytes: &pool.lp_bytes,
        dealer_claims: ClaimsInventoryObservation {
            market_id: request.market,
            product_id: invocation.product_runtime().runtime.product_id.to_bytes(),
            liability_basis_id: invocation.product_runtime().semantic_basis_id.to_bytes(),
            position_owner: request.dealer_position_owner,
            revision: request.dealer_claims_revision,
            inventory: &claims.dealer_inventory,
        },
        lp_claims: ClaimsInventoryObservation {
            market_id: request.market,
            product_id: invocation.product_runtime().runtime.product_id.to_bytes(),
            liability_basis_id: invocation.product_runtime().semantic_basis_id.to_bytes(),
            position_owner: request.lp_owner,
            revision: request.lp_claims_revision,
            inventory: &claims.lp_inventory,
        },
        product_record_digest: invocation
            .product_runtime()
            .runtime
            .product_record
            .content_digest
            .to_bytes(),
        linked_basis_record_digest: invocation.linked_basis_record().content_digest.to_bytes(),
        basis_scale: invocation.product_runtime().payout_scale,
        claims_market_revision: claims.market_revision,
        collateral: collateral.frame,
        locked_capital_floor: config.locked_capital_floor(),
        generation: request.generation,
        now: current_slot,
        expires_at: request.expires_at,
        terminal: false,
    };
    let semantic_context = MultiLpContextV3 {
        // The planner's own walk over three addresses this evaluator derived
        // moments ago; the relay is process-local and every one is re-derived
        // and re-compared there.
        bumps: MultiLpBumpHintsV3 {
            obligation: request.bumps().obligation(),
            principal_vault: request.bumps().principal_vault(),
            hoard_vault: request.bumps().hoard_vault(),
        },
        trading_program,
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
        // The scale comes from the SAME authenticated ProductBasisV3 record
        // whose `semantic_basis_id` was already joined to the Claims
        // aggregate's `basis_id` above. Nothing here declares it; the basis
        // owns it, and this route only reads it after that join has held.
        basis_scale: invocation.product_runtime().payout_scale,
    };
    let mut request_claims = vec![0_u64; width];
    let mut obligation_scratch = vec![0_u64; width];
    let mut residual_before = vec![0_u64; width];
    let mut residual_after = vec![0_u64; width];
    let mut claims_transferred = vec![0_u64; width];
    let mut post_dealer_claims = vec![0_u64; width];
    let mut post_lp_claims = vec![0_u64; width];
    let mut post_obligation = vec![0_u8; pool.obligation_bytes.len()];
    let mut post_lp = vec![0_u8; DEALER_LP_POSITION_BYTES_V3];
    // Each entry owns a full encoded Custody request. Keeping both the
    // commit-last scratch bank and the accepted effect bank inline gives this
    // SBF frame more than 12 KiB of stack. Their bounded geometry remains the
    // canonical fixed array; only its temporary storage moves to the heap.
    let mut custody_scratch: Box<
        [Option<MultiLpCustodyEffectV3>; MAX_MULTI_LP_CUSTODY_EFFECTS_V3],
    > = vec![None; MAX_MULTI_LP_CUSTODY_EFFECTS_V3]
        .into_boxed_slice()
        .try_into()
        .map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let mut custody_effects: Box<
        [Option<MultiLpCustodyEffectV3>; MAX_MULTI_LP_CUSTODY_EFFECTS_V3],
    > = vec![None; MAX_MULTI_LP_CUSTODY_EFFECTS_V3]
        .into_boxed_slice()
        .try_into()
        .map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let plan = prepare_equity_request_v3(
        request,
        &chain,
        &semantic_context,
        &mut request_claims,
        &mut obligation_scratch,
        &mut residual_before,
        &mut residual_after,
        &mut claims_transferred,
        &mut post_dealer_claims,
        &mut post_lp_claims,
        &mut post_obligation,
        &mut post_lp,
        &mut custody_scratch,
        &mut custody_effects,
    )
    .map_err(|_| DealerEquityAcceleratorErrorV4::Transition)?;
    project_candidate_bank(
        request,
        &plan,
        &custody_effects,
        current_slot,
        invocation.scalars(),
        invocation.identities(),
        candidate_bank,
    )?;
    Ok(plan)
}

/// SBF-side adapter for the host-only canonical register projector.
///
/// All semantic values below come from `prepare_equity_request_v3` and its
/// canonical child requests. This function mirrors only the documented Hot
/// register ABI because `project_dealer_equity_hot_registers_v3` is currently
/// gated out of `target_os = "solana"` with its host allocation helpers.
#[inline(never)]
fn project_candidate_bank(
    request: &DealerEquityRequestV3<'_>,
    plan: &MultiLpPlanV3,
    custody_effects: &[Option<MultiLpCustodyEffectV3>; MAX_MULTI_LP_CUSTODY_EFFECTS_V3],
    trusted_current_slot: u64,
    input_scalars: &[u64],
    input_identities: &[[u8; 32]],
    candidate_bank: &mut [u8],
) -> Result<(), DealerEquityAcceleratorErrorV4> {
    let request_action = action_for_request(request.action());
    if request_action != plan.action
        || request.shares != plan.share_delta
        || (plan.action == MultiLpActionV3::Add && request.collateral != plan.collateral_in)
        || (plan.action == MultiLpActionV3::Remove && request.collateral != 0)
        || trusted_current_slot > request.expires_at
        || multi_lp_custody_digest_v3(custody_effects, plan.custody_count)
            .map_err(|_| DealerEquityAcceleratorErrorV4::Transition)?
            != plan.custody_digest
    {
        return Err(DealerEquityAcceleratorErrorV4::Transition);
    }
    let scalar_count = dealer_equity_scalar_count_v3(plan.action)
        .map_err(|_| DealerEquityAcceleratorErrorV4::Transition)?;
    let identity_count = dealer_equity_identity_count_v3(plan.action)
        .map_err(|_| DealerEquityAcceleratorErrorV4::Transition)?;
    if input_scalars.len() != scalar_count || input_identities.len() != identity_count {
        return Err(DealerEquityAcceleratorErrorV4::Invocation);
    }
    // Admitted AOT must reproduce TransitionVM's complete output bank. V3
    // starts by copying its authenticated input, so AccountProfile-owned facts
    // such as the Claims evidence owner survive unless the canonical
    // Transition explicitly overwrites them.
    let mut scalars = input_scalars.to_vec();
    let mut identities = input_identities.to_vec();
    set_scalar(
        &mut scalars,
        DEALER_EQUITY_OBLIGATION_REVISION_SCALAR_V3,
        plan.obligation_revision_after,
    )?;
    set_scalar(
        &mut scalars,
        DEALER_EQUITY_TOTAL_SHARES_SCALAR_V3,
        plan.total_equity_shares_after,
    )?;
    set_scalar(
        &mut scalars,
        DEALER_EQUITY_LP_REVISION_SCALAR_V3,
        plan.lp_revision_after,
    )?;
    set_scalar(
        &mut scalars,
        DEALER_EQUITY_LP_SHARES_SCALAR_V3,
        plan.lp_equity_shares_after,
    )?;
    set_scalar(
        &mut scalars,
        DEALER_EQUITY_WITNESS_OFFSET_SCALAR_V3,
        u64::try_from(dealer_equity_witness_offset_v3())
            .map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?,
    )?;
    set_scalar(
        &mut scalars,
        DEALER_EQUITY_WITNESS_BYTES_SCALAR_V3,
        u64::try_from(request.claims_packet().len())
            .map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?,
    )?;
    set_scalar(
        &mut scalars,
        dealer_current_slot_scalar_register_v3(plan.action)
            .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?,
        trusted_current_slot,
    )?;
    set_scalar(
        &mut scalars,
        dealer_expiry_scalar_register_v3(plan.action)
            .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?,
        request.expires_at,
    )?;
    set_identity(
        &mut identities,
        DEALER_EQUITY_PARENT_REQUEST_DIGEST_IDENTITY_V3,
        hash(request.bytes()).to_bytes(),
    )?;

    // The canonical fixed slot bank owns full typed Custody requests. Keep its
    // bounded geometry while allocating the temporary bank directly on the
    // heap so the SBF projector remains below the 4 KiB frame ceiling.
    let mut by_slot: Box<[Option<MultiLpCustodyRequestV3>; MAX_MULTI_LP_CUSTODY_EFFECTS_V3]> =
        vec![None; MAX_MULTI_LP_CUSTODY_EFFECTS_V3]
            .into_boxed_slice()
            .try_into()
            .map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let active_count = usize::from(plan.custody_count);
    if active_count > custody_effects.len()
        || custody_effects
            .iter()
            .skip(active_count)
            .any(Option::is_some)
    {
        return Err(DealerEquityAcceleratorErrorV4::Transition);
    }
    for effect in custody_effects.iter().take(active_count) {
        let child = effect
            .ok_or(DealerEquityAcceleratorErrorV4::Transition)?
            .request;
        if child.custody().semantic.parent_request_digest != hash(request.bytes()).to_bytes() {
            return Err(DealerEquityAcceleratorErrorV4::Transition);
        }
        let slot =
            custody_slot(plan.action, child).ok_or(DealerEquityAcceleratorErrorV4::Transition)?;
        if by_slot
            .get_mut(slot)
            .ok_or(DealerEquityAcceleratorErrorV4::Transition)?
            .replace(child)
            .is_some()
        {
            return Err(DealerEquityAcceleratorErrorV4::Transition);
        }
    }
    let expected_amounts = match plan.action {
        MultiLpActionV3::Add => [plan.collateral_in, plan.maximum_complete_sets_to_merge, 0],
        MultiLpActionV3::Remove => [
            plan.minimum_complete_sets_to_split,
            plan.collateral_out,
            plan.maximum_complete_sets_to_merge,
        ],
    };
    let slots = match plan.action {
        MultiLpActionV3::Add => 2,
        MultiLpActionV3::Remove => 3,
    };
    for (slot, expected_amount) in expected_amounts.iter().copied().take(slots).enumerate() {
        match by_slot[slot] {
            Some(child) if expected_amount != 0 && child.custody().amount == expected_amount => {
                project_custody_registers(
                    u16::try_from(slot).map_err(|_| DealerEquityAcceleratorErrorV4::Arithmetic)?,
                    child,
                    &mut scalars,
                    &mut identities,
                )?;
            }
            None if expected_amount == 0 => {}
            _ => return Err(DealerEquityAcceleratorErrorV4::Transition),
        }
    }
    let staged = encode_bank(&scalars, &identities, candidate_bank.len())?;
    candidate_bank.copy_from_slice(&staged);
    Ok(())
}

fn project_custody_registers(
    slot: u16,
    request: MultiLpCustodyRequestV3,
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<(), DealerEquityAcceleratorErrorV4> {
    let custody = request.custody();
    for (field, value) in [
        (
            DealerCustodyScalarFieldV3::TransferIndex,
            u64::from(custody.semantic.transfer_index),
        ),
        (
            DealerCustodyScalarFieldV3::ExpectedRevision,
            custody.expected_revision,
        ),
        (
            DealerCustodyScalarFieldV3::ResultingRevision,
            custody.resulting_revision,
        ),
        (
            DealerCustodyScalarFieldV3::OrderNonce,
            custody.semantic.order_nonce,
        ),
        (
            DealerCustodyScalarFieldV3::Generation,
            custody.semantic.generation,
        ),
        (DealerCustodyScalarFieldV3::Amount, custody.amount),
        (
            DealerCustodyScalarFieldV3::RentLamports,
            custody.rent_lamports,
        ),
        (
            DealerCustodyScalarFieldV3::PageIndex,
            u64::from(custody.semantic.page_index),
        ),
        (
            DealerCustodyScalarFieldV3::ExecutionIndex,
            u64::from(custody.semantic.execution_index),
        ),
    ] {
        set_scalar(
            scalars,
            dealer_custody_scalar_register_v3(slot, field)
                .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?,
            value,
        )?;
    }
    let values = [
        custody.release_set,
        custody.market,
        custody.realm,
        custody.context,
        custody.caller_program,
        custody.semantic.candidate,
        custody.semantic.source_owner,
        custody.semantic.destination_owner,
        custody.semantic.order,
        custody.source,
        custody.destination,
        custody.source_vault_context,
        custody.destination_vault_context,
        custody.mint,
        custody.token_program,
        custody.payer,
        custody.rent_refund,
    ];
    let fields = [
        DealerCustodyIdentityFieldV3::ReleaseSet,
        DealerCustodyIdentityFieldV3::Market,
        DealerCustodyIdentityFieldV3::Realm,
        DealerCustodyIdentityFieldV3::Context,
        DealerCustodyIdentityFieldV3::CallerProgram,
        DealerCustodyIdentityFieldV3::Candidate,
        DealerCustodyIdentityFieldV3::SourceOwner,
        DealerCustodyIdentityFieldV3::DestinationOwner,
        DealerCustodyIdentityFieldV3::Order,
        DealerCustodyIdentityFieldV3::Source,
        DealerCustodyIdentityFieldV3::Destination,
        DealerCustodyIdentityFieldV3::SourceVaultContext,
        DealerCustodyIdentityFieldV3::DestinationVaultContext,
        DealerCustodyIdentityFieldV3::Mint,
        DealerCustodyIdentityFieldV3::TokenProgram,
        DealerCustodyIdentityFieldV3::Payer,
        DealerCustodyIdentityFieldV3::RentRefund,
    ];
    for (field, value) in fields.into_iter().zip(values) {
        set_identity(
            identities,
            dealer_custody_identity_register_v3(slot, field)
                .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?,
            value,
        )?;
    }
    if let MultiLpCustodyRequestV3::Delegated(delegated) = request {
        set_identity(
            identities,
            dealer_external_delegate_identity_register_v3(MultiLpActionV3::Add)
                .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?,
            delegated.delegate_before,
        )?;
    }
    Ok(())
}

fn custody_slot(action: MultiLpActionV3, request: MultiLpCustodyRequestV3) -> Option<usize> {
    let custody = request.custody();
    match (
        action,
        request,
        custody.source_compartment,
        custody.destination_compartment,
    ) {
        (
            MultiLpActionV3::Add,
            MultiLpCustodyRequestV3::Delegated(_),
            CompartmentV1::External,
            CompartmentV1::TradingPrincipal,
        ) => Some(0),
        (
            MultiLpActionV3::Add,
            MultiLpCustodyRequestV3::Canonical(_),
            CompartmentV1::HoardPrincipal,
            CompartmentV1::TradingPrincipal,
        ) => Some(1),
        (
            MultiLpActionV3::Remove,
            MultiLpCustodyRequestV3::Canonical(_),
            CompartmentV1::TradingPrincipal,
            CompartmentV1::HoardPrincipal,
        ) => Some(0),
        (
            MultiLpActionV3::Remove,
            MultiLpCustodyRequestV3::Canonical(_),
            CompartmentV1::TradingPrincipal,
            CompartmentV1::External,
        ) => Some(1),
        (
            MultiLpActionV3::Remove,
            MultiLpCustodyRequestV3::Canonical(_),
            CompartmentV1::HoardPrincipal,
            CompartmentV1::TradingPrincipal,
        ) => Some(2),
        _ => None,
    }
}

fn authenticate_realm(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    config: DealerConfigV4,
    bumps: DealerEquityBumpBankV3,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
) -> Result<RealmV1, DealerEquityAcceleratorErrorV4> {
    let registry = invocation.context().registry_program.to_bytes();
    let raw_key = derive_hinted(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &config.realm(),
        ],
        &Pubkey::new_from_array(registry),
        bumps.realm_raw_record(),
    )
    .0;
    let staging_key = derive_hinted(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &config.realm(),
        ],
        &Pubkey::new_from_array(registry),
        bumps.realm_staging_record(),
    )
    .0;
    if raw.key != &raw_key
        || raw.owner.to_bytes() != registry
        || staging.key != &staging_key
        || staging.owner != &system_program::ID
        || !staging.data_is_empty()
    {
        return Err(DealerEquityAcceleratorErrorV4::Custody);
    }
    let data = raw
        .try_borrow_data()
        .map_err(|_| DealerEquityAcceleratorErrorV4::Custody)?;
    if hash(&data).to_bytes() != config.realm() {
        return Err(DealerEquityAcceleratorErrorV4::Custody);
    }
    RealmV1::decode(&data).map_err(|_| DealerEquityAcceleratorErrorV4::Custody)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_custody_common(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerEquityRequestV3<'_>,
    route: &[AccountInfo<'_>],
    representative: &[AccountInfo<'_>],
    replay: Pubkey,
    authority: [u8; 32],
    mint: [u8; 32],
    token_program: [u8; 32],
) -> Result<(), DealerEquityAcceleratorErrorV4> {
    for index in [
        CUSTODY_CORE_MARKET_RELATIVE_V4,
        CUSTODY_ACTIVATION_RELATIVE_V4,
        CUSTODY_CALLER_PROGRAMDATA_RELATIVE_V4,
        CUSTODY_REALM_RAW_RELATIVE_V4,
        CUSTODY_REALM_STAGING_RELATIVE_V4,
        CUSTODY_REPLAY_RELATIVE_V4,
        CUSTODY_MINT_RELATIVE_V4,
        CUSTODY_AUTHORITY_RELATIVE_V4,
        CUSTODY_TOKEN_PROGRAM_RELATIVE_V4,
    ] {
        if route.get(index).map(|account| account.key)
            != representative.get(index).map(|account| account.key)
        {
            return Err(DealerEquityAcceleratorErrorV4::Custody);
        }
    }
    let registry = frame_account(route, CUSTODY_REGISTRY_RELATIVE_V4)?;
    let caller = frame_account(route, CUSTODY_CALLER_PROGRAM_RELATIVE_V4)?;
    let token = frame_account(route, CUSTODY_TOKEN_PROGRAM_RELATIVE_V4)?;
    if frame_account(route, CUSTODY_CORE_MARKET_RELATIVE_V4)?
        .key
        .to_bytes()
        != request.market
        || registry.key.to_bytes() != invocation.context().registry_program.to_bytes()
        || caller.key.to_bytes() != invocation.context().trading_program.to_bytes()
        || frame_account(route, CUSTODY_REPLAY_RELATIVE_V4)?.key != &replay
        || frame_account(route, CUSTODY_MINT_RELATIVE_V4)?
            .key
            .to_bytes()
            != mint
        || frame_account(route, CUSTODY_AUTHORITY_RELATIVE_V4)?
            .key
            .to_bytes()
            != authority
        || token.key.to_bytes() != token_program
        || !registry.executable
        || !caller.executable
        || !token.executable
    {
        return Err(DealerEquityAcceleratorErrorV4::Custody);
    }
    Ok(())
}

fn observe_token(
    account: &AccountInfo<'_>,
    mint: [u8; 32],
    token_program: [u8; 32],
) -> Result<TokenObservationV4, DealerEquityAcceleratorErrorV4> {
    if account.owner.to_bytes() != token_program || account.data_len() != ACCOUNT_BYTES {
        return Err(DealerEquityAcceleratorErrorV4::Custody);
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| DealerEquityAcceleratorErrorV4::Custody)?;
    let token = TokenAccount::parse(&data).map_err(|_| DealerEquityAcceleratorErrorV4::Custody)?;
    if token.mint != mint
        || token.state != AccountState::Initialized
        || !matches!(token.native_reserve, COption::None)
    {
        return Err(DealerEquityAcceleratorErrorV4::Custody);
    }
    Ok(TokenObservationV4 {
        key: account.key.to_bytes(),
        token,
    })
}

fn is_canonical_internal_vault(observed: TokenObservationV4, authority: [u8; 32]) -> bool {
    observed.token.owner == authority
        && observed.token.delegate.is_none()
        && observed.token.delegated_amount == 0
        && observed.token.close_authority.is_none()
}

fn vault_key(
    custody_program: [u8; 32],
    market: [u8; 32],
    release_set: [u8; 32],
    context: [u8; 32],
    compartment: CompartmentV1,
    hint: u8,
) -> [u8; 32] {
    derive_hinted(
        &CustodyVaultSeedsV1::new(market, release_set, context, compartment).as_slices(),
        &Pubkey::new_from_array(custody_program),
        hint,
    )
    .0
    .to_bytes()
}

/// One account body's own recorded bump. Zero is unrecorded and its reader
/// searches, so a body written before the byte existed is no worse off.
fn recorded_bump(body: &[u8], offset: usize) -> u8 {
    body.get(offset).copied().unwrap_or(0)
}

/// Widest seed set any hinted derivation in this evaluator carries, plus the
/// bump seed: `MarketCoreStateSeedsV2` is nine.
const HINTED_SEED_CAPACITY_V4: usize = 10;

/// Reproduce one address from a mined bump, degrading to the search this route
/// always ran.
///
/// READING A HINT MUST NOT BE ABLE TO REFUSE. An unrecorded bump, or one whose
/// derivation fails outright, falls back to `find_program_address`; only the
/// address equality the caller already had can refuse. A bump is never an
/// authority: canonicality is enforced where each of these accounts is MADE.
fn derive_hinted(seeds: &[&[u8]], program: &Pubkey, hint: u8) -> (Pubkey, u8) {
    if hint != 0 && seeds.len() < HINTED_SEED_CAPACITY_V4 {
        let bump = [hint];
        let mut buffer: [&[u8]; HINTED_SEED_CAPACITY_V4] = [&[]; HINTED_SEED_CAPACITY_V4];
        for (slot, seed) in buffer.iter_mut().zip(seeds) {
            *slot = seed;
        }
        if let Some(slot) = buffer.get_mut(seeds.len()) {
            *slot = &bump;
        }
        if let Some(all) = buffer.get(..=seeds.len()) {
            if let Ok(address) = Pubkey::create_program_address(all, program) {
                return (address, hint);
            }
        }
    }
    Pubkey::find_program_address(seeds, program)
}

fn custody_frame<'accounts, 'info>(
    runtime: &'accounts [AccountInfo<'info>],
    start: usize,
) -> Result<&'accounts [AccountInfo<'info>], DealerEquityAcceleratorErrorV4> {
    let end = start
        .checked_add(usize::from(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3))
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    runtime
        .get(start..end)
        .ok_or(DealerEquityAcceleratorErrorV4::Custody)
}

fn frame_account<'a, 'info>(
    frame: &'a [AccountInfo<'info>],
    relative: usize,
) -> Result<&'a AccountInfo<'info>, DealerEquityAcceleratorErrorV4> {
    frame
        .get(relative)
        .ok_or(DealerEquityAcceleratorErrorV4::Custody)
}

fn relative_account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    start: usize,
    relative: usize,
    error: DealerEquityAcceleratorErrorV4,
) -> Result<&'a AccountInfo<'info>, DealerEquityAcceleratorErrorV4> {
    accounts
        .get(
            start
                .checked_add(relative)
                .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?,
        )
        .ok_or(error)
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
    error: DealerEquityAcceleratorErrorV4,
) -> Result<&'a AccountInfo<'info>, DealerEquityAcceleratorErrorV4> {
    accounts.get(index).ok_or(error)
}

const fn action_for_request(action: EquityRequestActionV3) -> MultiLpActionV3 {
    match action {
        EquityRequestActionV3::Contribute => MultiLpActionV3::Add,
        EquityRequestActionV3::Redeem => MultiLpActionV3::Remove,
    }
}

fn scalar(scalars: &[u64], index: u16) -> Result<u64, DealerEquityAcceleratorErrorV4> {
    scalars
        .get(usize::from(index))
        .copied()
        .ok_or(DealerEquityAcceleratorErrorV4::Invocation)
}

fn identity(
    identities: &[[u8; 32]],
    index: u16,
) -> Result<[u8; 32], DealerEquityAcceleratorErrorV4> {
    identities
        .get(usize::from(index))
        .copied()
        .ok_or(DealerEquityAcceleratorErrorV4::Invocation)
}

fn set_scalar(
    scalars: &mut [u64],
    index: u16,
    value: u64,
) -> Result<(), DealerEquityAcceleratorErrorV4> {
    *scalars
        .get_mut(usize::from(index))
        .ok_or(DealerEquityAcceleratorErrorV4::Transition)? = value;
    Ok(())
}

fn set_identity(
    identities: &mut [[u8; 32]],
    index: u16,
    value: [u8; 32],
) -> Result<(), DealerEquityAcceleratorErrorV4> {
    *identities
        .get_mut(usize::from(index))
        .ok_or(DealerEquityAcceleratorErrorV4::Transition)? = value;
    Ok(())
}

fn encode_bank(
    scalars: &[u64],
    identities: &[[u8; 32]],
    expected_bytes: usize,
) -> Result<Vec<u8>, DealerEquityAcceleratorErrorV4> {
    let scalar_bytes = scalars
        .len()
        .checked_mul(8)
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    let identity_bytes = identities
        .len()
        .checked_mul(32)
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
    if scalar_bytes
        .checked_add(identity_bytes)
        .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?
        != expected_bytes
    {
        return Err(DealerEquityAcceleratorErrorV4::Invocation);
    }
    let mut output = vec![0_u8; expected_bytes];
    for (index, value) in scalars.iter().copied().enumerate() {
        let start = index
            .checked_mul(8)
            .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
        output
            .get_mut(start..start + 8)
            .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?
            .copy_from_slice(&value.to_le_bytes());
    }
    for (index, value) in identities.iter().enumerate() {
        let start = scalar_bytes
            .checked_add(
                index
                    .checked_mul(32)
                    .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?,
            )
            .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?;
        output
            .get_mut(start..start + 32)
            .ok_or(DealerEquityAcceleratorErrorV4::Arithmetic)?
            .copy_from_slice(value);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use dclutch_custody_contract::{
        ContextV1, CustodyRequestV1, DelegatedCustodyRequestV2, OperationV1,
    };
    use dclutch_dealer_codec::scenario::ScenarioSolvencyReport;

    use super::*;
    use super::{HINTED_SEED_CAPACITY_V4, derive_hinted};

    fn transfer(
        source_compartment: CompartmentV1,
        destination_compartment: CompartmentV1,
        marker: u8,
        parent_request_digest: [u8; 32],
    ) -> CustodyRequestV1 {
        let source_external = source_compartment == CompartmentV1::External;
        let destination_external = destination_compartment == CompartmentV1::External;
        CustodyRequestV1 {
            operation: OperationV1::Transfer,
            caller_role: CallerRoleV1::Trading,
            source_compartment,
            destination_compartment,
            release_set: [1; 32],
            market: [2; 32],
            realm: [3; 32],
            context: [4; 32],
            caller_program: [5; 32],
            semantic: ContextV1 {
                candidate: [6; 32],
                source_owner: if source_external { [7; 32] } else { [0; 32] },
                destination_owner: if destination_external {
                    [8; 32]
                } else {
                    [0; 32]
                },
                order: [9; 32],
                parent_request_digest,
                order_nonce: 10,
                generation: 11,
                page_index: 12,
                execution_index: 13,
                transfer_index: u16::from(marker),
            },
            source: [marker; 32],
            destination: [marker.saturating_add(1); 32],
            source_vault_context: if source_external { [0; 32] } else { [14; 32] },
            destination_vault_context: if destination_external {
                [0; 32]
            } else {
                [15; 32]
            },
            mint: [16; 32],
            token_program: [17; 32],
            payer: [0; 32],
            rent_refund: [0; 32],
            expected_revision: 18,
            resulting_revision: 19,
            amount: 21,
            rent_lamports: 0,
        }
    }

    fn write(bytes: &mut [u8], offset: usize, value: &[u8]) {
        bytes
            .get_mut(offset..offset + value.len())
            .expect("fixture range")
            .copy_from_slice(value);
    }

    fn request_bytes(action: EquityRequestActionV3, positions: u32) -> Vec<u8> {
        use super::super::v3_equity_claims::{
            EquityClaimsContextV3, EquityClaimsTransitionV3, encode_equity_claims_packet_v3,
            equity_claims_geometry_v3,
        };
        use super::super::v3_equity_operator::{
            DEALER_EQUITY_HEADER_BYTES_V3, DEALER_EQUITY_REQUEST_MAGIC_V3,
            DEALER_EQUITY_REQUEST_VERSION_V3, dealer_equity_selector_v3,
        };

        let (dealer_after, lp_after) = match positions {
            0 => ([1_u64, 1], [3_u64, 3]),
            1 => ([2_u64, 2], [3_u64, 3]),
            2 => ([2_u64, 2], [2_u64, 2]),
            _ => panic!("fixture only covers P0/P1/P2"),
        };
        let transition = EquityClaimsTransitionV3 {
            dealer_before: &[1, 1],
            dealer_after: &dealer_after,
            lp_before: &[3, 3],
            lp_after: &lp_after,
            // P1 has one net Dealer credit, so one complete set is split into
            // the aggregate; P2 transfers one atom from LP to Dealer and is
            // aggregate-neutral.
            minimum_complete_sets_to_split: u64::from(positions == 1),
            maximum_complete_sets_to_merge: 0,
        };
        let provisional = EquityClaimsContextV3 {
            release_set: [1; 32],
            market: [2; 32],
            request_id: [3; 32],
            product_record_digest: [4; 32],
            semantic_basis_id: [5; 32],
            linked_basis_record_digest: [6; 32],
            expected_market_revision: 7,
            dealer_owner: [8; 32],
            dealer_revision: 9,
            lp_owner: [10; 32],
            lp_revision: 11,
        };
        let geometry = equity_claims_geometry_v3(provisional, transition).expect("geometry");
        let selector = dealer_equity_selector_v3(action, positions).expect("selector");
        let mut header = [0_u8; DEALER_EQUITY_HEADER_BYTES_V3];
        write(&mut header, 0, &DEALER_EQUITY_REQUEST_MAGIC_V3);
        write(
            &mut header,
            8,
            &DEALER_EQUITY_REQUEST_VERSION_V3.to_le_bytes(),
        );
        write(&mut header, 10, &selector.to_le_bytes());
        write(&mut header, 12, &2_u32.to_le_bytes());
        for (offset, identity) in [
            (16, [1; 32]),
            (48, [2; 32]),
            (80, [3; 32]),
            (112, [4; 32]),
            (144, [10; 32]),
            (176, [5; 32]),
            (208, [6; 32]),
            (240, [7; 32]),
            (272, [8; 32]),
            (304, [9; 32]),
            (336, [10; 32]),
            (368, [11; 32]),
        ] {
            write(&mut header, offset, &identity);
        }
        for (offset, value) in [
            (400, 1_u64),
            (408, 1),
            (416, 9),
            (424, 11),
            (432, 1),
            (440, 100),
            (448, 0),
            (
                456,
                u64::from(action == EquityRequestActionV3::Contribute) * 21,
            ),
            (464, 7),
        ] {
            write(&mut header, offset, &value.to_le_bytes());
        }
        let packet_bytes = u32::try_from(geometry.packet_bytes).expect("packet width");
        write(&mut header, 472, &packet_bytes.to_le_bytes());
        let context = EquityClaimsContextV3 {
            request_id: hash(&header).to_bytes(),
            ..provisional
        };
        let mut packet = vec![0_u8; geometry.packet_bytes];
        encode_equity_claims_packet_v3(context, transition, &mut packet)
            .unwrap_or_else(|error| panic!("P{positions} packet: {error:?}"));
        let mut request = header.to_vec();
        request.extend_from_slice(&packet);
        request
    }

    fn effects(
        action: MultiLpActionV3,
        parent_request_digest: [u8; 32],
    ) -> [Option<MultiLpCustodyEffectV3>; MAX_MULTI_LP_CUSTODY_EFFECTS_V3] {
        let mut output = [None; MAX_MULTI_LP_CUSTODY_EFFECTS_V3];
        let templates = match action {
            MultiLpActionV3::Add => [
                MultiLpCustodyRequestV3::Delegated(DelegatedCustodyRequestV2 {
                    custody: transfer(
                        CompartmentV1::External,
                        CompartmentV1::TradingPrincipal,
                        22,
                        parent_request_digest,
                    ),
                    starts_atomic_debit: true,
                    terminal: true,
                    delegate_before: [31; 32],
                    delegate_after: [0; 32],
                    total_debit: 21,
                    allowance_before: 21,
                    allowance_after: 0,
                }),
                MultiLpCustodyRequestV3::Canonical(transfer(
                    CompartmentV1::HoardPrincipal,
                    CompartmentV1::TradingPrincipal,
                    24,
                    parent_request_digest,
                )),
                MultiLpCustodyRequestV3::Canonical(transfer(
                    CompartmentV1::TradingPrincipal,
                    CompartmentV1::HoardPrincipal,
                    26,
                    parent_request_digest,
                )),
            ],
            MultiLpActionV3::Remove => [
                MultiLpCustodyRequestV3::Canonical(transfer(
                    CompartmentV1::TradingPrincipal,
                    CompartmentV1::HoardPrincipal,
                    22,
                    parent_request_digest,
                )),
                MultiLpCustodyRequestV3::Canonical(transfer(
                    CompartmentV1::TradingPrincipal,
                    CompartmentV1::External,
                    24,
                    parent_request_digest,
                )),
                MultiLpCustodyRequestV3::Canonical(transfer(
                    CompartmentV1::HoardPrincipal,
                    CompartmentV1::TradingPrincipal,
                    26,
                    parent_request_digest,
                )),
            ],
        };
        let count = match action {
            MultiLpActionV3::Add => 2,
            MultiLpActionV3::Remove => 3,
        };
        for (index, request) in templates.into_iter().take(count).enumerate() {
            output[index] = Some(MultiLpCustodyEffectV3 {
                request,
                source_after: 100 - u64::try_from(index).expect("index"),
                destination_after: 200 + u64::try_from(index).expect("index"),
            });
        }
        output
    }

    fn plan(
        action: MultiLpActionV3,
        effects: &[Option<MultiLpCustodyEffectV3>; MAX_MULTI_LP_CUSTODY_EFFECTS_V3],
    ) -> MultiLpPlanV3 {
        let custody_count = match action {
            MultiLpActionV3::Add => 2,
            MultiLpActionV3::Remove => 3,
        };
        let report = ScenarioSolvencyReport {
            minimum_equity: 1,
            minimum_scenario: 0,
            present_capital: 1,
            locked_capital_floor: 0,
        };
        MultiLpPlanV3 {
            action,
            lp_owner: [10; 32],
            share_delta: 7,
            collateral_in: u64::from(action == MultiLpActionV3::Add) * 21,
            collateral_out: u64::from(action == MultiLpActionV3::Remove) * 21,
            minimum_complete_sets_to_split: u64::from(action == MultiLpActionV3::Remove) * 21,
            maximum_complete_sets_to_merge: 21,
            solvency_before: report,
            solvency_after: report,
            custody_digest: multi_lp_custody_digest_v3(effects, custody_count).expect("digest"),
            custody_count,
            external_after: 1,
            principal_after: 1,
            hoard_after: 1,
            obligation_digest_after: [12; 32],
            lp_digest_after: [13; 32],
            obligation_revision_after: 2,
            lp_revision_after: 2,
            total_equity_shares_after: 11,
            lp_equity_shares_after: 7,
        }
    }

    #[test]
    fn every_selector_shape_has_exact_appended_evidence_geometry() {
        for (action, expected_routes) in [
            (EquityRequestActionV3::Contribute, 2),
            (EquityRequestActionV3::Redeem, 3),
        ] {
            for positions in 0..=2 {
                let frame = equity_frame(action, positions).expect("frame");
                assert_eq!(frame.custody_route_count, expected_routes);
                assert_eq!(frame.evidence_start + 2, frame.logical_account_count);
                assert_eq!(frame.lp_position + 1, frame.custody_program);
                assert_eq!(frame.custody_program + 1, frame.evidence_start);
            }
        }
    }

    #[test]
    fn substituted_position_count_refuses_exactly() {
        assert_eq!(
            equity_frame(EquityRequestActionV3::Contribute, 3),
            Err(DealerEquityAcceleratorErrorV4::Invocation)
        );
        assert_eq!(
            equity_frame(EquityRequestActionV3::Redeem, u32::MAX),
            Err(DealerEquityAcceleratorErrorV4::Invocation)
        );
    }

    #[test]
    fn sbf_projector_matches_host_for_every_equity_shape_and_preserves_profile_facts() {
        use super::super::v3_hot_artifact::project_dealer_equity_hot_registers_v3;

        for (request_action, action) in [
            (EquityRequestActionV3::Contribute, MultiLpActionV3::Add),
            (EquityRequestActionV3::Redeem, MultiLpActionV3::Remove),
        ] {
            for positions in 0..=2 {
                let bytes = request_bytes(request_action, positions);
                let request = DealerEquityRequestV3::decode(&bytes).expect("request");
                let effects = effects(action, hash(request.bytes()).to_bytes());
                let plan = plan(action, &effects);
                let scalar_count = dealer_equity_scalar_count_v3(action).expect("scalars");
                let identity_count = dealer_equity_identity_count_v3(action).expect("identities");
                let mut expected_scalars = (0..scalar_count)
                    .map(|index| 0xA000_u64 + u64::try_from(index).expect("index"))
                    .collect::<Vec<_>>();
                let mut expected_identities = (0..identity_count)
                    .map(|index| [u8::try_from(index).expect("index").wrapping_add(1); 32])
                    .collect::<Vec<_>>();
                let evidence_owner = dealer_equity_evidence_owner_identity_register_v3(action)
                    .expect("evidence owner");
                expected_identities[usize::from(evidence_owner)] = [0xE1; 32];
                let input_scalars = expected_scalars.clone();
                let input_identities = expected_identities.clone();
                project_dealer_equity_hot_registers_v3(
                    request,
                    plan,
                    &effects,
                    99,
                    &mut expected_scalars,
                    &mut expected_identities,
                )
                .expect("host projection");

                let mut candidate = vec![0x5A; scalar_count * 8 + identity_count * 32];
                project_candidate_bank(
                    &request,
                    &plan,
                    &effects,
                    99,
                    &input_scalars,
                    &input_identities,
                    &mut candidate,
                )
                .expect("sbf projection");
                assert_eq!(
                    candidate,
                    encode_bank(&expected_scalars, &expected_identities, candidate.len())
                        .expect("bank")
                );
                let evidence_offset = scalar_count * 8 + usize::from(evidence_owner) * 32;
                assert_eq!(
                    &candidate[evidence_offset..evidence_offset + 32],
                    &[0xE1; 32]
                );

                let mut malformed = plan;
                malformed.custody_digest[0] ^= 1;
                let before = candidate.clone();
                assert_eq!(
                    project_candidate_bank(
                        &request,
                        &malformed,
                        &effects,
                        99,
                        &input_scalars,
                        &input_identities,
                        &mut candidate,
                    ),
                    Err(DealerEquityAcceleratorErrorV4::Transition)
                );
                assert_eq!(
                    candidate, before,
                    "{request_action:?}/P{positions} rollback"
                );
            }
        }
    }

    /// The three arms a hint reader owes: reproduce, name something else, and
    /// degrade. Only the third is the reader's own business; the second is the
    /// caller's address equality, which is why this asserts a DIFFERENT address
    /// rather than an error.
    #[test]
    fn a_hint_reproduces_misnames_or_degrades_but_never_refuses() {
        let program = Pubkey::new_from_array([0x41; 32]);
        let domain: &[u8] = b"dclutch/test/hinted/v4";
        let context = [0x42; 32];
        let seeds: [&[u8]; 2] = [domain, &context];
        let (searched, canonical) = Pubkey::find_program_address(&seeds, &program);

        assert_eq!(
            derive_hinted(&seeds, &program, canonical),
            (searched, canonical),
            "the canonical bump reproduces the address the search found"
        );

        let mut below = canonical;
        let mismatched = loop {
            below = below
                .checked_sub(1)
                .expect("a derivable bump below canonical");
            let bump = [below];
            if let Ok(address) =
                Pubkey::create_program_address(&[domain, &context, &bump], &program)
            {
                break address;
            }
        };
        assert_ne!(
            mismatched, searched,
            "a wrong bump names a different address, and the equality refuses"
        );
        assert_eq!(derive_hinted(&seeds, &program, below), (mismatched, below));

        assert_eq!(
            derive_hinted(&seeds, &program, 0),
            (searched, canonical),
            "an unrecorded bump searches exactly as this route always did"
        );
        if canonical < u8::MAX {
            // Every bump above canonical is on the curve by construction: the
            // search descends from 255 and stopped at the first that was not.
            assert_eq!(
                derive_hinted(&seeds, &program, u8::MAX),
                (searched, canonical),
                "a bump whose derivation fails degrades to the search"
            );
        }
    }

    #[test]
    fn a_seed_set_this_buffer_cannot_hold_degrades_to_the_search() {
        let program = Pubkey::new_from_array([0x43; 32]);
        let wide = [b"x".as_slice(); HINTED_SEED_CAPACITY_V4];
        assert_eq!(
            derive_hinted(&wide, &program, 255),
            Pubkey::find_program_address(&wide, &program),
            "the widest seed set leaves no room for a bump seed and searches"
        );
    }
}
