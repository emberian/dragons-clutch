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
/// General V2 monotone Market-runtime account discriminator.
pub const GENERAL_V2_MARKET_RUNTIME_ACCOUNT_TAG: u8 = 3;
/// First General V2 monotone Market-runtime version.
pub const GENERAL_V2_MARKET_RUNTIME_ACCOUNT_VERSION: u8 = 3;
/// Settlement-complete counted General V2 Epoch discriminator.
pub const GENERAL_V2_EPOCH_ACCOUNT_TAG: u8 = 11;
/// Settlement-complete counted General V2 Epoch version.
pub const GENERAL_V2_EPOCH_ACCOUNT_VERSION: u8 = 6;
/// Permanent General V2 Epoch tombstone discriminator.
pub const GENERAL_V2_EPOCH_TOMBSTONE_ACCOUNT_TAG: u8 = 0x76;
/// Rent-owner-complete permanent Epoch tombstone version.
pub const GENERAL_V2_EPOCH_TOMBSTONE_ACCOUNT_VERSION: u8 = 2;
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
/// General V2 owner-aggregated settlement account discriminator.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG: u8 = 0x81;
/// First exact owner-settlement outer version.
pub const GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION: u8 = 1;
/// General V2 allocation-complete settlement cash-pot discriminator.
pub const GENERAL_V2_SETTLEMENT_CASH_POT_ACCOUNT_TAG: u8 = 0x87;
/// First exact settlement cash-pot outer version.
pub const GENERAL_V2_SETTLEMENT_CASH_POT_ACCOUNT_VERSION: u8 = 1;
/// General V2 explicit-liability FinalPot discriminator.
pub const GENERAL_V2_FINAL_POT_ACCOUNT_TAG: u8 = 0x89;
/// First exact FinalPot outer version.
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
const _: () = assert!(EXTENSION_ENVELOPE_BYTES <= MAX_INTENT_BYTES);

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
        status: AllocationStatus::ReservedDisabled,
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
            tag: 0x76,
            version: 1,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "retirement-provisional-general-epoch-tombstone-v1-account",
    },
    CollisionLedgerEntry {
        coordinates: AllocationCoordinates::Exact {
            namespace: WireNamespace::MainAccount,
            tag: GENERAL_V2_EPOCH_TOMBSTONE_ACCOUNT_TAG,
            version: GENERAL_V2_EPOCH_TOMBSTONE_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-epoch-tombstone-v2-account",
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
        name: "general-v2-counted-epoch-v6-account",
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
            tag: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
            version: GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-owner-settlement-v1-account",
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
            tag: GENERAL_V2_SETTLEMENT_CASH_POT_ACCOUNT_TAG,
            version: GENERAL_V2_SETTLEMENT_CASH_POT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-settlement-cash-pot-v1-account",
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
            tag: GENERAL_V2_FINAL_POT_ACCOUNT_TAG,
            version: GENERAL_V2_FINAL_POT_ACCOUNT_VERSION,
        },
        status: AllocationStatus::ReservedDisabled,
        name: "general-v2-final-pot-v1-account",
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
    /// Consume one entitled slice.
    EntitleSlice = 25,
    /// Release one terminal reservation.
    ReleaseTerminalReservation = 26,
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
}

impl GeneralV2Action {
    /// First allocated General V2 local action tag.
    pub const FIRST_TAG: u8 = 1;
    /// Last allocated General V2 local action tag.
    pub const LAST_TAG: u8 = 34;

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
            Self::EntitleSlice => 25,
            Self::ReleaseTerminalReservation => 26,
            Self::CloseReceipt => 27,
            Self::CloseReservation => 28,
            Self::ClosePage => 29,
            Self::ClosePot => 30,
            Self::CloseCandidate => 31,
            Self::CloseClearWork => 32,
            Self::CloseEpoch => 33,
            Self::ClosePosition => 34,
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
            25 => Some(Self::EntitleSlice),
            26 => Some(Self::ReleaseTerminalReservation),
            27 => Some(Self::CloseReceipt),
            28 => Some(Self::CloseReservation),
            29 => Some(Self::ClosePage),
            30 => Some(Self::ClosePot),
            31 => Some(Self::CloseCandidate),
            32 => Some(Self::CloseClearWork),
            33 => Some(Self::CloseEpoch),
            34 => Some(Self::ClosePosition),
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

/// One allocated successor family-local action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtensionAction {
    /// One General V2 local action.
    GeneralV2(GeneralV2Action),
    /// One SourcePlane V3 action in the shared SourceSeries family.
    SourceV3(SourceSeriesAction),
}

impl ExtensionAction {
    /// Return the action's local tag.
    pub const fn local_tag(self) -> u8 {
        match self {
            Self::GeneralV2(action) => action.tag(),
            Self::SourceV3(action) => action.tag(),
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
        Some(ExtensionFamily::SourceSeries) => match SourceSeriesAction::from_tag(local_action) {
            Some(action) => Ok(ExtensionAction::SourceV3(action)),
            None => Err(RegistryError::UnknownLocalAction),
        },
        Some(
            ExtensionFamily::StructuredClaim | ExtensionFamily::Dealer | ExtensionFamily::Recovery,
        ) => Err(RegistryError::UnknownLocalAction),
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
    fn every_registered_successor_account_coordinate_is_reserved_but_disabled() {
        let expected = [
            (
                GENERAL_V2_MARKET_RUNTIME_ACCOUNT_TAG,
                GENERAL_V2_MARKET_RUNTIME_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_EPOCH_ACCOUNT_TAG,
                GENERAL_V2_EPOCH_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_EPOCH_TOMBSTONE_ACCOUNT_TAG,
                GENERAL_V2_EPOCH_TOMBSTONE_ACCOUNT_VERSION,
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
                GENERAL_V2_ECONOMIC_DOMAIN_ACCOUNT_TAG,
                GENERAL_V2_ECONOMIC_DOMAIN_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_SELECTED_CANDIDATE_ACCOUNT_TAG,
                GENERAL_V2_SELECTED_CANDIDATE_ACCOUNT_VERSION,
            ),
            (
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_TAG,
                GENERAL_V2_OWNER_SETTLEMENT_ACCOUNT_VERSION,
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
    fn general_and_source_actions_are_exhaustive_and_other_families_allocate_none() {
        for local_action in u8::MIN..=u8::MAX {
            let general = decode_extension_action(74, 1, local_action);
            assert_eq!(
                general.is_ok(),
                (GeneralV2Action::FIRST_TAG..=GeneralV2Action::LAST_TAG).contains(&local_action),
                "general action {local_action}"
            );
            let source = decode_extension_action(77, 2, local_action);
            assert_eq!(
                source.is_ok(),
                (SourceSeriesAction::FIRST_TAG..=SourceSeriesAction::LAST_TAG)
                    .contains(&local_action),
                "source action {local_action}"
            );
            for (tag, version) in [(75, 1), (76, 1), (78, 1)] {
                assert_eq!(
                    decode_extension_action(tag, version, local_action),
                    Err(RegistryError::UnknownLocalAction),
                    "{tag}/{version}/{local_action}"
                );
            }
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
