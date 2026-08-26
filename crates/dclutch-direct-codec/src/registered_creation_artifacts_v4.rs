//! Typed request and transition artifacts for registered Direct creation.
//!
//! RegisterSell and RegisterBuy share one register contract but select distinct
//! Transition programs.  Both require a maker signature, the sole GTC intent
//! lifecycle, an exact vacant registered-record lifecycle result, and checked
//! root/maker replay accounting.  Buy additionally derives the worst-case
//! collateral reserve with the same floor boundaries as the pure successor.

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
use dclutch_transition_vm::v3::{
    IdentityRegisterV3, InstructionV3, ProgramGeometryV3, ScalarRegisterV3, encode_program_atomic,
};

use crate::{
    execution_v3::{
        DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3, DIRECT_EXECUTION_REQUEST_MAGIC_V3,
        DIRECT_EXECUTION_REQUEST_VERSION_V3, DIRECT_REGISTRATION_REQUEST_BYTES_V3,
        DIRECT_SIGNED_PARTICIPANT_BYTES_V3, DirectExecutionActionV3, native_signature_slice_v3,
    },
    generated_intent_v2 as intent,
    successor::{
        DIRECT_FEE_DENOMINATOR_V1, DirectMakerReplayLayoutV1, DirectRegisteredRecordLayoutV2,
    },
};

/// Common scalar-bank width for both registered creation actions.
pub const DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4: usize = 56;
/// Common identity-bank width for both registered creation actions.
pub const DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4: usize = 32;
/// Registered creation has no per-Product-item register body.
pub const DIRECT_REGISTERED_CREATION_ITEM_SCALAR_STRIDE_V4: u16 = 0;
/// Registered creation has no per-Product-item identity body.
pub const DIRECT_REGISTERED_CREATION_ITEM_IDENTITY_STRIDE_V4: u16 = 0;

/// Parent request digest seeded by common Hot.
pub const REGISTERED_IDENTITY_PARENT_REQUEST_V4: usize = 0;
/// Native Ed25519 signer written by RequestProfileV2.
pub const REGISTERED_IDENTITY_NATIVE_SIGNER_V4: usize = 1;
/// Maker carried by the exact request.
pub const REGISTERED_IDENTITY_REQUEST_MAKER_V4: usize = 2;
/// Market carried by the signed intent.
pub const REGISTERED_IDENTITY_INTENT_MARKET_V4: usize = 3;
/// Authenticated Core Market.
pub const REGISTERED_IDENTITY_MARKET_V4: usize = 4;
/// Maker RentCredit account carried by the request.
pub const REGISTERED_IDENTITY_REQUEST_MAKER_RENT_CREDIT_V4: usize = 5;
/// Record RentCredit account carried by the request.
pub const REGISTERED_IDENTITY_REQUEST_RECORD_RENT_CREDIT_V4: usize = 6;
/// Maker RentCredit account observed in the account profile.
pub const REGISTERED_IDENTITY_MAKER_RENT_CREDIT_V4: usize = 7;
/// Record RentCredit account observed in the account profile.
pub const REGISTERED_IDENTITY_RECORD_RENT_CREDIT_V4: usize = 8;
/// Signed collateral account.
pub const REGISTERED_IDENTITY_COLLATERAL_REQUEST_V4: usize = 9;
/// Authenticated routed source collateral account.
pub const REGISTERED_IDENTITY_COLLATERAL_SOURCE_V4: usize = 10;
/// Selected execution release set.
pub const REGISTERED_IDENTITY_RELEASE_SET_V4: usize = 11;
/// Authenticated Product record digest.
pub const REGISTERED_IDENTITY_PRODUCT_RECORD_V4: usize = 12;
/// Product-owned semantic LiabilityBasis identity.
pub const REGISTERED_IDENTITY_SEMANTIC_BASIS_V4: usize = 13;
/// Authenticated raw ProductBasis digest.
pub const REGISTERED_IDENTITY_LINKED_BASIS_V4: usize = 14;
/// Current Registry-selected Trading program.
pub const REGISTERED_IDENTITY_TRADING_PROGRAM_V4: usize = 15;
/// Lifecycle-derived maker replay PDA.
pub const REGISTERED_IDENTITY_MAKER_STATE_V4: usize = 16;
/// Lifecycle-derived registered record PDA.
pub const REGISTERED_IDENTITY_RECORD_STATE_V4: usize = 17;
/// Lifecycle-derived maker RentCredit beneficiary.
pub const REGISTERED_IDENTITY_MAKER_BENEFICIARY_V4: usize = 18;
/// Lifecycle-derived record RentCredit beneficiary.
pub const REGISTERED_IDENTITY_RECORD_BENEFICIARY_V4: usize = 19;
/// Lifecycle-derived maker state owner.
pub const REGISTERED_IDENTITY_MAKER_STATE_OWNER_V4: usize = 20;
/// Lifecycle-derived record state owner.
pub const REGISTERED_IDENTITY_RECORD_STATE_OWNER_V4: usize = 21;
/// Trusted System Program.
pub const REGISTERED_IDENTITY_SYSTEM_PROGRAM_V4: usize = 22;
/// Authenticated immutable Realm.
pub const REGISTERED_IDENTITY_REALM_V4: usize = 23;
/// Realm-selected collateral mint.
pub const REGISTERED_IDENTITY_MINT_V4: usize = 24;
/// Realm-selected token program.
pub const REGISTERED_IDENTITY_TOKEN_PROGRAM_V4: usize = 25;
/// Custody transfer authority.
pub const REGISTERED_IDENTITY_CUSTODY_AUTHORITY_V4: usize = 26;
/// Record-keyed Custody Vault.
pub const REGISTERED_IDENTITY_CUSTODY_VAULT_V4: usize = 27;
/// Registration payer selected by the authenticated account profile.
pub const REGISTERED_IDENTITY_PAYER_V4: usize = 28;
/// Maker RentCredit beneficiary observed from a live replay state.
pub const REGISTERED_IDENTITY_MAKER_BENEFICIARY_OBSERVATION_V4: usize = 29;
/// Record RentCredit beneficiary observed from a live registered record.
pub const REGISTERED_IDENTITY_RECORD_BENEFICIARY_OBSERVATION_V4: usize = 30;

/// Root phase.
pub const REGISTERED_SCALAR_ROOT_PHASE_V4: usize = 0;
/// Trusted current slot.
pub const REGISTERED_SCALAR_SLOT_V4: usize = 1;
/// Signed validity start.
pub const REGISTERED_SCALAR_VALID_FROM_V4: usize = 2;
/// Signed validity end.
pub const REGISTERED_SCALAR_VALID_THROUGH_V4: usize = 3;
/// Signed side.
pub const REGISTERED_SCALAR_SIDE_V4: usize = 4;
/// Signed lifecycle.
pub const REGISTERED_SCALAR_LIFECYCLE_V4: usize = 5;
/// Signed outcome.
pub const REGISTERED_SCALAR_OUTCOME_V4: usize = 6;
/// Product-authenticated outcome count.
pub const REGISTERED_SCALAR_OUTCOME_COUNT_V4: usize = 7;
/// Signed generation.
pub const REGISTERED_SCALAR_GENERATION_V4: usize = 8;
/// Authenticated Market generation.
pub const REGISTERED_SCALAR_MARKET_GENERATION_V4: usize = 9;
/// Signed nonce.
pub const REGISTERED_SCALAR_NONCE_V4: usize = 10;
/// Maker replay next nonce.
pub const REGISTERED_SCALAR_NEXT_NONCE_V4: usize = 11;
/// Signed maximum fill.
pub const REGISTERED_SCALAR_MAXIMUM_V4: usize = 12;
/// Signed limit price.
pub const REGISTERED_SCALAR_LIMIT_V4: usize = 13;
/// Signed fee basis points.
pub const REGISTERED_SCALAR_FEE_BPS_V4: usize = 14;
/// Immutable config fee basis points.
pub const REGISTERED_SCALAR_POLICY_FEE_BPS_V4: usize = 15;
/// Immutable config price scale.
pub const REGISTERED_SCALAR_PRICE_SCALE_V4: usize = 16;
/// Pre-transition number of open maker replay roots.
pub const REGISTERED_SCALAR_ROOT_OPEN_COUNT_V4: usize = 17;
/// Lifecycle-created maker bit.
pub const REGISTERED_SCALAR_MAKER_CREATED_V4: usize = 18;
/// Maker bump observation.
pub const REGISTERED_SCALAR_MAKER_BUMP_OBSERVATION_V4: usize = 19;
/// Lifecycle-derived maker bump.
pub const REGISTERED_SCALAR_MAKER_BUMP_V4: usize = 20;
/// Maker historical principal observation.
pub const REGISTERED_SCALAR_MAKER_PRINCIPAL_OBSERVATION_V4: usize = 21;
/// Lifecycle-derived maker historical principal.
pub const REGISTERED_SCALAR_MAKER_PRINCIPAL_V4: usize = 22;
/// Maker principal carried by the request.
pub const REGISTERED_SCALAR_REQUEST_MAKER_PRINCIPAL_V4: usize = 23;
/// Lifecycle-created record bit; registration requires exactly one.
pub const REGISTERED_SCALAR_RECORD_CREATED_V4: usize = 24;
/// Record bump observation.
pub const REGISTERED_SCALAR_RECORD_BUMP_OBSERVATION_V4: usize = 25;
/// Lifecycle-derived record bump.
pub const REGISTERED_SCALAR_RECORD_BUMP_V4: usize = 26;
/// Record historical principal observation.
pub const REGISTERED_SCALAR_RECORD_PRINCIPAL_OBSERVATION_V4: usize = 27;
/// Lifecycle-derived record historical principal.
pub const REGISTERED_SCALAR_RECORD_PRINCIPAL_V4: usize = 28;
/// Record principal carried by the request.
pub const REGISTERED_SCALAR_REQUEST_RECORD_PRINCIPAL_V4: usize = 29;
/// Pre-transition live record count.
pub const REGISTERED_SCALAR_MAKER_LIVE_COUNT_V4: usize = 30;
/// Pre-transition minimum live nonce.
pub const REGISTERED_SCALAR_MINIMUM_LIVE_NONCE_V4: usize = 31;
/// Canonical zero constant.
pub const REGISTERED_SCALAR_ZERO_V4: usize = 32;
/// Canonical one constant.
pub const REGISTERED_SCALAR_ONE_V4: usize = 33;
/// Expected side constant selected by the descriptor action.
pub const REGISTERED_SCALAR_EXPECTED_SIDE_V4: usize = 34;
/// Registered lifecycle constant (`GTC = 2`).
pub const REGISTERED_SCALAR_GTC_V4: usize = 35;
/// Next maker replay nonce.
pub const REGISTERED_SCALAR_NEXT_NONCE_AFTER_V4: usize = 36;
/// Post-registration live record count.
pub const REGISTERED_SCALAR_MAKER_LIVE_COUNT_AFTER_V4: usize = 37;
/// Post-registration minimum live nonce.
pub const REGISTERED_SCALAR_MINIMUM_LIVE_NONCE_AFTER_V4: usize = 38;
/// Post-registration open maker-root count.
pub const REGISTERED_SCALAR_ROOT_OPEN_COUNT_AFTER_V4: usize = 39;
/// Maximum gross Buy reserve before fee.
pub const REGISTERED_SCALAR_GROSS_RESERVE_V4: usize = 40;
/// Maximum Buy fee reserve.
pub const REGISTERED_SCALAR_FEE_RESERVE_V4: usize = 41;
/// Exact reserved collateral; zero for Sell.
pub const REGISTERED_SCALAR_COLLATERAL_RESERVE_V4: usize = 42;
/// Maker state magic.
pub const REGISTERED_SCALAR_MAKER_MAGIC_V4: usize = 43;
/// Maker state version.
pub const REGISTERED_SCALAR_MAKER_VERSION_V4: usize = 44;
/// Registered record magic.
pub const REGISTERED_SCALAR_RECORD_MAGIC_V4: usize = 45;
/// Registered record version.
pub const REGISTERED_SCALAR_RECORD_VERSION_V4: usize = 46;
/// Custody revision after InitializeReplay.
pub const REGISTERED_SCALAR_CUSTODY_REVISION_ONE_V4: usize = 47;
/// Custody revision after OpenVault.
pub const REGISTERED_SCALAR_CUSTODY_REVISION_TWO_V4: usize = 48;
/// Custody revision after reserve deposit.
pub const REGISTERED_SCALAR_CUSTODY_REVISION_THREE_V4: usize = 49;
/// LifecycleV5 current Rent quote for Custody replay state.
pub const REGISTERED_SCALAR_REPLAY_RENT_V4: usize = 50;
/// LifecycleV5 current Rent quote for the Custody token Vault.
pub const REGISTERED_SCALAR_VAULT_RENT_V4: usize = 51;
/// LifecycleV5 current Rent quote for the maker replay state.
pub const REGISTERED_SCALAR_MAKER_CURRENT_RENT_V4: usize = 52;
/// LifecycleV5 current Rent quote for the registered record.
pub const REGISTERED_SCALAR_RECORD_CURRENT_RENT_V4: usize = 53;
/// Basis-point denominator.
pub const REGISTERED_SCALAR_FEE_DENOMINATOR_V4: usize = 54;
/// CompactIntentV2 magic used only for fresh registered-record initialization.
pub const REGISTERED_SCALAR_INTENT_MAGIC_V4: usize = 55;

const REQUEST_OPERATIONS: usize = 31;
const TRANSITION_INSTRUCTIONS: usize = 41;

/// Exact embedded RequestProfileV1 width.
pub const DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V1_BYTES_V4: usize =
    dclutch_request_profile_contract::HEADER_BYTES
        + REQUEST_OPERATIONS * dclutch_request_profile_contract::OPERATION_BYTES;
/// Exact signed RequestProfileV2 width.
pub const DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V2_BYTES_V4: usize =
    dclutch_request_profile_contract::v2::REQUEST_PROFILE_V2_HEADER_BYTES
        + DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V1_BYTES_V4
        + dclutch_request_profile_contract::v2::NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1;
/// Exact side-selected TransitionVMV3 width.
pub const DIRECT_REGISTERED_CREATION_TRANSITION_BYTES_V4: usize =
    dclutch_transition_vm::v3::HEADER_BYTES
        + TRANSITION_INSTRUCTIONS * dclutch_transition_vm::v3::INSTRUCTION_BYTES;

/// Stable registered creation artifact refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRegisteredCreationArtifactErrorV4 {
    /// Action or coordinate was invalid.
    Coordinate,
    /// RequestProfile encoding refused.
    RequestProfile,
    /// Transition encoding refused.
    Transition,
    /// Strategy construction refused.
    Strategy,
}

/// Emit the exact signed request profile for RegisterSell or RegisterBuy.
pub fn encode_direct_registered_creation_request_profile_v4_atomic(
    action: DirectExecutionActionV3,
    v1_scratch: &mut [u8],
    v1_candidate: &mut [u8],
    v2_scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredCreationArtifactErrorV4> {
    require_creation_action(action)?;
    let operations = request_operations(action)?;
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            coordinate(DIRECT_REGISTRATION_REQUEST_BYTES_V3)?,
            0,
            register(DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4)?,
            0,
            register(DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4)?,
            0,
        ),
        &operations,
        &[],
        v1_scratch,
        v1_candidate,
    )
    .map_err(|_| DirectRegisteredCreationArtifactErrorV4::RequestProfile)?;
    let signature = native_signature_slice_v3(action, 0, 0)
        .map_err(|_| DirectRegisteredCreationArtifactErrorV4::Coordinate)?;
    let requirement = [NativeSignatureRequirementV1::new(
        absolute_message_offset(signature.message_offset)?,
        signature.message_bytes,
        u32::try_from(REGISTERED_IDENTITY_NATIVE_SIGNER_V4)
            .map_err(|_| DirectRegisteredCreationArtifactErrorV4::Coordinate)?,
    )];
    encode_request_profile_v2_atomic(v1_candidate, &requirement, v2_scratch, output)
        .map_err(|_| DirectRegisteredCreationArtifactErrorV4::RequestProfile)
}

/// Emit the exact side-selected registered creation TransitionVM program.
pub fn encode_direct_registered_creation_transition_v4_atomic(
    action: DirectExecutionActionV3,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredCreationArtifactErrorV4> {
    let expected_side = expected_side(action)?;
    let instructions = transition_instructions(expected_side, action)?;
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: register(DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4)?,
            item_scalar_stride: 0,
            common_identities: register(DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4)?,
            item_identity_stride: 0,
        },
        &instructions,
        &[],
        &[],
        scratch,
        output,
    )
    .map_err(|_| DirectRegisteredCreationArtifactErrorV4::Transition)
}

/// Construct an interpreted strategy selecting one emitted creation transition.
pub fn direct_registered_creation_strategy_v4(
    transition_id: [u8; 32],
) -> Result<[u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2], DirectRegisteredCreationArtifactErrorV4> {
    let content = |value| {
        ContentId::new(value).map_err(|_| DirectRegisteredCreationArtifactErrorV4::Strategy)
    };
    ExecutionStrategyProgramV2::new(
        StrategyDispositionV2::Interpreted,
        content(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)?,
        content(transition_id)?,
        content(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)?,
        None,
        content(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2)?,
        None,
        content(ACCELERATOR_REQUEST_SCHEMA_ID_V2)?,
        content(ACCELERATOR_ACK_SCHEMA_ID_V2)?,
    )
    .map(ExecutionStrategyProgramV2::to_bytes)
    .map_err(|_| DirectRegisteredCreationArtifactErrorV4::Strategy)
}

fn request_operations(
    action: DirectExecutionActionV3,
) -> Result<[RequestInstructionV1; REQUEST_OPERATIONS], DirectRegisteredCreationArtifactErrorV4> {
    let intent_offset = DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + 32;
    let intent_body = intent_offset + 32;
    let maker_credit =
        DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + DIRECT_SIGNED_PARTICIPANT_BYTES_V3;
    Ok([
        require_u64(0, u64::from_le_bytes(DIRECT_EXECUTION_REQUEST_MAGIC_V3))?,
        require_u16(8, DIRECT_EXECUTION_REQUEST_VERSION_V3)?,
        require_u16(10, 0)?,
        require_u32(12, action as u32)?,
        require_u32(
            16,
            coordinate(
                DIRECT_REGISTRATION_REQUEST_BYTES_V3 - DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3,
            )?,
        )?,
        RequestInstructionV1::require_zero(request(20)?, 12),
        require_u64(intent_offset, domain_word(0)?)?,
        require_u64(intent_offset + 8, domain_word(8)?)?,
        require_u64(intent_offset + 16, domain_word(16)?)?,
        require_u64(intent_offset + 24, domain_word(24)?)?,
        require_u64(
            intent_body + intent::COMPACT_INTENT_MAGIC_OFFSET_V2,
            u64::from_le_bytes(intent::COMPACT_INTENT_MAGIC_V2),
        )?,
        require_u16(
            intent_body + intent::COMPACT_INTENT_VERSION_OFFSET_V2,
            intent::COMPACT_INTENT_VERSION_V2,
        )?,
        RequestInstructionV1::require_zero(
            request(intent_body + intent::COMPACT_INTENT_RESERVED_A_OFFSET_V2)?,
            4,
        ),
        RequestInstructionV1::require_zero(
            request(intent_body + intent::COMPACT_INTENT_RESERVED_B_OFFSET_V2)?,
            6,
        ),
        project_identity(
            DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3,
            REGISTERED_IDENTITY_REQUEST_MAKER_V4,
        )?,
        project_u8(
            intent_body + intent::COMPACT_INTENT_SIDE_OFFSET_V2,
            REGISTERED_SCALAR_SIDE_V4,
        )?,
        project_u8(
            intent_body + intent::COMPACT_INTENT_LIFECYCLE_OFFSET_V2,
            REGISTERED_SCALAR_LIFECYCLE_V4,
        )?,
        project_u32(
            intent_body + intent::COMPACT_INTENT_OUTCOME_OFFSET_V2,
            REGISTERED_SCALAR_OUTCOME_V4,
        )?,
        project_identity(
            intent_body + intent::COMPACT_INTENT_MARKET_OFFSET_V2,
            REGISTERED_IDENTITY_INTENT_MARKET_V4,
        )?,
        project_u64(
            intent_body + intent::COMPACT_INTENT_GENERATION_OFFSET_V2,
            REGISTERED_SCALAR_GENERATION_V4,
        )?,
        project_u64(
            intent_body + intent::COMPACT_INTENT_NONCE_OFFSET_V2,
            REGISTERED_SCALAR_NONCE_V4,
        )?,
        project_u64(
            intent_body + intent::COMPACT_INTENT_VALID_FROM_OFFSET_V2,
            REGISTERED_SCALAR_VALID_FROM_V4,
        )?,
        project_u64(
            intent_body + intent::COMPACT_INTENT_VALID_THROUGH_OFFSET_V2,
            REGISTERED_SCALAR_VALID_THROUGH_V4,
        )?,
        project_u64(
            intent_body + intent::COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2,
            REGISTERED_SCALAR_MAXIMUM_V4,
        )?,
        project_u64(
            intent_body + intent::COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2,
            REGISTERED_SCALAR_LIMIT_V4,
        )?,
        project_u16(
            intent_body + intent::COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2,
            REGISTERED_SCALAR_FEE_BPS_V4,
        )?,
        project_identity(
            intent_body + intent::COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2,
            REGISTERED_IDENTITY_COLLATERAL_REQUEST_V4,
        )?,
        project_identity(
            maker_credit,
            REGISTERED_IDENTITY_REQUEST_MAKER_RENT_CREDIT_V4,
        )?,
        project_identity(
            maker_credit + 32,
            REGISTERED_IDENTITY_REQUEST_RECORD_RENT_CREDIT_V4,
        )?,
        project_u64(
            maker_credit + 64,
            REGISTERED_SCALAR_REQUEST_MAKER_PRINCIPAL_V4,
        )?,
        project_u64(
            maker_credit + 72,
            REGISTERED_SCALAR_REQUEST_RECORD_PRINCIPAL_V4,
        )?,
    ])
}

fn transition_instructions(
    expected_side: u64,
    action: DirectExecutionActionV3,
) -> Result<[InstructionV3; TRANSITION_INSTRUCTIONS], DirectRegisteredCreationArtifactErrorV4> {
    let mut output = [InstructionV3::load_const(scalar(0)?, 0); TRANSITION_INSTRUCTIONS];
    let mut next = 0;
    for instruction in [
        InstructionV3::load_const(scalar(REGISTERED_SCALAR_ZERO_V4)?, 0),
        InstructionV3::load_const(scalar(REGISTERED_SCALAR_ONE_V4)?, 1),
        InstructionV3::load_const(scalar(REGISTERED_SCALAR_EXPECTED_SIDE_V4)?, expected_side),
        InstructionV3::load_const(scalar(REGISTERED_SCALAR_GTC_V4)?, 2),
        InstructionV3::load_const(
            scalar(REGISTERED_SCALAR_FEE_DENOMINATOR_V4)?,
            DIRECT_FEE_DENOMINATOR_V1 as u64,
        ),
        InstructionV3::load_const(
            scalar(REGISTERED_SCALAR_INTENT_MAGIC_V4)?,
            u64::from_le_bytes(intent::COMPACT_INTENT_MAGIC_V2),
        ),
        InstructionV3::load_const(
            scalar(REGISTERED_SCALAR_MAKER_MAGIC_V4)?,
            DirectMakerReplayLayoutV1::MAGIC_WORD,
        ),
        InstructionV3::load_const(
            scalar(REGISTERED_SCALAR_MAKER_VERSION_V4)?,
            u64::from(DirectMakerReplayLayoutV1::ABI_VERSION),
        ),
        InstructionV3::load_const(
            scalar(REGISTERED_SCALAR_RECORD_MAGIC_V4)?,
            DirectRegisteredRecordLayoutV2::MAGIC_WORD,
        ),
        InstructionV3::load_const(
            scalar(REGISTERED_SCALAR_RECORD_VERSION_V4)?,
            u64::from(DirectRegisteredRecordLayoutV2::ABI_VERSION),
        ),
        InstructionV3::load_const(scalar(REGISTERED_SCALAR_CUSTODY_REVISION_ONE_V4)?, 1),
        InstructionV3::load_const(scalar(REGISTERED_SCALAR_CUSTODY_REVISION_TWO_V4)?, 2),
        InstructionV3::load_const(scalar(REGISTERED_SCALAR_CUSTODY_REVISION_THREE_V4)?, 3),
        InstructionV3::scalar_eq(
            scalar(REGISTERED_SCALAR_ROOT_PHASE_V4)?,
            scalar(REGISTERED_SCALAR_ZERO_V4)?,
        ),
        InstructionV3::identity_eq(
            identity(REGISTERED_IDENTITY_NATIVE_SIGNER_V4)?,
            identity(REGISTERED_IDENTITY_REQUEST_MAKER_V4)?,
        ),
        InstructionV3::identity_eq(
            identity(REGISTERED_IDENTITY_INTENT_MARKET_V4)?,
            identity(REGISTERED_IDENTITY_MARKET_V4)?,
        ),
        InstructionV3::identity_eq(
            identity(REGISTERED_IDENTITY_REQUEST_MAKER_RENT_CREDIT_V4)?,
            identity(REGISTERED_IDENTITY_MAKER_RENT_CREDIT_V4)?,
        ),
        InstructionV3::identity_eq(
            identity(REGISTERED_IDENTITY_REQUEST_RECORD_RENT_CREDIT_V4)?,
            identity(REGISTERED_IDENTITY_RECORD_RENT_CREDIT_V4)?,
        ),
        InstructionV3::identity_eq(
            identity(REGISTERED_IDENTITY_COLLATERAL_REQUEST_V4)?,
            identity(REGISTERED_IDENTITY_COLLATERAL_SOURCE_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar(REGISTERED_SCALAR_SIDE_V4)?,
            scalar(REGISTERED_SCALAR_EXPECTED_SIDE_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar(REGISTERED_SCALAR_LIFECYCLE_V4)?,
            scalar(REGISTERED_SCALAR_GTC_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar(REGISTERED_SCALAR_GENERATION_V4)?,
            scalar(REGISTERED_SCALAR_MARKET_GENERATION_V4)?,
        ),
        InstructionV3::scalar_le(
            scalar(REGISTERED_SCALAR_VALID_FROM_V4)?,
            scalar(REGISTERED_SCALAR_SLOT_V4)?,
        ),
        InstructionV3::scalar_le(
            scalar(REGISTERED_SCALAR_SLOT_V4)?,
            scalar(REGISTERED_SCALAR_VALID_THROUGH_V4)?,
        ),
        InstructionV3::scalar_lt(
            scalar(REGISTERED_SCALAR_OUTCOME_V4)?,
            scalar(REGISTERED_SCALAR_OUTCOME_COUNT_V4)?,
        ),
        InstructionV3::nonzero(scalar(REGISTERED_SCALAR_MAXIMUM_V4)?),
        InstructionV3::scalar_le(
            scalar(REGISTERED_SCALAR_LIMIT_V4)?,
            scalar(REGISTERED_SCALAR_PRICE_SCALE_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar(REGISTERED_SCALAR_FEE_BPS_V4)?,
            scalar(REGISTERED_SCALAR_POLICY_FEE_BPS_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar(REGISTERED_SCALAR_NONCE_V4)?,
            scalar(REGISTERED_SCALAR_NEXT_NONCE_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar(REGISTERED_SCALAR_REQUEST_MAKER_PRINCIPAL_V4)?,
            scalar(REGISTERED_SCALAR_MAKER_PRINCIPAL_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar(REGISTERED_SCALAR_REQUEST_RECORD_PRINCIPAL_V4)?,
            scalar(REGISTERED_SCALAR_RECORD_PRINCIPAL_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar(REGISTERED_SCALAR_RECORD_CREATED_V4)?,
            scalar(REGISTERED_SCALAR_ONE_V4)?,
        ),
        InstructionV3::scalar_le(
            scalar(REGISTERED_SCALAR_MAKER_CREATED_V4)?,
            scalar(REGISTERED_SCALAR_ONE_V4)?,
        ),
        InstructionV3::increment_into(
            scalar(REGISTERED_SCALAR_NEXT_NONCE_V4)?,
            scalar(REGISTERED_SCALAR_NEXT_NONCE_AFTER_V4)?,
        ),
        InstructionV3::increment_into(
            scalar(REGISTERED_SCALAR_MAKER_LIVE_COUNT_V4)?,
            scalar(REGISTERED_SCALAR_MAKER_LIVE_COUNT_AFTER_V4)?,
        ),
        InstructionV3::checked_add_into(
            scalar(REGISTERED_SCALAR_ROOT_OPEN_COUNT_V4)?,
            scalar(REGISTERED_SCALAR_MAKER_CREATED_V4)?,
            scalar(REGISTERED_SCALAR_ROOT_OPEN_COUNT_AFTER_V4)?,
        ),
    ] {
        push(&mut output, &mut next, instruction)?;
    }
    // Preserve the previous minimum for nonempty replay roots; first
    // registration canonically installs the just-consumed nonce.
    push(
        &mut output,
        &mut next,
        InstructionV3::copy_scalar(
            scalar(REGISTERED_SCALAR_MINIMUM_LIVE_NONCE_V4)?,
            scalar(REGISTERED_SCALAR_MINIMUM_LIVE_NONCE_AFTER_V4)?,
        ),
    )?;
    push(
        &mut output,
        &mut next,
        InstructionV3::select_zero(
            scalar(REGISTERED_SCALAR_MAKER_LIVE_COUNT_V4)?,
            scalar(REGISTERED_SCALAR_NONCE_V4)?,
            scalar(REGISTERED_SCALAR_MINIMUM_LIVE_NONCE_AFTER_V4)?,
        ),
    )?;
    if action == DirectExecutionActionV3::RegisterBuy {
        for instruction in [
            InstructionV3::mul_div_floor(
                scalar(REGISTERED_SCALAR_MAXIMUM_V4)?,
                scalar(REGISTERED_SCALAR_LIMIT_V4)?,
                scalar(REGISTERED_SCALAR_PRICE_SCALE_V4)?,
                scalar(REGISTERED_SCALAR_GROSS_RESERVE_V4)?,
            ),
            InstructionV3::mul_div_floor(
                scalar(REGISTERED_SCALAR_GROSS_RESERVE_V4)?,
                scalar(REGISTERED_SCALAR_POLICY_FEE_BPS_V4)?,
                scalar(REGISTERED_SCALAR_FEE_DENOMINATOR_V4)?,
                scalar(REGISTERED_SCALAR_FEE_RESERVE_V4)?,
            ),
            InstructionV3::checked_add_into(
                scalar(REGISTERED_SCALAR_GROSS_RESERVE_V4)?,
                scalar(REGISTERED_SCALAR_FEE_RESERVE_V4)?,
                scalar(REGISTERED_SCALAR_COLLATERAL_RESERVE_V4)?,
            ),
        ] {
            push(&mut output, &mut next, instruction)?;
        }
    } else {
        for destination in [
            REGISTERED_SCALAR_GROSS_RESERVE_V4,
            REGISTERED_SCALAR_FEE_RESERVE_V4,
            REGISTERED_SCALAR_COLLATERAL_RESERVE_V4,
        ] {
            push(
                &mut output,
                &mut next,
                InstructionV3::load_const(scalar(destination)?, 0),
            )?;
        }
    }
    if next != output.len() {
        return Err(DirectRegisteredCreationArtifactErrorV4::Coordinate);
    }
    Ok(output)
}

const fn expected_side(
    action: DirectExecutionActionV3,
) -> Result<u64, DirectRegisteredCreationArtifactErrorV4> {
    match action {
        DirectExecutionActionV3::RegisterSell => Ok(0),
        DirectExecutionActionV3::RegisterBuy => Ok(1),
        _ => Err(DirectRegisteredCreationArtifactErrorV4::Coordinate),
    }
}

const fn require_creation_action(
    action: DirectExecutionActionV3,
) -> Result<(), DirectRegisteredCreationArtifactErrorV4> {
    match action {
        DirectExecutionActionV3::RegisterSell | DirectExecutionActionV3::RegisterBuy => Ok(()),
        _ => Err(DirectRegisteredCreationArtifactErrorV4::Coordinate),
    }
}

fn push(
    output: &mut [InstructionV3],
    next: &mut usize,
    instruction: InstructionV3,
) -> Result<(), DirectRegisteredCreationArtifactErrorV4> {
    *output
        .get_mut(*next)
        .ok_or(DirectRegisteredCreationArtifactErrorV4::Coordinate)? = instruction;
    *next = next
        .checked_add(1)
        .ok_or(DirectRegisteredCreationArtifactErrorV4::Coordinate)?;
    Ok(())
}

fn request(offset: usize) -> Result<RequestCoordinateV1, DirectRegisteredCreationArtifactErrorV4> {
    Ok(RequestCoordinateV1::fixed(coordinate(offset)?))
}

fn coordinate(value: usize) -> Result<u32, DirectRegisteredCreationArtifactErrorV4> {
    u32::try_from(value).map_err(|_| DirectRegisteredCreationArtifactErrorV4::Coordinate)
}

fn register(value: usize) -> Result<u16, DirectRegisteredCreationArtifactErrorV4> {
    u16::try_from(value).map_err(|_| DirectRegisteredCreationArtifactErrorV4::Coordinate)
}

fn scalar(value: usize) -> Result<ScalarRegisterV3, DirectRegisteredCreationArtifactErrorV4> {
    register(value).map(ScalarRegisterV3::common)
}

fn identity(value: usize) -> Result<IdentityRegisterV3, DirectRegisteredCreationArtifactErrorV4> {
    register(value).map(IdentityRegisterV3::common)
}

fn absolute_message_offset(relative: u32) -> Result<u16, DirectRegisteredCreationArtifactErrorV4> {
    u32::try_from(HOT_FAMILY_REQUEST_OFFSET_V3)
        .map_err(|_| DirectRegisteredCreationArtifactErrorV4::Coordinate)?
        .checked_add(relative)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or(DirectRegisteredCreationArtifactErrorV4::Coordinate)
}

fn domain_word(offset: usize) -> Result<u64, DirectRegisteredCreationArtifactErrorV4> {
    let end = offset
        .checked_add(8)
        .ok_or(DirectRegisteredCreationArtifactErrorV4::Coordinate)?;
    Ok(u64::from_le_bytes(
        intent::COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2
            .get(offset..end)
            .ok_or(DirectRegisteredCreationArtifactErrorV4::Coordinate)?
            .try_into()
            .map_err(|_| DirectRegisteredCreationArtifactErrorV4::Coordinate)?,
    ))
}

fn require_u16(
    offset: usize,
    value: u16,
) -> Result<RequestInstructionV1, DirectRegisteredCreationArtifactErrorV4> {
    Ok(RequestInstructionV1::require_u16(request(offset)?, value))
}

fn require_u32(
    offset: usize,
    value: u32,
) -> Result<RequestInstructionV1, DirectRegisteredCreationArtifactErrorV4> {
    Ok(RequestInstructionV1::require_u32(request(offset)?, value))
}

fn require_u64(
    offset: usize,
    value: u64,
) -> Result<RequestInstructionV1, DirectRegisteredCreationArtifactErrorV4> {
    Ok(RequestInstructionV1::require_u64(request(offset)?, value))
}

fn project_u8(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectRegisteredCreationArtifactErrorV4> {
    Ok(RequestInstructionV1::project_u8(
        request(offset)?,
        ScalarRegisterV1::common(register(destination)?),
    ))
}

fn project_u16(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectRegisteredCreationArtifactErrorV4> {
    Ok(RequestInstructionV1::project_u16(
        request(offset)?,
        ScalarRegisterV1::common(register(destination)?),
    ))
}

fn project_u32(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectRegisteredCreationArtifactErrorV4> {
    Ok(RequestInstructionV1::project_u32(
        request(offset)?,
        ScalarRegisterV1::common(register(destination)?),
    ))
}

fn project_u64(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectRegisteredCreationArtifactErrorV4> {
    Ok(RequestInstructionV1::project_u64(
        request(offset)?,
        ScalarRegisterV1::common(register(destination)?),
    ))
}

fn project_identity(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectRegisteredCreationArtifactErrorV4> {
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
        execution_v3::{DirectRegistrationRequestV3, DirectSignedParticipantV3},
        intent_v2::CompactIntentV2,
        registered_requests_v4::encode_direct_registration_request_v3_atomic,
    };
    use dclutch_request_profile_contract::{
        ProjectionRegistersV1,
        v2::{REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID, RequestProfileV2},
    };
    use dclutch_transition_vm::v3::{
        ProgramV3, RegisterInput, RegisterOutput, execute_fold_atomic,
    };
    use sha2::{Digest, Sha256};

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn set_scalar(registers: &mut [u64], index: usize, value: u64) {
        *registers.get_mut(index).expect("scalar register") = value;
    }

    fn scalar_value(registers: &[u64], index: usize) -> u64 {
        *registers.get(index).expect("scalar register")
    }

    fn request(action: DirectExecutionActionV3) -> [u8; DIRECT_REGISTRATION_REQUEST_BYTES_V3] {
        let mut output = [0_u8; DIRECT_REGISTRATION_REQUEST_BYTES_V3];
        encode_direct_registration_request_v3_atomic(
            action,
            DirectRegistrationRequestV3 {
                participant: DirectSignedParticipantV3 {
                    maker: id(2),
                    intent: CompactIntentV2 {
                        side: u8::from(action == DirectExecutionActionV3::RegisterBuy),
                        lifecycle: 2,
                        outcome: 1,
                        market: id(4),
                        generation: 9,
                        nonce: 7,
                        valid_from: 2,
                        valid_through: 20,
                        maximum_fill: 40,
                        limit_price: 50,
                        fee_basis_points: 1_000,
                        collateral_account: id(10),
                    },
                },
                maker_rent_credit: id(7),
                record_rent_credit: id(8),
                maker_rent_principal: 100,
                record_rent_principal: 200,
            },
            &mut output,
        )
        .expect("request");
        output
    }

    fn profile(
        action: DirectExecutionActionV3,
    ) -> [u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V2_BYTES_V4] {
        let mut v1_scratch = [0_u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V1_BYTES_V4];
        let mut v1 = [0_u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V1_BYTES_V4];
        let mut v2_scratch = [0_u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V2_BYTES_V4];
        let mut output = [0_u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V2_BYTES_V4];
        encode_direct_registered_creation_request_profile_v4_atomic(
            action,
            &mut v1_scratch,
            &mut v1,
            &mut v2_scratch,
            &mut output,
        )
        .expect("profile");
        output
    }

    fn execute(action: DirectExecutionActionV3) -> std::vec::Vec<u64> {
        let profile_bytes = profile(action);
        let profile_id: [u8; 32] = Sha256::digest(profile_bytes).into();
        let profile = RequestProfileV2::decode_selected(profile_id, profile_id, &profile_bytes)
            .expect("profile decode");
        assert_eq!(REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID.len(), 32);
        let mut scalars = std::vec![0_u64; DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4];
        let mut identities = [[0_u8; 32]; DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4];
        identities[REGISTERED_IDENTITY_NATIVE_SIGNER_V4] = id(2);
        identities[REGISTERED_IDENTITY_MARKET_V4] = id(4);
        identities[REGISTERED_IDENTITY_MAKER_RENT_CREDIT_V4] = id(7);
        identities[REGISTERED_IDENTITY_RECORD_RENT_CREDIT_V4] = id(8);
        identities[REGISTERED_IDENTITY_COLLATERAL_SOURCE_V4] = id(10);
        set_scalar(&mut scalars, REGISTERED_SCALAR_ROOT_PHASE_V4, 0);
        set_scalar(&mut scalars, REGISTERED_SCALAR_SLOT_V4, 10);
        set_scalar(&mut scalars, REGISTERED_SCALAR_OUTCOME_COUNT_V4, 3);
        set_scalar(&mut scalars, REGISTERED_SCALAR_MARKET_GENERATION_V4, 9);
        set_scalar(&mut scalars, REGISTERED_SCALAR_NEXT_NONCE_V4, 7);
        set_scalar(&mut scalars, REGISTERED_SCALAR_POLICY_FEE_BPS_V4, 1_000);
        set_scalar(&mut scalars, REGISTERED_SCALAR_PRICE_SCALE_V4, 100);
        set_scalar(&mut scalars, REGISTERED_SCALAR_ROOT_OPEN_COUNT_V4, 4);
        set_scalar(&mut scalars, REGISTERED_SCALAR_MAKER_CREATED_V4, 1);
        set_scalar(&mut scalars, REGISTERED_SCALAR_MAKER_PRINCIPAL_V4, 100);
        set_scalar(&mut scalars, REGISTERED_SCALAR_RECORD_CREATED_V4, 1);
        set_scalar(&mut scalars, REGISTERED_SCALAR_RECORD_PRINCIPAL_V4, 200);
        set_scalar(&mut scalars, REGISTERED_SCALAR_MAKER_LIVE_COUNT_V4, 0);
        let input_scalars = scalars.clone();
        let input_identities = identities;
        let mut projected_scalars = scalars.clone();
        let mut projected_identities = identities;
        profile
            .project_request_atomic(
                0,
                &request(action),
                ProjectionRegistersV1 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scalars,
                    scratch_identities: &mut identities,
                    output_scalars: &mut projected_scalars,
                    output_identities: &mut projected_identities,
                },
            )
            .expect("projection");
        let mut transition_scratch = [0_u8; DIRECT_REGISTERED_CREATION_TRANSITION_BYTES_V4];
        let mut transition = [0_u8; DIRECT_REGISTERED_CREATION_TRANSITION_BYTES_V4];
        encode_direct_registered_creation_transition_v4_atomic(
            action,
            &mut transition_scratch,
            &mut transition,
        )
        .expect("transition");
        let program = ProgramV3::decode(&transition).expect("transition decode");
        let mut scratch_scalars = projected_scalars.clone();
        let mut scratch_identities = projected_identities;
        let mut output_scalars = std::vec![0_u64; DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4];
        let mut output_identities = [[0_u8; 32]; DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4];
        execute_fold_atomic(
            program,
            0,
            RegisterInput {
                scalars: &projected_scalars,
                identities: &projected_identities,
            },
            RegisterOutput {
                scalars: &mut scratch_scalars,
                identities: &mut scratch_identities,
            },
            RegisterOutput {
                scalars: &mut output_scalars,
                identities: &mut output_identities,
            },
        )
        .expect("execute");
        output_scalars
    }

    #[test]
    fn sell_and_buy_share_request_geometry_but_select_distinct_reserves() {
        let sell = execute(DirectExecutionActionV3::RegisterSell);
        let buy = execute(DirectExecutionActionV3::RegisterBuy);
        assert_eq!(
            scalar_value(&sell, REGISTERED_SCALAR_COLLATERAL_RESERVE_V4),
            0
        );
        assert_eq!(scalar_value(&buy, REGISTERED_SCALAR_GROSS_RESERVE_V4), 20);
        assert_eq!(scalar_value(&buy, REGISTERED_SCALAR_FEE_RESERVE_V4), 2);
        assert_eq!(
            scalar_value(&buy, REGISTERED_SCALAR_COLLATERAL_RESERVE_V4),
            22
        );
        for output in [sell, buy] {
            assert_eq!(
                scalar_value(&output, REGISTERED_SCALAR_NEXT_NONCE_AFTER_V4),
                8
            );
            assert_eq!(
                scalar_value(&output, REGISTERED_SCALAR_MAKER_LIVE_COUNT_AFTER_V4),
                1
            );
            assert_eq!(
                scalar_value(&output, REGISTERED_SCALAR_MINIMUM_LIVE_NONCE_AFTER_V4),
                7
            );
            assert_eq!(
                scalar_value(&output, REGISTERED_SCALAR_ROOT_OPEN_COUNT_AFTER_V4),
                5
            );
        }
    }

    #[test]
    fn wrong_side_or_live_record_refuses_without_output_mutation() {
        let action = DirectExecutionActionV3::RegisterBuy;
        let mut transition_scratch = [0_u8; DIRECT_REGISTERED_CREATION_TRANSITION_BYTES_V4];
        let mut transition = [0_u8; DIRECT_REGISTERED_CREATION_TRANSITION_BYTES_V4];
        encode_direct_registered_creation_transition_v4_atomic(
            action,
            &mut transition_scratch,
            &mut transition,
        )
        .expect("transition");
        let program = ProgramV3::decode(&transition).expect("decode");
        for (side, created) in [(0_u64, 1_u64), (1, 0)] {
            let mut input_scalars = std::vec![0_u64; DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4];
            let input_identities = [id(1); DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4];
            set_scalar(&mut input_scalars, REGISTERED_SCALAR_SIDE_V4, side);
            set_scalar(&mut input_scalars, REGISTERED_SCALAR_LIFECYCLE_V4, 2);
            set_scalar(
                &mut input_scalars,
                REGISTERED_SCALAR_RECORD_CREATED_V4,
                created,
            );
            set_scalar(&mut input_scalars, REGISTERED_SCALAR_ONE_V4, 1);
            let mut scratch_scalars = input_scalars.clone();
            let mut scratch_identities = input_identities;
            let mut output_scalars =
                std::vec![0x55_u64; DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4];
            let before = output_scalars.clone();
            let mut output_identities =
                [[0x55_u8; 32]; DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4];
            assert!(
                execute_fold_atomic(
                    program,
                    0,
                    RegisterInput {
                        scalars: &input_scalars,
                        identities: &input_identities
                    },
                    RegisterOutput {
                        scalars: &mut scratch_scalars,
                        identities: &mut scratch_identities
                    },
                    RegisterOutput {
                        scalars: &mut output_scalars,
                        identities: &mut output_identities
                    },
                )
                .is_err()
            );
            assert_eq!(output_scalars, before);
        }
    }
}
