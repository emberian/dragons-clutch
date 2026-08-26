//! Ordered local-state and Custody effects for registered Buy admission.
//!
//! The fixed topology initializes the Direct maker/record candidates, then
//! invokes Custody InitializeReplay, OpenVault, and one terminal delegated
//! external-to-record deposit.  Receipt dependencies bind that exact order;
//! local/root commits remain owned by common Hot after every child receipt is
//! authenticated.

use dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1;
use dclutch_custody_contract::{
    CUSTODY_RECEIPT_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1, CustodyRequestLayoutV1,
    CustodyRequestV1, DELEGATED_CUSTODY_REQUEST_BYTES_V2, DelegatedCustodyRequestLayoutV2,
    DelegatedCustodyRequestV2, OperationV1,
};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{
        HEADER_BYTES, OPERATION_BYTES, ProgramV3, RECEIPT_DEPENDENCY_BYTES, ROUTE_BYTES,
        RouteKindV3, RouteReceiptDependencyV3,
        encode::{
            AccountCoordinateV3, EffectGeometryV3, EffectInstructionV3, IdentityCoordinateV3,
            RequestSpaceV3, RouteInputV3, ScalarCoordinateV3, encode_effect_program_v4_atomic,
        },
    },
    v4::{BorrowedRangePolicyV4, HEADER_BYTES_V4, ProgramV4, encode_program_v4_atomic},
};

use crate::{
    execution_v3::DIRECT_REGISTRATION_REQUEST_BYTES_V3,
    generated_intent_v2 as intent,
    registered_account_artifacts_v4::{
        DIRECT_REGISTER_BUY_DEPOSIT_ACCOUNT_START_V4, DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4,
        DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4, DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4,
    },
    registered_creation_artifacts_v4::{
        DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4,
        DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4,
        DIRECT_REGISTERED_CREATION_ITEM_IDENTITY_STRIDE_V4,
        DIRECT_REGISTERED_CREATION_ITEM_SCALAR_STRIDE_V4, REGISTERED_IDENTITY_COLLATERAL_SOURCE_V4,
        REGISTERED_IDENTITY_CUSTODY_AUTHORITY_V4, REGISTERED_IDENTITY_CUSTODY_VAULT_V4,
        REGISTERED_IDENTITY_MAKER_BENEFICIARY_V4, REGISTERED_IDENTITY_MARKET_V4,
        REGISTERED_IDENTITY_MINT_V4, REGISTERED_IDENTITY_PARENT_REQUEST_V4,
        REGISTERED_IDENTITY_PAYER_V4, REGISTERED_IDENTITY_REALM_V4,
        REGISTERED_IDENTITY_RECORD_BENEFICIARY_V4, REGISTERED_IDENTITY_RECORD_STATE_V4,
        REGISTERED_IDENTITY_RELEASE_SET_V4, REGISTERED_IDENTITY_REQUEST_MAKER_V4,
        REGISTERED_IDENTITY_TOKEN_PROGRAM_V4, REGISTERED_IDENTITY_TRADING_PROGRAM_V4,
        REGISTERED_SCALAR_COLLATERAL_RESERVE_V4, REGISTERED_SCALAR_FEE_BPS_V4,
        REGISTERED_SCALAR_GENERATION_V4, REGISTERED_SCALAR_INTENT_MAGIC_V4,
        REGISTERED_SCALAR_LIFECYCLE_V4, REGISTERED_SCALAR_LIMIT_V4,
        REGISTERED_SCALAR_MAKER_BUMP_V4, REGISTERED_SCALAR_MAKER_LIVE_COUNT_AFTER_V4,
        REGISTERED_SCALAR_MAKER_MAGIC_V4, REGISTERED_SCALAR_MAKER_PRINCIPAL_V4,
        REGISTERED_SCALAR_MAKER_VERSION_V4, REGISTERED_SCALAR_MAXIMUM_V4,
        REGISTERED_SCALAR_MINIMUM_LIVE_NONCE_AFTER_V4, REGISTERED_SCALAR_NEXT_NONCE_AFTER_V4,
        REGISTERED_SCALAR_NONCE_V4, REGISTERED_SCALAR_OUTCOME_V4, REGISTERED_SCALAR_RECORD_BUMP_V4,
        REGISTERED_SCALAR_RECORD_MAGIC_V4, REGISTERED_SCALAR_RECORD_PRINCIPAL_V4,
        REGISTERED_SCALAR_RECORD_VERSION_V4, REGISTERED_SCALAR_REPLAY_RENT_V4,
        REGISTERED_SCALAR_ROOT_OPEN_COUNT_AFTER_V4, REGISTERED_SCALAR_SIDE_V4,
        REGISTERED_SCALAR_VALID_FROM_V4, REGISTERED_SCALAR_VALID_THROUGH_V4,
        REGISTERED_SCALAR_VAULT_RENT_V4, REGISTERED_SCALAR_ZERO_V4,
    },
    registered_state_artifacts_v4::{
        DIRECT_REGISTERED_MAKER_ACCOUNT_V4, DIRECT_REGISTERED_RECORD_ACCOUNT_V4,
    },
    successor::{
        DirectMakerReplayLayoutV1, DirectRegisteredRecordLayoutV2, DirectRootStateLayoutV1,
    },
};

const ROUTE_COUNT: usize = 3;
const DEPENDENCY_COUNT: usize = 3;
const FIXED_INSTRUCTION_COUNT: usize = 87;
const REQUEST_BANK_BYTES: usize =
    2 * dclutch_custody_contract::CUSTODY_REQUEST_BYTES_V1 + DELEGATED_CUSTODY_REQUEST_BYTES_V2;
const BASE_EFFECT_BYTES: usize = HEADER_BYTES
    + ROUTE_COUNT * ROUTE_BYTES
    + DEPENDENCY_COUNT * RECEIPT_DEPENDENCY_BYTES
    + FIXED_INSTRUCTION_COUNT * OPERATION_BYTES
    + REQUEST_BANK_BYTES;

/// Exact zero-extension DCE5 bytes for registered Buy creation.
pub const DIRECT_REGISTER_BUY_EFFECT_BYTES_V4: usize = HEADER_BYTES_V4 + BASE_EFFECT_BYTES;

/// Stable registered Effect artifact refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRegisteredEffectArtifactErrorV4 {
    /// One request, register, or account coordinate did not fit.
    Coordinate,
    /// A canonical Custody request template refused.
    ChildRequest,
    /// The Effect semantic-owner encoder or hostile decoder refused.
    Effect,
}

/// Emit the exact registered Buy EffectProgramV4 atomically.
pub fn encode_direct_register_buy_effect_v4_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredEffectArtifactErrorV4> {
    if scratch.len() != DIRECT_REGISTER_BUY_EFFECT_BYTES_V4
        || output.len() != DIRECT_REGISTER_BUY_EFFECT_BYTES_V4
    {
        return Err(DirectRegisteredEffectArtifactErrorV4::Coordinate);
    }
    let mut base_scratch = [0_u8; BASE_EFFECT_BYTES];
    let mut base = [0_u8; BASE_EFFECT_BYTES];
    encode_base_atomic(&mut base_scratch, &mut base)?;
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        u32::try_from(DIRECT_REGISTRATION_REQUEST_BYTES_V3)
            .map_err(|_| DirectRegisteredEffectArtifactErrorV4::Coordinate)?,
        &[],
        &[],
        scratch,
        output,
    )
    .map_err(|_| DirectRegisteredEffectArtifactErrorV4::Effect)?;
    ProgramV4::decode(output).map_err(|_| DirectRegisteredEffectArtifactErrorV4::Effect)?;
    Ok(())
}

fn encode_base_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredEffectArtifactErrorV4> {
    let initialize = initialize_template()?;
    let open = open_template()?;
    let deposit = deposit_template()?;
    let routes = [
        route(
            DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4,
            12,
            &initialize,
        ),
        route(DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4, 16, &open),
        route(DIRECT_REGISTER_BUY_DEPOSIT_ACCOUNT_START_V4, 14, &deposit),
    ];
    let initialize_dependency =
        RouteReceiptDependencyV3::new(FixedRole::Custody, 0, width16(CUSTODY_RECEIPT_BYTES_V1)?);
    let open_dependency =
        RouteReceiptDependencyV3::new(FixedRole::Custody, 1, width16(CUSTODY_RECEIPT_BYTES_V1)?);
    let route1 = [initialize_dependency];
    let route2 = [initialize_dependency, open_dependency];
    let dependencies: [&[RouteReceiptDependencyV3]; ROUTE_COUNT] = [&[], &route1, &route2];
    let instructions = effect_instructions()?;
    encode_effect_program_v4_atomic(
        EffectGeometryV3 {
            fixed_accounts: DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4,
            item_account_stride: 0,
            common_scalars: scalar(DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4)?,
            item_scalar_stride: DIRECT_REGISTERED_CREATION_ITEM_SCALAR_STRIDE_V4,
            common_identities: identity(DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4)?,
            item_identity_stride: DIRECT_REGISTERED_CREATION_ITEM_IDENTITY_STRIDE_V4,
        },
        &routes,
        &dependencies,
        &instructions,
        &[],
        scratch,
        output,
    )
    .map_err(|_| DirectRegisteredEffectArtifactErrorV4::Effect)?;
    ProgramV3::decode(output).map_err(|_| DirectRegisteredEffectArtifactErrorV4::Effect)?;
    Ok(())
}

fn effect_instructions()
-> Result<[EffectInstructionV3; FIXED_INSTRUCTION_COUNT], DirectRegisteredEffectArtifactErrorV4> {
    let placeholder = EffectInstructionV3::write_u64(
        AccountCoordinateV3::fixed(0),
        0,
        ScalarCoordinateV3::common(0),
    );
    let mut output = [placeholder; FIXED_INSTRUCTION_COUNT];
    let mut next = 0;
    push_local_state(&mut output, &mut next)?;
    push_initialize_request(&mut output, &mut next)?;
    push_open_request(&mut output, &mut next)?;
    push_deposit_request(&mut output, &mut next)?;
    if next != output.len() {
        return Err(DirectRegisteredEffectArtifactErrorV4::Coordinate);
    }
    Ok(output)
}

fn push_local_state(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
) -> Result<(), DirectRegisteredEffectArtifactErrorV4> {
    push(
        output,
        next,
        EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(0),
            offset(
                CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT,
            )?,
            scalar_coordinate(REGISTERED_SCALAR_ROOT_OPEN_COUNT_AFTER_V4)?,
        ),
    )?;
    let maker = AccountCoordinateV3::fixed(DIRECT_REGISTERED_MAKER_ACCOUNT_V4);
    for instruction in [
        EffectInstructionV3::write_u64(
            maker,
            offset(DirectMakerReplayLayoutV1::MAGIC)?,
            scalar_coordinate(REGISTERED_SCALAR_MAKER_MAGIC_V4)?,
        ),
        EffectInstructionV3::write_u16(
            maker,
            offset(DirectMakerReplayLayoutV1::VERSION)?,
            scalar_coordinate(REGISTERED_SCALAR_MAKER_VERSION_V4)?,
        ),
        EffectInstructionV3::write_u8(
            maker,
            offset(DirectMakerReplayLayoutV1::BUMP)?,
            scalar_coordinate(REGISTERED_SCALAR_MAKER_BUMP_V4)?,
        ),
        EffectInstructionV3::write_identity(
            maker,
            offset(DirectMakerReplayLayoutV1::MARKET)?,
            identity_coordinate(REGISTERED_IDENTITY_MARKET_V4)?,
        ),
        EffectInstructionV3::write_u64(
            maker,
            offset(DirectMakerReplayLayoutV1::GENERATION)?,
            scalar_coordinate(REGISTERED_SCALAR_GENERATION_V4)?,
        ),
        EffectInstructionV3::write_identity(
            maker,
            offset(DirectMakerReplayLayoutV1::MAKER)?,
            identity_coordinate(REGISTERED_IDENTITY_REQUEST_MAKER_V4)?,
        ),
        EffectInstructionV3::write_u64(
            maker,
            offset(DirectMakerReplayLayoutV1::NEXT_NONCE)?,
            scalar_coordinate(REGISTERED_SCALAR_NEXT_NONCE_AFTER_V4)?,
        ),
        EffectInstructionV3::write_u64(
            maker,
            offset(DirectMakerReplayLayoutV1::LIVE_COUNT)?,
            scalar_coordinate(REGISTERED_SCALAR_MAKER_LIVE_COUNT_AFTER_V4)?,
        ),
        EffectInstructionV3::write_u64(
            maker,
            offset(DirectMakerReplayLayoutV1::MINIMUM_LIVE_NONCE)?,
            scalar_coordinate(REGISTERED_SCALAR_MINIMUM_LIVE_NONCE_AFTER_V4)?,
        ),
        EffectInstructionV3::write_identity(
            maker,
            offset(DirectMakerReplayLayoutV1::RENT_OWNER)?,
            identity_coordinate(REGISTERED_IDENTITY_MAKER_BENEFICIARY_V4)?,
        ),
        EffectInstructionV3::write_u64(
            maker,
            offset(DirectMakerReplayLayoutV1::RENT_PRINCIPAL)?,
            scalar_coordinate(REGISTERED_SCALAR_MAKER_PRINCIPAL_V4)?,
        ),
    ] {
        push(output, next, instruction)?;
    }
    let record = AccountCoordinateV3::fixed(DIRECT_REGISTERED_RECORD_ACCOUNT_V4);
    let intent_base = DirectRegisteredRecordLayoutV2::INTENT;
    for instruction in [
        EffectInstructionV3::write_u64(
            record,
            offset(DirectRegisteredRecordLayoutV2::MAGIC)?,
            scalar_coordinate(REGISTERED_SCALAR_RECORD_MAGIC_V4)?,
        ),
        EffectInstructionV3::write_u16(
            record,
            offset(DirectRegisteredRecordLayoutV2::VERSION)?,
            scalar_coordinate(REGISTERED_SCALAR_RECORD_VERSION_V4)?,
        ),
        EffectInstructionV3::write_u8(
            record,
            offset(DirectRegisteredRecordLayoutV2::BUMP)?,
            scalar_coordinate(REGISTERED_SCALAR_RECORD_BUMP_V4)?,
        ),
        EffectInstructionV3::write_identity(
            record,
            offset(DirectRegisteredRecordLayoutV2::MAKER)?,
            identity_coordinate(REGISTERED_IDENTITY_REQUEST_MAKER_V4)?,
        ),
        EffectInstructionV3::write_u64(
            record,
            offset(intent_base + intent::COMPACT_INTENT_MAGIC_OFFSET_V2)?,
            scalar_coordinate(REGISTERED_SCALAR_INTENT_MAGIC_V4)?,
        ),
        EffectInstructionV3::write_u16(
            record,
            offset(intent_base + intent::COMPACT_INTENT_VERSION_OFFSET_V2)?,
            scalar_coordinate(REGISTERED_SCALAR_RECORD_VERSION_V4)?,
        ),
        EffectInstructionV3::write_u8(
            record,
            offset(intent_base + intent::COMPACT_INTENT_SIDE_OFFSET_V2)?,
            scalar_coordinate(REGISTERED_SCALAR_SIDE_V4)?,
        ),
        EffectInstructionV3::write_u8(
            record,
            offset(intent_base + intent::COMPACT_INTENT_LIFECYCLE_OFFSET_V2)?,
            scalar_coordinate(REGISTERED_SCALAR_LIFECYCLE_V4)?,
        ),
        EffectInstructionV3::write_u32(
            record,
            offset(intent_base + intent::COMPACT_INTENT_OUTCOME_OFFSET_V2)?,
            scalar_coordinate(REGISTERED_SCALAR_OUTCOME_V4)?,
        ),
        EffectInstructionV3::write_identity(
            record,
            offset(intent_base + intent::COMPACT_INTENT_MARKET_OFFSET_V2)?,
            identity_coordinate(REGISTERED_IDENTITY_MARKET_V4)?,
        ),
        EffectInstructionV3::write_u64(
            record,
            offset(intent_base + intent::COMPACT_INTENT_GENERATION_OFFSET_V2)?,
            scalar_coordinate(REGISTERED_SCALAR_GENERATION_V4)?,
        ),
        EffectInstructionV3::write_u64(
            record,
            offset(intent_base + intent::COMPACT_INTENT_NONCE_OFFSET_V2)?,
            scalar_coordinate(REGISTERED_SCALAR_NONCE_V4)?,
        ),
        EffectInstructionV3::write_u64(
            record,
            offset(intent_base + intent::COMPACT_INTENT_VALID_FROM_OFFSET_V2)?,
            scalar_coordinate(REGISTERED_SCALAR_VALID_FROM_V4)?,
        ),
        EffectInstructionV3::write_u64(
            record,
            offset(intent_base + intent::COMPACT_INTENT_VALID_THROUGH_OFFSET_V2)?,
            scalar_coordinate(REGISTERED_SCALAR_VALID_THROUGH_V4)?,
        ),
        EffectInstructionV3::write_u64(
            record,
            offset(intent_base + intent::COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2)?,
            scalar_coordinate(REGISTERED_SCALAR_MAXIMUM_V4)?,
        ),
        EffectInstructionV3::write_u64(
            record,
            offset(intent_base + intent::COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2)?,
            scalar_coordinate(REGISTERED_SCALAR_LIMIT_V4)?,
        ),
        EffectInstructionV3::write_u16(
            record,
            offset(intent_base + intent::COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2)?,
            scalar_coordinate(REGISTERED_SCALAR_FEE_BPS_V4)?,
        ),
        EffectInstructionV3::write_identity(
            record,
            offset(intent_base + intent::COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2)?,
            identity_coordinate(REGISTERED_IDENTITY_COLLATERAL_SOURCE_V4)?,
        ),
        EffectInstructionV3::write_u64(
            record,
            offset(DirectRegisteredRecordLayoutV2::FILLED)?,
            scalar_coordinate(REGISTERED_SCALAR_ZERO_V4)?,
        ),
        EffectInstructionV3::write_u64(
            record,
            offset(DirectRegisteredRecordLayoutV2::RESERVED_CLAIMS)?,
            scalar_coordinate(REGISTERED_SCALAR_ZERO_V4)?,
        ),
        EffectInstructionV3::write_u64(
            record,
            offset(DirectRegisteredRecordLayoutV2::RESERVED_COLLATERAL)?,
            scalar_coordinate(REGISTERED_SCALAR_COLLATERAL_RESERVE_V4)?,
        ),
        EffectInstructionV3::write_u64(
            record,
            offset(DirectRegisteredRecordLayoutV2::CUMULATIVE_GROSS)?,
            scalar_coordinate(REGISTERED_SCALAR_ZERO_V4)?,
        ),
        EffectInstructionV3::write_u64(
            record,
            offset(DirectRegisteredRecordLayoutV2::CUMULATIVE_FEE)?,
            scalar_coordinate(REGISTERED_SCALAR_ZERO_V4)?,
        ),
        EffectInstructionV3::write_identity(
            record,
            offset(DirectRegisteredRecordLayoutV2::RENT_OWNER)?,
            identity_coordinate(REGISTERED_IDENTITY_RECORD_BENEFICIARY_V4)?,
        ),
        EffectInstructionV3::write_u64(
            record,
            offset(DirectRegisteredRecordLayoutV2::RENT_PRINCIPAL)?,
            scalar_coordinate(REGISTERED_SCALAR_RECORD_PRINCIPAL_V4)?,
        ),
    ] {
        push(output, next, instruction)?;
    }
    Ok(())
}

fn push_initialize_request(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
) -> Result<(), DirectRegisteredEffectArtifactErrorV4> {
    push_common_request(output, next, 0)?;
    for (field, value) in [
        (CustodyRequestLayoutV1::PAYER, REGISTERED_IDENTITY_PAYER_V4),
        (
            CustodyRequestLayoutV1::RENT_REFUND,
            REGISTERED_IDENTITY_RECORD_BENEFICIARY_V4,
        ),
    ] {
        push_request_identity(output, next, 0, field, value)?;
    }
    push_request_u64(
        output,
        next,
        0,
        CustodyRequestLayoutV1::RENT_LAMPORTS,
        REGISTERED_SCALAR_REPLAY_RENT_V4,
    )
}

fn push_open_request(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
) -> Result<(), DirectRegisteredEffectArtifactErrorV4> {
    push_common_request(output, next, 1)?;
    for (field, value) in [
        (
            CustodyRequestLayoutV1::DESTINATION,
            REGISTERED_IDENTITY_CUSTODY_VAULT_V4,
        ),
        (
            CustodyRequestLayoutV1::DESTINATION_VAULT_CONTEXT,
            REGISTERED_IDENTITY_RECORD_STATE_V4,
        ),
        (CustodyRequestLayoutV1::MINT, REGISTERED_IDENTITY_MINT_V4),
        (
            CustodyRequestLayoutV1::TOKEN_PROGRAM,
            REGISTERED_IDENTITY_TOKEN_PROGRAM_V4,
        ),
        (CustodyRequestLayoutV1::PAYER, REGISTERED_IDENTITY_PAYER_V4),
        (
            CustodyRequestLayoutV1::RENT_REFUND,
            REGISTERED_IDENTITY_RECORD_BENEFICIARY_V4,
        ),
    ] {
        push_request_identity(output, next, 1, field, value)?;
    }
    push_request_u64(
        output,
        next,
        1,
        CustodyRequestLayoutV1::RENT_LAMPORTS,
        REGISTERED_SCALAR_VAULT_RENT_V4,
    )
}

fn push_deposit_request(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
) -> Result<(), DirectRegisteredEffectArtifactErrorV4> {
    let base = DelegatedCustodyRequestLayoutV2::BASE;
    push_common_request_at(output, next, 2, base)?;
    for (field, value) in [
        (
            CustodyRequestLayoutV1::SOURCE_OWNER,
            REGISTERED_IDENTITY_REQUEST_MAKER_V4,
        ),
        (
            CustodyRequestLayoutV1::SOURCE,
            REGISTERED_IDENTITY_COLLATERAL_SOURCE_V4,
        ),
        (
            CustodyRequestLayoutV1::DESTINATION,
            REGISTERED_IDENTITY_CUSTODY_VAULT_V4,
        ),
        (
            CustodyRequestLayoutV1::DESTINATION_VAULT_CONTEXT,
            REGISTERED_IDENTITY_RECORD_STATE_V4,
        ),
        (CustodyRequestLayoutV1::MINT, REGISTERED_IDENTITY_MINT_V4),
        (
            CustodyRequestLayoutV1::TOKEN_PROGRAM,
            REGISTERED_IDENTITY_TOKEN_PROGRAM_V4,
        ),
    ] {
        push_request_identity(output, next, 2, add(base, field)?, value)?;
    }
    push_request_u64(
        output,
        next,
        2,
        add(base, CustodyRequestLayoutV1::AMOUNT)?,
        REGISTERED_SCALAR_COLLATERAL_RESERVE_V4,
    )?;
    push_request_identity(
        output,
        next,
        2,
        DelegatedCustodyRequestLayoutV2::DELEGATE_BEFORE,
        REGISTERED_IDENTITY_CUSTODY_AUTHORITY_V4,
    )?;
    for field in [
        DelegatedCustodyRequestLayoutV2::TOTAL_DEBIT,
        DelegatedCustodyRequestLayoutV2::ALLOWANCE_BEFORE,
    ] {
        push_request_u64(
            output,
            next,
            2,
            field,
            REGISTERED_SCALAR_COLLATERAL_RESERVE_V4,
        )?;
    }
    Ok(())
}

fn push_common_request(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
    route: u16,
) -> Result<(), DirectRegisteredEffectArtifactErrorV4> {
    push_common_request_at(output, next, route, 0)
}

fn push_common_request_at(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
    route: u16,
    base: usize,
) -> Result<(), DirectRegisteredEffectArtifactErrorV4> {
    for (field, value) in [
        (
            CustodyRequestLayoutV1::RELEASE_SET,
            REGISTERED_IDENTITY_RELEASE_SET_V4,
        ),
        (
            CustodyRequestLayoutV1::MARKET,
            REGISTERED_IDENTITY_MARKET_V4,
        ),
        (CustodyRequestLayoutV1::REALM, REGISTERED_IDENTITY_REALM_V4),
        (
            CustodyRequestLayoutV1::CONTEXT,
            REGISTERED_IDENTITY_RECORD_STATE_V4,
        ),
        (
            CustodyRequestLayoutV1::CALLER_PROGRAM,
            REGISTERED_IDENTITY_TRADING_PROGRAM_V4,
        ),
        (
            CustodyRequestLayoutV1::ORDER,
            REGISTERED_IDENTITY_RECORD_STATE_V4,
        ),
        (
            CustodyRequestLayoutV1::PARENT_REQUEST_DIGEST,
            REGISTERED_IDENTITY_PARENT_REQUEST_V4,
        ),
    ] {
        push_request_identity(output, next, route, add(base, field)?, value)?;
    }
    for (field, value) in [
        (
            CustodyRequestLayoutV1::ORDER_NONCE,
            REGISTERED_SCALAR_NONCE_V4,
        ),
        (
            CustodyRequestLayoutV1::GENERATION,
            REGISTERED_SCALAR_GENERATION_V4,
        ),
    ] {
        push_request_u64(output, next, route, add(base, field)?, value)?;
    }
    push(
        output,
        next,
        EffectInstructionV3::write_request_u32(
            route,
            RequestSpaceV3::Fixed,
            offset(add(base, CustodyRequestLayoutV1::EXECUTION_INDEX)?)?,
            scalar_coordinate(REGISTERED_SCALAR_OUTCOME_V4)?,
        ),
    )
}

fn initialize_template() -> Result<
    [u8; dclutch_custody_contract::CUSTODY_REQUEST_BYTES_V1],
    DirectRegisteredEffectArtifactErrorV4,
> {
    custody_template(OperationV1::InitializeReplay)
        .to_bytes()
        .map_err(|_| DirectRegisteredEffectArtifactErrorV4::ChildRequest)
}

fn open_template() -> Result<
    [u8; dclutch_custody_contract::CUSTODY_REQUEST_BYTES_V1],
    DirectRegisteredEffectArtifactErrorV4,
> {
    custody_template(OperationV1::OpenVault)
        .to_bytes()
        .map_err(|_| DirectRegisteredEffectArtifactErrorV4::ChildRequest)
}

fn custody_template(operation: OperationV1) -> CustodyRequestV1 {
    let open = operation == OperationV1::OpenVault;
    CustodyRequestV1 {
        operation,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::None,
        destination_compartment: if open {
            CompartmentV1::TradingPrincipal
        } else {
            CompartmentV1::None
        },
        release_set: id(1),
        market: id(2),
        realm: id(3),
        context: id(4),
        caller_program: id(5),
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order: id(6),
            parent_request_digest: id(7),
            order_nonce: 1,
            generation: 1,
            page_index: 0,
            execution_index: 0,
            transfer_index: u16::from(open),
        },
        source: [0; 32],
        destination: if open { id(8) } else { [0; 32] },
        source_vault_context: [0; 32],
        destination_vault_context: if open { id(4) } else { [0; 32] },
        mint: if open { id(9) } else { [0; 32] },
        token_program: if open { id(10) } else { [0; 32] },
        payer: id(11),
        rent_refund: id(12),
        expected_revision: u64::from(open),
        resulting_revision: u64::from(open) + 1,
        amount: 0,
        rent_lamports: 1,
    }
}

fn deposit_template()
-> Result<[u8; DELEGATED_CUSTODY_REQUEST_BYTES_V2], DirectRegisteredEffectArtifactErrorV4> {
    DelegatedCustodyRequestV2 {
        custody: CustodyRequestV1 {
            operation: OperationV1::Transfer,
            caller_role: CallerRoleV1::Trading,
            source_compartment: CompartmentV1::External,
            destination_compartment: CompartmentV1::TradingPrincipal,
            release_set: id(1),
            market: id(2),
            realm: id(3),
            context: id(4),
            caller_program: id(5),
            semantic: ContextV1 {
                candidate: [0; 32],
                source_owner: id(6),
                destination_owner: [0; 32],
                order: id(7),
                parent_request_digest: id(8),
                order_nonce: 1,
                generation: 1,
                page_index: 0,
                execution_index: 0,
                transfer_index: 2,
            },
            source: id(9),
            destination: id(10),
            source_vault_context: [0; 32],
            destination_vault_context: id(4),
            mint: id(11),
            token_program: id(12),
            payer: [0; 32],
            rent_refund: [0; 32],
            expected_revision: 2,
            resulting_revision: 3,
            amount: 1,
            rent_lamports: 0,
        },
        starts_atomic_debit: true,
        terminal: true,
        delegate_before: id(13),
        delegate_after: [0; 32],
        total_debit: 1,
        allowance_before: 1,
        allowance_after: 0,
    }
    .encode()
    .map_err(|_| DirectRegisteredEffectArtifactErrorV4::ChildRequest)
}

fn route<'a>(
    fixed_account_start: u16,
    fixed_account_count: u16,
    fixed_request: &'a [u8],
) -> RouteInputV3<'a> {
    RouteInputV3 {
        role: FixedRole::Custody,
        kind: RouteKindV3::Once,
        enable_common_scalar: None,
        witness_range_common_scalar: None,
        receipt_dependency: None,
        fixed_account_start,
        fixed_account_count,
        item_account_start: 0,
        item_account_count: 0,
        fixed_request,
        item_request: &[],
    }
}

fn push_request_identity(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
    route: u16,
    field: usize,
    value: usize,
) -> Result<(), DirectRegisteredEffectArtifactErrorV4> {
    push(
        output,
        next,
        EffectInstructionV3::write_request_identity(
            route,
            RequestSpaceV3::Fixed,
            offset(field)?,
            identity_coordinate(value)?,
        ),
    )
}

fn push_request_u64(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
    route: u16,
    field: usize,
    value: usize,
) -> Result<(), DirectRegisteredEffectArtifactErrorV4> {
    push(
        output,
        next,
        EffectInstructionV3::write_request_u64(
            route,
            RequestSpaceV3::Fixed,
            offset(field)?,
            scalar_coordinate(value)?,
        ),
    )
}

fn push(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
    instruction: EffectInstructionV3,
) -> Result<(), DirectRegisteredEffectArtifactErrorV4> {
    *output
        .get_mut(*next)
        .ok_or(DirectRegisteredEffectArtifactErrorV4::Coordinate)? = instruction;
    *next = next
        .checked_add(1)
        .ok_or(DirectRegisteredEffectArtifactErrorV4::Coordinate)?;
    Ok(())
}

fn scalar(value: usize) -> Result<u16, DirectRegisteredEffectArtifactErrorV4> {
    u16::try_from(value).map_err(|_| DirectRegisteredEffectArtifactErrorV4::Coordinate)
}
fn identity(value: usize) -> Result<u16, DirectRegisteredEffectArtifactErrorV4> {
    u16::try_from(value).map_err(|_| DirectRegisteredEffectArtifactErrorV4::Coordinate)
}
fn scalar_coordinate(
    value: usize,
) -> Result<ScalarCoordinateV3, DirectRegisteredEffectArtifactErrorV4> {
    scalar(value).map(ScalarCoordinateV3::common)
}
fn identity_coordinate(
    value: usize,
) -> Result<IdentityCoordinateV3, DirectRegisteredEffectArtifactErrorV4> {
    identity(value).map(IdentityCoordinateV3::common)
}
fn offset(value: usize) -> Result<u32, DirectRegisteredEffectArtifactErrorV4> {
    u32::try_from(value).map_err(|_| DirectRegisteredEffectArtifactErrorV4::Coordinate)
}
fn width16(value: usize) -> Result<u16, DirectRegisteredEffectArtifactErrorV4> {
    u16::try_from(value).map_err(|_| DirectRegisteredEffectArtifactErrorV4::Coordinate)
}
fn add(left: usize, right: usize) -> Result<usize, DirectRegisteredEffectArtifactErrorV4> {
    left.checked_add(right)
        .ok_or(DirectRegisteredEffectArtifactErrorV4::Coordinate)
}
const fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_v4_round_trips_exact_ordered_custody_chain() {
        let mut scratch = [0_u8; DIRECT_REGISTER_BUY_EFFECT_BYTES_V4];
        let mut output = [0_u8; DIRECT_REGISTER_BUY_EFFECT_BYTES_V4];
        encode_direct_register_buy_effect_v4_atomic(&mut scratch, &mut output).expect("effect");
        let program = ProgramV4::decode(&output).expect("decode");
        let base = program.base();
        assert_eq!(base.route_count(), 3);
        assert_eq!(
            base.fixed_account_count(),
            DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4
        );
        assert_eq!(base.route(0).expect("route0").receipt_dependency_count(), 0);
        assert_eq!(base.route(1).expect("route1").receipt_dependency_count(), 1);
        assert_eq!(base.route(2).expect("route2").receipt_dependency_count(), 2);
        assert_eq!(
            dclutch_custody_contract::DELEGATED_CUSTODY_RECEIPT_BYTES_V2,
            488
        );
    }

    #[test]
    fn wrong_width_preserves_output() {
        let mut scratch = [0_u8; DIRECT_REGISTER_BUY_EFFECT_BYTES_V4];
        let mut output = [0x55_u8; DIRECT_REGISTER_BUY_EFFECT_BYTES_V4 - 1];
        let before = output;
        assert_eq!(
            encode_direct_register_buy_effect_v4_atomic(&mut scratch, &mut output),
            Err(DirectRegisteredEffectArtifactErrorV4::Coordinate)
        );
        assert_eq!(output, before);
    }
}
