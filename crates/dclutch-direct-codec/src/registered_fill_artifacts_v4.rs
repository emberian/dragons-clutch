//! Request, transition, and interpreted strategy for registered ordinary fills.
//!
//! The matcher request is intentionally unsigned. Authority comes from two
//! previously authenticated GTC records and their maker replay coordinates.
//! The transition re-proves both reservations, charges cumulative-difference
//! fees, derives partial/terminal candidates, and exposes only checked child
//! effect quantities.

use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_request_profile_contract::encode::{
    RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1, ScalarRegisterV1,
    encode_request_profile_v1_atomic,
};
use dclutch_transition_vm::v3::{
    IdentityRegisterV3, InstructionV3, ProgramGeometryV3, ScalarRegisterV3, encode_program_atomic,
};

use crate::{
    execution_v3::{
        DIRECT_EXECUTION_REQUEST_MAGIC_V3, DIRECT_EXECUTION_REQUEST_VERSION_V3,
        DIRECT_REGISTERED_FILL_REQUEST_BYTES_V3, DirectExecutionActionV3,
    },
    successor::DIRECT_FEE_DENOMINATOR_V1,
};

/// Common scalar-bank width for registered ordinary fills.
pub const DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4: usize = 92;
/// Common identity-bank width for registered ordinary fills.
pub const DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4: usize = 40;
/// Registered ordinary fills have no per-Product-item register body.
pub const DIRECT_REGISTERED_FILL_ITEM_SCALAR_STRIDE_V4: u16 = 0;
/// Registered ordinary fills have no per-Product-item identity body.
pub const DIRECT_REGISTERED_FILL_ITEM_IDENTITY_STRIDE_V4: u16 = 0;

/// Authenticated root phase.
pub const FILL_SCALAR_ROOT_PHASE_V4: usize = 0;
/// Trusted current slot.
pub const FILL_SCALAR_SLOT_V4: usize = 1;
/// Product-authenticated outcome count.
pub const FILL_SCALAR_OUTCOME_COUNT_V4: usize = 2;
/// Core Market generation.
pub const FILL_SCALAR_MARKET_GENERATION_V4: usize = 3;
/// Immutable config price scale.
pub const FILL_SCALAR_PRICE_SCALE_V4: usize = 4;
/// Immutable config fee basis points.
pub const FILL_SCALAR_POLICY_FEE_BPS_V4: usize = 5;
/// Matcher-selected positive fill quantity.
pub const FILL_SCALAR_QUANTITY_V4: usize = 6;
/// Matcher-selected execution price.
pub const FILL_SCALAR_EXECUTION_PRICE_V4: usize = 7;
/// Canonical zero constant.
pub const FILL_SCALAR_ZERO_V4: usize = 8;
/// Canonical one constant.
pub const FILL_SCALAR_ONE_V4: usize = 9;
/// Registered lifecycle constant.
pub const FILL_SCALAR_GTC_V4: usize = 10;
/// Fee denominator constant.
pub const FILL_SCALAR_FEE_DENOMINATOR_V4: usize = 11;
/// Number of live maker roots; unchanged by record fill.
pub const FILL_SCALAR_ROOT_OPEN_COUNT_V4: usize = 12;

/// Seller persisted side.
pub const FILL_SCALAR_SELLER_SIDE_V4: usize = 13;
/// Seller persisted lifecycle.
pub const FILL_SCALAR_SELLER_LIFECYCLE_V4: usize = 14;
/// Seller outcome.
pub const FILL_SCALAR_SELLER_OUTCOME_V4: usize = 15;
/// Seller Market generation.
pub const FILL_SCALAR_SELLER_GENERATION_V4: usize = 16;
/// Seller record nonce.
pub const FILL_SCALAR_SELLER_NONCE_V4: usize = 17;
/// Seller validity start.
pub const FILL_SCALAR_SELLER_VALID_FROM_V4: usize = 18;
/// Seller validity end.
pub const FILL_SCALAR_SELLER_VALID_THROUGH_V4: usize = 19;
/// Seller maximum quantity.
pub const FILL_SCALAR_SELLER_MAXIMUM_V4: usize = 20;
/// Seller minimum price.
pub const FILL_SCALAR_SELLER_LIMIT_V4: usize = 21;
/// Seller signed fee rate.
pub const FILL_SCALAR_SELLER_FEE_BPS_V4: usize = 22;
/// Seller already-filled quantity.
pub const FILL_SCALAR_SELLER_FILLED_V4: usize = 23;
/// Seller remaining claim reserve.
pub const FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4: usize = 24;
/// Seller collateral reserve, canonically zero.
pub const FILL_SCALAR_SELLER_RESERVED_COLLATERAL_V4: usize = 25;
/// Seller cumulative gross.
pub const FILL_SCALAR_SELLER_CUMULATIVE_GROSS_V4: usize = 26;
/// Seller cumulative fee.
pub const FILL_SCALAR_SELLER_CUMULATIVE_FEE_V4: usize = 27;
/// Seller maker replay next nonce.
pub const FILL_SCALAR_SELLER_NEXT_NONCE_V4: usize = 28;
/// Seller maker replay live record count.
pub const FILL_SCALAR_SELLER_LIVE_COUNT_V4: usize = 29;
/// Seller replay invalidation threshold.
pub const FILL_SCALAR_SELLER_MINIMUM_NONCE_V4: usize = 30;
/// Seller maker replay Market generation.
pub const FILL_SCALAR_SELLER_MAKER_GENERATION_V4: usize = 31;

/// Buyer persisted side.
pub const FILL_SCALAR_BUYER_SIDE_V4: usize = 32;
/// Buyer persisted lifecycle.
pub const FILL_SCALAR_BUYER_LIFECYCLE_V4: usize = 33;
/// Buyer outcome.
pub const FILL_SCALAR_BUYER_OUTCOME_V4: usize = 34;
/// Buyer Market generation.
pub const FILL_SCALAR_BUYER_GENERATION_V4: usize = 35;
/// Buyer record nonce.
pub const FILL_SCALAR_BUYER_NONCE_V4: usize = 36;
/// Buyer validity start.
pub const FILL_SCALAR_BUYER_VALID_FROM_V4: usize = 37;
/// Buyer validity end.
pub const FILL_SCALAR_BUYER_VALID_THROUGH_V4: usize = 38;
/// Buyer maximum quantity.
pub const FILL_SCALAR_BUYER_MAXIMUM_V4: usize = 39;
/// Buyer maximum price.
pub const FILL_SCALAR_BUYER_LIMIT_V4: usize = 40;
/// Buyer signed fee rate.
pub const FILL_SCALAR_BUYER_FEE_BPS_V4: usize = 41;
/// Buyer already-filled quantity.
pub const FILL_SCALAR_BUYER_FILLED_V4: usize = 42;
/// Buyer claim reserve, canonically zero.
pub const FILL_SCALAR_BUYER_RESERVED_CLAIMS_V4: usize = 43;
/// Buyer remaining collateral reserve.
pub const FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4: usize = 44;
/// Buyer cumulative gross.
pub const FILL_SCALAR_BUYER_CUMULATIVE_GROSS_V4: usize = 45;
/// Buyer cumulative fee.
pub const FILL_SCALAR_BUYER_CUMULATIVE_FEE_V4: usize = 46;
/// Buyer maker replay next nonce.
pub const FILL_SCALAR_BUYER_NEXT_NONCE_V4: usize = 47;
/// Buyer maker replay live record count.
pub const FILL_SCALAR_BUYER_LIVE_COUNT_V4: usize = 48;
/// Buyer replay invalidation threshold.
pub const FILL_SCALAR_BUYER_MINIMUM_NONCE_V4: usize = 49;
/// Buyer maker replay Market generation.
pub const FILL_SCALAR_BUYER_MAKER_GENERATION_V4: usize = 50;

/// Seller filled quantity after this match.
pub const FILL_SCALAR_SELLER_FILLED_AFTER_V4: usize = 51;
/// Buyer filled quantity after this match.
pub const FILL_SCALAR_BUYER_FILLED_AFTER_V4: usize = 52;
/// Seller remaining quantity after this match.
pub const FILL_SCALAR_SELLER_REMAINING_AFTER_V4: usize = 53;
/// Buyer remaining quantity after this match.
pub const FILL_SCALAR_BUYER_REMAINING_AFTER_V4: usize = 54;
/// Exact common gross quote.
pub const FILL_SCALAR_GROSS_V4: usize = 55;
/// Seller cumulative gross after this match.
pub const FILL_SCALAR_SELLER_CUMULATIVE_GROSS_AFTER_V4: usize = 56;
/// Buyer cumulative gross after this match.
pub const FILL_SCALAR_BUYER_CUMULATIVE_GROSS_AFTER_V4: usize = 57;
/// Seller cumulative fee after this match.
pub const FILL_SCALAR_SELLER_CUMULATIVE_FEE_AFTER_V4: usize = 58;
/// Buyer cumulative fee after this match.
pub const FILL_SCALAR_BUYER_CUMULATIVE_FEE_AFTER_V4: usize = 59;
/// Seller difference-of-floors fee.
pub const FILL_SCALAR_SELLER_FEE_DELTA_V4: usize = 60;
/// Buyer difference-of-floors fee.
pub const FILL_SCALAR_BUYER_FEE_DELTA_V4: usize = 61;
/// Net collateral credited to the seller.
pub const FILL_SCALAR_SELLER_NET_V4: usize = 62;
/// Gross plus buyer fee debited from buyer escrow.
pub const FILL_SCALAR_BUYER_DEBIT_V4: usize = 63;
/// Combined seller and buyer fee transfer.
pub const FILL_SCALAR_TOTAL_FEE_V4: usize = 64;
/// Seller claim reserve after this match.
pub const FILL_SCALAR_SELLER_RESERVED_CLAIMS_AFTER_V4: usize = 65;
/// Buyer collateral reserve after this match.
pub const FILL_SCALAR_BUYER_RESERVED_COLLATERAL_AFTER_V4: usize = 66;
/// One exactly when the seller record becomes terminal.
pub const FILL_SCALAR_SELLER_TERMINAL_V4: usize = 67;
/// One exactly when the buyer record becomes terminal.
pub const FILL_SCALAR_BUYER_TERMINAL_V4: usize = 68;
/// Seller maker live count after optional terminal close.
pub const FILL_SCALAR_SELLER_LIVE_COUNT_AFTER_V4: usize = 69;
/// Buyer maker live count after optional terminal close.
pub const FILL_SCALAR_BUYER_LIVE_COUNT_AFTER_V4: usize = 70;
/// Temporary current seller fee recomputation.
pub const FILL_SCALAR_SELLER_CURRENT_FEE_CHECK_V4: usize = 71;
/// Temporary current buyer fee recomputation.
pub const FILL_SCALAR_BUYER_CURRENT_FEE_CHECK_V4: usize = 72;
/// Temporary current seller remaining quantity.
pub const FILL_SCALAR_SELLER_CURRENT_REMAINING_V4: usize = 73;
/// Temporary initial buyer gross reserve.
pub const FILL_SCALAR_BUYER_INITIAL_GROSS_V4: usize = 74;
/// Temporary initial buyer fee reserve.
pub const FILL_SCALAR_BUYER_INITIAL_FEE_V4: usize = 75;
/// Temporary total initial buyer reserve.
pub const FILL_SCALAR_BUYER_INITIAL_RESERVE_V4: usize = 76;
/// Temporary buyer amount already spent.
pub const FILL_SCALAR_BUYER_SPENT_V4: usize = 77;
/// Temporary expected current buyer reserve.
pub const FILL_SCALAR_BUYER_CURRENT_RESERVE_CHECK_V4: usize = 78;
/// Conservation scratch: seller net plus combined fee.
pub const FILL_SCALAR_CONSERVATION_V4: usize = 79;
/// Seller record Position expected revision.
pub const FILL_SCALAR_CLAIM_SOURCE_REVISION_V4: usize = 80;
/// Buyer Position expected revision.
pub const FILL_SCALAR_CLAIM_DESTINATION_REVISION_V4: usize = 81;
/// Seller record Position resulting revision.
pub const FILL_SCALAR_CLAIM_SOURCE_REVISION_AFTER_V4: usize = 82;
/// Buyer Position resulting revision.
pub const FILL_SCALAR_CLAIM_DESTINATION_REVISION_AFTER_V4: usize = 83;
/// Buyer Custody replay revision before the first transfer.
pub const FILL_SCALAR_CUSTODY_REVISION_V4: usize = 84;
/// Buyer Custody revision after the seller transfer.
pub const FILL_SCALAR_CUSTODY_REVISION_AFTER_SELLER_V4: usize = 85;
/// Buyer Custody revision after the fee transfer.
pub const FILL_SCALAR_CUSTODY_REVISION_AFTER_FEE_V4: usize = 86;
/// Final terminal constant for child delegated transfer envelopes.
pub const FILL_SCALAR_TERMINAL_V4: usize = 87;
/// Seller maker replay historical rent principal.
pub const FILL_SCALAR_SELLER_MAKER_RENT_PRINCIPAL_V4: usize = 88;
/// Seller registered-record historical rent principal.
pub const FILL_SCALAR_SELLER_RECORD_RENT_PRINCIPAL_V4: usize = 89;
/// Buyer maker replay historical rent principal.
pub const FILL_SCALAR_BUYER_MAKER_RENT_PRINCIPAL_V4: usize = 90;
/// Buyer registered-record historical rent principal.
pub const FILL_SCALAR_BUYER_RECORD_RENT_PRINCIPAL_V4: usize = 91;

/// Parent request digest seeded by common Hot.
pub const FILL_IDENTITY_PARENT_REQUEST_V4: usize = 0;
/// Authenticated Core Market.
pub const FILL_IDENTITY_MARKET_V4: usize = 1;
/// Selected release set.
pub const FILL_IDENTITY_RELEASE_SET_V4: usize = 2;
/// Authenticated Product record digest.
pub const FILL_IDENTITY_PRODUCT_RECORD_V4: usize = 3;
/// Product semantic LiabilityBasis identity.
pub const FILL_IDENTITY_SEMANTIC_BASIS_V4: usize = 4;
/// Authenticated raw ProductBasis digest.
pub const FILL_IDENTITY_LINKED_BASIS_V4: usize = 5;
/// Registry-selected Trading program.
pub const FILL_IDENTITY_TRADING_PROGRAM_V4: usize = 6;
/// Authenticated Realm.
pub const FILL_IDENTITY_REALM_V4: usize = 7;
/// Realm collateral mint.
pub const FILL_IDENTITY_MINT_V4: usize = 8;
/// Realm token program.
pub const FILL_IDENTITY_TOKEN_PROGRAM_V4: usize = 9;
/// Immutable fee recipient.
pub const FILL_IDENTITY_FEE_RECIPIENT_V4: usize = 10;
/// Seller record maker.
pub const FILL_IDENTITY_SELLER_MAKER_V4: usize = 11;
/// Buyer record maker.
pub const FILL_IDENTITY_BUYER_MAKER_V4: usize = 12;
/// Seller intent Market.
pub const FILL_IDENTITY_SELLER_INTENT_MARKET_V4: usize = 13;
/// Buyer intent Market.
pub const FILL_IDENTITY_BUYER_INTENT_MARKET_V4: usize = 14;
/// Seller maker replay Market.
pub const FILL_IDENTITY_SELLER_MAKER_MARKET_V4: usize = 15;
/// Buyer maker replay Market.
pub const FILL_IDENTITY_BUYER_MAKER_MARKET_V4: usize = 16;
/// Seller record account.
pub const FILL_IDENTITY_SELLER_RECORD_V4: usize = 17;
/// Buyer record account.
pub const FILL_IDENTITY_BUYER_RECORD_V4: usize = 18;
/// Seller maker replay account.
pub const FILL_IDENTITY_SELLER_MAKER_STATE_V4: usize = 19;
/// Buyer maker replay account.
pub const FILL_IDENTITY_BUYER_MAKER_STATE_V4: usize = 20;
/// Signed seller collateral destination.
pub const FILL_IDENTITY_SELLER_COLLATERAL_DESTINATION_V4: usize = 21;
/// Signed buyer collateral refund account.
pub const FILL_IDENTITY_BUYER_COLLATERAL_REFUND_V4: usize = 22;
/// Buyer record-keyed Custody vault.
pub const FILL_IDENTITY_BUYER_CUSTODY_VAULT_V4: usize = 23;
/// Custody transfer authority.
pub const FILL_IDENTITY_CUSTODY_AUTHORITY_V4: usize = 24;
/// Claims aggregate selected by the Product basis.
pub const FILL_IDENTITY_CLAIMS_AGGREGATE_V4: usize = 25;
/// Seller record Position owner.
pub const FILL_IDENTITY_CLAIM_SOURCE_OWNER_V4: usize = 26;
/// Buyer user Position owner.
pub const FILL_IDENTITY_CLAIM_DESTINATION_OWNER_V4: usize = 27;
/// Seller record RentCredit beneficiary.
pub const FILL_IDENTITY_SELLER_RENT_OWNER_V4: usize = 28;
/// Buyer record RentCredit beneficiary.
pub const FILL_IDENTITY_BUYER_RENT_OWNER_V4: usize = 29;
/// Seller identity stored in the maker replay account.
pub const FILL_IDENTITY_SELLER_MAKER_REPLAY_OWNER_V4: usize = 30;
/// Buyer identity stored in the maker replay account.
pub const FILL_IDENTITY_BUYER_MAKER_REPLAY_OWNER_V4: usize = 31;

const REQUEST_OPERATIONS: usize = 8;
const TRANSITION_INSTRUCTIONS: usize = 99;

/// Exact unsigned RequestProfileV1 width.
pub const DIRECT_REGISTERED_FILL_REQUEST_PROFILE_BYTES_V4: usize =
    dclutch_request_profile_contract::HEADER_BYTES
        + REQUEST_OPERATIONS * dclutch_request_profile_contract::OPERATION_BYTES;
/// Exact registered-fill TransitionVMV3 width.
pub const DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4: usize =
    dclutch_transition_vm::v3::HEADER_BYTES
        + TRANSITION_INSTRUCTIONS * dclutch_transition_vm::v3::INSTRUCTION_BYTES;
/// Exact interpreted ExecutionStrategy width.
pub const DIRECT_REGISTERED_FILL_STRATEGY_BYTES_V4: usize = EXECUTION_STRATEGY_PROGRAM_BYTES_V2;

/// Stable registered fill artifact refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRegisteredFillArtifactErrorV4 {
    /// A register or byte coordinate did not fit.
    Coordinate,
    /// RequestProfile construction or hostile decoding refused.
    RequestProfile,
    /// Transition construction or hostile decoding refused.
    Transition,
    /// Interpreted strategy construction refused.
    Strategy,
}

/// Emit the exact unsigned registered-fill RequestProfileV1 atomically.
pub fn encode_direct_registered_fill_request_profile_v4_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredFillArtifactErrorV4> {
    if scratch.len() != DIRECT_REGISTERED_FILL_REQUEST_PROFILE_BYTES_V4
        || output.len() != DIRECT_REGISTERED_FILL_REQUEST_PROFILE_BYTES_V4
    {
        return Err(DirectRegisteredFillArtifactErrorV4::Coordinate);
    }
    let instructions = [
        RequestInstructionV1::require_u64(
            RequestCoordinateV1::fixed(0),
            u64::from_le_bytes(DIRECT_EXECUTION_REQUEST_MAGIC_V3),
        ),
        RequestInstructionV1::require_u16(
            RequestCoordinateV1::fixed(8),
            DIRECT_EXECUTION_REQUEST_VERSION_V3,
        ),
        RequestInstructionV1::require_zero(RequestCoordinateV1::fixed(10), 2),
        RequestInstructionV1::require_u32(
            RequestCoordinateV1::fixed(12),
            DirectExecutionActionV3::FillRegisteredOrdinary as u32,
        ),
        RequestInstructionV1::require_u32(RequestCoordinateV1::fixed(16), 16),
        RequestInstructionV1::require_zero(RequestCoordinateV1::fixed(20), 12),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(32),
            scalar_request(FILL_SCALAR_QUANTITY_V4)?,
        ),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(40),
            scalar_request(FILL_SCALAR_EXECUTION_PRICE_V4)?,
        ),
    ];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            width32(DIRECT_REGISTERED_FILL_REQUEST_BYTES_V3)?,
            0,
            width16(DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4)?,
            DIRECT_REGISTERED_FILL_ITEM_SCALAR_STRIDE_V4,
            width16(DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4)?,
            DIRECT_REGISTERED_FILL_ITEM_IDENTITY_STRIDE_V4,
        ),
        &instructions,
        &[],
        scratch,
        output,
    )
    .map_err(|_| DirectRegisteredFillArtifactErrorV4::RequestProfile)
}

/// Emit the exact registered ordinary candidate transition atomically.
pub fn encode_direct_registered_fill_transition_v4_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredFillArtifactErrorV4> {
    if scratch.len() != DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4
        || output.len() != DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4
    {
        return Err(DirectRegisteredFillArtifactErrorV4::Coordinate);
    }
    let instructions = transition_instructions()?;
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: width16(DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4)?,
            item_scalar_stride: DIRECT_REGISTERED_FILL_ITEM_SCALAR_STRIDE_V4,
            common_identities: width16(DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4)?,
            item_identity_stride: DIRECT_REGISTERED_FILL_ITEM_IDENTITY_STRIDE_V4,
        },
        &instructions,
        &[],
        &[],
        scratch,
        output,
    )
    .map_err(|_| DirectRegisteredFillArtifactErrorV4::Transition)
}

/// Construct the canonical interpreted strategy selecting `transition_id`.
pub fn direct_registered_fill_strategy_v4(
    transition_id: [u8; 32],
) -> Result<[u8; DIRECT_REGISTERED_FILL_STRATEGY_BYTES_V4], DirectRegisteredFillArtifactErrorV4> {
    let transition =
        ContentId::new(transition_id).map_err(|_| DirectRegisteredFillArtifactErrorV4::Strategy)?;
    let strategy = ExecutionStrategyProgramV2::new(
        StrategyDispositionV2::Interpreted,
        ContentId::new(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)
            .map_err(|_| DirectRegisteredFillArtifactErrorV4::Strategy)?,
        transition,
        ContentId::new(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)
            .map_err(|_| DirectRegisteredFillArtifactErrorV4::Strategy)?,
        None,
        ContentId::new(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2)
            .map_err(|_| DirectRegisteredFillArtifactErrorV4::Strategy)?,
        None,
        ContentId::new(ACCELERATOR_REQUEST_SCHEMA_ID_V2)
            .map_err(|_| DirectRegisteredFillArtifactErrorV4::Strategy)?,
        ContentId::new(ACCELERATOR_ACK_SCHEMA_ID_V2)
            .map_err(|_| DirectRegisteredFillArtifactErrorV4::Strategy)?,
    )
    .map_err(|_| DirectRegisteredFillArtifactErrorV4::Strategy)?;
    Ok(strategy.to_bytes())
}

fn transition_instructions()
-> Result<[InstructionV3; TRANSITION_INSTRUCTIONS], DirectRegisteredFillArtifactErrorV4> {
    let s = scalar_transition;
    let i = identity_transition;
    Ok([
        InstructionV3::load_const(s(FILL_SCALAR_ZERO_V4)?, 0),
        InstructionV3::load_const(s(FILL_SCALAR_ONE_V4)?, 1),
        InstructionV3::load_const(s(FILL_SCALAR_GTC_V4)?, 2),
        InstructionV3::load_const(
            s(FILL_SCALAR_FEE_DENOMINATOR_V4)?,
            u64::from(DIRECT_FEE_DENOMINATOR_V1),
        ),
        InstructionV3::load_const(s(FILL_SCALAR_TERMINAL_V4)?, 1),
        InstructionV3::scalar_eq(s(FILL_SCALAR_ROOT_PHASE_V4)?, s(FILL_SCALAR_ZERO_V4)?),
        InstructionV3::nonzero(s(FILL_SCALAR_ROOT_OPEN_COUNT_V4)?),
        InstructionV3::nonzero(s(FILL_SCALAR_QUANTITY_V4)?),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_EXECUTION_PRICE_V4)?,
            s(FILL_SCALAR_PRICE_SCALE_V4)?,
        ),
        InstructionV3::identity_eq(
            i(FILL_IDENTITY_MARKET_V4)?,
            i(FILL_IDENTITY_SELLER_INTENT_MARKET_V4)?,
        ),
        InstructionV3::identity_eq(
            i(FILL_IDENTITY_MARKET_V4)?,
            i(FILL_IDENTITY_BUYER_INTENT_MARKET_V4)?,
        ),
        InstructionV3::identity_eq(
            i(FILL_IDENTITY_MARKET_V4)?,
            i(FILL_IDENTITY_SELLER_MAKER_MARKET_V4)?,
        ),
        InstructionV3::identity_eq(
            i(FILL_IDENTITY_MARKET_V4)?,
            i(FILL_IDENTITY_BUYER_MAKER_MARKET_V4)?,
        ),
        InstructionV3::identity_ne(
            i(FILL_IDENTITY_SELLER_MAKER_V4)?,
            i(FILL_IDENTITY_BUYER_MAKER_V4)?,
        ),
        InstructionV3::identity_eq(
            i(FILL_IDENTITY_SELLER_MAKER_V4)?,
            i(FILL_IDENTITY_SELLER_MAKER_REPLAY_OWNER_V4)?,
        ),
        InstructionV3::identity_eq(
            i(FILL_IDENTITY_BUYER_MAKER_V4)?,
            i(FILL_IDENTITY_BUYER_MAKER_REPLAY_OWNER_V4)?,
        ),
        InstructionV3::scalar_eq(
            s(FILL_SCALAR_MARKET_GENERATION_V4)?,
            s(FILL_SCALAR_SELLER_GENERATION_V4)?,
        ),
        InstructionV3::scalar_eq(
            s(FILL_SCALAR_MARKET_GENERATION_V4)?,
            s(FILL_SCALAR_BUYER_GENERATION_V4)?,
        ),
        InstructionV3::scalar_eq(
            s(FILL_SCALAR_MARKET_GENERATION_V4)?,
            s(FILL_SCALAR_SELLER_MAKER_GENERATION_V4)?,
        ),
        InstructionV3::scalar_eq(
            s(FILL_SCALAR_MARKET_GENERATION_V4)?,
            s(FILL_SCALAR_BUYER_MAKER_GENERATION_V4)?,
        ),
        InstructionV3::scalar_eq(s(FILL_SCALAR_SELLER_SIDE_V4)?, s(FILL_SCALAR_ZERO_V4)?),
        InstructionV3::scalar_eq(s(FILL_SCALAR_BUYER_SIDE_V4)?, s(FILL_SCALAR_ONE_V4)?),
        InstructionV3::scalar_eq(s(FILL_SCALAR_SELLER_LIFECYCLE_V4)?, s(FILL_SCALAR_GTC_V4)?),
        InstructionV3::scalar_eq(s(FILL_SCALAR_BUYER_LIFECYCLE_V4)?, s(FILL_SCALAR_GTC_V4)?),
        InstructionV3::scalar_eq(
            s(FILL_SCALAR_SELLER_OUTCOME_V4)?,
            s(FILL_SCALAR_BUYER_OUTCOME_V4)?,
        ),
        InstructionV3::scalar_lt(
            s(FILL_SCALAR_SELLER_OUTCOME_V4)?,
            s(FILL_SCALAR_OUTCOME_COUNT_V4)?,
        ),
        InstructionV3::scalar_eq(
            s(FILL_SCALAR_SELLER_FEE_BPS_V4)?,
            s(FILL_SCALAR_POLICY_FEE_BPS_V4)?,
        ),
        InstructionV3::scalar_eq(
            s(FILL_SCALAR_BUYER_FEE_BPS_V4)?,
            s(FILL_SCALAR_POLICY_FEE_BPS_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_SELLER_VALID_FROM_V4)?,
            s(FILL_SCALAR_SLOT_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_SLOT_V4)?,
            s(FILL_SCALAR_SELLER_VALID_THROUGH_V4)?,
        ),
        InstructionV3::scalar_le(s(FILL_SCALAR_BUYER_VALID_FROM_V4)?, s(FILL_SCALAR_SLOT_V4)?),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_SLOT_V4)?,
            s(FILL_SCALAR_BUYER_VALID_THROUGH_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_SELLER_LIMIT_V4)?,
            s(FILL_SCALAR_EXECUTION_PRICE_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_EXECUTION_PRICE_V4)?,
            s(FILL_SCALAR_BUYER_LIMIT_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_BUYER_LIMIT_V4)?,
            s(FILL_SCALAR_PRICE_SCALE_V4)?,
        ),
        InstructionV3::scalar_lt(
            s(FILL_SCALAR_SELLER_NONCE_V4)?,
            s(FILL_SCALAR_SELLER_NEXT_NONCE_V4)?,
        ),
        InstructionV3::scalar_lt(
            s(FILL_SCALAR_BUYER_NONCE_V4)?,
            s(FILL_SCALAR_BUYER_NEXT_NONCE_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_SELLER_MINIMUM_NONCE_V4)?,
            s(FILL_SCALAR_SELLER_NONCE_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_BUYER_MINIMUM_NONCE_V4)?,
            s(FILL_SCALAR_BUYER_NONCE_V4)?,
        ),
        InstructionV3::nonzero(s(FILL_SCALAR_SELLER_LIVE_COUNT_V4)?),
        InstructionV3::nonzero(s(FILL_SCALAR_BUYER_LIVE_COUNT_V4)?),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_SELLER_LIVE_COUNT_V4)?,
            s(FILL_SCALAR_SELLER_NEXT_NONCE_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_SELLER_MINIMUM_NONCE_V4)?,
            s(FILL_SCALAR_SELLER_NEXT_NONCE_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_BUYER_LIVE_COUNT_V4)?,
            s(FILL_SCALAR_BUYER_NEXT_NONCE_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_BUYER_MINIMUM_NONCE_V4)?,
            s(FILL_SCALAR_BUYER_NEXT_NONCE_V4)?,
        ),
        InstructionV3::nonzero(s(FILL_SCALAR_SELLER_MAKER_RENT_PRINCIPAL_V4)?),
        InstructionV3::nonzero(s(FILL_SCALAR_SELLER_RECORD_RENT_PRINCIPAL_V4)?),
        InstructionV3::nonzero(s(FILL_SCALAR_BUYER_MAKER_RENT_PRINCIPAL_V4)?),
        InstructionV3::nonzero(s(FILL_SCALAR_BUYER_RECORD_RENT_PRINCIPAL_V4)?),
        InstructionV3::scalar_lt(
            s(FILL_SCALAR_SELLER_FILLED_V4)?,
            s(FILL_SCALAR_SELLER_MAXIMUM_V4)?,
        ),
        InstructionV3::scalar_lt(
            s(FILL_SCALAR_BUYER_FILLED_V4)?,
            s(FILL_SCALAR_BUYER_MAXIMUM_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_SELLER_CUMULATIVE_GROSS_V4)?,
            s(FILL_SCALAR_SELLER_FILLED_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_BUYER_CUMULATIVE_GROSS_V4)?,
            s(FILL_SCALAR_BUYER_FILLED_V4)?,
        ),
        InstructionV3::mul_div_floor(
            s(FILL_SCALAR_SELLER_CUMULATIVE_GROSS_V4)?,
            s(FILL_SCALAR_POLICY_FEE_BPS_V4)?,
            s(FILL_SCALAR_FEE_DENOMINATOR_V4)?,
            s(FILL_SCALAR_SELLER_CURRENT_FEE_CHECK_V4)?,
        ),
        InstructionV3::scalar_eq(
            s(FILL_SCALAR_SELLER_CURRENT_FEE_CHECK_V4)?,
            s(FILL_SCALAR_SELLER_CUMULATIVE_FEE_V4)?,
        ),
        InstructionV3::mul_div_floor(
            s(FILL_SCALAR_BUYER_CUMULATIVE_GROSS_V4)?,
            s(FILL_SCALAR_POLICY_FEE_BPS_V4)?,
            s(FILL_SCALAR_FEE_DENOMINATOR_V4)?,
            s(FILL_SCALAR_BUYER_CURRENT_FEE_CHECK_V4)?,
        ),
        InstructionV3::scalar_eq(
            s(FILL_SCALAR_BUYER_CURRENT_FEE_CHECK_V4)?,
            s(FILL_SCALAR_BUYER_CUMULATIVE_FEE_V4)?,
        ),
        InstructionV3::sub_into(
            s(FILL_SCALAR_SELLER_MAXIMUM_V4)?,
            s(FILL_SCALAR_SELLER_FILLED_V4)?,
            s(FILL_SCALAR_SELLER_CURRENT_REMAINING_V4)?,
        ),
        InstructionV3::scalar_eq(
            s(FILL_SCALAR_SELLER_CURRENT_REMAINING_V4)?,
            s(FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4)?,
        ),
        InstructionV3::scalar_eq(
            s(FILL_SCALAR_SELLER_RESERVED_COLLATERAL_V4)?,
            s(FILL_SCALAR_ZERO_V4)?,
        ),
        InstructionV3::mul_div_floor(
            s(FILL_SCALAR_BUYER_MAXIMUM_V4)?,
            s(FILL_SCALAR_BUYER_LIMIT_V4)?,
            s(FILL_SCALAR_PRICE_SCALE_V4)?,
            s(FILL_SCALAR_BUYER_INITIAL_GROSS_V4)?,
        ),
        InstructionV3::mul_div_floor(
            s(FILL_SCALAR_BUYER_INITIAL_GROSS_V4)?,
            s(FILL_SCALAR_POLICY_FEE_BPS_V4)?,
            s(FILL_SCALAR_FEE_DENOMINATOR_V4)?,
            s(FILL_SCALAR_BUYER_INITIAL_FEE_V4)?,
        ),
        InstructionV3::checked_add_into(
            s(FILL_SCALAR_BUYER_INITIAL_GROSS_V4)?,
            s(FILL_SCALAR_BUYER_INITIAL_FEE_V4)?,
            s(FILL_SCALAR_BUYER_INITIAL_RESERVE_V4)?,
        ),
        InstructionV3::checked_add_into(
            s(FILL_SCALAR_BUYER_CUMULATIVE_GROSS_V4)?,
            s(FILL_SCALAR_BUYER_CUMULATIVE_FEE_V4)?,
            s(FILL_SCALAR_BUYER_SPENT_V4)?,
        ),
        InstructionV3::sub_into(
            s(FILL_SCALAR_BUYER_INITIAL_RESERVE_V4)?,
            s(FILL_SCALAR_BUYER_SPENT_V4)?,
            s(FILL_SCALAR_BUYER_CURRENT_RESERVE_CHECK_V4)?,
        ),
        InstructionV3::scalar_eq(
            s(FILL_SCALAR_BUYER_CURRENT_RESERVE_CHECK_V4)?,
            s(FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4)?,
        ),
        InstructionV3::scalar_eq(
            s(FILL_SCALAR_BUYER_RESERVED_CLAIMS_V4)?,
            s(FILL_SCALAR_ZERO_V4)?,
        ),
        InstructionV3::checked_add_into(
            s(FILL_SCALAR_SELLER_FILLED_V4)?,
            s(FILL_SCALAR_QUANTITY_V4)?,
            s(FILL_SCALAR_SELLER_FILLED_AFTER_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_SELLER_FILLED_AFTER_V4)?,
            s(FILL_SCALAR_SELLER_MAXIMUM_V4)?,
        ),
        InstructionV3::checked_add_into(
            s(FILL_SCALAR_BUYER_FILLED_V4)?,
            s(FILL_SCALAR_QUANTITY_V4)?,
            s(FILL_SCALAR_BUYER_FILLED_AFTER_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_BUYER_FILLED_AFTER_V4)?,
            s(FILL_SCALAR_BUYER_MAXIMUM_V4)?,
        ),
        InstructionV3::sub_into(
            s(FILL_SCALAR_SELLER_MAXIMUM_V4)?,
            s(FILL_SCALAR_SELLER_FILLED_AFTER_V4)?,
            s(FILL_SCALAR_SELLER_REMAINING_AFTER_V4)?,
        ),
        InstructionV3::sub_into(
            s(FILL_SCALAR_BUYER_MAXIMUM_V4)?,
            s(FILL_SCALAR_BUYER_FILLED_AFTER_V4)?,
            s(FILL_SCALAR_BUYER_REMAINING_AFTER_V4)?,
        ),
        InstructionV3::mul_div_exact(
            s(FILL_SCALAR_QUANTITY_V4)?,
            s(FILL_SCALAR_EXECUTION_PRICE_V4)?,
            s(FILL_SCALAR_PRICE_SCALE_V4)?,
            s(FILL_SCALAR_GROSS_V4)?,
        ),
        InstructionV3::checked_add_into(
            s(FILL_SCALAR_SELLER_CUMULATIVE_GROSS_V4)?,
            s(FILL_SCALAR_GROSS_V4)?,
            s(FILL_SCALAR_SELLER_CUMULATIVE_GROSS_AFTER_V4)?,
        ),
        InstructionV3::checked_add_into(
            s(FILL_SCALAR_BUYER_CUMULATIVE_GROSS_V4)?,
            s(FILL_SCALAR_GROSS_V4)?,
            s(FILL_SCALAR_BUYER_CUMULATIVE_GROSS_AFTER_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_SELLER_CUMULATIVE_GROSS_AFTER_V4)?,
            s(FILL_SCALAR_SELLER_FILLED_AFTER_V4)?,
        ),
        InstructionV3::scalar_le(
            s(FILL_SCALAR_BUYER_CUMULATIVE_GROSS_AFTER_V4)?,
            s(FILL_SCALAR_BUYER_FILLED_AFTER_V4)?,
        ),
        InstructionV3::mul_div_floor(
            s(FILL_SCALAR_SELLER_CUMULATIVE_GROSS_AFTER_V4)?,
            s(FILL_SCALAR_POLICY_FEE_BPS_V4)?,
            s(FILL_SCALAR_FEE_DENOMINATOR_V4)?,
            s(FILL_SCALAR_SELLER_CUMULATIVE_FEE_AFTER_V4)?,
        ),
        InstructionV3::mul_div_floor(
            s(FILL_SCALAR_BUYER_CUMULATIVE_GROSS_AFTER_V4)?,
            s(FILL_SCALAR_POLICY_FEE_BPS_V4)?,
            s(FILL_SCALAR_FEE_DENOMINATOR_V4)?,
            s(FILL_SCALAR_BUYER_CUMULATIVE_FEE_AFTER_V4)?,
        ),
        InstructionV3::sub_into(
            s(FILL_SCALAR_SELLER_CUMULATIVE_FEE_AFTER_V4)?,
            s(FILL_SCALAR_SELLER_CUMULATIVE_FEE_V4)?,
            s(FILL_SCALAR_SELLER_FEE_DELTA_V4)?,
        ),
        InstructionV3::sub_into(
            s(FILL_SCALAR_BUYER_CUMULATIVE_FEE_AFTER_V4)?,
            s(FILL_SCALAR_BUYER_CUMULATIVE_FEE_V4)?,
            s(FILL_SCALAR_BUYER_FEE_DELTA_V4)?,
        ),
        InstructionV3::sub_into(
            s(FILL_SCALAR_GROSS_V4)?,
            s(FILL_SCALAR_SELLER_FEE_DELTA_V4)?,
            s(FILL_SCALAR_SELLER_NET_V4)?,
        ),
        InstructionV3::checked_add_into(
            s(FILL_SCALAR_GROSS_V4)?,
            s(FILL_SCALAR_BUYER_FEE_DELTA_V4)?,
            s(FILL_SCALAR_BUYER_DEBIT_V4)?,
        ),
        InstructionV3::checked_add_into(
            s(FILL_SCALAR_SELLER_FEE_DELTA_V4)?,
            s(FILL_SCALAR_BUYER_FEE_DELTA_V4)?,
            s(FILL_SCALAR_TOTAL_FEE_V4)?,
        ),
        InstructionV3::checked_add_into(
            s(FILL_SCALAR_SELLER_NET_V4)?,
            s(FILL_SCALAR_TOTAL_FEE_V4)?,
            s(FILL_SCALAR_CONSERVATION_V4)?,
        ),
        InstructionV3::scalar_eq(
            s(FILL_SCALAR_CONSERVATION_V4)?,
            s(FILL_SCALAR_BUYER_DEBIT_V4)?,
        ),
        InstructionV3::sub_into(
            s(FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4)?,
            s(FILL_SCALAR_QUANTITY_V4)?,
            s(FILL_SCALAR_SELLER_RESERVED_CLAIMS_AFTER_V4)?,
        ),
        InstructionV3::sub_into(
            s(FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4)?,
            s(FILL_SCALAR_BUYER_DEBIT_V4)?,
            s(FILL_SCALAR_BUYER_RESERVED_COLLATERAL_AFTER_V4)?,
        ),
        InstructionV3::load_const(s(FILL_SCALAR_SELLER_TERMINAL_V4)?, 0),
        InstructionV3::select_zero(
            s(FILL_SCALAR_SELLER_REMAINING_AFTER_V4)?,
            s(FILL_SCALAR_ONE_V4)?,
            s(FILL_SCALAR_SELLER_TERMINAL_V4)?,
        ),
        InstructionV3::load_const(s(FILL_SCALAR_BUYER_TERMINAL_V4)?, 0),
        InstructionV3::select_zero(
            s(FILL_SCALAR_BUYER_REMAINING_AFTER_V4)?,
            s(FILL_SCALAR_ONE_V4)?,
            s(FILL_SCALAR_BUYER_TERMINAL_V4)?,
        ),
        InstructionV3::sub_into(
            s(FILL_SCALAR_SELLER_LIVE_COUNT_V4)?,
            s(FILL_SCALAR_SELLER_TERMINAL_V4)?,
            s(FILL_SCALAR_SELLER_LIVE_COUNT_AFTER_V4)?,
        ),
        InstructionV3::sub_into(
            s(FILL_SCALAR_BUYER_LIVE_COUNT_V4)?,
            s(FILL_SCALAR_BUYER_TERMINAL_V4)?,
            s(FILL_SCALAR_BUYER_LIVE_COUNT_AFTER_V4)?,
        ),
        InstructionV3::increment_into(
            s(FILL_SCALAR_CLAIM_SOURCE_REVISION_V4)?,
            s(FILL_SCALAR_CLAIM_SOURCE_REVISION_AFTER_V4)?,
        ),
        InstructionV3::increment_into(
            s(FILL_SCALAR_CLAIM_DESTINATION_REVISION_V4)?,
            s(FILL_SCALAR_CLAIM_DESTINATION_REVISION_AFTER_V4)?,
        ),
        InstructionV3::increment_into(
            s(FILL_SCALAR_CUSTODY_REVISION_V4)?,
            s(FILL_SCALAR_CUSTODY_REVISION_AFTER_SELLER_V4)?,
        ),
        InstructionV3::increment_into(
            s(FILL_SCALAR_CUSTODY_REVISION_AFTER_SELLER_V4)?,
            s(FILL_SCALAR_CUSTODY_REVISION_AFTER_FEE_V4)?,
        ),
    ])
}

fn scalar_request(value: usize) -> Result<ScalarRegisterV1, DirectRegisteredFillArtifactErrorV4> {
    Ok(ScalarRegisterV1::common(width16(value)?))
}

fn scalar_transition(
    value: usize,
) -> Result<ScalarRegisterV3, DirectRegisteredFillArtifactErrorV4> {
    Ok(ScalarRegisterV3::common(width16(value)?))
}

fn identity_transition(
    value: usize,
) -> Result<IdentityRegisterV3, DirectRegisteredFillArtifactErrorV4> {
    Ok(IdentityRegisterV3::common(width16(value)?))
}

fn width16(value: usize) -> Result<u16, DirectRegisteredFillArtifactErrorV4> {
    u16::try_from(value).map_err(|_| DirectRegisteredFillArtifactErrorV4::Coordinate)
}

fn width32(value: usize) -> Result<u32, DirectRegisteredFillArtifactErrorV4> {
    u32::try_from(value).map_err(|_| DirectRegisteredFillArtifactErrorV4::Coordinate)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use dclutch_request_profile_contract::{
        ProjectionRegistersV1, RequestProfileV1, project_atomic,
    };
    use dclutch_transition_vm::v3::{
        ProgramV3, RegisterInput, RegisterOutput, execute_fold_atomic,
    };

    use super::*;
    use crate::{
        execution_v3::{DirectExecutionQuantityV3, DirectExecutionRequestV3},
        registered_requests_v4::encode_direct_registered_execution_request_v3_atomic,
    };

    fn valid_scalars() -> [u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4] {
        let mut scalars = [0_u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4];
        scalars[FILL_SCALAR_SLOT_V4] = 100;
        scalars[FILL_SCALAR_OUTCOME_COUNT_V4] = 3;
        scalars[FILL_SCALAR_MARKET_GENERATION_V4] = 7;
        scalars[FILL_SCALAR_PRICE_SCALE_V4] = 100;
        scalars[FILL_SCALAR_POLICY_FEE_BPS_V4] = 100;
        scalars[FILL_SCALAR_ROOT_OPEN_COUNT_V4] = 2;
        for (base, side, limit) in [(13, 0, 40), (32, 1, 60)] {
            scalars
                .get_mut(base..base + 19)
                .expect("participant register span")
                .copy_from_slice(&[
                    side, 2, 1, 7, 0, 90, 110, 20, limit, 100, 0, 0, 0, 0, 0, 1, 1, 0, 7,
                ]);
        }
        scalars[FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4] = 20;
        scalars[FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4] = 12;
        scalars[FILL_SCALAR_CLAIM_SOURCE_REVISION_V4] = 4;
        scalars[FILL_SCALAR_CLAIM_DESTINATION_REVISION_V4] = 9;
        scalars[FILL_SCALAR_CUSTODY_REVISION_V4] = 3;
        scalars[FILL_SCALAR_SELLER_MAKER_RENT_PRINCIPAL_V4] = 1;
        scalars[FILL_SCALAR_SELLER_RECORD_RENT_PRINCIPAL_V4] = 1;
        scalars[FILL_SCALAR_BUYER_MAKER_RENT_PRINCIPAL_V4] = 1;
        scalars[FILL_SCALAR_BUYER_RECORD_RENT_PRINCIPAL_V4] = 1;
        scalars
    }

    fn valid_identities() -> [[u8; 32]; DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4] {
        let mut identities = [[1_u8; 32]; DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4];
        identities[FILL_IDENTITY_MARKET_V4] = [2; 32];
        identities[FILL_IDENTITY_SELLER_INTENT_MARKET_V4] = [2; 32];
        identities[FILL_IDENTITY_BUYER_INTENT_MARKET_V4] = [2; 32];
        identities[FILL_IDENTITY_SELLER_MAKER_MARKET_V4] = [2; 32];
        identities[FILL_IDENTITY_BUYER_MAKER_MARKET_V4] = [2; 32];
        identities[FILL_IDENTITY_SELLER_MAKER_V4] = [3; 32];
        identities[FILL_IDENTITY_BUYER_MAKER_V4] = [4; 32];
        identities[FILL_IDENTITY_SELLER_MAKER_REPLAY_OWNER_V4] = [3; 32];
        identities[FILL_IDENTITY_BUYER_MAKER_REPLAY_OWNER_V4] = [4; 32];
        identities
    }

    #[test]
    fn request_and_transition_derive_conserving_partial_fill() {
        let mut request = [0_u8; DIRECT_REGISTERED_FILL_REQUEST_BYTES_V3];
        encode_direct_registered_execution_request_v3_atomic(
            DirectExecutionActionV3::FillRegisteredOrdinary,
            DirectExecutionQuantityV3 {
                fill: 10,
                execution_price: 50,
            },
            3,
            &mut request,
        )
        .expect("request");
        assert!(matches!(
            DirectExecutionRequestV3::decode(&request, 3),
            Ok(DirectExecutionRequestV3::FillRegisteredOrdinary(_))
        ));
        let mut profile_scratch = [0_u8; DIRECT_REGISTERED_FILL_REQUEST_PROFILE_BYTES_V4];
        let mut profile_bytes = [0_u8; DIRECT_REGISTERED_FILL_REQUEST_PROFILE_BYTES_V4];
        encode_direct_registered_fill_request_profile_v4_atomic(
            &mut profile_scratch,
            &mut profile_bytes,
        )
        .expect("profile");
        let profile = RequestProfileV1::decode(&profile_bytes).expect("decode profile");
        let input_scalars = valid_scalars();
        let input_identities = valid_identities();
        let mut projection_scratch_scalars = input_scalars;
        let mut projection_scratch_identities = input_identities;
        let mut scalars = input_scalars;
        let mut identities = input_identities;
        project_atomic(
            profile,
            3,
            &request,
            ProjectionRegistersV1 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut projection_scratch_scalars,
                scratch_identities: &mut projection_scratch_identities,
                output_scalars: &mut scalars,
                output_identities: &mut identities,
            },
        )
        .expect("project");
        assert_eq!(scalars[FILL_SCALAR_QUANTITY_V4], 10);
        assert_eq!(scalars[FILL_SCALAR_EXECUTION_PRICE_V4], 50);

        let mut transition_scratch = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
        let mut transition_bytes = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
        encode_direct_registered_fill_transition_v4_atomic(
            &mut transition_scratch,
            &mut transition_bytes,
        )
        .expect("transition");
        let transition = ProgramV3::decode(&transition_bytes).expect("decode transition");
        let input = scalars;
        let mut scalar_scratch = input;
        let mut output = input;
        let mut identity_scratch = identities;
        let mut identity_output = identities;
        execute_fold_atomic(
            transition,
            3,
            RegisterInput {
                scalars: &input,
                identities: &identities,
            },
            RegisterOutput {
                scalars: &mut scalar_scratch,
                identities: &mut identity_scratch,
            },
            RegisterOutput {
                scalars: &mut output,
                identities: &mut identity_output,
            },
        )
        .expect("execute");
        assert_eq!(output[FILL_SCALAR_GROSS_V4], 5);
        assert_eq!(output[FILL_SCALAR_SELLER_FILLED_AFTER_V4], 10);
        assert_eq!(output[FILL_SCALAR_BUYER_FILLED_AFTER_V4], 10);
        assert_eq!(output[FILL_SCALAR_SELLER_RESERVED_CLAIMS_AFTER_V4], 10);
        assert_eq!(output[FILL_SCALAR_BUYER_RESERVED_COLLATERAL_AFTER_V4], 7);
        assert_eq!(output[FILL_SCALAR_SELLER_TERMINAL_V4], 0);
        assert_eq!(output[FILL_SCALAR_BUYER_TERMINAL_V4], 0);
        assert_eq!(output[FILL_SCALAR_CLAIM_SOURCE_REVISION_AFTER_V4], 5);
        assert_eq!(output[FILL_SCALAR_CLAIM_DESTINATION_REVISION_AFTER_V4], 10);
        assert_eq!(output[FILL_SCALAR_CUSTODY_REVISION_AFTER_FEE_V4], 5);
    }

    #[test]
    fn substituted_market_or_nonintegral_quote_refuses_without_output_commit() {
        let mut scratch = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
        let mut bytes = [0_u8; DIRECT_REGISTERED_FILL_TRANSITION_BYTES_V4];
        encode_direct_registered_fill_transition_v4_atomic(&mut scratch, &mut bytes)
            .expect("transition");
        let transition = ProgramV3::decode(&bytes).expect("decode");
        let mut input = valid_scalars();
        input[FILL_SCALAR_QUANTITY_V4] = 3;
        input[FILL_SCALAR_EXECUTION_PRICE_V4] = 50;
        let mut identities = valid_identities();
        identities[FILL_IDENTITY_BUYER_INTENT_MARKET_V4] = [9; 32];
        let mut scalar_scratch = input;
        let mut output = [0x55_u64; DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4];
        let before = output;
        let mut identity_scratch = identities;
        let mut identity_output = identities;
        assert!(
            execute_fold_atomic(
                transition,
                3,
                RegisterInput {
                    scalars: &input,
                    identities: &identities,
                },
                RegisterOutput {
                    scalars: &mut scalar_scratch,
                    identities: &mut identity_scratch,
                },
                RegisterOutput {
                    scalars: &mut output,
                    identities: &mut identity_output,
                },
            )
            .is_err()
        );
        assert_eq!(output, before);
    }
}
