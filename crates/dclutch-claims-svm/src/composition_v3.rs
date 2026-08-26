//! Generic EffectProgram V3 composition for canonical Claims lifecycle routes.
//!
//! The selected EffectProgram owns route enablement, request templates, and
//! account geometry. This module adds only the cross-route economic join that
//! no individual child request can prove: an optional canonical Position admit
//! precedes exactly one affine batch, and an optional zero-Position close
//! follows it. It introduces no balance mutation, family tag, seed rule, or
//! parallel request DTO.

use dclutch_effect_kernel::v2::FixedRole;
use dclutch_effect_kernel::v3::{ProgramV3, RouteKindV3};

use crate::{
    affine_batch_v2::{AffineBatchPlanV2, AFFINE_BATCH_PLAN_MAGIC_V2},
    protocol_position_v2::{
        ProtocolPositionActionV2, ProtocolPositionPresenceV2, ProtocolPositionRequestV2,
        PROTOCOL_POSITION_REQUEST_MAGIC_V2,
    },
    CallerRole,
};

/// Stable cross-route composition refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimsCompositionErrorV3 {
    /// EffectProgram dimensions, register banks, or request bank differed.
    EffectProgram,
    /// An active Claims route used unsupported geometry or packet bytes.
    Route,
    /// The active Claims lifecycle order was not Admit? → Affine → Close?.
    Order,
    /// Release, Market, generation, or parent-request identity differed.
    ParentBinding,
    /// Admission did not create one affine Position at revision zero.
    AdmissionJoin,
    /// Close did not consume one affine Position at its exact post revision.
    CloseJoin,
    /// No canonical affine Claims mutation was selected.
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
    affine: AffineBatchPlanV2<'a>,
    close: Option<ProtocolPositionRequestV2>,
    admit_route: Option<u16>,
    affine_route: u16,
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
        let mut close = None;
        let mut admit_route = None;
        let mut affine_route = None;
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
                let end = invocation
                    .request_offset
                    .checked_add(invocation.request_len)
                    .ok_or(ClaimsCompositionErrorV3::EffectProgram)?;
                let request = request_bank
                    .get(invocation.request_offset..end)
                    .ok_or(ClaimsCompositionErrorV3::EffectProgram)?;
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
                    affine_route = Some(route_index);
                    state = CompositionStateV3::Affined;
                } else {
                    return Err(ClaimsCompositionErrorV3::Route);
                }
            }
            route_index = route_index
                .checked_add(1)
                .ok_or(ClaimsCompositionErrorV3::EffectProgram)?;
        }
        let affine = affine.ok_or(ClaimsCompositionErrorV3::MissingAffine)?;
        let affine_route = affine_route.ok_or(ClaimsCompositionErrorV3::MissingAffine)?;
        if let Some(request) = admit {
            require_admission_join(request, affine)?;
        }
        if let Some(request) = close {
            require_close_join(request, affine)?;
        }
        Ok(Self {
            admit,
            affine,
            close,
            admit_route,
            affine_route,
            close_route,
        })
    }

    /// Optional canonical Position admission request.
    pub const fn admit(self) -> Option<ProtocolPositionRequestV2> {
        self.admit
    }

    /// Sole canonical affine balance-mutation plan.
    pub const fn affine(self) -> AffineBatchPlanV2<'a> {
        self.affine
    }

    /// Optional canonical zero-Position close request.
    pub const fn close(self) -> Option<ProtocolPositionRequestV2> {
        self.close
    }

    /// EffectProgram route selecting admission, when present.
    pub const fn admit_route(self) -> Option<u16> {
        self.admit_route
    }

    /// EffectProgram route selecting the affine mutation.
    pub const fn affine_route(self) -> u16 {
        self.affine_route
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

    use crate::{
        affine_batch_v2::{
            plan_bytes, AffineBatchPlanInputV2, AffineBatchPositionV2, AffineBatchRowInputV2,
            AffineBatchRowV2, DeltaDirectionV2, SignedMagnitudeV2,
        },
        protocol_position_v2::ProtocolPositionOwnerKindV2,
    };

    use super::*;

    const TAIL_COUNT: u32 = 1;
    const MARKET_REVISION: u64 = 5;

    #[derive(Clone)]
    struct RouteFixture {
        role: u8,
        kind: u8,
        enabled: bool,
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
            request,
        }
    }

    fn effect(routes: &[RouteFixture]) -> (Vec<u8>, Vec<u8>) {
        let route_count = u16::try_from(routes.len()).expect("route count");
        let route_bytes = routes.len().checked_mul(24).expect("route bytes");
        let header = 32_usize.checked_add(route_bytes).expect("header");
        let templates = routes.iter().try_fold(0_usize, |total, route| {
            total.checked_add(route.request.len())
        });
        let mut bytes = vec![0; header + templates.expect("template bytes")];
        put(&mut bytes, 0, b"DCE3");
        put(&mut bytes, 4, &[3, 0]);
        put(&mut bytes, 6, &route_count.to_le_bytes());
        put(&mut bytes, 12, &1_u16.to_le_bytes());
        put(&mut bytes, 14, &1_u16.to_le_bytes());
        put(&mut bytes, 16, &1_u16.to_le_bytes());
        put(&mut bytes, 20, &1_u16.to_le_bytes());
        let mut template_offset = header;
        for (index, route) in routes.iter().enumerate() {
            let offset = 32_usize
                .checked_add(index.checked_mul(24).expect("route offset"))
                .expect("route offset");
            put(&mut bytes, offset, &[route.role]);
            put(&mut bytes, offset + 1, &[route.kind]);
            put(&mut bytes, offset + 2, &[u8::from(route.enabled)]);
            put(&mut bytes, offset + 8, &1_u16.to_le_bytes());
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
        assert_eq!(composition.affine().position_count(), 2);
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
}
