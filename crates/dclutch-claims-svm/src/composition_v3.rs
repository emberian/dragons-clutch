//! Generic EffectProgram V3 composition for canonical Claims lifecycle routes.
//!
//! The selected EffectProgram owns route enablement, request templates, and
//! account geometry. This module adds only the cross-route economic join that
//! no individual child request can prove: exactly one canonical Claims
//! mutation, optionally surrounded by the Position admit/close pair admitted
//! for affine mutation. It introduces no balance mutation, family tag, seed
//! rule, or parallel request DTO.

use dclutch_effect_kernel::v2::FixedRole;
use dclutch_effect_kernel::v3::{ProgramV3, RouteKindV3};
use dclutch_rational_representation_v2_lifecycle_contract::{
    LIFECYCLE_COMMON_ACCOUNT_COUNT_V2, LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2,
    LIFECYCLE_REQUEST_MAGIC_V2, LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2, LifecycleActionV2,
    LifecycleRequestV2,
};
use dclutch_rational_representation_v2_request_contract::{
    CallerRoleV2 as RepresentationCallerRoleV2,
    REQUEST_MAGIC_V2 as REPRESENTATION_REQUEST_MAGIC_V2, RepresentationRequestV2,
};

use crate::{
    CallerRole,
    affine_batch_v2::{AFFINE_BATCH_PLAN_MAGIC_V2, AffineBatchPlanV2},
    founding_v5::{CLAIMS_FOUNDING_REQUEST_MAGIC_V5, ClaimsFoundingRequestV5},
    protocol_position_v2::{
        PROTOCOL_POSITION_REQUEST_MAGIC_V2, ProtocolPositionActionV2, ProtocolPositionPresenceV2,
        ProtocolPositionRequestV2,
    },
    signed_delta_v3::{SIGNED_DELTA_PLAN_MAGIC_V3, SignedDeltaPlanV3},
    sparse_native_transfer_v1::{SPARSE_NATIVE_TRANSFER_MAGIC_V1, SparseNativeTransferV1},
};

/// Exact fixed SignedDeltaV3 account frame before its canonical Position tail.
pub const SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3: u16 = 20;
/// Exact fixed sparse native-transfer account frame.
pub const SPARSE_NATIVE_TRANSFER_FIXED_ACCOUNT_COUNT_V1: u16 = 22;
/// Exact fixed Claims FoundingV5 account frame.
pub const CLAIMS_FOUNDING_FIXED_ACCOUNT_COUNT_V5: u16 = 32;

/// Stable cross-route composition refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimsCompositionErrorV3 {
    /// EffectProgram dimensions, register banks, or request bank differed.
    EffectProgram,
    /// An active Claims route used unsupported geometry or packet bytes.
    Route,
    /// The active Claims lifecycle order was invalid for the selected mutation.
    Order,
    /// Release, Market, generation, or parent-request identity differed.
    ParentBinding,
    /// Admission did not create one affine Position at revision zero.
    AdmissionJoin,
    /// Close did not consume one affine Position at its exact post revision.
    CloseJoin,
    /// No sole canonical Claims mutation was selected.
    MissingAffine,
}

/// Immutable parent facts shared by every child request in one transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsCompositionParentV3 {
    /// Current immutable execution release-set identity.
    pub release_set: [u8; 32],
    /// Logical Core Market identity.
    pub market: [u8; 32],
    /// Immutable Core Market generation.
    pub generation: u64,
    /// SHA-256 identity of the exact authenticated Trading parent request.
    pub parent_request_digest: [u8; 32],
}

impl ClaimsCompositionParentV3 {
    fn validate(self) -> Result<(), ClaimsCompositionErrorV3> {
        if self.release_set == [0; 32]
            || self.market == [0; 32]
            || self.parent_request_digest == [0; 32]
        {
            Err(ClaimsCompositionErrorV3::ParentBinding)
        } else {
            Ok(())
        }
    }
}

/// Borrowed canonical Claims sub-composition selected by one EffectProgram.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsCompositionV3<'a> {
    admit: Option<ProtocolPositionRequestV2>,
    affine: Option<AffineBatchPlanV2<'a>>,
    signed_delta: Option<SignedDeltaPlanV3<'a>>,
    sparse_native_transfer: Option<SparseNativeTransferV1>,
    founding: Option<&'a [u8]>,
    rational_representation: Option<&'a [u8]>,
    rational_lifecycle: Option<&'a [u8]>,
    close: Option<ProtocolPositionRequestV2>,
    admit_route: Option<u16>,
    mutation_route: u16,
    close_route: Option<u16>,
}

impl<'a> ClaimsCompositionV3<'a> {
    /// Hostile-decode the enabled Claims routes from an exact projected bank.
    pub fn decode_selected(
        effect: ProgramV3<'_>,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
        request_bank: &'a [u8],
        parent: ClaimsCompositionParentV3,
    ) -> Result<Self, ClaimsCompositionErrorV3> {
        Self::decode_selected_with_witness(
            effect,
            tail_count,
            scalars,
            identities,
            request_bank,
            &[],
            parent,
        )
    }

    /// Decode Claims routes whose sole mutation may borrow an authenticated
    /// trailing family-request witness.
    #[allow(clippy::too_many_arguments)]
    pub fn decode_selected_with_witness(
        effect: ProgramV3<'_>,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
        request_bank: &'a [u8],
        family_request: &'a [u8],
        parent: ClaimsCompositionParentV3,
    ) -> Result<Self, ClaimsCompositionErrorV3> {
        parent.validate()?;
        if effect
            .request_bytes(tail_count)
            .map_err(|_| ClaimsCompositionErrorV3::EffectProgram)?
            != request_bank.len()
        {
            return Err(ClaimsCompositionErrorV3::EffectProgram);
        }
        let mut admit = None;
        let mut affine = None;
        let mut signed_delta = None;
        let mut sparse_native_transfer = None;
        let mut founding = None;
        let mut rational_representation = None;
        let mut rational_lifecycle = None;
        let mut close = None;
        let mut admit_route = None;
        let mut mutation_route = None;
        let mut close_route = None;
        let mut state = CompositionStateV3::Start;
        let mut route_index = 0_u16;
        while route_index < effect.route_count() {
            let route = effect
                .route(route_index)
                .map_err(|_| ClaimsCompositionErrorV3::EffectProgram)?;
            let invocation_count = effect
                .invocation_count(route_index, tail_count, scalars, identities)
                .map_err(|_| ClaimsCompositionErrorV3::EffectProgram)?;
            if route.role() == FixedRole::Claims && invocation_count != 0 {
                if invocation_count != 1 {
                    return Err(ClaimsCompositionErrorV3::Route);
                }
                let invocation = effect
                    .resolved_invocation(route_index, 0, tail_count, scalars, identities)
                    .map_err(|_| ClaimsCompositionErrorV3::EffectProgram)?;
                let request = claims_request(invocation, request_bank, family_request)?;
                if request.get(..8) == Some(PROTOCOL_POSITION_REQUEST_MAGIC_V2.as_slice()) {
                    if route.kind() != RouteKindV3::Once {
                        return Err(ClaimsCompositionErrorV3::Route);
                    }
                    let decoded = ProtocolPositionRequestV2::decode(request)
                        .map_err(|_| ClaimsCompositionErrorV3::Route)?;
                    require_position_parent(decoded, parent)?;
                    match decoded.action {
                        ProtocolPositionActionV2::Admit => {
                            if state != CompositionStateV3::Start
                                || decoded.presence != ProtocolPositionPresenceV2::Vacant
                            {
                                return Err(ClaimsCompositionErrorV3::Order);
                            }
                            admit = Some(decoded);
                            admit_route = Some(route_index);
                            state = CompositionStateV3::Admitted;
                        }
                        ProtocolPositionActionV2::Close => {
                            if state != CompositionStateV3::Affined
                                || decoded.presence != ProtocolPositionPresenceV2::Existing
                            {
                                return Err(ClaimsCompositionErrorV3::Order);
                            }
                            close = Some(decoded);
                            close_route = Some(route_index);
                            state = CompositionStateV3::Closed;
                        }
                    }
                } else if request.get(..8) == Some(AFFINE_BATCH_PLAN_MAGIC_V2.as_slice()) {
                    if route.kind() != RouteKindV3::AffineOnce
                        || !matches!(
                            state,
                            CompositionStateV3::Start | CompositionStateV3::Admitted
                        )
                    {
                        return Err(ClaimsCompositionErrorV3::Order);
                    }
                    let decoded = AffineBatchPlanV2::decode(request)
                        .map_err(|_| ClaimsCompositionErrorV3::Route)?;
                    require_affine_parent(decoded, parent)?;
                    affine = Some(decoded);
                    mutation_route = Some(route_index);
                    state = CompositionStateV3::Affined;
                } else if request.get(..8) == Some(SIGNED_DELTA_PLAN_MAGIC_V3.as_slice()) {
                    if route.kind() != RouteKindV3::Once
                        || state != CompositionStateV3::Start
                        || invocation.request_len != 0
                        || invocation.borrowed_witness.is_none()
                    {
                        return Err(ClaimsCompositionErrorV3::Order);
                    }
                    let decoded = SignedDeltaPlanV3::decode(request)
                        .map_err(|_| ClaimsCompositionErrorV3::Route)?;
                    require_signed_delta_parent(decoded, parent)?;
                    let expected_accounts = SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3
                        .checked_add(
                            u16::try_from(decoded.position_count())
                                .map_err(|_| ClaimsCompositionErrorV3::Route)?,
                        )
                        .ok_or(ClaimsCompositionErrorV3::Route)?;
                    if invocation.fixed_account_count != expected_accounts
                        || invocation.item_account_count != 0
                        || invocation.repeated_item_count != 0
                    {
                        return Err(ClaimsCompositionErrorV3::Route);
                    }
                    signed_delta = Some(decoded);
                    mutation_route = Some(route_index);
                    state = CompositionStateV3::Affined;
                } else if request.get(..8) == Some(SPARSE_NATIVE_TRANSFER_MAGIC_V1.as_slice()) {
                    if route.kind() != RouteKindV3::Once || state != CompositionStateV3::Start {
                        return Err(ClaimsCompositionErrorV3::Order);
                    }
                    let decoded = SparseNativeTransferV1::decode(request)
                        .map_err(|_| ClaimsCompositionErrorV3::Route)?;
                    require_sparse_native_parent(decoded, parent)?;
                    if invocation.fixed_account_count
                        != SPARSE_NATIVE_TRANSFER_FIXED_ACCOUNT_COUNT_V1
                        || invocation.item_account_count != 0
                        || invocation.repeated_item_count != 0
                    {
                        return Err(ClaimsCompositionErrorV3::Route);
                    }
                    sparse_native_transfer = Some(decoded);
                    mutation_route = Some(route_index);
                    state = CompositionStateV3::Affined;
                } else if request.get(..8) == Some(CLAIMS_FOUNDING_REQUEST_MAGIC_V5.as_slice()) {
                    validate_founding_route(route.kind(), state, invocation, request, parent)?;
                    founding = Some(request);
                    mutation_route = Some(route_index);
                    state = CompositionStateV3::Affined;
                } else if request.get(..8) == Some(REPRESENTATION_REQUEST_MAGIC_V2.as_slice()) {
                    validate_rational_representation_route(
                        route.kind(),
                        state,
                        invocation,
                        request,
                        parent,
                    )?;
                    rational_representation = Some(request);
                    mutation_route = Some(route_index);
                    state = CompositionStateV3::Affined;
                } else if request.get(..8) == Some(LIFECYCLE_REQUEST_MAGIC_V2.as_slice()) {
                    validate_rational_lifecycle_route(
                        route.kind(),
                        state,
                        invocation,
                        request,
                        parent,
                    )?;
                    rational_lifecycle = Some(request);
                    mutation_route = Some(route_index);
                    state = CompositionStateV3::Affined;
                } else {
                    return Err(ClaimsCompositionErrorV3::Route);
                }
            }
            route_index = route_index
                .checked_add(1)
                .ok_or(ClaimsCompositionErrorV3::EffectProgram)?;
        }
        let mutation_route = mutation_route.ok_or(ClaimsCompositionErrorV3::MissingAffine)?;
        let mutation_count = u8::from(affine.is_some())
            .checked_add(u8::from(signed_delta.is_some()))
            .and_then(|count| count.checked_add(u8::from(sparse_native_transfer.is_some())))
            .and_then(|count| count.checked_add(u8::from(founding.is_some())))
            .and_then(|count| count.checked_add(u8::from(rational_representation.is_some())))
            .and_then(|count| count.checked_add(u8::from(rational_lifecycle.is_some())))
            .ok_or(ClaimsCompositionErrorV3::MissingAffine)?;
        if mutation_count != 1 {
            return Err(ClaimsCompositionErrorV3::MissingAffine);
        }
        if let Some(affine) = affine {
            if let Some(request) = admit {
                require_admission_join(request, affine)?;
            }
            if let Some(request) = close {
                require_close_join(request, affine)?;
            }
        } else if admit.is_some() || close.is_some() {
            return Err(ClaimsCompositionErrorV3::Order);
        }
        Ok(Self {
            admit,
            affine,
            signed_delta,
            sparse_native_transfer,
            founding,
            rational_representation,
            rational_lifecycle,
            close,
            admit_route,
            mutation_route,
            close_route,
        })
    }

    /// Optional canonical Position admission request.
    pub const fn admit(self) -> Option<ProtocolPositionRequestV2> {
        self.admit
    }

    /// Sole canonical affine balance-mutation plan.
    pub const fn affine(self) -> Option<AffineBatchPlanV2<'a>> {
        self.affine
    }

    /// Sole canonical signed-delta mutation, when selected.
    pub const fn signed_delta(self) -> Option<SignedDeltaPlanV3<'a>> {
        self.signed_delta
    }

    /// Sole canonical sparse native transfer, when selected.
    pub const fn sparse_native_transfer(self) -> Option<SparseNativeTransferV1> {
        self.sparse_native_transfer
    }

    /// Sole canonical permit-authorized founding mutation, when selected.
    pub fn founding(self) -> Option<ClaimsFoundingRequestV5> {
        self.founding
            .and_then(|request| ClaimsFoundingRequestV5::decode(request).ok())
    }

    /// Sole canonical Rational Representation V2 mutation, when selected.
    pub fn rational_representation(self) -> Option<RepresentationRequestV2<'a>> {
        self.rational_representation
            .and_then(|request| RepresentationRequestV2::decode(request).ok())
    }

    /// Sole Rational physical lifecycle request, when selected.
    pub fn rational_lifecycle(self) -> Option<LifecycleRequestV2<'a>> {
        self.rational_lifecycle
            .and_then(|request| LifecycleRequestV2::decode(request).ok())
    }

    /// Optional canonical zero-Position close request.
    pub const fn close(self) -> Option<ProtocolPositionRequestV2> {
        self.close
    }

    /// EffectProgram route selecting admission, when present.
    pub const fn admit_route(self) -> Option<u16> {
        self.admit_route
    }

    /// EffectProgram route selecting the sole Claims mutation.
    ///
    /// Retained as the source-compatible name for existing affine callers.
    pub const fn affine_route(self) -> u16 {
        self.mutation_route
    }

    /// EffectProgram route selecting the sole Claims mutation.
    pub const fn mutation_route(self) -> u16 {
        self.mutation_route
    }

    /// EffectProgram route selecting close, when present.
    pub const fn close_route(self) -> Option<u16> {
        self.close_route
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompositionStateV3 {
    Start,
    Admitted,
    Affined,
    Closed,
}

fn claims_request<'a>(
    invocation: dclutch_effect_kernel::v3::ResolvedInvocationV3,
    request_bank: &'a [u8],
    family_request: &'a [u8],
) -> Result<&'a [u8], ClaimsCompositionErrorV3> {
    let end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(ClaimsCompositionErrorV3::EffectProgram)?;
    let fixed = request_bank
        .get(invocation.request_offset..end)
        .ok_or(ClaimsCompositionErrorV3::EffectProgram)?;
    match invocation.borrowed_witness {
        None => Ok(fixed),
        Some(witness) if fixed.is_empty() => witness
            .slice(family_request)
            .map_err(|_| ClaimsCompositionErrorV3::EffectProgram),
        Some(_) => Err(ClaimsCompositionErrorV3::Route),
    }
}

fn require_position_parent(
    request: ProtocolPositionRequestV2,
    parent: ClaimsCompositionParentV3,
) -> Result<(), ClaimsCompositionErrorV3> {
    if request.release_set != parent.release_set
        || request.market != parent.market
        || request.generation != parent.generation
        || request.parent_request_digest != parent.parent_request_digest
    {
        Err(ClaimsCompositionErrorV3::ParentBinding)
    } else {
        Ok(())
    }
}

fn require_affine_parent(
    plan: AffineBatchPlanV2<'_>,
    parent: ClaimsCompositionParentV3,
) -> Result<(), ClaimsCompositionErrorV3> {
    if plan.caller_role() != CallerRole::Trading
        || plan.release_set() != parent.release_set
        || plan.market() != parent.market
        || plan.request_id() != parent.parent_request_digest
    {
        Err(ClaimsCompositionErrorV3::ParentBinding)
    } else {
        Ok(())
    }
}

fn require_signed_delta_parent(
    plan: SignedDeltaPlanV3<'_>,
    parent: ClaimsCompositionParentV3,
) -> Result<(), ClaimsCompositionErrorV3> {
    if plan.caller_role() != CallerRole::Trading
        || plan.release_set() != parent.release_set
        || plan.market() != parent.market
    {
        Err(ClaimsCompositionErrorV3::ParentBinding)
    } else {
        Ok(())
    }
}

fn require_sparse_native_parent(
    request: SparseNativeTransferV1,
    parent: ClaimsCompositionParentV3,
) -> Result<(), ClaimsCompositionErrorV3> {
    let input = request.input();
    if input.caller_role != CallerRole::Trading
        || input.release_set != parent.release_set
        || input.market != parent.market
        || input.request_id != parent.parent_request_digest
        || input.generation != parent.generation
    {
        Err(ClaimsCompositionErrorV3::ParentBinding)
    } else {
        Ok(())
    }
}

fn require_founding_parent(
    request: ClaimsFoundingRequestV5,
    parent: ClaimsCompositionParentV3,
) -> Result<(), ClaimsCompositionErrorV3> {
    // The founding-intent digest is Core-owned and authenticated by the
    // one-shot permit inside Claims. It is intentionally not a second copy of
    // the outer Trading request digest.
    if request.release_set() != parent.release_set
        || request.market() != parent.market
        || request.generation() != parent.generation
    {
        Err(ClaimsCompositionErrorV3::ParentBinding)
    } else {
        Ok(())
    }
}

#[inline(never)]
fn validate_founding_route(
    kind: RouteKindV3,
    state: CompositionStateV3,
    invocation: dclutch_effect_kernel::v3::ResolvedInvocationV3,
    request: &[u8],
    parent: ClaimsCompositionParentV3,
) -> Result<(), ClaimsCompositionErrorV3> {
    if kind != RouteKindV3::Once || state != CompositionStateV3::Start {
        return Err(ClaimsCompositionErrorV3::Order);
    }
    let decoded =
        ClaimsFoundingRequestV5::decode(request).map_err(|_| ClaimsCompositionErrorV3::Route)?;
    require_founding_parent(decoded, parent)?;
    if invocation.fixed_account_count != CLAIMS_FOUNDING_FIXED_ACCOUNT_COUNT_V5
        || invocation.item_account_count != 0
        || invocation.repeated_item_count != 0
        || invocation.borrowed_witness.is_some()
    {
        return Err(ClaimsCompositionErrorV3::Route);
    }
    Ok(())
}

#[inline(never)]
fn validate_rational_representation_route(
    kind: RouteKindV3,
    state: CompositionStateV3,
    invocation: dclutch_effect_kernel::v3::ResolvedInvocationV3,
    request: &[u8],
    parent: ClaimsCompositionParentV3,
) -> Result<(), ClaimsCompositionErrorV3> {
    if kind != RouteKindV3::Once || state != CompositionStateV3::Start {
        return Err(ClaimsCompositionErrorV3::Order);
    }
    let decoded =
        RepresentationRequestV2::decode(request).map_err(|_| ClaimsCompositionErrorV3::Route)?;
    let header = decoded.header();
    if header.caller_role != RepresentationCallerRoleV2::Trading
        || header.release_set != parent.release_set
        || header.market != parent.market
        || header.generation != parent.generation
        || header.parent_context != parent.parent_request_digest
    {
        return Err(ClaimsCompositionErrorV3::ParentBinding);
    }
    let expected_accounts = decoded
        .physical_account_count()
        .map_err(|_| ClaimsCompositionErrorV3::Route)?;
    if usize::from(invocation.fixed_account_count) != expected_accounts
        || invocation.item_account_count != 0
        || invocation.repeated_item_count != 0
        || invocation.borrowed_witness.is_some()
    {
        return Err(ClaimsCompositionErrorV3::Route);
    }
    Ok(())
}

#[inline(never)]
fn validate_rational_lifecycle_route(
    kind: RouteKindV3,
    state: CompositionStateV3,
    invocation: dclutch_effect_kernel::v3::ResolvedInvocationV3,
    request: &[u8],
    parent: ClaimsCompositionParentV3,
) -> Result<(), ClaimsCompositionErrorV3> {
    if kind != RouteKindV3::Once || state != CompositionStateV3::Start {
        return Err(ClaimsCompositionErrorV3::Order);
    }
    let decoded =
        LifecycleRequestV2::decode(request).map_err(|_| ClaimsCompositionErrorV3::Route)?;
    let header = decoded.header();
    if header.release_set != parent.release_set
        || header.market != parent.market
        || header.generation != parent.generation
        || header.parent_context != parent.parent_request_digest
    {
        return Err(ClaimsCompositionErrorV3::ParentBinding);
    }
    let expected_accounts = match header.action {
        LifecycleActionV2::ActivateReceipt => LIFECYCLE_COMMON_ACCOUNT_COUNT_V2,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => {
            LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2
        }
        LifecycleActionV2::RetireReceipt => usize::try_from(header.coordinate_count)
            .ok()
            .and_then(|count| count.checked_mul(LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2))
            .and_then(|tail| tail.checked_add(LIFECYCLE_COMMON_ACCOUNT_COUNT_V2))
            .ok_or(ClaimsCompositionErrorV3::Route)?,
    };
    if usize::from(invocation.fixed_account_count) != expected_accounts
        || invocation.item_account_count != 0
        || invocation.repeated_item_count != 0
        || invocation.borrowed_witness.is_some()
    {
        return Err(ClaimsCompositionErrorV3::Route);
    }
    Ok(())
}

fn require_admission_join(
    request: ProtocolPositionRequestV2,
    affine: AffineBatchPlanV2<'_>,
) -> Result<(), ClaimsCompositionErrorV3> {
    if request.expected_market_revision != affine.expected_market_revision()
        || request.expected_position_revision != 0
        || position_revision(affine, request.position_owner) != Some(0)
    {
        Err(ClaimsCompositionErrorV3::AdmissionJoin)
    } else {
        Ok(())
    }
}

fn require_close_join(
    request: ProtocolPositionRequestV2,
    affine: AffineBatchPlanV2<'_>,
) -> Result<(), ClaimsCompositionErrorV3> {
    let post_market_revision = affine
        .expected_market_revision()
        .checked_add(1)
        .ok_or(ClaimsCompositionErrorV3::CloseJoin)?;
    let pre_position_revision = position_revision(affine, request.position_owner)
        .ok_or(ClaimsCompositionErrorV3::CloseJoin)?;
    let post_position_revision = pre_position_revision
        .checked_add(1)
        .ok_or(ClaimsCompositionErrorV3::CloseJoin)?;
    if request.expected_market_revision != post_market_revision
        || request.expected_position_revision != post_position_revision
    {
        Err(ClaimsCompositionErrorV3::CloseJoin)
    } else {
        Ok(())
    }
}

fn position_revision(plan: AffineBatchPlanV2<'_>, owner: [u8; 32]) -> Option<u64> {
    let mut index = 0_u32;
    while index < plan.position_count() {
        let position = plan.position(index).ok()?;
        if position.owner() == owner {
            return Some(position.expected_revision());
        }
        index = index.checked_add(1)?;
    }
    None
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;
    use std::vec::Vec;

    use dclutch_rational_representation_v2_lifecycle_contract::{
        LIFECYCLE_COORDINATE_BYTES_V2, LIFECYCLE_HEADER_BYTES_V2, LifecycleCoordinateV2,
        LifecycleHeaderV2,
    };
    use dclutch_rational_representation_v2_request_contract::{
        ABSENT_REVISION, ASSET_BYTES_V2, AssetV2, RepresentationActionV2,
        RepresentationRequestHeaderV2,
    };
    use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

    use crate::{
        affine_batch_v2::{
            AffineBatchPlanInputV2, AffineBatchPositionV2, AffineBatchRowInputV2, AffineBatchRowV2,
            DeltaDirectionV2, SignedMagnitudeV2, plan_bytes,
        },
        founding_v5::{ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5},
        protocol_position_v2::ProtocolPositionOwnerKindV2,
        signed_delta_v3::{
            DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SignedDeltaPlanInputV3,
            SignedDeltaPlanV3, SignedDeltaPositionV3, SignedDeltaV3,
            plan_bytes as signed_plan_bytes,
        },
        sparse_native_transfer_v1::{SparseNativeTransferInputV1, SparseNativeTransferV1},
    };

    use super::*;

    const TAIL_COUNT: u32 = 1;
    const MARKET_REVISION: u64 = 5;

    #[derive(Clone)]
    struct RouteFixture {
        role: u8,
        kind: u8,
        enabled: bool,
        fixed_account_count: u16,
        request: Vec<u8>,
    }

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn parent() -> ClaimsCompositionParentV3 {
        ClaimsCompositionParentV3 {
            release_set: id(1),
            market: id(2),
            generation: 7,
            parent_request_digest: id(3),
        }
    }

    fn position_request(
        action: ProtocolPositionActionV2,
        owner: [u8; 32],
        market_revision: u64,
        position_revision: u64,
    ) -> ProtocolPositionRequestV2 {
        ProtocolPositionRequestV2 {
            action,
            owner_kind: if owner == id(10) {
                ProtocolPositionOwnerKindV2::TradingRecord
            } else {
                ProtocolPositionOwnerKindV2::User
            },
            presence: match action {
                ProtocolPositionActionV2::Admit => ProtocolPositionPresenceV2::Vacant,
                ProtocolPositionActionV2::Close => ProtocolPositionPresenceV2::Existing,
            },
            release_set: parent().release_set,
            market: parent().market,
            position_owner: owner,
            parent_request_digest: parent().parent_request_digest,
            rent_credit: id(20),
            rent_program: id(21),
            generation: parent().generation,
            expected_market_revision: market_revision,
            expected_position_revision: position_revision,
            observed_position_lamports: 101,
            observed_admission_lamports: 103,
            position_rent_principal: 100,
            admission_rent_principal: 100,
            capability_descriptor: [0; 32],
            capability_outcome: 0,
        }
        .new()
        .expect("position request")
    }

    fn delta(direction: DeltaDirectionV2, magnitude: u64) -> SignedMagnitudeV2 {
        SignedMagnitudeV2::new(direction, magnitude).expect("delta")
    }

    fn affine(source: [u8; 32], source_revision: u64, destination: [u8; 32]) -> Vec<u8> {
        let positions = [
            AffineBatchPositionV2::new(source, source_revision).expect("source"),
            AffineBatchPositionV2::new(destination, 0).expect("destination"),
        ];
        let rows = [AffineBatchRowV2::new(
            AffineBatchRowInputV2 {
                source_present: true,
                destination_present: true,
                outcome: 1,
                source_position_index: 0,
                destination_position_index: 1,
                aggregate_delta: delta(DeltaDirectionV2::Neutral, 0),
                source_delta: delta(DeltaDirectionV2::Debit, 9),
                destination_delta: delta(DeltaDirectionV2::Credit, 9),
            },
            3,
            2,
        )
        .expect("row")];
        let mut output = vec![0; plan_bytes(2, 1).expect("plan width")];
        AffineBatchPlanV2::encode_into(
            AffineBatchPlanInputV2 {
                caller_role: CallerRole::Trading,
                release_set: parent().release_set,
                market: parent().market,
                request_id: parent().parent_request_digest,
                product_record_digest: id(30),
                semantic_basis_id: id(31),
                linked_basis_record_digest: id(32),
                expected_market_revision: MARKET_REVISION,
                outcome_count: 3,
            },
            &positions,
            &rows,
            &mut output,
        )
        .expect("affine plan");
        output
    }

    fn route(kind: u8, request: Vec<u8>) -> RouteFixture {
        RouteFixture {
            role: 1,
            kind,
            enabled: false,
            fixed_account_count: 1,
            request,
        }
    }

    fn effect(routes: &[RouteFixture]) -> (Vec<u8>, Vec<u8>) {
        let route_count = u16::try_from(routes.len()).expect("route count");
        let route_bytes = routes
            .len()
            .checked_mul(dclutch_effect_kernel::v3::ROUTE_BYTES)
            .expect("route bytes");
        let header = 32_usize.checked_add(route_bytes).expect("header");
        let templates = routes.iter().try_fold(0_usize, |total, route| {
            total.checked_add(route.request.len())
        });
        let mut bytes = vec![0; header + templates.expect("template bytes")];
        put(&mut bytes, 0, b"DCE4");
        put(&mut bytes, 4, &[4, 0]);
        put(&mut bytes, 6, &route_count.to_le_bytes());
        let fixed_accounts = routes
            .iter()
            .map(|route| route.fixed_account_count)
            .max()
            .unwrap_or(1);
        put(&mut bytes, 12, &fixed_accounts.to_le_bytes());
        put(&mut bytes, 14, &1_u16.to_le_bytes());
        put(&mut bytes, 16, &1_u16.to_le_bytes());
        put(&mut bytes, 20, &1_u16.to_le_bytes());
        let mut template_offset = header;
        for (index, route) in routes.iter().enumerate() {
            let offset = 32_usize
                .checked_add(
                    index
                        .checked_mul(dclutch_effect_kernel::v3::ROUTE_BYTES)
                        .expect("route offset"),
                )
                .expect("route offset");
            put(&mut bytes, offset, &[route.role]);
            put(&mut bytes, offset + 1, &[route.kind]);
            put(&mut bytes, offset + 2, &[u8::from(route.enabled)]);
            put(
                &mut bytes,
                offset + 8,
                &route.fixed_account_count.to_le_bytes(),
            );
            let request_len = u32::try_from(route.request.len()).expect("request len");
            put(&mut bytes, offset + 16, &request_len.to_le_bytes());
            put(&mut bytes, template_offset, &route.request);
            template_offset = template_offset
                .checked_add(route.request.len())
                .expect("template offset");
        }
        let request_bank = routes
            .iter()
            .flat_map(|route| route.request.iter().copied())
            .collect();
        (bytes, request_bank)
    }

    fn signed_family() -> (Vec<u8>, Vec<u8>) {
        let positions = [
            SignedDeltaPositionV3::new(id(10), 4).expect("dealer"),
            SignedDeltaPositionV3::new(id(11), 5).expect("LP"),
        ];
        let aggregates = [
            SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("zero"),
            SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("zero"),
        ];
        let rows = [
            PositionDeltaV3::new(
                PositionDeltaInputV3 {
                    position_index: 0,
                    outcome: 1,
                    delta: SignedDeltaV3::new(DeltaDirectionV3::Credit, 5).expect("credit"),
                },
                2,
                2,
            )
            .expect("dealer row"),
            PositionDeltaV3::new(
                PositionDeltaInputV3 {
                    position_index: 1,
                    outcome: 1,
                    delta: SignedDeltaV3::new(DeltaDirectionV3::Debit, 5).expect("debit"),
                },
                2,
                2,
            )
            .expect("LP row"),
        ];
        let mut packet = vec![0; signed_plan_bytes(2, 2, 2).expect("packet bytes")];
        SignedDeltaPlanV3::encode_into(
            SignedDeltaPlanInputV3 {
                caller_role: CallerRole::Trading,
                release_set: parent().release_set,
                market: parent().market,
                request_id: id(40),
                product_record_digest: id(41),
                semantic_basis_id: id(42),
                linked_basis_record_digest: id(43),
                expected_market_revision: MARKET_REVISION,
                claim_count: 2,
            },
            &positions,
            &aggregates,
            &rows,
            &mut packet,
        )
        .expect("signed packet");
        let mut family = vec![0; 480];
        family.extend_from_slice(&packet);

        let mut effect = vec![
            0;
            dclutch_effect_kernel::v3::HEADER_BYTES
                + dclutch_effect_kernel::v3::ROUTE_BYTES
        ];
        put(&mut effect, 0, b"DCE4");
        put(&mut effect, 4, &[4, 0]);
        put(&mut effect, 6, &1_u16.to_le_bytes());
        put(&mut effect, 12, &22_u16.to_le_bytes());
        put(&mut effect, 16, &3_u16.to_le_bytes());
        put(&mut effect, 20, &1_u16.to_le_bytes());
        put(&mut effect, 32, &[1, 0, 1, 1]);
        put(&mut effect, 36, &0_u16.to_le_bytes());
        put(&mut effect, 38, &0_u16.to_le_bytes());
        put(&mut effect, 40, &22_u16.to_le_bytes());
        put(&mut effect, 46, &1_u16.to_le_bytes());
        (effect, family)
    }

    fn put(output: &mut [u8], offset: usize, value: &[u8]) {
        let end = offset.checked_add(value.len()).expect("field end");
        output
            .get_mut(offset..end)
            .expect("field")
            .copy_from_slice(value);
    }

    #[test]
    fn composes_optional_admit_affine_and_exact_post_affine_close() {
        let record = id(10);
        let buyer = id(11);
        let admit = position_request(ProtocolPositionActionV2::Admit, buyer, MARKET_REVISION, 0)
            .to_bytes()
            .expect("admit")
            .to_vec();
        let close = position_request(
            ProtocolPositionActionV2::Close,
            record,
            MARKET_REVISION + 1,
            9,
        )
        .to_bytes()
        .expect("close")
        .to_vec();
        let (effect_bytes, requests) = effect(&[
            route(0, admit),
            route(1, affine(record, 8, buyer)),
            route(0, close),
        ]);
        let program = ProgramV3::decode(&effect_bytes).expect("EffectProgram");
        let composition = ClaimsCompositionV3::decode_selected(
            program,
            TAIL_COUNT,
            &[1],
            &[id(40)],
            &requests,
            parent(),
        )
        .expect("composition");
        assert_eq!(composition.admit_route(), Some(0));
        assert_eq!(composition.affine_route(), 1);
        assert_eq!(composition.close_route(), Some(2));
        assert_eq!(composition.affine().expect("affine").position_count(), 2);
    }

    #[test]
    fn composes_one_borrowed_signed_delta_with_exact_position_frame() {
        let (effect_bytes, family) = signed_family();
        let effect = ProgramV3::decode(&effect_bytes).expect("signed EffectProgram");
        let composition = ClaimsCompositionV3::decode_selected_with_witness(
            effect,
            0,
            &[1, 480, u64::try_from(family.len() - 480).expect("witness")],
            &[id(50)],
            &[],
            &family,
            parent(),
        )
        .expect("signed composition");
        assert!(composition.affine().is_none());
        assert_eq!(composition.mutation_route(), 0);
        assert_eq!(
            composition.signed_delta().expect("signed").position_count(),
            2
        );

        let mut wrong_frame = effect_bytes;
        put(&mut wrong_frame, 40, &21_u16.to_le_bytes());
        assert_eq!(
            ClaimsCompositionV3::decode_selected_with_witness(
                ProgramV3::decode(&wrong_frame).expect("structural effect"),
                0,
                &[1, 480, u64::try_from(family.len() - 480).expect("witness")],
                &[id(50)],
                &[],
                &family,
                parent(),
            ),
            Err(ClaimsCompositionErrorV3::Route)
        );
    }

    #[test]
    fn disabled_admission_is_absent_without_parsing_its_placeholder() {
        let record = id(10);
        let buyer = id(11);
        let (effect_bytes, requests) = effect(&[
            RouteFixture {
                role: 1,
                kind: 0,
                enabled: true,
                fixed_account_count: 1,
                request: vec![0xa5; 320],
            },
            route(1, affine(record, 8, buyer)),
        ]);
        let mut scalars = [0_u64];
        let program = ProgramV3::decode(&effect_bytes).expect("EffectProgram");
        let composition = ClaimsCompositionV3::decode_selected(
            program,
            TAIL_COUNT,
            &scalars,
            &[id(40)],
            &requests,
            parent(),
        )
        .expect("disabled admission composition");
        assert_eq!(composition.admit(), None);
        assert_eq!(composition.affine_route(), 1);
        scalars[0] = 1;
        assert_eq!(
            ClaimsCompositionV3::decode_selected(
                program,
                TAIL_COUNT,
                &scalars,
                &[id(40)],
                &requests,
                parent(),
            ),
            Err(ClaimsCompositionErrorV3::Route)
        );
    }

    #[test]
    fn refuses_wrong_order_parent_owner_and_post_revision() {
        let record = id(10);
        let buyer = id(11);
        let close = position_request(
            ProtocolPositionActionV2::Close,
            record,
            MARKET_REVISION + 1,
            9,
        )
        .to_bytes()
        .expect("close")
        .to_vec();
        let (wrong_order, requests) =
            effect(&[route(0, close.clone()), route(1, affine(record, 8, buyer))]);
        assert_eq!(
            ClaimsCompositionV3::decode_selected(
                ProgramV3::decode(&wrong_order).expect("program"),
                TAIL_COUNT,
                &[1],
                &[id(40)],
                &requests,
                parent(),
            ),
            Err(ClaimsCompositionErrorV3::Order)
        );

        let hostile_close = position_request(
            ProtocolPositionActionV2::Close,
            record,
            MARKET_REVISION + 1,
            10,
        )
        .to_bytes()
        .expect("hostile close")
        .to_vec();
        let (wrong_revision, requests) =
            effect(&[route(1, affine(record, 8, buyer)), route(0, hostile_close)]);
        assert_eq!(
            ClaimsCompositionV3::decode_selected(
                ProgramV3::decode(&wrong_revision).expect("program"),
                TAIL_COUNT,
                &[1],
                &[id(40)],
                &requests,
                parent(),
            ),
            Err(ClaimsCompositionErrorV3::CloseJoin)
        );

        let absent = position_request(ProtocolPositionActionV2::Admit, id(12), MARKET_REVISION, 0)
            .to_bytes()
            .expect("absent admit")
            .to_vec();
        let (wrong_owner, requests) =
            effect(&[route(0, absent), route(1, affine(record, 8, buyer))]);
        assert_eq!(
            ClaimsCompositionV3::decode_selected(
                ProgramV3::decode(&wrong_owner).expect("program"),
                TAIL_COUNT,
                &[1],
                &[id(40)],
                &requests,
                parent(),
            ),
            Err(ClaimsCompositionErrorV3::AdmissionJoin)
        );

        let (canonical, requests) = effect(&[route(1, affine(record, 8, buyer))]);
        let mut hostile_parent = parent();
        hostile_parent.parent_request_digest = id(99);
        assert_eq!(
            ClaimsCompositionV3::decode_selected(
                ProgramV3::decode(&canonical).expect("program"),
                TAIL_COUNT,
                &[1],
                &[id(40)],
                &requests,
                hostile_parent,
            ),
            Err(ClaimsCompositionErrorV3::ParentBinding)
        );
    }

    fn sparse_request() -> SparseNativeTransferV1 {
        SparseNativeTransferV1::new(SparseNativeTransferInputV1 {
            caller_role: CallerRole::Trading,
            release_set: parent().release_set,
            market: parent().market,
            request_id: parent().parent_request_digest,
            product_record_digest: id(30),
            semantic_basis_id: id(31),
            linked_basis_record_digest: id(32),
            source_owner: id(10),
            destination_owner: id(11),
            expected_market_revision: MARKET_REVISION,
            expected_source_revision: 7,
            expected_destination_revision: 8,
            generation: parent().generation,
            outcome: 1,
            claim_count: 3,
            quantity: 9,
        })
        .expect("sparse request")
    }

    fn founding_request() -> ClaimsFoundingRequestV5 {
        ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
            release_set: parent().release_set,
            market: parent().market,
            product_record_digest: id(30),
            product_instance_id: id(31),
            linked_basis_record_digest: id(32),
            semantic_basis_id: id(33),
            founder: id(34),
            founding_intent_digest: id(35),
            aggregate: id(36),
            position: id(37),
            admission: id(38),
            funding_source: id(39),
            hoard: id(40),
            custody_replay: id(41),
            rent_credit: id(42),
            rent_program: id(43),
            claims_program: id(44),
            trading_program: id(45),
            custody_request_digest: id(46),
            custody_receipt_digest: id(47),
            generation: parent().generation,
            claim_count: 3,
            quantity: 2,
            basis_scale: 5,
            pre_source_amount: 10,
            post_source_amount: 0,
            pre_hoard_amount: 7,
            post_hoard_amount: 17,
            pre_custody_revision: 0,
            post_custody_revision: 1,
            aggregate_rent_principal: 100,
            position_rent_principal: 101,
            admission_rent_principal: 102,
            observed_aggregate_lamports: 100,
            observed_position_lamports: 101,
            observed_admission_lamports: 102,
            pre_aggregate_revision: 0,
            post_aggregate_revision: 1,
            pre_position_revision: 0,
            post_position_revision: 1,
        })
        .expect("founding request")
    }

    fn rational_representation_request() -> Vec<u8> {
        let mut asset = [0_u8; ASSET_BYTES_V2];
        AssetV2 {
            shard_mint: id(50),
            actor_shard_account: id(51),
            structured_custody_account: id(52),
            claims_custody_owner: id(53),
            coefficient: 10,
            expected_shard_supply: 20,
            expected_actor_shards: 10,
            expected_structured_shards: 10,
        }
        .encode_into(&mut asset)
        .expect("representation asset");
        let request = RepresentationRequestV2::new(
            RepresentationRequestHeaderV2 {
                action: RepresentationActionV2::Denominate,
                caller_role: RepresentationCallerRoleV2::Trading,
                release_set: parent().release_set,
                market: parent().market,
                graph_id: id(54),
                descriptor_id: id(55),
                parent_context: parent().parent_request_digest,
                actor: id(56),
                receipt_mint: id(57),
                receipt_account: [0; 32],
                representation_authority: id(58),
                token_program: TOKEN_2022_PROGRAM_ID,
                realm: [0; 32],
                collateral_recipient: [0; 32],
                expected_representation_revision: 3,
                expected_claims_market_revision: MARKET_REVISION,
                expected_actor_position_revision: 4,
                expected_custody_position_revision: 5,
                expected_custody_replay_revision: ABSENT_REVISION,
                generation: parent().generation,
                quantity: 1,
                denominator: 10,
                expected_receipt_supply: 0,
                outcome_count: 3,
                selected_outcome: 1,
                asset_count: 1,
            },
            &asset,
        )
        .expect("representation request");
        let mut bytes = vec![
            0_u8;
            dclutch_rational_representation_v2_request_contract::REQUEST_HEADER_BYTES_V2
                + ASSET_BYTES_V2
        ];
        request
            .encode_into(&mut bytes)
            .expect("representation bytes");
        bytes
    }

    fn lifecycle_request(action: LifecycleActionV2, rows: u32) -> Vec<u8> {
        let mut coordinate_bytes = Vec::new();
        for row in 0..rows {
            let mut bytes = [0_u8; LIFECYCLE_COORDINATE_BYTES_V2];
            LifecycleCoordinateV2 {
                outcome: row,
                coefficient: 1,
                shard_mint: id(50_u8
                    .checked_add(u8::try_from(row).expect("small row"))
                    .expect("id")),
                structured_custody_account: id(60_u8
                    .checked_add(u8::try_from(row).expect("small row"))
                    .expect("id")),
                claims_custody_owner: id(70_u8
                    .checked_add(u8::try_from(row).expect("small row"))
                    .expect("id")),
                claims_custody_position: id(80_u8
                    .checked_add(u8::try_from(row).expect("small row"))
                    .expect("id")),
                position_admission: id(90_u8
                    .checked_add(u8::try_from(row).expect("small row"))
                    .expect("id")),
                observed_shard_lamports: 10,
                observed_structured_lamports: 11,
                observed_position_lamports: 12,
                observed_admission_lamports: 13,
                shard_rent_principal: 10,
                structured_rent_principal: 11,
                position_rent_principal: 12,
                admission_rent_principal: 13,
                expected_shard_supply: 0,
                expected_structured_amount: 0,
                expected_position_revision: 0,
            }
            .encode_into(&mut bytes)
            .expect("coordinate");
            coordinate_bytes.extend_from_slice(&bytes);
        }
        let request = LifecycleRequestV2::new(
            LifecycleHeaderV2 {
                action,
                release_set: parent().release_set,
                market: parent().market,
                graph_id: id(20),
                descriptor_id: id(21),
                parent_context: parent().parent_request_digest,
                representation_authority: id(22),
                receipt_mint: id(23),
                token_program: TOKEN_2022_PROGRAM_ID,
                rent_credit: id(24),
                rent_program: id(25),
                generation: parent().generation,
                expected_claims_market_revision: 3,
                observed_receipt_lamports: 10,
                receipt_rent_principal: 10,
                expected_receipt_supply: 0,
                outcome_count: 3,
                coordinate_count: rows,
                rent_credit_before: 100,
                rent_credit_after: 100,
            },
            &coordinate_bytes,
        )
        .expect("lifecycle request");
        let mut output = vec![
            0_u8;
            LIFECYCLE_HEADER_BYTES_V2
                + usize::try_from(rows).expect("rows")
                    * LIFECYCLE_COORDINATE_BYTES_V2
        ];
        request.encode_into(&mut output).expect("lifecycle bytes");
        output
    }

    #[test]
    fn composes_one_exact_sparse_native_transfer_and_refuses_geometry_or_parent_substitution() {
        let sparse = sparse_request().to_bytes().to_vec();
        let canonical_route = RouteFixture {
            role: 1,
            kind: 0,
            enabled: false,
            fixed_account_count: SPARSE_NATIVE_TRANSFER_FIXED_ACCOUNT_COUNT_V1,
            request: sparse.clone(),
        };
        let (canonical_effect_bytes, canonical_request_bank) =
            effect(core::slice::from_ref(&canonical_route));
        let composition = ClaimsCompositionV3::decode_selected(
            ProgramV3::decode(&canonical_effect_bytes).expect("sparse EffectProgram"),
            TAIL_COUNT,
            &[1],
            &[id(40)],
            &canonical_request_bank,
            parent(),
        )
        .expect("sparse composition");
        assert_eq!(composition.mutation_route(), 0);
        assert_eq!(composition.sparse_native_transfer(), Some(sparse_request()));
        assert!(composition.affine().is_none());
        assert!(composition.signed_delta().is_none());

        let mut wrong_geometry = canonical_route.clone();
        wrong_geometry.fixed_account_count = SPARSE_NATIVE_TRANSFER_FIXED_ACCOUNT_COUNT_V1 - 1;
        let (effect_bytes, request_bank) = effect(&[wrong_geometry]);
        assert_eq!(
            ClaimsCompositionV3::decode_selected(
                ProgramV3::decode(&effect_bytes).expect("structural EffectProgram"),
                TAIL_COUNT,
                &[1],
                &[id(40)],
                &request_bank,
                parent(),
            ),
            Err(ClaimsCompositionErrorV3::Route)
        );

        let mut hostile_parent = parent();
        hostile_parent.parent_request_digest = id(99);
        assert_eq!(
            ClaimsCompositionV3::decode_selected(
                ProgramV3::decode(&canonical_effect_bytes).expect("structural EffectProgram"),
                TAIL_COUNT,
                &[1],
                &[id(40)],
                &canonical_request_bank,
                hostile_parent,
            ),
            Err(ClaimsCompositionErrorV3::ParentBinding)
        );
    }

    #[test]
    fn composes_one_exact_founding_and_refuses_geometry_or_parent_substitution() {
        let canonical_route = RouteFixture {
            role: 1,
            kind: 0,
            enabled: false,
            fixed_account_count: CLAIMS_FOUNDING_FIXED_ACCOUNT_COUNT_V5,
            request: founding_request().to_bytes().to_vec(),
        };
        let (effect_bytes, request_bank) = effect(core::slice::from_ref(&canonical_route));
        let composition = ClaimsCompositionV3::decode_selected(
            ProgramV3::decode(&effect_bytes).expect("founding EffectProgram"),
            TAIL_COUNT,
            &[1],
            &[id(40)],
            &request_bank,
            parent(),
        )
        .expect("founding composition");
        assert_eq!(composition.mutation_route(), 0);
        assert_eq!(composition.founding(), Some(founding_request()));
        assert!(composition.affine().is_none());
        assert!(composition.signed_delta().is_none());
        assert!(composition.sparse_native_transfer().is_none());

        let mut wrong_geometry = canonical_route.clone();
        wrong_geometry.fixed_account_count = CLAIMS_FOUNDING_FIXED_ACCOUNT_COUNT_V5 - 1;
        let (wrong_effect, wrong_bank) = effect(&[wrong_geometry]);
        assert_eq!(
            ClaimsCompositionV3::decode_selected(
                ProgramV3::decode(&wrong_effect).expect("structural EffectProgram"),
                TAIL_COUNT,
                &[1],
                &[id(40)],
                &wrong_bank,
                parent(),
            ),
            Err(ClaimsCompositionErrorV3::Route)
        );

        for hostile_parent in [
            ClaimsCompositionParentV3 {
                release_set: id(99),
                ..parent()
            },
            ClaimsCompositionParentV3 {
                generation: parent().generation + 1,
                ..parent()
            },
        ] {
            assert_eq!(
                ClaimsCompositionV3::decode_selected(
                    ProgramV3::decode(&effect_bytes).expect("structural EffectProgram"),
                    TAIL_COUNT,
                    &[1],
                    &[id(40)],
                    &request_bank,
                    hostile_parent,
                ),
                Err(ClaimsCompositionErrorV3::ParentBinding)
            );
        }
    }

    #[test]
    fn composes_exact_rational_representation_and_refuses_frame_or_parent_substitution() {
        let request = rational_representation_request();
        let decoded = RepresentationRequestV2::decode(&request).expect("representation request");
        let canonical_route = RouteFixture {
            role: 1,
            kind: 0,
            enabled: false,
            fixed_account_count: u16::try_from(
                decoded.physical_account_count().expect("physical frame"),
            )
            .expect("u16 frame"),
            request: request.clone(),
        };
        let (effect_bytes, request_bank) = effect(core::slice::from_ref(&canonical_route));
        let composition = ClaimsCompositionV3::decode_selected(
            ProgramV3::decode(&effect_bytes).expect("representation EffectProgram"),
            TAIL_COUNT,
            &[1],
            &[id(40)],
            &request_bank,
            parent(),
        )
        .expect("representation composition");
        assert_eq!(
            composition
                .rational_representation()
                .expect("representation")
                .header(),
            decoded.header()
        );
        assert!(composition.rational_lifecycle().is_none());
        assert!(composition.affine().is_none());

        let mut wrong_frame = canonical_route.clone();
        wrong_frame.fixed_account_count = wrong_frame
            .fixed_account_count
            .checked_sub(1)
            .expect("nonzero frame");
        let (wrong_effect, wrong_bank) = effect(&[wrong_frame]);
        assert_eq!(
            ClaimsCompositionV3::decode_selected(
                ProgramV3::decode(&wrong_effect).expect("structural EffectProgram"),
                TAIL_COUNT,
                &[1],
                &[id(40)],
                &wrong_bank,
                parent(),
            ),
            Err(ClaimsCompositionErrorV3::Route)
        );

        for hostile_parent in [
            ClaimsCompositionParentV3 {
                generation: parent().generation + 1,
                ..parent()
            },
            ClaimsCompositionParentV3 {
                parent_request_digest: id(99),
                ..parent()
            },
        ] {
            assert_eq!(
                ClaimsCompositionV3::decode_selected(
                    ProgramV3::decode(&effect_bytes).expect("structural EffectProgram"),
                    TAIL_COUNT,
                    &[1],
                    &[id(40)],
                    &request_bank,
                    hostile_parent,
                ),
                Err(ClaimsCompositionErrorV3::ParentBinding)
            );
        }
    }

    #[test]
    fn composes_exact_rational_lifecycle_and_refuses_frame_or_parent_substitution() {
        let request = lifecycle_request(LifecycleActionV2::ActivateCoordinate, 1);
        let canonical_route = RouteFixture {
            role: 1,
            kind: 0,
            enabled: false,
            fixed_account_count: u16::try_from(LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2)
                .expect("frame"),
            request: request.clone(),
        };
        let (effect_bytes, request_bank) = effect(core::slice::from_ref(&canonical_route));
        let composition = ClaimsCompositionV3::decode_selected(
            ProgramV3::decode(&effect_bytes).expect("lifecycle EffectProgram"),
            TAIL_COUNT,
            &[1],
            &[id(40)],
            &request_bank,
            parent(),
        )
        .expect("lifecycle composition");
        let lifecycle = composition.rational_lifecycle().expect("lifecycle");
        assert_eq!(
            lifecycle.header().action,
            LifecycleActionV2::ActivateCoordinate
        );
        assert_eq!(
            lifecycle.header().parent_context,
            parent().parent_request_digest
        );
        assert!(composition.affine().is_none());
        assert!(composition.founding().is_none());

        let mut wrong_frame = canonical_route.clone();
        wrong_frame.fixed_account_count = wrong_frame
            .fixed_account_count
            .checked_sub(1)
            .expect("nonzero frame");
        let (wrong_effect, wrong_bank) = effect(&[wrong_frame]);
        assert_eq!(
            ClaimsCompositionV3::decode_selected(
                ProgramV3::decode(&wrong_effect).expect("structural EffectProgram"),
                TAIL_COUNT,
                &[1],
                &[id(40)],
                &wrong_bank,
                parent(),
            ),
            Err(ClaimsCompositionErrorV3::Route)
        );

        let mut hostile_parent = parent();
        hostile_parent.parent_request_digest = id(99);
        assert_eq!(
            ClaimsCompositionV3::decode_selected(
                ProgramV3::decode(&effect_bytes).expect("structural EffectProgram"),
                TAIL_COUNT,
                &[1],
                &[id(40)],
                &request_bank,
                hostile_parent,
            ),
            Err(ClaimsCompositionErrorV3::ParentBinding)
        );
    }
}
