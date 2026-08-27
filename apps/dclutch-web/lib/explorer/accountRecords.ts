/**
 * The account-record render map.
 *
 * Every canonical dClutch account carries an eight-byte ASCII magic at offset
 * zero. `lib/generated/` is the decode authority for what those magics are and
 * where each record's fields sit; this module is the table that turns one into
 * the other, and it states NO offset, width, magic or seed of its own — every
 * number below is an imported constant, which `lib/abiCoverage.test.ts` and
 * `lib/explorerCoverage.test.ts` between them enforce.
 *
 * The honesty rules this table is built to keep:
 *
 *   - A magic the generated modules declare is ALWAYS identified, even when no
 *     layout was emitted for it. Such a record renders with its name, its
 *     stated width, and a `note` saying the layout is not emitted — never with
 *     invented fields.
 *   - A magic the generated modules do NOT declare is rendered as unknown, with
 *     its bytes in hex and the magic shown as text if it is printable. It is
 *     never matched to a "close" layout.
 *   - A field's width is not guessed: it is the distance to the next declared
 *     offset, or to the record's declared end. A field whose kind and that
 *     width disagree is a decode refusal, not a silent reinterpretation.
 *   - A 32-byte field is rendered as base58 only when the generated module's
 *     own name for it says it is an account (`..._PROGRAM_`, `..._MARKET_`,
 *     `..._OWNER_`, `..._MINT_`), and as hex when it is a content identity.
 *     Where the emitted name settles neither, both forms are shown.
 */
import { PublicKey } from '@solana/web3.js';

import {
  CAPABILITY_ENTRY_BYTES_V1,
  CAPABILITY_FUNDING_QUOTE_AMOUNTS_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_BINDING_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_BYTES_V1,
  CAPABILITY_FUNDING_QUOTE_COLLATERAL_KIND_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_MAGIC_V1,
  CAPABILITY_FUNDING_QUOTE_RESERVED_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_SCHEMA_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_ACTIVATION_SLOT_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_BODY_RESERVED_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_BYTES_V1,
  CAPABILITY_FUNDING_STATE_ENTRY_INDEX_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_HEADER_RESERVED_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_MAGIC_V1,
  CAPABILITY_FUNDING_STATE_MANIFEST_ID_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_RELEASED_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_REMAINING_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_SCHEMA_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_STATUS_OFFSET_V1,
  CAPABILITY_MANIFEST_COUNT_OFFSET_V1,
  CAPABILITY_MANIFEST_HEADER_BYTES_V1,
  CAPABILITY_MANIFEST_MAGIC_V1,
  CAPABILITY_MANIFEST_PROFILE_OFFSET_V1,
  CAPABILITY_MANIFEST_RESERVED_OFFSET_V1,
  CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1,
  MARKET_OPENING_READINESS_BYTES_V1,
  MARKET_OPENING_READINESS_MAGIC_V1,
} from '../generated/capabilityManifestV1';
import {
  CORE_PHASE_FOUNDING_TAG,
  CORE_PHASE_OPEN_TAG,
  CORE_PHASE_RETIRED_TAG,
  CORE_PHASE_RETIRING_TAG,
  CORE_PHASE_TERMINAL_TAG,
  CORE_READINESS_CONSUMED_TAG,
  CORE_READINESS_PREPAID_TAG,
  CORE_READINESS_READY_TAG,
  CORE_REQUEST_BYTES,
  CORE_REQUEST_MAGIC,
  CORE_STATE_BYTES,
  CORE_STATE_CAPABILITY_MANIFEST_OFFSET,
  CORE_STATE_GENERATION_OFFSET,
  CORE_STATE_IDENTITY_REALM_OFFSET,
  CORE_STATE_MAGIC,
  CORE_STATE_MARKET_ID_OFFSET,
  CORE_STATE_OUTSTANDING_CAPABILITIES_OFFSET,
  CORE_STATE_PHASE_OFFSET,
  CORE_STATE_PRODUCT_ID_OFFSET,
  CORE_STATE_PRODUCT_RECORD_OFFSET,
  CORE_STATE_READINESS_OFFSET,
  CORE_STATE_REGISTRY_PROGRAM_OFFSET,
  CORE_STATE_RENT_BENEFICIARY_OFFSET,
  CORE_STATE_RESOLUTION_POLICY_OFFSET,
  CORE_STATE_SELECTED_RELEASE_SET_OFFSET,
  CORE_STATE_TERMINAL_RECEIPT_OFFSET,
  CORE_STATE_TERMINAL_WINNER_OFFSET,
  CORE_STATE_VERSION_OFFSET,
  LIABILITY_BASIS_MARKET_BASIS_OFFSET,
  LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET,
  LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET,
  LIABILITY_BASIS_MARKET_GENERATION_OFFSET,
  LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
  LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET,
  LIABILITY_BASIS_MARKET_MAGIC_V2,
  LIABILITY_BASIS_MARKET_PRODUCT_OFFSET,
  LIABILITY_BASIS_MARKET_REALM_OFFSET,
  LIABILITY_BASIS_MARKET_REGISTRY_OFFSET,
  LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET,
  LIABILITY_BASIS_MARKET_REVISION_OFFSET,
  LIABILITY_BASIS_POSITION_BASIS_OFFSET,
  LIABILITY_BASIS_POSITION_CLAIM_COUNT_OFFSET,
  LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
  LIABILITY_BASIS_POSITION_MAGIC_V2,
  LIABILITY_BASIS_POSITION_MARKET_OFFSET,
  LIABILITY_BASIS_POSITION_OWNER_OFFSET,
  LIABILITY_BASIS_POSITION_RESERVED_OFFSET,
  LIABILITY_BASIS_POSITION_REVISION_OFFSET,
  CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2,
  LIFECYCLE_RENT_ACTION_CREATE_V2,
  LIFECYCLE_RENT_CREDIT_BYTES_V2,
  LIFECYCLE_RENT_CREDIT_MAGIC_V2,
  LIFECYCLE_RENT_INSTRUCTION_ACTION_OFFSET_V2,
  LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2,
} from '../generated/coreFound';
import {
  ACCOUNT_PROFILE_HEADER_BYTES_V2,
  ACCOUNT_PROFILE_MAGIC_V2,
  DEALER_CONFIG_BYTES_V4,
  DEALER_CONFIG_LOCKED_CAPITAL_FLOOR_OFFSET_V4,
  DEALER_CONFIG_MAGIC_V4,
  DEALER_CONFIG_POSITION_OWNER_OFFSET_V4,
  DEALER_CONFIG_REALM_OFFSET_V4,
  DEALER_CONFIG_RELEASE_SET_OFFSET_V4,
  DEALER_EQUITY_CLAIMS_PACKET_BYTES_OFFSET_V3,
  DEALER_EQUITY_HEADER_BYTES_V3,
  DEALER_EQUITY_REQUEST_MAGIC_V3,
  DEALER_EQUITY_SELECTOR_OFFSET_V3,
  DEALER_LP_POSITION_BYTES_V3,
  DEALER_LP_POSITION_MAGIC_V3,
  DEALER_OBLIGATION_HEADER_BYTES_V3,
  DEALER_OBLIGATION_MAGIC_V3,
  SIGNED_DELTA_PLAN_HEADER_BYTES_V3,
  SIGNED_DELTA_PLAN_MAGIC_V3,
} from '../generated/dealerEquityV3';
import {
  BASIS_HEADER_BYTES_V3,
  BASIS_MAGIC_V3,
  BASIS_WIDTH_OFFSET_V3,
  CAPABILITY_EXECUTION_SELECTION_BYTES_V1,
  CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_MAGIC_V1,
  CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_PROFILE_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_RESERVED_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_SCHEMA_VERSION_OFFSET,
  CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2,
  CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V2,
  CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2,
  CAPABILITY_PROGRAM_SET_MAGIC_V2,
  CAPABILITY_PROGRAM_SET_RESERVED_OFFSET_V2,
  CAPABILITY_PROGRAM_SET_SELECTOR_ENDIAN_OFFSET_V2,
  CAPABILITY_PROGRAM_SET_SELECTOR_OFFSET_OFFSET_V2,
  CAPABILITY_PROGRAM_SET_SELECTOR_WIDTH_OFFSET_V2,
  CAPABILITY_PROGRAM_V3_ACCOUNT_PROFILE_OFFSET,
  CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE_OFFSET,
  CAPABILITY_PROGRAM_V3_BYTES,
  CAPABILITY_PROGRAM_V3_CAPACITY_PROFILE_OFFSET,
  CAPABILITY_PROGRAM_V3_CONFIG_SCHEMA_OFFSET,
  CAPABILITY_PROGRAM_V3_DERIVATION_POLICY_OFFSET,
  CAPABILITY_PROGRAM_V3_EFFECT_PROGRAM_OFFSET,
  CAPABILITY_PROGRAM_V3_KIND_OFFSET,
  CAPABILITY_PROGRAM_V3_MAGIC,
  CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_PROGRAM_OFFSET,
  CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_OFFSET,
  CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION_OFFSET,
  CAPABILITY_PROGRAM_V3_REQUEST_SCHEMA_OFFSET,
  CAPABILITY_PROGRAM_V3_ROOT_SCHEMA_OFFSET,
  CAPABILITY_PROGRAM_V3_ROOT_STATE_BYTES_OFFSET,
  CAPABILITY_PROGRAM_V3_SCHEMA_VERSION_OFFSET,
  CAPABILITY_PROGRAM_V3_TAIL_RESERVED_OFFSET,
  CAPABILITY_PROGRAM_V3_TRANSITION_PROGRAM_OFFSET,
  CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_OFFSET,
  CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION_OFFSET,
  CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_PROGRAM_OFFSET,
  CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_SCHEMA_OFFSET,
  CAPABILITY_PROGRAM_V4_ARTIFACT_PROFILE_OFFSET,
  CAPABILITY_PROGRAM_V4_BYTES,
  CAPABILITY_PROGRAM_V4_CAPACITY_PROFILE_OFFSET,
  CAPABILITY_PROGRAM_V4_CONFIG_SCHEMA_OFFSET,
  CAPABILITY_PROGRAM_V4_DERIVATION_POLICY_OFFSET,
  CAPABILITY_PROGRAM_V4_EFFECT_PROGRAM_OFFSET,
  CAPABILITY_PROGRAM_V4_EFFECT_SCHEMA_OFFSET,
  CAPABILITY_PROGRAM_V4_HEADER_RESERVED_OFFSET,
  CAPABILITY_PROGRAM_V4_KIND_OFFSET,
  CAPABILITY_PROGRAM_V4_LIFECYCLE_PROGRAM_OFFSET,
  CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET,
  CAPABILITY_PROGRAM_V4_MAGIC,
  CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_PROGRAM_OFFSET,
  CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_SCHEMA_OFFSET,
  CAPABILITY_PROGRAM_V4_REQUEST_SCHEMA_OFFSET,
  CAPABILITY_PROGRAM_V4_ROOT_SCHEMA_OFFSET,
  CAPABILITY_PROGRAM_V4_ROOT_STATE_BYTES_OFFSET,
  CAPABILITY_PROGRAM_V4_SCHEMA_VERSION_OFFSET,
  CAPABILITY_PROGRAM_V4_STRATEGY_PROGRAM_OFFSET,
  CAPABILITY_PROGRAM_V4_STRATEGY_SCHEMA_OFFSET,
  CAPABILITY_PROGRAM_V4_TAIL_RESERVED_OFFSET,
  CAPABILITY_PROGRAM_V4_TRANSITION_PROGRAM_OFFSET,
  CAPABILITY_PROGRAM_V4_TRANSITION_SCHEMA_OFFSET,
  CAPABILITY_ROOT_GENERATION_OFFSET,
  CAPABILITY_ROOT_HEADER_BYTES_V1,
  CAPABILITY_ROOT_MAGIC_V1,
  CAPABILITY_ROOT_MARKET_OFFSET,
  CAPABILITY_ROOT_PROFILE_OFFSET,
  CAPABILITY_ROOT_RELEASE_SET_OFFSET,
  CAPABILITY_ROOT_RESERVED_OFFSET,
  CAPABILITY_ROOT_SCHEMA_VERSION_OFFSET,
  CAPABILITY_ROOT_SELECTION_OFFSET,
  COMPACT_INTENT_BYTES_V2,
  COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2,
  COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2,
  COMPACT_INTENT_GENERATION_OFFSET_V2,
  COMPACT_INTENT_LIFECYCLE_OFFSET_V2,
  COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2,
  COMPACT_INTENT_MAGIC_V2,
  COMPACT_INTENT_MARKET_OFFSET_V2,
  COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2,
  COMPACT_INTENT_NONCE_OFFSET_V2,
  COMPACT_INTENT_OUTCOME_OFFSET_V2,
  COMPACT_INTENT_SIDE_OFFSET_V2,
  COMPACT_INTENT_VALID_FROM_OFFSET_V2,
  COMPACT_INTENT_VALID_THROUGH_OFFSET_V2,
  DIRECT_CONFIG_FEE_BPS_OFFSET_V1,
  DIRECT_CONFIG_FEE_RECIPIENT_OFFSET_V1,
  DIRECT_CONFIG_MAGIC_V1,
  DIRECT_CONFIG_PRICE_SCALE_OFFSET_V1,
  DIRECT_CONFIG_RESERVED_A_OFFSET_V1,
  DIRECT_CONFIG_RESERVED_B_OFFSET_V1,
  DIRECT_CONFIG_VERSION_OFFSET_V1,
  DIRECT_EXECUTION_CONFIG_BYTES_V1,
  DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3,
  DIRECT_EXECUTION_REQUEST_MAGIC_V3,
  DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3,
  EXECUTION_STRATEGY_PROGRAM_BYTES_V2,
  EXECUTION_STRATEGY_PROGRAM_MAGIC_V2,
  HOT_EXECUTION_MAGIC_V3,
  REQUEST_PROFILE_ARTIFACT_OFFSET,
  REQUEST_PROFILE_COMMON_IDENTITIES_OFFSET,
  REQUEST_PROFILE_COMMON_SCALARS_OFFSET,
  REQUEST_PROFILE_FIXED_OPERATIONS_OFFSET,
  REQUEST_PROFILE_FIXED_REQUEST_BYTES_OFFSET,
  REQUEST_PROFILE_HEADER_BYTES_V1,
  REQUEST_PROFILE_ITEM_IDENTITY_STRIDE_OFFSET,
  REQUEST_PROFILE_ITEM_OPERATIONS_OFFSET,
  REQUEST_PROFILE_ITEM_REQUEST_BYTES_OFFSET,
  REQUEST_PROFILE_ITEM_SCALAR_STRIDE_OFFSET,
  REQUEST_PROFILE_MAGIC_V1,
  REQUEST_PROFILE_OPERATION_BYTES_V1,
  REQUEST_PROFILE_V2_HEADER_BYTES,
  REQUEST_PROFILE_V2_MAGIC,
  REQUEST_PROFILE_VERSION_OFFSET,
  STRATEGY_DISPOSITION_OFFSET_V2,
  STRATEGY_TRANSITION_PROGRAM_OFFSET_V2,
  STRATEGY_TRANSITION_SCHEMA_OFFSET_V2,
} from '../generated/directInlineV3';
import {
  ACTION_CLOSE_V2,
  ACTION_COLLECT_V2,
  ACTION_CONSIDER_V2,
  ACTION_DISTRIBUTE_V2,
  ACTION_FREEZE_V2,
  ACTION_INITIALIZE_SETTLEMENT_V2,
  ACTION_MATERIALIZE_V2,
  GENERAL_ACK_EXECUTION_DIGEST_OFFSET_V3,
  GENERAL_ACK_GENERATION_OFFSET_V3,
  GENERAL_ACK_MARKET_OFFSET_V3,
  GENERAL_ACK_RELEASE_SET_OFFSET_V3,
  GENERAL_ACK_REQUEST_DIGEST_OFFSET_V3,
  GENERAL_ACK_ROOT_OFFSET_V3,
  GENERAL_ACK_ROOT_POSTSTATE_DIGEST_OFFSET_V3,
  GENERAL_ACK_ROOT_PRESTATE_DIGEST_OFFSET_V3,
  GENERAL_ACK_SELECTED_PROGRAM_OFFSET_V3,
  GENERAL_ENVELOPE_GENERATION_OFFSET_V3,
  GENERAL_ENVELOPE_MARKET_OFFSET_V3,
  GENERAL_ENVELOPE_RELEASE_SET_OFFSET_V3,
  GENERAL_ENVELOPE_REQUEST_BYTES_OFFSET_V3,
  GENERAL_ENVELOPE_RESERVED_OFFSET_V3,
  GENERAL_ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET_V3,
  GENERAL_HOT_ACK_BYTES_V3,
  GENERAL_HOT_ACK_MAGIC_V3,
  GENERAL_HOT_ENVELOPE_BYTES_V3,
  GENERAL_LOCAL_STATE_BENEFICIARY_OFFSET_V3,
  GENERAL_LOCAL_STATE_BODY_OFFSET_V3,
  GENERAL_LOCAL_STATE_BUMP_OFFSET_V3,
  GENERAL_LOCAL_STATE_HEADER_BYTES_V3,
  GENERAL_LOCAL_STATE_KIND_OFFSET_V3,
  GENERAL_LOCAL_STATE_MAGIC_V3,
  GENERAL_LOCAL_STATE_RENT_PRINCIPAL_OFFSET_V3,
  GENERAL_LOCAL_STATE_SELECTION_KIND_V3,
  GENERAL_LOCAL_STATE_SETTLEMENT_KIND_V3,
  GENERAL_PHASE_COLLECTING_V2,
  GENERAL_PHASE_DISTRIBUTING_V2,
  GENERAL_PHASE_MATERIALIZING_V2,
  GENERAL_PHASE_READY_TO_CLOSE_V2,
  GENERAL_PHASE_TERMINAL_V2,
  GENERAL_REQUEST_ACTION_OFFSET_V2,
  GENERAL_REQUEST_BYTES_V2,
  GENERAL_REQUEST_CANDIDATE_ID_OFFSET_V2,
  GENERAL_REQUEST_EXECUTION_INDEX_OFFSET_V2,
  GENERAL_REQUEST_EXPECTED_REVISION_OFFSET_V2,
  GENERAL_REQUEST_MAGIC_V2,
  GENERAL_REQUEST_MANIFEST_ORDER_OFFSET_V2,
  GENERAL_REQUEST_PAGE_INDEX_OFFSET_V2,
  GENERAL_REQUEST_STATE_BUMP_OFFSET_V2,
  GENERAL_REQUEST_TERMINAL_BUMP_OFFSET_V2,
  GENERAL_SELECTION_BATCH_ID_OFFSET_V2,
  GENERAL_SELECTION_BEST_CANDIDATE_OFFSET_V2,
  GENERAL_SELECTION_BEST_COORDINATE_OFFSET_V2,
  GENERAL_SELECTION_BYTES_V2,
  GENERAL_SELECTION_FILLED_LOTS_OFFSET_V2,
  GENERAL_SELECTION_MAGIC_V2,
  GENERAL_SELECTION_OUTCOME_COUNT_OFFSET_V2,
  GENERAL_SELECTION_PHASE_OFFSET_V2,
  GENERAL_SELECTION_POLICY_ID_OFFSET_V2,
  GENERAL_SELECTION_PRICE_SCALE_OFFSET_V2,
  GENERAL_SELECTION_PRODUCT_ID_OFFSET_V2,
  GENERAL_SELECTION_QUOTE_SURPLUS_OFFSET_V2,
  GENERAL_SELECTION_REVISION_OFFSET_V2,
  GENERAL_SELECTION_SUBMITTED_COUNT_OFFSET_V2,
  GENERAL_SELECTION_VERIFIED_DIGEST_OFFSET_V2,
  GENERAL_SELECTION_VERIFIED_REVISION_OFFSET_V2,
  GENERAL_SETTLEMENT_CANDIDATE_ID_OFFSET_V2,
  GENERAL_SETTLEMENT_COMPLETE_SET_OFFSET_V2,
  GENERAL_SETTLEMENT_HEADER_BYTES_V2,
  GENERAL_SETTLEMENT_INVENTORY_OFFSET_V2,
  GENERAL_SETTLEMENT_INVENTORY_STRIDE_V2,
  GENERAL_SETTLEMENT_MAGIC_V2,
  GENERAL_SETTLEMENT_NEXT_ORDER_OFFSET_V2,
  GENERAL_SETTLEMENT_ORDER_COUNT_OFFSET_V2,
  GENERAL_SETTLEMENT_OUTCOME_COUNT_OFFSET_V2,
  GENERAL_SETTLEMENT_PHASE_OFFSET_V2,
  GENERAL_SETTLEMENT_QUOTE_INVENTORY_OFFSET_V2,
  GENERAL_SETTLEMENT_REVISION_OFFSET_V2,
  GENERAL_SETTLEMENT_TERMINAL_OFFSET_V2,
} from '../generated/generalSuccessorV5';
import {
  GENERIC_FOUNDING_ACK_BYTES_V1,
  GENERIC_FOUNDING_ACK_IDENTITIES_OFFSET_V1,
  GENERIC_FOUNDING_ACK_MAGIC_V1,
  GENERIC_FOUNDING_ACK_SCALARS_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_BYTES_V1,
  GENERIC_FOUNDING_REQUEST_ENTRY_INDEX_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_FUNDING_COUNT_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_HEAD_RESERVED_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_IDENTITIES_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_IDENTITIES_V1,
  GENERIC_FOUNDING_REQUEST_MAGIC_V1,
  GENERIC_FOUNDING_REQUEST_SCALARS_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_SCALARS_V1,
  GENERIC_FOUNDING_REQUEST_STAGE_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_TAIL_RESERVED_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_VERSION_OFFSET_V1,
  GENERIC_FOUNDING_STAGES_V1,
  GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V1,
  GENERIC_MARKET_FOUNDING_MAGIC_V1,
} from '../generated/genericFoundingV1';
import {
  ADMISSION_MAGIC_BYTES_V2,
  ADMISSION_RECEIPT_BYTES_V2,
  ADMISSION_RECEIPT_MAGIC_V2,
  ADMISSION_REQUEST_BYTES_V2,
  ADMISSION_REQUEST_MAGIC_V2,
  ADMISSION_VERSION_OFFSET_V2,
  PRODUCT_DOMAIN_DIGEST_OFFSET_V2,
  PRODUCT_ID_OFFSET_V2,
  PRODUCT_PORTFOLIO_DIGEST_OFFSET_V2,
  PRODUCT_RECORD_BYTES_V2,
  PRODUCT_RECORD_MAGIC_V2,
  PRODUCT_RECORD_RESERVED_OFFSET_V2,
  RECEIPT_COUNT_OFFSET_V2,
  RECEIPT_RECORDS_OFFSET_V2,
  RECEIPT_RESERVED_OFFSET_V2,
  RECORD_COORDINATE_BYTES_V2,
  REQUEST_DOMAIN_DIGEST_OFFSET_V2,
  REQUEST_PORTFOLIO_DIGEST_OFFSET_V2,
  REQUEST_PRODUCT_DIGEST_OFFSET_V2,
  REQUEST_RESERVED_OFFSET_V2,
} from '../generated/productRuntimeV2Admission';
import {
  PRODUCT_V2_BYTES,
  PRODUCT_V2_HEADER_BYTES,
  PRODUCT_V2_KNOTS_OFFSET,
  PRODUCT_V2_MAGIC,
  PRODUCT_V2_TERMS_OFFSET,
} from '../generated/productV2Payoff';
import {
  PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V1,
} from '../generated/protocolInfrastructure';
import {
  ASSET_BYTES_V2,
  RATIONAL_TERMINAL_HOT_ACTION_OFFSET_V3,
  RATIONAL_TERMINAL_HOT_CALLER_ROLE_OFFSET_V3,
  RATIONAL_TERMINAL_HOT_MAGIC_V3,
  RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3,
  RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3,
  RATIONAL_TERMINAL_HOT_VERSION_OFFSET_V3,
  REQUEST_ACTION_OFFSET,
  REQUEST_ACTOR_OFFSET,
  REQUEST_ASSET_COUNT_OFFSET,
  REQUEST_CALLER_ROLE_OFFSET,
  REQUEST_COLLATERAL_RECIPIENT_OFFSET,
  REQUEST_DENOMINATOR_OFFSET,
  REQUEST_DESCRIPTOR_ID_OFFSET,
  REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET,
  REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET,
  REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET,
  REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET,
  REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET,
  REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET,
  REQUEST_GENERATION_OFFSET,
  REQUEST_GRAPH_ID_OFFSET,
  REQUEST_HEADER_BYTES_V2,
  REQUEST_MAGIC_V2,
  REQUEST_MARKET_OFFSET,
  REQUEST_OUTCOME_COUNT_OFFSET,
  REQUEST_PARENT_CONTEXT_OFFSET,
  REQUEST_QUANTITY_OFFSET,
  REQUEST_REALM_OFFSET,
  REQUEST_RECEIPT_ACCOUNT_OFFSET,
  REQUEST_RECEIPT_MINT_OFFSET,
  REQUEST_RELEASE_SET_OFFSET,
  REQUEST_REPRESENTATION_AUTHORITY_OFFSET,
  REQUEST_RESERVED_HEADER_OFFSET,
  REQUEST_RESERVED_TAIL_OFFSET,
  REQUEST_SELECTED_OUTCOME_OFFSET,
  REQUEST_TOKEN_PROGRAM_OFFSET,
  REQUEST_VERSION_OFFSET,
} from '../generated/rationalTerminalHotV3';
import {
  POSITION_BASE_BYTES_V1,
  POSITION_GENERATION_OFFSET_V1,
  POSITION_MAGIC_V1,
  POSITION_MARKET_OFFSET_V1,
  POSITION_OUTCOME_BALANCE_BYTES_V1,
  POSITION_OUTCOME_COUNT_OFFSET_V1,
  POSITION_OWNER_OFFSET_V1,
  POSITION_RESERVED_OFFSET_V1,
  POSITION_SCHEMA_VERSION_OFFSET_V1,
  REALM_ADAPTER_RELEASE_ID_OFFSET_V1,
  REALM_BYTES_V1,
  REALM_COLLATERAL_MINT_OFFSET_V1,
  REALM_FREEZE_AUTHORITY_POLICY_OFFSET_V1,
  REALM_MAGIC_V1,
  REALM_MINT_AUTHORITY_POLICY_OFFSET_V1,
  REALM_RESERVED_OFFSET_V1,
  REALM_SCHEMA_VERSION_OFFSET_V1,
  REALM_TOKEN_PROGRAM_OFFSET_V1,
} from '../generated/realmPositionV1';
import {
  REGISTERED_BUYER_POSITION_BUMP_OFFSET,
  REGISTERED_BUYER_REGISTRATION_BUMP_OFFSET,
  REGISTERED_CONTROLLER_BUMP_OFFSET,
  REGISTERED_CONTROLLER_BYTES_VALUE,
  REGISTERED_CONTROLLER_EXECUTION_PRICE_OFFSET,
  REGISTERED_CONTROLLER_FILL_OFFSET,
  REGISTERED_CONTROLLER_MAGIC_BYTES,
  REGISTERED_CONTROLLER_RESERVED_OFFSET,
  REGISTERED_CONTROLLER_VERSION_OFFSET,
  REGISTERED_CREATE_BYTES_VALUE,
  REGISTERED_CREATE_CONTROLLER_BUMP_OFFSET,
  REGISTERED_CREATE_INTENT_OFFSET,
  REGISTERED_CREATE_MAGIC_BYTES,
  REGISTERED_CREATE_REGISTRATION_BUMP_OFFSET,
  REGISTERED_CREATE_REPLAY_BUMP_OFFSET,
  REGISTERED_CREATE_RESERVED_OFFSET,
  REGISTERED_CREATE_VERSION_OFFSET,
  REGISTERED_RETIRE_BYTES_VALUE,
  REGISTERED_RETIRE_CONTROLLER_BUMP_OFFSET,
  REGISTERED_RETIRE_MAGIC_BYTES,
  REGISTERED_RETIRE_REGISTRATION_BUMP_OFFSET,
  REGISTERED_RETIRE_RESERVED_OFFSET,
  REGISTERED_RETIRE_VERSION_OFFSET,
  REGISTERED_SELLER_POSITION_BUMP_OFFSET,
  REGISTERED_SELLER_REGISTRATION_BUMP_OFFSET,
  REGISTERED_STATE_BYTES_VALUE,
  REGISTERED_STATE_CONTROLLER_OFFSET,
  REGISTERED_STATE_INTENT_OFFSET,
  REGISTERED_STATE_MAGIC_BYTES,
  REGISTERED_STATE_MAKER_OFFSET,
  REGISTERED_STATE_PHASE_OFFSET,
  REGISTERED_STATE_REMAINING_OFFSET,
  REGISTERED_STATE_RESERVED_OFFSET,
  REGISTERED_STATE_SEQUENCE_OFFSET,
  REGISTERED_STATE_VERSION_OFFSET,
  REGISTERED_TERMINAL_ACTION_OFFSET,
  REGISTERED_TERMINAL_BYTES_VALUE,
  REGISTERED_TERMINAL_CANCEL,
  REGISTERED_TERMINAL_CONTROLLER_BUMP_OFFSET,
  REGISTERED_TERMINAL_EXPECTED_SEQUENCE_OFFSET,
  REGISTERED_TERMINAL_EXPIRE,
  REGISTERED_TERMINAL_MAGIC_BYTES,
  REGISTERED_TERMINAL_REGISTRATION_BUMP_OFFSET,
  REGISTERED_TERMINAL_RESERVED_OFFSET,
  REGISTERED_TERMINAL_VERSION_OFFSET,
} from '../generated/registeredDirect';

/** Width of the canonical eight-byte ASCII magic every dClutch record opens with. */
export const RECORD_MAGIC_BYTES = ADMISSION_MAGIC_BYTES_V2;

// ------------------------------------------------------------------ the shape

/**
 * How a field's bytes are read.
 *
 * `pubkey` and `identity` are both exactly 32 bytes and differ only in how they
 * are shown: a `pubkey` is an account this explorer can navigate to; an
 * `identity` is a content digest that names a record body, not an address.
 * `identity32` is the honest middle: 32 bytes whose emitted name does not settle
 * which, shown as both.
 */
export type RecordFieldKind =
  | 'u8'
  | 'u16'
  | 'u32'
  | 'u64'
  | 'pubkey'
  | 'identity'
  | 'identity32'
  | 'enum'
  | 'reserved'
  | 'span';

export type EnumTag = Readonly<{ tag: number; name: string }>;

export type RecordField = Readonly<{
  label: string;
  offset: number;
  kind: RecordFieldKind;
  /** Named tags, for `enum` fields. A byte outside them renders as unnamed. */
  tags?: ReadonlyArray<EnumTag>;
  /** What the span holds, for `span` fields. */
  note?: string;
}>;

/** How wide the record is, and how that width is known. */
export type RecordWidth =
  | Readonly<{ kind: 'fixed'; bytes: number }>
  | Readonly<{
      kind: 'header-and-rows';
      headerBytes: number;
      strideBytes: number;
      countOffset: number;
      countKind: 'u8' | 'u16' | 'u32';
      rowLabel: string;
    }>
  | Readonly<{ kind: 'header-only'; headerBytes: number; note: string }>;

export type RecordSpec = Readonly<{
  /** The magic, exactly as the generated module exports it. */
  magic: string | Uint8Array;
  name: string;
  /** The program family that writes or reads it. */
  family: string;
  summary: string;
  width: RecordWidth;
  fields: ReadonlyArray<RecordField>;
  /**
   * Why the field list is empty or partial, when it is. A record whose module
   * emitted no offsets is still identified — it just says so here rather than
   * showing fields nobody emitted.
   */
  note: string | null;
}>;

// ------------------------------------------------------------- the render map

const CORE_PHASES: ReadonlyArray<EnumTag> = Object.freeze([
  Object.freeze({ tag: CORE_PHASE_FOUNDING_TAG, name: 'Founding' }),
  Object.freeze({ tag: CORE_PHASE_OPEN_TAG, name: 'Open' }),
  Object.freeze({ tag: CORE_PHASE_TERMINAL_TAG, name: 'Terminal' }),
  Object.freeze({ tag: CORE_PHASE_RETIRING_TAG, name: 'Retiring' }),
  Object.freeze({ tag: CORE_PHASE_RETIRED_TAG, name: 'Retired' }),
]);

const CORE_READINESS: ReadonlyArray<EnumTag> = Object.freeze([
  Object.freeze({ tag: CORE_READINESS_PREPAID_TAG, name: 'Prepaid' }),
  Object.freeze({ tag: CORE_READINESS_READY_TAG, name: 'Ready' }),
  Object.freeze({ tag: CORE_READINESS_CONSUMED_TAG, name: 'Consumed' }),
]);

const GENERAL_PHASES: ReadonlyArray<EnumTag> = Object.freeze([
  Object.freeze({ tag: GENERAL_PHASE_COLLECTING_V2, name: 'Collecting' }),
  Object.freeze({ tag: GENERAL_PHASE_MATERIALIZING_V2, name: 'Materializing' }),
  Object.freeze({ tag: GENERAL_PHASE_DISTRIBUTING_V2, name: 'Distributing' }),
  Object.freeze({ tag: GENERAL_PHASE_READY_TO_CLOSE_V2, name: 'ReadyToClose' }),
  Object.freeze({ tag: GENERAL_PHASE_TERMINAL_V2, name: 'Terminal' }),
]);

const GENERAL_ACTIONS: ReadonlyArray<EnumTag> = Object.freeze([
  Object.freeze({ tag: ACTION_CONSIDER_V2, name: 'Consider' }),
  Object.freeze({ tag: ACTION_FREEZE_V2, name: 'Freeze' }),
  Object.freeze({ tag: ACTION_INITIALIZE_SETTLEMENT_V2, name: 'InitializeSettlement' }),
  Object.freeze({ tag: ACTION_COLLECT_V2, name: 'Collect' }),
  Object.freeze({ tag: ACTION_MATERIALIZE_V2, name: 'Materialize' }),
  Object.freeze({ tag: ACTION_DISTRIBUTE_V2, name: 'Distribute' }),
  Object.freeze({ tag: ACTION_CLOSE_V2, name: 'Close' }),
]);

function field(label: string, offset: number, kind: RecordFieldKind, extra?: Partial<RecordField>): RecordField {
  return Object.freeze({ label, offset, kind, ...extra });
}

/** The version field every canonical record carries at the same offset. */
function version(offset: number): RecordField {
  return field('Schema version', offset, 'u16');
}

/**
 * Fields generated from an emitted name list. `genericFoundingV1.ts` exports
 * the request's ten identities and seven scalars in their exact encoded order,
 * which is the only place in `lib/generated/` where the FIELD NAMES themselves
 * are emitted — so this record is labelled entirely from the schema.
 */
function namedRun(
  names: ReadonlyArray<string>,
  base: number,
  stride: number,
  kind: RecordFieldKind,
): ReadonlyArray<RecordField> {
  return names.map((name, index) => field(name, base + index * stride, kind));
}

const IDENTITY_BYTES = 32;
const SCALAR_BYTES = 8;

/**
 * Every record the explorer renders.
 *
 * The gate in `lib/explorerCoverage.test.ts` reads the `magic:` constant names
 * out of this table and joins them against what `lib/generated/` declares. A
 * new magic in a generated module fails that gate until it appears here.
 */
const RECORD_RENDERERS: ReadonlyArray<RecordSpec> = Object.freeze([
  // ---------------------------------------------------------------- Core / Realm
  {
    magic: CORE_STATE_MAGIC,
    name: 'Market Core state',
    family: 'Core',
    summary: 'The Market itself: its phase, its nine identity seeds, and its terminal settlement.',
    width: { kind: 'fixed', bytes: CORE_STATE_BYTES },
    fields: [
      version(CORE_STATE_VERSION_OFFSET),
      field('Phase', CORE_STATE_PHASE_OFFSET, 'enum', { tags: CORE_PHASES }),
      field('Opening readiness', CORE_STATE_READINESS_OFFSET, 'enum', { tags: CORE_READINESS }),
      field('Terminal winner', CORE_STATE_TERMINAL_WINNER_OFFSET, 'u32'),
      field('Market ID', CORE_STATE_MARKET_ID_OFFSET, 'pubkey'),
      field('Realm identity', CORE_STATE_IDENTITY_REALM_OFFSET, 'identity'),
      field('Product record identity', CORE_STATE_PRODUCT_RECORD_OFFSET, 'identity'),
      field('Product instance identity', CORE_STATE_PRODUCT_ID_OFFSET, 'identity'),
      field('Resolution policy identity', CORE_STATE_RESOLUTION_POLICY_OFFSET, 'identity'),
      field('Capability manifest identity', CORE_STATE_CAPABILITY_MANIFEST_OFFSET, 'identity'),
      field('Selected release set', CORE_STATE_SELECTED_RELEASE_SET_OFFSET, 'identity'),
      field('Registry program', CORE_STATE_REGISTRY_PROGRAM_OFFSET, 'pubkey'),
      field('Generation', CORE_STATE_GENERATION_OFFSET, 'u64'),
      field('Outstanding capabilities', CORE_STATE_OUTSTANDING_CAPABILITIES_OFFSET, 'u64'),
      field('Rent beneficiary', CORE_STATE_RENT_BENEFICIARY_OFFSET, 'pubkey'),
      field('Terminal receipt identity', CORE_STATE_TERMINAL_RECEIPT_OFFSET, 'identity'),
    ],
    note: null,
  },
  {
    magic: CORE_REQUEST_MAGIC,
    name: 'Core request',
    family: 'Core',
    summary: 'The Core instruction request body. Instruction data, not an account.',
    width: { kind: 'fixed', bytes: CORE_REQUEST_BYTES },
    fields: [],
    note: 'coreFound.ts emits this magic and the exact request width, and no field offsets. The bytes are shown raw rather than split into fields nobody emitted.',
  },
  {
    magic: LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2,
    name: 'Lifecycle rent instruction',
    family: 'Rent',
    summary: 'The Rent program instruction that creates, sweeps, or closes a Market RentCredit.',
    width: { kind: 'fixed', bytes: CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2 },
    fields: [
      field('Action', LIFECYCLE_RENT_INSTRUCTION_ACTION_OFFSET_V2, 'enum', {
        tags: [{ tag: LIFECYCLE_RENT_ACTION_CREATE_V2, name: 'Create' }],
      }),
    ],
    note: 'coreFound.ts emits the action offset, the Create tag, and the width of the CREATE request. Sweep and Close share this magic and are neither sized nor tagged there, so an action byte outside Create renders as unnamed rather than guessed, and the width check will disagree for them.',
  },
  {
    magic: LIFECYCLE_RENT_CREDIT_MAGIC_V2,
    name: 'Lifecycle RentCredit',
    family: 'Rent',
    summary: 'A Market’s prepaid rent, held against a named refund wallet for one generation.',
    width: { kind: 'fixed', bytes: LIFECYCLE_RENT_CREDIT_BYTES_V2 },
    fields: [],
    note: 'coreFound.ts emits this magic, its schema version and its exact width, and no field offsets. lib/decoders.ts still states this record’s layout by hand — it is in the abi-coverage baseline — and this view will not restate it a third time; the bytes are shown raw until the layout is emitted.',
  },
  {
    magic: REALM_MAGIC_V1,
    name: 'Realm',
    family: 'Core',
    summary: 'The collateral binding: which token program and mint back every claim in a Market.',
    width: { kind: 'fixed', bytes: REALM_BYTES_V1 },
    fields: [
      version(REALM_SCHEMA_VERSION_OFFSET_V1),
      field('Mint authority policy', REALM_MINT_AUTHORITY_POLICY_OFFSET_V1, 'enum', {
        tags: [
          { tag: 0, name: 'Require absent' },
          { tag: 1, name: 'Admit issuer control' },
        ],
      }),
      field('Freeze authority policy', REALM_FREEZE_AUTHORITY_POLICY_OFFSET_V1, 'enum', {
        tags: [
          { tag: 0, name: 'Require absent' },
          { tag: 1, name: 'Admit issuer control' },
        ],
      }),
      field('Reserved', REALM_RESERVED_OFFSET_V1, 'reserved'),
      field('Token program', REALM_TOKEN_PROGRAM_OFFSET_V1, 'pubkey'),
      field('Collateral mint', REALM_COLLATERAL_MINT_OFFSET_V1, 'pubkey'),
      field('Adapter release identity', REALM_ADAPTER_RELEASE_ID_OFFSET_V1, 'identity'),
    ],
    note: null,
  },
  {
    magic: POSITION_MAGIC_V1,
    name: 'Position',
    family: 'Direct',
    summary: "The Direct family's per-owner outcome balances. Not the Claims Position.",
    width: {
      kind: 'header-and-rows',
      headerBytes: POSITION_BASE_BYTES_V1,
      strideBytes: POSITION_OUTCOME_BALANCE_BYTES_V1,
      countOffset: POSITION_OUTCOME_COUNT_OFFSET_V1,
      countKind: 'u8',
      rowLabel: 'outcome balance',
    },
    fields: [
      version(POSITION_SCHEMA_VERSION_OFFSET_V1),
      field('Outcome count', POSITION_OUTCOME_COUNT_OFFSET_V1, 'u8'),
      field('Reserved', POSITION_RESERVED_OFFSET_V1, 'reserved'),
      field('Market', POSITION_MARKET_OFFSET_V1, 'pubkey'),
      field('Owner', POSITION_OWNER_OFFSET_V1, 'pubkey'),
      field('Generation', POSITION_GENERATION_OFFSET_V1, 'u64'),
    ],
    note: null,
  },
  {
    magic: PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V1,
    name: 'Protocol infrastructure profile',
    family: 'Release',
    summary: 'The Registry and Rent programs the protocol runs on, with the artifacts that authenticate them.',
    width: { kind: 'fixed', bytes: PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1 },
    fields: [
      version(PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V1),
      field('Artifact profile', PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V1, 'u16'),
      field('Reserved', PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V1, 'reserved'),
      field('Registry program', PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V1, 'pubkey'),
      field('Registry artifact', PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V1, 'identity'),
      field('Rent program', PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V1, 'pubkey'),
      field('Rent artifact', PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V1, 'identity'),
    ],
    note: null,
  },

  // -------------------------------------------------------------------- Claims
  {
    magic: LIABILITY_BASIS_MARKET_MAGIC_V2,
    name: 'Claims aggregate',
    family: 'Claims',
    summary: "The Market's total claim supply and the Custody namespace its Hoard lives in.",
    width: {
      kind: 'header-and-rows',
      headerBytes: LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
      strideBytes: SCALAR_BYTES,
      countOffset: LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET,
      countKind: 'u32',
      rowLabel: 'claim supply',
    },
    fields: [
      field('Claim count', LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET, 'u32'),
      field('Revision', LIABILITY_BASIS_MARKET_REVISION_OFFSET, 'u64'),
      field('Logical Market', LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET, 'pubkey'),
      field('Selected release set', LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET, 'identity'),
      field('Registry program', LIABILITY_BASIS_MARKET_REGISTRY_OFFSET, 'pubkey'),
      field('Product instance identity', LIABILITY_BASIS_MARKET_PRODUCT_OFFSET, 'identity'),
      field('Liability basis identity', LIABILITY_BASIS_MARKET_BASIS_OFFSET, 'identity'),
      field('Realm identity', LIABILITY_BASIS_MARKET_REALM_OFFSET, 'identity'),
      field('Custody context', LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET, 'identity'),
      field('Generation', LIABILITY_BASIS_MARKET_GENERATION_OFFSET, 'u64'),
    ],
    note: null,
  },
  {
    magic: LIABILITY_BASIS_POSITION_MAGIC_V2,
    name: 'Claims position',
    family: 'Claims',
    summary: "One owner's per-outcome claim balances against a Claims aggregate.",
    width: {
      kind: 'header-and-rows',
      headerBytes: LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
      strideBytes: SCALAR_BYTES,
      countOffset: LIABILITY_BASIS_POSITION_CLAIM_COUNT_OFFSET,
      countKind: 'u32',
      rowLabel: 'claim balance',
    },
    fields: [
      field('Claim count', LIABILITY_BASIS_POSITION_CLAIM_COUNT_OFFSET, 'u32'),
      field('Revision', LIABILITY_BASIS_POSITION_REVISION_OFFSET, 'u64'),
      field('Claims aggregate', LIABILITY_BASIS_POSITION_MARKET_OFFSET, 'pubkey'),
      field('Owner', LIABILITY_BASIS_POSITION_OWNER_OFFSET, 'pubkey'),
      field('Liability basis identity', LIABILITY_BASIS_POSITION_BASIS_OFFSET, 'identity'),
      field('Reserved', LIABILITY_BASIS_POSITION_RESERVED_OFFSET, 'reserved'),
    ],
    note: null,
  },

  // ---------------------------------------------------------------- Capability
  {
    magic: CAPABILITY_MANIFEST_MAGIC_V1,
    name: 'Capability manifest',
    family: 'Capability',
    summary: 'The immutable list of capabilities a Market was founded with.',
    width: {
      kind: 'header-and-rows',
      headerBytes: CAPABILITY_MANIFEST_HEADER_BYTES_V1,
      strideBytes: CAPABILITY_ENTRY_BYTES_V1,
      countOffset: CAPABILITY_MANIFEST_COUNT_OFFSET_V1,
      countKind: 'u16',
      rowLabel: 'capability entry',
    },
    fields: [
      version(CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1),
      field('Artifact profile', CAPABILITY_MANIFEST_PROFILE_OFFSET_V1, 'u16'),
      field('Capability count', CAPABILITY_MANIFEST_COUNT_OFFSET_V1, 'u16'),
      field('Reserved', CAPABILITY_MANIFEST_RESERVED_OFFSET_V1, 'reserved'),
    ],
    note: 'The 528-byte entries are decoded field by field by lib/capabilityManifest.ts and rendered in the Market lens; the header alone is shown here.',
  },
  {
    magic: CAPABILITY_FUNDING_QUOTE_MAGIC_V1,
    name: 'Capability funding quote',
    family: 'Capability',
    summary: "One capability's prepaid compartments, and the Realm collateral they are denominated in.",
    width: { kind: 'fixed', bytes: CAPABILITY_FUNDING_QUOTE_BYTES_V1 },
    fields: [
      version(CAPABILITY_FUNDING_QUOTE_SCHEMA_OFFSET_V1),
      field('Collateral kind', CAPABILITY_FUNDING_QUOTE_COLLATERAL_KIND_OFFSET_V1, 'u8'),
      field('Reserved', CAPABILITY_FUNDING_QUOTE_RESERVED_OFFSET_V1, 'reserved'),
      field('Realm collateral binding', CAPABILITY_FUNDING_QUOTE_BINDING_OFFSET_V1, 'span', {
        note: 'Realm identity, collateral release, token program, mint, refund beneficiary',
      }),
      field('Compartment amounts', CAPABILITY_FUNDING_QUOTE_AMOUNTS_OFFSET_V1, 'span', {
        note: 'Rent, Creation, Work, Provider, Bounty, Liquidity, Service, then the two totals',
      }),
    ],
    note: null,
  },
  {
    magic: CAPABILITY_FUNDING_STATE_MAGIC_V1,
    name: 'Capability funding state',
    family: 'Capability',
    summary: 'What one capability has left to spend, and what it has released.',
    width: { kind: 'fixed', bytes: CAPABILITY_FUNDING_STATE_BYTES_V1 },
    fields: [
      version(CAPABILITY_FUNDING_STATE_SCHEMA_OFFSET_V1),
      field('Status', CAPABILITY_FUNDING_STATE_STATUS_OFFSET_V1, 'u8'),
      field('Reserved', CAPABILITY_FUNDING_STATE_HEADER_RESERVED_OFFSET_V1, 'reserved'),
      field('Manifest identity', CAPABILITY_FUNDING_STATE_MANIFEST_ID_OFFSET_V1, 'identity'),
      field('Manifest entry index', CAPABILITY_FUNDING_STATE_ENTRY_INDEX_OFFSET_V1, 'u16'),
      field('Reserved', CAPABILITY_FUNDING_STATE_BODY_RESERVED_OFFSET_V1, 'reserved'),
      field('Activation slot', CAPABILITY_FUNDING_STATE_ACTIVATION_SLOT_OFFSET_V1, 'u64'),
      field('Remaining compartments', CAPABILITY_FUNDING_STATE_REMAINING_OFFSET_V1, 'span'),
      field('Released compartments', CAPABILITY_FUNDING_STATE_RELEASED_OFFSET_V1, 'span'),
    ],
    note: null,
  },
  {
    magic: MARKET_OPENING_READINESS_MAGIC_V1,
    name: 'Market opening readiness',
    family: 'Capability',
    summary: 'The record Open-last consumes to prove every capability was activated.',
    width: { kind: 'fixed', bytes: MARKET_OPENING_READINESS_BYTES_V1 },
    fields: [],
    note: 'capabilityManifestV1.ts emits this magic, its schema version and its exact width, and no field offsets.',
  },
  {
    magic: CAPABILITY_ROOT_MAGIC_V1,
    name: 'Capability root',
    family: 'Capability',
    summary: "A capability's per-Market root account, carrying its execution selection.",
    width: { kind: 'header-only', headerBytes: CAPABILITY_ROOT_HEADER_BYTES_V1, note: 'the family tail follows the header and is family-defined' },
    fields: [
      version(CAPABILITY_ROOT_SCHEMA_VERSION_OFFSET),
      field('Artifact profile', CAPABILITY_ROOT_PROFILE_OFFSET, 'u16'),
      field('Reserved', CAPABILITY_ROOT_RESERVED_OFFSET, 'reserved'),
      field('Release set', CAPABILITY_ROOT_RELEASE_SET_OFFSET, 'identity'),
      field('Market', CAPABILITY_ROOT_MARKET_OFFSET, 'pubkey'),
      field('Generation', CAPABILITY_ROOT_GENERATION_OFFSET, 'u64'),
      field('Execution selection', CAPABILITY_ROOT_SELECTION_OFFSET, 'span', {
        note: 'an embedded DCLTCER1 selection record',
      }),
    ],
    note: null,
  },
  {
    magic: CAPABILITY_EXECUTION_SELECTION_MAGIC_V1,
    name: 'Capability execution selection',
    family: 'Capability',
    summary: 'Which manifest entry a capability root executes under.',
    width: { kind: 'fixed', bytes: CAPABILITY_EXECUTION_SELECTION_BYTES_V1 },
    fields: [
      version(CAPABILITY_EXECUTION_SELECTION_SCHEMA_VERSION_OFFSET),
      field('Artifact profile', CAPABILITY_EXECUTION_SELECTION_PROFILE_OFFSET, 'u16'),
      field('Manifest entry index', CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET, 'u16'),
      field('Reserved', CAPABILITY_EXECUTION_SELECTION_RESERVED_OFFSET, 'reserved'),
      field('Manifest identity', CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET, 'identity'),
      field('Capability kind', CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET, 'identity'),
      field('Release identity', CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET, 'identity'),
      field('Config identity', CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET, 'identity'),
    ],
    note: null,
  },
  {
    magic: CAPABILITY_PROGRAM_V3_MAGIC,
    name: 'Capability program descriptor V3',
    family: 'Capability',
    summary: 'The artifact that says which programs and schemas a capability executes through.',
    width: { kind: 'fixed', bytes: CAPABILITY_PROGRAM_V3_BYTES },
    fields: [
      version(CAPABILITY_PROGRAM_V3_SCHEMA_VERSION_OFFSET),
      field('Artifact profile', CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE_OFFSET, 'u16'),
      field('Transition schema version', CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION_OFFSET, 'u16'),
      field('Request profile schema version', CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION_OFFSET, 'u16'),
      field('Capability kind', CAPABILITY_PROGRAM_V3_KIND_OFFSET, 'identity'),
      field('Config schema', CAPABILITY_PROGRAM_V3_CONFIG_SCHEMA_OFFSET, 'identity'),
      field('Request schema', CAPABILITY_PROGRAM_V3_REQUEST_SCHEMA_OFFSET, 'identity'),
      field('Root schema', CAPABILITY_PROGRAM_V3_ROOT_SCHEMA_OFFSET, 'identity'),
      field('Account profile', CAPABILITY_PROGRAM_V3_ACCOUNT_PROFILE_OFFSET, 'identity'),
      field('Derivation policy', CAPABILITY_PROGRAM_V3_DERIVATION_POLICY_OFFSET, 'identity'),
      field('Capacity profile', CAPABILITY_PROGRAM_V3_CAPACITY_PROFILE_OFFSET, 'identity'),
      field('Effect program', CAPABILITY_PROGRAM_V3_EFFECT_PROGRAM_OFFSET, 'pubkey'),
      field('Request profile schema', CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_OFFSET, 'identity'),
      field('Request profile program', CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_PROGRAM_OFFSET, 'pubkey'),
      field('Transition schema', CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_OFFSET, 'identity'),
      field('Transition program', CAPABILITY_PROGRAM_V3_TRANSITION_PROGRAM_OFFSET, 'pubkey'),
      field('Root state bytes', CAPABILITY_PROGRAM_V3_ROOT_STATE_BYTES_OFFSET, 'u32'),
      field('Reserved', CAPABILITY_PROGRAM_V3_TAIL_RESERVED_OFFSET, 'reserved'),
    ],
    note: null,
  },
  {
    magic: CAPABILITY_PROGRAM_V4_MAGIC,
    name: 'Capability program descriptor V4',
    family: 'Capability',
    summary: 'The V4 descriptor: every schema and program a capability may reach, named separately.',
    width: { kind: 'fixed', bytes: CAPABILITY_PROGRAM_V4_BYTES },
    fields: [
      version(CAPABILITY_PROGRAM_V4_SCHEMA_VERSION_OFFSET),
      field('Artifact profile', CAPABILITY_PROGRAM_V4_ARTIFACT_PROFILE_OFFSET, 'u16'),
      field('Reserved', CAPABILITY_PROGRAM_V4_HEADER_RESERVED_OFFSET, 'reserved'),
      field('Capability kind', CAPABILITY_PROGRAM_V4_KIND_OFFSET, 'identity'),
      field('Config schema', CAPABILITY_PROGRAM_V4_CONFIG_SCHEMA_OFFSET, 'identity'),
      field('Request schema', CAPABILITY_PROGRAM_V4_REQUEST_SCHEMA_OFFSET, 'identity'),
      field('Root schema', CAPABILITY_PROGRAM_V4_ROOT_SCHEMA_OFFSET, 'identity'),
      field('Derivation policy', CAPABILITY_PROGRAM_V4_DERIVATION_POLICY_OFFSET, 'identity'),
      field('Capacity profile', CAPABILITY_PROGRAM_V4_CAPACITY_PROFILE_OFFSET, 'identity'),
      field('Account profile schema', CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_SCHEMA_OFFSET, 'identity'),
      field('Account profile program', CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_PROGRAM_OFFSET, 'pubkey'),
      field('Request profile schema', CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_SCHEMA_OFFSET, 'identity'),
      field('Request profile program', CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_PROGRAM_OFFSET, 'pubkey'),
      field('Lifecycle schema', CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET, 'identity'),
      field('Lifecycle program', CAPABILITY_PROGRAM_V4_LIFECYCLE_PROGRAM_OFFSET, 'pubkey'),
      field('Strategy schema', CAPABILITY_PROGRAM_V4_STRATEGY_SCHEMA_OFFSET, 'identity'),
      field('Strategy program', CAPABILITY_PROGRAM_V4_STRATEGY_PROGRAM_OFFSET, 'pubkey'),
      field('Transition schema', CAPABILITY_PROGRAM_V4_TRANSITION_SCHEMA_OFFSET, 'identity'),
      field('Transition program', CAPABILITY_PROGRAM_V4_TRANSITION_PROGRAM_OFFSET, 'pubkey'),
      field('Effect schema', CAPABILITY_PROGRAM_V4_EFFECT_SCHEMA_OFFSET, 'identity'),
      field('Effect program', CAPABILITY_PROGRAM_V4_EFFECT_PROGRAM_OFFSET, 'pubkey'),
      field('Root state bytes', CAPABILITY_PROGRAM_V4_ROOT_STATE_BYTES_OFFSET, 'u32'),
      field('Reserved', CAPABILITY_PROGRAM_V4_TAIL_RESERVED_OFFSET, 'reserved'),
    ],
    note: null,
  },
  {
    magic: CAPABILITY_PROGRAM_SET_MAGIC_V2,
    name: 'Capability program set',
    family: 'Capability',
    summary: 'The selector-indexed table of descriptor programs a capability may dispatch to.',
    width: {
      kind: 'header-and-rows',
      headerBytes: CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2,
      strideBytes: CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2,
      countOffset: CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V2,
      countKind: 'u16',
      rowLabel: 'program-set entry',
    },
    fields: [
      field('Selector offset', CAPABILITY_PROGRAM_SET_SELECTOR_OFFSET_OFFSET_V2, 'u32'),
      field('Selector width', CAPABILITY_PROGRAM_SET_SELECTOR_WIDTH_OFFSET_V2, 'u8'),
      field('Selector endianness', CAPABILITY_PROGRAM_SET_SELECTOR_ENDIAN_OFFSET_V2, 'u8'),
      field('Entry count', CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V2, 'u16'),
      field('Reserved', CAPABILITY_PROGRAM_SET_RESERVED_OFFSET_V2, 'reserved'),
    ],
    note: 'The schema version and artifact profile sit in the header before the selector offset; directInlineV3.ts emits their values but not their offsets.',
  },

  // -------------------------------------------------------------------- Direct
  {
    magic: HOT_EXECUTION_MAGIC_V3,
    name: 'Hot execution envelope',
    family: 'Trading',
    summary: 'The 128-byte envelope every family request travels inside, naming the Market and its prestate.',
    width: { kind: 'header-only', headerBytes: GENERAL_HOT_ENVELOPE_BYTES_V3, note: 'the family request follows, its length given by the envelope' },
    fields: [
      field('Request bytes', GENERAL_ENVELOPE_REQUEST_BYTES_OFFSET_V3, 'u32'),
      field('Release set', GENERAL_ENVELOPE_RELEASE_SET_OFFSET_V3, 'identity'),
      field('Market', GENERAL_ENVELOPE_MARKET_OFFSET_V3, 'pubkey'),
      field('Generation', GENERAL_ENVELOPE_GENERATION_OFFSET_V3, 'u64'),
      field('Root prestate digest', GENERAL_ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET_V3, 'identity'),
      field('Reserved', GENERAL_ENVELOPE_RESERVED_OFFSET_V3, 'reserved'),
    ],
    note: 'directInlineV3.ts declares this magic without offsets; generalSuccessorV5.ts emits the offsets for the same eight bytes and the same 128-byte envelope, and those are what is read here.',
  },
  {
    magic: DIRECT_EXECUTION_REQUEST_MAGIC_V3,
    name: 'Direct execution request',
    family: 'Direct',
    summary: "The Direct family's inline request: two signed intents and the fill they agree on.",
    width: { kind: 'header-only', headerBytes: DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3, note: 'the participant bodies follow the header' },
    fields: [field('Action selector', DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3, 'u32')],
    note: 'directInlineV3.ts emits the header width and the selector offset; the participant bodies after the header have no emitted offsets of their own.',
  },
  {
    magic: COMPACT_INTENT_MAGIC_V2,
    name: 'Compact intent',
    family: 'Direct',
    summary: 'A maker’s signed order: side, outcome, price bound, validity window and collateral.',
    width: { kind: 'fixed', bytes: COMPACT_INTENT_BYTES_V2 },
    fields: [
      field('Side', COMPACT_INTENT_SIDE_OFFSET_V2, 'u8'),
      field('Lifecycle', COMPACT_INTENT_LIFECYCLE_OFFSET_V2, 'u8'),
      field('Outcome', COMPACT_INTENT_OUTCOME_OFFSET_V2, 'u32'),
      field('Market', COMPACT_INTENT_MARKET_OFFSET_V2, 'pubkey'),
      field('Generation', COMPACT_INTENT_GENERATION_OFFSET_V2, 'u64'),
      field('Nonce', COMPACT_INTENT_NONCE_OFFSET_V2, 'u64'),
      field('Valid from', COMPACT_INTENT_VALID_FROM_OFFSET_V2, 'u64'),
      field('Valid through', COMPACT_INTENT_VALID_THROUGH_OFFSET_V2, 'u64'),
      field('Maximum fill', COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2, 'u64'),
      field('Limit price', COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2, 'u64'),
      field('Fee basis points', COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2, 'u16'),
      field('Collateral account', COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2, 'pubkey'),
    ],
    note: null,
  },
  {
    magic: DIRECT_CONFIG_MAGIC_V1,
    name: 'Direct execution config',
    family: 'Direct',
    summary: 'The price scale and fee schedule a Direct capability executes under.',
    width: { kind: 'fixed', bytes: DIRECT_EXECUTION_CONFIG_BYTES_V1 },
    fields: [
      version(DIRECT_CONFIG_VERSION_OFFSET_V1),
      field('Reserved', DIRECT_CONFIG_RESERVED_A_OFFSET_V1, 'reserved'),
      field('Price scale', DIRECT_CONFIG_PRICE_SCALE_OFFSET_V1, 'u64'),
      field('Fee basis points', DIRECT_CONFIG_FEE_BPS_OFFSET_V1, 'u16'),
      field('Reserved', DIRECT_CONFIG_RESERVED_B_OFFSET_V1, 'reserved'),
      field('Fee recipient', DIRECT_CONFIG_FEE_RECIPIENT_OFFSET_V1, 'pubkey'),
    ],
    note: null,
  },
  {
    magic: BASIS_MAGIC_V3,
    name: 'Graded basis',
    family: 'Product',
    summary: "The payoff basis a Market's claims are graded against.",
    width: { kind: 'header-only', headerBytes: BASIS_HEADER_BYTES_V3, note: 'knot and term rows follow the header' },
    fields: [field('Categorical width', BASIS_WIDTH_OFFSET_V3, 'u32')],
    note: 'directInlineV3.ts emits the header width, the knot and term strides, and exactly one field offset. The rest of the 256-byte header has no emitted offsets.',
  },
  {
    magic: REQUEST_PROFILE_MAGIC_V1,
    name: 'Request profile V1',
    family: 'Capability',
    summary: 'The declarative program that copies a request into the transition register file.',
    width: {
      kind: 'header-and-rows',
      headerBytes: REQUEST_PROFILE_HEADER_BYTES_V1,
      strideBytes: REQUEST_PROFILE_OPERATION_BYTES_V1,
      countOffset: REQUEST_PROFILE_FIXED_OPERATIONS_OFFSET,
      countKind: 'u16',
      rowLabel: 'fixed operation',
    },
    fields: [
      version(REQUEST_PROFILE_VERSION_OFFSET),
      field('Artifact profile', REQUEST_PROFILE_ARTIFACT_OFFSET, 'u16'),
      field('Fixed request bytes', REQUEST_PROFILE_FIXED_REQUEST_BYTES_OFFSET, 'u32'),
      field('Item request bytes', REQUEST_PROFILE_ITEM_REQUEST_BYTES_OFFSET, 'u32'),
      field('Fixed operations', REQUEST_PROFILE_FIXED_OPERATIONS_OFFSET, 'u16'),
      field('Item operations', REQUEST_PROFILE_ITEM_OPERATIONS_OFFSET, 'u16'),
      field('Common scalars', REQUEST_PROFILE_COMMON_SCALARS_OFFSET, 'u16'),
      field('Item scalar stride', REQUEST_PROFILE_ITEM_SCALAR_STRIDE_OFFSET, 'u16'),
      field('Common identities', REQUEST_PROFILE_COMMON_IDENTITIES_OFFSET, 'u16'),
      field('Item identity stride', REQUEST_PROFILE_ITEM_IDENTITY_STRIDE_OFFSET, 'u16'),
    ],
    note: 'The operation rows have their own emitted offsets (REQUEST_OPERATION_*); they are shown as rows here rather than expanded.',
  },
  {
    magic: REQUEST_PROFILE_V2_MAGIC,
    name: 'Request profile V2',
    family: 'Capability',
    summary: 'The V2 profile: a V1 profile plus native-signature requirements.',
    width: { kind: 'header-only', headerBytes: REQUEST_PROFILE_V2_HEADER_BYTES, note: 'an embedded V1 profile and the requirement rows follow' },
    fields: [],
    note: 'directInlineV3.ts emits the V2 header width and three offsets under BARE names (EMBEDDED_V1_BYTES_OFFSET, REQUIREMENT_COUNT_OFFSET, HEADER_RESERVED_OFFSET) that are not tied to this magic by their naming. Attributing them here would be this browser deciding what the emission did not say, so the header is shown raw.',
  },
  {
    magic: EXECUTION_STRATEGY_PROGRAM_MAGIC_V2,
    name: 'Execution strategy program',
    family: 'Capability',
    summary: 'Which transition program a capability executes, and under what disposition.',
    width: { kind: 'fixed', bytes: EXECUTION_STRATEGY_PROGRAM_BYTES_V2 },
    fields: [
      field('Disposition', STRATEGY_DISPOSITION_OFFSET_V2, 'u32'),
      field('Transition schema', STRATEGY_TRANSITION_SCHEMA_OFFSET_V2, 'identity'),
      field('Transition program', STRATEGY_TRANSITION_PROGRAM_OFFSET_V2, 'pubkey'),
    ],
    note: 'Two modules emit this magic; directInlineV3.ts emits the most offsets and is what is read here.',
  },
  {
    magic: ACCOUNT_PROFILE_MAGIC_V2,
    name: 'Account profile',
    family: 'Capability',
    summary: 'The declarative account-frame program a capability authenticates its accounts with.',
    width: { kind: 'header-only', headerBytes: ACCOUNT_PROFILE_HEADER_BYTES_V2, note: 'rule and operation rows follow, their shape given by the artifact profile' },
    fields: [],
    note: 'Both dealerEquityV3.ts and directInlineV3.ts declare this magic and neither emits a field offset keyed to it. The offsets that exist nearby belong to sub-profiles selected by an artifact-profile discriminant, not to this header.',
  },

  // -------------------------------------------------------- Registered Direct
  {
    magic: REGISTERED_STATE_MAGIC_BYTES,
    name: 'Registered intent state',
    family: 'Direct',
    summary: 'A resting order: the maker, the intent it registered, and what is left of it.',
    width: { kind: 'fixed', bytes: REGISTERED_STATE_BYTES_VALUE },
    fields: [
      version(REGISTERED_STATE_VERSION_OFFSET),
      field('Phase', REGISTERED_STATE_PHASE_OFFSET, 'u8'),
      field('Reserved', REGISTERED_STATE_RESERVED_OFFSET, 'reserved'),
      field('Controller', REGISTERED_STATE_CONTROLLER_OFFSET, 'pubkey'),
      field('Maker', REGISTERED_STATE_MAKER_OFFSET, 'pubkey'),
      field('Registered intent', REGISTERED_STATE_INTENT_OFFSET, 'span', {
        note: 'an embedded Direct intent record',
      }),
      field('Remaining', REGISTERED_STATE_REMAINING_OFFSET, 'u64'),
      field('Sequence', REGISTERED_STATE_SEQUENCE_OFFSET, 'u64'),
    ],
    note: null,
  },
  {
    magic: REGISTERED_CONTROLLER_MAGIC_BYTES,
    name: 'Registered fill instruction',
    family: 'Direct',
    summary: 'The controller instruction that fills two registered intents against each other.',
    width: { kind: 'fixed', bytes: REGISTERED_CONTROLLER_BYTES_VALUE },
    fields: [
      version(REGISTERED_CONTROLLER_VERSION_OFFSET),
      field('Controller bump', REGISTERED_CONTROLLER_BUMP_OFFSET, 'u8'),
      field('Seller registration bump', REGISTERED_SELLER_REGISTRATION_BUMP_OFFSET, 'u8'),
      field('Buyer registration bump', REGISTERED_BUYER_REGISTRATION_BUMP_OFFSET, 'u8'),
      field('Seller position bump', REGISTERED_SELLER_POSITION_BUMP_OFFSET, 'u8'),
      field('Buyer position bump', REGISTERED_BUYER_POSITION_BUMP_OFFSET, 'u8'),
      field('Reserved', REGISTERED_CONTROLLER_RESERVED_OFFSET, 'reserved'),
      field('Fill', REGISTERED_CONTROLLER_FILL_OFFSET, 'u64'),
      field('Execution price', REGISTERED_CONTROLLER_EXECUTION_PRICE_OFFSET, 'u64'),
    ],
    note: null,
  },
  {
    magic: REGISTERED_CREATE_MAGIC_BYTES,
    name: 'Registered create instruction',
    family: 'Direct',
    summary: 'The instruction that registers a signed intent as a resting order.',
    width: { kind: 'fixed', bytes: REGISTERED_CREATE_BYTES_VALUE },
    fields: [
      version(REGISTERED_CREATE_VERSION_OFFSET),
      field('Controller bump', REGISTERED_CREATE_CONTROLLER_BUMP_OFFSET, 'u8'),
      field('Replay bump', REGISTERED_CREATE_REPLAY_BUMP_OFFSET, 'u8'),
      field('Registration bump', REGISTERED_CREATE_REGISTRATION_BUMP_OFFSET, 'u8'),
      field('Reserved', REGISTERED_CREATE_RESERVED_OFFSET, 'reserved'),
      field('Intent', REGISTERED_CREATE_INTENT_OFFSET, 'span', {
        note: 'an embedded Direct intent record',
      }),
    ],
    note: null,
  },
  {
    magic: REGISTERED_TERMINAL_MAGIC_BYTES,
    name: 'Registered terminal instruction',
    family: 'Direct',
    summary: 'Cancel or expire a resting order at an exact sequence.',
    width: { kind: 'fixed', bytes: REGISTERED_TERMINAL_BYTES_VALUE },
    fields: [
      version(REGISTERED_TERMINAL_VERSION_OFFSET),
      field('Action', REGISTERED_TERMINAL_ACTION_OFFSET, 'enum', {
        tags: [
          { tag: REGISTERED_TERMINAL_CANCEL, name: 'Cancel' },
          { tag: REGISTERED_TERMINAL_EXPIRE, name: 'Expire' },
        ],
      }),
      field('Controller bump', REGISTERED_TERMINAL_CONTROLLER_BUMP_OFFSET, 'u8'),
      field('Registration bump', REGISTERED_TERMINAL_REGISTRATION_BUMP_OFFSET, 'u8'),
      field('Reserved', REGISTERED_TERMINAL_RESERVED_OFFSET, 'reserved'),
      field('Expected sequence', REGISTERED_TERMINAL_EXPECTED_SEQUENCE_OFFSET, 'u64'),
    ],
    note: null,
  },
  {
    magic: REGISTERED_RETIRE_MAGIC_BYTES,
    name: 'Registered retire instruction',
    family: 'Direct',
    summary: 'Close a spent registration and return its rent.',
    width: { kind: 'fixed', bytes: REGISTERED_RETIRE_BYTES_VALUE },
    fields: [
      version(REGISTERED_RETIRE_VERSION_OFFSET),
      field('Controller bump', REGISTERED_RETIRE_CONTROLLER_BUMP_OFFSET, 'u8'),
      field('Registration bump', REGISTERED_RETIRE_REGISTRATION_BUMP_OFFSET, 'u8'),
      field('Reserved', REGISTERED_RETIRE_RESERVED_OFFSET, 'reserved'),
    ],
    note: null,
  },

  // ------------------------------------------------------------------ General
  {
    magic: GENERAL_HOT_ACK_MAGIC_V3,
    name: 'Hot execution acknowledgement',
    family: 'Trading',
    summary: 'What the Hot route returns: the prestate it read, the poststate it wrote, and the program that ran.',
    width: { kind: 'fixed', bytes: GENERAL_HOT_ACK_BYTES_V3 },
    fields: [
      field('Release set', GENERAL_ACK_RELEASE_SET_OFFSET_V3, 'identity'),
      field('Market', GENERAL_ACK_MARKET_OFFSET_V3, 'pubkey'),
      field('Generation', GENERAL_ACK_GENERATION_OFFSET_V3, 'u64'),
      field('Root', GENERAL_ACK_ROOT_OFFSET_V3, 'pubkey'),
      field('Request digest', GENERAL_ACK_REQUEST_DIGEST_OFFSET_V3, 'identity'),
      field('Selected program', GENERAL_ACK_SELECTED_PROGRAM_OFFSET_V3, 'pubkey'),
      field('Root prestate digest', GENERAL_ACK_ROOT_PRESTATE_DIGEST_OFFSET_V3, 'identity'),
      field('Root poststate digest', GENERAL_ACK_ROOT_POSTSTATE_DIGEST_OFFSET_V3, 'identity'),
      field('Execution digest', GENERAL_ACK_EXECUTION_DIGEST_OFFSET_V3, 'identity'),
    ],
    note: null,
  },
  {
    magic: GENERAL_LOCAL_STATE_MAGIC_V3,
    name: 'General local state',
    family: 'General',
    summary: "General's per-batch account: a selection or a settlement, prepaid and beneficiary-bound.",
    width: {
      kind: 'header-only',
      headerBytes: GENERAL_LOCAL_STATE_HEADER_BYTES_V3,
      note: `the body begins at ${GENERAL_LOCAL_STATE_BODY_OFFSET_V3} and is a General selection or settlement record, chosen by the kind byte`,
    },
    fields: [
      field('Kind', GENERAL_LOCAL_STATE_KIND_OFFSET_V3, 'enum', {
        tags: [
          { tag: GENERAL_LOCAL_STATE_SELECTION_KIND_V3, name: 'Selection' },
          { tag: GENERAL_LOCAL_STATE_SETTLEMENT_KIND_V3, name: 'Settlement' },
        ],
      }),
      field('Bump', GENERAL_LOCAL_STATE_BUMP_OFFSET_V3, 'u8'),
      field('Rent principal', GENERAL_LOCAL_STATE_RENT_PRINCIPAL_OFFSET_V3, 'u64'),
      field('Beneficiary', GENERAL_LOCAL_STATE_BENEFICIARY_OFFSET_V3, 'pubkey'),
    ],
    note: null,
  },
  {
    magic: GENERAL_REQUEST_MAGIC_V2,
    name: 'General request',
    family: 'General',
    summary: "One step of General's batch lifecycle, from Consider to Close.",
    width: { kind: 'fixed', bytes: GENERAL_REQUEST_BYTES_V2 },
    fields: [
      field('Action', GENERAL_REQUEST_ACTION_OFFSET_V2, 'enum', { tags: GENERAL_ACTIONS }),
      field('Manifest order', GENERAL_REQUEST_MANIFEST_ORDER_OFFSET_V2, 'u8'),
      field('Expected revision', GENERAL_REQUEST_EXPECTED_REVISION_OFFSET_V2, 'u64'),
      field('Candidate identity', GENERAL_REQUEST_CANDIDATE_ID_OFFSET_V2, 'identity'),
      field('Page index', GENERAL_REQUEST_PAGE_INDEX_OFFSET_V2, 'u32'),
      field('Execution index', GENERAL_REQUEST_EXECUTION_INDEX_OFFSET_V2, 'u8'),
      field('State bump', GENERAL_REQUEST_STATE_BUMP_OFFSET_V2, 'u8'),
      field('Terminal bump', GENERAL_REQUEST_TERMINAL_BUMP_OFFSET_V2, 'u8'),
    ],
    note: null,
  },
  {
    magic: GENERAL_SELECTION_MAGIC_V2,
    name: 'General selection',
    family: 'General',
    summary: 'The batch auction: submitted candidates, the best one, and what it verified to.',
    width: { kind: 'fixed', bytes: GENERAL_SELECTION_BYTES_V2 },
    fields: [
      field('Phase', GENERAL_SELECTION_PHASE_OFFSET_V2, 'enum', { tags: GENERAL_PHASES }),
      field('Outcome count', GENERAL_SELECTION_OUTCOME_COUNT_OFFSET_V2, 'u32'),
      field('Revision', GENERAL_SELECTION_REVISION_OFFSET_V2, 'u64'),
      field('Submitted count', GENERAL_SELECTION_SUBMITTED_COUNT_OFFSET_V2, 'u32'),
      field('Best coordinate', GENERAL_SELECTION_BEST_COORDINATE_OFFSET_V2, 'u32'),
      field('Verified revision', GENERAL_SELECTION_VERIFIED_REVISION_OFFSET_V2, 'u64'),
      field('Price scale', GENERAL_SELECTION_PRICE_SCALE_OFFSET_V2, 'u64'),
      field('Product identity', GENERAL_SELECTION_PRODUCT_ID_OFFSET_V2, 'identity'),
      field('Batch identity', GENERAL_SELECTION_BATCH_ID_OFFSET_V2, 'identity'),
      field('Policy identity', GENERAL_SELECTION_POLICY_ID_OFFSET_V2, 'identity'),
      field('Best candidate', GENERAL_SELECTION_BEST_CANDIDATE_OFFSET_V2, 'identity'),
      field('Verified digest', GENERAL_SELECTION_VERIFIED_DIGEST_OFFSET_V2, 'identity'),
      field('Filled lots', GENERAL_SELECTION_FILLED_LOTS_OFFSET_V2, 'u64'),
      field('Quote surplus', GENERAL_SELECTION_QUOTE_SURPLUS_OFFSET_V2, 'u64'),
    ],
    note: null,
  },
  {
    magic: GENERAL_SETTLEMENT_MAGIC_V2,
    name: 'General settlement',
    family: 'General',
    summary: 'The settlement walk: order cursor, quote inventory, and per-outcome inventory.',
    width: {
      kind: 'header-and-rows',
      headerBytes: GENERAL_SETTLEMENT_HEADER_BYTES_V2,
      strideBytes: GENERAL_SETTLEMENT_INVENTORY_STRIDE_V2,
      countOffset: GENERAL_SETTLEMENT_OUTCOME_COUNT_OFFSET_V2,
      countKind: 'u32',
      rowLabel: 'outcome inventory',
    },
    fields: [
      field('Phase', GENERAL_SETTLEMENT_PHASE_OFFSET_V2, 'enum', { tags: GENERAL_PHASES }),
      field('Outcome count', GENERAL_SETTLEMENT_OUTCOME_COUNT_OFFSET_V2, 'u32'),
      field('Order count', GENERAL_SETTLEMENT_ORDER_COUNT_OFFSET_V2, 'u32'),
      field('Next order', GENERAL_SETTLEMENT_NEXT_ORDER_OFFSET_V2, 'u32'),
      field('Revision', GENERAL_SETTLEMENT_REVISION_OFFSET_V2, 'u64'),
      field('Candidate identity', GENERAL_SETTLEMENT_CANDIDATE_ID_OFFSET_V2, 'identity'),
      field('Quote inventory', GENERAL_SETTLEMENT_QUOTE_INVENTORY_OFFSET_V2, 'u64'),
      field('Complete sets', GENERAL_SETTLEMENT_COMPLETE_SET_OFFSET_V2, 'u64'),
      field('Terminal', GENERAL_SETTLEMENT_TERMINAL_OFFSET_V2, 'u64'),
    ],
    note: `The per-outcome inventory rows begin at ${GENERAL_SETTLEMENT_INVENTORY_OFFSET_V2}, one u64 each, and are shown as rows.`,
  },

  // ------------------------------------------------------------------- Dealer
  {
    magic: DEALER_CONFIG_MAGIC_V4,
    name: 'Dealer immutable config',
    family: 'Dealer',
    summary: "The Dealer pool's binding: release set, Realm, position owner, capital floor.",
    width: { kind: 'fixed', bytes: DEALER_CONFIG_BYTES_V4 },
    fields: [
      field('Release set', DEALER_CONFIG_RELEASE_SET_OFFSET_V4, 'identity'),
      field('Realm identity', DEALER_CONFIG_REALM_OFFSET_V4, 'identity'),
      field('Position owner', DEALER_CONFIG_POSITION_OWNER_OFFSET_V4, 'pubkey'),
      field('Locked capital floor', DEALER_CONFIG_LOCKED_CAPITAL_FLOOR_OFFSET_V4, 'u64'),
    ],
    note: 'dealerEquityV3.ts emits the config version but no offset for it, so the header before the release set is shown raw.',
  },
  {
    magic: DEALER_EQUITY_REQUEST_MAGIC_V3,
    name: 'Dealer equity request',
    family: 'Dealer',
    summary: 'Contribute to or redeem from a Dealer pool, in one of three phases.',
    width: { kind: 'header-only', headerBytes: DEALER_EQUITY_HEADER_BYTES_V3, note: 'a Claims packet of the declared width follows the header' },
    fields: [
      field('Selector', DEALER_EQUITY_SELECTOR_OFFSET_V3, 'u8'),
      field('Claims packet bytes', DEALER_EQUITY_CLAIMS_PACKET_BYTES_OFFSET_V3, 'u64'),
    ],
    note: 'dealerEquityV3.ts emits two of this record’s field offsets and its six selector values; the rest of the 480-byte header has no emitted offsets.',
  },
  {
    magic: DEALER_LP_POSITION_MAGIC_V3,
    name: 'Dealer LP position',
    family: 'Dealer',
    summary: "One liquidity provider's share of a Dealer pool.",
    width: { kind: 'fixed', bytes: DEALER_LP_POSITION_BYTES_V3 },
    fields: [],
    note: 'dealerEquityV3.ts emits this magic, its version and its exact width, and no field offsets.',
  },
  {
    magic: DEALER_OBLIGATION_MAGIC_V3,
    name: 'Dealer obligation',
    family: 'Dealer',
    summary: "The pool's outstanding obligation against the Market.",
    width: { kind: 'header-only', headerBytes: DEALER_OBLIGATION_HEADER_BYTES_V3, note: 'a per-position tail follows the header' },
    fields: [],
    note: 'dealerEquityV3.ts emits this magic, its version and its header width, and no field offsets.',
  },
  {
    magic: SIGNED_DELTA_PLAN_MAGIC_V3,
    name: 'Signed delta plan',
    family: 'Dealer',
    summary: 'A batch of signed position deltas the Claims program applies atomically.',
    width: { kind: 'header-only', headerBytes: SIGNED_DELTA_PLAN_HEADER_BYTES_V3, note: 'position and delta rows follow the header' },
    fields: [],
    note: 'dealerEquityV3.ts emits this magic, its wire version, its header width and its three row strides, and no field offsets.',
  },

  // ----------------------------------------------------------------- Rational
  {
    magic: REQUEST_MAGIC_V2,
    name: 'Rational representation request',
    family: 'Claims',
    summary: 'Issue, unwrap, denominate or redeem a rational representation of a claim.',
    width: {
      kind: 'header-and-rows',
      headerBytes: REQUEST_HEADER_BYTES_V2,
      strideBytes: ASSET_BYTES_V2,
      countOffset: REQUEST_ASSET_COUNT_OFFSET,
      countKind: 'u32',
      rowLabel: 'asset',
    },
    fields: [
      version(REQUEST_VERSION_OFFSET),
      field('Action', REQUEST_ACTION_OFFSET, 'u8'),
      field('Caller role', REQUEST_CALLER_ROLE_OFFSET, 'u8'),
      field('Reserved', REQUEST_RESERVED_HEADER_OFFSET, 'reserved'),
      field('Release set', REQUEST_RELEASE_SET_OFFSET, 'identity'),
      field('Market', REQUEST_MARKET_OFFSET, 'pubkey'),
      field('Graph identity', REQUEST_GRAPH_ID_OFFSET, 'identity'),
      field('Descriptor identity', REQUEST_DESCRIPTOR_ID_OFFSET, 'identity'),
      field('Parent context', REQUEST_PARENT_CONTEXT_OFFSET, 'identity'),
      field('Actor', REQUEST_ACTOR_OFFSET, 'pubkey'),
      field('Receipt mint', REQUEST_RECEIPT_MINT_OFFSET, 'pubkey'),
      field('Receipt account', REQUEST_RECEIPT_ACCOUNT_OFFSET, 'pubkey'),
      field('Representation authority', REQUEST_REPRESENTATION_AUTHORITY_OFFSET, 'pubkey'),
      field('Token program', REQUEST_TOKEN_PROGRAM_OFFSET, 'pubkey'),
      field('Realm identity', REQUEST_REALM_OFFSET, 'identity'),
      field('Collateral recipient', REQUEST_COLLATERAL_RECIPIENT_OFFSET, 'pubkey'),
      field('Expected representation revision', REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET, 'u64'),
      field('Expected Claims market revision', REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET, 'u64'),
      field('Expected actor position revision', REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET, 'u64'),
      field('Expected custody position revision', REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET, 'u64'),
      field('Expected custody replay revision', REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET, 'u64'),
      field('Generation', REQUEST_GENERATION_OFFSET, 'u64'),
      field('Quantity', REQUEST_QUANTITY_OFFSET, 'u64'),
      field('Denominator', REQUEST_DENOMINATOR_OFFSET, 'u64'),
      field('Expected receipt supply', REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET, 'u64'),
      field('Outcome count', REQUEST_OUTCOME_COUNT_OFFSET, 'u32'),
      field('Selected outcome', REQUEST_SELECTED_OUTCOME_OFFSET, 'u32'),
      field('Asset count', REQUEST_ASSET_COUNT_OFFSET, 'u32'),
      field('Reserved', REQUEST_RESERVED_TAIL_OFFSET, 'reserved'),
    ],
    note: null,
  },
  {
    magic: RATIONAL_TERMINAL_HOT_MAGIC_V3,
    name: 'Rational terminal Hot request',
    family: 'Claims',
    summary: 'Terminal redemption of a rational representation, at exactly one asset.',
    width: { kind: 'fixed', bytes: RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3 },
    fields: [
      version(RATIONAL_TERMINAL_HOT_VERSION_OFFSET_V3),
      field('Action', RATIONAL_TERMINAL_HOT_ACTION_OFFSET_V3, 'u8'),
      field('Caller role', RATIONAL_TERMINAL_HOT_CALLER_ROLE_OFFSET_V3, 'u8'),
      field('Parent context', RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3, 'identity'),
    ],
    note: 'rationalTerminalHotV3.ts restates only five offsets under the V3 name. The record is the 488-byte V2 header plus one asset, so the remaining fields are those of the Rational representation request above; they are not restated here because the emission does not restate them.',
  },

  // ---------------------------------------------------------------- Product V2
  {
    magic: PRODUCT_V2_MAGIC,
    name: 'Product V2 payoff',
    family: 'Product',
    summary: 'The piecewise payoff a Market grades claims against: knots and terms.',
    width: { kind: 'fixed', bytes: PRODUCT_V2_BYTES },
    fields: [
      field('Knots', PRODUCT_V2_KNOTS_OFFSET, 'span', { note: 'up to 16 knots, 16 bytes each' }),
      field('Terms', PRODUCT_V2_TERMS_OFFSET, 'span', { note: 'up to 16 terms, 16 bytes each' }),
    ],
    note: `productV2Payoff.ts emits the two array offsets and the ${PRODUCT_V2_HEADER_BYTES}-byte header width, and no header field offsets. The header is shown raw.`,
  },
  {
    magic: ADMISSION_REQUEST_MAGIC_V2,
    name: 'Product admission request',
    family: 'Product',
    summary: 'Ask the Product runtime to admit a product, its result domain and its portfolio.',
    width: { kind: 'fixed', bytes: ADMISSION_REQUEST_BYTES_V2 },
    fields: [
      version(ADMISSION_VERSION_OFFSET_V2),
      field('Reserved', REQUEST_RESERVED_OFFSET_V2, 'reserved'),
      field('Product digest', REQUEST_PRODUCT_DIGEST_OFFSET_V2, 'identity'),
      field('Result domain digest', REQUEST_DOMAIN_DIGEST_OFFSET_V2, 'identity'),
      field('Portfolio digest', REQUEST_PORTFOLIO_DIGEST_OFFSET_V2, 'identity'),
    ],
    note: null,
  },
  {
    magic: PRODUCT_RECORD_MAGIC_V2,
    name: 'Product record',
    family: 'Product',
    summary: 'The admitted product: its instance identity and the two digests it was admitted against.',
    width: { kind: 'fixed', bytes: PRODUCT_RECORD_BYTES_V2 },
    fields: [
      version(ADMISSION_VERSION_OFFSET_V2),
      field('Reserved', PRODUCT_RECORD_RESERVED_OFFSET_V2, 'reserved'),
      field('Product instance identity', PRODUCT_ID_OFFSET_V2, 'identity'),
      field('Result domain digest', PRODUCT_DOMAIN_DIGEST_OFFSET_V2, 'identity'),
      field('Portfolio digest', PRODUCT_PORTFOLIO_DIGEST_OFFSET_V2, 'identity'),
    ],
    note: 'Structurally identical to the admission request; only the magic distinguishes them.',
  },
  {
    magic: ADMISSION_RECEIPT_MAGIC_V2,
    name: 'Product admission receipt',
    family: 'Product',
    summary: 'Proof that three records were admitted together, with their coordinates.',
    width: {
      kind: 'header-and-rows',
      headerBytes: RECEIPT_RECORDS_OFFSET_V2,
      strideBytes: RECORD_COORDINATE_BYTES_V2,
      countOffset: RECEIPT_COUNT_OFFSET_V2,
      countKind: 'u8',
      rowLabel: 'record coordinate',
    },
    fields: [
      version(ADMISSION_VERSION_OFFSET_V2),
      field('Record count', RECEIPT_COUNT_OFFSET_V2, 'u8'),
      field('Reserved', RECEIPT_RESERVED_OFFSET_V2, 'reserved'),
    ],
    note: `The receipt is exactly ${ADMISSION_RECEIPT_BYTES_V2} bytes at the canonical record count.`,
  },

  // --------------------------------------------------------- Generic founding
  {
    magic: GENERIC_MARKET_FOUNDING_MAGIC_V1,
    name: 'Generic market founding',
    family: 'Trading',
    summary: 'The whole-founding outer instruction. Its data is the magic and nothing else.',
    width: { kind: 'fixed', bytes: GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V1 },
    fields: [],
    note: 'genericFoundingV1.ts says it plainly: "the outer carries no other bytes". There is nothing after the magic to decode; the whole request travels in four readonly accounts.',
  },
  {
    magic: GENERIC_FOUNDING_REQUEST_MAGIC_V1,
    name: 'Generic founding request',
    family: 'Trading',
    summary: 'The founding request body: ten identities, seven scalars, one stage.',
    width: { kind: 'fixed', bytes: GENERIC_FOUNDING_REQUEST_BYTES_V1 },
    fields: [
      version(GENERIC_FOUNDING_REQUEST_VERSION_OFFSET_V1),
      field('Stage', GENERIC_FOUNDING_REQUEST_STAGE_OFFSET_V1, 'enum', {
        tags: GENERIC_FOUNDING_STAGES_V1.map((stage) => ({ tag: stage.tag, name: stage.name })),
      }),
      field('Funding state count', GENERIC_FOUNDING_REQUEST_FUNDING_COUNT_OFFSET_V1, 'u8'),
      field('Reserved', GENERIC_FOUNDING_REQUEST_HEAD_RESERVED_OFFSET_V1, 'reserved'),
      ...namedRun(
        GENERIC_FOUNDING_REQUEST_IDENTITIES_V1,
        GENERIC_FOUNDING_REQUEST_IDENTITIES_OFFSET_V1,
        IDENTITY_BYTES,
        'identity32',
      ),
      ...namedRun(
        GENERIC_FOUNDING_REQUEST_SCALARS_V1,
        GENERIC_FOUNDING_REQUEST_SCALARS_OFFSET_V1,
        SCALAR_BYTES,
        'u64',
      ),
      field('Capability entry index', GENERIC_FOUNDING_REQUEST_ENTRY_INDEX_OFFSET_V1, 'u16'),
      field('Reserved', GENERIC_FOUNDING_REQUEST_TAIL_RESERVED_OFFSET_V1, 'reserved'),
    ],
    note: 'The seventeen field names come from GENERIC_FOUNDING_REQUEST_IDENTITIES_V1 and _SCALARS_V1 — the one record in lib/generated/ whose field NAMES are emitted, not just its offsets.',
  },
  {
    magic: GENERIC_FOUNDING_ACK_MAGIC_V1,
    name: 'Generic founding acknowledgement',
    family: 'Trading',
    summary: 'What the founding route returns to its caller.',
    width: { kind: 'fixed', bytes: GENERIC_FOUNDING_ACK_BYTES_V1 },
    fields: [
      field('Identities', GENERIC_FOUNDING_ACK_IDENTITIES_OFFSET_V1, 'span'),
      field('Scalars', GENERIC_FOUNDING_ACK_SCALARS_OFFSET_V1, 'span'),
    ],
    note: 'genericFoundingV1.ts emits the two region offsets and the width, and no NAME list for either region — the request’s ten identity names do not fit the acknowledgement’s seven slots. The regions are shown, the fields inside them are not named.',
  },
]);

// -------------------------------------------------------------------- decoding

/** The eight-byte magic as text, whichever form the generated module used. */
export function magicText(magic: string | Uint8Array): string {
  if (typeof magic === 'string') return magic;
  return String.fromCharCode(...magic);
}

const BY_MAGIC: ReadonlyMap<string, RecordSpec> = new Map(
  RECORD_RENDERERS.map((spec) => [magicText(spec.magic), spec]),
);

/** Every rendered record, for the coverage table and the schema browser. */
export function renderedRecords(): ReadonlyArray<RecordSpec> {
  return RECORD_RENDERERS;
}

/** The magic at the head of `data`, as text, when the leading bytes are printable. */
export function leadingMagic(data: Uint8Array): string | null {
  if (data.length < RECORD_MAGIC_BYTES) return null;
  let text = '';
  for (let index = 0; index < RECORD_MAGIC_BYTES; index += 1) {
    const byte = data[index];
    if (byte < 0x20 || byte > 0x7e) return null;
    text += String.fromCharCode(byte);
  }
  return text;
}

/** The spec for an account's leading magic, or `null` when none is rendered. */
export function specForData(data: Uint8Array): RecordSpec | null {
  const magic = leadingMagic(data);
  return magic === null ? null : (BY_MAGIC.get(magic) ?? null);
}

/** The spec for a magic given as text. */
export function specForMagic(magic: string): RecordSpec | null {
  return BY_MAGIC.get(magic) ?? null;
}

export type DecodedFieldValue =
  | Readonly<{ form: 'scalar'; text: string }>
  | Readonly<{ form: 'address'; base58: string }>
  | Readonly<{ form: 'identity'; hex: string }>
  | Readonly<{ form: 'both'; base58: string | null; hex: string }>
  | Readonly<{ form: 'enum'; tag: number; name: string | null }>
  | Readonly<{ form: 'reserved'; zero: boolean; hex: string }>
  | Readonly<{ form: 'span'; bytes: number; hex: string; note: string | null }>
  | Readonly<{ form: 'refused'; reason: string }>;

export type DecodedField = Readonly<{
  label: string;
  offset: number;
  bytes: number;
  kind: RecordFieldKind;
  value: DecodedFieldValue;
}>;

export type DecodedRows = Readonly<{
  label: string;
  count: number;
  strideBytes: number;
  offset: number;
  /** Rows are shown as u64 scalars when the stride is exactly eight bytes. */
  scalars: ReadonlyArray<string> | null;
}>;

export type DecodedRecord = Readonly<{
  spec: RecordSpec;
  magic: string;
  accountBytes: number;
  /** Whether the observed width matches what the schema says it should be. */
  widthCheck: Readonly<{ ok: boolean; expected: string; observed: number }>;
  fields: ReadonlyArray<DecodedField>;
  rows: DecodedRows | null;
}>;

const HEX_HEAD_BYTES = 16;

function hexOf(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

function readUnsigned(data: Uint8Array, offset: number, width: number): bigint {
  let value = 0n;
  for (let index = width - 1; index >= 0; index -= 1) {
    value = (value << 8n) | BigInt(data[offset + index]);
  }
  return value;
}

const SCALAR_WIDTHS: Readonly<Record<'u8' | 'u16' | 'u32' | 'u64', number>> = Object.freeze({
  u8: 1,
  u16: 2,
  u32: 4,
  u64: 8,
});

/**
 * How wide a field is.
 *
 * A scalar's width is its kind. A 32-byte identity's width is 32. Everything
 * else — reserved runs and spans — is measured to the next declared offset, or
 * to the end of the header, because that is the only thing the emission
 * actually determines.
 */
function fieldWidth(spec: RecordSpec, index: number, headerEnd: number): number {
  const declared = spec.fields[index];
  switch (declared.kind) {
    case 'u8':
    case 'u16':
    case 'u32':
    case 'u64':
      return SCALAR_WIDTHS[declared.kind];
    case 'enum':
      return 1;
    case 'pubkey':
    case 'identity':
    case 'identity32':
      return IDENTITY_BYTES;
    case 'reserved':
    case 'span': {
      const next = spec.fields
        .map((entry) => entry.offset)
        .filter((offset) => offset > declared.offset)
        .sort((left, right) => left - right)[0];
      return (next ?? headerEnd) - declared.offset;
    }
  }
}

function headerEndOf(width: RecordWidth): number {
  switch (width.kind) {
    case 'fixed':
      return width.bytes;
    case 'header-and-rows':
      return width.headerBytes;
    case 'header-only':
      return width.headerBytes;
  }
}

function base58OrNull(bytes: Uint8Array): string | null {
  try {
    return new PublicKey(bytes).toBase58();
  } catch {
    return null;
  }
}

function decodeValue(field_: RecordField, data: Uint8Array, width: number): DecodedFieldValue {
  const end = field_.offset + width;
  if (end > data.length) {
    return Object.freeze({
      form: 'refused',
      reason: `bytes ${field_.offset}..${end} lie past the ${data.length}-byte account`,
    });
  }
  const slice = data.slice(field_.offset, end);
  switch (field_.kind) {
    case 'u8':
    case 'u16':
    case 'u32':
    case 'u64':
      return Object.freeze({ form: 'scalar', text: readUnsigned(data, field_.offset, width).toString() });
    case 'enum': {
      const tag = data[field_.offset];
      const named = field_.tags?.find((entry) => entry.tag === tag) ?? null;
      return Object.freeze({ form: 'enum', tag, name: named?.name ?? null });
    }
    case 'pubkey': {
      const base58 = base58OrNull(slice);
      return base58 === null
        ? Object.freeze({ form: 'refused', reason: 'not a valid 32-byte public key' })
        : Object.freeze({ form: 'address', base58 });
    }
    case 'identity':
      return Object.freeze({ form: 'identity', hex: hexOf(slice) });
    case 'identity32':
      return Object.freeze({ form: 'both', base58: base58OrNull(slice), hex: hexOf(slice) });
    case 'reserved':
      return Object.freeze({
        form: 'reserved',
        zero: slice.every((byte) => byte === 0),
        hex: hexOf(slice),
      });
    case 'span':
      return Object.freeze({
        form: 'span',
        bytes: width,
        hex: hexOf(slice.slice(0, HEX_HEAD_BYTES)),
        note: field_.note ?? null,
      });
  }
}

/** Decode one account's bytes against its spec. Never throws. */
export function decodeAgainstSpec(spec: RecordSpec, data: Uint8Array): DecodedRecord {
  const headerEnd = headerEndOf(spec.width);
  const fields = spec.fields.map((declared, index) => {
    const width = fieldWidth(spec, index, headerEnd);
    return Object.freeze({
      label: declared.label,
      offset: declared.offset,
      bytes: width,
      kind: declared.kind,
      value: decodeValue(declared, data, width),
    });
  });

  let rows: DecodedRows | null = null;
  let expected: string;
  let ok: boolean;
  switch (spec.width.kind) {
    case 'fixed':
      expected = `exactly ${spec.width.bytes} bytes`;
      ok = data.length === spec.width.bytes;
      break;
    case 'header-only':
      expected = `at least ${spec.width.headerBytes} bytes (${spec.width.note})`;
      ok = data.length >= spec.width.headerBytes;
      break;
    case 'header-and-rows': {
      const { headerBytes, strideBytes, countOffset, countKind, rowLabel } = spec.width;
      if (countOffset + SCALAR_WIDTHS[countKind] > data.length) {
        expected = `${headerBytes} bytes plus rows`;
        ok = false;
        break;
      }
      const count = Number(readUnsigned(data, countOffset, SCALAR_WIDTHS[countKind]));
      const total = headerBytes + count * strideBytes;
      expected = `${headerBytes} + ${count} × ${strideBytes} = ${total} bytes`;
      ok = data.length === total;
      const scalars =
        strideBytes === SCALAR_BYTES && ok
          ? Object.freeze(
              Array.from({ length: count }, (_unused, index) =>
                readUnsigned(data, headerBytes + index * SCALAR_BYTES, SCALAR_BYTES).toString(),
              ),
            )
          : null;
      rows = Object.freeze({ label: rowLabel, count, strideBytes, offset: headerBytes, scalars });
      break;
    }
  }

  return Object.freeze({
    spec,
    magic: magicText(spec.magic),
    accountBytes: data.length,
    widthCheck: Object.freeze({ ok, expected, observed: data.length }),
    fields: Object.freeze(fields),
    rows,
  });
}
