//! EffectProgram V3 route admission for canonical Dealer Custody effects.
//!
//! Dealer does not own a child wire or CPI dispatcher.  This module compares
//! the request bank produced by the selected family-neutral EffectProgram to
//! the exact canonical `CustodyRequestV1` sequence admitted by Dealer
//! semantics.  The common Trading hot outer remains the sole executor and
//! receipt sequencer for the resulting route coordinates.

use dclutch_capability_program_contract::hot_v3::HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3;
use dclutch_custody_contract::{CUSTODY_REQUEST_BYTES_V1, DELEGATED_CUSTODY_REQUEST_BYTES_V2};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3, RouteKindV3},
};

use super::{
    v3_composer::{MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3, ScenarioAtomicPlanV3},
    v3_equity_operator::{DEALER_EQUITY_HEADER_BYTES_V3, DealerEquityRequestV3},
    v3_multi_lp::{MultiLpActionV3, MultiLpCustodyRequestV3, MultiLpPlanV3},
};

const SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3: u16 = 20;
const CUSTODY_TRANSFER_ACCOUNT_COUNT_V1: u16 = 14;
const DEALER_LOCAL_STATE_ACCOUNT_COUNT_V3: u16 = 2;

/// Exact maximum Custody invocations in one Dealer V3 action.
pub const MAX_DEALER_CUSTODY_ROUTES_V3: usize = MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3;

/// Stable refusal from EffectProgram-to-Dealer composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerRouteErrorV3 {
    /// A sequence exceeded fixed Dealer effect capacity or contained a gap.
    InvalidSequence,
    /// EffectProgram registers, route geometry, or request-bank width refused.
    InvalidProgram,
    /// A Custody route request differed byte-for-byte from the semantic plan.
    RequestMismatch,
    /// EffectProgram emitted too few or too many Custody invocations.
    RouteMismatch,
    /// Borrowed Claims packet, exact Position frame, or global route order differed.
    ClaimsMismatch,
}

/// Result alias for Dealer V3 route composition.
pub type DealerRouteResultV3<T> = core::result::Result<T, DealerRouteErrorV3>;

/// One expected canonical Custody request sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerCustodySequenceV3 {
    requests: [Option<MultiLpCustodyRequestV3>; MAX_DEALER_CUSTODY_ROUTES_V3],
    count: u8,
}

impl DealerCustodySequenceV3 {
    /// Project the exact ordered sequence from a scenario-fill plan.
    pub fn from_scenario(
        plan: ScenarioAtomicPlanV3,
        effects: &[Option<super::v3_composer::ScenarioCustodyEffectV3>],
    ) -> DealerRouteResultV3<Self> {
        let mut requests = [None; MAX_DEALER_CUSTODY_ROUTES_V3];
        let count = usize::from(plan.custody_count);
        if count > requests.len() || effects.len() != requests.len() {
            return Err(DealerRouteErrorV3::InvalidSequence);
        }
        for (index, destination) in requests.iter_mut().take(count).enumerate() {
            *destination = Some(MultiLpCustodyRequestV3::Canonical(
                effects
                    .get(index)
                    .copied()
                    .flatten()
                    .ok_or(DealerRouteErrorV3::InvalidSequence)?
                    .request,
            ));
        }
        if effects.iter().skip(count).any(Option::is_some) {
            return Err(DealerRouteErrorV3::InvalidSequence);
        }
        Ok(Self {
            requests,
            count: plan.custody_count,
        })
    }

    /// Project the exact ordered Custody subsequence from one equity action.
    pub fn from_multi_lp(plan: MultiLpPlanV3) -> DealerRouteResultV3<Self> {
        let mut requests = [None; MAX_DEALER_CUSTODY_ROUTES_V3];
        let count = usize::from(plan.custody_count);
        if count > requests.len() || count > plan.custody.len() {
            return Err(DealerRouteErrorV3::InvalidSequence);
        }
        for (index, destination) in requests.iter_mut().take(count).enumerate() {
            *destination = Some(
                plan.custody
                    .get(index)
                    .copied()
                    .flatten()
                    .ok_or(DealerRouteErrorV3::InvalidSequence)?
                    .request,
            );
        }
        if plan.custody.iter().skip(count).any(Option::is_some) {
            return Err(DealerRouteErrorV3::InvalidSequence);
        }
        Ok(Self {
            requests,
            count: plan.custody_count,
        })
    }

    /// Active request count.
    pub const fn count(self) -> u8 {
        self.count
    }

    /// Decode one active request.
    pub fn request(self, index: u8) -> DealerRouteResultV3<MultiLpCustodyRequestV3> {
        if index >= self.count {
            return Err(DealerRouteErrorV3::InvalidSequence);
        }
        self.requests
            .get(usize::from(index))
            .copied()
            .flatten()
            .ok_or(DealerRouteErrorV3::InvalidSequence)
    }
}

/// One admitted global EffectProgram route/invocation coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerCustodyRouteV3 {
    /// Global route coordinate.
    pub route: u16,
    /// Invocation coordinate within `Once`, `AffineOnce`, or `Each`.
    pub invocation: u32,
    /// Authenticated route geometry.
    pub kind: RouteKindV3,
}

/// Exact route coordinates handed to the common Trading Custody executor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerCustodyCompositionV3 {
    routes: [Option<DealerCustodyRouteV3>; MAX_DEALER_CUSTODY_ROUTES_V3],
    count: u8,
}

/// Fully joined junior-equity child-route composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerEquityCompositionV3 {
    claims_route: Option<u16>,
    custody: DealerCustodyCompositionV3,
}

impl DealerEquityCompositionV3 {
    /// Exact global Claims route, absent only for a true Claims no-op.
    pub const fn claims_route(self) -> Option<u16> {
        self.claims_route
    }

    /// Exact ordered Custody subsequence admitted by the equity plan.
    pub const fn custody(self) -> DealerCustodyCompositionV3 {
        self.custody
    }
}

impl DealerCustodyCompositionV3 {
    /// Active route count.
    pub const fn count(self) -> u8 {
        self.count
    }

    /// Decode one admitted route coordinate.
    pub fn route(self, index: u8) -> DealerRouteResultV3<DealerCustodyRouteV3> {
        if index >= self.count {
            return Err(DealerRouteErrorV3::RouteMismatch);
        }
        self.routes
            .get(usize::from(index))
            .copied()
            .flatten()
            .ok_or(DealerRouteErrorV3::RouteMismatch)
    }
}

/// Authenticate every Custody invocation against the exact Dealer sequence.
///
/// Non-Custody routes remain owned by the common outer and other fixed-role
/// composition validators.  Disabled routes emit no invocation and therefore
/// cannot stand in for an expected Dealer effect.
pub fn authenticate_dealer_custody_routes_v3(
    program: ProgramV3<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    request_bank: &[u8],
    expected: DealerCustodySequenceV3,
) -> DealerRouteResultV3<DealerCustodyCompositionV3> {
    if program
        .request_bytes(tail_count)
        .map_err(|_| DealerRouteErrorV3::InvalidProgram)?
        != request_bank.len()
    {
        return Err(DealerRouteErrorV3::InvalidProgram);
    }
    let mut routes = [None; MAX_DEALER_CUSTODY_ROUTES_V3];
    let mut observed = 0_u8;
    let mut route = 0_u16;
    while route < program.route_count() {
        let invocations = program
            .invocation_count(route, tail_count, scalars, identities)
            .map_err(|_| DealerRouteErrorV3::InvalidProgram)?;
        let mut invocation = 0_u32;
        while invocation < invocations {
            let resolved = program
                .resolved_invocation(route, invocation, tail_count, scalars, identities)
                .map_err(|_| DealerRouteErrorV3::InvalidProgram)?;
            if resolved.role == FixedRole::Custody {
                let expected_request = expected
                    .request(observed)
                    .map_err(|_| DealerRouteErrorV3::RouteMismatch)?;
                let end = resolved
                    .request_offset
                    .checked_add(resolved.request_len)
                    .ok_or(DealerRouteErrorV3::InvalidProgram)?;
                let actual = request_bank
                    .get(resolved.request_offset..end)
                    .ok_or(DealerRouteErrorV3::InvalidProgram)?;
                let mut expected_bytes =
                    [0_u8; dclutch_custody_contract::DELEGATED_CUSTODY_REQUEST_BYTES_V2];
                let expected_slice = expected_bytes
                    .get_mut(..expected_request.encoded_len())
                    .ok_or(DealerRouteErrorV3::InvalidSequence)?;
                expected_request
                    .encode_into(expected_slice)
                    .map_err(|_| DealerRouteErrorV3::InvalidSequence)?;
                if actual != expected_slice {
                    return Err(DealerRouteErrorV3::RequestMismatch);
                }
                *routes
                    .get_mut(usize::from(observed))
                    .ok_or(DealerRouteErrorV3::RouteMismatch)? = Some(DealerCustodyRouteV3 {
                    route,
                    invocation,
                    kind: resolved.kind,
                });
                observed = observed
                    .checked_add(1)
                    .ok_or(DealerRouteErrorV3::RouteMismatch)?;
            }
            invocation = invocation
                .checked_add(1)
                .ok_or(DealerRouteErrorV3::InvalidProgram)?;
        }
        route = route
            .checked_add(1)
            .ok_or(DealerRouteErrorV3::InvalidProgram)?;
    }
    if observed != expected.count() {
        return Err(DealerRouteErrorV3::RouteMismatch);
    }
    Ok(DealerCustodyCompositionV3 {
        routes,
        count: observed,
    })
}

/// Authenticate the complete global child-route order for junior equity.
///
/// Contribution cash arrives before Claims and any complete-set merge follows
/// it. Redemption complete-set splitting precedes Claims, while cash payout
/// and any remaining-set merge follow it. This ordering makes the already
/// authenticated Claims/Custody receipts a physical dependency chain rather
/// than independent effects that merely happen to share one transaction.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_dealer_equity_routes_v3(
    program: ProgramV3<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    request_bank: &[u8],
    family_request: &[u8],
    request: DealerEquityRequestV3<'_>,
    plan: MultiLpPlanV3,
) -> DealerRouteResultV3<DealerEquityCompositionV3> {
    if family_request != request.bytes() {
        return Err(DealerRouteErrorV3::ClaimsMismatch);
    }
    let expected_custody = DealerCustodySequenceV3::from_multi_lp(plan)?;
    let custody = authenticate_dealer_custody_routes_v3(
        program,
        tail_count,
        scalars,
        identities,
        request_bank,
        expected_custody,
    )?;
    let signed = request
        .claims_plan()
        .map_err(|_| DealerRouteErrorV3::ClaimsMismatch)?;
    validate_equity_route_grammar(program, tail_count, scalars, identities, signed, plan)?;
    let custody_before_claims = match plan.action {
        MultiLpActionV3::Add => usize::from(plan.collateral_in != 0),
        MultiLpActionV3::Remove => usize::from(plan.minimum_complete_sets_to_split != 0),
    };
    let mut claims_route = None;
    let mut custody_index = 0_u8;
    let mut child_index = 0_usize;
    let mut route = 0_u16;
    while route < program.route_count() {
        let invocation_count = program
            .invocation_count(route, tail_count, scalars, identities)
            .map_err(|_| DealerRouteErrorV3::InvalidProgram)?;
        let mut invocation_index = 0_u32;
        while invocation_index < invocation_count {
            let invocation = program
                .resolved_invocation(route, invocation_index, tail_count, scalars, identities)
                .map_err(|_| DealerRouteErrorV3::InvalidProgram)?;
            match invocation.role {
                FixedRole::Claims => {
                    let signed = signed.ok_or(DealerRouteErrorV3::ClaimsMismatch)?;
                    let witness = invocation
                        .borrowed_witness
                        .ok_or(DealerRouteErrorV3::ClaimsMismatch)?;
                    let expected_accounts = SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3
                        .checked_add(
                            u16::try_from(signed.position_count())
                                .map_err(|_| DealerRouteErrorV3::ClaimsMismatch)?,
                        )
                        .ok_or(DealerRouteErrorV3::ClaimsMismatch)?;
                    let request_end = invocation
                        .request_offset
                        .checked_add(invocation.request_len)
                        .ok_or(DealerRouteErrorV3::ClaimsMismatch)?;
                    if claims_route.is_some()
                        || invocation_index != 0
                        || invocation.kind != RouteKindV3::Once
                        || invocation.fixed_account_count != expected_accounts
                        || invocation.item_account_count != 0
                        || invocation.repeated_item_count != 0
                        || invocation.request_len != 0
                        || request_bank
                            .get(invocation.request_offset..request_end)
                            .is_none_or(|fixed| !fixed.is_empty())
                        || witness.source_offset() != DEALER_EQUITY_HEADER_BYTES_V3
                        || witness.len() != request.claims_packet().len()
                        || witness
                            .slice(family_request)
                            .map_err(|_| DealerRouteErrorV3::ClaimsMismatch)?
                            != request.claims_packet()
                        || child_index != custody_before_claims
                    {
                        return Err(DealerRouteErrorV3::ClaimsMismatch);
                    }
                    claims_route = Some(route);
                }
                FixedRole::Custody => {
                    let admitted = custody.route(custody_index)?;
                    if admitted.route != route || admitted.invocation != invocation_index {
                        return Err(DealerRouteErrorV3::RouteMismatch);
                    }
                    custody_index = custody_index
                        .checked_add(1)
                        .ok_or(DealerRouteErrorV3::RouteMismatch)?;
                }
                FixedRole::Core | FixedRole::Resolution => {
                    return Err(DealerRouteErrorV3::RouteMismatch);
                }
            }
            child_index = child_index
                .checked_add(1)
                .ok_or(DealerRouteErrorV3::RouteMismatch)?;
            invocation_index = invocation_index
                .checked_add(1)
                .ok_or(DealerRouteErrorV3::InvalidProgram)?;
        }
        route = route
            .checked_add(1)
            .ok_or(DealerRouteErrorV3::InvalidProgram)?;
    }
    let expected_child_count = usize::from(custody.count())
        .checked_add(usize::from(signed.is_some()))
        .ok_or(DealerRouteErrorV3::RouteMismatch)?;
    if custody_index != custody.count()
        || claims_route.is_some() != signed.is_some()
        || child_index != expected_child_count
    {
        return Err(DealerRouteErrorV3::RouteMismatch);
    }
    Ok(DealerEquityCompositionV3 {
        claims_route,
        custody,
    })
}

fn validate_equity_route_grammar(
    program: ProgramV3<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    signed: Option<dclutch_claims_svm::signed_delta_v3::SignedDeltaPlanV3<'_>>,
    plan: MultiLpPlanV3,
) -> DealerRouteResultV3<()> {
    let (route_count, active) = match plan.action {
        MultiLpActionV3::Add => (
            3_u16,
            [
                plan.collateral_in != 0,
                signed.is_some(),
                plan.maximum_complete_sets_to_merge != 0,
                false,
            ],
        ),
        MultiLpActionV3::Remove => (
            4_u16,
            [
                plan.minimum_complete_sets_to_split != 0,
                signed.is_some(),
                plan.collateral_out != 0,
                plan.maximum_complete_sets_to_merge != 0,
            ],
        ),
    };
    let positions = signed.map_or(0, |value| value.position_count());
    let claims_accounts = SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3
        .checked_add(u16::try_from(positions).map_err(|_| DealerRouteErrorV3::ClaimsMismatch)?)
        .ok_or(DealerRouteErrorV3::ClaimsMismatch)?;
    if program.route_count() != route_count || program.item_account_stride() != 0 {
        return Err(DealerRouteErrorV3::InvalidProgram);
    }
    let mut expected_start = u16::try_from(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3)
        .map_err(|_| DealerRouteErrorV3::InvalidProgram)?;
    let mut route_index = 0_u16;
    while route_index < route_count {
        let route = program
            .route(route_index)
            .map_err(|_| DealerRouteErrorV3::InvalidProgram)?;
        let claims = route_index == 1;
        let expected_role = if claims {
            FixedRole::Claims
        } else {
            FixedRole::Custody
        };
        let expected_accounts = if claims {
            claims_accounts
        } else {
            CUSTODY_TRANSFER_ACCOUNT_COUNT_V1
        };
        let expected_request = if claims {
            0
        } else if plan.action == MultiLpActionV3::Add && route_index == 0 {
            u32::try_from(DELEGATED_CUSTODY_REQUEST_BYTES_V2)
                .map_err(|_| DealerRouteErrorV3::InvalidProgram)?
        } else {
            u32::try_from(CUSTODY_REQUEST_BYTES_V1)
                .map_err(|_| DealerRouteErrorV3::InvalidProgram)?
        };
        let invocation_count = program
            .invocation_count(route_index, tail_count, scalars, identities)
            .map_err(|_| DealerRouteErrorV3::InvalidProgram)?;
        let expected_active = active
            .get(usize::from(route_index))
            .copied()
            .ok_or(DealerRouteErrorV3::InvalidProgram)?;
        if route.role() != expected_role
            || route.kind() != RouteKindV3::Once
            || route.fixed_account_start() != expected_start
            || route.fixed_account_count() != expected_accounts
            || route.item_account_count() != 0
            || route.fixed_request_bytes() != expected_request
            || route.item_request_bytes() != 0
            || route.borrows_witness() != claims
            || invocation_count != u32::from(expected_active)
        {
            return Err(if claims {
                DealerRouteErrorV3::ClaimsMismatch
            } else {
                DealerRouteErrorV3::RouteMismatch
            });
        }
        expected_start = expected_start
            .checked_add(expected_accounts)
            .ok_or(DealerRouteErrorV3::InvalidProgram)?;
        route_index = route_index
            .checked_add(1)
            .ok_or(DealerRouteErrorV3::InvalidProgram)?;
    }
    let expected_fixed_accounts = expected_start
        .checked_add(DEALER_LOCAL_STATE_ACCOUNT_COUNT_V3)
        .ok_or(DealerRouteErrorV3::InvalidProgram)?;
    if program.fixed_account_count() != expected_fixed_accounts {
        return Err(DealerRouteErrorV3::InvalidProgram);
    }
    Ok(())
}
