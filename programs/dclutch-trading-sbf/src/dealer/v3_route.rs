//! EffectProgram V3 route admission for canonical Dealer Custody effects.
//!
//! Dealer does not own a child wire or CPI dispatcher.  This module compares
//! the request bank produced by the selected family-neutral EffectProgram to
//! the exact canonical `CustodyRequestV1` sequence admitted by Dealer
//! semantics.  The common Trading hot outer remains the sole executor and
//! receipt sequencer for the resulting route coordinates.

use dclutch_custody_contract::CustodyRequestV1;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3, RouteKindV3},
};

use super::{
    v3_composer::{MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3, ScenarioAtomicPlanV3},
    v3_multi_lp::MultiLpPlanV3,
};

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
}

/// Result alias for Dealer V3 route composition.
pub type DealerRouteResultV3<T> = core::result::Result<T, DealerRouteErrorV3>;

/// One expected canonical Custody request sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerCustodySequenceV3 {
    requests: [Option<CustodyRequestV1>; MAX_DEALER_CUSTODY_ROUTES_V3],
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
            *destination = Some(
                effects
                    .get(index)
                    .copied()
                    .flatten()
                    .ok_or(DealerRouteErrorV3::InvalidSequence)?
                    .request,
            );
        }
        if effects.iter().skip(count).any(Option::is_some) {
            return Err(DealerRouteErrorV3::InvalidSequence);
        }
        Ok(Self {
            requests,
            count: plan.custody_count,
        })
    }

    /// Project the sole exact Custody request from one multi-LP action.
    pub const fn from_multi_lp(plan: MultiLpPlanV3) -> Self {
        Self {
            requests: [Some(plan.custody.request), None, None, None],
            count: 1,
        }
    }

    /// Active request count.
    pub const fn count(self) -> u8 {
        self.count
    }

    /// Decode one active request.
    pub fn request(self, index: u8) -> DealerRouteResultV3<CustodyRequestV1> {
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
                let expected_bytes = expected_request
                    .to_bytes()
                    .map_err(|_| DealerRouteErrorV3::InvalidSequence)?;
                if actual != expected_bytes {
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
