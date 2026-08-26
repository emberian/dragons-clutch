//! Typed EffectProgram V3 generator for Dealer junior-equity execution.
//!
//! The generator emits data for the family-neutral Hot interpreter. It owns
//! no runtime authority: RequestProfile/Transition artifacts must populate the
//! documented register ABI from authenticated accounts and the exact signed
//! Dealer request. EffectProgram then constructs canonical Custody V1 packets,
//! borrows the complete SignedDeltaV3 suffix, and writes only the two
//! Trading-owned optimistic state fields after child execution succeeds.

#[cfg(not(target_os = "solana"))]
extern crate alloc;

#[cfg(not(target_os = "solana"))]
use alloc::vec::Vec;

use dclutch_claims_svm::signed_delta_v3::SIGNED_DELTA_RECEIPT_BYTES_V3;
use dclutch_custody_contract::{
    CUSTODY_REQUEST_BYTES_V1, CallerRoleV1, CompartmentV1, CustodyRequestV1, OperationV1,
};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{
        HEADER_BYTES, OPERATION_BYTES, ROUTE_BYTES, RouteKindV3, RouteReceiptDependencyV3,
        encode::{
            AccountCoordinateV3, EffectGeometryV3, EffectInstructionV3, IdentityCoordinateV3,
            RequestSpaceV3, RouteInputV3, ScalarCoordinateV3, encode_effect_program_v3_atomic,
        },
    },
};

use super::{v3_equity_operator::DEALER_EQUITY_HEADER_BYTES_V3, v3_multi_lp::MultiLpActionV3};

/// Logical Hot coordinates injected by the common outer before family suffix:
/// root, config, Product root, portfolio, and linked liability basis.
pub const DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3: u16 = 5;
/// Exact canonical Custody transfer frame.
pub const DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3: u16 = 14;
/// Exact canonical SignedDelta frame before its Position tail.
pub const DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3: u16 = 20;
/// Trading-owned state accounts committed after all child routes.
pub const DEALER_EQUITY_LOCAL_ACCOUNT_COUNT_V3: u16 = 2;

/// Transition register holding next obligation revision.
pub const DEALER_EQUITY_OBLIGATION_REVISION_SCALAR_V3: u16 = 0;
/// Transition register holding next total junior-share supply.
pub const DEALER_EQUITY_TOTAL_SHARES_SCALAR_V3: u16 = 1;
/// Transition register holding next LP Position revision.
pub const DEALER_EQUITY_LP_REVISION_SCALAR_V3: u16 = 2;
/// Transition register holding next LP junior-share balance.
pub const DEALER_EQUITY_LP_SHARES_SCALAR_V3: u16 = 3;
/// Signed family-prefix width; canonical value is 480.
pub const DEALER_EQUITY_WITNESS_OFFSET_SCALAR_V3: u16 = 4;
/// Exact SignedDelta suffix width and Claims-route enable scalar.
pub const DEALER_EQUITY_WITNESS_BYTES_SCALAR_V3: u16 = 5;
/// Hot-injected parent request digest identity.
pub const DEALER_EQUITY_PARENT_REQUEST_DIGEST_IDENTITY_V3: u16 = 0;

const CUSTODY_SCALAR_BASE_V3: u16 = 6;
const CUSTODY_SCALAR_STRIDE_V3: u16 = 9;
const CUSTODY_IDENTITY_BASE_V3: u16 = 1;
const CUSTODY_IDENTITY_STRIDE_V3: u16 = 17;
const CUSTODY_IDENTITY_FIELD_COUNT_V3: usize = 17;
const OBLIGATION_REVISION_OFFSET_V3: u32 = 16;
const OBLIGATION_TOTAL_SHARES_OFFSET_V3: u32 = 184;
const LP_REVISION_OFFSET_V3: u32 = 16;
const LP_SHARES_OFFSET_V3: u32 = 216;

// Canonical generated Custody V1 wire offsets. Every generated artifact is
// round-tripped through CustodyRequestV1 in tests, so ABI drift refuses rather
// than silently moving a patch to another field.
const CUSTODY_TRANSFER_INDEX_OFFSET_V1: u32 = 14;
const CUSTODY_PARENT_DIGEST_OFFSET_V1: u32 = 304;
const CUSTODY_EXPECTED_REVISION_OFFSET_V1: u32 = 592;
const CUSTODY_RESULTING_REVISION_OFFSET_V1: u32 = 600;
const CUSTODY_ORDER_NONCE_OFFSET_V1: u32 = 608;
const CUSTODY_GENERATION_OFFSET_V1: u32 = 616;
const CUSTODY_AMOUNT_OFFSET_V1: u32 = 624;
const CUSTODY_RENT_LAMPORTS_OFFSET_V1: u32 = 632;
const CUSTODY_PAGE_INDEX_OFFSET_V1: u32 = 640;
const CUSTODY_EXECUTION_INDEX_OFFSET_V1: u32 = 644;
const CUSTODY_IDENTITY_OFFSETS_V1: [u32; CUSTODY_IDENTITY_FIELD_COUNT_V3] = [
    16, 48, 80, 112, 144, 176, 208, 240, 272, 336, 368, 400, 432, 464, 496, 528, 560,
];

/// Stable artifact construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerEquityArtifactErrorV3 {
    /// Action, Position-frame width, or template count differed.
    Geometry,
    /// A Custody template was not the exact static transfer kind for its route.
    CustodyTemplate,
    /// Checked byte/register/account arithmetic overflowed.
    Arithmetic,
    /// The family-neutral EffectProgram encoder refused the complete artifact.
    EffectProgram,
}

/// Dynamic scalar field within one canonical Custody request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerCustodyScalarFieldV3 {
    /// Ordered active transfer coordinate.
    TransferIndex,
    /// Optimistic replay revision.
    ExpectedRevision,
    /// Required next replay revision.
    ResultingRevision,
    /// Caller semantic order nonce.
    OrderNonce,
    /// Core Market generation.
    Generation,
    /// Positive raw collateral atoms and route enable.
    Amount,
    /// Exact lamport rent movement.
    RentLamports,
    /// Caller semantic page coordinate.
    PageIndex,
    /// Caller semantic execution coordinate.
    ExecutionIndex,
}

/// Dynamic identity field within one canonical Custody request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerCustodyIdentityFieldV3 {
    /// Release-set content identity.
    ReleaseSet,
    /// Core Market.
    Market,
    /// Immutable Realm.
    Realm,
    /// Replay context.
    Context,
    /// Current Trading program.
    CallerProgram,
    /// Candidate coordinate.
    Candidate,
    /// Semantic source owner.
    SourceOwner,
    /// Semantic destination owner.
    DestinationOwner,
    /// Semantic order identity.
    Order,
    /// Exact source token account.
    Source,
    /// Exact destination token account.
    Destination,
    /// Source Custody vault namespace.
    SourceVaultContext,
    /// Destination Custody vault namespace.
    DestinationVaultContext,
    /// Realm-selected collateral mint.
    Mint,
    /// Realm-selected token program.
    TokenProgram,
    /// Optional rent payer.
    Payer,
    /// Optional rent refund beneficiary.
    RentRefund,
}

/// Return the exact common scalar register for one Custody slot/field.
pub fn dealer_custody_scalar_register_v3(
    slot: u16,
    field: DealerCustodyScalarFieldV3,
) -> Option<u16> {
    let field = match field {
        DealerCustodyScalarFieldV3::TransferIndex => 0,
        DealerCustodyScalarFieldV3::ExpectedRevision => 1,
        DealerCustodyScalarFieldV3::ResultingRevision => 2,
        DealerCustodyScalarFieldV3::OrderNonce => 3,
        DealerCustodyScalarFieldV3::Generation => 4,
        DealerCustodyScalarFieldV3::Amount => 5,
        DealerCustodyScalarFieldV3::RentLamports => 6,
        DealerCustodyScalarFieldV3::PageIndex => 7,
        DealerCustodyScalarFieldV3::ExecutionIndex => 8,
    };
    slot
        .checked_mul(CUSTODY_SCALAR_STRIDE_V3)
        .and_then(|offset| CUSTODY_SCALAR_BASE_V3.checked_add(offset))
        .and_then(|base| base.checked_add(field))
}

/// Return the exact common identity register for one Custody slot/field.
pub fn dealer_custody_identity_register_v3(
    slot: u16,
    field: DealerCustodyIdentityFieldV3,
) -> Option<u16> {
    let field = match field {
        DealerCustodyIdentityFieldV3::ReleaseSet => 0,
        DealerCustodyIdentityFieldV3::Market => 1,
        DealerCustodyIdentityFieldV3::Realm => 2,
        DealerCustodyIdentityFieldV3::Context => 3,
        DealerCustodyIdentityFieldV3::CallerProgram => 4,
        DealerCustodyIdentityFieldV3::Candidate => 5,
        DealerCustodyIdentityFieldV3::SourceOwner => 6,
        DealerCustodyIdentityFieldV3::DestinationOwner => 7,
        DealerCustodyIdentityFieldV3::Order => 8,
        DealerCustodyIdentityFieldV3::Source => 9,
        DealerCustodyIdentityFieldV3::Destination => 10,
        DealerCustodyIdentityFieldV3::SourceVaultContext => 11,
        DealerCustodyIdentityFieldV3::DestinationVaultContext => 12,
        DealerCustodyIdentityFieldV3::Mint => 13,
        DealerCustodyIdentityFieldV3::TokenProgram => 14,
        DealerCustodyIdentityFieldV3::Payer => 15,
        DealerCustodyIdentityFieldV3::RentRefund => 16,
    };
    slot
        .checked_mul(CUSTODY_IDENTITY_STRIDE_V3)
        .and_then(|offset| CUSTODY_IDENTITY_BASE_V3.checked_add(offset))
        .and_then(|base| base.checked_add(field))
}

/// Exact encoded EffectProgram width for one action-specific P0/P1/P2 shape.
pub fn dealer_equity_effect_program_bytes_v3(
    action: MultiLpActionV3,
) -> Result<usize, DealerEquityArtifactErrorV3> {
    let slots = custody_slot_count(action);
    let routes = slots
        .checked_add(1)
        .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
    let operations = slots
        .checked_mul(27)
        .and_then(|value| value.checked_add(4))
        .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
    HEADER_BYTES
        .checked_add(
            routes
                .checked_mul(ROUTE_BYTES)
                .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?,
        )
        .and_then(|value| value.checked_add(operations.checked_mul(OPERATION_BYTES)?))
        .and_then(|value| value.checked_add(slots.checked_mul(CUSTODY_REQUEST_BYTES_V1)?))
        .ok_or(DealerEquityArtifactErrorV3::Arithmetic)
}

/// Emit one complete action/P-specific EffectProgram into caller-owned bytes.
///
/// Templates contribute only their canonical static operation, caller role,
/// compartment pair, magic, and version. Every request-owned identity/scalar
/// is overwritten from the documented register bank before child execution.
#[cfg(not(target_os = "solana"))]
pub fn encode_dealer_equity_effect_program_v3(
    action: MultiLpActionV3,
    signed_position_count: u32,
    custody_templates: &[CustodyRequestV1],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerEquityArtifactErrorV3> {
    let slots = custody_slot_count(action);
    if custody_templates.len() != slots || signed_position_count > 2 {
        return Err(DealerEquityArtifactErrorV3::Geometry);
    }
    let expected = dealer_equity_effect_program_bytes_v3(action)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(DealerEquityArtifactErrorV3::Geometry);
    }
    let claims_accounts = DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3
        .checked_add(
            u16::try_from(signed_position_count)
                .map_err(|_| DealerEquityArtifactErrorV3::Arithmetic)?,
        )
        .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
    let mut templates = Vec::with_capacity(slots);
    for (slot, template) in custody_templates.iter().copied().enumerate() {
        validate_template(action, slot, template)?;
        templates.push(
            template
                .to_bytes()
                .map_err(|_| DealerEquityArtifactErrorV3::CustodyTemplate)?,
        );
    }
    let mut routes = Vec::with_capacity(slots.saturating_add(1));
    let mut account_start = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3;
    for route_index in 0..slots.saturating_add(1) {
        if route_index == 1 {
            routes.push(RouteInputV3 {
                role: FixedRole::Claims,
                kind: RouteKindV3::Once,
                enable_common_scalar: Some(DEALER_EQUITY_WITNESS_BYTES_SCALAR_V3),
                witness_range_common_scalar: Some(DEALER_EQUITY_WITNESS_OFFSET_SCALAR_V3),
                receipt_dependency: None,
                fixed_account_start: account_start,
                fixed_account_count: claims_accounts,
                item_account_start: 0,
                item_account_count: 0,
                fixed_request: &[],
                item_request: &[],
            });
            account_start = account_start
                .checked_add(claims_accounts)
                .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
            continue;
        }
        let slot = if route_index == 0 {
            0
        } else {
            route_index
                .checked_sub(1)
                .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?
        };
        let slot_u16 = u16::try_from(slot).map_err(|_| DealerEquityArtifactErrorV3::Arithmetic)?;
        let receipt_dependency = if route_index > 1 && signed_position_count != 0 {
            Some(RouteReceiptDependencyV3::new(
                FixedRole::Claims,
                1,
                u16::try_from(SIGNED_DELTA_RECEIPT_BYTES_V3)
                    .map_err(|_| DealerEquityArtifactErrorV3::Arithmetic)?,
            ))
        } else {
            None
        };
        routes.push(RouteInputV3 {
            role: FixedRole::Custody,
            kind: RouteKindV3::Once,
            enable_common_scalar: Some(custody_scalar(
                slot_u16,
                DealerCustodyScalarFieldV3::Amount,
            )?),
            witness_range_common_scalar: None,
            receipt_dependency,
            fixed_account_start: account_start,
            fixed_account_count: DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3,
            item_account_start: 0,
            item_account_count: 0,
            fixed_request: templates
                .get(slot)
                .map(|value| value.as_slice())
                .ok_or(DealerEquityArtifactErrorV3::Geometry)?,
            item_request: &[],
        });
        account_start = account_start
            .checked_add(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3)
            .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
    }
    let obligation_account = account_start;
    let lp_account = obligation_account
        .checked_add(1)
        .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
    let fixed_accounts = lp_account
        .checked_add(1)
        .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
    let mut instructions = Vec::with_capacity(slots.saturating_mul(27).saturating_add(4));
    instructions.extend_from_slice(&[
        EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(obligation_account),
            OBLIGATION_REVISION_OFFSET_V3,
            ScalarCoordinateV3::common(DEALER_EQUITY_OBLIGATION_REVISION_SCALAR_V3),
        ),
        EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(obligation_account),
            OBLIGATION_TOTAL_SHARES_OFFSET_V3,
            ScalarCoordinateV3::common(DEALER_EQUITY_TOTAL_SHARES_SCALAR_V3),
        ),
        EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(lp_account),
            LP_REVISION_OFFSET_V3,
            ScalarCoordinateV3::common(DEALER_EQUITY_LP_REVISION_SCALAR_V3),
        ),
        EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(lp_account),
            LP_SHARES_OFFSET_V3,
            ScalarCoordinateV3::common(DEALER_EQUITY_LP_SHARES_SCALAR_V3),
        ),
    ]);
    for slot in 0..slots {
        let route = if slot == 0 { 0 } else { slot + 1 };
        push_custody_projection(slot, route, &mut instructions)?;
    }
    let geometry = EffectGeometryV3 {
        fixed_accounts,
        item_account_stride: 0,
        common_scalars: scalar_count(action)?,
        item_scalar_stride: 0,
        common_identities: identity_count(action)?,
        item_identity_stride: 0,
    };
    encode_effect_program_v3_atomic(geometry, &routes, &instructions, &[], scratch, output)
        .map_err(|_| DealerEquityArtifactErrorV3::EffectProgram)
}

#[cfg(not(target_os = "solana"))]
fn push_custody_projection(
    slot: usize,
    route: usize,
    output: &mut Vec<EffectInstructionV3>,
) -> Result<(), DealerEquityArtifactErrorV3> {
    let slot = u16::try_from(slot).map_err(|_| DealerEquityArtifactErrorV3::Arithmetic)?;
    let route = u16::try_from(route).map_err(|_| DealerEquityArtifactErrorV3::Arithmetic)?;
    output.push(EffectInstructionV3::write_request_u16(
        route,
        RequestSpaceV3::Fixed,
        CUSTODY_TRANSFER_INDEX_OFFSET_V1,
        ScalarCoordinateV3::common(custody_scalar(
            slot,
            DealerCustodyScalarFieldV3::TransferIndex,
        )?),
    ));
    output.push(EffectInstructionV3::write_request_identity(
        route,
        RequestSpaceV3::Fixed,
        CUSTODY_PARENT_DIGEST_OFFSET_V1,
        IdentityCoordinateV3::common(DEALER_EQUITY_PARENT_REQUEST_DIGEST_IDENTITY_V3),
    ));
    for (field, offset) in identity_fields()
        .into_iter()
        .zip(CUSTODY_IDENTITY_OFFSETS_V1)
    {
        output.push(EffectInstructionV3::write_request_identity(
            route,
            RequestSpaceV3::Fixed,
            offset,
            IdentityCoordinateV3::common(custody_identity(slot, field)?),
        ));
    }
    for (field, offset) in [
        (
            DealerCustodyScalarFieldV3::ExpectedRevision,
            CUSTODY_EXPECTED_REVISION_OFFSET_V1,
        ),
        (
            DealerCustodyScalarFieldV3::ResultingRevision,
            CUSTODY_RESULTING_REVISION_OFFSET_V1,
        ),
        (
            DealerCustodyScalarFieldV3::OrderNonce,
            CUSTODY_ORDER_NONCE_OFFSET_V1,
        ),
        (
            DealerCustodyScalarFieldV3::Generation,
            CUSTODY_GENERATION_OFFSET_V1,
        ),
        (DealerCustodyScalarFieldV3::Amount, CUSTODY_AMOUNT_OFFSET_V1),
        (
            DealerCustodyScalarFieldV3::RentLamports,
            CUSTODY_RENT_LAMPORTS_OFFSET_V1,
        ),
    ] {
        output.push(EffectInstructionV3::write_request_u64(
            route,
            RequestSpaceV3::Fixed,
            offset,
            ScalarCoordinateV3::common(custody_scalar(slot, field)?),
        ));
    }
    for (field, offset) in [
        (
            DealerCustodyScalarFieldV3::PageIndex,
            CUSTODY_PAGE_INDEX_OFFSET_V1,
        ),
        (
            DealerCustodyScalarFieldV3::ExecutionIndex,
            CUSTODY_EXECUTION_INDEX_OFFSET_V1,
        ),
    ] {
        output.push(EffectInstructionV3::write_request_u32(
            route,
            RequestSpaceV3::Fixed,
            offset,
            ScalarCoordinateV3::common(custody_scalar(slot, field)?),
        ));
    }
    Ok(())
}

fn validate_template(
    action: MultiLpActionV3,
    slot: usize,
    template: CustodyRequestV1,
) -> Result<(), DealerEquityArtifactErrorV3> {
    let expected = match (action, slot) {
        (MultiLpActionV3::Add, 0) => (CompartmentV1::External, CompartmentV1::TradingPrincipal),
        (MultiLpActionV3::Add, 1) => (
            CompartmentV1::HoardPrincipal,
            CompartmentV1::TradingPrincipal,
        ),
        (MultiLpActionV3::Remove, 0) => (
            CompartmentV1::TradingPrincipal,
            CompartmentV1::HoardPrincipal,
        ),
        (MultiLpActionV3::Remove, 1) => (CompartmentV1::TradingPrincipal, CompartmentV1::External),
        (MultiLpActionV3::Remove, 2) => (
            CompartmentV1::HoardPrincipal,
            CompartmentV1::TradingPrincipal,
        ),
        _ => return Err(DealerEquityArtifactErrorV3::Geometry),
    };
    if template.operation != OperationV1::Transfer
        || template.caller_role != CallerRoleV1::Trading
        || (
            template.source_compartment,
            template.destination_compartment,
        ) != expected
    {
        return Err(DealerEquityArtifactErrorV3::CustodyTemplate);
    }
    Ok(())
}

const fn custody_slot_count(action: MultiLpActionV3) -> usize {
    match action {
        MultiLpActionV3::Add => 2,
        MultiLpActionV3::Remove => 3,
    }
}

fn scalar_count(action: MultiLpActionV3) -> Result<u16, DealerEquityArtifactErrorV3> {
    u16::try_from(custody_slot_count(action))
        .ok()
        .and_then(|slots| slots.checked_mul(CUSTODY_SCALAR_STRIDE_V3))
        .and_then(|width| CUSTODY_SCALAR_BASE_V3.checked_add(width))
        .ok_or(DealerEquityArtifactErrorV3::Arithmetic)
}

fn identity_count(action: MultiLpActionV3) -> Result<u16, DealerEquityArtifactErrorV3> {
    u16::try_from(custody_slot_count(action))
        .ok()
        .and_then(|slots| slots.checked_mul(CUSTODY_IDENTITY_STRIDE_V3))
        .and_then(|width| CUSTODY_IDENTITY_BASE_V3.checked_add(width))
        .ok_or(DealerEquityArtifactErrorV3::Arithmetic)
}

fn custody_scalar(
    slot: u16,
    field: DealerCustodyScalarFieldV3,
) -> Result<u16, DealerEquityArtifactErrorV3> {
    dealer_custody_scalar_register_v3(slot, field).ok_or(DealerEquityArtifactErrorV3::Arithmetic)
}

fn custody_identity(
    slot: u16,
    field: DealerCustodyIdentityFieldV3,
) -> Result<u16, DealerEquityArtifactErrorV3> {
    dealer_custody_identity_register_v3(slot, field).ok_or(DealerEquityArtifactErrorV3::Arithmetic)
}

const fn identity_fields() -> [DealerCustodyIdentityFieldV3; CUSTODY_IDENTITY_FIELD_COUNT_V3] {
    [
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
    ]
}

/// Canonical borrowed witness begins at the end of the signed fixed header.
pub const fn dealer_equity_witness_offset_v3() -> usize {
    DEALER_EQUITY_HEADER_BYTES_V3
}

#[cfg(all(test, not(target_os = "solana")))]
mod tests {
    use super::*;
    use dclutch_custody_contract::ContextV1;
    use dclutch_effect_kernel::v3::ProgramV3;
    use std::vec;

    fn transfer_template(
        source_compartment: CompartmentV1,
        destination_compartment: CompartmentV1,
        marker: u8,
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
                parent_request_digest: [10; 32],
                order_nonce: 11,
                generation: 12,
                page_index: 13,
                execution_index: 14,
                transfer_index: u16::from(marker),
            },
            source: [marker; 32],
            destination: [marker.saturating_add(1); 32],
            source_vault_context: if source_external { [0; 32] } else { [15; 32] },
            destination_vault_context: if destination_external {
                [0; 32]
            } else {
                [16; 32]
            },
            mint: [17; 32],
            token_program: [18; 32],
            payer: [0; 32],
            rent_refund: [0; 32],
            expected_revision: 19,
            resulting_revision: 20,
            amount: 21,
            rent_lamports: 0,
        }
    }

    #[test]
    fn typed_p2_contribution_artifact_has_exact_routes_and_dependency() {
        let templates = [
            transfer_template(CompartmentV1::External, CompartmentV1::TradingPrincipal, 22),
            transfer_template(
                CompartmentV1::HoardPrincipal,
                CompartmentV1::TradingPrincipal,
                24,
            ),
        ];
        let width =
            dealer_equity_effect_program_bytes_v3(MultiLpActionV3::Add).expect("artifact width");
        assert_eq!(width, 2_864);
        let mut scratch = vec![0; width];
        let mut output = vec![0; width];
        encode_dealer_equity_effect_program_v3(
            MultiLpActionV3::Add,
            2,
            &templates,
            &mut scratch,
            &mut output,
        )
        .expect("typed artifact");
        let program = ProgramV3::decode(&output).expect("hostile decode");
        assert_eq!(program.route_count(), 3);
        assert_eq!(program.fixed_account_count(), 57);
        assert_eq!(program.common_scalar_count(), 24);
        assert_eq!(program.common_identity_count(), 35);
        assert_eq!(program.fixed_operation_count(), 58);
        assert_eq!(program.item_operation_count(), 0);

        let custody_in = program.route(0).expect("cash route");
        assert_eq!(custody_in.role(), FixedRole::Custody);
        assert_eq!(custody_in.fixed_account_start(), 5);
        assert_eq!(custody_in.fixed_account_count(), 14);
        assert_eq!(custody_in.fixed_request_bytes(), 672);
        assert_eq!(custody_in.receipt_dependency(), None);

        let claims = program.route(1).expect("Claims route");
        assert_eq!(claims.role(), FixedRole::Claims);
        assert_eq!(claims.fixed_account_start(), 19);
        assert_eq!(claims.fixed_account_count(), 22);
        assert!(claims.borrows_witness());
        assert_eq!(claims.fixed_request_bytes(), 0);

        let merge = program.route(2).expect("merge route");
        assert_eq!(merge.role(), FixedRole::Custody);
        assert_eq!(merge.fixed_account_start(), 41);
        let dependency = merge.receipt_dependency().expect("Claims dependency");
        assert_eq!(dependency.producer_role(), FixedRole::Claims);
        assert_eq!(dependency.producer_route(), 1);
        assert_eq!(
            dependency.expected_receipt_bytes(),
            u16::try_from(SIGNED_DELTA_RECEIPT_BYTES_V3).expect("receipt width")
        );
        assert_eq!(
            program.route_template(0).expect("cash template").0,
            templates[0].to_bytes().expect("cash bytes")
        );
        assert_eq!(
            program.route_template(2).expect("merge template").0,
            templates[1].to_bytes().expect("merge bytes")
        );
    }

    #[test]
    fn typed_p0_redemption_retains_all_conditional_custody_slots() {
        let templates = [
            transfer_template(
                CompartmentV1::TradingPrincipal,
                CompartmentV1::HoardPrincipal,
                22,
            ),
            transfer_template(CompartmentV1::TradingPrincipal, CompartmentV1::External, 24),
            transfer_template(
                CompartmentV1::HoardPrincipal,
                CompartmentV1::TradingPrincipal,
                26,
            ),
        ];
        let width =
            dealer_equity_effect_program_bytes_v3(MultiLpActionV3::Remove).expect("artifact width");
        assert_eq!(width, 4_216);
        let mut scratch = vec![0; width];
        let mut output = vec![0; width];
        encode_dealer_equity_effect_program_v3(
            MultiLpActionV3::Remove,
            0,
            &templates,
            &mut scratch,
            &mut output,
        )
        .expect("typed redemption artifact");
        let program = ProgramV3::decode(&output).expect("hostile decode");
        assert_eq!(program.route_count(), 4);
        assert_eq!(program.fixed_account_count(), 69);
        assert_eq!(program.common_scalar_count(), 33);
        assert_eq!(program.common_identity_count(), 52);
        assert_eq!(program.fixed_operation_count(), 85);
        assert_eq!(program.route(0).expect("split").fixed_account_start(), 5);
        assert_eq!(program.route(1).expect("Claims").fixed_account_start(), 19);
        assert_eq!(program.route(1).expect("Claims").fixed_account_count(), 20);
        assert_eq!(
            program.route(2).expect("cash out").fixed_account_start(),
            39
        );
        assert_eq!(program.route(3).expect("merge").fixed_account_start(), 53);
        for route in 0..4 {
            assert_eq!(
                program.route(route).expect("route").receipt_dependency(),
                None
            );
        }
    }

    #[test]
    fn wrong_compartment_or_output_width_refuses_atomically() {
        let wrong = [
            transfer_template(CompartmentV1::TradingPrincipal, CompartmentV1::External, 22),
            transfer_template(
                CompartmentV1::HoardPrincipal,
                CompartmentV1::TradingPrincipal,
                24,
            ),
        ];
        let width =
            dealer_equity_effect_program_bytes_v3(MultiLpActionV3::Add).expect("artifact width");
        let mut scratch = vec![0xa5; width];
        let mut output = vec![0x5a; width];
        assert_eq!(
            encode_dealer_equity_effect_program_v3(
                MultiLpActionV3::Add,
                2,
                &wrong,
                &mut scratch,
                &mut output,
            ),
            Err(DealerEquityArtifactErrorV3::CustodyTemplate)
        );
        assert!(output.iter().all(|byte| *byte == 0x5a));

        let valid = [
            transfer_template(CompartmentV1::External, CompartmentV1::TradingPrincipal, 22),
            transfer_template(
                CompartmentV1::HoardPrincipal,
                CompartmentV1::TradingPrincipal,
                24,
            ),
        ];
        assert_eq!(
            encode_dealer_equity_effect_program_v3(
                MultiLpActionV3::Add,
                2,
                &valid,
                &mut scratch,
                &mut output[..width - 1],
            ),
            Err(DealerEquityArtifactErrorV3::Geometry)
        );
        assert!(output.iter().all(|byte| *byte == 0x5a));
    }
}
