//! Complete artifact family for terminal registered Direct records.
//!
//! `CancelRegistered` and `ExpireRegistered` share one authenticated state and
//! settlement topology.  Cancellation additionally binds the exact signed
//! CompactIntent to participant zero; expiry is unsigned and admits only when
//! the trusted current slot is strictly greater than the signed inclusive
//! `valid_through` bound.  The Transition derives the pure successor's full
//! residual refund and maker live-count candidate.  EffectV4 sends a Sell's
//! residual claims from the record Position to its maker, or sends a Buy's
//! residual collateral from its record-keyed Custody vault to the signed token
//! account and then closes the empty vault and replay.  LifecycleV5 alone
//! reclaims the Trading record to its immutable RentCredit.

use dclutch_account_profile_contract::{
    lifecycle_v3::{
        ACTION_PLAN_BYTES, HEADER_BYTES as LIFECYCLE_HEADER_BYTES,
        IMMUTABLE_IDENTITY_BINDING_BYTES, PROTECTED_OUTPUT_BYTES, RECIPE_BYTES, SEED_BYTES,
        StateLifecyclePolicyV5,
        encode::{
            LifecycleAccountCoordinateV3, LifecycleGuardInputV3,
            LifecycleImmutableIdentityBindingInputV4, LifecycleOperationInputV3,
            LifecyclePlanInputV3, LifecycleRecipeInputV3, LifecycleRegisterCoordinateV3,
            LifecycleSeedInputV3, encode_lifecycle_policy_v5_atomic,
        },
    },
    v2::{
        AccountPrestateV2, AccountProfileV2, FIXED_DATA_PREDICATE_BYTES,
        FIXED_DATA_PREDICATE_HEADER_BYTES, OPERATION_BYTES as ACCOUNT_OPERATION_BYTES, RULE_BYTES,
        TrustedBuiltinIdentityV2, TrustedEnvironmentV2, TrustedIdentityEnvironmentV2,
        encode::{
            AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
            AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
            AccountRuleWithPrestateInputV2, FixedDataPredicateInputV2, IdentityCoordinateV2,
            RegisterGeometryV2, ScalarCoordinateV2,
            encode_account_profile_with_fixed_data_predicates_v2_atomic,
        },
    },
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_GENERATION_OFFSET, CAPABILITY_ROOT_HEADER_BYTES_V1,
    CAPABILITY_ROOT_MARKET_OFFSET, CAPABILITY_ROOT_RELEASE_SET_OFFSET,
    hot_v3::HOT_FAMILY_REQUEST_OFFSET_V3,
};
use dclutch_claims_svm::{
    CallerRole as ClaimsCallerRole,
    frame_spec_v1::{ClaimsFrameDataV1, ClaimsFrameRoleV1, SparseNativeTransferFrameSpecV1},
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        LiabilityBasisMarketLayoutV2, LiabilityBasisPositionLayoutV2,
    },
    sparse_native_transfer_v1::{
        SPARSE_NATIVE_TRANSFER_BYTES_V1, SPARSE_NATIVE_TRANSFER_RECEIPT_BYTES_V1,
        SparseNativeTransferInputV1, SparseNativeTransferLayoutV1, SparseNativeTransferV1,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CLOSE_REPLAY_ACCOUNT_COUNT_V1, CLOSE_VAULT_ACCOUNT_COUNT_V1, CUSTODY_RECEIPT_BYTES_V1,
    CUSTODY_REQUEST_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1, CustodyFrameDataV1,
    CustodyFrameRoleV1, CustodyFrameSpecV1, CustodyReplayLayoutV1, CustodyRequestLayoutV1,
    CustodyRequestV1, OperationV1, TRANSFER_ACCOUNT_COUNT_V1,
};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{
        HEADER_BYTES as EFFECT_HEADER_BYTES, OPERATION_BYTES as EFFECT_OPERATION_BYTES,
        ProgramV3 as EffectProgramV3, RECEIPT_DEPENDENCY_BYTES, ROUTE_BYTES, RouteKindV3,
        RouteReceiptDependencyV3,
        encode::{
            AccountCoordinateV3, EffectGeometryV3, EffectInstructionV3, IdentityCoordinateV3,
            RequestSpaceV3, RouteInputV3, ScalarCoordinateV3, encode_effect_program_v4_atomic,
        },
    },
    v4::{BorrowedRangePolicyV4, HEADER_BYTES_V4, ProgramV4, encode_program_v4_atomic},
};
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_market_core_codec::STATE_BYTES as CORE_STATE_BYTES;
use dclutch_product_payoff_v2_codec::runtime_v3::BASIS_WIDTH_OFFSET_V3;
use dclutch_product_runtime_v2::{
    PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_HEADER_BYTES, PORTFOLIO_LIABILITY_BASIS_ID_OFFSET,
};
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_realm_contract::{REALM_BYTES, RealmLayoutV1};
use dclutch_registry_contract::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
use dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES;
use dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;
use dclutch_request_profile_contract::{
    HEADER_BYTES as REQUEST_PROFILE_HEADER_BYTES, OPERATION_BYTES as REQUEST_OPERATION_BYTES,
    RequestProfileV1,
    encode::{
        IdentityRegisterV1, RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1,
        ScalarRegisterV1, encode_request_profile_v1_atomic,
    },
    v2::{
        NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1, REQUEST_PROFILE_V2_HEADER_BYTES,
        NativeSignatureRequirementV1, RequestProfileV2, encode_request_profile_v2_atomic,
    },
};
use dclutch_sha256_adapter::digest;
use dclutch_transition_vm::v3::{
    HEADER_BYTES as TRANSITION_HEADER_BYTES, INSTRUCTION_BYTES as TRANSITION_INSTRUCTION_BYTES,
    IdentityRegisterV3, InstructionV3, ProgramGeometryV3, ProgramV3 as TransitionProgramV3,
    ScalarRegisterV3, encode_program_atomic,
};

use crate::{
    execution_v3::{
        DIRECT_EMPTY_ACTION_REQUEST_BYTES_V3, DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3,
        DIRECT_EXECUTION_REQUEST_MAGIC_V3, DIRECT_EXECUTION_REQUEST_VERSION_V3,
        DIRECT_REGISTERED_CANCEL_REQUEST_BYTES_V3, DirectExecutionActionV3,
        native_signature_slice_v3,
    },
    generated_intent_v2 as intent,
    successor::{
        DIRECT_EXECUTION_CONFIG_BYTES_V1, DIRECT_FEE_DENOMINATOR_V1,
        DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1,
        DIRECT_REGISTERED_RECORD_BYTES_V2,
        DIRECT_REGISTERED_RECORD_PDA_DOMAIN_V2, DIRECT_ROOT_STATE_BYTES_V1,
        DirectExecutionConfigLayoutV1, DirectMakerReplayLayoutV1, DirectRegisteredRecordLayoutV2,
        DirectRootStateLayoutV1,
    },
};

/// First logical account of the sparse Claims refund frame.
pub const DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4: u16 = 10;
/// First logical account of the Custody residual-refund frame.
pub const DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4: u16 = 32;
/// First logical account of the Custody vault-close frame.
pub const DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4: u16 = 46;
/// First logical account of the Custody replay-close frame.
pub const DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_REPLAY_ACCOUNT_START_V4: u16 = 60;
/// Executable Custody program used by the three Custody routes.
pub const DIRECT_REGISTERED_TERMINAL_CUSTODY_PROGRAM_ACCOUNT_V4: u16 = 70;
/// Exact fixed logical account count for both terminal actions.
pub const DIRECT_REGISTERED_TERMINAL_FIXED_ACCOUNTS_V4: u16 = 71;

const ROOT_ACCOUNT: u16 = 0;
const CONFIG_ACCOUNT: u16 = 1;
const PRODUCT_ACCOUNT: u16 = 2;
const PORTFOLIO_ACCOUNT: u16 = 3;
const BASIS_ACCOUNT: u16 = 4;
const MAKER_ACCOUNT: u16 = 5;
const RECORD_ACCOUNT: u16 = 6;
const RENT_CREDIT_ACCOUNT: u16 = 7;
const RENT_PROGRAM_ACCOUNT: u16 = 8;
const CLAIMS_MARKET_ACCOUNT: u16 = DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4 + 1;
const CLAIMS_SOURCE_POSITION_ACCOUNT: u16 =
    DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4 + 20;
const CLAIMS_DESTINATION_POSITION_ACCOUNT: u16 =
    DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4 + 21;
const CUSTODY_REALM_ACCOUNT: u16 =
    DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 6;
const CUSTODY_REPLAY_ACCOUNT: u16 =
    DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 8;
const CUSTODY_MINT_ACCOUNT: u16 =
    DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 9;
const CUSTODY_VAULT_ACCOUNT: u16 =
    DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 10;
const CUSTODY_DESTINATION_ACCOUNT: u16 =
    DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 11;
const CUSTODY_AUTHORITY_ACCOUNT: u16 =
    DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 12;
const CUSTODY_TOKEN_PROGRAM_ACCOUNT: u16 =
    DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 13;

/// Common scalar bank width shared by terminal Profile14, RequestProfile,
/// TransitionVMV3, and EffectV4.
pub const DIRECT_REGISTERED_TERMINAL_COMMON_SCALARS_V4: usize = 64;
/// Common identity bank width shared by terminal artifacts.
pub const DIRECT_REGISTERED_TERMINAL_COMMON_IDENTITIES_V4: usize = 40;
/// Terminal artifacts have no per-Product-item scalar body.
pub const DIRECT_REGISTERED_TERMINAL_ITEM_SCALAR_STRIDE_V4: u16 = 0;
/// Terminal artifacts have no per-Product-item identity body.
pub const DIRECT_REGISTERED_TERMINAL_ITEM_IDENTITY_STRIDE_V4: u16 = 0;

/// Parent request digest seeded by generic Hot.
pub const TERMINAL_IDENTITY_PARENT_REQUEST_V4: usize = 0;
/// Registry-authenticated current Trading program.
pub const TERMINAL_IDENTITY_TRADING_PROGRAM_V4: usize = 1;
/// Selected Core Market identity.
pub const TERMINAL_IDENTITY_MARKET_V4: usize = 2;
/// Selected execution release set.
pub const TERMINAL_IDENTITY_RELEASE_SET_V4: usize = 3;
/// Exact registered-record PDA.
pub const TERMINAL_IDENTITY_RECORD_STATE_V4: usize = 4;
/// Exact maker replay PDA.
pub const TERMINAL_IDENTITY_MAKER_STATE_V4: usize = 5;
/// Persisted maker identity.
pub const TERMINAL_IDENTITY_RECORD_MAKER_V4: usize = 6;
/// Persisted signed intent Market.
pub const TERMINAL_IDENTITY_RECORD_MARKET_V4: usize = 7;
/// Persisted signed collateral/refund account.
pub const TERMINAL_IDENTITY_RECORD_COLLATERAL_V4: usize = 8;
/// Persisted record RentCredit beneficiary.
pub const TERMINAL_IDENTITY_RECORD_RENT_OWNER_V4: usize = 9;
/// Maker replay Market.
pub const TERMINAL_IDENTITY_MAKER_MARKET_V4: usize = 10;
/// Maker replay maker.
pub const TERMINAL_IDENTITY_MAKER_OWNER_V4: usize = 11;
/// One physical lifecycle RentCredit account.
pub const TERMINAL_IDENTITY_RENT_CREDIT_V4: usize = 12;
/// Executable Rent program owning the credit.
pub const TERMINAL_IDENTITY_RENT_PROGRAM_V4: usize = 13;
/// Authenticated Product record digest.
pub const TERMINAL_IDENTITY_PRODUCT_RECORD_V4: usize = 14;
/// Product-owned LiabilityBasis semantic identity.
pub const TERMINAL_IDENTITY_SEMANTIC_BASIS_V4: usize = 15;
/// Finalized linked-basis record digest.
pub const TERMINAL_IDENTITY_LINKED_BASIS_V4: usize = 16;
/// Native participant-zero signer for cancellation.
pub const TERMINAL_IDENTITY_NATIVE_SIGNER_V4: usize = 17;
/// Maker carried by the cancellation request.
pub const TERMINAL_IDENTITY_REQUEST_MAKER_V4: usize = 18;
/// Market carried by the cancellation request.
pub const TERMINAL_IDENTITY_REQUEST_MARKET_V4: usize = 19;
/// Collateral account carried by the cancellation request.
pub const TERMINAL_IDENTITY_REQUEST_COLLATERAL_V4: usize = 20;
/// Custody replay's immutable Realm identity.
pub const TERMINAL_IDENTITY_REALM_V4: usize = 21;
/// Realm-selected collateral mint.
pub const TERMINAL_IDENTITY_MINT_V4: usize = 22;
/// Realm-selected token program.
pub const TERMINAL_IDENTITY_TOKEN_PROGRAM_V4: usize = 23;
/// Custody replay PDA.
pub const TERMINAL_IDENTITY_CUSTODY_REPLAY_V4: usize = 24;
/// Record-keyed Custody vault.
pub const TERMINAL_IDENTITY_CUSTODY_VAULT_V4: usize = 25;
/// Custody transfer authority.
pub const TERMINAL_IDENTITY_CUSTODY_AUTHORITY_V4: usize = 26;
/// External Buy refund token account.
pub const TERMINAL_IDENTITY_CUSTODY_DESTINATION_V4: usize = 27;
/// Replay-bound context identity.
pub const TERMINAL_IDENTITY_CUSTODY_CONTEXT_V4: usize = 28;
/// Replay-bound release set.
pub const TERMINAL_IDENTITY_CUSTODY_RELEASE_SET_V4: usize = 29;
/// Replay-bound Market.
pub const TERMINAL_IDENTITY_CUSTODY_MARKET_V4: usize = 30;
/// Replay-bound caller program.
pub const TERMINAL_IDENTITY_CUSTODY_CALLER_V4: usize = 31;
/// Replay-bound rent refund.
pub const TERMINAL_IDENTITY_CUSTODY_RENT_REFUND_V4: usize = 32;

/// Trusted current slot.
pub const TERMINAL_SCALAR_SLOT_V4: usize = 0;
/// Root open-maker count.
pub const TERMINAL_SCALAR_ROOT_OPEN_COUNT_V4: usize = 1;
/// Root Market generation.
pub const TERMINAL_SCALAR_MARKET_GENERATION_V4: usize = 2;
/// Immutable price scale.
pub const TERMINAL_SCALAR_PRICE_SCALE_V4: usize = 3;
/// Immutable fee rate.
pub const TERMINAL_SCALAR_POLICY_FEE_BPS_V4: usize = 4;
/// Product-authenticated outcome count.
pub const TERMINAL_SCALAR_OUTCOME_COUNT_V4: usize = 5;
/// Maker replay generation.
pub const TERMINAL_SCALAR_MAKER_GENERATION_V4: usize = 6;
/// Maker next nonce.
pub const TERMINAL_SCALAR_MAKER_NEXT_NONCE_V4: usize = 7;
/// Maker live registered-record count.
pub const TERMINAL_SCALAR_MAKER_LIVE_COUNT_V4: usize = 8;
/// Maker minimum still-live nonce.
pub const TERMINAL_SCALAR_MAKER_MINIMUM_LIVE_NONCE_V4: usize = 9;
/// Persisted record PDA bump.
pub const TERMINAL_SCALAR_RECORD_BUMP_V4: usize = 10;
/// Persisted signed side.
pub const TERMINAL_SCALAR_RECORD_SIDE_V4: usize = 11;
/// Persisted signed lifecycle.
pub const TERMINAL_SCALAR_RECORD_LIFECYCLE_V4: usize = 12;
/// Persisted signed outcome.
pub const TERMINAL_SCALAR_RECORD_OUTCOME_V4: usize = 13;
/// Persisted signed generation.
pub const TERMINAL_SCALAR_RECORD_GENERATION_V4: usize = 14;
/// Persisted signed nonce.
pub const TERMINAL_SCALAR_RECORD_NONCE_V4: usize = 15;
/// Persisted signed first valid slot.
pub const TERMINAL_SCALAR_RECORD_VALID_FROM_V4: usize = 16;
/// Persisted signed inclusive last valid slot.
pub const TERMINAL_SCALAR_RECORD_VALID_THROUGH_V4: usize = 17;
/// Persisted signed maximum fill.
pub const TERMINAL_SCALAR_RECORD_MAXIMUM_V4: usize = 18;
/// Persisted signed limit price.
pub const TERMINAL_SCALAR_RECORD_LIMIT_V4: usize = 19;
/// Persisted signed fee rate.
pub const TERMINAL_SCALAR_RECORD_FEE_BPS_V4: usize = 20;
/// Aggregate record fill.
pub const TERMINAL_SCALAR_RECORD_FILLED_V4: usize = 21;
/// Remaining Sell claim reserve.
pub const TERMINAL_SCALAR_RECORD_RESERVED_CLAIMS_V4: usize = 22;
/// Remaining Buy collateral reserve.
pub const TERMINAL_SCALAR_RECORD_RESERVED_COLLATERAL_V4: usize = 23;
/// Historical record-rent principal.
pub const TERMINAL_SCALAR_RECORD_RENT_PRINCIPAL_V4: usize = 24;
/// Current record lamports.
pub const TERMINAL_SCALAR_RECORD_LAMPORTS_V4: usize = 25;
/// Claims Market revision.
pub const TERMINAL_SCALAR_CLAIMS_MARKET_REVISION_V4: usize = 26;
/// Record Position revision.
pub const TERMINAL_SCALAR_CLAIMS_SOURCE_REVISION_V4: usize = 27;
/// Maker Position revision.
pub const TERMINAL_SCALAR_CLAIMS_DESTINATION_REVISION_V4: usize = 28;
/// Custody replay next revision.
pub const TERMINAL_SCALAR_CUSTODY_REVISION_V4: usize = 29;
/// Custody replay open-vault count.
pub const TERMINAL_SCALAR_CUSTODY_OPEN_VAULTS_V4: usize = 30;
/// Current vault lamports returned by CloseVault.
pub const TERMINAL_SCALAR_CUSTODY_VAULT_LAMPORTS_V4: usize = 31;
/// Current replay lamports returned by CloseReplay.
pub const TERMINAL_SCALAR_CUSTODY_REPLAY_LAMPORTS_V4: usize = 32;
/// Canonical zero.
pub const TERMINAL_SCALAR_ZERO_V4: usize = 33;
/// Canonical one.
pub const TERMINAL_SCALAR_ONE_V4: usize = 34;
/// Registered lifecycle tag.
pub const TERMINAL_SCALAR_GTC_V4: usize = 35;
/// Basis-point denominator.
pub const TERMINAL_SCALAR_FEE_DENOMINATOR_V4: usize = 36;
/// Remaining quantity.
pub const TERMINAL_SCALAR_REMAINING_V4: usize = 37;
/// Sell route enable.
pub const TERMINAL_SCALAR_SELL_ENABLED_V4: usize = 38;
/// Buy route enable.
pub const TERMINAL_SCALAR_BUY_ENABLED_V4: usize = 39;
/// Expected Sell reserve.
pub const TERMINAL_SCALAR_EXPECTED_CLAIMS_V4: usize = 40;
/// Remaining Buy gross reserve.
pub const TERMINAL_SCALAR_GROSS_V4: usize = 41;
/// Remaining Buy fee reserve.
pub const TERMINAL_SCALAR_FEE_V4: usize = 42;
/// Remaining Buy total reserve.
pub const TERMINAL_SCALAR_TOTAL_COLLATERAL_V4: usize = 43;
/// Expected side-selected collateral reserve.
pub const TERMINAL_SCALAR_EXPECTED_COLLATERAL_V4: usize = 44;
/// Maker live count after closure.
pub const TERMINAL_SCALAR_MAKER_LIVE_COUNT_AFTER_V4: usize = 45;
/// Custody revision after residual refund.
pub const TERMINAL_SCALAR_CUSTODY_REVISION_AFTER_REFUND_V4: usize = 46;
/// Custody revision after Vault close.
pub const TERMINAL_SCALAR_CUSTODY_REVISION_AFTER_VAULT_V4: usize = 47;
/// Custody revision after replay close.
pub const TERMINAL_SCALAR_CUSTODY_REVISION_AFTER_REPLAY_V4: usize = 48;
/// Cancellation-request side.
pub const TERMINAL_SCALAR_REQUEST_SIDE_V4: usize = 49;
/// Cancellation-request lifecycle.
pub const TERMINAL_SCALAR_REQUEST_LIFECYCLE_V4: usize = 50;
/// Cancellation-request outcome.
pub const TERMINAL_SCALAR_REQUEST_OUTCOME_V4: usize = 51;
/// Cancellation-request generation.
pub const TERMINAL_SCALAR_REQUEST_GENERATION_V4: usize = 52;
/// Cancellation-request nonce.
pub const TERMINAL_SCALAR_REQUEST_NONCE_V4: usize = 53;
/// Cancellation-request first valid slot.
pub const TERMINAL_SCALAR_REQUEST_VALID_FROM_V4: usize = 54;
/// Cancellation-request last valid slot.
pub const TERMINAL_SCALAR_REQUEST_VALID_THROUGH_V4: usize = 55;
/// Cancellation-request maximum fill.
pub const TERMINAL_SCALAR_REQUEST_MAXIMUM_V4: usize = 56;
/// Cancellation-request limit price.
pub const TERMINAL_SCALAR_REQUEST_LIMIT_V4: usize = 57;
/// Cancellation-request fee rate.
pub const TERMINAL_SCALAR_REQUEST_FEE_BPS_V4: usize = 58;
/// Aggregate gross collateral already executed by the live record.
pub const TERMINAL_SCALAR_RECORD_CUMULATIVE_GROSS_V4: usize = 59;
/// Aggregate cumulative-difference fee already executed by the live record.
pub const TERMINAL_SCALAR_RECORD_CUMULATIVE_FEE_V4: usize = 60;
/// Initial maximum Buy gross reserve.
pub const TERMINAL_SCALAR_INITIAL_GROSS_V4: usize = 61;
/// Initial maximum Buy fee reserve.
pub const TERMINAL_SCALAR_INITIAL_FEE_V4: usize = 62;
/// Initial maximum Buy gross-plus-fee reserve.
pub const TERMINAL_SCALAR_INITIAL_TOTAL_V4: usize = 63;

const FIXED_DATA_PREDICATES: usize = 9;
const FIXED_OPERATIONS: usize = 64;
const CANCEL_REQUEST_OPERATIONS: usize = 27;
const EXPIRE_REQUEST_OPERATIONS: usize = 6;
const COMMON_TRANSITION_INSTRUCTIONS: usize = 44;
const CANCEL_TRANSITION_INSTRUCTIONS: usize = COMMON_TRANSITION_INSTRUCTIONS + 14;
const EXPIRE_TRANSITION_INSTRUCTIONS: usize = COMMON_TRANSITION_INSTRUCTIONS + 1;

/// Exact terminal Profile14 width.
pub const DIRECT_REGISTERED_TERMINAL_ACCOUNT_PROFILE_BYTES_V4: usize =
    FIXED_DATA_PREDICATE_HEADER_BYTES
        + FIXED_DATA_PREDICATES * FIXED_DATA_PREDICATE_BYTES
        + DIRECT_REGISTERED_TERMINAL_FIXED_ACCOUNTS_V4 as usize * RULE_BYTES
        + FIXED_OPERATIONS * ACCOUNT_OPERATION_BYTES;
/// Exact signed cancellation RequestProfileV2 width.
pub const DIRECT_REGISTERED_CANCEL_REQUEST_PROFILE_BYTES_V4: usize =
    REQUEST_PROFILE_V2_HEADER_BYTES
        + REQUEST_PROFILE_HEADER_BYTES
        + CANCEL_REQUEST_OPERATIONS * REQUEST_OPERATION_BYTES
        + NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1;
/// Exact unsigned expiry RequestProfileV1 width.
pub const DIRECT_REGISTERED_EXPIRE_REQUEST_PROFILE_BYTES_V4: usize =
    REQUEST_PROFILE_HEADER_BYTES + EXPIRE_REQUEST_OPERATIONS * REQUEST_OPERATION_BYTES;
/// Exact cancellation TransitionVMV3 width.
pub const DIRECT_REGISTERED_CANCEL_TRANSITION_BYTES_V4: usize = TRANSITION_HEADER_BYTES
    + CANCEL_TRANSITION_INSTRUCTIONS * TRANSITION_INSTRUCTION_BYTES;
/// Exact expiry TransitionVMV3 width.
pub const DIRECT_REGISTERED_EXPIRE_TRANSITION_BYTES_V4: usize = TRANSITION_HEADER_BYTES
    + EXPIRE_TRANSITION_INSTRUCTIONS * TRANSITION_INSTRUCTION_BYTES;
/// Exact interpreted terminal strategy width.
pub const DIRECT_REGISTERED_TERMINAL_STRATEGY_BYTES_V4: usize =
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2;
/// Exact terminal LifecycleV5 width for maker authentication plus record close.
pub const DIRECT_REGISTERED_TERMINAL_LIFECYCLE_BYTES_V5: usize = LIFECYCLE_HEADER_BYTES
    + 2 * RECIPE_BYTES
    + 11 * SEED_BYTES
    + 2 * ACTION_PLAN_BYTES
    + 2 * PROTECTED_OUTPUT_BYTES
    + 4 * IMMUTABLE_IDENTITY_BINDING_BYTES;

const ROUTE_COUNT: usize = 4;
const DEPENDENCY_COUNT: usize = 3;
const EFFECT_INSTRUCTIONS: usize = 67;
const REQUEST_BANK_BYTES: usize =
    SPARSE_NATIVE_TRANSFER_BYTES_V1 + 3 * CUSTODY_REQUEST_BYTES_V1;
const EFFECT_BASE_BYTES: usize = EFFECT_HEADER_BYTES
    + ROUTE_COUNT * ROUTE_BYTES
    + DEPENDENCY_COUNT * RECEIPT_DEPENDENCY_BYTES
    + EFFECT_INSTRUCTIONS * EFFECT_OPERATION_BYTES
    + REQUEST_BANK_BYTES;
/// Exact terminal EffectV4 width.
pub const DIRECT_REGISTERED_TERMINAL_EFFECT_BYTES_V4: usize =
    HEADER_BYTES_V4 + EFFECT_BASE_BYTES;

/// Exact chain-observed logical widths in terminal Profile14 coordinate order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRegisteredTerminalAccountProfileInputV4<'a> {
    /// One width for every logical coordinate, including route aliases.
    pub logical_data_lengths: &'a [u32],
}

/// Stable terminal artifact refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRegisteredTerminalArtifactErrorV4 {
    /// Action, coordinate, width, or fixed geometry was invalid.
    Coordinate,
    /// A Claims or Custody semantic-owner frame refused.
    Frame,
    /// Profile14 construction or hostile decoding refused.
    AccountProfile,
    /// LifecycleV5 construction or hostile decoding refused.
    Lifecycle,
    /// RequestProfile construction or hostile decoding refused.
    RequestProfile,
    /// Transition construction or hostile decoding refused.
    Transition,
    /// Interpreted strategy construction refused.
    Strategy,
    /// EffectV4 or a canonical child request refused.
    Effect,
}

/// Emit the shared fixed-topology terminal Profile14 atomically.
pub fn encode_direct_registered_terminal_account_profile_v4_atomic(
    input: DirectRegisteredTerminalAccountProfileInputV4<'_>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    if scratch.len() != DIRECT_REGISTERED_TERMINAL_ACCOUNT_PROFILE_BYTES_V4
        || output.len() != DIRECT_REGISTERED_TERMINAL_ACCOUNT_PROFILE_BYTES_V4
    {
        return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate);
    }
    validate_logical_lengths(input.logical_data_lengths)?;
    let rules = account_rules(input.logical_data_lengths)?;
    let predicates = fixed_data_predicates()?;
    let operations = account_operations()?;
    encode_account_profile_with_fixed_data_predicates_v2_atomic(
        TrustedEnvironmentV2::CurrentSlot {
            destination: scalar_u16(TERMINAL_SCALAR_SLOT_V4)?,
        },
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: identity_u16(TERMINAL_IDENTITY_TRADING_PROGRAM_V4)?,
        },
        TrustedBuiltinIdentityV2::None,
        &[],
        &predicates,
        &rules,
        &[],
        &operations,
        register_geometry()?,
        scratch,
        output,
    )
    .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::AccountProfile)?;
    AccountProfileV2::decode(output)
        .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::AccountProfile)?;
    Ok(())
}

/// Emit maker authentication followed by the action-selected record close.
pub fn encode_direct_registered_terminal_lifecycle_v5_atomic(
    action: DirectExecutionActionV3,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    require_terminal_action(action)?;
    if scratch.len() != DIRECT_REGISTERED_TERMINAL_LIFECYCLE_BYTES_V5
        || output.len() != DIRECT_REGISTERED_TERMINAL_LIFECYCLE_BYTES_V5
    {
        return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate);
    }
    let recipes = [
        LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(MAKER_ACCOUNT),
            seed_start: 0,
            seed_count: 5,
            bump_offset: 4,
            data_base: u32::try_from(DIRECT_MAKER_REPLAY_BYTES_V1)
                .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Coordinate)?,
            data_stride: 0,
        },
        LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(RECORD_ACCOUNT),
            seed_start: 5,
            seed_count: 6,
            bump_offset: 5,
            data_base: u32::try_from(DIRECT_REGISTERED_RECORD_BYTES_V2)
                .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Coordinate)?,
            data_stride: 0,
        },
    ];
    let seeds = [
        LifecycleSeedInputV3::Literal(DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(TERMINAL_IDENTITY_MARKET_V4)?),
        LifecycleSeedInputV3::CommonScalar {
            index: scalar_u16(TERMINAL_SCALAR_RECORD_GENERATION_V4)?,
            width: 8,
        },
        LifecycleSeedInputV3::CommonIdentity(identity_u16(
            TERMINAL_IDENTITY_RECORD_MAKER_V4,
        )?),
        LifecycleSeedInputV3::CanonicalBump,
        LifecycleSeedInputV3::Literal(DIRECT_REGISTERED_RECORD_PDA_DOMAIN_V2),
        LifecycleSeedInputV3::CommonIdentity(identity_u16(TERMINAL_IDENTITY_MARKET_V4)?),
        LifecycleSeedInputV3::CommonScalar {
            index: scalar_u16(TERMINAL_SCALAR_RECORD_GENERATION_V4)?,
            width: 8,
        },
        LifecycleSeedInputV3::CommonIdentity(identity_u16(
            TERMINAL_IDENTITY_RECORD_MAKER_V4,
        )?),
        LifecycleSeedInputV3::CommonScalar {
            index: scalar_u16(TERMINAL_SCALAR_RECORD_NONCE_V4)?,
            width: 8,
        },
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = [
        LifecyclePlanInputV3 {
            action: action as u32,
            operation: LifecycleOperationInputV3::Authenticate,
            recipe: 0,
            payer: None,
            rent_credit: None,
            principal: None,
            beneficiary: None,
            guard: LifecycleGuardInputV3::Always,
        },
        LifecyclePlanInputV3 {
            action: action as u32,
            operation: LifecycleOperationInputV3::Close,
            recipe: 1,
            payer: None,
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(RENT_CREDIT_ACCOUNT)),
            principal: Some(LifecycleRegisterCoordinateV3::common(scalar_u16(
                TERMINAL_SCALAR_RECORD_RENT_PRINCIPAL_V4,
            )?)),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(identity_u16(
                TERMINAL_IDENTITY_RECORD_RENT_OWNER_V4,
            )?)),
            guard: LifecycleGuardInputV3::Always,
        },
    ];
    let bindings = [
        LifecycleImmutableIdentityBindingInputV4 {
            plan: 0,
            data_offset: u32::try_from(DirectMakerReplayLayoutV1::MARKET)
                .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Coordinate)?,
            canonical: LifecycleRegisterCoordinateV3::common(identity_u16(
                TERMINAL_IDENTITY_MARKET_V4,
            )?),
        },
        LifecycleImmutableIdentityBindingInputV4 {
            plan: 0,
            data_offset: u32::try_from(DirectMakerReplayLayoutV1::MAKER)
                .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Coordinate)?,
            canonical: LifecycleRegisterCoordinateV3::common(identity_u16(
                TERMINAL_IDENTITY_RECORD_MAKER_V4,
            )?),
        },
        LifecycleImmutableIdentityBindingInputV4 {
            plan: 1,
            data_offset: u32::try_from(DirectRegisteredRecordLayoutV2::MAKER)
                .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Coordinate)?,
            canonical: LifecycleRegisterCoordinateV3::common(identity_u16(
                TERMINAL_IDENTITY_RECORD_MAKER_V4,
            )?),
        },
        LifecycleImmutableIdentityBindingInputV4 {
            plan: 1,
            data_offset: u32::try_from(
                DirectRegisteredRecordLayoutV2::INTENT + intent::COMPACT_INTENT_MARKET_OFFSET_V2,
            )
            .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Coordinate)?,
            canonical: LifecycleRegisterCoordinateV3::common(identity_u16(
                TERMINAL_IDENTITY_MARKET_V4,
            )?),
        },
    ];
    encode_lifecycle_policy_v5_atomic(
        &recipes,
        &seeds,
        &plans,
        &[None, None],
        &bindings,
        &[],
        scratch,
        output,
    )
    .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Lifecycle)?;
    let id = digest(output);
    StateLifecyclePolicyV5::decode_selected(id, id, output)
        .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Lifecycle)?;
    Ok(())
}

/// Emit maker-signed participant-zero cancellation RequestProfileV2 bytes.
pub fn encode_direct_registered_cancel_request_profile_v4_atomic(
    v1_scratch: &mut [u8],
    v1_candidate: &mut [u8],
    v2_scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    let v1_width = REQUEST_PROFILE_HEADER_BYTES + CANCEL_REQUEST_OPERATIONS * REQUEST_OPERATION_BYTES;
    if v1_scratch.len() != v1_width
        || v1_candidate.len() != v1_width
        || v2_scratch.len() != DIRECT_REGISTERED_CANCEL_REQUEST_PROFILE_BYTES_V4
        || output.len() != DIRECT_REGISTERED_CANCEL_REQUEST_PROFILE_BYTES_V4
    {
        return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate);
    }
    let operations = cancel_request_operations()?;
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            width32(DIRECT_REGISTERED_CANCEL_REQUEST_BYTES_V3)?,
            0,
            scalar_u16(DIRECT_REGISTERED_TERMINAL_COMMON_SCALARS_V4)?,
            0,
            identity_u16(DIRECT_REGISTERED_TERMINAL_COMMON_IDENTITIES_V4)?,
            0,
        ),
        &operations,
        &[],
        v1_scratch,
        v1_candidate,
    )
    .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::RequestProfile)?;
    let slice = native_signature_slice_v3(DirectExecutionActionV3::CancelRegistered, 0, 0)
        .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Coordinate)?;
    let requirements = [NativeSignatureRequirementV1::new(
        absolute_message_offset(slice.message_offset)?,
        slice.message_bytes,
        u32::try_from(TERMINAL_IDENTITY_NATIVE_SIGNER_V4)
            .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Coordinate)?,
    )];
    encode_request_profile_v2_atomic(v1_candidate, &requirements, v2_scratch, output)
        .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::RequestProfile)?;
    let id = digest(output);
    RequestProfileV2::decode_selected(id, id, output)
        .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::RequestProfile)?;
    Ok(())
}

/// Emit the unsigned empty-body expiry RequestProfileV1 bytes atomically.
pub fn encode_direct_registered_expire_request_profile_v4_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    if scratch.len() != DIRECT_REGISTERED_EXPIRE_REQUEST_PROFILE_BYTES_V4
        || output.len() != DIRECT_REGISTERED_EXPIRE_REQUEST_PROFILE_BYTES_V4
    {
        return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate);
    }
    let operations = empty_request_operations(DirectExecutionActionV3::ExpireRegistered)?;
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            width32(DIRECT_EMPTY_ACTION_REQUEST_BYTES_V3)?,
            0,
            scalar_u16(DIRECT_REGISTERED_TERMINAL_COMMON_SCALARS_V4)?,
            0,
            identity_u16(DIRECT_REGISTERED_TERMINAL_COMMON_IDENTITIES_V4)?,
            0,
        ),
        &operations,
        &[],
        scratch,
        output,
    )
    .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::RequestProfile)?;
    RequestProfileV1::decode(output)
        .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::RequestProfile)?;
    Ok(())
}

/// Emit the action-selected terminal TransitionVMV3 bytes atomically.
pub fn encode_direct_registered_terminal_transition_v4_atomic(
    action: DirectExecutionActionV3,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    require_terminal_action(action)?;
    let expected = match action {
        DirectExecutionActionV3::CancelRegistered => DIRECT_REGISTERED_CANCEL_TRANSITION_BYTES_V4,
        DirectExecutionActionV3::ExpireRegistered => DIRECT_REGISTERED_EXPIRE_TRANSITION_BYTES_V4,
        _ => return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate),
    };
    if scratch.len() != expected || output.len() != expected {
        return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate);
    }
    let common = common_transition_instructions()?;
    match action {
        DirectExecutionActionV3::CancelRegistered => {
            let terminal = cancel_transition_instructions()?;
            encode_program_atomic(register_program_geometry()?, &common, &[], &terminal, scratch, output)
        }
        DirectExecutionActionV3::ExpireRegistered => {
            let terminal = [InstructionV3::scalar_lt(
                scalar_register(TERMINAL_SCALAR_RECORD_VALID_THROUGH_V4)?,
                scalar_register(TERMINAL_SCALAR_SLOT_V4)?,
            )];
            encode_program_atomic(register_program_geometry()?, &common, &[], &terminal, scratch, output)
        }
        _ => Err(dclutch_transition_vm::v3::Error::InvalidLength),
    }
    .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Transition)?;
    TransitionProgramV3::decode(output)
        .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Transition)?;
    Ok(())
}

/// Construct the interpreted strategy selecting one terminal transition.
pub fn direct_registered_terminal_strategy_v4(
    transition_id: [u8; 32],
) -> Result<[u8; DIRECT_REGISTERED_TERMINAL_STRATEGY_BYTES_V4], DirectRegisteredTerminalArtifactErrorV4>
{
    let content = |bytes| {
        ContentId::new(bytes).map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Strategy)
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
    .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Strategy)
}

fn validate_logical_lengths(
    lengths: &[u32],
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    if lengths.len() != usize::from(DIRECT_REGISTERED_TERMINAL_FIXED_ACCOUNTS_V4)
        || length(lengths, ROOT_ACCOUNT)?
            != width32(CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1)?
        || length(lengths, CONFIG_ACCOUNT)? != width32(DIRECT_EXECUTION_CONFIG_BYTES_V1)?
        || length(lengths, PRODUCT_ACCOUNT)? != width32(PRODUCT_RECORD_BYTES_V2)?
        || length(lengths, MAKER_ACCOUNT)? != width32(DIRECT_MAKER_REPLAY_BYTES_V1)?
        || length(lengths, RECORD_ACCOUNT)? != width32(DIRECT_REGISTERED_RECORD_BYTES_V2)?
        || length(lengths, RENT_CREDIT_ACCOUNT)? != width32(LIFECYCLE_RENT_CREDIT_BYTES_V2)?
        || length(lengths, CLAIMS_MARKET_ACCOUNT)?
            < width32(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2)?
        || length(lengths, CLAIMS_SOURCE_POSITION_ACCOUNT)?
            < width32(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2)?
        || length(lengths, CLAIMS_DESTINATION_POSITION_ACCOUNT)?
            < width32(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2)?
        || length(lengths, CUSTODY_REALM_ACCOUNT)? != width32(REALM_BYTES)?
        || length(lengths, CUSTODY_REPLAY_ACCOUNT)? != width32(CustodyReplayLayoutV1::BYTES)?
    {
        return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate);
    }
    for (account, representative) in ROUTE_ALIASES {
        if length(lengths, *account)? != length(lengths, *representative)? {
            return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate);
        }
    }
    Ok(())
}

fn account_rules(
    lengths: &[u32],
) -> Result<
    [AccountRuleWithPrestateInputV2; DIRECT_REGISTERED_TERMINAL_FIXED_ACCOUNTS_V4 as usize],
    DirectRegisteredTerminalArtifactErrorV4,
> {
    let readonly = AccountPrivilegesV2::new(false, false, false);
    let writable = AccountPrivilegesV2::new(false, true, false);
    let executable = AccountPrivilegesV2::new(false, false, true);
    let none = AccountEffectPermissionsV2::new(false, false, false);
    let mut rules = [exact(readonly, none, 0, 0);
        DIRECT_REGISTERED_TERMINAL_FIXED_ACCOUNTS_V4 as usize];
    for (index, observed) in lengths.iter().copied().enumerate() {
        *rules
            .get_mut(index)
            .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)? =
            exact(readonly, none, observed, 0);
    }
    *rule_mut(&mut rules, MAKER_ACCOUNT)? = exact(
        writable,
        AccountEffectPermissionsV2::new(false, false, true),
        width32(DIRECT_MAKER_REPLAY_BYTES_V1)?,
        0,
    );
    *rule_mut(&mut rules, RECORD_ACCOUNT)? = exact(
        writable,
        AccountEffectPermissionsV2::new(true, false, false),
        width32(DIRECT_REGISTERED_RECORD_BYTES_V2)?,
        0,
    );
    *rule_mut(&mut rules, RENT_CREDIT_ACCOUNT)? = exact(
        writable,
        AccountEffectPermissionsV2::new(false, true, false),
        width32(LIFECYCLE_RENT_CREDIT_BYTES_V2)?,
        0,
    );
    *rule_mut(&mut rules, RENT_PROGRAM_ACCOUNT)? = opaque(executable);
    *rule_mut(
        &mut rules,
        DIRECT_REGISTERED_TERMINAL_CUSTODY_PROGRAM_ACCOUNT_V4,
    )? = opaque(executable);

    let claims = SparseNativeTransferFrameSpecV1;
    let mut local = 0_u16;
    while local < claims.account_count() {
        let account = claims
            .account(local)
            .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Frame)?;
        let privileges = if account.role() == ClaimsFrameRoleV1::CallerAuthority {
            readonly
        } else {
            let value = account.privileges();
            AccountPrivilegesV2::new(value.signer(), value.writable(), value.executable())
        };
        let coordinate = DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4 + local;
        *rule_mut(&mut rules, coordinate)? = match claims
            .data(local)
            .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Frame)?
        {
            ClaimsFrameDataV1::OpaqueData | ClaimsFrameDataV1::ProgramData(_) => {
                opaque(privileges)
            }
            ClaimsFrameDataV1::ProductTail { base, item_stride } => {
                exact(privileges, none, base, item_stride)
            }
            ClaimsFrameDataV1::Exact(value) => exact(privileges, none, value, 0),
            ClaimsFrameDataV1::ProductRecord => {
                exact(privileges, none, width32(PRODUCT_RECORD_BYTES_V2)?, 0)
            }
            ClaimsFrameDataV1::CoreMarket => {
                exact(privileges, none, width32(CORE_STATE_BYTES)?, 0)
            }
            ClaimsFrameDataV1::ActivationCache => exact(
                privileges,
                none,
                width32(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)?,
                0,
            ),
            ClaimsFrameDataV1::UpgradeableProgram => {
                exact(privileges, none, width32(LOADER_V3_PROGRAM_BYTES)?, 0)
            }
            ClaimsFrameDataV1::LinkedBasisRecord
            | ClaimsFrameDataV1::ResultDomainRecord
            | ClaimsFrameDataV1::PortfolioRecord
            | ClaimsFrameDataV1::RentSysvar
            | ClaimsFrameDataV1::PositionOwnerIdentity
            | ClaimsFrameDataV1::RentCredit => {
                exact(privileges, none, length(lengths, coordinate)?, 0)
            }
        };
        local = local
            .checked_add(1)
            .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)?;
    }

    for (operation, start) in [
        (
            OperationV1::Transfer,
            DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4,
        ),
        (
            OperationV1::CloseVault,
            DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4,
        ),
        (
            OperationV1::CloseReplay,
            DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_REPLAY_ACCOUNT_START_V4,
        ),
    ] {
        let frame = CustodyFrameSpecV1::new(operation);
        let mut local = 0_u16;
        while local < frame.account_count() {
            let account = frame
                .account(local)
                .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Frame)?;
            let privileges = if account.role() == CustodyFrameRoleV1::CallerAuthority {
                readonly
            } else {
                let value = account.privileges();
                AccountPrivilegesV2::new(value.signer(), value.writable(), value.executable())
            };
            let coordinate = start
                .checked_add(local)
                .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)?;
            *rule_mut(&mut rules, coordinate)? = match frame
                .data(local)
                .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Frame)?
            {
                CustodyFrameDataV1::OpaqueData
                | CustodyFrameDataV1::CallerProgramData
                | CustodyFrameDataV1::TokenMint
                | CustodyFrameDataV1::TokenAccount
                | CustodyFrameDataV1::TokenProgram => opaque(privileges),
                CustodyFrameDataV1::Exact(value) => exact(privileges, none, value, 0),
                CustodyFrameDataV1::CoreMarket => {
                    exact(privileges, none, width32(CORE_STATE_BYTES)?, 0)
                }
                CustodyFrameDataV1::ActivationCache => exact(
                    privileges,
                    none,
                    width32(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)?,
                    0,
                ),
                CustodyFrameDataV1::UpgradeableProgram => {
                    exact(privileges, none, width32(LOADER_V3_PROGRAM_BYTES)?, 0)
                }
                CustodyFrameDataV1::RealmRecord => {
                    exact(privileges, none, width32(REALM_BYTES)?, 0)
                }
                CustodyFrameDataV1::RentSysvar => {
                    exact(privileges, none, length(lengths, coordinate)?, 0)
                }
            };
            local = local
                .checked_add(1)
                .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)?;
        }
    }

    for (account, representative) in ROUTE_ALIASES {
        *rule_mut(&mut rules, *account)? = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: readonly,
                effect_permissions: none,
                alias: AccountAliasInputV2::Fixed(*representative),
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedRouteAlias,
        };
    }
    Ok(rules)
}

const ROUTE_ALIASES: &[(u16, u16)] = &[
    // Claims frame facts already present in the terminal prefix.
    (DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4 + 2, BASIS_ACCOUNT),
    (DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4 + 4, PRODUCT_ACCOUNT),
    (DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4 + 8, PORTFOLIO_ACCOUNT),
    // Custody common frame facts shared with Claims.
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4, DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 1, DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4 + 11),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 2, DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4 + 12),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 3, DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4 + 13),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 4, DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4 + 14),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 5, DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4 + 15),
    // CloseVault repeats Transfer's entire common prefix and token resources.
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4 + 1, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 1),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4 + 2, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 2),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4 + 3, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 3),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4 + 4, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 4),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4 + 5, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 5),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4 + 6, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 6),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4 + 7, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 7),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4 + 8, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 8),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4 + 9, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 9),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4 + 10, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 10),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4 + 11, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 12),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4 + 12, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 13),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4 + 13, RENT_CREDIT_ACCOUNT),
    // CloseReplay repeats the Custody common prefix and refund credit.
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_REPLAY_ACCOUNT_START_V4, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_REPLAY_ACCOUNT_START_V4 + 1, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 1),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_REPLAY_ACCOUNT_START_V4 + 2, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 2),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_REPLAY_ACCOUNT_START_V4 + 3, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 3),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_REPLAY_ACCOUNT_START_V4 + 4, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 4),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_REPLAY_ACCOUNT_START_V4 + 5, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 5),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_REPLAY_ACCOUNT_START_V4 + 6, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 6),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_REPLAY_ACCOUNT_START_V4 + 7, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 7),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_REPLAY_ACCOUNT_START_V4 + 8, DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4 + 8),
    (DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_REPLAY_ACCOUNT_START_V4 + 9, RENT_CREDIT_ACCOUNT),
];

fn fixed_data_predicates(
) -> Result<[FixedDataPredicateInputV2; FIXED_DATA_PREDICATES], DirectRegisteredTerminalArtifactErrorV4>
{
    let root = |offset_value: usize| {
        CAPABILITY_ROOT_HEADER_BYTES_V1
            .checked_add(offset_value)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)
    };
    Ok([
        FixedDataPredicateInputV2::RequireDataU64 {
            account: ROOT_ACCOUNT,
            data_offset: root(DirectRootStateLayoutV1::MAGIC)?,
            value: DirectRootStateLayoutV1::MAGIC_WORD,
        },
        FixedDataPredicateInputV2::RequireDataU16 {
            account: ROOT_ACCOUNT,
            data_offset: root(DirectRootStateLayoutV1::VERSION)?,
            value: DirectRootStateLayoutV1::ABI_VERSION,
        },
        FixedDataPredicateInputV2::RequireZeroRange {
            account: ROOT_ACCOUNT,
            data_offset: root(DirectRootStateLayoutV1::RESERVED)?,
            length: width32(DirectRootStateLayoutV1::RESERVED_BYTES)?,
        },
        state_magic(
            MAKER_ACCOUNT,
            DirectMakerReplayLayoutV1::MAGIC,
            DirectMakerReplayLayoutV1::MAGIC_WORD,
        )?,
        state_version(
            MAKER_ACCOUNT,
            DirectMakerReplayLayoutV1::VERSION,
            DirectMakerReplayLayoutV1::ABI_VERSION,
        )?,
        state_reserved(
            MAKER_ACCOUNT,
            DirectMakerReplayLayoutV1::RESERVED,
            DirectMakerReplayLayoutV1::RESERVED_BYTES,
        )?,
        state_magic(
            RECORD_ACCOUNT,
            DirectRegisteredRecordLayoutV2::MAGIC,
            DirectRegisteredRecordLayoutV2::MAGIC_WORD,
        )?,
        state_version(
            RECORD_ACCOUNT,
            DirectRegisteredRecordLayoutV2::VERSION,
            DirectRegisteredRecordLayoutV2::ABI_VERSION,
        )?,
        state_reserved(
            RECORD_ACCOUNT,
            DirectRegisteredRecordLayoutV2::RESERVED,
            DirectRegisteredRecordLayoutV2::RESERVED_BYTES,
        )?,
    ])
}

fn state_magic(
    account: u16,
    offset_value: usize,
    value: u64,
) -> Result<FixedDataPredicateInputV2, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(FixedDataPredicateInputV2::RequireDataU64 {
        account,
        data_offset: width32(offset_value)?,
        value,
    })
}

fn state_version(
    account: u16,
    offset_value: usize,
    value: u16,
) -> Result<FixedDataPredicateInputV2, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(FixedDataPredicateInputV2::RequireDataU16 {
        account,
        data_offset: width32(offset_value)?,
        value,
    })
}

fn state_reserved(
    account: u16,
    offset_value: usize,
    bytes: usize,
) -> Result<FixedDataPredicateInputV2, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(FixedDataPredicateInputV2::RequireZeroRange {
        account,
        data_offset: width32(offset_value)?,
        length: width32(bytes)?,
    })
}

fn account_operations(
) -> Result<[AccountOperationInputV2; FIXED_OPERATIONS], DirectRegisteredTerminalArtifactErrorV4> {
    let placeholder = require_owner(ROOT_ACCOUNT, TERMINAL_IDENTITY_TRADING_PROGRAM_V4)?;
    let mut output = [placeholder; FIXED_OPERATIONS];
    let mut next = 0_usize;
    let intent_base = DirectRegisteredRecordLayoutV2::INTENT;
    for operation in [
        require_owner(ROOT_ACCOUNT, TERMINAL_IDENTITY_TRADING_PROGRAM_V4)?,
        project_identity(
            ROOT_ACCOUNT,
            CAPABILITY_ROOT_RELEASE_SET_OFFSET,
            TERMINAL_IDENTITY_RELEASE_SET_V4,
        )?,
        project_identity(
            ROOT_ACCOUNT,
            CAPABILITY_ROOT_MARKET_OFFSET,
            TERMINAL_IDENTITY_MARKET_V4,
        )?,
        project_u64(
            ROOT_ACCOUNT,
            CAPABILITY_ROOT_GENERATION_OFFSET,
            TERMINAL_SCALAR_MARKET_GENERATION_V4,
        )?,
        project_u64(
            ROOT_ACCOUNT,
            CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT,
            TERMINAL_SCALAR_ROOT_OPEN_COUNT_V4,
        )?,
        project_u64(
            CONFIG_ACCOUNT,
            DirectExecutionConfigLayoutV1::PRICE_SCALE,
            TERMINAL_SCALAR_PRICE_SCALE_V4,
        )?,
        project_u16(
            CONFIG_ACCOUNT,
            DirectExecutionConfigLayoutV1::FEE_BASIS_POINTS,
            TERMINAL_SCALAR_POLICY_FEE_BPS_V4,
        )?,
        project_key(PRODUCT_ACCOUNT, TERMINAL_IDENTITY_PRODUCT_RECORD_V4)?,
        project_identity(
            PORTFOLIO_ACCOUNT,
            PORTFOLIO_LIABILITY_BASIS_ID_OFFSET,
            TERMINAL_IDENTITY_SEMANTIC_BASIS_V4,
        )?,
        project_key(BASIS_ACCOUNT, TERMINAL_IDENTITY_LINKED_BASIS_V4)?,
        AccountOperationInputV2::ProjectTailCountU32 {
            account: AccountCoordinateV2::fixed(BASIS_ACCOUNT),
            destination: ScalarCoordinateV2::common(scalar_u16(
                TERMINAL_SCALAR_OUTCOME_COUNT_V4,
            )?),
            data_offset: width32(BASIS_WIDTH_OFFSET_V3)?,
        },
        require_owner(MAKER_ACCOUNT, TERMINAL_IDENTITY_TRADING_PROGRAM_V4)?,
        project_key(MAKER_ACCOUNT, TERMINAL_IDENTITY_MAKER_STATE_V4)?,
        project_identity(
            MAKER_ACCOUNT,
            DirectMakerReplayLayoutV1::MARKET,
            TERMINAL_IDENTITY_MAKER_MARKET_V4,
        )?,
        project_u64(
            MAKER_ACCOUNT,
            DirectMakerReplayLayoutV1::GENERATION,
            TERMINAL_SCALAR_MAKER_GENERATION_V4,
        )?,
        project_identity(
            MAKER_ACCOUNT,
            DirectMakerReplayLayoutV1::MAKER,
            TERMINAL_IDENTITY_MAKER_OWNER_V4,
        )?,
        project_u64(
            MAKER_ACCOUNT,
            DirectMakerReplayLayoutV1::NEXT_NONCE,
            TERMINAL_SCALAR_MAKER_NEXT_NONCE_V4,
        )?,
        project_u64(
            MAKER_ACCOUNT,
            DirectMakerReplayLayoutV1::LIVE_COUNT,
            TERMINAL_SCALAR_MAKER_LIVE_COUNT_V4,
        )?,
        project_u64(
            MAKER_ACCOUNT,
            DirectMakerReplayLayoutV1::MINIMUM_LIVE_NONCE,
            TERMINAL_SCALAR_MAKER_MINIMUM_LIVE_NONCE_V4,
        )?,
        require_owner(RECORD_ACCOUNT, TERMINAL_IDENTITY_TRADING_PROGRAM_V4)?,
        project_key(RECORD_ACCOUNT, TERMINAL_IDENTITY_RECORD_STATE_V4)?,
        project_u8(
            RECORD_ACCOUNT,
            DirectRegisteredRecordLayoutV2::BUMP,
            TERMINAL_SCALAR_RECORD_BUMP_V4,
        )?,
        project_identity(
            RECORD_ACCOUNT,
            DirectRegisteredRecordLayoutV2::MAKER,
            TERMINAL_IDENTITY_RECORD_MAKER_V4,
        )?,
        project_identity(
            RECORD_ACCOUNT,
            intent_base + intent::COMPACT_INTENT_MARKET_OFFSET_V2,
            TERMINAL_IDENTITY_RECORD_MARKET_V4,
        )?,
        project_u8(
            RECORD_ACCOUNT,
            intent_base + intent::COMPACT_INTENT_SIDE_OFFSET_V2,
            TERMINAL_SCALAR_RECORD_SIDE_V4,
        )?,
        project_u8(
            RECORD_ACCOUNT,
            intent_base + intent::COMPACT_INTENT_LIFECYCLE_OFFSET_V2,
            TERMINAL_SCALAR_RECORD_LIFECYCLE_V4,
        )?,
        project_u32(
            RECORD_ACCOUNT,
            intent_base + intent::COMPACT_INTENT_OUTCOME_OFFSET_V2,
            TERMINAL_SCALAR_RECORD_OUTCOME_V4,
        )?,
        project_u64(
            RECORD_ACCOUNT,
            intent_base + intent::COMPACT_INTENT_GENERATION_OFFSET_V2,
            TERMINAL_SCALAR_RECORD_GENERATION_V4,
        )?,
        project_u64(
            RECORD_ACCOUNT,
            intent_base + intent::COMPACT_INTENT_NONCE_OFFSET_V2,
            TERMINAL_SCALAR_RECORD_NONCE_V4,
        )?,
        project_u64(
            RECORD_ACCOUNT,
            intent_base + intent::COMPACT_INTENT_VALID_FROM_OFFSET_V2,
            TERMINAL_SCALAR_RECORD_VALID_FROM_V4,
        )?,
        project_u64(
            RECORD_ACCOUNT,
            intent_base + intent::COMPACT_INTENT_VALID_THROUGH_OFFSET_V2,
            TERMINAL_SCALAR_RECORD_VALID_THROUGH_V4,
        )?,
        project_u64(
            RECORD_ACCOUNT,
            intent_base + intent::COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2,
            TERMINAL_SCALAR_RECORD_MAXIMUM_V4,
        )?,
        project_u64(
            RECORD_ACCOUNT,
            intent_base + intent::COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2,
            TERMINAL_SCALAR_RECORD_LIMIT_V4,
        )?,
        project_u16(
            RECORD_ACCOUNT,
            intent_base + intent::COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2,
            TERMINAL_SCALAR_RECORD_FEE_BPS_V4,
        )?,
        project_identity(
            RECORD_ACCOUNT,
            intent_base + intent::COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2,
            TERMINAL_IDENTITY_RECORD_COLLATERAL_V4,
        )?,
        project_u64(
            RECORD_ACCOUNT,
            DirectRegisteredRecordLayoutV2::FILLED,
            TERMINAL_SCALAR_RECORD_FILLED_V4,
        )?,
        project_u64(
            RECORD_ACCOUNT,
            DirectRegisteredRecordLayoutV2::RESERVED_CLAIMS,
            TERMINAL_SCALAR_RECORD_RESERVED_CLAIMS_V4,
        )?,
        project_u64(
            RECORD_ACCOUNT,
            DirectRegisteredRecordLayoutV2::RESERVED_COLLATERAL,
            TERMINAL_SCALAR_RECORD_RESERVED_COLLATERAL_V4,
        )?,
        project_u64(
            RECORD_ACCOUNT,
            DirectRegisteredRecordLayoutV2::CUMULATIVE_GROSS,
            TERMINAL_SCALAR_RECORD_CUMULATIVE_GROSS_V4,
        )?,
        project_u64(
            RECORD_ACCOUNT,
            DirectRegisteredRecordLayoutV2::CUMULATIVE_FEE,
            TERMINAL_SCALAR_RECORD_CUMULATIVE_FEE_V4,
        )?,
        project_identity(
            RECORD_ACCOUNT,
            DirectRegisteredRecordLayoutV2::RENT_OWNER,
            TERMINAL_IDENTITY_RECORD_RENT_OWNER_V4,
        )?,
        project_u64(
            RECORD_ACCOUNT,
            DirectRegisteredRecordLayoutV2::RENT_PRINCIPAL,
            TERMINAL_SCALAR_RECORD_RENT_PRINCIPAL_V4,
        )?,
        project_lamports(RECORD_ACCOUNT, TERMINAL_SCALAR_RECORD_LAMPORTS_V4)?,
        project_key(RENT_CREDIT_ACCOUNT, TERMINAL_IDENTITY_RENT_CREDIT_V4)?,
        project_key(RENT_PROGRAM_ACCOUNT, TERMINAL_IDENTITY_RENT_PROGRAM_V4)?,
        project_u64(
            CLAIMS_MARKET_ACCOUNT,
            LiabilityBasisMarketLayoutV2::REVISION,
            TERMINAL_SCALAR_CLAIMS_MARKET_REVISION_V4,
        )?,
        project_u64(
            CLAIMS_SOURCE_POSITION_ACCOUNT,
            LiabilityBasisPositionLayoutV2::REVISION,
            TERMINAL_SCALAR_CLAIMS_SOURCE_REVISION_V4,
        )?,
        project_u64(
            CLAIMS_DESTINATION_POSITION_ACCOUNT,
            LiabilityBasisPositionLayoutV2::REVISION,
            TERMINAL_SCALAR_CLAIMS_DESTINATION_REVISION_V4,
        )?,
        project_identity(
            CUSTODY_REPLAY_ACCOUNT,
            CustodyReplayLayoutV1::REALM_OFFSET,
            TERMINAL_IDENTITY_REALM_V4,
        )?,
        project_identity(
            CUSTODY_REALM_ACCOUNT,
            RealmLayoutV1::COLLATERAL_MINT,
            TERMINAL_IDENTITY_MINT_V4,
        )?,
        project_identity(
            CUSTODY_REALM_ACCOUNT,
            RealmLayoutV1::TOKEN_PROGRAM,
            TERMINAL_IDENTITY_TOKEN_PROGRAM_V4,
        )?,
        project_key(CUSTODY_REPLAY_ACCOUNT, TERMINAL_IDENTITY_CUSTODY_REPLAY_V4)?,
        project_identity(
            CUSTODY_REPLAY_ACCOUNT,
            CustodyReplayLayoutV1::RELEASE_SET_OFFSET,
            TERMINAL_IDENTITY_CUSTODY_RELEASE_SET_V4,
        )?,
        project_identity(
            CUSTODY_REPLAY_ACCOUNT,
            CustodyReplayLayoutV1::MARKET_OFFSET,
            TERMINAL_IDENTITY_CUSTODY_MARKET_V4,
        )?,
        project_identity(
            CUSTODY_REPLAY_ACCOUNT,
            CustodyReplayLayoutV1::CONTEXT_OFFSET,
            TERMINAL_IDENTITY_CUSTODY_CONTEXT_V4,
        )?,
        project_identity(
            CUSTODY_REPLAY_ACCOUNT,
            CustodyReplayLayoutV1::CALLER_PROGRAM_OFFSET,
            TERMINAL_IDENTITY_CUSTODY_CALLER_V4,
        )?,
        project_identity(
            CUSTODY_REPLAY_ACCOUNT,
            CustodyReplayLayoutV1::RENT_REFUND_OFFSET,
            TERMINAL_IDENTITY_CUSTODY_RENT_REFUND_V4,
        )?,
        project_u32(
            CUSTODY_REPLAY_ACCOUNT,
            CustodyReplayLayoutV1::OPEN_VAULT_COUNT_OFFSET,
            TERMINAL_SCALAR_CUSTODY_OPEN_VAULTS_V4,
        )?,
        project_u64(
            CUSTODY_REPLAY_ACCOUNT,
            CustodyReplayLayoutV1::NEXT_REVISION_OFFSET,
            TERMINAL_SCALAR_CUSTODY_REVISION_V4,
        )?,
        project_lamports(
            CUSTODY_REPLAY_ACCOUNT,
            TERMINAL_SCALAR_CUSTODY_REPLAY_LAMPORTS_V4,
        )?,
        project_key(CUSTODY_VAULT_ACCOUNT, TERMINAL_IDENTITY_CUSTODY_VAULT_V4)?,
        project_lamports(
            CUSTODY_VAULT_ACCOUNT,
            TERMINAL_SCALAR_CUSTODY_VAULT_LAMPORTS_V4,
        )?,
        project_key(
            CUSTODY_DESTINATION_ACCOUNT,
            TERMINAL_IDENTITY_CUSTODY_DESTINATION_V4,
        )?,
        project_key(
            CUSTODY_AUTHORITY_ACCOUNT,
            TERMINAL_IDENTITY_CUSTODY_AUTHORITY_V4,
        )?,
    ] {
        push_account_operation(&mut output, &mut next, operation)?;
    }
    if next != output.len() {
        return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate);
    }
    Ok(output)
}

fn cancel_request_operations(
) -> Result<[RequestInstructionV1; CANCEL_REQUEST_OPERATIONS], DirectRegisteredTerminalArtifactErrorV4>
{
    let header = DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3;
    let domain = header + 32;
    let body = domain + 32;
    Ok([
        require_request_u64(0, u64::from_le_bytes(DIRECT_EXECUTION_REQUEST_MAGIC_V3))?,
        require_request_u16(8, DIRECT_EXECUTION_REQUEST_VERSION_V3)?,
        require_request_u16(10, 0)?,
        require_request_u32(12, DirectExecutionActionV3::CancelRegistered as u32)?,
        require_request_u32(
            16,
            width32(DIRECT_REGISTERED_CANCEL_REQUEST_BYTES_V3 - header)?,
        )?,
        RequestInstructionV1::require_zero(request_coordinate(20)?, 12),
        require_request_u64(domain, intent_domain_word(0)?)?,
        require_request_u64(domain + 8, intent_domain_word(8)?)?,
        require_request_u64(domain + 16, intent_domain_word(16)?)?,
        require_request_u64(domain + 24, intent_domain_word(24)?)?,
        require_request_u64(
            body + intent::COMPACT_INTENT_MAGIC_OFFSET_V2,
            u64::from_le_bytes(intent::COMPACT_INTENT_MAGIC_V2),
        )?,
        require_request_u16(
            body + intent::COMPACT_INTENT_VERSION_OFFSET_V2,
            intent::COMPACT_INTENT_VERSION_V2,
        )?,
        RequestInstructionV1::require_zero(
            request_coordinate(body + intent::COMPACT_INTENT_RESERVED_A_OFFSET_V2)?,
            4,
        ),
        RequestInstructionV1::require_zero(
            request_coordinate(body + intent::COMPACT_INTENT_RESERVED_B_OFFSET_V2)?,
            6,
        ),
        project_request_identity(header, TERMINAL_IDENTITY_REQUEST_MAKER_V4)?,
        project_request_u8(
            body + intent::COMPACT_INTENT_SIDE_OFFSET_V2,
            TERMINAL_SCALAR_REQUEST_SIDE_V4,
        )?,
        project_request_u8(
            body + intent::COMPACT_INTENT_LIFECYCLE_OFFSET_V2,
            TERMINAL_SCALAR_REQUEST_LIFECYCLE_V4,
        )?,
        project_request_u32(
            body + intent::COMPACT_INTENT_OUTCOME_OFFSET_V2,
            TERMINAL_SCALAR_REQUEST_OUTCOME_V4,
        )?,
        project_request_identity(
            body + intent::COMPACT_INTENT_MARKET_OFFSET_V2,
            TERMINAL_IDENTITY_REQUEST_MARKET_V4,
        )?,
        project_request_u64(
            body + intent::COMPACT_INTENT_GENERATION_OFFSET_V2,
            TERMINAL_SCALAR_REQUEST_GENERATION_V4,
        )?,
        project_request_u64(
            body + intent::COMPACT_INTENT_NONCE_OFFSET_V2,
            TERMINAL_SCALAR_REQUEST_NONCE_V4,
        )?,
        project_request_u64(
            body + intent::COMPACT_INTENT_VALID_FROM_OFFSET_V2,
            TERMINAL_SCALAR_REQUEST_VALID_FROM_V4,
        )?,
        project_request_u64(
            body + intent::COMPACT_INTENT_VALID_THROUGH_OFFSET_V2,
            TERMINAL_SCALAR_REQUEST_VALID_THROUGH_V4,
        )?,
        project_request_u64(
            body + intent::COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2,
            TERMINAL_SCALAR_REQUEST_MAXIMUM_V4,
        )?,
        project_request_u64(
            body + intent::COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2,
            TERMINAL_SCALAR_REQUEST_LIMIT_V4,
        )?,
        project_request_u16(
            body + intent::COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2,
            TERMINAL_SCALAR_REQUEST_FEE_BPS_V4,
        )?,
        project_request_identity(
            body + intent::COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2,
            TERMINAL_IDENTITY_REQUEST_COLLATERAL_V4,
        )?,
    ])
}

fn empty_request_operations(
    action: DirectExecutionActionV3,
) -> Result<[RequestInstructionV1; EXPIRE_REQUEST_OPERATIONS], DirectRegisteredTerminalArtifactErrorV4>
{
    if action != DirectExecutionActionV3::ExpireRegistered {
        return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate);
    }
    Ok([
        require_request_u64(0, u64::from_le_bytes(DIRECT_EXECUTION_REQUEST_MAGIC_V3))?,
        require_request_u16(8, DIRECT_EXECUTION_REQUEST_VERSION_V3)?,
        require_request_u16(10, 0)?,
        require_request_u32(12, action as u32)?,
        require_request_u32(16, 0)?,
        RequestInstructionV1::require_zero(request_coordinate(20)?, 12),
    ])
}

fn common_transition_instructions(
) -> Result<[InstructionV3; COMMON_TRANSITION_INSTRUCTIONS], DirectRegisteredTerminalArtifactErrorV4>
{
    let mut output = [InstructionV3::load_const(scalar_register(0)?, 0);
        COMMON_TRANSITION_INSTRUCTIONS];
    let mut next = 0_usize;
    for instruction in [
        InstructionV3::load_const(scalar_register(TERMINAL_SCALAR_ZERO_V4)?, 0),
        InstructionV3::load_const(scalar_register(TERMINAL_SCALAR_ONE_V4)?, 1),
        InstructionV3::load_const(scalar_register(TERMINAL_SCALAR_GTC_V4)?, 2),
        InstructionV3::load_const(
            scalar_register(TERMINAL_SCALAR_FEE_DENOMINATOR_V4)?,
            u64::from(DIRECT_FEE_DENOMINATOR_V1),
        ),
        InstructionV3::identity_eq(
            identity_register(TERMINAL_IDENTITY_MARKET_V4)?,
            identity_register(TERMINAL_IDENTITY_MAKER_MARKET_V4)?,
        ),
        InstructionV3::identity_eq(
            identity_register(TERMINAL_IDENTITY_MARKET_V4)?,
            identity_register(TERMINAL_IDENTITY_RECORD_MARKET_V4)?,
        ),
        InstructionV3::identity_eq(
            identity_register(TERMINAL_IDENTITY_RECORD_MAKER_V4)?,
            identity_register(TERMINAL_IDENTITY_MAKER_OWNER_V4)?,
        ),
        InstructionV3::identity_eq(
            identity_register(TERMINAL_IDENTITY_RECORD_RENT_OWNER_V4)?,
            identity_register(TERMINAL_IDENTITY_RENT_CREDIT_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_MARKET_GENERATION_V4)?,
            scalar_register(TERMINAL_SCALAR_MAKER_GENERATION_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_MARKET_GENERATION_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_GENERATION_V4)?,
        ),
        InstructionV3::nonzero(scalar_register(TERMINAL_SCALAR_ROOT_OPEN_COUNT_V4)?),
        InstructionV3::scalar_le(
            scalar_register(TERMINAL_SCALAR_RECORD_SIDE_V4)?,
            scalar_register(TERMINAL_SCALAR_ONE_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_RECORD_LIFECYCLE_V4)?,
            scalar_register(TERMINAL_SCALAR_GTC_V4)?,
        ),
        InstructionV3::scalar_lt(
            scalar_register(TERMINAL_SCALAR_RECORD_OUTCOME_V4)?,
            scalar_register(TERMINAL_SCALAR_OUTCOME_COUNT_V4)?,
        ),
        InstructionV3::nonzero(scalar_register(TERMINAL_SCALAR_RECORD_MAXIMUM_V4)?),
        InstructionV3::scalar_lt(
            scalar_register(TERMINAL_SCALAR_RECORD_FILLED_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_MAXIMUM_V4)?,
        ),
        InstructionV3::scalar_le(
            scalar_register(TERMINAL_SCALAR_RECORD_VALID_FROM_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_VALID_THROUGH_V4)?,
        ),
        InstructionV3::nonzero(scalar_register(TERMINAL_SCALAR_PRICE_SCALE_V4)?),
        InstructionV3::scalar_le(
            scalar_register(TERMINAL_SCALAR_RECORD_LIMIT_V4)?,
            scalar_register(TERMINAL_SCALAR_PRICE_SCALE_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_RECORD_FEE_BPS_V4)?,
            scalar_register(TERMINAL_SCALAR_POLICY_FEE_BPS_V4)?,
        ),
        InstructionV3::scalar_lt(
            scalar_register(TERMINAL_SCALAR_RECORD_NONCE_V4)?,
            scalar_register(TERMINAL_SCALAR_MAKER_NEXT_NONCE_V4)?,
        ),
        InstructionV3::scalar_le(
            scalar_register(TERMINAL_SCALAR_MAKER_MINIMUM_LIVE_NONCE_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_NONCE_V4)?,
        ),
        InstructionV3::nonzero(scalar_register(TERMINAL_SCALAR_MAKER_LIVE_COUNT_V4)?),
        InstructionV3::nonzero(scalar_register(TERMINAL_SCALAR_RECORD_RENT_PRINCIPAL_V4)?),
        InstructionV3::scalar_le(
            scalar_register(TERMINAL_SCALAR_RECORD_RENT_PRINCIPAL_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_LAMPORTS_V4)?,
        ),
        InstructionV3::sub_into(
            scalar_register(TERMINAL_SCALAR_RECORD_MAXIMUM_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_FILLED_V4)?,
            scalar_register(TERMINAL_SCALAR_REMAINING_V4)?,
        ),
        InstructionV3::nonzero(scalar_register(TERMINAL_SCALAR_REMAINING_V4)?),
        InstructionV3::sub_into(
            scalar_register(TERMINAL_SCALAR_ONE_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_SIDE_V4)?,
            scalar_register(TERMINAL_SCALAR_SELL_ENABLED_V4)?,
        ),
        InstructionV3::copy_scalar(
            scalar_register(TERMINAL_SCALAR_RECORD_SIDE_V4)?,
            scalar_register(TERMINAL_SCALAR_BUY_ENABLED_V4)?,
        ),
        InstructionV3::checked_mul_into(
            scalar_register(TERMINAL_SCALAR_REMAINING_V4)?,
            scalar_register(TERMINAL_SCALAR_SELL_ENABLED_V4)?,
            scalar_register(TERMINAL_SCALAR_EXPECTED_CLAIMS_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_RECORD_RESERVED_CLAIMS_V4)?,
            scalar_register(TERMINAL_SCALAR_EXPECTED_CLAIMS_V4)?,
        ),
        InstructionV3::scalar_le(
            scalar_register(TERMINAL_SCALAR_RECORD_CUMULATIVE_GROSS_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_FILLED_V4)?,
        ),
        InstructionV3::mul_div_floor(
            scalar_register(TERMINAL_SCALAR_RECORD_CUMULATIVE_GROSS_V4)?,
            scalar_register(TERMINAL_SCALAR_POLICY_FEE_BPS_V4)?,
            scalar_register(TERMINAL_SCALAR_FEE_DENOMINATOR_V4)?,
            scalar_register(TERMINAL_SCALAR_FEE_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_RECORD_CUMULATIVE_FEE_V4)?,
            scalar_register(TERMINAL_SCALAR_FEE_V4)?,
        ),
        InstructionV3::mul_div_floor(
            scalar_register(TERMINAL_SCALAR_RECORD_MAXIMUM_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_LIMIT_V4)?,
            scalar_register(TERMINAL_SCALAR_PRICE_SCALE_V4)?,
            scalar_register(TERMINAL_SCALAR_INITIAL_GROSS_V4)?,
        ),
        InstructionV3::mul_div_floor(
            scalar_register(TERMINAL_SCALAR_INITIAL_GROSS_V4)?,
            scalar_register(TERMINAL_SCALAR_POLICY_FEE_BPS_V4)?,
            scalar_register(TERMINAL_SCALAR_FEE_DENOMINATOR_V4)?,
            scalar_register(TERMINAL_SCALAR_INITIAL_FEE_V4)?,
        ),
        InstructionV3::checked_add_into(
            scalar_register(TERMINAL_SCALAR_INITIAL_GROSS_V4)?,
            scalar_register(TERMINAL_SCALAR_INITIAL_FEE_V4)?,
            scalar_register(TERMINAL_SCALAR_INITIAL_TOTAL_V4)?,
        ),
        InstructionV3::checked_add_into(
            scalar_register(TERMINAL_SCALAR_RECORD_CUMULATIVE_GROSS_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_CUMULATIVE_FEE_V4)?,
            scalar_register(TERMINAL_SCALAR_TOTAL_COLLATERAL_V4)?,
        ),
        InstructionV3::sub_into(
            scalar_register(TERMINAL_SCALAR_INITIAL_TOTAL_V4)?,
            scalar_register(TERMINAL_SCALAR_TOTAL_COLLATERAL_V4)?,
            scalar_register(TERMINAL_SCALAR_GROSS_V4)?,
        ),
        InstructionV3::checked_mul_into(
            scalar_register(TERMINAL_SCALAR_GROSS_V4)?,
            scalar_register(TERMINAL_SCALAR_BUY_ENABLED_V4)?,
            scalar_register(TERMINAL_SCALAR_EXPECTED_COLLATERAL_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_RECORD_RESERVED_COLLATERAL_V4)?,
            scalar_register(TERMINAL_SCALAR_EXPECTED_COLLATERAL_V4)?,
        ),
        InstructionV3::sub_into(
            scalar_register(TERMINAL_SCALAR_MAKER_LIVE_COUNT_V4)?,
            scalar_register(TERMINAL_SCALAR_ONE_V4)?,
            scalar_register(TERMINAL_SCALAR_MAKER_LIVE_COUNT_AFTER_V4)?,
        ),
        InstructionV3::increment_into(
            scalar_register(TERMINAL_SCALAR_CUSTODY_REVISION_V4)?,
            scalar_register(TERMINAL_SCALAR_CUSTODY_REVISION_AFTER_REFUND_V4)?,
        ),
        InstructionV3::increment_into(
            scalar_register(TERMINAL_SCALAR_CUSTODY_REVISION_AFTER_REFUND_V4)?,
            scalar_register(TERMINAL_SCALAR_CUSTODY_REVISION_AFTER_VAULT_V4)?,
        ),
        InstructionV3::increment_into(
            scalar_register(TERMINAL_SCALAR_CUSTODY_REVISION_AFTER_VAULT_V4)?,
            scalar_register(TERMINAL_SCALAR_CUSTODY_REVISION_AFTER_REPLAY_V4)?,
        ),
    ] {
        push_transition(&mut output, &mut next, instruction)?;
    }
    if next != output.len() {
        return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate);
    }
    Ok(output)
}

fn cancel_transition_instructions(
) -> Result<[InstructionV3; 14], DirectRegisteredTerminalArtifactErrorV4> {
    Ok([
        InstructionV3::identity_eq(
            identity_register(TERMINAL_IDENTITY_NATIVE_SIGNER_V4)?,
            identity_register(TERMINAL_IDENTITY_REQUEST_MAKER_V4)?,
        ),
        InstructionV3::identity_eq(
            identity_register(TERMINAL_IDENTITY_REQUEST_MAKER_V4)?,
            identity_register(TERMINAL_IDENTITY_RECORD_MAKER_V4)?,
        ),
        InstructionV3::identity_eq(
            identity_register(TERMINAL_IDENTITY_REQUEST_MARKET_V4)?,
            identity_register(TERMINAL_IDENTITY_RECORD_MARKET_V4)?,
        ),
        InstructionV3::identity_eq(
            identity_register(TERMINAL_IDENTITY_REQUEST_COLLATERAL_V4)?,
            identity_register(TERMINAL_IDENTITY_RECORD_COLLATERAL_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_REQUEST_SIDE_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_SIDE_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_REQUEST_LIFECYCLE_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_LIFECYCLE_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_REQUEST_OUTCOME_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_OUTCOME_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_REQUEST_GENERATION_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_GENERATION_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_REQUEST_NONCE_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_NONCE_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_REQUEST_VALID_FROM_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_VALID_FROM_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_REQUEST_VALID_THROUGH_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_VALID_THROUGH_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_REQUEST_MAXIMUM_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_MAXIMUM_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_REQUEST_LIMIT_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_LIMIT_V4)?,
        ),
        InstructionV3::scalar_eq(
            scalar_register(TERMINAL_SCALAR_REQUEST_FEE_BPS_V4)?,
            scalar_register(TERMINAL_SCALAR_RECORD_FEE_BPS_V4)?,
        ),
    ])
}

/// Emit the shared side-selected terminal EffectV4 atomically.
pub fn encode_direct_registered_terminal_effect_v4_atomic(
    action: DirectExecutionActionV3,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    require_terminal_action(action)?;
    if scratch.len() != DIRECT_REGISTERED_TERMINAL_EFFECT_BYTES_V4
        || output.len() != DIRECT_REGISTERED_TERMINAL_EFFECT_BYTES_V4
    {
        return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate);
    }
    let mut base_scratch = [0_u8; EFFECT_BASE_BYTES];
    let mut base = [0_u8; EFFECT_BASE_BYTES];
    encode_terminal_effect_base_atomic(&mut base_scratch, &mut base)?;
    let semantic_prefix = match action {
        DirectExecutionActionV3::CancelRegistered => DIRECT_REGISTERED_CANCEL_REQUEST_BYTES_V3,
        DirectExecutionActionV3::ExpireRegistered => DIRECT_EMPTY_ACTION_REQUEST_BYTES_V3,
        _ => return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate),
    };
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        width32(semantic_prefix)?,
        &[],
        &[],
        scratch,
        output,
    )
    .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Effect)?;
    ProgramV4::decode(output).map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Effect)?;
    Ok(())
}

fn encode_terminal_effect_base_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    let claims = claims_template()?;
    let refund = custody_terminal_template(OperationV1::Transfer)?;
    let close_vault = custody_terminal_template(OperationV1::CloseVault)?;
    let close_replay = custody_terminal_template(OperationV1::CloseReplay)?;
    let routes = [
        effect_route(
            FixedRole::Claims,
            Some(scalar_u16(TERMINAL_SCALAR_SELL_ENABLED_V4)?),
            DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4,
            SparseNativeTransferFrameSpecV1.account_count(),
            &claims,
        ),
        effect_route(
            FixedRole::Custody,
            Some(scalar_u16(TERMINAL_SCALAR_BUY_ENABLED_V4)?),
            DIRECT_REGISTERED_TERMINAL_CUSTODY_REFUND_ACCOUNT_START_V4,
            TRANSFER_ACCOUNT_COUNT_V1,
            &refund,
        ),
        effect_route(
            FixedRole::Custody,
            Some(scalar_u16(TERMINAL_SCALAR_BUY_ENABLED_V4)?),
            DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_VAULT_ACCOUNT_START_V4,
            CLOSE_VAULT_ACCOUNT_COUNT_V1,
            &close_vault,
        ),
        effect_route(
            FixedRole::Custody,
            Some(scalar_u16(TERMINAL_SCALAR_BUY_ENABLED_V4)?),
            DIRECT_REGISTERED_TERMINAL_CUSTODY_CLOSE_REPLAY_ACCOUNT_START_V4,
            CLOSE_REPLAY_ACCOUNT_COUNT_V1,
            &close_replay,
        ),
    ];
    let refund_receipt = RouteReceiptDependencyV3::new(
        FixedRole::Custody,
        1,
        width16(CUSTODY_RECEIPT_BYTES_V1)?,
    );
    let close_vault_receipt = RouteReceiptDependencyV3::new(
        FixedRole::Custody,
        2,
        width16(CUSTODY_RECEIPT_BYTES_V1)?,
    );
    let route2 = [refund_receipt];
    let route3 = [refund_receipt, close_vault_receipt];
    let dependencies: [&[RouteReceiptDependencyV3]; ROUTE_COUNT] = [&[], &[], &route2, &route3];
    let instructions = terminal_effect_instructions()?;
    encode_effect_program_v4_atomic(
        EffectGeometryV3 {
            fixed_accounts: DIRECT_REGISTERED_TERMINAL_FIXED_ACCOUNTS_V4,
            item_account_stride: 0,
            common_scalars: scalar_u16(DIRECT_REGISTERED_TERMINAL_COMMON_SCALARS_V4)?,
            item_scalar_stride: DIRECT_REGISTERED_TERMINAL_ITEM_SCALAR_STRIDE_V4,
            common_identities: identity_u16(DIRECT_REGISTERED_TERMINAL_COMMON_IDENTITIES_V4)?,
            item_identity_stride: DIRECT_REGISTERED_TERMINAL_ITEM_IDENTITY_STRIDE_V4,
        },
        &routes,
        &dependencies,
        &instructions,
        &[],
        scratch,
        output,
    )
    .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Effect)?;
    EffectProgramV3::decode(output).map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Effect)?;
    Ok(())
}

fn terminal_effect_instructions(
) -> Result<[EffectInstructionV3; EFFECT_INSTRUCTIONS], DirectRegisteredTerminalArtifactErrorV4> {
    let placeholder = EffectInstructionV3::write_u64(
        AccountCoordinateV3::fixed(MAKER_ACCOUNT),
        0,
        ScalarCoordinateV3::common(0),
    );
    let mut output = [placeholder; EFFECT_INSTRUCTIONS];
    let mut next = 0_usize;
    push_effect(
        &mut output,
        &mut next,
        EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(MAKER_ACCOUNT),
            width32(DirectMakerReplayLayoutV1::LIVE_COUNT)?,
            effect_scalar(TERMINAL_SCALAR_MAKER_LIVE_COUNT_AFTER_V4)?,
        ),
    )?;
    push_claims_refund_request(&mut output, &mut next)?;
    push_custody_refund_request(&mut output, &mut next)?;
    push_custody_close_vault_request(&mut output, &mut next)?;
    push_custody_close_replay_request(&mut output, &mut next)?;
    if next != output.len() {
        return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate);
    }
    Ok(output)
}

fn push_claims_refund_request(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    for (field, identity) in [
        (SparseNativeTransferLayoutV1::RELEASE_SET, TERMINAL_IDENTITY_RELEASE_SET_V4),
        (SparseNativeTransferLayoutV1::MARKET, TERMINAL_IDENTITY_MARKET_V4),
        (SparseNativeTransferLayoutV1::REQUEST_ID, TERMINAL_IDENTITY_PARENT_REQUEST_V4),
        (SparseNativeTransferLayoutV1::PRODUCT_RECORD, TERMINAL_IDENTITY_PRODUCT_RECORD_V4),
        (SparseNativeTransferLayoutV1::SEMANTIC_BASIS, TERMINAL_IDENTITY_SEMANTIC_BASIS_V4),
        (SparseNativeTransferLayoutV1::LINKED_BASIS_RECORD, TERMINAL_IDENTITY_LINKED_BASIS_V4),
        (SparseNativeTransferLayoutV1::SOURCE_OWNER, TERMINAL_IDENTITY_RECORD_STATE_V4),
        (SparseNativeTransferLayoutV1::DESTINATION_OWNER, TERMINAL_IDENTITY_RECORD_MAKER_V4),
    ] {
        push_effect_request_identity(output, next, 0, field, identity)?;
    }
    for (field, scalar) in [
        (SparseNativeTransferLayoutV1::MARKET_REVISION, TERMINAL_SCALAR_CLAIMS_MARKET_REVISION_V4),
        (SparseNativeTransferLayoutV1::SOURCE_REVISION, TERMINAL_SCALAR_CLAIMS_SOURCE_REVISION_V4),
        (SparseNativeTransferLayoutV1::DESTINATION_REVISION, TERMINAL_SCALAR_CLAIMS_DESTINATION_REVISION_V4),
        (SparseNativeTransferLayoutV1::GENERATION, TERMINAL_SCALAR_RECORD_GENERATION_V4),
        (SparseNativeTransferLayoutV1::QUANTITY, TERMINAL_SCALAR_REMAINING_V4),
    ] {
        push_effect_request_u64(output, next, 0, field, scalar)?;
    }
    for (field, scalar) in [
        (SparseNativeTransferLayoutV1::OUTCOME, TERMINAL_SCALAR_RECORD_OUTCOME_V4),
        (SparseNativeTransferLayoutV1::CLAIM_COUNT, TERMINAL_SCALAR_OUTCOME_COUNT_V4),
    ] {
        push_effect_request_u32(output, next, 0, field, scalar)?;
    }
    Ok(())
}

fn push_custody_common_request(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
    route: u16,
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    for (field, identity) in [
        (CustodyRequestLayoutV1::RELEASE_SET, TERMINAL_IDENTITY_RELEASE_SET_V4),
        (CustodyRequestLayoutV1::MARKET, TERMINAL_IDENTITY_MARKET_V4),
        (CustodyRequestLayoutV1::REALM, TERMINAL_IDENTITY_REALM_V4),
        (CustodyRequestLayoutV1::CONTEXT, TERMINAL_IDENTITY_RECORD_STATE_V4),
        (CustodyRequestLayoutV1::CALLER_PROGRAM, TERMINAL_IDENTITY_TRADING_PROGRAM_V4),
        (CustodyRequestLayoutV1::ORDER, TERMINAL_IDENTITY_RECORD_STATE_V4),
        (CustodyRequestLayoutV1::PARENT_REQUEST_DIGEST, TERMINAL_IDENTITY_PARENT_REQUEST_V4),
    ] {
        push_effect_request_identity(output, next, route, field, identity)?;
    }
    for (field, scalar) in [
        (CustodyRequestLayoutV1::ORDER_NONCE, TERMINAL_SCALAR_RECORD_NONCE_V4),
        (CustodyRequestLayoutV1::GENERATION, TERMINAL_SCALAR_RECORD_GENERATION_V4),
    ] {
        push_effect_request_u64(output, next, route, field, scalar)?;
    }
    push_effect_request_u32(
        output,
        next,
        route,
        CustodyRequestLayoutV1::EXECUTION_INDEX,
        TERMINAL_SCALAR_RECORD_OUTCOME_V4,
    )
}

fn push_custody_refund_request(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    push_custody_common_request(output, next, 1)?;
    for (field, identity) in [
        (CustodyRequestLayoutV1::DESTINATION_OWNER, TERMINAL_IDENTITY_RECORD_MAKER_V4),
        (CustodyRequestLayoutV1::SOURCE, TERMINAL_IDENTITY_CUSTODY_VAULT_V4),
        (CustodyRequestLayoutV1::DESTINATION, TERMINAL_IDENTITY_RECORD_COLLATERAL_V4),
        (CustodyRequestLayoutV1::SOURCE_VAULT_CONTEXT, TERMINAL_IDENTITY_RECORD_STATE_V4),
        (CustodyRequestLayoutV1::MINT, TERMINAL_IDENTITY_MINT_V4),
        (CustodyRequestLayoutV1::TOKEN_PROGRAM, TERMINAL_IDENTITY_TOKEN_PROGRAM_V4),
    ] {
        push_effect_request_identity(output, next, 1, field, identity)?;
    }
    for (field, scalar) in [
        (CustodyRequestLayoutV1::EXPECTED_REVISION, TERMINAL_SCALAR_CUSTODY_REVISION_V4),
        (CustodyRequestLayoutV1::RESULTING_REVISION, TERMINAL_SCALAR_CUSTODY_REVISION_AFTER_REFUND_V4),
        (CustodyRequestLayoutV1::AMOUNT, TERMINAL_SCALAR_EXPECTED_COLLATERAL_V4),
    ] {
        push_effect_request_u64(output, next, 1, field, scalar)?;
    }
    Ok(())
}

fn push_custody_close_vault_request(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    push_custody_common_request(output, next, 2)?;
    for (field, identity) in [
        (CustodyRequestLayoutV1::SOURCE, TERMINAL_IDENTITY_CUSTODY_VAULT_V4),
        (CustodyRequestLayoutV1::SOURCE_VAULT_CONTEXT, TERMINAL_IDENTITY_RECORD_STATE_V4),
        (CustodyRequestLayoutV1::MINT, TERMINAL_IDENTITY_MINT_V4),
        (CustodyRequestLayoutV1::TOKEN_PROGRAM, TERMINAL_IDENTITY_TOKEN_PROGRAM_V4),
        (CustodyRequestLayoutV1::RENT_REFUND, TERMINAL_IDENTITY_RECORD_RENT_OWNER_V4),
    ] {
        push_effect_request_identity(output, next, 2, field, identity)?;
    }
    for (field, scalar) in [
        (CustodyRequestLayoutV1::EXPECTED_REVISION, TERMINAL_SCALAR_CUSTODY_REVISION_AFTER_REFUND_V4),
        (CustodyRequestLayoutV1::RESULTING_REVISION, TERMINAL_SCALAR_CUSTODY_REVISION_AFTER_VAULT_V4),
        (CustodyRequestLayoutV1::RENT_LAMPORTS, TERMINAL_SCALAR_CUSTODY_VAULT_LAMPORTS_V4),
    ] {
        push_effect_request_u64(output, next, 2, field, scalar)?;
    }
    Ok(())
}

fn push_custody_close_replay_request(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    push_custody_common_request(output, next, 3)?;
    push_effect_request_identity(
        output,
        next,
        3,
        CustodyRequestLayoutV1::RENT_REFUND,
        TERMINAL_IDENTITY_RECORD_RENT_OWNER_V4,
    )?;
    for (field, scalar) in [
        (CustodyRequestLayoutV1::EXPECTED_REVISION, TERMINAL_SCALAR_CUSTODY_REVISION_AFTER_VAULT_V4),
        (CustodyRequestLayoutV1::RESULTING_REVISION, TERMINAL_SCALAR_CUSTODY_REVISION_AFTER_REPLAY_V4),
        (CustodyRequestLayoutV1::RENT_LAMPORTS, TERMINAL_SCALAR_CUSTODY_REPLAY_LAMPORTS_V4),
    ] {
        push_effect_request_u64(output, next, 3, field, scalar)?;
    }
    Ok(())
}

fn claims_template(
) -> Result<[u8; SPARSE_NATIVE_TRANSFER_BYTES_V1], DirectRegisteredTerminalArtifactErrorV4> {
    SparseNativeTransferV1::new(SparseNativeTransferInputV1 {
        caller_role: ClaimsCallerRole::Trading,
        release_set: test_id(1),
        market: test_id(2),
        request_id: test_id(3),
        product_record_digest: test_id(4),
        semantic_basis_id: test_id(5),
        linked_basis_record_digest: test_id(6),
        source_owner: test_id(7),
        destination_owner: test_id(8),
        expected_market_revision: 1,
        expected_source_revision: 1,
        expected_destination_revision: 1,
        generation: 1,
        outcome: 0,
        claim_count: 1,
        quantity: 1,
    })
    .map(SparseNativeTransferV1::to_bytes)
    .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Effect)
}

fn custody_terminal_template(
    operation: OperationV1,
) -> Result<[u8; CUSTODY_REQUEST_BYTES_V1], DirectRegisteredTerminalArtifactErrorV4> {
    let (source_compartment, destination_compartment, transfer_index) = match operation {
        OperationV1::Transfer => (CompartmentV1::TradingPrincipal, CompartmentV1::External, 0),
        OperationV1::CloseVault => (CompartmentV1::TradingPrincipal, CompartmentV1::None, 1),
        OperationV1::CloseReplay => (CompartmentV1::None, CompartmentV1::None, 2),
        _ => return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate),
    };
    let transfer = operation == OperationV1::Transfer;
    let close_vault = operation == OperationV1::CloseVault;
    let expected = match operation {
        OperationV1::Transfer => 1,
        OperationV1::CloseVault => 2,
        OperationV1::CloseReplay => 3,
        _ => return Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate),
    };
    CustodyRequestV1 {
        operation,
        caller_role: CallerRoleV1::Trading,
        source_compartment,
        destination_compartment,
        release_set: test_id(1),
        market: test_id(2),
        realm: test_id(3),
        context: test_id(4),
        caller_program: test_id(5),
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: [0; 32],
            destination_owner: if transfer { test_id(6) } else { [0; 32] },
            order: test_id(7),
            parent_request_digest: test_id(8),
            order_nonce: 1,
            generation: 1,
            page_index: 0,
            execution_index: 0,
            transfer_index,
        },
        source: if transfer || close_vault { test_id(9) } else { [0; 32] },
        destination: if transfer { test_id(10) } else { [0; 32] },
        source_vault_context: if transfer || close_vault { test_id(4) } else { [0; 32] },
        destination_vault_context: [0; 32],
        mint: if transfer || close_vault { test_id(11) } else { [0; 32] },
        token_program: if transfer || close_vault { test_id(12) } else { [0; 32] },
        payer: [0; 32],
        rent_refund: if transfer { [0; 32] } else { test_id(13) },
        expected_revision: expected,
        resulting_revision: expected + 1,
        amount: u64::from(transfer),
        rent_lamports: u64::from(!transfer),
    }
    .to_bytes()
    .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Effect)
}

fn effect_route<'a>(
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

const fn require_terminal_action(
    action: DirectExecutionActionV3,
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    match action {
        DirectExecutionActionV3::CancelRegistered | DirectExecutionActionV3::ExpireRegistered => {
            Ok(())
        }
        _ => Err(DirectRegisteredTerminalArtifactErrorV4::Coordinate),
    }
}

const fn exact(
    privileges: AccountPrivilegesV2,
    effect_permissions: AccountEffectPermissionsV2,
    data_length: u32,
    data_item_stride: u32,
) -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges,
            effect_permissions,
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length,
            data_item_stride,
        },
        prestate: AccountPrestateV2::Exact,
    }
}

const fn opaque(privileges: AccountPrivilegesV2) -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges,
            effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 0,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
    }
}

fn rule_mut(
    rules: &mut [AccountRuleWithPrestateInputV2],
    index: u16,
) -> Result<&mut AccountRuleWithPrestateInputV2, DirectRegisteredTerminalArtifactErrorV4> {
    rules
        .get_mut(usize::from(index))
        .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)
}

fn length(
    lengths: &[u32],
    index: u16,
) -> Result<u32, DirectRegisteredTerminalArtifactErrorV4> {
    lengths
        .get(usize::from(index))
        .copied()
        .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)
}

fn register_geometry(
) -> Result<RegisterGeometryV2, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(RegisterGeometryV2 {
        common_scalars: scalar_u16(DIRECT_REGISTERED_TERMINAL_COMMON_SCALARS_V4)?,
        item_scalar_stride: DIRECT_REGISTERED_TERMINAL_ITEM_SCALAR_STRIDE_V4,
        common_identities: identity_u16(DIRECT_REGISTERED_TERMINAL_COMMON_IDENTITIES_V4)?,
        item_identity_stride: DIRECT_REGISTERED_TERMINAL_ITEM_IDENTITY_STRIDE_V4,
    })
}

fn register_program_geometry(
) -> Result<ProgramGeometryV3, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(ProgramGeometryV3 {
        common_scalars: scalar_u16(DIRECT_REGISTERED_TERMINAL_COMMON_SCALARS_V4)?,
        item_scalar_stride: DIRECT_REGISTERED_TERMINAL_ITEM_SCALAR_STRIDE_V4,
        common_identities: identity_u16(DIRECT_REGISTERED_TERMINAL_COMMON_IDENTITIES_V4)?,
        item_identity_stride: DIRECT_REGISTERED_TERMINAL_ITEM_IDENTITY_STRIDE_V4,
    })
}

fn require_owner(
    account: u16,
    expected: usize,
) -> Result<AccountOperationInputV2, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(AccountOperationInputV2::RequireOwner {
        account: AccountCoordinateV2::fixed(account),
        expected: IdentityCoordinateV2::common(identity_u16(expected)?),
    })
}

fn project_key(
    account: u16,
    destination: usize,
) -> Result<AccountOperationInputV2, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(AccountOperationInputV2::ProjectKey {
        account: AccountCoordinateV2::fixed(account),
        destination: IdentityCoordinateV2::common(identity_u16(destination)?),
    })
}

fn project_lamports(
    account: u16,
    destination: usize,
) -> Result<AccountOperationInputV2, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(AccountOperationInputV2::ProjectLamports {
        account: AccountCoordinateV2::fixed(account),
        destination: ScalarCoordinateV2::common(scalar_u16(destination)?),
    })
}

fn project_u8(
    account: u16,
    data_offset: usize,
    destination: usize,
) -> Result<AccountOperationInputV2, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(AccountOperationInputV2::ProjectDataU8 {
        account: AccountCoordinateV2::fixed(account),
        destination: ScalarCoordinateV2::common(scalar_u16(destination)?),
        data_offset: width32(data_offset)?,
    })
}

fn project_u16(
    account: u16,
    data_offset: usize,
    destination: usize,
) -> Result<AccountOperationInputV2, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(AccountOperationInputV2::ProjectDataU16 {
        account: AccountCoordinateV2::fixed(account),
        destination: ScalarCoordinateV2::common(scalar_u16(destination)?),
        data_offset: width32(data_offset)?,
    })
}

fn project_u32(
    account: u16,
    data_offset: usize,
    destination: usize,
) -> Result<AccountOperationInputV2, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(AccountOperationInputV2::ProjectDataU32 {
        account: AccountCoordinateV2::fixed(account),
        destination: ScalarCoordinateV2::common(scalar_u16(destination)?),
        data_offset: width32(data_offset)?,
    })
}

fn project_u64(
    account: u16,
    data_offset: usize,
    destination: usize,
) -> Result<AccountOperationInputV2, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(AccountOperationInputV2::ProjectDataU64 {
        account: AccountCoordinateV2::fixed(account),
        destination: ScalarCoordinateV2::common(scalar_u16(destination)?),
        data_offset: width32(data_offset)?,
    })
}

fn project_identity(
    account: u16,
    data_offset: usize,
    destination: usize,
) -> Result<AccountOperationInputV2, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(AccountOperationInputV2::ProjectDataIdentity {
        account: AccountCoordinateV2::fixed(account),
        destination: IdentityCoordinateV2::common(identity_u16(destination)?),
        data_offset: width32(data_offset)?,
    })
}

fn push_account_operation(
    output: &mut [AccountOperationInputV2],
    next: &mut usize,
    operation: AccountOperationInputV2,
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    *output
        .get_mut(*next)
        .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)? = operation;
    *next = next
        .checked_add(1)
        .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)?;
    Ok(())
}

fn request_coordinate(
    offset: usize,
) -> Result<RequestCoordinateV1, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(RequestCoordinateV1::fixed(width32(offset)?))
}

fn require_request_u16(
    offset: usize,
    value: u16,
) -> Result<RequestInstructionV1, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(RequestInstructionV1::require_u16(request_coordinate(offset)?, value))
}

fn require_request_u32(
    offset: usize,
    value: u32,
) -> Result<RequestInstructionV1, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(RequestInstructionV1::require_u32(request_coordinate(offset)?, value))
}

fn require_request_u64(
    offset: usize,
    value: u64,
) -> Result<RequestInstructionV1, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(RequestInstructionV1::require_u64(request_coordinate(offset)?, value))
}

fn project_request_u8(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(RequestInstructionV1::project_u8(
        request_coordinate(offset)?,
        ScalarRegisterV1::common(scalar_u16(destination)?),
    ))
}

fn project_request_u16(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(RequestInstructionV1::project_u16(
        request_coordinate(offset)?,
        ScalarRegisterV1::common(scalar_u16(destination)?),
    ))
}

fn project_request_u32(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(RequestInstructionV1::project_u32(
        request_coordinate(offset)?,
        ScalarRegisterV1::common(scalar_u16(destination)?),
    ))
}

fn project_request_u64(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(RequestInstructionV1::project_u64(
        request_coordinate(offset)?,
        ScalarRegisterV1::common(scalar_u16(destination)?),
    ))
}

fn project_request_identity(
    offset: usize,
    destination: usize,
) -> Result<RequestInstructionV1, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(RequestInstructionV1::project_identity(
        request_coordinate(offset)?,
        IdentityRegisterV1::common(identity_u16(destination)?),
    ))
}

fn absolute_message_offset(
    relative: u32,
) -> Result<u16, DirectRegisteredTerminalArtifactErrorV4> {
    u32::try_from(HOT_FAMILY_REQUEST_OFFSET_V3)
        .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Coordinate)?
        .checked_add(relative)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)
}

fn intent_domain_word(offset: usize) -> Result<u64, DirectRegisteredTerminalArtifactErrorV4> {
    let end = offset
        .checked_add(8)
        .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)?;
    Ok(u64::from_le_bytes(
        intent::COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2
            .get(offset..end)
            .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)?
            .try_into()
            .map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Coordinate)?,
    ))
}

fn push_transition(
    output: &mut [InstructionV3],
    next: &mut usize,
    instruction: InstructionV3,
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    *output
        .get_mut(*next)
        .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)? = instruction;
    *next = next
        .checked_add(1)
        .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)?;
    Ok(())
}

fn scalar_register(
    value: usize,
) -> Result<ScalarRegisterV3, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(ScalarRegisterV3::common(scalar_u16(value)?))
}

fn identity_register(
    value: usize,
) -> Result<IdentityRegisterV3, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(IdentityRegisterV3::common(identity_u16(value)?))
}

fn effect_scalar(
    value: usize,
) -> Result<ScalarCoordinateV3, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(ScalarCoordinateV3::common(scalar_u16(value)?))
}

fn effect_identity(
    value: usize,
) -> Result<IdentityCoordinateV3, DirectRegisteredTerminalArtifactErrorV4> {
    Ok(IdentityCoordinateV3::common(identity_u16(value)?))
}

fn push_effect_request_identity(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
    route: u16,
    field: usize,
    value: usize,
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    push_effect(
        output,
        next,
        EffectInstructionV3::write_request_identity(
            route,
            RequestSpaceV3::Fixed,
            width32(field)?,
            effect_identity(value)?,
        ),
    )
}

fn push_effect_request_u64(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
    route: u16,
    field: usize,
    value: usize,
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    push_effect(
        output,
        next,
        EffectInstructionV3::write_request_u64(
            route,
            RequestSpaceV3::Fixed,
            width32(field)?,
            effect_scalar(value)?,
        ),
    )
}

fn push_effect_request_u32(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
    route: u16,
    field: usize,
    value: usize,
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    push_effect(
        output,
        next,
        EffectInstructionV3::write_request_u32(
            route,
            RequestSpaceV3::Fixed,
            width32(field)?,
            effect_scalar(value)?,
        ),
    )
}

fn push_effect(
    output: &mut [EffectInstructionV3],
    next: &mut usize,
    instruction: EffectInstructionV3,
) -> Result<(), DirectRegisteredTerminalArtifactErrorV4> {
    *output
        .get_mut(*next)
        .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)? = instruction;
    *next = next
        .checked_add(1)
        .ok_or(DirectRegisteredTerminalArtifactErrorV4::Coordinate)?;
    Ok(())
}

fn scalar_u16(value: usize) -> Result<u16, DirectRegisteredTerminalArtifactErrorV4> {
    u16::try_from(value).map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Coordinate)
}

fn identity_u16(value: usize) -> Result<u16, DirectRegisteredTerminalArtifactErrorV4> {
    u16::try_from(value).map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Coordinate)
}

fn width16(value: usize) -> Result<u16, DirectRegisteredTerminalArtifactErrorV4> {
    u16::try_from(value).map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Coordinate)
}

fn width32(value: usize) -> Result<u32, DirectRegisteredTerminalArtifactErrorV4> {
    u32::try_from(value).map_err(|_| DirectRegisteredTerminalArtifactErrorV4::Coordinate)
}

const fn test_id(value: u8) -> [u8; 32] {
    [value; 32]
}
