//! Exact fixed-role EffectProgram for inline ordinary Direct V3.
//!
//! The program first invokes the canonical fixed-width sparse Claims transfer,
//! then selects the exact positive Custody route shapes for seller-net and
//! combined-fee transfers. Four Custody route declarations are required because
//! the delegated-allowance wire binds start/terminal flags and the post-delegate
//! identity statically; their enable registers are mutually exclusive. Local
//! Direct root and maker candidates are written only after all selected child
//! receipts have been authenticated by the common Trading outer.

use dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1;
use dclutch_claims_svm::{
    CallerRole as ClaimsCallerRole,
    sparse_native_transfer_v1::{
        SPARSE_NATIVE_TRANSFER_BYTES_V1, SPARSE_NATIVE_TRANSFER_RECEIPT_BYTES_V1,
        SparseNativeTransferInputV1, SparseNativeTransferLayoutV1, SparseNativeTransferV1,
    },
};
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyRequestLayoutV1, CustodyRequestV1,
    DELEGATED_CUSTODY_RECEIPT_BYTES_V2, DELEGATED_CUSTODY_REQUEST_BYTES_V2,
    DelegatedCustodyRequestLayoutV2, DelegatedCustodyRequestV2, OperationV1,
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
    execution_v3::DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3,
    ordinary_v3::{
        DIRECT_ORDINARY_COMMON_IDENTITIES_V3, DIRECT_ORDINARY_COMMON_SCALARS_V3,
        DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3, DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3,
        IDENTITY_BUYER_MAKER_ROOT_V3, IDENTITY_BUYER_REQUEST_MAKER_V3,
        IDENTITY_BUYER_TOKEN_ACCOUNT_V3, IDENTITY_CUSTODY_AUTHORITY_V3, IDENTITY_FEE_RECIPIENT_V3,
        IDENTITY_FEE_TOKEN_ACCOUNT_V3, IDENTITY_LINKED_BASIS_RECORD_V3, IDENTITY_MARKET_V3,
        IDENTITY_MINT_V3, IDENTITY_PARENT_REQUEST_DIGEST_V3, IDENTITY_PRODUCT_RECORD_DIGEST_V3,
        IDENTITY_REALM_V3, IDENTITY_RELEASE_SET_V3, IDENTITY_SELLER_RENT_BENEFICIARY_V3,
        IDENTITY_SELLER_REQUEST_MAKER_V3, IDENTITY_SELLER_TOKEN_ACCOUNT_V3,
        IDENTITY_SEMANTIC_BASIS_V3, IDENTITY_TOKEN_PROGRAM_V3, IDENTITY_TRADING_PROGRAM_V3,
        SCALAR_BUYER_BUMP_V3, SCALAR_BUYER_DEBIT_V3, SCALAR_BUYER_NONCE_AFTER_V3,
        SCALAR_BUYER_NONCE_V3, SCALAR_BUYER_OUTCOME_V3, SCALAR_BUYER_POSITION_REVISION_V3,
        SCALAR_BUYER_RENT_PRINCIPAL_V3, SCALAR_CLAIM_TRANSFER_V3, SCALAR_CLAIMS_MARKET_REVISION_V3,
        SCALAR_COMBINED_FEE_V3, SCALAR_CUSTODY_AFTER_FEE_V3, SCALAR_CUSTODY_AFTER_SELLER_V3,
        SCALAR_CUSTODY_REVISION_V3, SCALAR_FEE_SOLE_ROUTE_ENABLED_V3, SCALAR_MAKER_MAGIC_V3,
        SCALAR_MAKER_VERSION_V3, SCALAR_MARKET_GENERATION_V3, SCALAR_OUTCOME_COUNT_V3,
        SCALAR_ROOT_OPEN_COUNT_AFTER_V3, SCALAR_SELLER_BUMP_V3,
        SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V3, SCALAR_SELLER_NET_V3,
        SCALAR_SELLER_NONCE_AFTER_V3, SCALAR_SELLER_OUTCOME_V3, SCALAR_SELLER_POSITION_REVISION_V3,
        SCALAR_SELLER_RENT_PRINCIPAL_V3, SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V3, SCALAR_ZERO_V3,
    },
    state_artifacts_v3::{DIRECT_BUYER_MAKER_ACCOUNT_V3, DIRECT_SELLER_MAKER_ACCOUNT_V3},
    successor::{DirectMakerReplayLayoutV1, DirectRootStateLayoutV1},
};

/// Logical account count of Profile9 for ordinary inline execution.
pub const DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3: u16 = 90;
/// Claims fixed22 frame start.
pub const DIRECT_INLINE_CLAIMS_ACCOUNT_START_V3: u16 = 12;
/// Seller-only terminal Custody frame start.
pub const DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3: u16 = 34;
/// Seller-before-fee Custody frame start.
pub const DIRECT_INLINE_SELLER_INTERMEDIATE_ACCOUNT_START_V3: u16 = 48;
/// Fee-after-seller Custody frame start.
pub const DIRECT_INLINE_FEE_CONTINUATION_ACCOUNT_START_V3: u16 = 62;
/// Fee-only Custody frame start.
pub const DIRECT_INLINE_FEE_SOLE_ACCOUNT_START_V3: u16 = 76;

const ROUTE_COUNT: usize = 5;
const DEPENDENCY_COUNT: usize = 5;
const FIXED_INSTRUCTION_COUNT: usize = 131;
const REQUEST_BANK_BYTES: usize =
    SPARSE_NATIVE_TRANSFER_BYTES_V1 + 4 * DELEGATED_CUSTODY_REQUEST_BYTES_V2;

const DIRECT_INLINE_ORDINARY_EFFECT_BASE_BYTES_V4: usize = HEADER_BYTES
    + ROUTE_COUNT * ROUTE_BYTES
    + DEPENDENCY_COUNT * RECEIPT_DEPENDENCY_BYTES
    + FIXED_INSTRUCTION_COUNT * OPERATION_BYTES
    + REQUEST_BANK_BYTES;
/// Exact zero-extension EffectProgram V4 envelope width.
pub const DIRECT_INLINE_ORDINARY_EFFECT_BYTES_V4: usize =
    HEADER_BYTES_V4 + DIRECT_INLINE_ORDINARY_EFFECT_BASE_BYTES_V4;

/// Stable no-allocation Effect artifact refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectOrdinaryEffectArtifactErrorV3 {
    /// A checked register, offset, or instruction count was not representable.
    Coordinate,
    /// A canonical child request template refused.
    ChildRequest,
    /// The Effect semantic-owner encoder or hostile decoder refused.
    Effect,
}

/// Emit the exact inline-ordinary EffectProgram atomically.
pub fn encode_direct_inline_ordinary_effect_v4_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectOrdinaryEffectArtifactErrorV3> {
    if scratch.len() != DIRECT_INLINE_ORDINARY_EFFECT_BYTES_V4
        || output.len() != DIRECT_INLINE_ORDINARY_EFFECT_BYTES_V4
    {
        return Err(DirectOrdinaryEffectArtifactErrorV3::Coordinate);
    }
    let mut base_scratch = [0_u8; DIRECT_INLINE_ORDINARY_EFFECT_BASE_BYTES_V4];
    let mut base = [0_u8; DIRECT_INLINE_ORDINARY_EFFECT_BASE_BYTES_V4];
    encode_direct_inline_ordinary_effect_base_v4_atomic(&mut base_scratch, &mut base)?;
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        u32::try_from(DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3)
            .map_err(|_| DirectOrdinaryEffectArtifactErrorV3::Coordinate)?,
        &[],
        &[],
        scratch,
        output,
    )
    .map_err(|_| DirectOrdinaryEffectArtifactErrorV3::Effect)?;
    ProgramV4::decode(output).map_err(|_| DirectOrdinaryEffectArtifactErrorV3::Effect)?;
    Ok(())
}

fn encode_direct_inline_ordinary_effect_base_v4_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectOrdinaryEffectArtifactErrorV3> {
    if scratch.len() != DIRECT_INLINE_ORDINARY_EFFECT_BASE_BYTES_V4
        || output.len() != DIRECT_INLINE_ORDINARY_EFFECT_BASE_BYTES_V4
    {
        return Err(DirectOrdinaryEffectArtifactErrorV3::Coordinate);
    }
    let claims = claims_template()?;
    let seller_terminal = custody_template(0, true, true, false)?;
    let seller_intermediate = custody_template(0, true, false, true)?;
    let fee_continuation = custody_template(1, false, true, false)?;
    let fee_sole = custody_template(0, true, true, false)?;
    let routes = [
        route(
            FixedRole::Claims,
            None,
            DIRECT_INLINE_CLAIMS_ACCOUNT_START_V3,
            22,
            &claims,
        ),
        route(
            FixedRole::Custody,
            Some(scalar(SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V3)?),
            DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3,
            14,
            &seller_terminal,
        ),
        route(
            FixedRole::Custody,
            Some(scalar(SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V3)?),
            DIRECT_INLINE_SELLER_INTERMEDIATE_ACCOUNT_START_V3,
            14,
            &seller_intermediate,
        ),
        route(
            FixedRole::Custody,
            Some(scalar(SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V3)?),
            DIRECT_INLINE_FEE_CONTINUATION_ACCOUNT_START_V3,
            14,
            &fee_continuation,
        ),
        route(
            FixedRole::Custody,
            Some(scalar(SCALAR_FEE_SOLE_ROUTE_ENABLED_V3)?),
            DIRECT_INLINE_FEE_SOLE_ACCOUNT_START_V3,
            14,
            &fee_sole,
        ),
    ];
    let claims_dependency = RouteReceiptDependencyV3::new(
        FixedRole::Claims,
        0,
        width16(SPARSE_NATIVE_TRANSFER_RECEIPT_BYTES_V1)?,
    );
    let seller_dependency = RouteReceiptDependencyV3::new(
        FixedRole::Custody,
        2,
        width16(DELEGATED_CUSTODY_RECEIPT_BYTES_V2)?,
    );
    let route1 = [claims_dependency];
    let route2 = [claims_dependency];
    let route3 = [claims_dependency, seller_dependency];
    let route4 = [claims_dependency];
    let dependencies: [&[RouteReceiptDependencyV3]; ROUTE_COUNT] =
        [&[], &route1, &route2, &route3, &route4];
    let instructions = effect_instructions()?;
    encode_effect_program_v4_atomic(
        EffectGeometryV3 {
            fixed_accounts: DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3,
            item_account_stride: 0,
            common_scalars: scalar(DIRECT_ORDINARY_COMMON_SCALARS_V3)?,
            item_scalar_stride: DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3,
            common_identities: identity(DIRECT_ORDINARY_COMMON_IDENTITIES_V3)?,
            item_identity_stride: DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3,
        },
        &routes,
        &dependencies,
        &instructions,
        &[],
        scratch,
        output,
    )
    .map_err(|_| DirectOrdinaryEffectArtifactErrorV3::Effect)?;
    ProgramV3::decode(output).map_err(|_| DirectOrdinaryEffectArtifactErrorV3::Effect)?;
    Ok(())
}

fn effect_instructions()
-> Result<[EffectInstructionV3; FIXED_INSTRUCTION_COUNT], DirectOrdinaryEffectArtifactErrorV3> {
    let placeholder = EffectInstructionV3::write_u64(
        AccountCoordinateV3::fixed(0),
        0,
        ScalarCoordinateV3::common(0),
    );
    let mut output = [placeholder; FIXED_INSTRUCTION_COUNT];
    let mut next = 0_usize;
    push_local_state(&mut output, &mut next)?;
    push_claims_request(&mut output, &mut next)?;
    push_custody_request(
        &mut output,
        &mut next,
        1,
        IDENTITY_SELLER_REQUEST_MAKER_V3,
        IDENTITY_SELLER_TOKEN_ACCOUNT_V3,
        SCALAR_CUSTODY_REVISION_V3,
        SCALAR_CUSTODY_AFTER_SELLER_V3,
        SCALAR_SELLER_NET_V3,
        SCALAR_BUYER_DEBIT_V3,
        SCALAR_BUYER_DEBIT_V3,
        SCALAR_ZERO_V3,
        false,
    )?;
    push_custody_request(
        &mut output,
        &mut next,
        2,
        IDENTITY_SELLER_REQUEST_MAKER_V3,
        IDENTITY_SELLER_TOKEN_ACCOUNT_V3,
        SCALAR_CUSTODY_REVISION_V3,
        SCALAR_CUSTODY_AFTER_SELLER_V3,
        SCALAR_SELLER_NET_V3,
        SCALAR_BUYER_DEBIT_V3,
        SCALAR_BUYER_DEBIT_V3,
        SCALAR_COMBINED_FEE_V3,
        true,
    )?;
    push_custody_request(
        &mut output,
        &mut next,
        3,
        IDENTITY_FEE_RECIPIENT_V3,
        IDENTITY_FEE_TOKEN_ACCOUNT_V3,
        SCALAR_CUSTODY_AFTER_SELLER_V3,
        SCALAR_CUSTODY_AFTER_FEE_V3,
        SCALAR_COMBINED_FEE_V3,
        SCALAR_BUYER_DEBIT_V3,
        SCALAR_COMBINED_FEE_V3,
        SCALAR_ZERO_V3,
        false,
    )?;
    push_custody_request(
        &mut output,
        &mut next,
        4,
        IDENTITY_FEE_RECIPIENT_V3,
        IDENTITY_FEE_TOKEN_ACCOUNT_V3,
        SCALAR_CUSTODY_REVISION_V3,
        SCALAR_CUSTODY_AFTER_FEE_V3,
        SCALAR_COMBINED_FEE_V3,
        SCALAR_BUYER_DEBIT_V3,
        SCALAR_BUYER_DEBIT_V3,
        SCALAR_ZERO_V3,
        false,
    )?;
    if next != output.len() {
        return Err(DirectOrdinaryEffectArtifactErrorV3::Coordinate);
    }
    Ok(output)
}

fn push_local_state(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
) -> Result<(), DirectOrdinaryEffectArtifactErrorV3> {
    push(
        output,
        next,
        EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(0),
            offset(
                CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT,
            )?,
            scalar_coordinate(SCALAR_ROOT_OPEN_COUNT_AFTER_V3)?,
        ),
    )?;
    push_maker_state(
        output,
        next,
        DIRECT_SELLER_MAKER_ACCOUNT_V3,
        IDENTITY_SELLER_REQUEST_MAKER_V3,
        IDENTITY_SELLER_RENT_BENEFICIARY_V3,
        SCALAR_SELLER_BUMP_V3,
        SCALAR_SELLER_NONCE_AFTER_V3,
        SCALAR_SELLER_RENT_PRINCIPAL_V3,
    )?;
    push_maker_state(
        output,
        next,
        DIRECT_BUYER_MAKER_ACCOUNT_V3,
        IDENTITY_BUYER_REQUEST_MAKER_V3,
        crate::ordinary_v3::IDENTITY_BUYER_RENT_BENEFICIARY_V3,
        SCALAR_BUYER_BUMP_V3,
        SCALAR_BUYER_NONCE_AFTER_V3,
        SCALAR_BUYER_RENT_PRINCIPAL_V3,
    )
}

#[allow(clippy::too_many_arguments)]
fn push_maker_state(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
    account: u16,
    maker: usize,
    rent_owner: usize,
    bump: usize,
    nonce_after: usize,
    rent_principal: usize,
) -> Result<(), DirectOrdinaryEffectArtifactErrorV3> {
    let account = AccountCoordinateV3::fixed(account);
    for instruction in [
        EffectInstructionV3::write_u64(
            account,
            offset(DirectMakerReplayLayoutV1::MAGIC)?,
            scalar_coordinate(SCALAR_MAKER_MAGIC_V3)?,
        ),
        EffectInstructionV3::write_u16(
            account,
            offset(DirectMakerReplayLayoutV1::VERSION)?,
            scalar_coordinate(SCALAR_MAKER_VERSION_V3)?,
        ),
        EffectInstructionV3::write_u8(
            account,
            offset(DirectMakerReplayLayoutV1::BUMP)?,
            scalar_coordinate(bump)?,
        ),
        EffectInstructionV3::write_identity(
            account,
            offset(DirectMakerReplayLayoutV1::MARKET)?,
            identity_coordinate(IDENTITY_MARKET_V3)?,
        ),
        EffectInstructionV3::write_u64(
            account,
            offset(DirectMakerReplayLayoutV1::GENERATION)?,
            scalar_coordinate(SCALAR_MARKET_GENERATION_V3)?,
        ),
        EffectInstructionV3::write_identity(
            account,
            offset(DirectMakerReplayLayoutV1::MAKER)?,
            identity_coordinate(maker)?,
        ),
        EffectInstructionV3::write_u64(
            account,
            offset(DirectMakerReplayLayoutV1::NEXT_NONCE)?,
            scalar_coordinate(nonce_after)?,
        ),
        EffectInstructionV3::write_u64(
            account,
            offset(DirectMakerReplayLayoutV1::LIVE_COUNT)?,
            scalar_coordinate(SCALAR_ZERO_V3)?,
        ),
        EffectInstructionV3::write_u64(
            account,
            offset(DirectMakerReplayLayoutV1::MINIMUM_LIVE_NONCE)?,
            scalar_coordinate(SCALAR_ZERO_V3)?,
        ),
        EffectInstructionV3::write_identity(
            account,
            offset(DirectMakerReplayLayoutV1::RENT_OWNER)?,
            identity_coordinate(rent_owner)?,
        ),
        EffectInstructionV3::write_u64(
            account,
            offset(DirectMakerReplayLayoutV1::RENT_PRINCIPAL)?,
            scalar_coordinate(rent_principal)?,
        ),
    ] {
        push(output, next, instruction)?;
    }
    Ok(())
}

fn push_claims_request(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
) -> Result<(), DirectOrdinaryEffectArtifactErrorV3> {
    for (field, value) in [
        (
            SparseNativeTransferLayoutV1::RELEASE_SET,
            IDENTITY_RELEASE_SET_V3,
        ),
        (SparseNativeTransferLayoutV1::MARKET, IDENTITY_MARKET_V3),
        (
            SparseNativeTransferLayoutV1::REQUEST_ID,
            IDENTITY_PARENT_REQUEST_DIGEST_V3,
        ),
        (
            SparseNativeTransferLayoutV1::PRODUCT_RECORD,
            IDENTITY_PRODUCT_RECORD_DIGEST_V3,
        ),
        (
            SparseNativeTransferLayoutV1::SEMANTIC_BASIS,
            IDENTITY_SEMANTIC_BASIS_V3,
        ),
        (
            SparseNativeTransferLayoutV1::LINKED_BASIS_RECORD,
            IDENTITY_LINKED_BASIS_RECORD_V3,
        ),
        (
            SparseNativeTransferLayoutV1::SOURCE_OWNER,
            IDENTITY_SELLER_REQUEST_MAKER_V3,
        ),
        (
            SparseNativeTransferLayoutV1::DESTINATION_OWNER,
            IDENTITY_BUYER_REQUEST_MAKER_V3,
        ),
    ] {
        push_request_identity(output, next, 0, field, value)?;
    }
    for (field, value) in [
        (
            SparseNativeTransferLayoutV1::MARKET_REVISION,
            SCALAR_CLAIMS_MARKET_REVISION_V3,
        ),
        (
            SparseNativeTransferLayoutV1::SOURCE_REVISION,
            SCALAR_SELLER_POSITION_REVISION_V3,
        ),
        (
            SparseNativeTransferLayoutV1::DESTINATION_REVISION,
            SCALAR_BUYER_POSITION_REVISION_V3,
        ),
        (
            SparseNativeTransferLayoutV1::GENERATION,
            SCALAR_MARKET_GENERATION_V3,
        ),
        (
            SparseNativeTransferLayoutV1::QUANTITY,
            SCALAR_CLAIM_TRANSFER_V3,
        ),
    ] {
        push_request_u64(output, next, 0, field, value)?;
    }
    for (field, value) in [
        (
            SparseNativeTransferLayoutV1::OUTCOME,
            SCALAR_SELLER_OUTCOME_V3,
        ),
        (
            SparseNativeTransferLayoutV1::CLAIM_COUNT,
            SCALAR_OUTCOME_COUNT_V3,
        ),
    ] {
        push(
            output,
            next,
            EffectInstructionV3::write_request_u32(
                0,
                RequestSpaceV3::Fixed,
                offset(field)?,
                scalar_coordinate(value)?,
            ),
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_custody_request(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
    route: u16,
    destination_owner: usize,
    destination: usize,
    expected_revision: usize,
    resulting_revision: usize,
    amount: usize,
    total_debit: usize,
    allowance_before: usize,
    allowance_after: usize,
    retain_delegate: bool,
) -> Result<(), DirectOrdinaryEffectArtifactErrorV3> {
    let base = DelegatedCustodyRequestLayoutV2::BASE;
    for (field, value) in [
        (CustodyRequestLayoutV1::RELEASE_SET, IDENTITY_RELEASE_SET_V3),
        (CustodyRequestLayoutV1::MARKET, IDENTITY_MARKET_V3),
        (CustodyRequestLayoutV1::REALM, IDENTITY_REALM_V3),
        (
            CustodyRequestLayoutV1::CONTEXT,
            IDENTITY_BUYER_MAKER_ROOT_V3,
        ),
        (
            CustodyRequestLayoutV1::CALLER_PROGRAM,
            IDENTITY_TRADING_PROGRAM_V3,
        ),
        (
            CustodyRequestLayoutV1::SOURCE_OWNER,
            IDENTITY_BUYER_REQUEST_MAKER_V3,
        ),
        (CustodyRequestLayoutV1::DESTINATION_OWNER, destination_owner),
        (
            CustodyRequestLayoutV1::ORDER,
            IDENTITY_PARENT_REQUEST_DIGEST_V3,
        ),
        (
            CustodyRequestLayoutV1::PARENT_REQUEST_DIGEST,
            IDENTITY_PARENT_REQUEST_DIGEST_V3,
        ),
        (
            CustodyRequestLayoutV1::SOURCE,
            IDENTITY_BUYER_TOKEN_ACCOUNT_V3,
        ),
        (CustodyRequestLayoutV1::DESTINATION, destination),
        (CustodyRequestLayoutV1::MINT, IDENTITY_MINT_V3),
        (
            CustodyRequestLayoutV1::TOKEN_PROGRAM,
            IDENTITY_TOKEN_PROGRAM_V3,
        ),
    ] {
        push_request_identity(output, next, route, add(base, field)?, value)?;
    }
    for (field, value) in [
        (CustodyRequestLayoutV1::ORDER_NONCE, SCALAR_BUYER_NONCE_V3),
        (
            CustodyRequestLayoutV1::GENERATION,
            SCALAR_MARKET_GENERATION_V3,
        ),
        (CustodyRequestLayoutV1::EXPECTED_REVISION, expected_revision),
        (
            CustodyRequestLayoutV1::RESULTING_REVISION,
            resulting_revision,
        ),
        (CustodyRequestLayoutV1::AMOUNT, amount),
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
            scalar_coordinate(SCALAR_BUYER_OUTCOME_V3)?,
        ),
    )?;
    push_request_identity(
        output,
        next,
        route,
        DelegatedCustodyRequestLayoutV2::DELEGATE_BEFORE,
        IDENTITY_CUSTODY_AUTHORITY_V3,
    )?;
    if retain_delegate {
        push_request_identity(
            output,
            next,
            route,
            DelegatedCustodyRequestLayoutV2::DELEGATE_AFTER,
            IDENTITY_CUSTODY_AUTHORITY_V3,
        )?;
    }
    for (field, value) in [
        (DelegatedCustodyRequestLayoutV2::TOTAL_DEBIT, total_debit),
        (
            DelegatedCustodyRequestLayoutV2::ALLOWANCE_BEFORE,
            allowance_before,
        ),
        (
            DelegatedCustodyRequestLayoutV2::ALLOWANCE_AFTER,
            allowance_after,
        ),
    ] {
        push_request_u64(output, next, route, field, value)?;
    }
    Ok(())
}

fn claims_template()
-> Result<[u8; SPARSE_NATIVE_TRANSFER_BYTES_V1], DirectOrdinaryEffectArtifactErrorV3> {
    SparseNativeTransferV1::new(SparseNativeTransferInputV1 {
        caller_role: ClaimsCallerRole::Trading,
        release_set: id(1),
        market: id(2),
        request_id: id(3),
        product_record_digest: id(4),
        semantic_basis_id: id(5),
        linked_basis_record_digest: id(6),
        source_owner: id(7),
        destination_owner: id(8),
        expected_market_revision: 1,
        expected_source_revision: 1,
        expected_destination_revision: 1,
        generation: 1,
        outcome: 0,
        claim_count: 1,
        quantity: 1,
    })
    .map(SparseNativeTransferV1::to_bytes)
    .map_err(|_| DirectOrdinaryEffectArtifactErrorV3::ChildRequest)
}

fn custody_template(
    transfer_index: u16,
    starts_atomic_debit: bool,
    terminal: bool,
    retain_delegate: bool,
) -> Result<[u8; DELEGATED_CUSTODY_REQUEST_BYTES_V2], DirectOrdinaryEffectArtifactErrorV3> {
    DelegatedCustodyRequestV2 {
        custody: CustodyRequestV1 {
            operation: OperationV1::Transfer,
            caller_role: CallerRoleV1::Trading,
            source_compartment: CompartmentV1::External,
            destination_compartment: CompartmentV1::External,
            release_set: id(1),
            market: id(2),
            realm: id(3),
            context: id(4),
            caller_program: id(5),
            semantic: ContextV1 {
                candidate: [0; 32],
                source_owner: id(6),
                destination_owner: id(7),
                order: id(8),
                parent_request_digest: id(8),
                order_nonce: 1,
                generation: 1,
                page_index: 0,
                execution_index: 0,
                transfer_index,
            },
            source: id(9),
            destination: id(10),
            source_vault_context: [0; 32],
            destination_vault_context: [0; 32],
            mint: id(11),
            token_program: id(12),
            payer: [0; 32],
            rent_refund: [0; 32],
            expected_revision: 1,
            resulting_revision: 2,
            amount: 1,
            rent_lamports: 0,
        },
        starts_atomic_debit,
        terminal,
        delegate_before: id(13),
        delegate_after: if retain_delegate { id(13) } else { [0; 32] },
        total_debit: if starts_atomic_debit && terminal {
            1
        } else {
            2
        },
        allowance_before: if terminal { 1 } else { 2 },
        allowance_after: if terminal { 0 } else { 1 },
    }
    .encode()
    .map_err(|_| DirectOrdinaryEffectArtifactErrorV3::ChildRequest)
}

fn route<'a>(
    role: FixedRole,
    enable_common_scalar: Option<u16>,
    fixed_account_start: u16,
    fixed_account_count: u16,
    fixed_request: &'a [u8],
) -> RouteInputV3<'a> {
    RouteInputV3 {
        role,
        kind: RouteKindV3::Once,
        enable_common_scalar,
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
) -> Result<(), DirectOrdinaryEffectArtifactErrorV3> {
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
) -> Result<(), DirectOrdinaryEffectArtifactErrorV3> {
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
) -> Result<(), DirectOrdinaryEffectArtifactErrorV3> {
    *output
        .get_mut(*next)
        .ok_or(DirectOrdinaryEffectArtifactErrorV3::Coordinate)? = instruction;
    *next = next
        .checked_add(1)
        .ok_or(DirectOrdinaryEffectArtifactErrorV3::Coordinate)?;
    Ok(())
}

fn scalar(value: usize) -> Result<u16, DirectOrdinaryEffectArtifactErrorV3> {
    u16::try_from(value).map_err(|_| DirectOrdinaryEffectArtifactErrorV3::Coordinate)
}

fn identity(value: usize) -> Result<u16, DirectOrdinaryEffectArtifactErrorV3> {
    u16::try_from(value).map_err(|_| DirectOrdinaryEffectArtifactErrorV3::Coordinate)
}

fn scalar_coordinate(
    value: usize,
) -> Result<ScalarCoordinateV3, DirectOrdinaryEffectArtifactErrorV3> {
    scalar(value).map(ScalarCoordinateV3::common)
}

fn identity_coordinate(
    value: usize,
) -> Result<IdentityCoordinateV3, DirectOrdinaryEffectArtifactErrorV3> {
    identity(value).map(IdentityCoordinateV3::common)
}

fn offset(value: usize) -> Result<u32, DirectOrdinaryEffectArtifactErrorV3> {
    u32::try_from(value).map_err(|_| DirectOrdinaryEffectArtifactErrorV3::Coordinate)
}

fn width16(value: usize) -> Result<u16, DirectOrdinaryEffectArtifactErrorV3> {
    u16::try_from(value).map_err(|_| DirectOrdinaryEffectArtifactErrorV3::Coordinate)
}

fn add(left: usize, right: usize) -> Result<usize, DirectOrdinaryEffectArtifactErrorV3> {
    left.checked_add(right)
        .ok_or(DirectOrdinaryEffectArtifactErrorV3::Coordinate)
}

const fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn exact_effect_round_trips_ordered_claims_and_custody_routes() {
        let mut scratch = [0_u8; DIRECT_INLINE_ORDINARY_EFFECT_BYTES_V4];
        let mut output = [0x55_u8; DIRECT_INLINE_ORDINARY_EFFECT_BYTES_V4];
        encode_direct_inline_ordinary_effect_v4_atomic(&mut scratch, &mut output).expect("effect");
        let effect = ProgramV4::decode(&output).expect("decode");
        assert_eq!(effect.span_count(), 0);
        assert_eq!(effect.range_count(), 0);
        assert_eq!(
            effect.semantic_prefix_bytes(),
            u32::try_from(DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3).expect("request width")
        );
        let effect = effect.base();
        assert_eq!(effect.route_count(), 5);
        assert_eq!(effect.fixed_account_count(), 90);
        assert_eq!(effect.common_scalar_count(), 65);
        assert_eq!(effect.item_scalar_stride(), 2);
        assert_eq!(effect.common_identity_count(), 32);
        assert_eq!(effect.request_bytes(0).expect("request bank"), 3_424);
        assert_eq!(effect.fixed_operation_count(), 131);
        assert_eq!(
            effect.route(0).expect("Claims route").role(),
            FixedRole::Claims
        );
        assert_eq!(
            effect
                .route(3)
                .expect("fee continuation")
                .receipt_dependency_count(),
            2
        );
    }

    #[test]
    fn wrong_width_refuses_without_output_mutation() {
        let mut scratch = [0_u8; DIRECT_INLINE_ORDINARY_EFFECT_BYTES_V4 - 1];
        let mut output = [0x66_u8; DIRECT_INLINE_ORDINARY_EFFECT_BYTES_V4];
        let before = output;
        assert_eq!(
            encode_direct_inline_ordinary_effect_v4_atomic(&mut scratch, &mut output),
            Err(DirectOrdinaryEffectArtifactErrorV3::Coordinate)
        );
        assert_eq!(output, before);
    }
}
