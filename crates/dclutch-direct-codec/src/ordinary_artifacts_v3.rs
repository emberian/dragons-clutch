//! Typed RequestProfile artifact for signed inline ordinary Direct V3.
//!
//! The embedded V1 program validates and projects the complete selected Direct
//! request. The V2 wrapper additionally requires the two adjacent native
//! Ed25519 messages and places their signers in distinct registers. Transition
//! semantics then require those signers to equal the request-carried makers.

use dclutch_capability_program_contract::hot_v3::HOT_FAMILY_REQUEST_OFFSET_V3;
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_request_profile_contract::{
    encode::{
        IdentityRegisterV1, RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1,
        ScalarRegisterV1, encode_request_profile_v1_atomic,
    },
    v2::{NativeSignatureRequirementV1, encode_request_profile_v2_atomic},
};

use crate::{
    execution_v3::{
        DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3, DIRECT_EXECUTION_REQUEST_MAGIC_V3,
        DIRECT_EXECUTION_REQUEST_VERSION_V3, DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
        DirectExecutionActionV3, native_signature_slice_v3,
    },
    generated_intent_v2 as intent,
    ordinary_v3::{
        DIRECT_ORDINARY_COMMON_IDENTITIES_V3, DIRECT_ORDINARY_COMMON_SCALARS_V3,
        DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3, DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3,
        IDENTITY_BUYER_COLLATERAL_REQUEST_V3, IDENTITY_BUYER_INTENT_MARKET_V3,
        IDENTITY_BUYER_NATIVE_SIGNER_V3, IDENTITY_BUYER_REQUEST_MAKER_V3,
        IDENTITY_SELLER_COLLATERAL_REQUEST_V3, IDENTITY_SELLER_INTENT_MARKET_V3,
        IDENTITY_SELLER_NATIVE_SIGNER_V3, IDENTITY_SELLER_REQUEST_MAKER_V3,
        SCALAR_BUYER_FEE_BPS_V3, SCALAR_BUYER_GENERATION_V3, SCALAR_BUYER_LIFECYCLE_V3,
        SCALAR_BUYER_LIMIT_V3, SCALAR_BUYER_MAXIMUM_V3, SCALAR_BUYER_NONCE_V3,
        SCALAR_BUYER_OUTCOME_V3, SCALAR_BUYER_SIDE_V3, SCALAR_BUYER_VALID_FROM_V3,
        SCALAR_BUYER_VALID_THROUGH_V3, SCALAR_EXECUTION_PRICE_V3, SCALAR_FILL_V3,
        SCALAR_SELLER_FEE_BPS_V3, SCALAR_SELLER_GENERATION_V3, SCALAR_SELLER_LIFECYCLE_V3,
        SCALAR_SELLER_LIMIT_V3, SCALAR_SELLER_MAXIMUM_V3, SCALAR_SELLER_NONCE_V3,
        SCALAR_SELLER_OUTCOME_V3, SCALAR_SELLER_SIDE_V3, SCALAR_SELLER_VALID_FROM_V3,
        SCALAR_SELLER_VALID_THROUGH_V3,
    },
};

const INLINE_ORDINARY_REQUEST_BYTES_V3: usize =
    DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + 2 * DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 16;
const INLINE_ORDINARY_REQUEST_OPERATIONS_V3: usize = 50;

/// Exact embedded RequestProfile V1 byte width.
pub const DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V1_BYTES_V3: usize =
    dclutch_request_profile_contract::HEADER_BYTES
        + INLINE_ORDINARY_REQUEST_OPERATIONS_V3 * dclutch_request_profile_contract::OPERATION_BYTES;
/// Exact signed RequestProfile V2 byte width.
pub const DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V2_BYTES_V3: usize =
    dclutch_request_profile_contract::v2::REQUEST_PROFILE_V2_HEADER_BYTES
        + DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V1_BYTES_V3
        + 2 * dclutch_request_profile_contract::v2::NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1;
/// SHA-256 content identity of the exact emitted signed RequestProfile V2.
pub const DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_ID_V3: [u8; 32] = [
    0x60, 0x15, 0xce, 0xda, 0x5a, 0xbf, 0x1d, 0x01, 0x49, 0xc3, 0x08, 0x76, 0x8e, 0xae, 0x56, 0xda,
    0xc4, 0xb7, 0x66, 0xc4, 0xca, 0x42, 0x4e, 0xfc, 0xb5, 0xa5, 0xc5, 0x67, 0x7c, 0x5e, 0x94, 0xc9,
];
/// SHA-256 content identity of the exact ordinary TransitionVM V3 program.
pub const DIRECT_INLINE_ORDINARY_TRANSITION_ID_V3: [u8; 32] = [
    0xb8, 0xbb, 0xa5, 0x93, 0x6e, 0x61, 0x4a, 0xd5, 0xf9, 0xbb, 0x1e, 0xd3, 0x2e, 0x6c, 0x8a, 0xcc,
    0x84, 0x5e, 0xc1, 0x73, 0x2d, 0xa3, 0x9a, 0xca, 0xa9, 0x53, 0x54, 0xde, 0x50, 0x18, 0x00, 0xbf,
];
/// SHA-256 content identity of the interpreted Strategy V2 selecting that transition.
pub const DIRECT_INLINE_ORDINARY_STRATEGY_ID_V3: [u8; 32] = [
    0xbb, 0xde, 0xab, 0xcf, 0x02, 0x8a, 0xa4, 0xc2, 0x3d, 0x20, 0xe7, 0x00, 0x8a, 0x9e, 0x77, 0x11,
    0xf9, 0xb5, 0x4d, 0x8f, 0xa7, 0x44, 0x0b, 0x7f, 0x9e, 0x3e, 0x73, 0x2b, 0x72, 0x28, 0x48, 0xd5,
];

/// Stable typed Direct artifact-emission refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectOrdinaryArtifactErrorV3 {
    /// A checked request, register, or instruction coordinate was not representable.
    Coordinate,
    /// RequestProfile V1 hostile encoding refused.
    RequestProfileV1,
    /// Signed RequestProfile V2 hostile encoding refused.
    RequestProfileV2,
    /// Exact interpreted ExecutionStrategy construction refused.
    Strategy,
}

/// Construct the exact interpreted Strategy V2 selecting the ordinary transition.
pub fn direct_inline_ordinary_strategy_v3()
-> Result<[u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2], DirectOrdinaryArtifactErrorV3> {
    let content =
        |value| ContentId::new(value).map_err(|_| DirectOrdinaryArtifactErrorV3::Strategy);
    ExecutionStrategyProgramV2::new(
        StrategyDispositionV2::Interpreted,
        content(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)?,
        content(DIRECT_INLINE_ORDINARY_TRANSITION_ID_V3)?,
        content(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)?,
        None,
        content(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2)?,
        None,
        content(ACCELERATOR_REQUEST_SCHEMA_ID_V2)?,
        content(ACCELERATOR_ACK_SCHEMA_ID_V2)?,
    )
    .map(ExecutionStrategyProgramV2::to_bytes)
    .map_err(|_| DirectOrdinaryArtifactErrorV3::Strategy)
}

/// Emit the exact signed inline-ordinary RequestProfile V2 atomically.
///
/// `v1_scratch` and `v1_candidate` are caller-owned temporary buffers. Only
/// `output` is the final artifact and remains unchanged if V2 construction
/// refuses. Every buffer must have its corresponding exact public width.
pub fn encode_inline_ordinary_request_profile_v3_atomic(
    v1_scratch: &mut [u8],
    v1_candidate: &mut [u8],
    v2_scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectOrdinaryArtifactErrorV3> {
    let operations = inline_ordinary_operations()?;
    let geometry = RequestGeometryV1::new(
        coordinate(INLINE_ORDINARY_REQUEST_BYTES_V3)?,
        0,
        register(DIRECT_ORDINARY_COMMON_SCALARS_V3)?,
        DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3,
        register(DIRECT_ORDINARY_COMMON_IDENTITIES_V3)?,
        DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3,
    );
    encode_request_profile_v1_atomic(geometry, &operations, &[], v1_scratch, v1_candidate)
        .map_err(|_| DirectOrdinaryArtifactErrorV3::RequestProfileV1)?;

    let seller = native_signature_slice_v3(DirectExecutionActionV3::InlineOrdinary, 0, 0)
        .map_err(|_| DirectOrdinaryArtifactErrorV3::Coordinate)?;
    let buyer = native_signature_slice_v3(DirectExecutionActionV3::InlineOrdinary, 0, 1)
        .map_err(|_| DirectOrdinaryArtifactErrorV3::Coordinate)?;
    let requirements = [
        NativeSignatureRequirementV1::new(
            absolute_message_offset(seller.message_offset)?,
            seller.message_bytes,
            identity_destination(IDENTITY_SELLER_NATIVE_SIGNER_V3)?,
        ),
        NativeSignatureRequirementV1::new(
            absolute_message_offset(buyer.message_offset)?,
            buyer.message_bytes,
            identity_destination(IDENTITY_BUYER_NATIVE_SIGNER_V3)?,
        ),
    ];
    encode_request_profile_v2_atomic(v1_candidate, &requirements, v2_scratch, output)
        .map_err(|_| DirectOrdinaryArtifactErrorV3::RequestProfileV2)
}

fn inline_ordinary_operations() -> Result<
    [RequestInstructionV1; INLINE_ORDINARY_REQUEST_OPERATIONS_V3],
    DirectOrdinaryArtifactErrorV3,
> {
    let seller = participant_offsets(32)?;
    let buyer = participant_offsets(
        32_usize
            .checked_add(DIRECT_SIGNED_PARTICIPANT_BYTES_V3)
            .ok_or(DirectOrdinaryArtifactErrorV3::Coordinate)?,
    )?;
    Ok([
        require_u64(0, u64::from_le_bytes(DIRECT_EXECUTION_REQUEST_MAGIC_V3))?,
        require_u16(8, DIRECT_EXECUTION_REQUEST_VERSION_V3)?,
        require_u16(10, 0)?,
        require_u32(12, DirectExecutionActionV3::InlineOrdinary as u32)?,
        require_u32(
            16,
            coordinate(
                INLINE_ORDINARY_REQUEST_BYTES_V3 - DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3,
            )?,
        )?,
        RequestInstructionV1::require_zero(request(20)?, 12),
        require_u64(seller.domain, domain_word(0)?)?,
        require_u64(seller.domain + 8, domain_word(8)?)?,
        require_u64(seller.domain + 16, domain_word(16)?)?,
        require_u64(seller.domain + 24, domain_word(24)?)?,
        require_u64(
            seller.intent,
            u64::from_le_bytes(intent::COMPACT_INTENT_MAGIC_V2),
        )?,
        require_u16(
            seller.intent + intent::COMPACT_INTENT_VERSION_OFFSET_V2,
            intent::COMPACT_INTENT_VERSION_V2,
        )?,
        RequestInstructionV1::require_zero(
            request(seller.intent + intent::COMPACT_INTENT_RESERVED_A_OFFSET_V2)?,
            4,
        ),
        RequestInstructionV1::require_zero(
            request(seller.intent + intent::COMPACT_INTENT_RESERVED_B_OFFSET_V2)?,
            6,
        ),
        project_identity(seller.maker, IDENTITY_SELLER_REQUEST_MAKER_V3)?,
        project_u8(
            seller.intent + intent::COMPACT_INTENT_SIDE_OFFSET_V2,
            SCALAR_SELLER_SIDE_V3,
        )?,
        project_u8(
            seller.intent + intent::COMPACT_INTENT_LIFECYCLE_OFFSET_V2,
            SCALAR_SELLER_LIFECYCLE_V3,
        )?,
        project_u32(
            seller.intent + intent::COMPACT_INTENT_OUTCOME_OFFSET_V2,
            SCALAR_SELLER_OUTCOME_V3,
        )?,
        project_identity(
            seller.intent + intent::COMPACT_INTENT_MARKET_OFFSET_V2,
            IDENTITY_SELLER_INTENT_MARKET_V3,
        )?,
        project_u64(
            seller.intent + intent::COMPACT_INTENT_GENERATION_OFFSET_V2,
            SCALAR_SELLER_GENERATION_V3,
        )?,
        project_u64(
            seller.intent + intent::COMPACT_INTENT_NONCE_OFFSET_V2,
            SCALAR_SELLER_NONCE_V3,
        )?,
        project_u64(
            seller.intent + intent::COMPACT_INTENT_VALID_FROM_OFFSET_V2,
            SCALAR_SELLER_VALID_FROM_V3,
        )?,
        project_u64(
            seller.intent + intent::COMPACT_INTENT_VALID_THROUGH_OFFSET_V2,
            SCALAR_SELLER_VALID_THROUGH_V3,
        )?,
        project_u64(
            seller.intent + intent::COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2,
            SCALAR_SELLER_MAXIMUM_V3,
        )?,
        project_u64(
            seller.intent + intent::COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2,
            SCALAR_SELLER_LIMIT_V3,
        )?,
        project_u16(
            seller.intent + intent::COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2,
            SCALAR_SELLER_FEE_BPS_V3,
        )?,
        project_identity(
            seller.intent + intent::COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2,
            IDENTITY_SELLER_COLLATERAL_REQUEST_V3,
        )?,
        require_u64(buyer.domain, domain_word(0)?)?,
        require_u64(buyer.domain + 8, domain_word(8)?)?,
        require_u64(buyer.domain + 16, domain_word(16)?)?,
        require_u64(buyer.domain + 24, domain_word(24)?)?,
        require_u64(
            buyer.intent,
            u64::from_le_bytes(intent::COMPACT_INTENT_MAGIC_V2),
        )?,
        require_u16(
            buyer.intent + intent::COMPACT_INTENT_VERSION_OFFSET_V2,
            intent::COMPACT_INTENT_VERSION_V2,
        )?,
        RequestInstructionV1::require_zero(
            request(buyer.intent + intent::COMPACT_INTENT_RESERVED_A_OFFSET_V2)?,
            4,
        ),
        RequestInstructionV1::require_zero(
            request(buyer.intent + intent::COMPACT_INTENT_RESERVED_B_OFFSET_V2)?,
            6,
        ),
        project_identity(buyer.maker, IDENTITY_BUYER_REQUEST_MAKER_V3)?,
        project_u8(
            buyer.intent + intent::COMPACT_INTENT_SIDE_OFFSET_V2,
            SCALAR_BUYER_SIDE_V3,
        )?,
        project_u8(
            buyer.intent + intent::COMPACT_INTENT_LIFECYCLE_OFFSET_V2,
            SCALAR_BUYER_LIFECYCLE_V3,
        )?,
        project_u32(
            buyer.intent + intent::COMPACT_INTENT_OUTCOME_OFFSET_V2,
            SCALAR_BUYER_OUTCOME_V3,
        )?,
        project_identity(
            buyer.intent + intent::COMPACT_INTENT_MARKET_OFFSET_V2,
            IDENTITY_BUYER_INTENT_MARKET_V3,
        )?,
        project_u64(
            buyer.intent + intent::COMPACT_INTENT_GENERATION_OFFSET_V2,
            SCALAR_BUYER_GENERATION_V3,
        )?,
        project_u64(
            buyer.intent + intent::COMPACT_INTENT_NONCE_OFFSET_V2,
            SCALAR_BUYER_NONCE_V3,
        )?,
        project_u64(
            buyer.intent + intent::COMPACT_INTENT_VALID_FROM_OFFSET_V2,
            SCALAR_BUYER_VALID_FROM_V3,
        )?,
        project_u64(
            buyer.intent + intent::COMPACT_INTENT_VALID_THROUGH_OFFSET_V2,
            SCALAR_BUYER_VALID_THROUGH_V3,
        )?,
        project_u64(
            buyer.intent + intent::COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2,
            SCALAR_BUYER_MAXIMUM_V3,
        )?,
        project_u64(
            buyer.intent + intent::COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2,
            SCALAR_BUYER_LIMIT_V3,
        )?,
        project_u16(
            buyer.intent + intent::COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2,
            SCALAR_BUYER_FEE_BPS_V3,
        )?,
        project_identity(
            buyer.intent + intent::COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2,
            IDENTITY_BUYER_COLLATERAL_REQUEST_V3,
        )?,
        project_u64(INLINE_ORDINARY_REQUEST_BYTES_V3 - 16, SCALAR_FILL_V3)?,
        project_u64(
            INLINE_ORDINARY_REQUEST_BYTES_V3 - 8,
            SCALAR_EXECUTION_PRICE_V3,
        )?,
    ])
}

#[derive(Clone, Copy)]
struct ParticipantOffsets {
    maker: usize,
    domain: usize,
    intent: usize,
}

fn participant_offsets(maker: usize) -> Result<ParticipantOffsets, DirectOrdinaryArtifactErrorV3> {
    let domain = maker
        .checked_add(32)
        .ok_or(DirectOrdinaryArtifactErrorV3::Coordinate)?;
    let intent = domain
        .checked_add(32)
        .ok_or(DirectOrdinaryArtifactErrorV3::Coordinate)?;
    Ok(ParticipantOffsets {
        maker,
        domain,
        intent,
    })
}

fn domain_word(offset: usize) -> Result<u64, DirectOrdinaryArtifactErrorV3> {
    let end = offset
        .checked_add(8)
        .ok_or(DirectOrdinaryArtifactErrorV3::Coordinate)?;
    Ok(u64::from_le_bytes(
        intent::COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2
            .get(offset..end)
            .ok_or(DirectOrdinaryArtifactErrorV3::Coordinate)?
            .try_into()
            .map_err(|_| DirectOrdinaryArtifactErrorV3::Coordinate)?,
    ))
}

fn request(offset: usize) -> Result<RequestCoordinateV1, DirectOrdinaryArtifactErrorV3> {
    Ok(RequestCoordinateV1::fixed(coordinate(offset)?))
}

fn coordinate(value: usize) -> Result<u32, DirectOrdinaryArtifactErrorV3> {
    u32::try_from(value).map_err(|_| DirectOrdinaryArtifactErrorV3::Coordinate)
}

fn register(value: usize) -> Result<u16, DirectOrdinaryArtifactErrorV3> {
    u16::try_from(value).map_err(|_| DirectOrdinaryArtifactErrorV3::Coordinate)
}

fn identity_destination(value: usize) -> Result<u32, DirectOrdinaryArtifactErrorV3> {
    u32::try_from(value).map_err(|_| DirectOrdinaryArtifactErrorV3::Coordinate)
}

fn absolute_message_offset(relative: u32) -> Result<u16, DirectOrdinaryArtifactErrorV3> {
    let offset = u32::try_from(HOT_FAMILY_REQUEST_OFFSET_V3)
        .map_err(|_| DirectOrdinaryArtifactErrorV3::Coordinate)?
        .checked_add(relative)
        .ok_or(DirectOrdinaryArtifactErrorV3::Coordinate)?;
    u16::try_from(offset).map_err(|_| DirectOrdinaryArtifactErrorV3::Coordinate)
}

fn require_u16(
    offset: usize,
    value: u16,
) -> Result<RequestInstructionV1, DirectOrdinaryArtifactErrorV3> {
    Ok(RequestInstructionV1::require_u16(request(offset)?, value))
}

fn require_u32(
    offset: usize,
    value: u32,
) -> Result<RequestInstructionV1, DirectOrdinaryArtifactErrorV3> {
    Ok(RequestInstructionV1::require_u32(request(offset)?, value))
}

fn require_u64(
    offset: usize,
    value: u64,
) -> Result<RequestInstructionV1, DirectOrdinaryArtifactErrorV3> {
    Ok(RequestInstructionV1::require_u64(request(offset)?, value))
}

fn project_u8(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectOrdinaryArtifactErrorV3> {
    Ok(RequestInstructionV1::project_u8(
        request(offset)?,
        ScalarRegisterV1::common(register(destination)?),
    ))
}

fn project_u16(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectOrdinaryArtifactErrorV3> {
    Ok(RequestInstructionV1::project_u16(
        request(offset)?,
        ScalarRegisterV1::common(register(destination)?),
    ))
}

fn project_u32(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectOrdinaryArtifactErrorV3> {
    Ok(RequestInstructionV1::project_u32(
        request(offset)?,
        ScalarRegisterV1::common(register(destination)?),
    ))
}

fn project_u64(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectOrdinaryArtifactErrorV3> {
    Ok(RequestInstructionV1::project_u64(
        request(offset)?,
        ScalarRegisterV1::common(register(destination)?),
    ))
}

fn project_identity(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectOrdinaryArtifactErrorV3> {
    Ok(RequestInstructionV1::project_identity(
        request(offset)?,
        IdentityRegisterV1::common(register(destination)?),
    ))
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::{
        execution_v3::{DirectExecutionRequestV3, encode_header_v3},
        intent_v2::CompactIntentV2,
        ordinary_v3::{
            DIRECT_ORDINARY_TRANSITION_BYTES_V3, IDENTITY_BUYER_COLLATERAL_REQUEST_V3,
            IDENTITY_BUYER_REQUEST_MAKER_V3, IDENTITY_SELLER_COLLATERAL_REQUEST_V3,
            IDENTITY_SELLER_REQUEST_MAKER_V3, SCALAR_BUYER_OUTCOME_V3, SCALAR_EXECUTION_PRICE_V3,
            SCALAR_FILL_V3, SCALAR_SELLER_OUTCOME_V3, encode_direct_ordinary_transition_v3,
        },
    };
    use dclutch_request_profile_contract::{ProjectionRegistersV1, v2::RequestProfileV2};
    use sha2::{Digest, Sha256};

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn intent(side: u8, outcome: u32, collateral: [u8; 32]) -> CompactIntentV2 {
        CompactIntentV2 {
            side,
            lifecycle: 1,
            outcome,
            market: id(3),
            generation: 5,
            nonce: 7 + u64::from(side),
            valid_from: 10,
            valid_through: 20,
            maximum_fill: 30,
            limit_price: if side == 0 { 40 } else { 60 },
            fee_basis_points: 25,
            collateral_account: collateral,
        }
    }

    fn canonical_request() -> [u8; INLINE_ORDINARY_REQUEST_BYTES_V3] {
        let seller = intent(0, 2, id(30));
        let buyer = intent(1, 2, id(31));
        let mut request = [0_u8; INLINE_ORDINARY_REQUEST_BYTES_V3];
        let body = encode_header_v3(DirectExecutionActionV3::InlineOrdinary, &mut request)
            .expect("header");
        put(body, 0, &id(6));
        put(
            body,
            32,
            &seller.signed_preimage().expect("seller preimage"),
        );
        let buyer_offset = DIRECT_SIGNED_PARTICIPANT_BYTES_V3;
        put(body, buyer_offset, &id(7));
        put(
            body,
            buyer_offset + 32,
            &buyer.signed_preimage().expect("buyer preimage"),
        );
        let quantity = 2 * DIRECT_SIGNED_PARTICIPANT_BYTES_V3;
        put(body, quantity, &11_u64.to_le_bytes());
        put(body, quantity + 8, &50_u64.to_le_bytes());
        assert!(matches!(
            DirectExecutionRequestV3::decode(&request, 0),
            Ok(DirectExecutionRequestV3::InlineOrdinary(_))
        ));
        request
    }

    fn encoded_profile() -> [u8; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V2_BYTES_V3] {
        let mut v1_scratch = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V1_BYTES_V3];
        let mut v1_candidate = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V1_BYTES_V3];
        let mut v2_scratch = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V2_BYTES_V3];
        let mut output = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V2_BYTES_V3];
        encode_inline_ordinary_request_profile_v3_atomic(
            &mut v1_scratch,
            &mut v1_candidate,
            &mut v2_scratch,
            &mut output,
        )
        .expect("profile");
        output
    }

    #[test]
    fn signed_profile_round_trips_and_projects_exact_successor_fields() {
        assert_eq!(DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V1_BYTES_V3, 1_232);
        assert_eq!(DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V2_BYTES_V3, 1_272);
        let bytes = encoded_profile();
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(bytes)),
            DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_ID_V3
        );
        let profile = RequestProfileV2::decode(&bytes).expect("decode");
        let signatures = profile.native_signatures();
        assert_eq!(signatures.requirement_count(), 2);
        let seller = signatures.requirement(0).expect("seller requirement");
        let buyer = signatures.requirement(1).expect("buyer requirement");
        assert_eq!(seller.message_offset(), 192);
        assert_eq!(seller.message_bytes(), 172);
        assert_eq!(seller.destination_identity_register(), 4);
        assert_eq!(buyer.message_offset(), 396);
        assert_eq!(buyer.message_bytes(), 172);
        assert_eq!(buyer.destination_identity_register(), 5);

        let request = canonical_request();
        let scalar_input = [0_u64; DIRECT_ORDINARY_COMMON_SCALARS_V3];
        let mut identity_input = [[0_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
        identity_input[IDENTITY_SELLER_NATIVE_SIGNER_V3] = id(6);
        identity_input[IDENTITY_BUYER_NATIVE_SIGNER_V3] = id(7);
        let mut scalar_scratch = [0_u64; DIRECT_ORDINARY_COMMON_SCALARS_V3];
        let mut identity_scratch = [[0_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
        let mut scalar_output = [0_u64; DIRECT_ORDINARY_COMMON_SCALARS_V3];
        let mut identity_output = [[0_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
        profile
            .project_request_atomic(
                0,
                &request,
                ProjectionRegistersV1 {
                    input_scalars: &scalar_input,
                    input_identities: &identity_input,
                    scratch_scalars: &mut scalar_scratch,
                    scratch_identities: &mut identity_scratch,
                    output_scalars: &mut scalar_output,
                    output_identities: &mut identity_output,
                },
            )
            .expect("project");
        assert_eq!(scalar_output[SCALAR_SELLER_OUTCOME_V3], 2);
        assert_eq!(scalar_output[SCALAR_BUYER_OUTCOME_V3], 2);
        assert_eq!(scalar_output[SCALAR_FILL_V3], 11);
        assert_eq!(scalar_output[SCALAR_EXECUTION_PRICE_V3], 50);
        assert_eq!(identity_output[IDENTITY_SELLER_REQUEST_MAKER_V3], id(6));
        assert_eq!(identity_output[IDENTITY_BUYER_REQUEST_MAKER_V3], id(7));
        assert_eq!(
            identity_output[IDENTITY_SELLER_COLLATERAL_REQUEST_V3],
            id(30)
        );
        assert_eq!(
            identity_output[IDENTITY_BUYER_COLLATERAL_REQUEST_V3],
            id(31)
        );
    }

    #[test]
    fn transition_and_interpreted_strategy_content_ids_are_fresh() {
        let mut transition_scratch = [0_u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3];
        let mut transition = [0_u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3];
        encode_direct_ordinary_transition_v3(&mut transition_scratch, &mut transition)
            .expect("transition");
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(transition)),
            DIRECT_INLINE_ORDINARY_TRANSITION_ID_V3
        );
        let strategy = direct_inline_ordinary_strategy_v3().expect("strategy");
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(strategy)),
            DIRECT_INLINE_ORDINARY_STRATEGY_ID_V3
        );
    }

    #[test]
    fn hostile_action_and_reserved_bytes_refuse_without_output_commit() {
        let bytes = encoded_profile();
        let profile = RequestProfileV2::decode(&bytes).expect("decode");
        for offset in [12_usize, 20] {
            let mut request = canonical_request();
            *request.get_mut(offset).expect("hostile coordinate") ^= 1;
            let scalar_input = [0_u64; DIRECT_ORDINARY_COMMON_SCALARS_V3];
            let identity_input = [[0_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
            let mut scalar_scratch = [0_u64; DIRECT_ORDINARY_COMMON_SCALARS_V3];
            let mut identity_scratch = [[0_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
            let mut scalar_output = [91_u64; DIRECT_ORDINARY_COMMON_SCALARS_V3];
            let mut identity_output = [[0x91_u8; 32]; DIRECT_ORDINARY_COMMON_IDENTITIES_V3];
            let scalar_before = scalar_output;
            let identity_before = identity_output;
            assert!(
                profile
                    .project_request_atomic(
                        0,
                        &request,
                        ProjectionRegistersV1 {
                            input_scalars: &scalar_input,
                            input_identities: &identity_input,
                            scratch_scalars: &mut scalar_scratch,
                            scratch_identities: &mut identity_scratch,
                            output_scalars: &mut scalar_output,
                            output_identities: &mut identity_output,
                        },
                    )
                    .is_err()
            );
            assert_eq!(scalar_output, scalar_before);
            assert_eq!(identity_output, identity_before);
        }
    }

    #[test]
    fn wrapper_refusal_preserves_final_output() {
        let mut short_v1_scratch = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V1_BYTES_V3 - 1];
        let mut v1_candidate = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V1_BYTES_V3];
        let mut v2_scratch = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V2_BYTES_V3];
        let mut output = [0x7a_u8; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V2_BYTES_V3];
        let before = output;
        assert_eq!(
            encode_inline_ordinary_request_profile_v3_atomic(
                &mut short_v1_scratch,
                &mut v1_candidate,
                &mut v2_scratch,
                &mut output,
            ),
            Err(DirectOrdinaryArtifactErrorV3::RequestProfileV1)
        );
        assert_eq!(output, before);
    }

    fn put(output: &mut [u8], offset: usize, bytes: &[u8]) {
        let end = offset.checked_add(bytes.len()).expect("test width");
        output
            .get_mut(offset..end)
            .expect("test coordinate")
            .copy_from_slice(bytes);
    }
}
