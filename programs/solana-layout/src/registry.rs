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
/// General V2 active-width sealed-feed successor account discriminator.
pub const GENERAL_V2_FEED_ACCOUNT_TAG: u8 = 18;
/// General V2 active-width sealed-feed successor account version.
pub const GENERAL_V2_FEED_ACCOUNT_VERSION: u8 = 2;
/// General V2 Window successor account discriminator.
pub const GENERAL_V2_WINDOW_ACCOUNT_TAG: u8 = 24;
/// General V2 Window successor account version.
pub const GENERAL_V2_WINDOW_ACCOUNT_VERSION: u8 = 4;
/// General V2 active-width feed-stage successor account discriminator.
pub const GENERAL_V2_FEED_STAGE_ACCOUNT_TAG: u8 = 25;
/// General V2 active-width feed-stage successor account version.
pub const GENERAL_V2_FEED_STAGE_ACCOUNT_VERSION: u8 = 2;
/// Funded General V2 admission-node account discriminator.
pub const GENERAL_V2_ADMISSION_NODE_ACCOUNT_TAG: u8 = 0x77;
/// Funded General V2 admission-node account version.
pub const GENERAL_V2_ADMISSION_NODE_ACCOUNT_VERSION: u8 = 1;
/// General V2 epoch-budget account discriminator.
pub const GENERAL_V2_EPOCH_BUDGET_ACCOUNT_TAG: u8 = 0x78;
/// General V2 epoch-budget account version.
pub const GENERAL_V2_EPOCH_BUDGET_ACCOUNT_VERSION: u8 = 1;
/// General V2 immutable Market-binding account discriminator.
pub const GENERAL_V2_MARKET_BINDING_ACCOUNT_TAG: u8 = 0x79;
/// General V2 immutable Market-binding account version.
pub const GENERAL_V2_MARKET_BINDING_ACCOUNT_VERSION: u8 = 1;
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
/// Non-production Dealer staged-policy account discriminator.
pub const DEALER_POLICY_STAGE_ACCOUNT_TAG: u8 = 0x7d;
/// Dealer staged-policy account version.
pub const DEALER_POLICY_STAGE_ACCOUNT_VERSION: u8 = 1;
/// Immutable Dealer policy catalog account discriminator.
pub const DEALER_POLICY_ACCOUNT_TAG: u8 = 0x7e;
/// Dealer immutable policy account version.
pub const DEALER_POLICY_ACCOUNT_VERSION: u8 = 1;
/// Frozen canonical `DealerPolicyV1` semantic-body length.
pub const DEALER_POLICY_BODY_BYTES: usize = 1_148;
/// Exact adapter-owned upload-stage header length.
pub const DEALER_POLICY_STAGE_HEADER_BYTES: usize = 140;
/// Exact upload-stage account length.
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
pub const SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION: u8 = 1;
/// Source/Series present-funding account discriminator.
pub const SOURCE_SERIES_FUNDING_ACCOUNT_TAG: u8 = 0x80;
/// Source/Series present-funding account version.
pub const SOURCE_SERIES_FUNDING_ACCOUNT_VERSION: u8 = 1;
/// General V2 owner-aggregated settlement account discriminator.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG: u8 = 0x81;
/// Withdrawn non-aliasing General V2 owner-settlement V1 version.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V1: u8 = 1;
/// Withdrawn presence-explicit General V2 owner-settlement V2 version.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V2: u8 = 2;
/// Sole future Reservation-handoff General owner-settlement version.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V3: u8 = 3;
/// Current General owner-settlement version; an alias only for V3.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION: u8 =
    GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V3;
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
/// StructuredClaim immutable descriptor envelope version.
pub const STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_VERSION: u8 = 1;
/// General V2 final settlement-pot account discriminator.
pub const GENERAL_V2_FINAL_POT_ACCOUNT_TAG: u8 = 0x89;
/// General V2 final settlement-pot account version.
pub const GENERAL_V2_FINAL_POT_ACCOUNT_VERSION: u8 = 1;
/// Immutable authenticated SourcePlane V3 release account discriminator.
pub const SOURCE_V3_RELEASE_ACCOUNT_TAG: u8 = 0x8a;
/// SourcePlane V3 release account version.
pub const SOURCE_V3_RELEASE_ACCOUNT_VERSION: u8 = 1;
/// Mutable SourcePlane V3 head account discriminator.
pub const SOURCE_V3_HEAD_ACCOUNT_TAG: u8 = 0x8b;
/// SourcePlane V3 head account version.
pub const SOURCE_V3_HEAD_ACCOUNT_VERSION: u8 = 1;
/// Durable SourcePlane V3 reopen-lineage account discriminator.
pub const SOURCE_V3_REOPEN_LINEAGE_ACCOUNT_TAG: u8 = 0x8c;
/// SourcePlane V3 reopen-lineage account version.
pub const SOURCE_V3_REOPEN_LINEAGE_ACCOUNT_VERSION: u8 = 1;
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
    DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 472;
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
pub const DEALER_LEASE_V2_ACCOUNT_BYTES: usize = DEALER_RUNTIME_ACCOUNT_HEADER_BYTES + 1_068;
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
/// Bytes occupied by the successor family tag, family version, and local action.
pub const EXTENSION_ENVELOPE_BYTES: usize = 3;
/// Largest successor action payload without changing the frozen packet ceiling.
pub const MAX_EXTENSION_PAYLOAD_BYTES: usize = MAX_INTENT_BYTES - EXTENSION_ENVELOPE_BYTES;

const _: () = assert!(GENERAL_V2_FAMILY_TAG == 74);
const _: () = assert!(GENERAL_V2_FAMILY_TAG == 0x4a);
const _: () = assert!(LEGACY_INTENT_FIRST_TAG == super::CREATE_TAG);
const _: () = assert!(LEGACY_INTENT_LAST_TAG == super::SEAL_SOURCE_ARCHIVE_V2_TAG);
const _: () = assert!(LEGACY_INTENT_VERSION == super::INTENT_VERSION);
const _: () = assert!(SOURCE_ARCHIVE_V2_ACCOUNT_TAG == 116);
const _: () = assert!(SOURCE_ARCHIVE_V2_ACCOUNT_TAG == 0x74);
const _: () = assert!(GENERAL_V2_FAMILY_TAG != SOURCE_ARCHIVE_V2_ACCOUNT_TAG);
const _: () = assert!(GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_TAG == 15);
const _: () = assert!(GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_VERSION == 3);
const _: () = assert!(GENERAL_ORDER_PAGE_V5_ACCOUNT_TAG == 8);
const _: () = assert!(GENERAL_ORDER_PAGE_V5_ACCOUNT_VERSION == 5);
const _: () = assert!(GENERAL_ORDER_PAGE_V5_ACCOUNT_TAG == super::order_page_v5::ORDER_PAGE_V5_TAG);
const _: () =
    assert!(GENERAL_ORDER_PAGE_V5_ACCOUNT_VERSION == super::order_page_v5::ORDER_PAGE_V5_VERSION);
const _: () = assert!(EXTENSION_ENVELOPE_BYTES <= MAX_INTENT_BYTES);
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
    /// Whether the coordinates are frozen or merely reserved and disabled.
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
        status: AllocationStatus::ReservedDisabled,
        name: "general-settlement-receipt-v3-account",
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
            version: SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-series-registry-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: SOURCE_SERIES_FUNDING_ACCOUNT_TAG,
            version: SOURCE_SERIES_FUNDING_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-series-funding-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
            version: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V1,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-owner-settlement-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
            version: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V2,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-owner-settlement-v2-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
            version: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION_V3,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-owner-settlement-v3-account",
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
            version: STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "structured-claim-descriptor-v1-account",
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
            version: SOURCE_V3_RELEASE_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "source-v3-release-v1-account",
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
        status: AllocationStatus::ReservedDisabled,
        name: "failure-replay-tombstone-v1-account",
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
}

/// Dealer family-local policy-catalog transport actions.
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
    /// Validate and materialize the immutable content-addressed policy.
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

/// Exact body bytes accepted in each staged Dealer-policy write.
pub const DEALER_POLICY_CHUNK_BYTES: usize = 192;
/// Exact Begin payload bytes: policy ID, neutral sink, expiry slot.
pub const DEALER_BEGIN_POLICY_PAYLOAD_BYTES: usize = 32 + 32 + 8;
/// Exact Write payload bytes: policy ID, cursor, active length, padded chunk.
pub const DEALER_WRITE_POLICY_PAYLOAD_BYTES: usize = 32 + 2 + 2 + DEALER_POLICY_CHUNK_BYTES;
/// Exact Seal/Abort payload bytes: policy ID.
pub const DEALER_POLICY_ID_PAYLOAD_BYTES: usize = 32;

const _: () = assert!(DEALER_WRITE_POLICY_PAYLOAD_BYTES <= MAX_EXTENSION_PAYLOAD_BYTES);

impl GeneralV2Action {
    /// First allocated General V2 local action tag.
    pub const FIRST_TAG: u8 = 1;
    /// Last allocated General V2 local action tag.
    pub const LAST_TAG: u8 = 38;

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
    /// Capitalize the five Series funding compartments.
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
}

impl RecoveryAction {
    /// First Recovery-owned local action tag.
    pub const FIRST_TAG: u8 = 1;
    /// Last Recovery-owned local action tag.
    pub const LAST_TAG: u8 = 9;

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
                GENERAL_ORDER_PAGE_V5_ACCOUNT_TAG,
                GENERAL_ORDER_PAGE_V5_ACCOUNT_VERSION,
            ),
            (
                GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_TAG,
                GENERAL_SETTLEMENT_RECEIPT_V3_ACCOUNT_VERSION,
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
            (GENERAL_V2_FEED_ACCOUNT_TAG, GENERAL_V2_FEED_ACCOUNT_VERSION),
            (
                GENERAL_V2_WINDOW_ACCOUNT_TAG,
                GENERAL_V2_WINDOW_ACCOUNT_VERSION,
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
                GENERAL_V2_EPOCH_BUDGET_ACCOUNT_TAG,
                GENERAL_V2_EPOCH_BUDGET_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_MARKET_BINDING_ACCOUNT_TAG,
                GENERAL_V2_MARKET_BINDING_ACCOUNT_VERSION,
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
    fn coordinated_post_selected_account_block_is_complete_and_disabled() {
        let expected = [
            (
                DEALER_POLICY_STAGE_ACCOUNT_TAG,
                DEALER_POLICY_STAGE_ACCOUNT_VERSION,
            ),
            (DEALER_POLICY_ACCOUNT_TAG, DEALER_POLICY_ACCOUNT_VERSION),
            (
                SOURCE_SERIES_REGISTRY_ACCOUNT_TAG,
                SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION,
            ),
            (
                SOURCE_SERIES_FUNDING_ACCOUNT_TAG,
                SOURCE_SERIES_FUNDING_ACCOUNT_VERSION,
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
                STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_TAG,
                STRUCTURED_CLAIM_DESCRIPTOR_ACCOUNT_VERSION,
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
    }

    #[test]
    fn source_v3_account_block_is_complete_and_disabled() {
        let expected = [
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
    fn failure_recovery_account_block_is_complete_and_disabled() {
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
            (
                FAILURE_REPLAY_TOMBSTONE_ACCOUNT_TAG,
                FAILURE_REPLAY_TOMBSTONE_ACCOUNT_VERSION,
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
        assert_eq!(DEALER_BEGIN_POLICY_PAYLOAD_BYTES, 72);
        assert_eq!(DEALER_WRITE_POLICY_PAYLOAD_BYTES, 228);
        assert_eq!(DEALER_POLICY_ID_PAYLOAD_BYTES, 32);

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
