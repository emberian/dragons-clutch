//! Canonical host projection of ordinary Direct child requests.
//!
//! Route addresses depend on hashes of the exact child requests. Those bytes
//! are therefore projected by the selected Transition and Effect programs,
//! never rebuilt by an exterior caller. AccountProfile supplies the alias,
//! permission, and data geometry which the Effect kernel requires.

extern crate alloc;

use alloc::vec;
use dclutch_account_profile_contract::v2::{
    AccountProfileV2, PhysicalAccountDataGeometryV2, derive_effect_permissions,
};
use dclutch_claims_svm::sparse_native_transfer_v1::SPARSE_NATIVE_TRANSFER_BYTES_V1;
use dclutch_custody_contract::DELEGATED_CUSTODY_REQUEST_BYTES_V2;
use dclutch_effect_kernel::{
    v2::{AccountInput, AccountPermission, FixedRole},
    v3::{ProjectionV3, RouteKindV3},
    v4::{ProgramV4, ResolvedWriteRangeV4, project_atomic},
};
use dclutch_transition_vm::v3::{ProgramV3, RegisterInput, RegisterOutput, execute_fold_atomic};

use crate::{
    execution_v3::DirectInlineOrdinaryRequestV3,
    inline_candidate_v2::{
        DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2, DIRECT_INLINE_CUSTODY_ROUTE_SLOTS_V2,
        DIRECT_INLINE_ORDINARY_REQUEST_BANK_BYTES_V3, DirectInlineEffectDispatchV2,
    },
    ordinary_v3::{
        DIRECT_ORDINARY_COMMON_IDENTITIES_V3, DIRECT_ORDINARY_COMMON_SCALARS_V3,
        DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3, DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3,
        DirectOrdinaryAuthenticatedContextV3, project_direct_ordinary_registers_v3,
    },
};

/// Exact projected requests in ordinary Effect route order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineOrdinaryChildRequestsV3 {
    /// Claims sparse-native-transfer request.
    pub claims: [u8; SPARSE_NATIVE_TRANSFER_BYTES_V1],
    /// Four delegated Custody requests: seller terminal, seller intermediate,
    /// fee continuation, then fee sole.
    pub custody: [[u8; DELEGATED_CUSTODY_REQUEST_BYTES_V2]; 4],
    /// Exact ordered enabled Custody child partition resolved from the Effect.
    pub dispatch: DirectInlineEffectDispatchV2,
}

/// Stable refusal from canonical child-request projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectInlineOrdinaryChildProjectionErrorV3 {
    /// Register widths or authenticated context refused.
    Registers,
    /// The selected Transition program refused.
    Transition,
    /// The selected AccountProfile refused.
    Profile,
    /// The selected Effect program or its exact request partition refused.
    Effect,
}

/// Project exact child requests through authenticated Transition/Effect bytes.
pub fn project_direct_inline_ordinary_child_requests_v3(
    request: DirectInlineOrdinaryRequestV3,
    context: DirectOrdinaryAuthenticatedContextV3,
    account_profile_bytes: &[u8],
    transition_bytes: &[u8],
    effect_bytes: &[u8],
) -> Result<DirectInlineOrdinaryChildRequestsV3, DirectInlineOrdinaryChildProjectionErrorV3> {
    let outcome_count = context.outcome_count;
    let tail = usize::try_from(outcome_count)
        .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Registers)?;
    let scalar_width = DIRECT_ORDINARY_COMMON_SCALARS_V3
        .checked_add(
            tail.checked_mul(usize::from(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3))
                .ok_or(DirectInlineOrdinaryChildProjectionErrorV3::Registers)?,
        )
        .ok_or(DirectInlineOrdinaryChildProjectionErrorV3::Registers)?;
    let identity_width = DIRECT_ORDINARY_COMMON_IDENTITIES_V3
        .checked_add(
            tail.checked_mul(usize::from(DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3))
                .ok_or(DirectInlineOrdinaryChildProjectionErrorV3::Registers)?,
        )
        .ok_or(DirectInlineOrdinaryChildProjectionErrorV3::Registers)?;
    let mut register_scratch_scalars = vec![0_u64; scalar_width];
    let mut register_scratch_identities = vec![[0_u8; 32]; identity_width];
    let mut input_scalars = vec![0_u64; scalar_width];
    let mut input_identities = vec![[0_u8; 32]; identity_width];
    project_direct_ordinary_registers_v3(
        request,
        context,
        &mut register_scratch_scalars,
        &mut register_scratch_identities,
        &mut input_scalars,
        &mut input_identities,
    )
    .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Registers)?;

    let transition = ProgramV3::decode(transition_bytes)
        .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Transition)?;
    let mut transition_scratch_scalars = vec![0_u64; scalar_width];
    let mut transition_scratch_identities = vec![[0_u8; 32]; identity_width];
    let mut output_scalars = vec![0_u64; scalar_width];
    let mut output_identities = vec![[0_u8; 32]; identity_width];
    execute_fold_atomic(
        transition,
        outcome_count,
        RegisterInput {
            scalars: &input_scalars,
            identities: &input_identities,
        },
        RegisterOutput {
            scalars: &mut transition_scratch_scalars,
            identities: &mut transition_scratch_identities,
        },
        RegisterOutput {
            scalars: &mut output_scalars,
            identities: &mut output_identities,
        },
    )
    .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Transition)?;

    let profile = AccountProfileV2::decode(account_profile_bytes)
        .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Profile)?;
    let effect = ProgramV4::decode(effect_bytes)
        .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Effect)?;
    let account_count = effect
        .account_count(outcome_count, &output_scalars)
        .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Effect)?;
    if profile
        .logical_account_count(outcome_count)
        .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Profile)?
        != account_count
    {
        return Err(DirectInlineOrdinaryChildProjectionErrorV3::Profile);
    }
    let mut aliases = vec![0_usize; account_count];
    for (coordinate, representative) in aliases.iter_mut().enumerate() {
        *representative = profile
            .representative(outcome_count, coordinate)
            .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Profile)?;
    }
    let mut permissions = vec![AccountPermission::read_only(); account_count];
    derive_effect_permissions(profile, outcome_count, &mut permissions)
        .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Profile)?;
    let mut accounts = vec![
        AccountInput {
            lamports: 1_000_000_000_000,
            data_len: 0,
        };
        account_count
    ];
    for (coordinate, account) in accounts.iter_mut().enumerate() {
        let ordinal = profile
            .physical_account_ordinal(outcome_count, coordinate)
            .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Profile)?;
        let geometry = profile
            .physical_account_geometry_with_dynamic_spans(outcome_count, &[], ordinal)
            .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Profile)?;
        account.data_len = match geometry.data() {
            PhysicalAccountDataGeometryV2::Exact { bytes }
            | PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes: bytes } => bytes,
            PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
                minimum_bytes
                    .checked_add(1)
                    .ok_or(DirectInlineOrdinaryChildProjectionErrorV3::Profile)?
            }
            PhysicalAccountDataGeometryV2::Opaque => 0,
        };
    }
    let mut scratch_lamports = vec![0_u64; account_count];
    let mut output_lamports = vec![0_u64; account_count];
    let request_bytes = effect
        .base()
        .request_bytes(outcome_count)
        .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Effect)?;
    if request_bytes != DIRECT_INLINE_ORDINARY_REQUEST_BANK_BYTES_V3 {
        return Err(DirectInlineOrdinaryChildProjectionErrorV3::Effect);
    }
    let mut requests = vec![0_u8; request_bytes];
    let write_count = effect
        .data_write_operation_count(outcome_count)
        .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Effect)?;
    let mut write_ranges = vec![ResolvedWriteRangeV4::vacant(); write_count];
    project_atomic(
        effect,
        outcome_count,
        ProjectionV3 {
            scalars: &output_scalars,
            identities: &output_identities,
            aliases: &aliases,
            accounts: &accounts,
            permissions: &permissions,
            scratch_lamports: &mut scratch_lamports,
            output_lamports: &mut output_lamports,
            requests: &mut requests,
        },
        &mut write_ranges,
    )
    .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Effect)?;

    let claims_end = SPARSE_NATIVE_TRANSFER_BYTES_V1;
    let claims = requests
        .get(..claims_end)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(DirectInlineOrdinaryChildProjectionErrorV3::Effect)?;
    let mut custody = [[0_u8; DELEGATED_CUSTODY_REQUEST_BYTES_V2]; 4];
    let mut cursor = claims_end;
    for output in &mut custody {
        let end = cursor
            .checked_add(DELEGATED_CUSTODY_REQUEST_BYTES_V2)
            .ok_or(DirectInlineOrdinaryChildProjectionErrorV3::Effect)?;
        *output = requests
            .get(cursor..end)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(DirectInlineOrdinaryChildProjectionErrorV3::Effect)?;
        cursor = end;
    }
    if cursor != requests.len() {
        return Err(DirectInlineOrdinaryChildProjectionErrorV3::Effect);
    }
    let base = effect.base();
    if base.route_count() != 5
        || base.route(0).map(|route| (route.role(), route.kind()))
            != Ok((FixedRole::Claims, RouteKindV3::Once))
        || base.invocation_count(0, outcome_count, &output_scalars, &output_identities) != Ok(1)
    {
        return Err(DirectInlineOrdinaryChildProjectionErrorV3::Effect);
    }
    let mut custody_slots = [0_u8; DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2];
    let mut custody_count = 0_usize;
    let mut child_dispatch_writable = [false; DIRECT_INLINE_CUSTODY_ROUTE_SLOTS_V2];
    for slot in 0..DIRECT_INLINE_CUSTODY_ROUTE_SLOTS_V2 {
        let route_index = u16::try_from(slot)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(DirectInlineOrdinaryChildProjectionErrorV3::Effect)?;
        let route = base
            .route(route_index)
            .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Effect)?;
        if route.role() != FixedRole::Custody || route.kind() != RouteKindV3::Once {
            return Err(DirectInlineOrdinaryChildProjectionErrorV3::Effect);
        }
        let invocation_count = base
            .invocation_count(
                route_index,
                outcome_count,
                &output_scalars,
                &output_identities,
            )
            .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Effect)?;
        match invocation_count {
            0 => {}
            1 => {
                let target = custody_slots
                    .get_mut(custody_count)
                    .ok_or(DirectInlineOrdinaryChildProjectionErrorV3::Effect)?;
                *target = u8::try_from(slot)
                    .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Effect)?;
                *child_dispatch_writable
                    .get_mut(slot)
                    .ok_or(DirectInlineOrdinaryChildProjectionErrorV3::Effect)? = true;
                custody_count = custody_count
                    .checked_add(1)
                    .ok_or(DirectInlineOrdinaryChildProjectionErrorV3::Effect)?;
            }
            _ => return Err(DirectInlineOrdinaryChildProjectionErrorV3::Effect),
        }
    }
    Ok(DirectInlineOrdinaryChildRequestsV3 {
        claims,
        custody,
        dispatch: DirectInlineEffectDispatchV2 {
            custody_slots,
            custody_count: u8::try_from(custody_count)
                .map_err(|_| DirectInlineOrdinaryChildProjectionErrorV3::Effect)?,
            child_dispatch_writable,
        },
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    extern crate std;

    use dclutch_claims_svm::sparse_native_transfer_v1::SparseNativeTransferV1;
    use dclutch_custody_contract::DelegatedCustodyRequestV2;
    use dclutch_sha256_adapter::digest;

    use super::*;
    use crate::{
        execution_v3::{DirectInlineOrdinaryRequestV3, DirectSignedParticipantV3},
        intent_v2::CompactIntentV2,
        ordinary_bundle_v4::tests::canonical_bundle_for_cross_module_tests,
        successor::{DIRECT_MAX_FEE_BASIS_POINTS_V1, DirectExecutionConfigV1},
    };

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn request() -> DirectInlineOrdinaryRequestV3 {
        DirectInlineOrdinaryRequestV3 {
            seller: DirectSignedParticipantV3 {
                maker: id(2),
                intent: CompactIntentV2 {
                    side: 0,
                    lifecycle: 1,
                    outcome: 2,
                    market: id(1),
                    generation: 7,
                    nonce: 4,
                    valid_from: 10,
                    valid_through: 30,
                    maximum_fill: 50,
                    limit_price: 40,
                    fee_basis_points: 500,
                    collateral_account: id(20),
                },
            },
            buyer: DirectSignedParticipantV3 {
                maker: id(3),
                intent: CompactIntentV2 {
                    side: 1,
                    lifecycle: 1,
                    outcome: 2,
                    market: id(1),
                    generation: 7,
                    nonce: 9,
                    valid_from: 5,
                    valid_through: 40,
                    maximum_fill: 50,
                    limit_price: 60,
                    fee_basis_points: 500,
                    collateral_account: id(21),
                },
            },
            // Forty, not twenty: five percent of a gross of ten floors to
            // nothing per side, and a fee-bearing fixture with no fee tests
            // the zero-fee route twice.
            fill: 40,
            execution_price: 50,
        }
    }

    fn context() -> DirectOrdinaryAuthenticatedContextV3 {
        let config = DirectExecutionConfigV1::new(100, 500, id(7)).expect("config");
        DirectOrdinaryAuthenticatedContextV3 {
            parent_request_digest: id(30),
            config_content_id: digest(&config.encode()),
            config,
            market: id(1),
            generation: 7,
            outcome_count: 3,
            slot: 20,
            root_phase: 0,
            seller_next_nonce: 4,
            buyer_next_nonce: 9,
            root_open_maker_count: 2,
            seller_created: false,
            seller_bump_observation: 1,
            seller_bump: 1,
            seller_rent_principal_observation: 100,
            seller_rent_principal: 100,
            buyer_created: false,
            buyer_bump_observation: 2,
            buyer_bump: 2,
            buyer_rent_principal_observation: 100,
            buyer_rent_principal: 100,
            claims_market_revision: 11,
            seller_position_revision: 12,
            buyer_position_revision: 13,
            custody_revision: 14,
            release_set: id(31),
            product_record_digest: id(32),
            semantic_basis: id(33),
            linked_basis_record_digest: id(34),
            trading_program: id(35),
            realm: id(38),
            mint: id(39),
            token_program: id(40),
            seller_maker_root: id(42),
            buyer_maker_root: id(43),
            system_program: [0; 32],
            custody_authority: id(45),
            seller_rent_beneficiary: id(71),
            seller_rent_beneficiary_observation: id(71),
            buyer_rent_beneficiary: id(72),
            buyer_rent_beneficiary_observation: id(72),
            fee_token_account: id(48),
            seller_token_account: id(20),
            buyer_token_account: id(21),
            seller_native_signer: id(2),
            buyer_native_signer: id(3),
        }
    }

    #[test]
    fn canonical_projection_emits_the_seller_leg_alone() {
        let bundle = canonical_bundle_for_cross_module_tests();
        let projected = project_direct_inline_ordinary_child_requests_v3(
            request(),
            context(),
            &bundle.account_profile,
            &bundle.transition,
            &bundle.effect,
        )
        .expect("child projection");
        SparseNativeTransferV1::decode(&projected.claims).expect("Claims request");
        // A fee-bearing fill. The Effect selects the NON-terminal seller route
        // and nothing else: the fee continuation is a second transaction and
        // `FeeSole` is retired, so slots 0, 2 and 3 stay inactive projection
        // bytes that are not child requests.
        assert!(DelegatedCustodyRequestV2::decode(&projected.custody[0]).is_err());
        let seller = DelegatedCustodyRequestV2::decode(&projected.custody[1])
            .expect("seller-intermediate Custody request");
        assert!(DelegatedCustodyRequestV2::decode(&projected.custody[2]).is_err());
        assert!(DelegatedCustodyRequestV2::decode(&projected.custody[3]).is_err());
        assert_eq!(
            projected.dispatch,
            DirectInlineEffectDispatchV2 {
                custody_slots: [1],
                custody_count: 1,
                child_dispatch_writable: [false, true, false, false],
            }
        );
        // The obligation, visible on the wire the fee transaction will spend:
        // the leg is non-terminal, the delegate survives it, and what is left
        // standing is exactly the combined fee. Fill 20 at 50 of 100 is a gross
        // of 10; five percent of ten floors to nothing per side, so raise the
        // fill until it does not.
        assert!(!seller.terminal);
        assert_eq!(seller.delegate_after, seller.delegate_before);
        assert_eq!(
            seller.allowance_before - seller.custody.amount,
            seller.allowance_after
        );
        assert_eq!(seller.allowance_after, 2);
        assert_eq!(seller.custody.amount, 19);
    }

    #[test]
    fn a_zero_fee_fill_closes_the_delegation_and_a_fee_bearing_one_does_not() {
        let bundle = canonical_bundle_for_cross_module_tests();
        // Zero bps takes the terminal seller-only route and closes the
        // delegation; the band's own edge takes the non-terminal one and leaves
        // the fee standing. No rate between them reaches slot 2 or slot 3.
        for (fee_basis_points, expected_slot, terminal) in [
            (0_u16, 0_u8, true),
            (DIRECT_MAX_FEE_BASIS_POINTS_V1, 1_u8, false),
        ] {
            let mut request = request();
            request.seller.intent.fee_basis_points = fee_basis_points;
            request.buyer.intent.fee_basis_points = fee_basis_points;
            let mut context = context();
            context.config =
                DirectExecutionConfigV1::new(100, fee_basis_points, id(7)).expect("config");
            context.config_content_id = digest(&context.config.encode());
            let projected = project_direct_inline_ordinary_child_requests_v3(
                request,
                context,
                &bundle.account_profile,
                &bundle.transition,
                &bundle.effect,
            )
            .expect("terminal partition");
            let mut writable = [false; DIRECT_INLINE_CUSTODY_ROUTE_SLOTS_V2];
            writable[usize::from(expected_slot)] = true;
            assert_eq!(
                projected.dispatch,
                DirectInlineEffectDispatchV2 {
                    custody_slots: [expected_slot],
                    custody_count: 1,
                    child_dispatch_writable: writable,
                }
            );
            for (slot, bytes) in projected.custody.iter().enumerate() {
                let decoded = DelegatedCustodyRequestV2::decode(bytes);
                assert_eq!(decoded.is_ok(), slot == usize::from(expected_slot));
                if let Ok(request) = decoded {
                    assert_eq!(request.terminal, terminal);
                    assert_eq!(request.allowance_after == 0, terminal);
                }
            }
        }
    }

    #[test]
    fn selected_profile_transition_and_effect_bytes_are_hostile_checked() {
        let bundle = canonical_bundle_for_cross_module_tests();
        let mut profile = bundle.account_profile;
        profile[0] ^= 1;
        assert_eq!(
            project_direct_inline_ordinary_child_requests_v3(
                request(),
                context(),
                &profile,
                &bundle.transition,
                &bundle.effect,
            ),
            Err(DirectInlineOrdinaryChildProjectionErrorV3::Profile)
        );

        let mut transition = bundle.transition;
        transition[0] ^= 1;
        assert_eq!(
            project_direct_inline_ordinary_child_requests_v3(
                request(),
                context(),
                &bundle.account_profile,
                &transition,
                &bundle.effect,
            ),
            Err(DirectInlineOrdinaryChildProjectionErrorV3::Transition)
        );

        let mut effect = bundle.effect;
        effect[0] ^= 1;
        assert_eq!(
            project_direct_inline_ordinary_child_requests_v3(
                request(),
                context(),
                &bundle.account_profile,
                &bundle.transition,
                &effect,
            ),
            Err(DirectInlineOrdinaryChildProjectionErrorV3::Effect)
        );
    }
}
