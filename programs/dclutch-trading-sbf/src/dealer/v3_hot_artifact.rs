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
use alloc::{vec, vec::Vec};

use dclutch_capability_program_contract::hot_v3::HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3;
use dclutch_claims_svm::signed_delta_v3::SIGNED_DELTA_RECEIPT_BYTES_V3;
use dclutch_custody_contract::{
    CUSTODY_REQUEST_BYTES_V1, CallerRoleV1, CompartmentV1, CustodyRequestLayoutV1,
    DELEGATED_CUSTODY_REQUEST_BYTES_V2, DelegatedCustodyRequestLayoutV2, OperationV1,
};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{
        HEADER_BYTES, OPERATION_BYTES, RECEIPT_DEPENDENCY_BYTES, ROUTE_BYTES, RouteKindV3,
        RouteReceiptDependencyV3,
        encode::{
            AccountCoordinateV3, EffectGeometryV3, EffectInstructionV3, IdentityCoordinateV3,
            RequestSpaceV3, RouteInputV3, ScalarCoordinateV3, encode_effect_program_v3_atomic,
        },
    },
};
#[cfg(not(target_os = "solana"))]
use solana_program::hash::hash;

use super::{
    v3_equity_operator::{
        DEALER_EQUITY_HEADER_BYTES_V3, DealerEquityRequestV3, EquityRequestActionV3,
    },
    v3_multi_lp::{
        MAX_MULTI_LP_CUSTODY_EFFECTS_V3, MultiLpActionV3, MultiLpCustodyEffectV3,
        MultiLpCustodyRequestV3, MultiLpPlanV3, multi_lp_custody_digest_v3,
    },
};

/// Logical Hot coordinates injected by the common outer before family suffix:
/// root, config, Product root, portfolio, and linked liability basis.
pub const DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3: u16 = 5;
const _: () =
    assert!(DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 as usize == HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3);
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

/// Stable artifact construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerEquityArtifactErrorV3 {
    /// Action, Position-frame width, or template count differed.
    Geometry,
    /// A Custody template was not the exact static transfer kind for its route.
    CustodyTemplate,
    /// Signed request, semantic plan, or register facts did not rejoin exactly.
    Projection,
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
    slot.checked_mul(CUSTODY_SCALAR_STRIDE_V3)
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
    slot.checked_mul(CUSTODY_IDENTITY_STRIDE_V3)
        .and_then(|offset| CUSTODY_IDENTITY_BASE_V3.checked_add(offset))
        .and_then(|base| base.checked_add(field))
}

/// Common identity register containing the exact delegated Custody authority.
pub fn dealer_external_delegate_identity_register_v3(action: MultiLpActionV3) -> Option<u16> {
    if action != MultiLpActionV3::Add {
        return None;
    }
    u16::try_from(custody_slot_count(action))
        .ok()
        .and_then(|slots| slots.checked_mul(CUSTODY_IDENTITY_STRIDE_V3))
        .and_then(|width| CUSTODY_IDENTITY_BASE_V3.checked_add(width))
}

/// AccountProfile5 destination for the trusted current slot.
pub fn dealer_current_slot_scalar_register_v3(action: MultiLpActionV3) -> Option<u16> {
    u16::try_from(custody_slot_count(action))
        .ok()
        .and_then(|slots| slots.checked_mul(CUSTODY_SCALAR_STRIDE_V3))
        .and_then(|width| CUSTODY_SCALAR_BASE_V3.checked_add(width))
}

/// RequestProfile destination for the request expiry coordinate.
pub fn dealer_expiry_scalar_register_v3(action: MultiLpActionV3) -> Option<u16> {
    dealer_current_slot_scalar_register_v3(action).and_then(|index| index.checked_add(1))
}

/// Exact scalar register count selected by one equity action.
pub fn dealer_equity_scalar_count_v3(
    action: MultiLpActionV3,
) -> Result<usize, DealerEquityArtifactErrorV3> {
    scalar_count(action).map(usize::from)
}

/// Exact identity register count selected by one equity action.
pub fn dealer_equity_identity_count_v3(
    action: MultiLpActionV3,
) -> Result<usize, DealerEquityArtifactErrorV3> {
    identity_count(action).map(usize::from)
}

/// Build the exact Hot register bank from one authenticated request and plan.
///
/// This is an unsigned-operator projection, not a second semantic authority:
/// every emitted register is copied from the exact request, the canonical
/// physical plan, or one child request already owned by that plan. Both output
/// buffers remain byte-for-byte unchanged on every refusal.
#[cfg(not(target_os = "solana"))]
pub fn project_dealer_equity_hot_registers_v3(
    request: DealerEquityRequestV3<'_>,
    plan: MultiLpPlanV3,
    custody_effects: &[Option<MultiLpCustodyEffectV3>; MAX_MULTI_LP_CUSTODY_EFFECTS_V3],
    trusted_current_slot: u64,
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<(), DealerEquityArtifactErrorV3> {
    let request_action = match request.action() {
        EquityRequestActionV3::Contribute => MultiLpActionV3::Add,
        EquityRequestActionV3::Redeem => MultiLpActionV3::Remove,
    };
    if request_action != plan.action
        || request.shares != plan.share_delta
        || (plan.action == MultiLpActionV3::Add && request.collateral != plan.collateral_in)
        || (plan.action == MultiLpActionV3::Remove && request.collateral != 0)
        || trusted_current_slot > request.expires_at
    {
        return Err(DealerEquityArtifactErrorV3::Projection);
    }
    let expected_scalars = dealer_equity_scalar_count_v3(plan.action)?;
    let expected_identities = dealer_equity_identity_count_v3(plan.action)?;
    if scalars.len() != expected_scalars || identities.len() != expected_identities {
        return Err(DealerEquityArtifactErrorV3::Geometry);
    }
    let signed_position_count = request
        .claims_plan()
        .map_err(|_| DealerEquityArtifactErrorV3::Projection)?
        .map_or(0, |signed| signed.position_count());
    if signed_position_count > 2 {
        return Err(DealerEquityArtifactErrorV3::Geometry);
    }

    let parent_request_digest = hash(request.bytes()).to_bytes();
    let mut by_slot = [None; 3];
    let active_count = usize::from(plan.custody_count);
    if active_count > custody_effects.len()
        || multi_lp_custody_digest_v3(custody_effects, plan.custody_count)
            .map_err(|_| DealerEquityArtifactErrorV3::Projection)?
            != plan.custody_digest
    {
        return Err(DealerEquityArtifactErrorV3::Projection);
    }
    for effect in custody_effects.iter().take(active_count) {
        let child = effect
            .ok_or(DealerEquityArtifactErrorV3::Projection)?
            .request;
        let custody = child.custody();
        if custody.semantic.parent_request_digest != parent_request_digest {
            return Err(DealerEquityArtifactErrorV3::Projection);
        }
        let slot = equity_custody_slot(plan.action, child)
            .ok_or(DealerEquityArtifactErrorV3::Projection)?;
        if by_slot
            .get_mut(slot)
            .ok_or(DealerEquityArtifactErrorV3::Projection)?
            .replace(child)
            .is_some()
        {
            return Err(DealerEquityArtifactErrorV3::Projection);
        }
        let mut encoded = vec![0; child.encoded_len()];
        child
            .encode_into(&mut encoded)
            .map_err(|_| DealerEquityArtifactErrorV3::Projection)?;
    }
    if custody_effects
        .iter()
        .skip(active_count)
        .any(Option::is_some)
    {
        return Err(DealerEquityArtifactErrorV3::Projection);
    }
    let expected_amounts = match plan.action {
        MultiLpActionV3::Add => [plan.collateral_in, plan.maximum_complete_sets_to_merge, 0],
        MultiLpActionV3::Remove => [
            plan.minimum_complete_sets_to_split,
            plan.collateral_out,
            plan.maximum_complete_sets_to_merge,
        ],
    };
    for (slot, amount) in expected_amounts
        .iter()
        .copied()
        .take(custody_slot_count(plan.action))
        .enumerate()
    {
        match by_slot.get(slot).copied().flatten() {
            Some(child) if amount != 0 && child.custody().amount == amount => {}
            None if amount == 0 => {}
            _ => return Err(DealerEquityArtifactErrorV3::Projection),
        }
    }

    let mut staged_scalars = vec![0_u64; expected_scalars];
    let mut staged_identities = vec![[0_u8; 32]; expected_identities];
    set_scalar(
        &mut staged_scalars,
        DEALER_EQUITY_OBLIGATION_REVISION_SCALAR_V3,
        plan.obligation_revision_after,
    )?;
    set_scalar(
        &mut staged_scalars,
        DEALER_EQUITY_TOTAL_SHARES_SCALAR_V3,
        plan.total_equity_shares_after,
    )?;
    set_scalar(
        &mut staged_scalars,
        DEALER_EQUITY_LP_REVISION_SCALAR_V3,
        plan.lp_revision_after,
    )?;
    set_scalar(
        &mut staged_scalars,
        DEALER_EQUITY_LP_SHARES_SCALAR_V3,
        plan.lp_equity_shares_after,
    )?;
    set_scalar(
        &mut staged_scalars,
        DEALER_EQUITY_WITNESS_OFFSET_SCALAR_V3,
        u64::try_from(dealer_equity_witness_offset_v3())
            .map_err(|_| DealerEquityArtifactErrorV3::Arithmetic)?,
    )?;
    set_scalar(
        &mut staged_scalars,
        DEALER_EQUITY_WITNESS_BYTES_SCALAR_V3,
        u64::try_from(request.claims_packet().len())
            .map_err(|_| DealerEquityArtifactErrorV3::Arithmetic)?,
    )?;
    let current_slot = dealer_current_slot_scalar_register_v3(plan.action)
        .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
    let expiry = dealer_expiry_scalar_register_v3(plan.action)
        .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
    set_scalar(&mut staged_scalars, current_slot, trusted_current_slot)?;
    set_scalar(&mut staged_scalars, expiry, request.expires_at)?;
    *staged_identities
        .get_mut(usize::from(DEALER_EQUITY_PARENT_REQUEST_DIGEST_IDENTITY_V3))
        .ok_or(DealerEquityArtifactErrorV3::Geometry)? = parent_request_digest;
    for (slot, child) in by_slot
        .iter()
        .copied()
        .take(custody_slot_count(plan.action))
        .enumerate()
    {
        let Some(child) = child else { continue };
        project_custody_registers(
            u16::try_from(slot).map_err(|_| DealerEquityArtifactErrorV3::Arithmetic)?,
            child,
            &mut staged_scalars,
            &mut staged_identities,
        )?;
    }
    scalars.copy_from_slice(&staged_scalars);
    identities.copy_from_slice(&staged_identities);
    Ok(())
}

#[cfg(not(target_os = "solana"))]
fn set_scalar(
    scalars: &mut [u64],
    index: u16,
    value: u64,
) -> Result<(), DealerEquityArtifactErrorV3> {
    *scalars
        .get_mut(usize::from(index))
        .ok_or(DealerEquityArtifactErrorV3::Geometry)? = value;
    Ok(())
}

/// Exact encoded EffectProgram width for one action-specific P0/P1/P2 shape.
pub fn dealer_equity_effect_program_bytes_v3(
    action: MultiLpActionV3,
    signed_position_count: u32,
) -> Result<usize, DealerEquityArtifactErrorV3> {
    if signed_position_count > 2 {
        return Err(DealerEquityArtifactErrorV3::Geometry);
    }
    let slots = custody_slot_count(action);
    let routes = slots
        .checked_add(1)
        .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
    let operations = slots
        .checked_mul(27)
        .and_then(|value| value.checked_add(4))
        .and_then(|value| {
            value.checked_add(usize::from(action == MultiLpActionV3::Add).saturating_mul(3))
        })
        .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
    let template_bytes = match action {
        MultiLpActionV3::Add => {
            DELEGATED_CUSTODY_REQUEST_BYTES_V2.checked_add(CUSTODY_REQUEST_BYTES_V1)
        }
        MultiLpActionV3::Remove => slots.checked_mul(CUSTODY_REQUEST_BYTES_V1),
    }
    .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
    let dependency_bytes = if signed_position_count == 0 {
        0
    } else {
        slots
            .checked_sub(1)
            .and_then(|count| count.checked_mul(RECEIPT_DEPENDENCY_BYTES))
            .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?
    };
    HEADER_BYTES
        .checked_add(
            routes
                .checked_mul(ROUTE_BYTES)
                .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?,
        )
        .and_then(|value| value.checked_add(dependency_bytes))
        .and_then(|value| value.checked_add(operations.checked_mul(OPERATION_BYTES)?))
        .and_then(|value| value.checked_add(template_bytes))
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
    custody_templates: &[MultiLpCustodyRequestV3],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerEquityArtifactErrorV3> {
    let slots = custody_slot_count(action);
    if custody_templates.len() != slots || signed_position_count > 2 {
        return Err(DealerEquityArtifactErrorV3::Geometry);
    }
    let expected = dealer_equity_effect_program_bytes_v3(action, signed_position_count)?;
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
        let mut encoded = vec![0_u8; template.encoded_len()];
        template
            .encode_into(&mut encoded)
            .map_err(|_| DealerEquityArtifactErrorV3::CustodyTemplate)?;
        templates.push(encoded);
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
    let mut instructions = Vec::with_capacity(
        slots
            .saturating_mul(27)
            .saturating_add(4)
            .saturating_add(usize::from(action == MultiLpActionV3::Add).saturating_mul(3)),
    );
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
        push_custody_projection(action, slot, route, &mut instructions)?;
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
    action: MultiLpActionV3,
    slot: usize,
    route: usize,
    output: &mut Vec<EffectInstructionV3>,
) -> Result<(), DealerEquityArtifactErrorV3> {
    let slot = u16::try_from(slot).map_err(|_| DealerEquityArtifactErrorV3::Arithmetic)?;
    let route = u16::try_from(route).map_err(|_| DealerEquityArtifactErrorV3::Arithmetic)?;
    let delegated = action == MultiLpActionV3::Add && slot == 0;
    let base = if delegated {
        DelegatedCustodyRequestLayoutV2::BASE
    } else {
        0
    };
    output.push(EffectInstructionV3::write_request_u16(
        route,
        RequestSpaceV3::Fixed,
        request_offset(base, CustodyRequestLayoutV1::TRANSFER_INDEX)?,
        ScalarCoordinateV3::common(custody_scalar(
            slot,
            DealerCustodyScalarFieldV3::TransferIndex,
        )?),
    ));
    output.push(EffectInstructionV3::write_request_identity(
        route,
        RequestSpaceV3::Fixed,
        request_offset(base, CustodyRequestLayoutV1::PARENT_REQUEST_DIGEST)?,
        IdentityCoordinateV3::common(DEALER_EQUITY_PARENT_REQUEST_DIGEST_IDENTITY_V3),
    ));
    for (field, offset) in identity_fields().into_iter().zip(identity_offsets()) {
        output.push(EffectInstructionV3::write_request_identity(
            route,
            RequestSpaceV3::Fixed,
            request_offset(base, offset)?,
            IdentityCoordinateV3::common(custody_identity(slot, field)?),
        ));
    }
    for (field, offset) in [
        (
            DealerCustodyScalarFieldV3::ExpectedRevision,
            CustodyRequestLayoutV1::EXPECTED_REVISION,
        ),
        (
            DealerCustodyScalarFieldV3::ResultingRevision,
            CustodyRequestLayoutV1::RESULTING_REVISION,
        ),
        (
            DealerCustodyScalarFieldV3::OrderNonce,
            CustodyRequestLayoutV1::ORDER_NONCE,
        ),
        (
            DealerCustodyScalarFieldV3::Generation,
            CustodyRequestLayoutV1::GENERATION,
        ),
        (
            DealerCustodyScalarFieldV3::Amount,
            CustodyRequestLayoutV1::AMOUNT,
        ),
        (
            DealerCustodyScalarFieldV3::RentLamports,
            CustodyRequestLayoutV1::RENT_LAMPORTS,
        ),
    ] {
        output.push(EffectInstructionV3::write_request_u64(
            route,
            RequestSpaceV3::Fixed,
            request_offset(base, offset)?,
            ScalarCoordinateV3::common(custody_scalar(slot, field)?),
        ));
    }
    for (field, offset) in [
        (
            DealerCustodyScalarFieldV3::PageIndex,
            CustodyRequestLayoutV1::PAGE_INDEX,
        ),
        (
            DealerCustodyScalarFieldV3::ExecutionIndex,
            CustodyRequestLayoutV1::EXECUTION_INDEX,
        ),
    ] {
        output.push(EffectInstructionV3::write_request_u32(
            route,
            RequestSpaceV3::Fixed,
            request_offset(base, offset)?,
            ScalarCoordinateV3::common(custody_scalar(slot, field)?),
        ));
    }
    if delegated {
        let delegate = dealer_external_delegate_identity_register_v3(action)
            .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
        let amount = custody_scalar(slot, DealerCustodyScalarFieldV3::Amount)?;
        output.push(EffectInstructionV3::write_request_identity(
            route,
            RequestSpaceV3::Fixed,
            request_offset(0, DelegatedCustodyRequestLayoutV2::DELEGATE_BEFORE)?,
            IdentityCoordinateV3::common(delegate),
        ));
        for offset in [
            DelegatedCustodyRequestLayoutV2::TOTAL_DEBIT,
            DelegatedCustodyRequestLayoutV2::ALLOWANCE_BEFORE,
        ] {
            output.push(EffectInstructionV3::write_request_u64(
                route,
                RequestSpaceV3::Fixed,
                request_offset(0, offset)?,
                ScalarCoordinateV3::common(amount),
            ));
        }
    }
    Ok(())
}

fn validate_template(
    action: MultiLpActionV3,
    slot: usize,
    template: MultiLpCustodyRequestV3,
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
    let custody = template.custody();
    let kind_matches = matches!(
        (action, slot, template),
        (
            MultiLpActionV3::Add,
            0,
            MultiLpCustodyRequestV3::Delegated(_)
        ) | (
            MultiLpActionV3::Add,
            1,
            MultiLpCustodyRequestV3::Canonical(_)
        ) | (
            MultiLpActionV3::Remove,
            _,
            MultiLpCustodyRequestV3::Canonical(_)
        )
    );
    if !kind_matches
        || custody.operation != OperationV1::Transfer
        || custody.caller_role != CallerRoleV1::Trading
        || (custody.source_compartment, custody.destination_compartment) != expected
    {
        return Err(DealerEquityArtifactErrorV3::CustodyTemplate);
    }
    Ok(())
}

#[cfg(not(target_os = "solana"))]
fn equity_custody_slot(action: MultiLpActionV3, request: MultiLpCustodyRequestV3) -> Option<usize> {
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

#[cfg(not(target_os = "solana"))]
fn project_custody_registers(
    slot: u16,
    request: MultiLpCustodyRequestV3,
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<(), DealerEquityArtifactErrorV3> {
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
        let register = dealer_custody_scalar_register_v3(slot, field)
            .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
        *scalars
            .get_mut(usize::from(register))
            .ok_or(DealerEquityArtifactErrorV3::Geometry)? = value;
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
    for (field, value) in identity_fields().into_iter().zip(values) {
        let register = dealer_custody_identity_register_v3(slot, field)
            .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
        *identities
            .get_mut(usize::from(register))
            .ok_or(DealerEquityArtifactErrorV3::Geometry)? = value;
    }
    if let MultiLpCustodyRequestV3::Delegated(delegated) = request {
        let register = dealer_external_delegate_identity_register_v3(MultiLpActionV3::Add)
            .ok_or(DealerEquityArtifactErrorV3::Arithmetic)?;
        *identities
            .get_mut(usize::from(register))
            .ok_or(DealerEquityArtifactErrorV3::Geometry)? = delegated.delegate_before;
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
        .and_then(|width| width.checked_add(2))
        .ok_or(DealerEquityArtifactErrorV3::Arithmetic)
}

fn identity_count(action: MultiLpActionV3) -> Result<u16, DealerEquityArtifactErrorV3> {
    u16::try_from(custody_slot_count(action))
        .ok()
        .and_then(|slots| slots.checked_mul(CUSTODY_IDENTITY_STRIDE_V3))
        .and_then(|width| CUSTODY_IDENTITY_BASE_V3.checked_add(width))
        .and_then(|width| width.checked_add(u16::from(action == MultiLpActionV3::Add)))
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

fn request_offset(base: usize, field: usize) -> Result<u32, DealerEquityArtifactErrorV3> {
    base.checked_add(field)
        .and_then(|offset| u32::try_from(offset).ok())
        .ok_or(DealerEquityArtifactErrorV3::Arithmetic)
}

const fn identity_offsets() -> [usize; CUSTODY_IDENTITY_FIELD_COUNT_V3] {
    [
        CustodyRequestLayoutV1::RELEASE_SET,
        CustodyRequestLayoutV1::MARKET,
        CustodyRequestLayoutV1::REALM,
        CustodyRequestLayoutV1::CONTEXT,
        CustodyRequestLayoutV1::CALLER_PROGRAM,
        CustodyRequestLayoutV1::CANDIDATE,
        CustodyRequestLayoutV1::SOURCE_OWNER,
        CustodyRequestLayoutV1::DESTINATION_OWNER,
        CustodyRequestLayoutV1::ORDER,
        CustodyRequestLayoutV1::SOURCE,
        CustodyRequestLayoutV1::DESTINATION,
        CustodyRequestLayoutV1::SOURCE_VAULT_CONTEXT,
        CustodyRequestLayoutV1::DESTINATION_VAULT_CONTEXT,
        CustodyRequestLayoutV1::MINT,
        CustodyRequestLayoutV1::TOKEN_PROGRAM,
        CustodyRequestLayoutV1::PAYER,
        CustodyRequestLayoutV1::RENT_REFUND,
    ]
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
    use dclutch_custody_contract::{ContextV1, CustodyRequestV1, DelegatedCustodyRequestV2};
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

    fn delegated_template(custody: CustodyRequestV1) -> MultiLpCustodyRequestV3 {
        MultiLpCustodyRequestV3::Delegated(DelegatedCustodyRequestV2 {
            custody,
            starts_atomic_debit: true,
            terminal: true,
            delegate_before: [31; 32],
            delegate_after: [0; 32],
            total_debit: custody.amount,
            allowance_before: custody.amount,
            allowance_after: 0,
        })
    }

    const fn canonical_template(custody: CustodyRequestV1) -> MultiLpCustodyRequestV3 {
        MultiLpCustodyRequestV3::Canonical(custody)
    }

    fn encoded(request: MultiLpCustodyRequestV3) -> Vec<u8> {
        let mut output = vec![0; request.encoded_len()];
        request.encode_into(&mut output).expect("template bytes");
        output
    }

    #[test]
    fn typed_p2_contribution_artifact_has_exact_routes_and_dependency() {
        let templates = [
            delegated_template(transfer_template(
                CompartmentV1::External,
                CompartmentV1::TradingPrincipal,
                22,
            )),
            canonical_template(transfer_template(
                CompartmentV1::HoardPrincipal,
                CompartmentV1::TradingPrincipal,
                24,
            )),
        ];
        let width =
            dealer_equity_effect_program_bytes_v3(MultiLpActionV3::Add, 2).expect("artifact width");
        assert_eq!(width, 3_048);
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
        assert_eq!(program.common_scalar_count(), 26);
        assert_eq!(program.common_identity_count(), 36);
        assert_eq!(program.fixed_operation_count(), 61);
        assert_eq!(program.item_operation_count(), 0);

        let custody_in = program.route(0).expect("cash route");
        assert_eq!(custody_in.role(), FixedRole::Custody);
        assert_eq!(custody_in.fixed_account_start(), 5);
        assert_eq!(custody_in.fixed_account_count(), 14);
        assert_eq!(custody_in.fixed_request_bytes(), 776);
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
            encoded(templates[0])
        );
        assert_eq!(
            program.route_template(2).expect("merge template").0,
            encoded(templates[1])
        );
    }

    #[test]
    fn typed_p0_redemption_retains_all_conditional_custody_slots() {
        let templates = [
            canonical_template(transfer_template(
                CompartmentV1::TradingPrincipal,
                CompartmentV1::HoardPrincipal,
                22,
            )),
            canonical_template(transfer_template(
                CompartmentV1::TradingPrincipal,
                CompartmentV1::External,
                24,
            )),
            canonical_template(transfer_template(
                CompartmentV1::HoardPrincipal,
                CompartmentV1::TradingPrincipal,
                26,
            )),
        ];
        let width = dealer_equity_effect_program_bytes_v3(MultiLpActionV3::Remove, 0)
            .expect("artifact width");
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
        assert_eq!(program.common_scalar_count(), 35);
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
            canonical_template(transfer_template(
                CompartmentV1::TradingPrincipal,
                CompartmentV1::External,
                22,
            )),
            canonical_template(transfer_template(
                CompartmentV1::HoardPrincipal,
                CompartmentV1::TradingPrincipal,
                24,
            )),
        ];
        let width =
            dealer_equity_effect_program_bytes_v3(MultiLpActionV3::Add, 2).expect("artifact width");
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
            delegated_template(transfer_template(
                CompartmentV1::External,
                CompartmentV1::TradingPrincipal,
                22,
            )),
            canonical_template(transfer_template(
                CompartmentV1::HoardPrincipal,
                CompartmentV1::TradingPrincipal,
                24,
            )),
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
