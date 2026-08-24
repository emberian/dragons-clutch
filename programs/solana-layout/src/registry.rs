//! Central wire-allocation registry for legacy intents and successor families.
//!
//! The legacy intent space is frozen at tags `1..=73`, all at wire version 3.
//! Successors use one family tag and one family version followed by a
//! family-local action byte.  Registering a family or action here does not
//! make it executable: runtime capability admission is a separate, fail-closed
//! decision.
//!
//! Account discriminators are a different namespace.  In particular, General
//! V2 intent family tag 74 is decimal 74 (`0x4a`), while the already-frozen
//! Source Archive V2 account discriminator `0x74` is decimal 116.  The
//! constants and compile-time assertions below make that distinction
//! mechanical rather than typographical.

use super::MAX_INTENT_BYTES;

/// First frozen legacy intent tag.
pub const LEGACY_INTENT_FIRST_TAG: u8 = 1;
/// Last frozen legacy intent tag.
pub const LEGACY_INTENT_LAST_TAG: u8 = 73;
/// Frozen wire version shared by legacy intent tags `1..=73`.
pub const LEGACY_INTENT_VERSION: u8 = 3;

/// General V2 successor intent-family tag: decimal 74, hexadecimal `0x4a`.
pub const GENERAL_V2_FAMILY_TAG: u8 = 74;
/// General V2 successor intent-family version.
pub const GENERAL_V2_FAMILY_VERSION: u8 = 1;
/// Structured-claim successor intent-family tag.
pub const STRUCTURED_CLAIM_FAMILY_TAG: u8 = 75;
/// Structured-claim successor intent-family version.
pub const STRUCTURED_CLAIM_FAMILY_VERSION: u8 = 1;
/// Covered-dealer successor intent-family tag.
pub const DEALER_FAMILY_TAG: u8 = 76;
/// Covered-dealer successor intent-family version.
pub const DEALER_FAMILY_VERSION: u8 = 1;
/// Source-plane and Series successor intent-family tag.
pub const SOURCE_SERIES_FAMILY_TAG: u8 = 77;
/// Source-plane and Series successor intent-family version.
pub const SOURCE_SERIES_FAMILY_VERSION: u8 = 2;
/// Evidence-only recovery successor intent-family tag.
pub const RECOVERY_FAMILY_TAG: u8 = 78;
/// Evidence-only recovery successor intent-family version.
pub const RECOVERY_FAMILY_VERSION: u8 = 1;
/// Exact fractional-redemption successor intent-family tag.
pub const FRACTIONAL_REDEMPTION_FAMILY_TAG: u8 = 79;
/// Exact fractional-redemption successor intent-family version.
pub const FRACTIONAL_REDEMPTION_FAMILY_VERSION: u8 = 1;
/// Current Direct-market successor intent-family tag.
pub const DIRECT_MARKET_FAMILY_TAG: u8 = 80;
/// Current Direct-market successor intent-family version.
pub const DIRECT_MARKET_FAMILY_VERSION: u8 = 1;

/// Existing Source Archive V2 **account** discriminator: hexadecimal `0x74`,
/// decimal 116.
pub const SOURCE_ARCHIVE_V2_ACCOUNT_TAG: u8 = 0x74;
/// Existing Source Archive V2 account version.
pub const SOURCE_ARCHIVE_V2_ACCOUNT_VERSION: u8 = 1;
/// General SettlementReceipt successor discriminator. This deliberately
/// reuses legacy receipt tag `0x0f` under a fresh version.
pub const GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_TAG: u8 = 0x0f;
/// General SettlementReceipt successor version.
pub const GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_VERSION: u8 = 3;
/// General SettlementReceipt V4 discriminator. V3 remains withdrawn and is
/// never reinterpreted despite the shared tag and width.
pub const GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_TAG: u8 = 0x0f;
/// General SettlementReceipt V4 version.
pub const GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_VERSION: u8 = 4;
/// Rent-owned General SettlementReceipt V5 discriminator. V4 remains
/// historical and cannot enter future executable routes.
pub const GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_TAG: u8 = 0x0f;
/// Sole future rent-owned General SettlementReceipt version.
pub const GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_VERSION: u8 = 5;
/// Exact rent-owned General SettlementReceipt V5 width.
pub const GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_BYTES: usize = 298;
/// Historical counted General Reservation discriminator.
pub const GENERAL_RESERVATION_V5_ACCOUNT_TAG: u8 = 0x13;
/// Historical counted General Reservation version.
pub const GENERAL_RESERVATION_V5_ACCOUNT_VERSION: u8 = 5;
/// Exact historical counted General Reservation width.
pub const GENERAL_RESERVATION_V5_ACCOUNT_BYTES: usize = 627;
/// Withdrawn deletable General Reservation discriminator.
pub const GENERAL_RESERVATION_V7_ACCOUNT_TAG: u8 = 0x13;
/// Withdrawn deletable General Reservation version.
pub const GENERAL_RESERVATION_V7_ACCOUNT_VERSION: u8 = 7;
/// Exact withdrawn deletable General Reservation width.
pub const GENERAL_RESERVATION_V7_ACCOUNT_BYTES: usize = 675;
/// Rent-owned General Reservation successor discriminator.
pub const GENERAL_RESERVATION_V9_ACCOUNT_TAG: u8 = 0x13;
/// Sole future rent-owned General Reservation version.
pub const GENERAL_RESERVATION_V9_ACCOUNT_VERSION: u8 = 9;
/// Exact rent-owned General Reservation width.
pub const GENERAL_RESERVATION_V9_ACCOUNT_BYTES: usize = 666;
/// General OrderPage successor discriminator. This deliberately reuses the
/// historical OrderPage tag under a fresh version.
pub const GENERAL_ORDER_PAGE_V5_ACCOUNT_TAG: u8 = 8;
/// General OrderPage successor version.
pub const GENERAL_ORDER_PAGE_V5_ACCOUNT_VERSION: u8 = 5;
/// General V2 genesis-assisted Market-runtime account discriminator.
pub const GENERAL_V2_MARKET_RUNTIME_ACCOUNT_TAG: u8 = 3;
/// RelationV2-native General Market-runtime account version.
pub const GENERAL_V2_MARKET_RUNTIME_ACCOUNT_VERSION: u8 = 3;
/// General V2 counted Epoch account discriminator.
pub const GENERAL_V2_EPOCH_ACCOUNT_TAG: u8 = 11;
/// RelationV2-native counted General Epoch account version.
pub const GENERAL_V2_EPOCH_ACCOUNT_VERSION: u8 = 6;
/// Full-width Realm-selected Hoard successor discriminator.
pub const COLLATERAL_HOARD_V2_ACCOUNT_TAG: u8 = 0x05;
/// Full-width Realm-selected Hoard successor version.
pub const COLLATERAL_HOARD_V2_ACCOUNT_VERSION: u8 = 2;
/// Full-width native ClaimLedger successor discriminator.
pub const CLAIM_LEDGER_V3_ACCOUNT_TAG: u8 = 0x41;
/// Full-width native ClaimLedger successor version.
pub const CLAIM_LEDGER_V3_ACCOUNT_VERSION: u8 = 3;
/// Full-width central Resolution successor discriminator.
pub const RESOLUTION_V5_ACCOUNT_TAG: u8 = 16;
/// Full-width central Resolution successor version.
pub const RESOLUTION_V5_ACCOUNT_VERSION: u8 = 5;
const _: () = assert!(RESOLUTION_V5_ACCOUNT_TAG == 16);
const _: () = assert!(RESOLUTION_V5_ACCOUNT_VERSION == 5);
/// Full-width global Position successor discriminator.
pub const RETIREMENT_V3_POSITION_ACCOUNT_TAG: u8 = 6;
/// Full-width global Position successor version.
pub const RETIREMENT_V3_POSITION_ACCOUNT_VERSION: u8 = 3;
/// Full-identity permanent Position tombstone discriminator.
pub const RETIREMENT_POSITION_TOMBSTONE_V3_ACCOUNT_TAG: u8 = 0x75;
/// Full-identity permanent Position tombstone version.
pub const RETIREMENT_POSITION_TOMBSTONE_ACCOUNT_VERSION_V3: u8 = 3;
/// General V2 active-width ClearWork successor account discriminator.
pub const GENERAL_V2_CLEAR_WORK_ACCOUNT_TAG: u8 = 17;
/// General V2 active-width ClearWork successor account version.
pub const GENERAL_V2_CLEAR_WORK_ACCOUNT_VERSION: u8 = 2;
/// Resumable RelationV2 General ClearWork successor account version.
pub const GENERAL_V2_CLEAR_WORK_ACCOUNT_VERSION_V3: u8 = 3;
/// General V2 active-width sealed-feed successor account discriminator.
pub const GENERAL_V2_FEED_ACCOUNT_TAG: u8 = 18;
/// General V2 active-width sealed-feed successor account version.
pub const GENERAL_V2_FEED_ACCOUNT_VERSION: u8 = 2;
/// General V2 Window successor account discriminator.
pub const GENERAL_V2_WINDOW_ACCOUNT_TAG: u8 = 24;
/// General V2 Window successor account version.
pub const GENERAL_V2_WINDOW_ACCOUNT_VERSION: u8 = 4;
/// General V2 full-rank Window successor version.
pub const GENERAL_V2_WINDOW_ACCOUNT_VERSION_V2: u8 = 5;
/// General V2 active-width feed-stage successor account discriminator.
pub const GENERAL_V2_FEED_STAGE_ACCOUNT_TAG: u8 = 25;
/// General V2 active-width feed-stage successor account version.
pub const GENERAL_V2_FEED_STAGE_ACCOUNT_VERSION: u8 = 2;
/// Funded General V2 admission-node account discriminator.
pub const GENERAL_V2_ADMISSION_NODE_ACCOUNT_TAG: u8 = 0x77;
/// Funded General V2 admission-node account version.
pub const GENERAL_V2_ADMISSION_NODE_ACCOUNT_VERSION: u8 = 1;
/// General V2 cost-certificate AdmissionNode successor version.
pub const GENERAL_V2_ADMISSION_NODE_ACCOUNT_VERSION_V2: u8 = 2;
/// General V2 epoch-budget account discriminator.
pub const GENERAL_V2_EPOCH_BUDGET_ACCOUNT_TAG: u8 = 0x78;
/// General V2 epoch-budget account version.
pub const GENERAL_V2_EPOCH_BUDGET_ACCOUNT_VERSION: u8 = 1;
/// General V2 immutable Market-binding account discriminator.
pub const GENERAL_V2_MARKET_BINDING_ACCOUNT_TAG: u8 = 0x79;
/// General V2 immutable Market-binding account version.
pub const GENERAL_V2_MARKET_BINDING_ACCOUNT_VERSION: u8 = 1;
/// General V2 immutable candidate-cost Market-binding successor version.
pub const GENERAL_V2_MARKET_BINDING_ACCOUNT_VERSION_V2: u8 = 2;
/// Counted-retirement Replay-successor account discriminator.
pub const REPLAY_SUCCESSOR_ACCOUNT_TAG: u8 = 0x7a;
/// Counted-retirement Replay-successor account version.
pub const REPLAY_SUCCESSOR_ACCOUNT_VERSION: u8 = 1;
/// Canonical purpose-owned Replay V3 envelope discriminator.
pub const PURPOSE_REPLAY_V3_ACCOUNT_TAG: u8 = REPLAY_SUCCESSOR_ACCOUNT_TAG;
/// Canonical purpose-owned Replay envelope version paired with Position V3.
pub const PURPOSE_REPLAY_V3_ACCOUNT_VERSION: u8 = 3;
/// Counted-retirement Market wrapper discriminator.
pub const RETIREMENT_V2_MARKET_ACCOUNT_TAG: u8 = 3;
/// Counted-retirement Market wrapper version.
pub const RETIREMENT_V2_MARKET_ACCOUNT_VERSION: u8 = 2;
/// Counted-retirement Position wrapper discriminator.
pub const RETIREMENT_V2_POSITION_ACCOUNT_TAG: u8 = 6;
/// Counted-retirement Position wrapper version.
pub const RETIREMENT_V2_POSITION_ACCOUNT_VERSION: u8 = 2;
/// Rent-owner-complete permanent Position tombstone discriminator.
pub const RETIREMENT_POSITION_TOMBSTONE_ACCOUNT_TAG: u8 = 0x75;
/// Rent-owner-complete permanent Position tombstone version.
pub const RETIREMENT_POSITION_TOMBSTONE_ACCOUNT_VERSION_V2: u8 = 2;
/// Counted-retirement general-Epoch wrapper discriminator.
pub const RETIREMENT_V2_EPOCH_ACCOUNT_TAG: u8 = 11;
/// Counted-retirement general-Epoch wrapper version.
pub const RETIREMENT_V2_EPOCH_ACCOUNT_VERSION: u8 = 5;
/// General V2 canonical EconomicDomain artifact account discriminator.
pub const GENERAL_V2_ECONOMIC_DOMAIN_ACCOUNT_TAG: u8 = 0x7b;
/// General V2 canonical EconomicDomain artifact account version.
pub const GENERAL_V2_ECONOMIC_DOMAIN_ACCOUNT_VERSION: u8 = 1;
/// General V2 selected-candidate settlement-authority account discriminator.
pub const GENERAL_V2_SELECTED_CANDIDATE_ACCOUNT_TAG: u8 = 0x7c;
/// General V2 selected-candidate settlement-authority account version.
pub const GENERAL_V2_SELECTED_CANDIDATE_ACCOUNT_VERSION: u8 = 1;
/// Non-production Dealer staged-catalog account discriminator.
pub const DEALER_POLICY_STAGE_ACCOUNT_TAG: u8 = 0x7d;
/// Dealer staged-catalog account version.
pub const DEALER_POLICY_STAGE_ACCOUNT_VERSION: u8 = 1;
/// Immutable Dealer policy catalog account discriminator.
pub const DEALER_POLICY_ACCOUNT_TAG: u8 = 0x7e;
/// Dealer immutable policy account version.
pub const DEALER_POLICY_ACCOUNT_VERSION: u8 = 1;
/// Frozen canonical `DealerPolicyV1` semantic-body length.
pub const DEALER_POLICY_BODY_BYTES: usize = 1_148;
/// Exact adapter-owned typed upload-stage header length.
pub const DEALER_POLICY_STAGE_HEADER_BYTES: usize = 140;
/// Exact maximum-width upload-stage account length.
pub const DEALER_POLICY_STAGE_ACCOUNT_BYTES: usize =
    DEALER_POLICY_STAGE_HEADER_BYTES + DEALER_POLICY_BODY_BYTES;
/// Exact adapter-owned immutable catalog header length.
pub const DEALER_POLICY_ACCOUNT_HEADER_BYTES: usize = 56;
/// Exact immutable catalog account length.
pub const DEALER_POLICY_ACCOUNT_BYTES: usize =
    DEALER_POLICY_ACCOUNT_HEADER_BYTES + DEALER_POLICY_BODY_BYTES;
/// Source/Series registry account discriminator.
pub const SOURCE_SERIES_REGISTRY_ACCOUNT_TAG: u8 = 0x7f;
/// Source/Series registry account version.
pub const SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V1: u8 = 1;
/// Current Source/Series registry account version retaining BundleV5.
pub const SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V2: u8 = 2;
/// Historical decoder coordinate retained for untrusted index clients only.
/// Runtime authority must use [`SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V2`].
#[deprecated(note = "V1 is withdrawn; use SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V2")]
pub const SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION: u8 =
    SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V1;
/// Source/Series present-funding account discriminator.
pub const SOURCE_SERIES_FUNDING_ACCOUNT_TAG: u8 = 0x80;
/// Source/Series present-funding account version.
pub const SOURCE_SERIES_FUNDING_ACCOUNT_VERSION_V1: u8 = 1;
/// Current BundleV5/QuoteV4 Series funding account version.
pub const SOURCE_SERIES_FUNDING_ACCOUNT_VERSION_V2: u8 = 2;
/// Historical decoder coordinate retained for untrusted index clients only.
#[deprecated(note = "V1 is withdrawn; use SOURCE_SERIES_FUNDING_ACCOUNT_VERSION_V2")]
pub const SOURCE_SERIES_FUNDING_ACCOUNT_VERSION: u8 =
    SOURCE_SERIES_FUNDING_ACCOUNT_VERSION_V1;
/// General V2 owner-aggregated settlement account discriminator.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG: u8 = 0x81;
/// Withdrawn non-aliasing General V2 owner-settlement V1 version.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V1: u8 = 1;
/// Withdrawn presence-explicit General V2 owner-settlement V2 version.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V2: u8 = 2;
/// Withdrawn Reservation-handoff General owner-settlement version.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V3: u8 = 3;
/// Historical delivery-complete General owner-settlement version.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V4: u8 = 4;
/// Sole future rent-owned General owner-settlement version.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V5: u8 = 5;
/// Current sole-future rent-owned General owner-settlement version.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION: u8 =
    GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V5;
/// Exact rent-owned General owner-settlement V5 width.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_BYTES_V5: usize = 340;
/// General V2 selected composite-fee record envelope discriminator.
pub const GENERAL_V2_SELECTED_FEE_RECORD_ACCOUNT_TAG: u8 = 0x82;
/// General V2 selected composite-fee record envelope version.
pub const GENERAL_V2_SELECTED_FEE_RECORD_ACCOUNT_VERSION: u8 = 1;
/// General V2 owner fee-carry envelope discriminator.
pub const GENERAL_V2_OWNER_FEE_CARRY_ACCOUNT_TAG: u8 = 0x83;
/// General V2 owner fee-carry envelope version.
pub const GENERAL_V2_OWNER_FEE_CARRY_ACCOUNT_VERSION: u8 = 1;
/// General V2 in-place owner fee-finalization successor version.
pub const GENERAL_V2_OWNER_FEE_FINALIZATION_ACCOUNT_VERSION: u8 = 2;
/// General V2 temporary owner payer-allocation envelope discriminator.
pub const GENERAL_V2_PAYER_ALLOCATION_ACCOUNT_TAG: u8 = 0x84;
/// General V2 temporary owner payer-allocation envelope version.
pub const GENERAL_V2_PAYER_ALLOCATION_ACCOUNT_VERSION: u8 = 1;
/// General V2 temporary candidate-wide recipient-allocation discriminator.
pub const GENERAL_V2_RECIPIENT_ALLOCATION_ACCOUNT_TAG: u8 = 0x85;
/// General V2 temporary candidate-wide recipient-allocation version.
pub const GENERAL_V2_RECIPIENT_ALLOCATION_ACCOUNT_VERSION: u8 = 1;
/// General V2 selected-record treasury-ledger envelope discriminator.
pub const GENERAL_V2_TREASURY_LEDGER_ACCOUNT_TAG: u8 = 0x86;
/// General V2 selected-record treasury-ledger envelope version.
pub const GENERAL_V2_TREASURY_LEDGER_ACCOUNT_VERSION: u8 = 1;
/// General V2 buyer-first settlement cash-pot envelope discriminator.
pub const GENERAL_V2_SETTLEMENT_CASH_POT_ACCOUNT_TAG: u8 = 0x87;
/// General V2 buyer-first settlement cash-pot envelope version.
pub const GENERAL_V2_SETTLEMENT_CASH_POT_ACCOUNT_VERSION: u8 = 1;
/// StructuredClaim immutable descriptor envelope discriminator.
pub const STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_TAG: u8 = 0x88;
/// Withdrawn StructuredClaim descriptor version with one ambiguous authority bump.
pub const HISTORICAL_STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_VERSION_V1: u8 = 1;
/// Sole future StructuredClaim descriptor version with distinct authority bumps.
pub const STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_VERSION: u8 = 2;
/// General V2 final settlement-pot account discriminator.
pub const GENERAL_V2_FINAL_POT_ACCOUNT_TAG: u8 = 0x89;
/// General V2 final settlement-pot account version.
pub const GENERAL_V2_FINAL_POT_ACCOUNT_VERSION: u8 = 1;
/// Immutable authenticated SourcePlane V3 release account discriminator.
pub const SOURCE_V3_RELEASE_ACCOUNT_TAG: u8 = 0x8a;
/// Historical SourcePlane V3 release account version. Its 1,008-byte body did
/// not authenticate the upgradeable receiver release and is never executable.
pub const SOURCE_V3_RELEASE_ACCOUNT_VERSION_V1: u8 = 1;
/// Current SourcePlane V3 release account version.
pub const SOURCE_V3_RELEASE_ACCOUNT_VERSION: u8 = 2;
/// Mutable SourcePlane V3 head account discriminator.
pub const SOURCE_V3_HEAD_ACCOUNT_TAG: u8 = 0x8b;
/// SourcePlane V3 head account version.
pub const SOURCE_V3_HEAD_ACCOUNT_VERSION: u8 = 1;
/// Durable SourcePlane V3 reopen-lineage account discriminator.
pub const SOURCE_V3_REOPEN_LINEAGE_ACCOUNT_TAG: u8 = 0x8c;
/// SourcePlane V3 release/route-bound reopen-lineage account version.
pub const SOURCE_V3_REOPEN_LINEAGE_ACCOUNT_VERSION: u8 = 2;
/// Exact SourcePlane V3 release/route-bound reopen-lineage account width.
pub const SOURCE_V3_REOPEN_LINEAGE_ACCOUNT_BYTES: usize = 352;
/// Mutable SourcePlane V3 open-page account discriminator.
pub const SOURCE_V3_OPEN_RAW_PAGE_ACCOUNT_TAG: u8 = 0x8d;
/// SourcePlane V3 open-page account version.
pub const SOURCE_V3_OPEN_RAW_PAGE_ACCOUNT_VERSION: u8 = 1;
/// Immutable SourcePlane V3 raw-page account discriminator.
pub const SOURCE_V3_RAW_PAGE_ACCOUNT_TAG: u8 = 0x8e;
/// SourcePlane V3 raw-page account version.
pub const SOURCE_V3_RAW_PAGE_ACCOUNT_VERSION: u8 = 1;
/// Mutable SourcePlane V3 WindowWork account discriminator.
pub const SOURCE_V3_WINDOW_WORK_ACCOUNT_TAG: u8 = 0x8f;
/// SourcePlane V3 WindowWork account version.
pub const SOURCE_V3_WINDOW_WORK_ACCOUNT_VERSION: u8 = 1;
/// Immutable SourcePlane V3 WindowSeal account discriminator.
pub const SOURCE_V3_WINDOW_SEAL_ACCOUNT_TAG: u8 = 0x90;
/// SourcePlane V3 WindowSeal account version.
pub const SOURCE_V3_WINDOW_SEAL_ACCOUNT_VERSION: u8 = 1;
/// Immutable SourcePlane V3 StatisticResult account discriminator.
pub const SOURCE_V3_STATISTIC_RESULT_ACCOUNT_TAG: u8 = 0x91;
/// SourcePlane V3 StatisticResult account version.
pub const SOURCE_V3_STATISTIC_RESULT_ACCOUNT_VERSION: u8 = 1;
/// Immutable SourcePlane V3 liveness-work receipt account discriminator.
pub const SOURCE_V3_WORK_RECEIPT_ACCOUNT_TAG: u8 = 0x92;
/// SourcePlane V3 liveness-work receipt account version.
pub const SOURCE_V3_WORK_RECEIPT_ACCOUNT_VERSION: u8 = 1;
/// Fixed global envelope preceding each Dealer runtime semantic body.
pub const DEALER_RUNTIME_ACCOUNT_HEADER_BYTES: usize = 8;
/// Immutable Dealer liveness-schedule account discriminator.
pub const DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG: u8 = 0x93;
/// Immutable Dealer liveness-schedule account version.
pub const DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION: u8 = 1;
/// Exact Dealer liveness-schedule account bytes.
pub const DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES: usize = DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 372;
/// Authoritative Dealer State V2 account discriminator.
pub const DEALER_STATE_V2_ACCOUNT_TAG: u8 = 0x94;
/// Authoritative Dealer State V2 account version.
pub const DEALER_STATE_V2_ACCOUNT_VERSION: u8 = 1;
/// Exact Dealer State V2 account bytes.
pub const DEALER_STATE_V2_ACCOUNT_BYTES: usize = DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 972;
/// Counted funded-dependencies account discriminator.
pub const DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG: u8 = 0x95;
/// Counted funded-dependencies account version.
pub const DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION: u8 = 1;
/// Exact counted funded-dependencies account bytes.
pub const DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES: usize =
    DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 600;
/// Immutable-after-activation Dealer LP page discriminator.
pub const DEALER_LP_PAGE_V2_ACCOUNT_TAG: u8 = 0x98;
/// Dealer LP page V2 account version.
pub const DEALER_LP_PAGE_V2_ACCOUNT_VERSION: u8 = 1;
/// Exact Dealer LP page V2 account bytes.
pub const DEALER_LP_PAGE_V2_ACCOUNT_BYTES: usize = DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 972;
/// One-generation Dealer Lease V2 discriminator.
pub const DEALER_LEASE_V2_ACCOUNT_TAG: u8 = 0x99;
/// Dealer Lease V2 account version.
pub const DEALER_LEASE_V2_ACCOUNT_VERSION: u8 = 1;
/// Exact Dealer Lease V2 account bytes.
pub const DEALER_LEASE_V2_ACCOUNT_BYTES: usize = DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 1_132;
/// Three-stage Dealer SettlementPot V2 discriminator.
pub const DEALER_SETTLEMENT_POT_V2_ACCOUNT_TAG: u8 = 0x9a;
/// Dealer SettlementPot V2 account version.
pub const DEALER_SETTLEMENT_POT_V2_ACCOUNT_VERSION: u8 = 1;
/// Exact Dealer SettlementPot V2 account bytes.
pub const DEALER_SETTLEMENT_POT_V2_ACCOUNT_BYTES: usize =
    DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 1_228;
/// Counted Dealer Epoch-binding V2 discriminator.
pub const DEALER_EPOCH_BINDING_V2_ACCOUNT_TAG: u8 = 0x9b;
/// Dealer Epoch-binding V2 account version.
pub const DEALER_EPOCH_BINDING_V2_ACCOUNT_VERSION: u8 = 1;
/// Exact Dealer Epoch-binding V2 account bytes.
pub const DEALER_EPOCH_BINDING_V2_ACCOUNT_BYTES: usize = DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 772;
/// Page-scoped Dealer terminal-allocation discriminator.
pub const DEALER_TERMINAL_ALLOCATION_ACCOUNT_TAG: u8 = 0x9c;
/// Dealer terminal-allocation account version.
pub const DEALER_TERMINAL_ALLOCATION_ACCOUNT_VERSION: u8 = 1;
/// Exact Dealer terminal-allocation account bytes.
pub const DEALER_TERMINAL_ALLOCATION_ACCOUNT_BYTES: usize =
    DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 748;
/// Singleton Dealer terminal ClaimWork discriminator.
pub const DEALER_CLAIM_WORK_ACCOUNT_TAG: u8 = 0x9d;
/// Dealer terminal ClaimWork account version.
pub const DEALER_CLAIM_WORK_ACCOUNT_VERSION: u8 = 1;
/// Exact Dealer terminal ClaimWork account bytes.
pub const DEALER_CLAIM_WORK_ACCOUNT_BYTES: usize = DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 1_140;
/// Permanent Dealer root-tombstone V2 discriminator.
pub const DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_TAG: u8 = 0x9e;
/// Dealer root-tombstone V2 account version.
pub const DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_VERSION: u8 = 1;
/// Exact current Dealer root-tombstone V2 account bytes.
pub const DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_BYTES: usize = DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 468;
/// Owner-scoped Dealer exit-ticket discriminator.
pub const DEALER_EXIT_TICKET_ACCOUNT_TAG: u8 = 0x9f;
/// Dealer exit-ticket account version.
pub const DEALER_EXIT_TICKET_ACCOUNT_VERSION: u8 = 1;
/// Exact Dealer exit-ticket account bytes.
pub const DEALER_EXIT_TICKET_ACCOUNT_BYTES: usize = DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 356;
/// Single-custody failure semantic root account discriminator.
pub const FAILURE_EXTERNAL_ROOT_ACCOUNT_TAG: u8 = 0xa0;
/// Single-custody failure semantic root account version.
pub const FAILURE_EXTERNAL_ROOT_ACCOUNT_VERSION: u8 = 1;
/// Shared-Market failure policy/funding root successor version.
pub const FAILURE_MARKET_ROOT_ACCOUNT_VERSION_V2: u8 = 2;
/// Market-scoped mutable Failure runtime root successor version.
pub const FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_VERSION_V1: u8 = 3;
/// Immutable runtime-liveness policy account discriminator.
pub const FAILURE_LIVENESS_POLICY_ACCOUNT_TAG: u8 = 0xa1;
/// Immutable runtime-liveness policy account version.
pub const FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION: u8 = 1;
/// Sole persisted Recovery work/rent custody account discriminator.
pub const FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG: u8 = 0xa2;
/// Sole persisted Recovery work/rent custody account version.
pub const FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION: u8 = 1;
/// Permanent failure-generation replay tombstone discriminator.
pub const FAILURE_REPLAY_TOMBSTONE_ACCOUNT_TAG: u8 = 0xa3;
/// Permanent failure-generation replay tombstone version.
pub const FAILURE_REPLAY_TOMBSTONE_ACCOUNT_VERSION: u8 = 1;
/// Permanent shared-Market Failure replay successor version.
pub const FAILURE_MARKET_REPLAY_ACCOUNT_VERSION_V2: u8 = 2;
/// Exact framed shared-Market Failure replay bytes.
pub const FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2: usize = 256;
/// Immutable exact fractional-redemption policy discriminator.
pub const FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_TAG: u8 = 0xa4;
/// Withdrawn policy version whose offset 80 meant payout-vector digest.
pub const FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_V1_VERSION: u8 = 1;
/// Canonical Resolution-V5-data-bound policy version.
pub const FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_VERSION: u8 = 2;
/// Exact immutable fractional-redemption policy bytes.
pub const FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_BYTES: usize = 296;
/// Sole aggregate numerator-credit ledger discriminator.
pub const FRACTIONAL_REDEMPTION_LEDGER_ACCOUNT_TAG: u8 = 0xa5;
/// Sole aggregate numerator-credit ledger version.
pub const FRACTIONAL_REDEMPTION_LEDGER_ACCOUNT_VERSION: u8 = 1;
/// Exact aggregate numerator-credit ledger bytes.
pub const FRACTIONAL_REDEMPTION_LEDGER_ACCOUNT_BYTES: usize = 224;
/// Owner-scoped exact numerator-credit discriminator.
pub const FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_TAG: u8 = 0xa6;
/// Withdrawn credit version whose offset 176 meant payout-vector digest.
pub const FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_V1_VERSION: u8 = 1;
/// Canonical Resolution-V5-data-bound owner-credit version.
pub const FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_VERSION: u8 = 2;
/// Exact owner-scoped numerator-credit bytes.
pub const FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_BYTES: usize = 296;
/// Permanent zero-credit replay tombstone discriminator.
pub const FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_TAG: u8 = 0xa7;
/// Withdrawn tombstone version whose offset 160 meant payout-vector digest.
pub const FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_V1_VERSION: u8 = 1;
/// Canonical Resolution-V5-data-bound replay tombstone version.
pub const FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_VERSION: u8 = 2;
/// Exact permanent zero-credit replay tombstone bytes.
pub const FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_BYTES: usize = 232;
/// Mutable exhaustive quantized interval-consensus work discriminator.
pub const FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_TAG: u8 = 0xab;
/// Withdrawn one-shot work-account version.
pub const FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_V1_VERSION: u8 = 1;
/// Reusable Market interval-cell version.
pub const FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_VERSION: u8 = 2;
/// Exact framed interval-consensus work account bytes.
pub const FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES: usize = 1_088;
/// Permanent Market interval-history discriminator.
pub const FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_TAG: u8 = 0xac;
/// Withdrawn one-shot replay-account version.
pub const FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_V1_VERSION: u8 = 1;
/// Append-only Market interval-history version.
pub const FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_VERSION: u8 = 2;
/// Exact permanent Market interval-history bytes.
pub const FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES: usize = 512;
/// Series-link-scoped mutable Structured descriptor-family root.
pub const STRUCTURED_MARKET_ROOT_ACCOUNT_TAG: u8 = 0xb7;
/// First Structured root account version.
pub const STRUCTURED_MARKET_ROOT_ACCOUNT_VERSION: u8 = 1;
/// Exact Structured root account width.
pub const STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES: usize = 656;
/// Immutable, deletable Dealer action-work receipt discriminator.
pub const DEALER_ACTION_RECEIPT_ACCOUNT_TAG: u8 = 0xa8;
/// Dealer action-work receipt account version.
pub const DEALER_ACTION_RECEIPT_ACCOUNT_VERSION: u8 = 1;
/// Exact Dealer action receipt bytes including the global envelope.
pub const DEALER_ACTION_RECEIPT_ACCOUNT_BYTES: usize = DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 532;
/// Counted General V2 candidate-scoped settlement root discriminator.
pub const GENERAL_V2_SETTLEMENT_ROOT_ACCOUNT_TAG: u8 = 0xa9;
/// First counted General V2 settlement-root version.
pub const GENERAL_V2_SETTLEMENT_ROOT_ACCOUNT_VERSION: u8 = 1;
/// Exact fixed width of the counted General V2 settlement root.
pub const GENERAL_V2_SETTLEMENT_ROOT_ACCOUNT_BYTES: usize = 980;
/// Counted exact-index General V2 settlement-root successor version.
pub const GENERAL_V2_INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION: u8 = 2;
/// Exact fixed width of the counted exact-index settlement-root successor.
pub const GENERAL_V2_INDEXED_SETTLEMENT_ROOT_ACCOUNT_BYTES: usize = 1_228;
/// Product shared Market lifecycle root discriminator.
pub const PRODUCT_MARKET_LIFECYCLE_ROOT_ACCOUNT_TAG: u8 = 0xaa;
/// First Product shared Market lifecycle-root version.
pub const PRODUCT_MARKET_LIFECYCLE_ROOT_ACCOUNT_VERSION: u8 = 1;
/// Product per-Series/ordinal Market-admission link discriminator.
pub const PRODUCT_SERIES_MARKET_LINK_ACCOUNT_TAG: u8 = 0xad;
/// First Product Series-Market-link version.
pub const PRODUCT_SERIES_MARKET_LINK_ACCOUNT_VERSION: u8 = 1;
/// Counted Dealer CoveredDealer selection attachment discriminator.
pub const DEALER_COVERED_SELECTION_ACCOUNT_TAG: u8 = 0xae;
/// Current Direct root and lifecycle-count owner discriminator.
pub const DIRECT_MARKET_ROOT_ACCOUNT_TAG: u8 = 0xb1;
/// Current Direct root account version.
pub const DIRECT_MARKET_ROOT_ACCOUNT_VERSION: u8 = 1;
/// Current Direct exact-selection owner discriminator.
pub const DIRECT_SELECTION_ACCOUNT_TAG: u8 = 0xb2;
/// Current Direct exact-selection account version.
pub const DIRECT_SELECTION_ACCOUNT_VERSION: u8 = 1;
/// Current Direct permanent action replay/terminal receipt discriminator.
pub const DIRECT_ACTION_REPLAY_ACCOUNT_TAG: u8 = 0xb3;
/// Current Direct permanent action replay/terminal receipt version.
pub const DIRECT_ACTION_REPLAY_ACCOUNT_VERSION: u8 = 1;
/// Current Direct funded Reservation owner discriminator.
pub const DIRECT_RESERVATION_ACCOUNT_TAG: u8 = 0xb4;
/// Current Direct funded Reservation account version.
pub const DIRECT_RESERVATION_ACCOUNT_VERSION: u8 = 1;
/// Immutable General V2 frozen-order locator discriminator.
pub const GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_TAG: u8 = 0xb5;
/// First frozen-order locator account version.
pub const GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_VERSION: u8 = 1;
/// Largest exact active frozen-order locator account body.
pub const GENERAL_V2_FROZEN_ORDER_LOCATOR_MAX_ACCOUNT_BYTES: usize = 528;
/// Immutable General V2 selected-candidate adjacency discriminator.
pub const GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_TAG: u8 = 0xb6;
/// First selected-candidate adjacency account version.
pub const GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_VERSION: u8 = 1;
/// Largest exact active selected-candidate adjacency account body.
pub const GENERAL_V2_CANDIDATE_ADJACENCY_MAX_ACCOUNT_BYTES: usize = 2_448;
/// First Dealer CoveredDealer selection attachment version.
pub const DEALER_COVERED_SELECTION_ACCOUNT_VERSION: u8 = 1;
/// Exact attachment bytes including the Dealer global envelope.
pub const DEALER_COVERED_SELECTION_ACCOUNT_BYTES: usize =
    DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 5_436;
/// Permanent compact Product Market-lifecycle replay receipt discriminator.
pub const PRODUCT_MARKET_LIFECYCLE_REPLAY_ACCOUNT_TAG: u8 = 0xb0;
/// First Product Market-lifecycle replay receipt version.
pub const PRODUCT_MARKET_LIFECYCLE_REPLAY_ACCOUNT_VERSION: u8 = 1;
/// Bytes occupied by the successor family tag, family version, and local action.
pub const EXTENSION_ENVELOPE_BYTES: usize = 3;
/// Largest successor action payload without changing the frozen packet ceiling.
pub const MAX_EXTENSION_PAYLOAD_BYTES: usize = MAX_INTENT_BYTES - EXTENSION_ENVELOPE_BYTES;

const _: () = assert!(GENERAL_V2_FAMILY_TAG == 74);
const _: () = assert!(DEALER_COVERED_SELECTION_ACCOUNT_TAG == 0xae);
const _: () = assert!(PRODUCT_MARKET_LIFECYCLE_REPLAY_ACCOUNT_TAG == 0xb0);
const _: () = assert!(DIRECT_MARKET_ROOT_ACCOUNT_TAG == 0xb1);
const _: () = assert!(DIRECT_SELECTION_ACCOUNT_TAG == 0xb2);
const _: () = assert!(DIRECT_ACTION_REPLAY_ACCOUNT_TAG == 0xb3);
const _: () = assert!(DIRECT_RESERVATION_ACCOUNT_TAG == 0xb4);
const _: () = assert!(GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_TAG == 0xb5);
const _: () = assert!(GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_TAG == 0xb6);
const _: () = assert!(GENERAL_V2_FAMILY_TAG == 0x4a);
const _: () = assert!(LEGACY_INTENT_FIRST_TAG == super::CREATE_TAG);
const _: () = assert!(LEGACY_INTENT_LAST_TAG == super::SEAL_SOURCE_ARCHIVE_V2_TAG);
const _: () = assert!(LEGACY_INTENT_VERSION == super::INTENT_VERSION);
const _: () = assert!(SOURCE_ARCHIVE_V2_ACCOUNT_TAG == 116);
const _: () = assert!(SOURCE_ARCHIVE_V2_ACCOUNT_TAG == 0x74);
const _: () = assert!(GENERAL_V2_FAMILY_TAG != SOURCE_ARCHIVE_V2_ACCOUNT_TAG);
const _: () = assert!(GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_TAG == 15);
const _: () = assert!(GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_VERSION == 3);
const _: () = assert!(GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_TAG == 15);
const _: () = assert!(GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_VERSION == 4);
const _: () = assert!(GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_TAG == 15);
const _: () = assert!(GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_VERSION == 5);
const _: () = assert!(GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_BYTES == 298);
const _: () = assert!(
    GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_BYTES == super::account_len::SETTLEMENT_RECEIPT_V5
);
const _: () = assert!(GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V5 == 5);
const _: () = assert!(GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_BYTES_V5 == 340);
const _: () = assert!(GENERAL_RESERVATION_V5_ACCOUNT_TAG == 0x13);
const _: () = assert!(GENERAL_RESERVATION_V5_ACCOUNT_VERSION == 5);
const _: () = assert!(GENERAL_RESERVATION_V5_ACCOUNT_BYTES == 627);
const _: () = assert!(GENERAL_RESERVATION_V7_ACCOUNT_TAG == 0x13);
const _: () = assert!(GENERAL_RESERVATION_V7_ACCOUNT_VERSION == 7);
const _: () = assert!(GENERAL_RESERVATION_V7_ACCOUNT_BYTES == 675);
const _: () =
    assert!(GENERAL_RESERVATION_V9_ACCOUNT_TAG == super::reservation::RESERVATION_ACCOUNT_TAG);
const _: () = assert!(
    GENERAL_RESERVATION_V9_ACCOUNT_VERSION == super::reservation_v9::RESERVATION_ACCOUNT_VERSION_V9
);
const _: () = assert!(
    GENERAL_RESERVATION_V9_ACCOUNT_BYTES == super::reservation_v9::RESERVATION_ACCOUNT_BYTES_V9
);
const _: () = assert!(GENERAL_ORDER_PAGE_V5_ACCOUNT_TAG == 8);
const _: () = assert!(GENERAL_ORDER_PAGE_V5_ACCOUNT_VERSION == 5);
const _: () = assert!(GENERAL_ORDER_PAGE_V5_ACCOUNT_TAG == super::order_page_v5::ORDER_PAGE_V5_TAG);
const _: () =
    assert!(GENERAL_ORDER_PAGE_V5_ACCOUNT_VERSION == super::order_page_v5::ORDER_PAGE_V5_VERSION);
const _: () = assert!(EXTENSION_ENVELOPE_BYTES <= MAX_INTENT_BYTES);
const _: () = assert!(FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_V1_VERSION == 1);
const _: () = assert!(FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_VERSION == 2);
const _: () = assert!(FRACTIONAL_REDEMPTION_LEDGER_ACCOUNT_VERSION == 1);
const _: () = assert!(FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_V1_VERSION == 1);
const _: () = assert!(FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_VERSION == 2);
const _: () = assert!(FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_V1_VERSION == 1);
const _: () = assert!(FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_VERSION == 2);
const _: () = assert!(DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG == 0x93);
const _: () = assert!(DEALER_STATE_V2_ACCOUNT_TAG == 0x94);
const _: () = assert!(DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG == 0x95);
const _: () = assert!(DEALER_LP_PAGE_V2_ACCOUNT_TAG == 0x98);
const _: () = assert!(DEALER_LEASE_V2_ACCOUNT_TAG == 0x99);
const _: () = assert!(DEALER_SETTLEMENT_POT_V2_ACCOUNT_TAG == 0x9a);
const _: () = assert!(DEALER_EPOCH_BINDING_V2_ACCOUNT_TAG == 0x9b);
const _: () = assert!(DEALER_TERMINAL_ALLOCATION_ACCOUNT_TAG == 0x9c);
const _: () = assert!(DEALER_CLAIM_WORK_ACCOUNT_TAG == 0x9d);
const _: () = assert!(DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_TAG == 0x9e);
const _: () = assert!(DEALER_EXIT_TICKET_ACCOUNT_TAG == 0x9f);
const _: () = assert!(DEALER_ACTION_RECEIPT_ACCOUNT_TAG == 0xa8);
const _: () = assert!(GENERAL_V2_SETTLEMENT_ROOT_ACCOUNT_TAG == 0xa9);
const _: () = assert!(GENERAL_V2_INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION == 2);
const _: () = assert!(GENERAL_V2_INDEXED_SETTLEMENT_ROOT_ACCOUNT_BYTES == 1_228);
const _: () = assert!(PRODUCT_MARKET_LIFECYCLE_ROOT_ACCOUNT_TAG == 0xaa);
const _: () = assert!(FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_TAG == 0xab);
const _: () = assert!(FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_TAG == 0xac);
const _: () = assert!(PRODUCT_SERIES_MARKET_LINK_ACCOUNT_TAG == 0xad);
const _: () = assert!(STRUCTURED_MARKET_ROOT_ACCOUNT_TAG == 0xb7);
const _: () = assert!(STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES == 656);

/// Disjoint wire namespaces represented in the collision ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireNamespace {
    /// The main program's signed intent namespace.
    MainIntent,
    /// The main program's persisted-account namespace.
    MainAccount,
}

/// Stability of one central allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationStatus {
    /// Bytes already have a frozen deployed or golden-tested meaning.
    Frozen,
    /// Coordinates are reserved, but every runtime capability is disabled.
    ReservedDisabled,
    /// Executable only in one explicitly named non-production laboratory
    /// profile; every production profile remains disabled.
    NonProductionLab,
    /// Coordinates remain permanently occupied but no decoder, constructor,
    /// migration, or executable route may treat them as current state.
    Withdrawn,
}

/// Coordinates occupied by one collision-ledger entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationCoordinates {
    /// An inclusive range in one wire namespace at one version.
    Range {
        /// Namespace containing the range.
        namespace: WireNamespace,
        /// Inclusive first tag.
        first_tag: u8,
        /// Inclusive last tag.
        last_tag: u8,
        /// Exact wire version.
        version: u8,
    },
    /// One exact tag/version pair in one wire namespace.
    Exact {
        /// Namespace containing the pair.
        namespace: WireNamespace,
        /// Exact tag.
        tag: u8,
        /// Exact version.
        version: u8,
    },
}

/// One recorded collision-sensitive allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollisionLedgerEntry {
    /// Occupied wire coordinates.
    pub coordinates: AllocationCoordinates,
    /// Whether the coordinates are frozen, disabled, laboratory-only, or withdrawn.
    pub status: AllocationStatus,
    /// Stable human-readable allocation name.
    pub name: &'static str,
}

/// Collision-sensitive allocations for the extension seam.
///
/// This is not a complete account-codec inventory. Existing account modules
/// remain the semantic owners of their bytes. Every newly allocated successor
/// coordinate must be checked against this shared ledger and a complete legacy
/// inventory before any runtime capability is enabled.
pub const CENTRAL_COLLISION_LEDGER: &[CollisionLedgerEntry] = &[
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Range {
            namespace: WireNamespace::MainIntent,
            first_tag: LEGACY_INTENT_FIRST_TAG,
            last_tag: LEGACY_INTENT_LAST_TAG,
            version: LEGACY_INTENT_VERSION,
        },
        status: AllocationStatus::Frozen,
        name: "legacy-intents-v3",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainIntent,
            tag: GENERAL_V2_FAMILY_TAG,
            version: GENERAL_V2_FAMILY_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_RESERVATION_V5_ACCOUNT_TAG,
            version: GENERAL_RESERVATION_V5_ACCOUNT_VERSION,
        },
        status: AllocationStatus::Withdrawn,
        name: "withdrawn-counted-general-reservation-v5-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_RESERVATION_V7_ACCOUNT_TAG,
            version: GENERAL_RESERVATION_V7_ACCOUNT_VERSION,
        },
        status: AllocationStatus::Withdrawn,
        name: "withdrawn-deletable-general-reservation-v7-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_RESERVATION_V9_ACCOUNT_TAG,
            version: GENERAL_RESERVATION_V9_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-reservation-v9-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainIntent,
            tag: STRUCTURED_CLAIM_FAMILY_TAG,
            version: STRUCTURED_CLAIM_FAMILY_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "structured-claim",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainIntent,
            tag: DEALER_FAMILY_TAG,
            version: DEALER_FAMILY_VERSION,
        },
        status: AllocationStatus::NonProductionLab,
        name: "covered-dealer",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainIntent,
            tag: SOURCE_SERIES_FAMILY_TAG,
            version: SOURCE_SERIES_FAMILY_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-series",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainIntent,
            tag: RECOVERY_FAMILY_TAG,
            version: RECOVERY_FAMILY_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "evidence-only-recovery",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainIntent,
            tag: FRACTIONAL_REDEMPTION_FAMILY_TAG,
            version: FRACTIONAL_REDEMPTION_FAMILY_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "exact-fractional-redemption",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainIntent,
            tag: DIRECT_MARKET_FAMILY_TAG,
            version: DIRECT_MARKET_FAMILY_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "direct-market-v1",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DIRECT_MARKET_ROOT_ACCOUNT_TAG,
            version: DIRECT_MARKET_ROOT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "direct-market-root-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DIRECT_SELECTION_ACCOUNT_TAG,
            version: DIRECT_SELECTION_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "direct-selection-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DIRECT_ACTION_REPLAY_ACCOUNT_TAG,
            version: DIRECT_ACTION_REPLAY_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "direct-action-replay-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DIRECT_RESERVATION_ACCOUNT_TAG,
            version: DIRECT_RESERVATION_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "direct-reservation-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_TAG,
            version: GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-frozen-order-locator-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_TAG,
            version: GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-candidate-adjacency-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_ARCHIVE_V2_ACCOUNT_TAG,
            version: SOURCE_ARCHIVE_V2_ACCOUNT_VERSION,
        },
        status: AllocationStatus::Frozen,
        name: "source-archive-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_TAG,
            version: GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_VERSION,
        },
        status: AllocationStatus::Withdrawn,
        name: "general-settlement-receipt-v3-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_TAG,
            version: GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_VERSION,
        },
        status: AllocationStatus::Withdrawn,
        name: "historical-general-settlement-receipt-v4-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_TAG,
            version: GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "rent-owned-general-settlement-receipt-v5-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_ORDER_PAGE_V5_ACCOUNT_TAG,
            version: GENERAL_ORDER_PAGE_V5_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-order-page-v5-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_MARKET_RUNTIME_ACCOUNT_TAG,
            version: GENERAL_V2_MARKET_RUNTIME_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-market-runtime-v3-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_EPOCH_ACCOUNT_TAG,
            version: GENERAL_V2_EPOCH_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-epoch-v6-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: 0x75,
            version: 1,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "retirement-provisional-position-tombstone-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: RETIREMENT_POSITION_TOMBSTONE_ACCOUNT_TAG,
            version: RETIREMENT_POSITION_TOMBSTONE_ACCOUNT_VERSION_V2,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "retirement-position-tombstone-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: RETIREMENT_POSITION_TOMBSTONE_V3_ACCOUNT_TAG,
            version: RETIREMENT_POSITION_TOMBSTONE_ACCOUNT_VERSION_V3,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "retirement-full-identity-position-tombstone-v3-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: RETIREMENT_V3_POSITION_ACCOUNT_TAG,
            version: RETIREMENT_V3_POSITION_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "retirement-full-width-position-v3-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: COLLATERAL_HOARD_V2_ACCOUNT_TAG,
            version: COLLATERAL_HOARD_V2_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "collateral-full-width-hoard-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: CLAIM_LEDGER_V3_ACCOUNT_TAG,
            version: CLAIM_LEDGER_V3_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "native-claim-ledger-v3-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: RESOLUTION_V5_ACCOUNT_TAG,
            version: RESOLUTION_V5_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "full-width-resolution-v5-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: 0x76,
            version: 1,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "retirement-provisional-general-epoch-tombstone-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: RETIREMENT_V2_MARKET_ACCOUNT_TAG,
            version: RETIREMENT_V2_MARKET_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "retirement-v2-market-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: RETIREMENT_V2_POSITION_ACCOUNT_TAG,
            version: RETIREMENT_V2_POSITION_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "retirement-v2-position-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: RETIREMENT_V2_EPOCH_ACCOUNT_TAG,
            version: RETIREMENT_V2_EPOCH_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "retirement-v2-general-epoch-v5-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_CLEAR_WORK_ACCOUNT_TAG,
            version: GENERAL_V2_CLEAR_WORK_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-clear-work-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_CLEAR_WORK_ACCOUNT_TAG,
            version: GENERAL_V2_CLEAR_WORK_ACCOUNT_VERSION_V3,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-clear-work-v3-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_FEED_ACCOUNT_TAG,
            version: GENERAL_V2_FEED_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-feed-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_WINDOW_ACCOUNT_TAG,
            version: GENERAL_V2_WINDOW_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-window-v4-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_WINDOW_ACCOUNT_TAG,
            version: GENERAL_V2_WINDOW_ACCOUNT_VERSION_V2,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-window-v5-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_FEED_STAGE_ACCOUNT_TAG,
            version: GENERAL_V2_FEED_STAGE_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-feed-stage-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_ADMISSION_NODE_ACCOUNT_TAG,
            version: GENERAL_V2_ADMISSION_NODE_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-admission-node-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_ADMISSION_NODE_ACCOUNT_TAG,
            version: GENERAL_V2_ADMISSION_NODE_ACCOUNT_VERSION_V2,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-admission-node-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_EPOCH_BUDGET_ACCOUNT_TAG,
            version: GENERAL_V2_EPOCH_BUDGET_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-epoch-budget-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_MARKET_BINDING_ACCOUNT_TAG,
            version: GENERAL_V2_MARKET_BINDING_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-market-binding-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_MARKET_BINDING_ACCOUNT_TAG,
            version: GENERAL_V2_MARKET_BINDING_ACCOUNT_VERSION_V2,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-market-binding-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: REPLAY_SUCCESSOR_ACCOUNT_TAG,
            version: REPLAY_SUCCESSOR_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "replay-successor-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: PURPOSE_REPLAY_V3_ACCOUNT_TAG,
            version: PURPOSE_REPLAY_V3_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "purpose-owned-replay-v3-envelope",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_ECONOMIC_DOMAIN_ACCOUNT_TAG,
            version: GENERAL_V2_ECONOMIC_DOMAIN_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-economic-domain-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_SELECTED_CANDIDATE_ACCOUNT_TAG,
            version: GENERAL_V2_SELECTED_CANDIDATE_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-selected-candidate-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_POLICY_STAGE_ACCOUNT_TAG,
            version: DEALER_POLICY_STAGE_ACCOUNT_VERSION,
        },
        status: AllocationStatus::NonProductionLab,
        name: "non-production-dealer-policy-stage-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_POLICY_ACCOUNT_TAG,
            version: DEALER_POLICY_ACCOUNT_VERSION,
        },
        status: AllocationStatus::NonProductionLab,
        name: "dealer-policy-catalog-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_SERIES_REGISTRY_ACCOUNT_TAG,
            version: SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V1,
        },
        status: AllocationStatus::Withdrawn,
        name: "withdrawn-source-series-registry-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_SERIES_REGISTRY_ACCOUNT_TAG,
            version: SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V2,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-series-registry-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_SERIES_FUNDING_ACCOUNT_TAG,
            version: SOURCE_SERIES_FUNDING_ACCOUNT_VERSION_V1,
        },
        status: AllocationStatus::Withdrawn,
        name: "withdrawn-source-series-funding-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_SERIES_FUNDING_ACCOUNT_TAG,
            version: SOURCE_SERIES_FUNDING_ACCOUNT_VERSION_V2,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-series-funding-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
            version: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V1,
        },
        status: AllocationStatus::Withdrawn,
        name: "general-v2-owner-settlement-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
            version: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V2,
        },
        status: AllocationStatus::Withdrawn,
        name: "general-v2-owner-settlement-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
            version: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V3,
        },
        status: AllocationStatus::Withdrawn,
        name: "general-owner-settlement-v3-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
            version: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V4,
        },
        status: AllocationStatus::Withdrawn,
        name: "historical-general-owner-settlement-v4-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
            version: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V5,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "rent-owned-general-owner-settlement-v5-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_SELECTED_FEE_RECORD_ACCOUNT_TAG,
            version: GENERAL_V2_SELECTED_FEE_RECORD_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-selected-fee-record-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_OWNER_FEE_CARRY_ACCOUNT_TAG,
            version: GENERAL_V2_OWNER_FEE_CARRY_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-owner-fee-carry-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_OWNER_FEE_CARRY_ACCOUNT_TAG,
            version: GENERAL_V2_OWNER_FEE_FINALIZATION_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-owner-fee-finalization-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_PAYER_ALLOCATION_ACCOUNT_TAG,
            version: GENERAL_V2_PAYER_ALLOCATION_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-payer-allocation-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_RECIPIENT_ALLOCATION_ACCOUNT_TAG,
            version: GENERAL_V2_RECIPIENT_ALLOCATION_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-recipient-allocation-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_TREASURY_LEDGER_ACCOUNT_TAG,
            version: GENERAL_V2_TREASURY_LEDGER_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-treasury-ledger-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_SETTLEMENT_CASH_POT_ACCOUNT_TAG,
            version: GENERAL_V2_SETTLEMENT_CASH_POT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-settlement-cash-pot-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_TAG,
            version: HISTORICAL_STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_VERSION_V1,
        },
        status: AllocationStatus::Withdrawn,
        name: "withdrawn-structured-claim-descriptor-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_TAG,
            version: STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "structured-claim-descriptor-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_FINAL_POT_ACCOUNT_TAG,
            version: GENERAL_V2_FINAL_POT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-final-pot-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_V3_RELEASE_ACCOUNT_TAG,
            version: SOURCE_V3_RELEASE_ACCOUNT_VERSION_V1,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-v3-release-v1-account-historical",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_V3_RELEASE_ACCOUNT_TAG,
            version: SOURCE_V3_RELEASE_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-v3-release-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_V3_HEAD_ACCOUNT_TAG,
            version: SOURCE_V3_HEAD_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-v3-head-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_V3_REOPEN_LINEAGE_ACCOUNT_TAG,
            version: SOURCE_V3_REOPEN_LINEAGE_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-v3-reopen-lineage-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_V3_OPEN_RAW_PAGE_ACCOUNT_TAG,
            version: SOURCE_V3_OPEN_RAW_PAGE_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-v3-open-raw-page-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_V3_RAW_PAGE_ACCOUNT_TAG,
            version: SOURCE_V3_RAW_PAGE_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-v3-raw-page-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_V3_WINDOW_WORK_ACCOUNT_TAG,
            version: SOURCE_V3_WINDOW_WORK_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-v3-window-work-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_V3_WINDOW_SEAL_ACCOUNT_TAG,
            version: SOURCE_V3_WINDOW_SEAL_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-v3-window-seal-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_V3_STATISTIC_RESULT_ACCOUNT_TAG,
            version: SOURCE_V3_STATISTIC_RESULT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-v3-statistic-result-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_V3_WORK_RECEIPT_ACCOUNT_TAG,
            version: SOURCE_V3_WORK_RECEIPT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-v3-work-receipt-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG,
            version: DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "dealer-liveness-schedule-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_STATE_V2_ACCOUNT_TAG,
            version: DEALER_STATE_V2_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "dealer-state-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG,
            version: DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "dealer-funded-dependencies-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_LP_PAGE_V2_ACCOUNT_TAG,
            version: DEALER_LP_PAGE_V2_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "dealer-lp-page-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_LEASE_V2_ACCOUNT_TAG,
            version: DEALER_LEASE_V2_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "dealer-lease-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_SETTLEMENT_POT_V2_ACCOUNT_TAG,
            version: DEALER_SETTLEMENT_POT_V2_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "dealer-settlement-pot-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_EPOCH_BINDING_V2_ACCOUNT_TAG,
            version: DEALER_EPOCH_BINDING_V2_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "dealer-epoch-binding-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_TERMINAL_ALLOCATION_ACCOUNT_TAG,
            version: DEALER_TERMINAL_ALLOCATION_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "dealer-terminal-allocation-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_CLAIM_WORK_ACCOUNT_TAG,
            version: DEALER_CLAIM_WORK_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "dealer-claim-work-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_TAG,
            version: DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "dealer-root-tombstone-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_EXIT_TICKET_ACCOUNT_TAG,
            version: DEALER_EXIT_TICKET_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "dealer-exit-ticket-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FAILURE_EXTERNAL_ROOT_ACCOUNT_TAG,
            version: FAILURE_EXTERNAL_ROOT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "failure-external-root-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FAILURE_EXTERNAL_ROOT_ACCOUNT_TAG,
            version: FAILURE_MARKET_ROOT_ACCOUNT_VERSION_V2,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "failure-market-root-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FAILURE_EXTERNAL_ROOT_ACCOUNT_TAG,
            version: FAILURE_MARKET_RUNTIME_ROOT_ACCOUNT_VERSION_V1,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "failure-market-runtime-root-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
            version: FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "failure-liveness-policy-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
            version: FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "failure-external-recovery-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FAILURE_REPLAY_TOMBSTONE_ACCOUNT_TAG,
            version: FAILURE_REPLAY_TOMBSTONE_ACCOUNT_VERSION,
        },
        status: AllocationStatus::Withdrawn,
        name: "failure-replay-tombstone-v1-withdrawn-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FAILURE_REPLAY_TOMBSTONE_ACCOUNT_TAG,
            version: FAILURE_MARKET_REPLAY_ACCOUNT_VERSION_V2,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "failure-market-replay-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_TAG,
            version: FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_V1_VERSION,
        },
        status: AllocationStatus::Withdrawn,
        name: "fractional-redemption-policy-v1-withdrawn-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_TAG,
            version: FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "fractional-redemption-policy-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FRACTIONAL_REDEMPTION_LEDGER_ACCOUNT_TAG,
            version: FRACTIONAL_REDEMPTION_LEDGER_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "fractional-redemption-ledger-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_TAG,
            version: FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_V1_VERSION,
        },
        status: AllocationStatus::Withdrawn,
        name: "fractional-redemption-credit-v1-withdrawn-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_TAG,
            version: FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "fractional-redemption-credit-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_TAG,
            version: FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_V1_VERSION,
        },
        status: AllocationStatus::Withdrawn,
        name: "fractional-redemption-credit-tombstone-v1-withdrawn-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_TAG,
            version: FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "fractional-redemption-credit-tombstone-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
            version: DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "dealer-action-receipt-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_SETTLEMENT_ROOT_ACCOUNT_TAG,
            version: GENERAL_V2_SETTLEMENT_ROOT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-settlement-root-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_SETTLEMENT_ROOT_ACCOUNT_TAG,
            version: GENERAL_V2_INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-indexed-settlement-root-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: PRODUCT_MARKET_LIFECYCLE_ROOT_ACCOUNT_TAG,
            version: PRODUCT_MARKET_LIFECYCLE_ROOT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "product-market-lifecycle-root-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: DEALER_COVERED_SELECTION_ACCOUNT_TAG,
            version: DEALER_COVERED_SELECTION_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "dealer-covered-selection-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_TAG,
            version: FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_V1_VERSION,
        },
        status: AllocationStatus::Withdrawn,
        name: "failure-interval-consensus-work-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_TAG,
            version: FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_V1_VERSION,
        },
        status: AllocationStatus::Withdrawn,
        name: "failure-interval-consensus-replay-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_TAG,
            version: FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "failure-market-interval-work-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_TAG,
            version: FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "failure-market-interval-history-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: PRODUCT_SERIES_MARKET_LINK_ACCOUNT_TAG,
            version: PRODUCT_SERIES_MARKET_LINK_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "product-series-market-link-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: STRUCTURED_MARKET_ROOT_ACCOUNT_TAG,
            version: STRUCTURED_MARKET_ROOT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "structured-market-root-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: PRODUCT_MARKET_LIFECYCLE_REPLAY_ACCOUNT_TAG,
            version: PRODUCT_MARKET_LIFECYCLE_REPLAY_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "product-market-lifecycle-replay-v1-account",
    },
];

/// One reserved successor intent family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtensionFamily {
    /// General V2 clearing and settlement.
    GeneralV2,
    /// Structured-claim base services.
    StructuredClaim,
    /// Covered-dealer services.
    Dealer,
    /// Source-plane and Series services.
    SourceSeries,
    /// Evidence-only recovery services.
    Recovery,
    /// Exact fractional-redemption and owner-credit services.
    FractionalRedemption,
    /// Current Direct-market lifecycle and settlement services.
    DirectMarket,
}

impl ExtensionFamily {
    /// Return the exact family tag.
    pub const fn tag(self) -> u8 {
        match self {
            Self::GeneralV2 => GENERAL_V2_FAMILY_TAG,
            Self::StructuredClaim => STRUCTURED_CLAIM_FAMILY_TAG,
            Self::Dealer => DEALER_FAMILY_TAG,
            Self::SourceSeries => SOURCE_SERIES_FAMILY_TAG,
            Self::Recovery => RECOVERY_FAMILY_TAG,
            Self::FractionalRedemption => FRACTIONAL_REDEMPTION_FAMILY_TAG,
            Self::DirectMarket => DIRECT_MARKET_FAMILY_TAG,
        }
    }

    /// Return the exact family version.
    pub const fn version(self) -> u8 {
        match self {
            Self::GeneralV2 => GENERAL_V2_FAMILY_VERSION,
            Self::StructuredClaim => STRUCTURED_CLAIM_FAMILY_VERSION,
            Self::Dealer => DEALER_FAMILY_VERSION,
            Self::SourceSeries => SOURCE_SERIES_FAMILY_VERSION,
            Self::Recovery => RECOVERY_FAMILY_VERSION,
            Self::FractionalRedemption => FRACTIONAL_REDEMPTION_FAMILY_VERSION,
            Self::DirectMarket => DIRECT_MARKET_FAMILY_VERSION,
        }
    }

    /// Decode one exact family tag/version pair.
    pub const fn from_wire(tag: u8, version: u8) -> Option<Self> {
        match (tag, version) {
            (GENERAL_V2_FAMILY_TAG, GENERAL_V2_FAMILY_VERSION) => Some(Self::GeneralV2),
            (STRUCTURED_CLAIM_FAMILY_TAG, STRUCTURED_CLAIM_FAMILY_VERSION) => {
                Some(Self::StructuredClaim)
            }
            (DEALER_FAMILY_TAG, DEALER_FAMILY_VERSION) => Some(Self::Dealer),
            (SOURCE_SERIES_FAMILY_TAG, SOURCE_SERIES_FAMILY_VERSION) => Some(Self::SourceSeries),
            (RECOVERY_FAMILY_TAG, RECOVERY_FAMILY_VERSION) => Some(Self::Recovery),
            (FRACTIONAL_REDEMPTION_FAMILY_TAG, FRACTIONAL_REDEMPTION_FAMILY_VERSION) => {
                Some(Self::FractionalRedemption)
            }
            (DIRECT_MARKET_FAMILY_TAG, DIRECT_MARKET_FAMILY_VERSION) => Some(Self::DirectMarket),
            _ => None,
        }
    }

    /// Return this family's status from the central collision ledger.
    ///
    /// Runtime adapters must not infer enablement merely because a family or
    /// action has a typed coordinate. The ledger remains the single owner of
    /// allocation status.
    pub fn allocation_status(self) -> Option<AllocationStatus> {
        for entry in CENTRAL_COLLISION_LEDGER {
            if let AllocationCoordinates::Exact {
                namespace: WireNamespace::MainIntent,
                tag,
                version,
            } = entry.coordinates
            {
                if tag == self.tag() && version == self.version() {
                    return Some(entry.status);
                }
            }
        }
        None
    }
}

/// Classification of one exact intent tag/version pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntentAllocation {
    /// One of the frozen version-3 intent tags `1..=73`.
    LegacyV3,
    /// One reserved successor family.
    Extension(ExtensionFamily),
}

/// Classify an exact intent tag/version pair.
pub const fn classify_intent(tag: u8, version: u8) -> Option<IntentAllocation> {
    if version == LEGACY_INTENT_VERSION
        && tag >= LEGACY_INTENT_FIRST_TAG
        && tag <= LEGACY_INTENT_LAST_TAG
    {
        Some(IntentAllocation::LegacyV3)
    } else {
        match ExtensionFamily::from_wire(tag, version) {
            Some(family) => Some(IntentAllocation::Extension(family)),
            None => None,
        }
    }
}

/// General V2 family-local action allocations.
///
/// These names reserve collision-free local tags.  They deliberately do not
/// define payload codecs or enable any runtime route.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralV2Action {
    /// Create one General V2 market.
    CreateMarket = 1,
    /// Initialize one General V2 epoch.
    InitEpoch = 2,
    /// Initialize one General V2 order page.
    InitOrderPage = 3,
    /// Place one General V2 order.
    PlaceOrder = 4,
    /// Cancel one General V2 order.
    CancelOrder = 5,
    /// Freeze one General V2 epoch.
    FreezeEpoch = 6,
    /// Begin one candidate.
    BeginCandidate = 7,
    /// Write one candidate-feed segment.
    WriteCandidateFeed = 8,
    /// Seal one candidate.
    SealCandidate = 9,
    /// Initialize candidate verification work.
    InitClearWork = 10,
    /// Grow candidate verification work.
    GrowClearWork = 11,
    /// Advance the order phase of candidate verification.
    AdvanceClearOrders = 12,
    /// Advance the slice phase of candidate verification.
    AdvanceClearSlices = 13,
    /// Complete candidate verification.
    CompleteCandidateVerification = 14,
    /// Finalize candidate selection.
    FinalizeSelection = 15,
    /// Expire one candidate.
    ExpireCandidate = 16,
    /// Mark candidate work closed.
    MarkWorkClosed = 17,
    /// Claim a candidate bond.
    ClaimCandidateBond = 18,
    /// Claim candidate work funding.
    ClaimCandidateWork = 19,
    /// Clean up one terminal candidate.
    CleanupCandidate = 20,
    /// Claim the solver entitlement.
    ClaimSolver = 21,
    /// Close one candidate-index page.
    CloseCandidateIndexPage = 22,
    /// Claim unused epoch funding.
    ClaimEpochUnused = 23,
    /// Freeze one settlement entitlement.
    FreezeEntitlement = 24,
    /// Account one authenticated receipt end without delivering Eggs.
    AccountReceiptEnd = 25,
    /// Atomically consume both real ends of one direct Egg receipt.
    ConsumeDirectReceiptEggs = 26,
    /// Close one General V2 receipt.
    CloseReceipt = 27,
    /// Close one General V2 reservation.
    CloseReservation = 28,
    /// Close one General V2 page.
    ClosePage = 29,
    /// Close one General V2 pot.
    ClosePot = 30,
    /// Close one General V2 candidate.
    CloseCandidate = 31,
    /// Close one General V2 clear-work account.
    CloseClearWork = 32,
    /// Close one General V2 epoch.
    CloseEpoch = 33,
    /// Close one position.
    ClosePosition = 34,
    /// Atomically transfer free cash and native Eggs between two Positions.
    TransferPositionAssets = 35,
    /// Atomically split complete-set inventory and consume its real buy end.
    ConsumeVirtualSplitReceiptEggs = 36,
    /// Atomically consume a real sell end and merge complete-set inventory.
    ConsumeVirtualMergeReceiptEggs = 37,
    /// Atomically realize one accounting-complete owner into the cash pot.
    FinalizeOwnerSettlement = 38,
    /// Create the counted candidate-scoped settlement root and exact singleton children.
    InitializeSettlementRoot = 39,
    /// Complete one merge receipt's separately authenticated payment latch.
    FinalizeMergeReceiptPayment = 40,
    /// Release and close one exact zero-fill Reservation.
    ReleaseUnfilledReservation = 41,
    /// Atomically consume one exact full coefficient-portfolio pair.
    ConsumePortfolioPairEggs = 42,
    /// Freeze one nonempty V5 order book under the successor chain.
    FreezeEpochV5 = 43,
    /// Retire one complete coefficient-portfolio archive set.
    RetirePortfolioPairArchives = 44,
    /// Retire both compact exact-index children atomically.
    RetireExactIndexChildren = 45,
    /// Retire the retained Feed after every child liability is discharged.
    RetireRetainedFeed = 46,
    /// Close one terminal indexed SettlementRoot and decrement its Epoch.
    CloseIndexedSettlementRoot = 47,
    /// Close one finalized OwnerSettlement V5 row.
    CloseOwnerSettlementRow = 48,
    /// Close one rent-owned owner fee-finalization account.
    CloseOwnerFeeFinalization = 49,
    /// Consume one authenticated candidate-wide fee terminal receipt.
    RetireSelectedFeeRecord = 50,
    /// Advance one fully discharged counted root from Settling to Retiring.
    BeginSettlementRetirement = 51,
}

/// Exact immutable artifact carried by the Dealer catalog transport.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerCatalogArtifactKindV1 {
    /// Covered-dealer economic policy body.
    Policy = 1,
    /// Fine-grained Dealer action quote schedule.
    LivenessSchedule = 2,
    /// Generic seven-compartment runtime-liveness policy.
    RuntimeLivenessPolicy = 3,
}

impl DealerCatalogArtifactKindV1 {
    pub const fn from_byte(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Policy),
            2 => Some(Self::LivenessSchedule),
            3 => Some(Self::RuntimeLivenessPolicy),
            _ => None,
        }
    }

    pub const fn body_bytes(self) -> usize {
        match self {
            Self::Policy => DEALER_POLICY_BODY_BYTES,
            Self::LivenessSchedule => 372,
            Self::RuntimeLivenessPolicy => 1_132,
        }
    }
}

/// Dealer family-local immutable-catalog transport actions.
///
/// These nonzero wire values deliberately do not reuse the pure Dealer
/// runtime enum's `0..=21` representation. Only `SealPolicy` completes the
/// semantic `CreatePolicy` action; the other values are its bounded transport
/// lifecycle.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerPolicyAction {
    /// Create one exact upload stage.
    BeginPolicy = 1,
    /// Append the next strict fixed-width chunk.
    WritePolicy = 2,
    /// Validate and materialize the selected immutable catalog artifact.
    SealPolicy = 3,
    /// Close an incomplete stage under its stored rent split.
    AbortPolicy = 4,
}

impl DealerPolicyAction {
    /// First allocated Dealer policy action.
    pub const FIRST_TAG: u8 = 1;
    /// Last allocated Dealer policy action.
    pub const LAST_TAG: u8 = 4;

    /// Return the exact local wire tag.
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// Decode one exact allocated local tag.
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::BeginPolicy),
            2 => Some(Self::WritePolicy),
            3 => Some(Self::SealPolicy),
            4 => Some(Self::AbortPolicy),
            _ => None,
        }
    }
}

/// Dealer facility runtime actions following the four policy transport tags.
///
/// These coordinates are allocated but remain capability-disabled. Values
/// `5..=25` map in order to pure runtime actions `Initialize..=Retire`; the
/// pure `CreatePolicy` coordinate is represented only by `SealPolicy` above.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerFacilityAction {
    /// Initialize the facility State, Position, Replay, and funded dependency.
    Initialize = 5,
    /// Create the next counted LP ownership page.
    CreateLpPage = 6,
    /// Contribute one exact LP share bundle.
    Contribute = 7,
    /// Withdraw pre-activation LP funding.
    WithdrawFunding = 8,
    /// Activate the fully funded facility.
    Activate = 9,
    /// Cancel a stale or explicitly halted funding phase.
    CancelFunding = 10,
    /// Refund cancelled sponsor capital.
    RefundCancelledSponsor = 11,
    /// Bind one authenticated General Epoch.
    BindEpoch = 12,
    /// Lapse one unused Dealer Epoch binding.
    LapseEpoch = 13,
    /// Select one lease and atomically begin custody.
    SelectLeaseAndBegin = 14,
    /// Collect one bounded settlement slice.
    Collect = 15,
    /// Deliver one bounded settlement slice.
    Deliver = 16,
    /// Finalize settlement and return custody to Position.
    FinalizeSettlement = 17,
    /// Abort an uncollected lease and restore Position.
    AbortBeforeCollection = 18,
    /// Queue one LP exit.
    QueueExit = 19,
    /// Sponsor-authorized halt.
    SponsorHalt = 20,
    /// Enter exposure-reducing unwind.
    EnterUnwind = 21,
    /// Permissionlessly enter unwind after the deadline.
    TimedClose = 22,
    /// Resolve the terminal facility and allocate LP claims.
    Resolve = 23,
    /// Deliver one LP terminal claim.
    Claim = 24,
    /// Close one terminal counted artifact or root.
    Retire = 25,
}

impl DealerFacilityAction {
    /// First allocated Dealer facility action.
    pub const FIRST_TAG: u8 = 5;
    /// Last allocated Dealer facility action.
    pub const LAST_TAG: u8 = 25;

    /// Return the local action tag.
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// Decode one allocated facility action.
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            5 => Some(Self::Initialize),
            6 => Some(Self::CreateLpPage),
            7 => Some(Self::Contribute),
            8 => Some(Self::WithdrawFunding),
            9 => Some(Self::Activate),
            10 => Some(Self::CancelFunding),
            11 => Some(Self::RefundCancelledSponsor),
            12 => Some(Self::BindEpoch),
            13 => Some(Self::LapseEpoch),
            14 => Some(Self::SelectLeaseAndBegin),
            15 => Some(Self::Collect),
            16 => Some(Self::Deliver),
            17 => Some(Self::FinalizeSettlement),
            18 => Some(Self::AbortBeforeCollection),
            19 => Some(Self::QueueExit),
            20 => Some(Self::SponsorHalt),
            21 => Some(Self::EnterUnwind),
            22 => Some(Self::TimedClose),
            23 => Some(Self::Resolve),
            24 => Some(Self::Claim),
            25 => Some(Self::Retire),
            _ => None,
        }
    }
}

/// Exact body bytes accepted in each staged Dealer-catalog write.
pub const DEALER_POLICY_CHUNK_BYTES: usize = 192;
/// Exact Begin payload bytes: kind/padding, identity, neutral sink, expiry.
pub const DEALER_BEGIN_POLICY_PAYLOAD_BYTES: usize = 8 + 32 + 32 + 8;
/// Exact Write payload bytes: kind/padding, identity, cursor, active length, chunk.
pub const DEALER_WRITE_POLICY_PAYLOAD_BYTES: usize = 8 + 32 + 2 + 2 + DEALER_POLICY_CHUNK_BYTES;
/// Exact typed-identity payload bytes for Seal and Abort.
pub const DEALER_POLICY_ID_PAYLOAD_BYTES: usize = 8 + 32;

const _: () = assert!(DEALER_WRITE_POLICY_PAYLOAD_BYTES <= MAX_EXTENSION_PAYLOAD_BYTES);

impl GeneralV2Action {
    /// First allocated General V2 local action tag.
    pub const FIRST_TAG: u8 = 1;
    /// Last allocated General V2 local action tag.
    pub const LAST_TAG: u8 = 51;

    /// Return the local action tag.
    pub const fn tag(self) -> u8 {
        match self {
            Self::CreateMarket => 1,
            Self::InitEpoch => 2,
            Self::InitOrderPage => 3,
            Self::PlaceOrder => 4,
            Self::CancelOrder => 5,
            Self::FreezeEpoch => 6,
            Self::BeginCandidate => 7,
            Self::WriteCandidateFeed => 8,
            Self::SealCandidate => 9,
            Self::InitClearWork => 10,
            Self::GrowClearWork => 11,
            Self::AdvanceClearOrders => 12,
            Self::AdvanceClearSlices => 13,
            Self::CompleteCandidateVerification => 14,
            Self::FinalizeSelection => 15,
            Self::ExpireCandidate => 16,
            Self::MarkWorkClosed => 17,
            Self::ClaimCandidateBond => 18,
            Self::ClaimCandidateWork => 19,
            Self::CleanupCandidate => 20,
            Self::ClaimSolver => 21,
            Self::CloseCandidateIndexPage => 22,
            Self::ClaimEpochUnused => 23,
            Self::FreezeEntitlement => 24,
            Self::AccountReceiptEnd => 25,
            Self::ConsumeDirectReceiptEggs => 26,
            Self::CloseReceipt => 27,
            Self::CloseReservation => 28,
            Self::ClosePage => 29,
            Self::ClosePot => 30,
            Self::CloseCandidate => 31,
            Self::CloseClearWork => 32,
            Self::CloseEpoch => 33,
            Self::ClosePosition => 34,
            Self::TransferPositionAssets => 35,
            Self::ConsumeVirtualSplitReceiptEggs => 36,
            Self::ConsumeVirtualMergeReceiptEggs => 37,
            Self::FinalizeOwnerSettlement => 38,
            Self::InitializeSettlementRoot => 39,
            Self::FinalizeMergeReceiptPayment => 40,
            Self::ReleaseUnfilledReservation => 41,
            Self::ConsumePortfolioPairEggs => 42,
            Self::FreezeEpochV5 => 43,
            Self::RetirePortfolioPairArchives => 44,
            Self::RetireExactIndexChildren => 45,
            Self::RetireRetainedFeed => 46,
            Self::CloseIndexedSettlementRoot => 47,
            Self::CloseOwnerSettlementRow => 48,
            Self::CloseOwnerFeeFinalization => 49,
            Self::RetireSelectedFeeRecord => 50,
            Self::BeginSettlementRetirement => 51,
        }
    }

    /// Decode one allocated General V2 local action tag.
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::CreateMarket),
            2 => Some(Self::InitEpoch),
            3 => Some(Self::InitOrderPage),
            4 => Some(Self::PlaceOrder),
            5 => Some(Self::CancelOrder),
            6 => Some(Self::FreezeEpoch),
            7 => Some(Self::BeginCandidate),
            8 => Some(Self::WriteCandidateFeed),
            9 => Some(Self::SealCandidate),
            10 => Some(Self::InitClearWork),
            11 => Some(Self::GrowClearWork),
            12 => Some(Self::AdvanceClearOrders),
            13 => Some(Self::AdvanceClearSlices),
            14 => Some(Self::CompleteCandidateVerification),
            15 => Some(Self::FinalizeSelection),
            16 => Some(Self::ExpireCandidate),
            17 => Some(Self::MarkWorkClosed),
            18 => Some(Self::ClaimCandidateBond),
            19 => Some(Self::ClaimCandidateWork),
            20 => Some(Self::CleanupCandidate),
            21 => Some(Self::ClaimSolver),
            22 => Some(Self::CloseCandidateIndexPage),
            23 => Some(Self::ClaimEpochUnused),
            24 => Some(Self::FreezeEntitlement),
            25 => Some(Self::AccountReceiptEnd),
            26 => Some(Self::ConsumeDirectReceiptEggs),
            27 => Some(Self::CloseReceipt),
            28 => Some(Self::CloseReservation),
            29 => Some(Self::ClosePage),
            30 => Some(Self::ClosePot),
            31 => Some(Self::CloseCandidate),
            32 => Some(Self::CloseClearWork),
            33 => Some(Self::CloseEpoch),
            34 => Some(Self::ClosePosition),
            35 => Some(Self::TransferPositionAssets),
            36 => Some(Self::ConsumeVirtualSplitReceiptEggs),
            37 => Some(Self::ConsumeVirtualMergeReceiptEggs),
            38 => Some(Self::FinalizeOwnerSettlement),
            39 => Some(Self::InitializeSettlementRoot),
            40 => Some(Self::FinalizeMergeReceiptPayment),
            41 => Some(Self::ReleaseUnfilledReservation),
            42 => Some(Self::ConsumePortfolioPairEggs),
            43 => Some(Self::FreezeEpochV5),
            44 => Some(Self::RetirePortfolioPairArchives),
            45 => Some(Self::RetireExactIndexChildren),
            46 => Some(Self::RetireRetainedFeed),
            47 => Some(Self::CloseIndexedSettlementRoot),
            48 => Some(Self::CloseOwnerSettlementRow),
            49 => Some(Self::CloseOwnerFeeFinalization),
            50 => Some(Self::RetireSelectedFeeRecord),
            51 => Some(Self::BeginSettlementRetirement),
            _ => None,
        }
    }
}

/// StructuredClaim family-local action allocations at `75/1`.
///
/// These tags reserve wire identity only. Every route remains capability
/// disabled until its exact payload, account, CPI, and release contracts are
/// admitted together.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredClaimAction {
    /// Create one immutable structured-claim descriptor.
    CreateDescriptor = 1,
    /// Wrap one canonical complete-set lot.
    WrapCanonical = 2,
    /// Wrap one explicit full-vector lot.
    WrapFull = 3,
    /// Unwrap one canonical complete-set lot.
    UnwrapCanonical = 4,
    /// Unwrap one explicit full-vector lot.
    UnwrapFull = 5,
    /// Compact separately observed donation residue.
    CompactDonation = 6,
    /// Redeem one terminal structured-claim lot.
    RedeemTerminal = 7,
    /// Retire one descriptor after its authenticated base vault close.
    RetireDescriptor = 8,
}

impl StructuredClaimAction {
    /// First allocated StructuredClaim local action tag.
    pub const FIRST_TAG: u8 = 1;
    /// Last allocated StructuredClaim local action tag.
    pub const LAST_TAG: u8 = 8;

    /// Return the local action tag.
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// Decode one allocated StructuredClaim local action tag.
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::CreateDescriptor),
            2 => Some(Self::WrapCanonical),
            3 => Some(Self::WrapFull),
            4 => Some(Self::UnwrapCanonical),
            5 => Some(Self::UnwrapFull),
            6 => Some(Self::CompactDonation),
            7 => Some(Self::RedeemTerminal),
            8 => Some(Self::RetireDescriptor),
            _ => None,
        }
    }
}

/// SourcePlane V3 family-local action allocations inside SourceSeries 77/v2.
///
/// Tags 13 and above are disjoint Series-owned coordinates. These Source tags
/// reserve wire identity only; SBF capability remains disabled independently.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceSeriesAction {
    /// Register one immutable reviewed Source release.
    RegisterRelease = 1,
    /// Initialize one authenticated SourceHead generation.
    InitializeHead = 2,
    /// Open one state-assigned mutable raw page.
    OpenRawPage = 3,
    /// Authenticate and append one bounded consecutive boundary batch.
    IngestBoundaryBatch = 4,
    /// Seal one immutable raw page and advance SourceHead.
    SealRawPage = 5,
    /// Initialize one predictable WindowWork account.
    InitializeWindowWork = 6,
    /// Fold one bounded raw-page batch into WindowWork.
    FoldWindowPages = 7,
    /// Seal one mature Window and its immutable evidence.
    SealWindow = 8,
    /// Invoke one reviewed evaluator and persist its exact result.
    EvaluateStatistic = 9,
    /// Emit one typed source-owned failure-policy handoff.
    EmitFailureHandoff = 10,
    /// Reopen the exact next durable Source generation.
    ReopenGeneration = 11,
    /// Close one terminal Source generation with exact rent disposition.
    CloseGeneration = 12,
}

impl SourceSeriesAction {
    /// First Source-owned local action tag.
    pub const FIRST_TAG: u8 = 1;
    /// Last Source-owned local action tag.
    pub const LAST_TAG: u8 = 12;

    /// Return the local action tag.
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// Decode one Source-owned local action tag.
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::RegisterRelease),
            2 => Some(Self::InitializeHead),
            3 => Some(Self::OpenRawPage),
            4 => Some(Self::IngestBoundaryBatch),
            5 => Some(Self::SealRawPage),
            6 => Some(Self::InitializeWindowWork),
            7 => Some(Self::FoldWindowPages),
            8 => Some(Self::SealWindow),
            9 => Some(Self::EvaluateStatistic),
            10 => Some(Self::EmitFailureHandoff),
            11 => Some(Self::ReopenGeneration),
            12 => Some(Self::CloseGeneration),
            _ => None,
        }
    }
}

/// Recurring-Series action allocations in the shared SourceSeries 77/v2 family.
///
/// SourcePlane V3 exclusively owns the disjoint [`SourceSeriesAction`] range.
/// These Series tags reserve wire identity only; capability remains separate.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecurringSeriesAction {
    /// Register one persistent V5 Series against an authenticated registry release.
    RegisterSeries = 13,
    /// Capitalize six lamport compartments and five collateral vaults.
    ActivateFunding = 14,
    /// Create or converge the next eligible occurrence atomically.
    AdvanceOccurrence = 15,
    /// Advance one elapsed ordinal without spending its allocation.
    LapseOccurrence = 16,
    /// Observe balance surplus as separately owned donation residue.
    ObserveDonation = 17,
    /// Refund remaining payer principal and dispose donation residue.
    CloseFunding = 18,
}

impl RecurringSeriesAction {
    /// First recurring-Series local action tag.
    pub const FIRST_TAG: u8 = 13;
    /// Last recurring-Series local action tag.
    pub const LAST_TAG: u8 = 18;

    /// Return the local action tag.
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// Decode one recurring-Series local action tag.
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            13 => Some(Self::RegisterSeries),
            14 => Some(Self::ActivateFunding),
            15 => Some(Self::AdvanceOccurrence),
            16 => Some(Self::LapseOccurrence),
            17 => Some(Self::ObserveDonation),
            18 => Some(Self::CloseFunding),
            _ => None,
        }
    }
}

/// Evidence-only Recovery family-local action allocations inside 78/v1.
///
/// These coordinates freeze payload/account contracts only. Every capability
/// remains disabled until an atomic release review promotes the whole family.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryAction {
    /// Create the semantic root after exact liveness funding is present.
    InitializeFailureRoot = 1,
    /// Record one authenticated source-owned failure trigger.
    TriggerSourceFailure = 2,
    /// Record the frozen evidence relation's deterministic refusal.
    TriggerRelationRefusal = 3,
    /// Advance the finite immutable repair schedule.
    AdvanceRecoverySchedule = 4,
    /// Accept one source-authenticated repair unit and spend liveness work.
    AcceptRecoveryWork = 5,
    /// Resolve from caller-funded accepted evidence.
    ResolveCallerFunded = 6,
    /// Resolve while paying one final source-authenticated repair unit.
    ResolvePaidRecovery = 7,
    /// Close only the external liveness Recovery compartment.
    CloseRecoveryFunding = 8,
    /// Close the resolved semantic root after retirement/source/replay joins.
    CloseFailureRoot = 9,
    /// Create one dedicated exhaustive interval-consensus work lifecycle.
    BeginIntervalConsensus = 10,
    /// Evaluate one bounded exact coordinate chunk and pay through liveness.
    AdvanceIntervalConsensus = 11,
    /// Restore the authenticated Product capability and resolve atomically.
    ResolveIntervalConsensus = 12,
    /// Close deletable consensus work while retaining permanent replay.
    CloseIntervalConsensusWork = 13,
}

impl RecoveryAction {
    /// First Recovery-owned local action tag.
    pub const FIRST_TAG: u8 = 1;
    /// Last Recovery-owned local action tag.
    pub const LAST_TAG: u8 = 13;

    /// Return the family-local action tag.
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// Decode one Recovery-owned action tag.
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::InitializeFailureRoot),
            2 => Some(Self::TriggerSourceFailure),
            3 => Some(Self::TriggerRelationRefusal),
            4 => Some(Self::AdvanceRecoverySchedule),
            5 => Some(Self::AcceptRecoveryWork),
            6 => Some(Self::ResolveCallerFunded),
            7 => Some(Self::ResolvePaidRecovery),
            8 => Some(Self::CloseRecoveryFunding),
            9 => Some(Self::CloseFailureRoot),
            10 => Some(Self::BeginIntervalConsensus),
            11 => Some(Self::AdvanceIntervalConsensus),
            12 => Some(Self::ResolveIntervalConsensus),
            13 => Some(Self::CloseIntervalConsensusWork),
            _ => None,
        }
    }
}

/// Exact fractional-redemption family-local actions inside 79/v1.
///
/// Coordinates freeze the runtime contract while every tuple remains
/// capability-disabled. Activation must join Resolution, ClaimLedger V3,
/// Hoard V2, Position/Replay V3, Realm collateral, Token-2022 claims, and rent
/// atomically.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalRedemptionAction {
    /// Create the immutable policy and sole aggregate-credit ledger.
    Initialize = 1,
    /// Redeem an exact internal lot without owner-credit state.
    RedeemInternalExact = 2,
    /// Redeem an exact bearer lot without owner-credit state.
    RedeemBearerExact = 3,
    /// Redeem arbitrary internal claims into exact owner credit.
    RedeemInternalCredit = 4,
    /// Redeem arbitrary bearer claims into exact owner credit.
    RedeemBearerCredit = 5,
    /// Transfer an explicit numerator between owner credits.
    TransferCredit = 6,
    /// Merge one entire source residue into a destination credit.
    MergeCredit = 7,
    /// Close one zero credit into its permanent tombstone.
    CloseZeroCredit = 8,
    /// Seal canonical native supply exhausted without sweeping backing.
    SealClaimsExhausted = 9,
    /// Close a claims/credit/backing-empty fractional domain under Product root.
    CloseEmptyLedger = 10,
}

impl FractionalRedemptionAction {
    /// First fractional-redemption local action.
    pub const FIRST_TAG: u8 = 1;
    /// Last fractional-redemption local action.
    pub const LAST_TAG: u8 = 10;

    /// Return the family-local tag.
    pub const fn tag(self) -> u8 {
        match self {
            Self::Initialize => 1,
            Self::RedeemInternalExact => 2,
            Self::RedeemBearerExact => 3,
            Self::RedeemInternalCredit => 4,
            Self::RedeemBearerCredit => 5,
            Self::TransferCredit => 6,
            Self::MergeCredit => 7,
            Self::CloseZeroCredit => 8,
            Self::SealClaimsExhausted => 9,
            Self::CloseEmptyLedger => 10,
        }
    }

    /// Decode one exact family-local action.
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Initialize),
            2 => Some(Self::RedeemInternalExact),
            3 => Some(Self::RedeemBearerExact),
            4 => Some(Self::RedeemInternalCredit),
            5 => Some(Self::RedeemBearerCredit),
            6 => Some(Self::TransferCredit),
            7 => Some(Self::MergeCredit),
            8 => Some(Self::CloseZeroCredit),
            9 => Some(Self::SealClaimsExhausted),
            10 => Some(Self::CloseEmptyLedger),
            _ => None,
        }
    }
}

/// Current Direct-market family-local action allocations inside `80/1`.
///
/// These coordinates remain capability-disabled until all thirteen actions,
/// their Product family joins, and their bounded SBF frames are admitted as
/// one release unit. They never reuse legacy Direct V3 tags `36..=46`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectMarketAction {
    /// Create the root and permanent replay while admitting one Product child.
    InitializeMarket = 1,
    /// Admit one of the exact zero-to-two funded order Reservations.
    AdmitOrder = 2,
    /// Cancel one pre-freeze Reservation and return its locked liabilities.
    CancelOrder = 3,
    /// Freeze the exhaustive Reservation prefix into one immutable book.
    FreezeBook = 4,
    /// Submit one bounded valid RelationV2 candidate.
    SubmitCandidate = 5,
    /// Seal submissions and initialize exhaustive verification traversal.
    BeginVerification = 6,
    /// Verify the next canonical retained candidate coordinate.
    VerifyCandidate = 7,
    /// Select the best valid submitted candidate or the exact empty result.
    FinalizeSelection = 8,
    /// Atomically settle the selected Egg/cash pair and both GEN1 replays.
    SettlePair = 9,
    /// Terminalize a frozen epoch with no submitted candidate.
    LapseEmpty = 10,
    /// Terminalize a nonempty epoch whose verification never selected.
    LapseUnselected = 11,
    /// Terminalize selected authority after its settlement deadline.
    LapseSelected = 12,
    /// Retire all transient state and terminalize the Product Direct child.
    RetireTerminal = 13,
}

impl DirectMarketAction {
    /// First Direct-market local action.
    pub const FIRST_TAG: u8 = 1;
    /// Last Direct-market local action.
    pub const LAST_TAG: u8 = 13;

    /// Return the exact family-local tag without an unchecked cast.
    pub const fn tag(self) -> u8 {
        match self {
            Self::InitializeMarket => 1,
            Self::AdmitOrder => 2,
            Self::CancelOrder => 3,
            Self::FreezeBook => 4,
            Self::SubmitCandidate => 5,
            Self::BeginVerification => 6,
            Self::VerifyCandidate => 7,
            Self::FinalizeSelection => 8,
            Self::SettlePair => 9,
            Self::LapseEmpty => 10,
            Self::LapseUnselected => 11,
            Self::LapseSelected => 12,
            Self::RetireTerminal => 13,
        }
    }

    /// Decode one exact allocated local tag.
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::InitializeMarket),
            2 => Some(Self::AdmitOrder),
            3 => Some(Self::CancelOrder),
            4 => Some(Self::FreezeBook),
            5 => Some(Self::SubmitCandidate),
            6 => Some(Self::BeginVerification),
            7 => Some(Self::VerifyCandidate),
            8 => Some(Self::FinalizeSelection),
            9 => Some(Self::SettlePair),
            10 => Some(Self::LapseEmpty),
            11 => Some(Self::LapseUnselected),
            12 => Some(Self::LapseSelected),
            13 => Some(Self::RetireTerminal),
            _ => None,
        }
    }
}

/// One allocated successor family-local action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtensionAction {
    /// One General V2 local action.
    GeneralV2(GeneralV2Action),
    /// One Dealer policy-catalog transport action.
    DealerPolicy(DealerPolicyAction),
    /// One capability-disabled Dealer facility action.
    DealerFacility(DealerFacilityAction),
    /// One StructuredClaim local action.
    StructuredClaim(StructuredClaimAction),
    /// One SourcePlane V3 action in the shared SourceSeries family.
    SourceV3(SourceSeriesAction),
    /// One recurring-Series action in the shared SourceSeries family.
    RecurringSeries(RecurringSeriesAction),
    /// One evidence-only Recovery action.
    Recovery(RecoveryAction),
    /// One exact fractional-redemption action.
    FractionalRedemption(FractionalRedemptionAction),
    /// One current Direct-market action.
    DirectMarket(DirectMarketAction),
}

impl ExtensionAction {
    /// Return the centrally allocated family containing this action.
    pub const fn family(self) -> ExtensionFamily {
        match self {
            Self::GeneralV2(_) => ExtensionFamily::GeneralV2,
            Self::DealerPolicy(_) | Self::DealerFacility(_) => ExtensionFamily::Dealer,
            Self::StructuredClaim(_) => ExtensionFamily::StructuredClaim,
            Self::SourceV3(_) | Self::RecurringSeries(_) => ExtensionFamily::SourceSeries,
            Self::Recovery(_) => ExtensionFamily::Recovery,
            Self::FractionalRedemption(_) => ExtensionFamily::FractionalRedemption,
            Self::DirectMarket(_) => ExtensionFamily::DirectMarket,
        }
    }

    /// Return the action's local tag.
    pub const fn local_tag(self) -> u8 {
        match self {
            Self::GeneralV2(action) => action.tag(),
            Self::DealerPolicy(action) => action.tag(),
            Self::DealerFacility(action) => action.tag(),
            Self::StructuredClaim(action) => action.tag(),
            Self::SourceV3(action) => action.tag(),
            Self::RecurringSeries(action) => action.tag(),
            Self::Recovery(action) => action.tag(),
            Self::FractionalRedemption(action) => action.tag(),
            Self::DirectMarket(action) => action.tag(),
        }
    }
}

/// Errors from the versioned successor-family envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryError {
    /// The envelope ended before tag, version, and local action were present.
    Truncated,
    /// The envelope exceeded [`MAX_INTENT_BYTES`].
    TooLong,
    /// The tag/version pair is not one reserved successor family.
    UnknownFamilyVersion,
    /// The local action is not allocated inside that exact family/version.
    UnknownLocalAction,
    /// The caller's output buffer cannot hold the exact envelope.
    OutputTooSmall,
}

/// Decode one action only under an exact reserved family/version pair.
pub const fn decode_extension_action(
    family_tag: u8,
    family_version: u8,
    local_action: u8,
) -> core::result::Result<ExtensionAction, RegistryError> {
    match ExtensionFamily::from_wire(family_tag, family_version) {
        Some(ExtensionFamily::GeneralV2) => match GeneralV2Action::from_tag(local_action) {
            Some(action) => Ok(ExtensionAction::GeneralV2(action)),
            None => Err(RegistryError::UnknownLocalAction),
        },
        Some(ExtensionFamily::StructuredClaim) => {
            match StructuredClaimAction::from_tag(local_action) {
                Some(action) => Ok(ExtensionAction::StructuredClaim(action)),
                None => Err(RegistryError::UnknownLocalAction),
            }
        }
        Some(ExtensionFamily::Dealer) => match DealerPolicyAction::from_tag(local_action) {
            Some(action) => Ok(ExtensionAction::DealerPolicy(action)),
            None => match DealerFacilityAction::from_tag(local_action) {
                Some(action) => Ok(ExtensionAction::DealerFacility(action)),
                None => Err(RegistryError::UnknownLocalAction),
            },
        },
        Some(ExtensionFamily::SourceSeries) => match SourceSeriesAction::from_tag(local_action) {
            Some(action) => Ok(ExtensionAction::SourceV3(action)),
            None => match RecurringSeriesAction::from_tag(local_action) {
                Some(action) => Ok(ExtensionAction::RecurringSeries(action)),
                None => Err(RegistryError::UnknownLocalAction),
            },
        },
        Some(ExtensionFamily::Recovery) => match RecoveryAction::from_tag(local_action) {
            Some(action) => Ok(ExtensionAction::Recovery(action)),
            None => Err(RegistryError::UnknownLocalAction),
        },
        Some(ExtensionFamily::FractionalRedemption) => {
            match FractionalRedemptionAction::from_tag(local_action) {
                Some(action) => Ok(ExtensionAction::FractionalRedemption(action)),
                None => Err(RegistryError::UnknownLocalAction),
            }
        }
        Some(ExtensionFamily::DirectMarket) => match DirectMarketAction::from_tag(local_action) {
            Some(action) => Ok(ExtensionAction::DirectMarket(action)),
            None => Err(RegistryError::UnknownLocalAction),
        },
        None => Err(RegistryError::UnknownFamilyVersion),
    }
}

/// Borrowed versioned successor-family envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtensionEnvelope<'a> {
    /// Exact reserved family.
    pub family: ExtensionFamily,
    /// Exact allocated family-local action.
    pub action: ExtensionAction,
    /// Action-owned bytes, whose codec is intentionally not frozen here.
    pub payload: &'a [u8],
}

impl<'a> ExtensionEnvelope<'a> {
    /// Validate the exact family/action pairing and unchanged packet ceiling.
    pub fn validate(&self) -> core::result::Result<(), RegistryError> {
        if self.encoded_len() > MAX_INTENT_BYTES {
            return Err(RegistryError::TooLong);
        }
        let decoded = decode_extension_action(
            self.family.tag(),
            self.family.version(),
            self.action.local_tag(),
        )?;
        if decoded != self.action {
            return Err(RegistryError::UnknownLocalAction);
        }
        Ok(())
    }

    /// Decode a strict successor envelope without accepting it as a legacy [`super::Intent`].
    pub fn decode(bytes: &'a [u8]) -> core::result::Result<Self, RegistryError> {
        if bytes.len() > MAX_INTENT_BYTES {
            return Err(RegistryError::TooLong);
        }
        if bytes.len() < EXTENSION_ENVELOPE_BYTES {
            return Err(RegistryError::Truncated);
        }
        let family = ExtensionFamily::from_wire(bytes[0], bytes[1])
            .ok_or(RegistryError::UnknownFamilyVersion)?;
        let action = decode_extension_action(bytes[0], bytes[1], bytes[2])?;
        Ok(Self {
            family,
            action,
            payload: &bytes[EXTENSION_ENVELOPE_BYTES..],
        })
    }

    /// Exact encoded length under the unchanged legacy intent ceiling.
    pub const fn encoded_len(&self) -> usize {
        EXTENSION_ENVELOPE_BYTES + self.payload.len()
    }

    /// Encode the strict family/version/action prefix and borrowed payload.
    pub fn encode(&self, out: &mut [u8]) -> core::result::Result<usize, RegistryError> {
        self.validate()?;
        let exact = self.encoded_len();
        if out.len() < exact {
            return Err(RegistryError::OutputTooSmall);
        }
        out[0] = self.family.tag();
        out[1] = self.family.version();
        out[2] = self.action.local_tag();
        out[EXTENSION_ENVELOPE_BYTES..exact].copy_from_slice(self.payload);
        Ok(exact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coordinates_include(
        coordinates: AllocationCoordinates,
        namespace: WireNamespace,
        tag: u8,
        version: u8,
    ) -> bool {
        match coordinates {
            AllocationCoordinates::Range {
                namespace: candidate_namespace,
                first_tag,
                last_tag,
                version: candidate_version,
            } => {
                candidate_namespace == namespace
                    && candidate_version == version
                    && tag >= first_tag
                    && tag <= last_tag
            }
            AllocationCoordinates::Exact {
                namespace: candidate_namespace,
                tag: candidate_tag,
                version: candidate_version,
            } => {
                candidate_namespace == namespace
                    && candidate_tag == tag
                    && candidate_version == version
            }
        }
    }

    #[test]
    fn every_intent_tag_version_pair_has_exactly_one_expected_classification() {
        for tag in u8::MIN..=u8::MAX {
            for version in u8::MIN..=u8::MAX {
                let expected = if version == LEGACY_INTENT_VERSION
                    && (LEGACY_INTENT_FIRST_TAG..=LEGACY_INTENT_LAST_TAG).contains(&tag)
                {
                    Some(IntentAllocation::LegacyV3)
                } else {
                    match (tag, version) {
                        (74, 1) => Some(IntentAllocation::Extension(ExtensionFamily::GeneralV2)),
                        (75, 1) => Some(IntentAllocation::Extension(
                            ExtensionFamily::StructuredClaim,
                        )),
                        (76, 1) => Some(IntentAllocation::Extension(ExtensionFamily::Dealer)),
                        (77, 2) => Some(IntentAllocation::Extension(ExtensionFamily::SourceSeries)),
                        (78, 1) => Some(IntentAllocation::Extension(ExtensionFamily::Recovery)),
                        (79, 1) => Some(IntentAllocation::Extension(
                            ExtensionFamily::FractionalRedemption,
                        )),
                        (80, 1) => Some(IntentAllocation::Extension(
                            ExtensionFamily::DirectMarket,
                        )),
                        _ => None,
                    }
                };
                assert_eq!(classify_intent(tag, version), expected, "{tag}/{version}");
            }
        }
    }

    #[test]
    fn collision_ledger_is_disjoint_inside_each_namespace() {
        for namespace in [WireNamespace::MainIntent, WireNamespace::MainAccount] {
            for tag in u8::MIN..=u8::MAX {
                for version in u8::MIN..=u8::MAX {
                    let mut occupants = 0_u8;
                    for entry in CENTRAL_COLLISION_LEDGER {
                        if coordinates_include(entry.coordinates, namespace, tag, version) {
                            occupants += 1;
                        }
                    }
                    assert!(occupants <= 1, "collision at {namespace:?}/{tag}/{version}");
                }
            }
        }
    }

    #[test]
    fn every_general_v2_account_coordinate_is_reserved_but_disabled() {
        let expected = [
            (
                GENERAL_RESERVATION_V9_ACCOUNT_TAG,
                GENERAL_RESERVATION_V9_ACCOUNT_VERSION,
            ),
            (
                GENERAL_ORDER_PAGE_V5_ACCOUNT_TAG,
                GENERAL_ORDER_PAGE_V5_ACCOUNT_VERSION,
            ),
            (
                GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_TAG,
                GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_MARKET_RUNTIME_ACCOUNT_TAG,
                GENERAL_V2_MARKET_RUNTIME_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_EPOCH_ACCOUNT_TAG,
                GENERAL_V2_EPOCH_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_CLEAR_WORK_ACCOUNT_TAG,
                GENERAL_V2_CLEAR_WORK_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_CLEAR_WORK_ACCOUNT_TAG,
                GENERAL_V2_CLEAR_WORK_ACCOUNT_VERSION_V3,
            ),
            (GENERAL_V2_FEED_ACCOUNT_TAG, GENERAL_V2_FEED_ACCOUNT_VERSION),
            (
                GENERAL_V2_WINDOW_ACCOUNT_TAG,
                GENERAL_V2_WINDOW_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_WINDOW_ACCOUNT_TAG,
                GENERAL_V2_WINDOW_ACCOUNT_VERSION_V2,
            ),
            (
                GENERAL_V2_FEED_STAGE_ACCOUNT_TAG,
                GENERAL_V2_FEED_STAGE_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_ADMISSION_NODE_ACCOUNT_TAG,
                GENERAL_V2_ADMISSION_NODE_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_ADMISSION_NODE_ACCOUNT_TAG,
                GENERAL_V2_ADMISSION_NODE_ACCOUNT_VERSION_V2,
            ),
            (
                GENERAL_V2_EPOCH_BUDGET_ACCOUNT_TAG,
                GENERAL_V2_EPOCH_BUDGET_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_MARKET_BINDING_ACCOUNT_TAG,
                GENERAL_V2_MARKET_BINDING_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_MARKET_BINDING_ACCOUNT_TAG,
                GENERAL_V2_MARKET_BINDING_ACCOUNT_VERSION_V2,
            ),
            (
                REPLAY_SUCCESSOR_ACCOUNT_TAG,
                REPLAY_SUCCESSOR_ACCOUNT_VERSION,
            ),
            (
                PURPOSE_REPLAY_V3_ACCOUNT_TAG,
                PURPOSE_REPLAY_V3_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_ECONOMIC_DOMAIN_ACCOUNT_TAG,
                GENERAL_V2_ECONOMIC_DOMAIN_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_SELECTED_CANDIDATE_ACCOUNT_TAG,
                GENERAL_V2_SELECTED_CANDIDATE_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V5,
            ),
            (
                GENERAL_V2_SELECTED_FEE_RECORD_ACCOUNT_TAG,
                GENERAL_V2_SELECTED_FEE_RECORD_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_OWNER_FEE_CARRY_ACCOUNT_TAG,
                GENERAL_V2_OWNER_FEE_CARRY_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_OWNER_FEE_CARRY_ACCOUNT_TAG,
                GENERAL_V2_OWNER_FEE_FINALIZATION_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_PAYER_ALLOCATION_ACCOUNT_TAG,
                GENERAL_V2_PAYER_ALLOCATION_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_RECIPIENT_ALLOCATION_ACCOUNT_TAG,
                GENERAL_V2_RECIPIENT_ALLOCATION_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_TREASURY_LEDGER_ACCOUNT_TAG,
                GENERAL_V2_TREASURY_LEDGER_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_SETTLEMENT_CASH_POT_ACCOUNT_TAG,
                GENERAL_V2_SETTLEMENT_CASH_POT_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_FINAL_POT_ACCOUNT_TAG,
                GENERAL_V2_FINAL_POT_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_SETTLEMENT_ROOT_ACCOUNT_TAG,
                GENERAL_V2_SETTLEMENT_ROOT_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_SETTLEMENT_ROOT_ACCOUNT_TAG,
                GENERAL_V2_INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_TAG,
                GENERAL_V2_FROZEN_ORDER_LOCATOR_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_TAG,
                GENERAL_V2_CANDIDATE_ADJACENCY_ACCOUNT_VERSION,
            ),
        ];
        for (tag, version) in expected {
            let mut matches = 0u8;
            let mut status = None;
            for entry in CENTRAL_COLLISION_LEDGER {
                if coordinates_include(entry.coordinates, WireNamespace::MainAccount, tag, version)
                {
                    matches += 1;
                    status = Some(entry.status);
                }
            }
            assert_eq!(matches, 1, "account {tag}/{version}");
            assert_eq!(status, Some(AllocationStatus::ReservedDisabled));
        }
    }

    #[test]
    fn reservation_history_is_not_reinterpreted_by_the_rent_owned_successor() {
        let expected = [
            (
                GENERAL_RESERVATION_V5_ACCOUNT_TAG,
                GENERAL_RESERVATION_V5_ACCOUNT_VERSION,
                AllocationStatus::Withdrawn,
            ),
            (
                GENERAL_RESERVATION_V7_ACCOUNT_TAG,
                GENERAL_RESERVATION_V7_ACCOUNT_VERSION,
                AllocationStatus::Withdrawn,
            ),
            (
                GENERAL_RESERVATION_V9_ACCOUNT_TAG,
                GENERAL_RESERVATION_V9_ACCOUNT_VERSION,
                AllocationStatus::ReservedDisabled,
            ),
        ];
        for (tag, version, expected_status) in expected {
            let matches: Vec<_> = CENTRAL_COLLISION_LEDGER
                .iter()
                .filter(|entry| {
                    coordinates_include(entry.coordinates, WireNamespace::MainAccount, tag, version)
                })
                .collect();
            assert_eq!(matches.len(), 1, "reservation {tag}/{version}");
            assert_eq!(matches[0].status, expected_status);
        }
    }

    #[test]
    fn settlement_history_is_withdrawn_before_rent_owned_successors() {
        let withdrawn = [
            (
                GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_TAG,
                GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_VERSION,
            ),
            (
                GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_TAG,
                GENERAL_SETTLEMENT_RECEIPT_V4_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V1,
            ),
            (
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V2,
            ),
            (
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V3,
            ),
            (
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V4,
            ),
        ];
        for (tag, version) in withdrawn {
            let matching = CENTRAL_COLLISION_LEDGER.iter().find(|entry| {
                coordinates_include(entry.coordinates, WireNamespace::MainAccount, tag, version)
            });
            assert_eq!(
                matching.map(|entry| entry.status),
                Some(AllocationStatus::Withdrawn),
                "historical settlement coordinate {tag}/{version}"
            );
        }
        for (tag, version) in [
            (
                GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_TAG,
                GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V5,
            ),
        ] {
            let matching = CENTRAL_COLLISION_LEDGER.iter().find(|entry| {
                coordinates_include(entry.coordinates, WireNamespace::MainAccount, tag, version)
            });
            assert_eq!(
                matching.map(|entry| entry.status),
                Some(AllocationStatus::ReservedDisabled),
                "rent-owned settlement coordinate {tag}/{version}"
            );
        }
    }

    #[test]
    fn coordinated_post_selected_account_block_is_complete_and_disabled() {
        let expected = [
            (
                DEALER_POLICY_STAGE_ACCOUNT_TAG,
                DEALER_POLICY_STAGE_ACCOUNT_VERSION,
            ),
            (DEALER_POLICY_ACCOUNT_TAG, DEALER_POLICY_ACCOUNT_VERSION),
            (
                SOURCE_SERIES_REGISTRY_ACCOUNT_TAG,
                SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V2,
            ),
            (
                SOURCE_SERIES_FUNDING_ACCOUNT_TAG,
                SOURCE_SERIES_FUNDING_ACCOUNT_VERSION_V2,
            ),
            (
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V5,
            ),
            (
                GENERAL_V2_SELECTED_FEE_RECORD_ACCOUNT_TAG,
                GENERAL_V2_SELECTED_FEE_RECORD_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_OWNER_FEE_CARRY_ACCOUNT_TAG,
                GENERAL_V2_OWNER_FEE_CARRY_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_OWNER_FEE_CARRY_ACCOUNT_TAG,
                GENERAL_V2_OWNER_FEE_FINALIZATION_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_PAYER_ALLOCATION_ACCOUNT_TAG,
                GENERAL_V2_PAYER_ALLOCATION_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_RECIPIENT_ALLOCATION_ACCOUNT_TAG,
                GENERAL_V2_RECIPIENT_ALLOCATION_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_TREASURY_LEDGER_ACCOUNT_TAG,
                GENERAL_V2_TREASURY_LEDGER_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_SETTLEMENT_CASH_POT_ACCOUNT_TAG,
                GENERAL_V2_SETTLEMENT_CASH_POT_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_FINAL_POT_ACCOUNT_TAG,
                GENERAL_V2_FINAL_POT_ACCOUNT_VERSION,
            ),
        ];
        for (tag, version) in expected {
            let matching = CENTRAL_COLLISION_LEDGER.iter().find(|entry| {
                coordinates_include(entry.coordinates, WireNamespace::MainAccount, tag, version)
            });
            assert_eq!(
                matching.map(|entry| entry.status),
                Some(AllocationStatus::ReservedDisabled)
            );
        }
        let historical_descriptor = CENTRAL_COLLISION_LEDGER.iter().find(|entry| {
            coordinates_include(
                entry.coordinates,
                WireNamespace::MainAccount,
                STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_TAG,
                HISTORICAL_STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_VERSION_V1,
            )
        });
        assert_eq!(
            historical_descriptor.map(|entry| entry.status),
            Some(AllocationStatus::Withdrawn)
        );
        let live_descriptor = CENTRAL_COLLISION_LEDGER.iter().find(|entry| {
            coordinates_include(
                entry.coordinates,
                WireNamespace::MainAccount,
                STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_TAG,
                STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_VERSION,
            )
        });
        assert_eq!(
            live_descriptor.map(|entry| entry.status),
            Some(AllocationStatus::ReservedDisabled)
        );
    }

    #[test]
    fn source_v3_account_block_is_complete_and_disabled() {
        let expected = [
            (
                SOURCE_V3_RELEASE_ACCOUNT_TAG,
                SOURCE_V3_RELEASE_ACCOUNT_VERSION_V1,
            ),
            (
                SOURCE_V3_RELEASE_ACCOUNT_TAG,
                SOURCE_V3_RELEASE_ACCOUNT_VERSION,
            ),
            (SOURCE_V3_HEAD_ACCOUNT_TAG, SOURCE_V3_HEAD_ACCOUNT_VERSION),
            (
                SOURCE_V3_REOPEN_LINEAGE_ACCOUNT_TAG,
                SOURCE_V3_REOPEN_LINEAGE_ACCOUNT_VERSION,
            ),
            (
                SOURCE_V3_OPEN_RAW_PAGE_ACCOUNT_TAG,
                SOURCE_V3_OPEN_RAW_PAGE_ACCOUNT_VERSION,
            ),
            (
                SOURCE_V3_RAW_PAGE_ACCOUNT_TAG,
                SOURCE_V3_RAW_PAGE_ACCOUNT_VERSION,
            ),
            (
                SOURCE_V3_WINDOW_WORK_ACCOUNT_TAG,
                SOURCE_V3_WINDOW_WORK_ACCOUNT_VERSION,
            ),
            (
                SOURCE_V3_WINDOW_SEAL_ACCOUNT_TAG,
                SOURCE_V3_WINDOW_SEAL_ACCOUNT_VERSION,
            ),
            (
                SOURCE_V3_STATISTIC_RESULT_ACCOUNT_TAG,
                SOURCE_V3_STATISTIC_RESULT_ACCOUNT_VERSION,
            ),
            (
                SOURCE_V3_WORK_RECEIPT_ACCOUNT_TAG,
                SOURCE_V3_WORK_RECEIPT_ACCOUNT_VERSION,
            ),
        ];
        for (tag, version) in expected {
            let matching = CENTRAL_COLLISION_LEDGER.iter().find(|entry| {
                coordinates_include(entry.coordinates, WireNamespace::MainAccount, tag, version)
            });
            assert_eq!(
                matching.map(|entry| entry.status),
                Some(AllocationStatus::ReservedDisabled)
            );
        }
    }

    #[test]
    fn failure_recovery_account_block_withdraws_legacy_replay_and_reserves_market_successor() {
        let expected = [
            (
                FAILURE_EXTERNAL_ROOT_ACCOUNT_TAG,
                FAILURE_EXTERNAL_ROOT_ACCOUNT_VERSION,
            ),
            (
                FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
                FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
            ),
            (
                FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
                FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
            ),
        ];
        for (offset, (tag, version)) in expected.into_iter().enumerate() {
            assert_eq!(tag, 0xa0 + u8::try_from(offset).expect("small block"));
            let mut matching = CENTRAL_COLLISION_LEDGER.iter().filter(|entry| {
                coordinates_include(entry.coordinates, WireNamespace::MainAccount, tag, version)
            });
            assert_eq!(
                matching.next().map(|entry| entry.status),
                Some(AllocationStatus::ReservedDisabled),
                "account {tag}/{version}"
            );
            assert!(
                matching.next().is_none(),
                "duplicate account {tag}/{version}"
            );
        }
        let mut market_root_successor = CENTRAL_COLLISION_LEDGER.iter().filter(|entry| {
            coordinates_include(
                entry.coordinates,
                WireNamespace::MainAccount,
                FAILURE_EXTERNAL_ROOT_ACCOUNT_TAG,
                FAILURE_MARKET_ROOT_ACCOUNT_VERSION_V2,
            )
        });
        assert_eq!(
            market_root_successor.next().map(|entry| entry.status),
            Some(AllocationStatus::ReservedDisabled)
        );
        assert!(market_root_successor.next().is_none());

        let legacy_replay = CENTRAL_COLLISION_LEDGER.iter().find(|entry| {
            coordinates_include(
                entry.coordinates,
                WireNamespace::MainAccount,
                FAILURE_REPLAY_TOMBSTONE_ACCOUNT_TAG,
                FAILURE_REPLAY_TOMBSTONE_ACCOUNT_VERSION,
            )
        });
        assert_eq!(
            legacy_replay.map(|entry| entry.status),
            Some(AllocationStatus::Withdrawn)
        );
        let market_replay = CENTRAL_COLLISION_LEDGER.iter().find(|entry| {
            coordinates_include(
                entry.coordinates,
                WireNamespace::MainAccount,
                FAILURE_REPLAY_TOMBSTONE_ACCOUNT_TAG,
                FAILURE_MARKET_REPLAY_ACCOUNT_VERSION_V2,
            )
        });
        assert_eq!(
            market_replay.map(|entry| entry.status),
            Some(AllocationStatus::ReservedDisabled)
        );
        assert_eq!(FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2, 256);
    }

    #[test]
    fn fractional_redemption_account_block_is_complete_and_disabled() {
        let expected = [
            (
                FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_TAG,
                FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_VERSION,
                FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_BYTES,
                296,
            ),
            (
                FRACTIONAL_REDEMPTION_LEDGER_ACCOUNT_TAG,
                FRACTIONAL_REDEMPTION_LEDGER_ACCOUNT_VERSION,
                FRACTIONAL_REDEMPTION_LEDGER_ACCOUNT_BYTES,
                224,
            ),
            (
                FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_TAG,
                FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_VERSION,
                FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_BYTES,
                296,
            ),
            (
                FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_TAG,
                FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_VERSION,
                FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_BYTES,
                232,
            ),
        ];
        for (offset, (tag, version, bytes, expected_bytes)) in expected.into_iter().enumerate() {
            assert_eq!(tag, 0xa4 + u8::try_from(offset).expect("small block"));
            assert_eq!(bytes, expected_bytes);
            let mut matching = CENTRAL_COLLISION_LEDGER.iter().filter(|entry| {
                coordinates_include(entry.coordinates, WireNamespace::MainAccount, tag, version)
            });
            assert_eq!(
                matching.next().map(|entry| entry.status),
                Some(AllocationStatus::ReservedDisabled),
                "account {tag}/{version}"
            );
            assert!(
                matching.next().is_none(),
                "duplicate account {tag}/{version}"
            );
        }
    }

    #[test]
    fn fractional_redemption_reinterpreted_v1_accounts_are_withdrawn() {
        let withdrawn = [
            (
                FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_TAG,
                FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_V1_VERSION,
            ),
            (
                FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_TAG,
                FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_V1_VERSION,
            ),
            (
                FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_TAG,
                FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_V1_VERSION,
            ),
        ];
        for (tag, version) in withdrawn {
            let mut matching = CENTRAL_COLLISION_LEDGER.iter().filter(|entry| {
                coordinates_include(entry.coordinates, WireNamespace::MainAccount, tag, version)
            });
            assert_eq!(
                matching.next().map(|entry| entry.status),
                Some(AllocationStatus::Withdrawn),
                "withdrawn account {tag}/{version}"
            );
            assert!(
                matching.next().is_none(),
                "duplicate withdrawn account {tag}/{version}"
            );
        }
    }

    #[test]
    fn counted_retirement_wrapper_coordinates_are_reserved_but_disabled() {
        let expected = [
            (
                RETIREMENT_V2_MARKET_ACCOUNT_TAG,
                RETIREMENT_V2_MARKET_ACCOUNT_VERSION,
            ),
            (
                RETIREMENT_V2_POSITION_ACCOUNT_TAG,
                RETIREMENT_V2_POSITION_ACCOUNT_VERSION,
            ),
            (
                RETIREMENT_V2_EPOCH_ACCOUNT_TAG,
                RETIREMENT_V2_EPOCH_ACCOUNT_VERSION,
            ),
        ];
        for (tag, version) in expected {
            let mut matches = 0u8;
            for entry in CENTRAL_COLLISION_LEDGER {
                if coordinates_include(entry.coordinates, WireNamespace::MainAccount, tag, version)
                {
                    matches += 1;
                    assert_eq!(entry.status, AllocationStatus::ReservedDisabled);
                }
            }
            assert_eq!(matches, 1, "account {tag}/{version}");
        }
    }

    #[test]
    fn decimal_intent_74_is_not_hexadecimal_account_0x74() {
        assert_eq!(GENERAL_V2_FAMILY_TAG, 74);
        assert_eq!(GENERAL_V2_FAMILY_TAG, 0x4a);
        assert_eq!(SOURCE_ARCHIVE_V2_ACCOUNT_TAG, 0x74);
        assert_eq!(SOURCE_ARCHIVE_V2_ACCOUNT_TAG, 116);
        assert_ne!(GENERAL_V2_FAMILY_TAG, SOURCE_ARCHIVE_V2_ACCOUNT_TAG);
    }

    #[test]
    fn allocated_extension_actions_are_exhaustive() {
        for local_action in u8::MIN..=u8::MAX {
            let general = decode_extension_action(74, 1, local_action);
            assert_eq!(
                general.is_ok(),
                (GeneralV2Action::FIRST_TAG..=GeneralV2Action::LAST_TAG).contains(&local_action),
                "general action {local_action}"
            );
            let dealer = decode_extension_action(76, 1, local_action);
            assert_eq!(
                dealer.is_ok(),
                (DealerPolicyAction::FIRST_TAG..=DealerPolicyAction::LAST_TAG)
                    .contains(&local_action)
                    || (DealerFacilityAction::FIRST_TAG..=DealerFacilityAction::LAST_TAG)
                        .contains(&local_action),
                "dealer action {local_action}"
            );
            let source = decode_extension_action(77, 2, local_action);
            assert_eq!(
                source.is_ok(),
                (SourceSeriesAction::FIRST_TAG..=SourceSeriesAction::LAST_TAG)
                    .contains(&local_action)
                    || (RecurringSeriesAction::FIRST_TAG..=RecurringSeriesAction::LAST_TAG)
                        .contains(&local_action),
                "source-series action {local_action}"
            );
            let structured = decode_extension_action(75, 1, local_action);
            assert_eq!(
                structured.is_ok(),
                (StructuredClaimAction::FIRST_TAG..=StructuredClaimAction::LAST_TAG)
                    .contains(&local_action),
                "structured-claim action {local_action}"
            );
            let recovery = decode_extension_action(78, 1, local_action);
            assert_eq!(
                recovery.is_ok(),
                (RecoveryAction::FIRST_TAG..=RecoveryAction::LAST_TAG).contains(&local_action),
                "recovery action {local_action}"
            );
            let fractional = decode_extension_action(79, 1, local_action);
            assert_eq!(
                fractional.is_ok(),
                (FractionalRedemptionAction::FIRST_TAG..=FractionalRedemptionAction::LAST_TAG)
                    .contains(&local_action),
                "fractional-redemption action {local_action}"
            );
            let direct = decode_extension_action(80, 1, local_action);
            assert_eq!(
                direct.is_ok(),
                (DirectMarketAction::FIRST_TAG..=DirectMarketAction::LAST_TAG)
                    .contains(&local_action),
                "direct-market action {local_action}"
            );
        }
    }

    #[test]
    fn dealer_policy_coordinates_and_payload_widths_are_frozen() {
        let expected = [
            (
                DEALER_POLICY_STAGE_ACCOUNT_TAG,
                DEALER_POLICY_STAGE_ACCOUNT_VERSION,
            ),
            (DEALER_POLICY_ACCOUNT_TAG, DEALER_POLICY_ACCOUNT_VERSION),
        ];
        for (tag, version) in expected {
            let entry = CENTRAL_COLLISION_LEDGER.iter().find(|entry| {
                coordinates_include(entry.coordinates, WireNamespace::MainAccount, tag, version)
            });
            assert_eq!(
                entry.map(|entry| entry.status),
                Some(AllocationStatus::NonProductionLab)
            );
        }
        assert_eq!(DEALER_POLICY_CHUNK_BYTES, 192);
        assert_eq!(DEALER_BEGIN_POLICY_PAYLOAD_BYTES, 80);
        assert_eq!(DEALER_WRITE_POLICY_PAYLOAD_BYTES, 236);
        assert_eq!(DEALER_POLICY_ID_PAYLOAD_BYTES, 40);

        for action in [
            DealerPolicyAction::BeginPolicy,
            DealerPolicyAction::WritePolicy,
            DealerPolicyAction::SealPolicy,
            DealerPolicyAction::AbortPolicy,
        ] {
            assert_eq!(DealerPolicyAction::from_tag(action.tag()), Some(action));
            assert_eq!(
                decode_extension_action(DEALER_FAMILY_TAG, DEALER_FAMILY_VERSION, action.tag()),
                Ok(ExtensionAction::DealerPolicy(action))
            );
        }
    }

    #[test]
    fn envelope_is_strict_bounded_and_round_trips() {
        let envelope = ExtensionEnvelope {
            family: ExtensionFamily::GeneralV2,
            action: ExtensionAction::GeneralV2(GeneralV2Action::CompleteCandidateVerification),
            payload: &[9, 8, 7],
        };
        let mut bytes = [0_u8; 6];
        assert_eq!(envelope.encode(&mut bytes), Ok(6));
        assert_eq!(bytes, [74, 1, 14, 9, 8, 7]);
        assert_eq!(ExtensionEnvelope::decode(&bytes), Ok(envelope));

        assert_eq!(
            ExtensionEnvelope::decode(&bytes[..2]),
            Err(RegistryError::Truncated)
        );
        assert_eq!(
            ExtensionEnvelope::decode(&[74, 2, 14]),
            Err(RegistryError::UnknownFamilyVersion)
        );
        assert_eq!(
            ExtensionEnvelope::decode(&[74, 1, 0]),
            Err(RegistryError::UnknownLocalAction)
        );
        let mismatched = ExtensionEnvelope {
            family: ExtensionFamily::StructuredClaim,
            action: ExtensionAction::GeneralV2(GeneralV2Action::CreateMarket),
            payload: &[],
        };
        let mut untouched = [0xa5_u8; 3];
        assert_eq!(
            mismatched.encode(&mut untouched),
            Err(RegistryError::UnknownLocalAction)
        );
        assert_eq!(untouched, [0xa5; 3]);
        let too_long = [0_u8; MAX_INTENT_BYTES + 1];
        assert_eq!(
            ExtensionEnvelope::decode(&too_long),
            Err(RegistryError::TooLong)
        );
    }

    #[test]
    fn successor_envelope_preserves_the_frozen_packet_ceiling() {
        let payload = [0_u8; MAX_EXTENSION_PAYLOAD_BYTES];
        let envelope = ExtensionEnvelope {
            family: ExtensionFamily::GeneralV2,
            action: ExtensionAction::GeneralV2(GeneralV2Action::CreateMarket),
            payload: &payload,
        };
        let mut bytes = [0_u8; MAX_INTENT_BYTES];
        assert_eq!(envelope.encode(&mut bytes), Ok(MAX_INTENT_BYTES));
        assert_eq!(ExtensionEnvelope::decode(&bytes), Ok(envelope));
    }
}
