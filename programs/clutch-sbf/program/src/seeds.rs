//! **PROPOSED** domain-separated PDA seed schema for the bring-up program.
//!
//! Nothing in this module is frozen.  The seed strings, the seed order, and the
//! choice of which identity goes into which seed are a proposal made by this
//! bring-up lane so that obligation 1 of
//! `docs/implementation/SOLANA_REFERENCE_ADAPTER.md` can be exercised end to
//! end against a real runtime.  Changing any byte here changes every account
//! address, so a later freeze is an ABI decision, not a refactor.
//!
//! Every prefix is a single seed of at most 32 bytes and every tuple is well
//! under the 16-seed limit, so `find_program_address` can never fail for
//! length reasons.
//!
//! The 32-byte owner identity carried by
//! [`clutch_solana_layout::PositionAccount::owner`] is interpreted here as the
//! raw bytes of the owning wallet address.  That interpretation is what lets
//! the program bind an authenticated signer to a stored position, and it is
//! also a proposal rather than a frozen rule.

use solana_pubkey::Pubkey;

/// Find the canonical program address and bump for one seed tuple.
///
/// Program-address derivation is a runtime syscall under `target_os =
/// "solana"`.  Off-chain derivation is not compiled into this crate: doing it
/// in process needs the `curve25519` backend, whose proc-macro dependency has
/// no archive in this host's offline crate cache.  The differential harness
/// derives the same addresses out of process with the pinned
/// `solana find-program-derived-address` command, using the seed prefixes
/// exported above so that there is still exactly one source of truth for the
/// seed bytes.
pub fn find(program_id: &Pubkey, seeds: &[&[u8]]) -> (Pubkey, u8) {
    #[cfg(target_os = "solana")]
    {
        Pubkey::find_program_address(seeds, program_id)
    }
    #[cfg(not(target_os = "solana"))]
    {
        let _ = (program_id, seeds);
        unimplemented!(
            "off-chain PDA derivation is not compiled into clutch-sbf; \
             see docs/implementation/SBF_BRINGUP.md"
        )
    }
}

/// Realm account seed prefix.
pub const SEED_REALM: &[u8] = b"dragons-clutch:realm:v1";
/// Profile account seed prefix.
pub const SEED_PROFILE: &[u8] = b"dragons-clutch:profile:v1";
/// Market account seed prefix.
pub const SEED_MARKET: &[u8] = b"dragons-clutch:market:v1";
/// Hoard account seed prefix.
pub const SEED_HOARD: &[u8] = b"dragons-clutch:hoard:v1";
/// Full-width MarketInstance Hoard V2 seed prefix.
pub const SEED_HOARD_V2: &[u8] = b"dc:hoard:v2";
/// Position account seed prefix.
pub const SEED_POSITION: &[u8] = b"dragons-clutch:position:v1";
/// Reference-only kernel-aggregate account seed prefix.
pub const SEED_KERNEL: &[u8] = b"dragons-clutch:kernel:v1";
/// Full-width native ClaimLedger V3 seed prefix.
pub const SEED_CLAIM_LEDGER_V3: &[u8] = b"dc:claim-ledger:v3";
/// Reference-only external-shadow account seed prefix.
pub const SEED_EXTERNAL: &[u8] = b"dragons-clutch:external:v1";
/// Reference-only replay-sequence account seed prefix.
pub const SEED_REPLAY: &[u8] = b"dragons-clutch:replay:v1";
/// Seed prefix for the market-wide supply-ledger account.
pub const SEED_SUPPLY: &[u8] = b"dragons-clutch:supply:v1";
/// Feed head account seed prefix.
pub const SEED_FEED: &[u8] = b"dragons-clutch:feed:v1";
/// Immutable terms account seed prefix.
pub const SEED_TERMS: &[u8] = b"dragons-clutch:terms:v1";
/// Frozen price-grid account seed prefix.
pub const SEED_GRID: &[u8] = b"dragons-clutch:grid:v1";
/// Resolution record account seed prefix.
pub const SEED_RESOLUTION: &[u8] = b"dragons-clutch:resolution:v1";
/// Full-width MarketInstance Resolution V5 account seed prefix.
pub const SEED_RESOLUTION_V5: &[u8] = b"dc:resolution:v5";
/// Epoch/book-domain account seed prefix.
pub const SEED_EPOCH: &[u8] = b"dragons-clutch:epoch:v1";
/// Order-page account seed prefix.
pub const SEED_PAGE: &[u8] = b"dragons-clutch:page:v1";
/// Per-order funded-reservation account seed prefix.
pub const SEED_RESERVATION: &[u8] = b"dragons-clutch:reservation:v1";
/// Candidate-record account seed prefix.
pub const SEED_CANDIDATE: &[u8] = b"dragons-clutch:candidate:v1";
/// Candidate-feed account seed prefix.
///
/// The feed is the stable byte carrier for one candidate's fills and pairing
/// slices.  Content equality alone is not account authority, so settlement
/// authenticates exactly one feed address per `(epoch, candidate)`.
pub const SEED_CANDIDATE_FEED: &[u8] = b"dragons-clutch:cand-feed:v1";

/// Clearing-checkpoint (`ClearWorkAccount`) seed prefix; 28 bytes.
pub const SEED_CLEAR_WORK: &[u8] = b"dragons-clutch:clear-work:v1";
/// General-epoch deadline-window (`EpochWindowAccount`) seed prefix; 30 bytes.
pub const SEED_EPOCH_WINDOW: &[u8] = b"dragons-clutch:epoch-window:v1";
/// Final-pot account seed prefix.
pub const SEED_POT: &[u8] = b"dragons-clutch:pot:v1";
/// Settlement-receipt account seed prefix.
pub const SEED_RECEIPT: &[u8] = b"dragons-clutch:receipt:v1";
/// General funding-ledger sibling seed prefix, keyed by the funded account's
/// own address so one machinery account has exactly one recorded funding.
pub const SEED_GENERAL_FUNDING: &[u8] = b"dc:gen-funding:v1";
/// Uploader-keyed typed artifact staging account seed prefix.
pub const SEED_ARTIFACT_STAGE: &[u8] = b"dragons-clutch:upload:v1";
/// Non-production uploader-keyed Dealer-policy stage prefix.
pub const SEED_DEALER_POLICY_STAGE: &[u8] = b"dc-dealer-policy-stage-v1";
/// Canonical immutable Dealer-policy prefix, frozen by the pure contract.
pub const SEED_DEALER_POLICY: &[u8] = clutch_dealer_runtime_contract::DEALER_POLICY_PDA_DOMAIN_V1;
/// Immutable Dealer liveness schedule.
pub const SEED_DEALER_LIVENESS_SCHEDULE: &[u8] =
    clutch_dealer_runtime_contract::DEALER_LIVENESS_SCHEDULE_PDA_DOMAIN_V1;
/// Immutable generic runtime-liveness policy selected by one Dealer facility.
pub const SEED_DEALER_RUNTIME_LIVENESS_POLICY: &[u8] = b"dc-dealer-runtime-liveness-policy-v1";
/// One facility- and compartment-scoped generic runtime-liveness account.
pub const SEED_DEALER_RUNTIME_LIVENESS_ACCOUNT: &[u8] = b"dc-dealer-live-account-v1";
/// Authoritative Dealer StateV2.
pub const SEED_DEALER_STATE_V2: &[u8] = clutch_dealer_runtime_contract::DEALER_STATE_PDA_DOMAIN_V2;
/// Counted Dealer funded-dependency child.
pub const SEED_DEALER_FUNDED_V2: &[u8] =
    clutch_dealer_runtime_contract::DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V2;
/// Dealer LP ownership page V2.
pub const SEED_DEALER_LP_PAGE_V2: &[u8] = clutch_dealer_runtime_contract::LP_PAGE_PDA_DOMAIN_V2;
/// Dealer LeaseV2.
pub const SEED_DEALER_LEASE_V2: &[u8] = clutch_dealer_runtime_contract::DEALER_LEASE_PDA_DOMAIN_V2;
/// Dealer SettlementPotV2.
pub const SEED_DEALER_POT_V2: &[u8] = clutch_dealer_runtime_contract::SETTLEMENT_POT_PDA_DOMAIN_V2;
/// Dealer counted Epoch-binding V2.
pub const SEED_DEALER_EPOCH_V2: &[u8] =
    clutch_dealer_runtime_contract::DEALER_EPOCH_BINDING_PDA_DOMAIN_V2;
/// Dealer page-scoped terminal allocation.
pub const SEED_DEALER_TERMINAL_ALLOCATION: &[u8] =
    clutch_dealer_runtime_contract::DEALER_TERMINAL_ALLOCATION_PDA_DOMAIN_V1;
/// Dealer singleton claim work.
pub const SEED_DEALER_CLAIM_WORK: &[u8] =
    clutch_dealer_runtime_contract::DEALER_CLAIM_WORK_PDA_DOMAIN_V1;
/// Dealer permanent root tombstone.
pub const SEED_DEALER_ROOT_V2: &[u8] =
    clutch_dealer_runtime_contract::DEALER_ROOT_TOMBSTONE_PDA_DOMAIN_V2;
/// Dealer owner-scoped exit ticket.
pub const SEED_DEALER_EXIT_TICKET: &[u8] =
    clutch_dealer_runtime_contract::DEALER_EXIT_TICKET_PDA_DOMAIN_V1;
/// Content-addressed Dealer action-work receipt.
pub const SEED_DEALER_ACTION_RECEIPT: &[u8] =
    clutch_dealer_runtime_contract::DEALER_ACTION_RECEIPT_PDA_DOMAIN_V1;
/// Canonical raw collateral-policy artifact seed prefix.
pub const SEED_POLICY: &[u8] = b"dragons-clutch:policy:v1";
/// Canonical full-width batch-policy artifact seed prefix.
pub const SEED_BATCH_POLICY: &[u8] = b"dragons-clutch:batch-policy:v1";
/// DirectBatchPolicy V3 final-artifact seed prefix, disjoint from legacy policy.
pub const SEED_DIRECT_BATCH_POLICY_V3: &[u8] = b"dc:direct-policy:v3";
/// Globally content-addressed successor Product/Series artifact prefix.
///
/// The kind byte is a seed so transparent 32-byte typed IDs are never cast
/// across artifact meanings. Realm binding remains inside Genesis V2.
pub const SEED_PRODUCT_ARTIFACT_V1: &[u8] = b"dc:product-artifact:v1";
/// Persistent V5 Series registration/replay-anchor prefix.
pub const SEED_SERIES_REGISTRY_V1: &[u8] = b"dc:series-registry:v1";
/// Mutable V5 Series funding-state prefix.
pub const SEED_SERIES_FUNDING_V1: &[u8] = b"dc:series-funding:v1";
/// Zero-data, System-owned per-component lamport custody prefix.
pub const SEED_SERIES_LAMPORT_VAULT_V1: &[u8] = b"dc:series-lamports:v1";
/// Sole PDA signing authority for one Series' collateral vault set.
pub const SEED_SERIES_COLLATERAL_AUTHORITY_V1: &[u8] = b"dc:series-collateral-auth:v1";
/// Per-component release-selected collateral custody prefix.
pub const SEED_SERIES_COLLATERAL_VAULT_V1: &[u8] = b"dc:series-collateral:v1";
/// Immutable SourcePlane V3 occurrence-provenance record prefix.
pub const SEED_SOURCE_OCCURRENCE_V1: &[u8] = b"dc:source-occurrence:v1";
/// Immutable Source-selected runtime-liveness policy account prefix.
pub const SEED_SOURCE_LIVENESS_POLICY_V1: &[u8] = b"dc:source-live-policy:v1";
/// Occurrence-scoped Product whole-Market lifecycle root prefix.
pub const SEED_PRODUCT_OCCURRENCE_ROOT_V1: &[u8] = b"dc:product-occurrence-root:v1";
/// Full-width Product/Failure-owned Resolution V5 prefix.
pub const SEED_RESOLUTION_V5: &[u8] = b"dc:resolution:v5";
/// Direct candidate-window account seed prefix.
pub const SEED_DIRECT_WINDOW: &[u8] = b"dragons-clutch:direct-window:v1";
/// Full-width verified direct candidate seed prefix.
pub const SEED_DIRECT_CANDIDATE: &[u8] = b"dragons-clutch:dir-candidate:v2";
/// Direct settlement-receipt seed prefix.
pub const SEED_DIRECT_RECEIPT: &[u8] = b"dragons-clutch:direct-receipt:v2";
/// Direct zero-pot seed prefix.
pub const SEED_DIRECT_POT: &[u8] = b"dragons-clutch:direct-pot:v2";
/// Direct V3 candidate-window seed prefix, disjoint from every V1/V2 address.
pub const SEED_DIRECT_WINDOW_V3: &[u8] = b"dc:direct-window:v3";
/// Direct V3 verified-candidate seed prefix, disjoint from every V1/V2 address.
pub const SEED_DIRECT_CANDIDATE_V3: &[u8] = b"dc:direct-candidate:v3";
/// Direct V3 finite WorkBudget seed prefix.
pub const SEED_DIRECT_WORK_V3: &[u8] = b"dc:direct-work:v3";
/// Direct V3 settlement-receipt seed prefix, disjoint from V2 receipts.
pub const SEED_DIRECT_RECEIPT_V3: &[u8] = b"dc:direct-receipt:v3";
/// Direct V3 zero-pot seed prefix, disjoint from V2 pots.
pub const SEED_DIRECT_POT_V3: &[u8] = b"dc:direct-pot:v3";
/// Immutable authenticated source-spec account seed prefix.
pub const SEED_SOURCE_SPEC: &[u8] = crate::source_archive::SOURCE_SPEC_SEED_V1;
/// Per-window authenticated source-archive account seed prefix.
pub const SEED_SOURCE_ARCHIVE: &[u8] = crate::source_archive::SOURCE_ARCHIVE_SEED_V1;
/// Per-Realm revenue-policy record seed prefix; exactly 32 bytes (the seed
/// cap), the string `docs/design/REVENUE_POLICY_V1.md` §3 names.
pub const SEED_REVENUE_POLICY: &[u8] = b"dragons-clutch:revenue-policy:v1";
/// One deterministic active ResolutionWork lock per Market.
pub const SEED_RESOLUTION_WORK: &[u8] = b"resolution-work-v1";
/// Program-owned prepaid Reserve bound to one deterministic Work PDA.
pub const SEED_RESOLUTION_RESERVE: &[u8] = b"resolution-reserve-v1";

/// Immutable General V2 MarketBinding seed prefix.
pub const SEED_GENERAL_V2_MARKET_BINDING: &[u8] =
    clutch_general_v2_contract::MARKET_BINDING_SEED_DOMAIN_V1;
/// Genesis-assisted General V2 mutable Market runtime seed prefix.
pub const SEED_GENERAL_V2_MARKET_RUNTIME: &[u8] =
    clutch_general_v2_contract::MARKET_RUNTIME_SEED_DOMAIN_V1;
/// Counted General V2 Epoch seed prefix.
pub const SEED_GENERAL_V2_EPOCH: &[u8] = clutch_general_v2_contract::EPOCH_SEED_DOMAIN_V1;
/// General V2 EconomicDomain artifact seed prefix.
pub const SEED_GENERAL_V2_ECONOMIC_DOMAIN: &[u8] =
    clutch_general_v2_contract::ECONOMIC_DOMAIN_SEED_DOMAIN_V1;
/// General V2 candidate Window seed prefix.
pub const SEED_GENERAL_V2_WINDOW: &[u8] = clutch_general_v2_contract::WINDOW_SEED_DOMAIN_V1;
/// General V2 root Budget seed prefix.
pub const SEED_GENERAL_V2_BUDGET: &[u8] = clutch_general_v2_contract::EPOCH_BUDGET_SEED_DOMAIN_V1;
/// General V2 ordinal-owned AdmissionNode seed prefix.
pub const SEED_GENERAL_V2_NODE: &[u8] = clutch_general_v2_contract::CANDIDATE_NODE_SEED_DOMAIN_V1;
/// General V2 active-width Feed/Stage seed prefix.
pub const SEED_GENERAL_V2_FEED: &[u8] = clutch_general_v2_contract::CANDIDATE_FEED_SEED_DOMAIN_V1;
/// General V2 active-width ClearWork seed prefix.
pub const SEED_GENERAL_V2_WORK: &[u8] = clutch_general_v2_contract::CLEAR_WORK_SEED_DOMAIN_V1;
/// Disabled resumable RelationV2 ClearWork V3 seed prefix.
pub const SEED_GENERAL_V2_WORK_V3: &[u8] = clutch_general_v2_contract::CLEAR_WORK_SEED_DOMAIN_V3;
/// General V2 selected settlement-authority seed prefix.
pub const SEED_GENERAL_V2_SELECTED: &[u8] =
    clutch_general_v2_contract::SELECTED_CANDIDATE_SEED_DOMAIN_V1;
/// Disabled General V2 OrderPage V5 seed prefix.
pub const SEED_GENERAL_V2_ORDER_PAGE_V5: &[u8] =
    clutch_general_v2_contract::ORDER_PAGE_SEED_DOMAIN_V1;
/// Disabled General V2 Reservation V3 seed prefix.
pub const SEED_GENERAL_V2_RESERVATION_V3: &[u8] =
    clutch_general_v2_contract::RESERVATION_SEED_DOMAIN_V1;
/// Disabled sole-future rent-owned General Reservation V9 seed prefix.
pub const SEED_GENERAL_V2_RESERVATION_V9: &[u8] =
    clutch_general_v2_contract::RESERVATION_SEED_DOMAIN_V9;
/// Disabled General SettlementReceipt V3 seed prefix.
pub const SEED_GENERAL_V2_RECEIPT: &[u8] = clutch_general_v2_contract::RECEIPT_SEED_DOMAIN_V3;
/// Disabled sole-future rent-owned General SettlementReceipt V5 seed prefix.
pub const SEED_GENERAL_V2_RECEIPT_V5: &[u8] = clutch_general_v2_contract::RECEIPT_SEED_DOMAIN_V5;
/// Disabled General V2 owner-aggregated settlement seed prefix.
pub const SEED_GENERAL_V2_OWNER_SETTLEMENT: &[u8] =
    clutch_general_v2_contract::OWNER_SETTLEMENT_SEED_DOMAIN_V2;
/// Disabled sole-future rent-owned General owner-settlement V5 seed prefix.
pub const SEED_GENERAL_V2_OWNER_SETTLEMENT_V5: &[u8] =
    clutch_general_v2_contract::OWNER_SETTLEMENT_SEED_DOMAIN_V5;
/// Disabled selected composite-fee record seed prefix.
pub const SEED_GENERAL_V2_SELECTED_FEE_RECORD: &[u8] =
    clutch_general_v2_contract::SELECTED_FEE_RECORD_SEED_DOMAIN_V1;
/// Disabled owner-scoped fee-carry seed prefix.
pub const SEED_GENERAL_V2_OWNER_FEE_CARRY: &[u8] =
    clutch_general_v2_contract::OWNER_FEE_CARRY_SEED_DOMAIN_V1;
/// Disabled owner payer-allocation seed prefix.
pub const SEED_GENERAL_V2_PAYER_ALLOCATION: &[u8] =
    clutch_general_v2_contract::PAYER_ALLOCATION_SEED_DOMAIN_V1;
/// Disabled candidate-wide recipient-allocation seed prefix.
pub const SEED_GENERAL_V2_RECIPIENT_ALLOCATION: &[u8] =
    clutch_general_v2_contract::RECIPIENT_ALLOCATION_SEED_DOMAIN_V1;
/// Disabled selected-record treasury-ledger seed prefix.
pub const SEED_GENERAL_V2_TREASURY_LEDGER: &[u8] =
    clutch_general_v2_contract::TREASURY_LEDGER_SEED_DOMAIN_V1;
/// Disabled buyer-first candidate settlement cash-pot seed prefix.
pub const SEED_GENERAL_V2_SETTLEMENT_CASH_POT: &[u8] =
    clutch_general_v2_contract::SETTLEMENT_CASH_POT_SEED_DOMAIN_V1;
/// Counted candidate-scoped General V2 SettlementRoot seed prefix.
pub const SEED_GENERAL_V2_SETTLEMENT_ROOT: &[u8] =
    clutch_general_v2_contract::SETTLEMENT_ROOT_SEED_DOMAIN_V1;

/// Single-custody failure semantic root, keyed by V2 market and generation.
pub const SEED_FAILURE_EXTERNAL_ROOT: &[u8] = b"dc:failure-root:v2";
/// Shared-Market Failure admission root successor, disjoint from legacy V1.
pub const SEED_FAILURE_MARKET_ROOT_V2: &[u8] = b"dc:failure-market-root:v2";
/// Immutable runtime-liveness policy account.
pub const SEED_FAILURE_LIVENESS_POLICY: &[u8] = b"dc:failure-live-policy:v1";
/// Sole external Recovery work/rent custody account.
pub const SEED_FAILURE_EXTERNAL_RECOVERY: &[u8] = b"dc:failure-recovery:v1";
/// Permanent failure-generation replay tombstone.
pub const SEED_FAILURE_REPLAY_TOMBSTONE: &[u8] = b"dc:failure-replay:v1";
/// Dedicated exhaustive interval-consensus work lifecycle.
pub const SEED_FAILURE_INTERVAL_CONSENSUS_WORK: &[u8] = b"dc:failure-interval-work:v1";
/// Permanent exhaustive interval-consensus replay receipt.
pub const SEED_FAILURE_INTERVAL_CONSENSUS_REPLAY: &[u8] = b"dc:failure-interval-replay:v1";

/// Canonical Realm address and bump.
pub fn realm_pda(program_id: &Pubkey, realm: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_REALM, realm])
}

/// Canonical Profile address and bump.
pub fn profile_pda(program_id: &Pubkey, realm: &[u8; 32], profile: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_PROFILE, realm, profile])
}

/// Canonical Market address and bump.
pub fn market_pda(program_id: &Pubkey, realm: &[u8; 32], market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_MARKET, realm, market])
}

/// Canonical Hoard address and bump.
pub fn hoard_pda(program_id: &Pubkey, market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_HOARD, market])
}

/// Canonical full-width Hoard V2 address.
pub fn hoard_v2_pda(program_id: &Pubkey, market_instance_v2_id: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_HOARD_V2, market_instance_v2_id])
}

/// Canonical Position address and bump.
pub fn position_pda(program_id: &Pubkey, market: &[u8; 32], owner: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_POSITION, market, owner])
}

/// Canonical reference-only kernel-aggregate address and bump.
pub fn kernel_pda(program_id: &Pubkey, market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_KERNEL, market])
}

/// Canonical full-width ClaimLedger V3 address.
pub fn claim_ledger_v3_pda(program_id: &Pubkey, market_instance_v2_id: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_CLAIM_LEDGER_V3, market_instance_v2_id])
}

/// Canonical full-width Resolution V5 address.
pub fn resolution_v5_pda(program_id: &Pubkey, market_instance_v2_id: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_RESOLUTION_V5, market_instance_v2_id])
}

/// Canonical reference-only external-shadow address and bump.
pub fn external_pda(
    program_id: &Pubkey,
    market: &[u8; 32],
    owner: &[u8; 32],
    generation: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_EXTERNAL, market, owner, &generation.to_le_bytes()],
    )
}

/// Canonical reference-only replay-sequence address and bump.
pub fn replay_pda(
    program_id: &Pubkey,
    market: &[u8; 32],
    owner: &[u8; 32],
    generation: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_REPLAY, market, owner, &generation.to_le_bytes()],
    )
}

/// Canonical single-custody failure semantic root.
pub fn failure_external_root_pda(
    program_id: &Pubkey,
    market_instance_v2_id: &[u8; 32],
    generation: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_FAILURE_EXTERNAL_ROOT,
            market_instance_v2_id,
            &generation.to_le_bytes(),
        ],
    )
}

/// Canonical shared-Market Failure admission root successor.
pub fn failure_market_root_v2_pda(
    program_id: &Pubkey,
    market_instance_v2_id: &[u8; 32],
    generation: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_FAILURE_MARKET_ROOT_V2,
            market_instance_v2_id,
            &generation.to_le_bytes(),
        ],
    )
}

/// Canonical immutable runtime-liveness policy account.
pub fn failure_liveness_policy_pda(program_id: &Pubkey, policy_id: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_FAILURE_LIVENESS_POLICY, policy_id])
}

/// Canonical sole Recovery work/rent custody account.
pub fn failure_external_recovery_pda(
    program_id: &Pubkey,
    lifecycle_id: &[u8; 32],
    generation: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_FAILURE_EXTERNAL_RECOVERY,
            lifecycle_id,
            &generation.to_le_bytes(),
        ],
    )
}

/// Canonical permanent replay tombstone for one failure generation.
pub fn failure_replay_tombstone_pda(
    program_id: &Pubkey,
    market_instance_v2_id: &[u8; 32],
    generation: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_FAILURE_REPLAY_TOMBSTONE,
            market_instance_v2_id,
            &generation.to_le_bytes(),
        ],
    )
}

/// Canonical mutable interval-consensus work PDA for one Failure generation.
pub fn failure_interval_consensus_work_pda(
    program_id: &Pubkey,
    market_instance_v2_id: &[u8; 32],
    generation: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_FAILURE_INTERVAL_CONSENSUS_WORK,
            market_instance_v2_id,
            &generation.to_le_bytes(),
        ],
    )
}

/// Canonical permanent interval-consensus replay PDA for one Failure generation.
pub fn failure_interval_consensus_replay_pda(
    program_id: &Pubkey,
    market_instance_v2_id: &[u8; 32],
    generation: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_FAILURE_INTERVAL_CONSENSUS_REPLAY,
            market_instance_v2_id,
            &generation.to_le_bytes(),
        ],
    )
}

/// Canonical market-wide supply-ledger address and bump.
///
/// One ledger per market, not per position: the two-term ledger is a
/// market-wide aggregate, so its address must not carry an owner.
pub fn supply_pda(program_id: &Pubkey, market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_SUPPLY, market])
}

/// Canonical feed-head address and bump.
///
/// A feed is a Realm-scoped shared cursor, so the feed identity alone names it.
pub fn feed_pda(program_id: &Pubkey, feed: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_FEED, feed])
}

/// Canonical immutable source-spec address and bump.
pub fn source_spec_pda(program_id: &Pubkey, feed: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_SOURCE_SPEC, feed])
}

/// Canonical sealed source-archive address and bump for one exact window.
pub fn source_archive_pda(program_id: &Pubkey, feed: &[u8; 32], window: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_SOURCE_ARCHIVE, feed, window])
}

/// Canonical single-active-work address for one Market.
pub fn resolution_work_pda(program_id: &Pubkey, market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_RESOLUTION_WORK, market])
}

/// Canonical prepaid Reserve for one Market and its deterministic Work PDA.
pub fn resolution_reserve_pda(
    program_id: &Pubkey,
    market: &[u8; 32],
    work: &[u8; 32],
) -> (Pubkey, u8) {
    find(program_id, &[SEED_RESOLUTION_RESERVE, market, work])
}

/// Canonical immutable General V2 MarketBinding PDA.
pub fn general_v2_market_binding_pda(
    program_id: &Pubkey,
    market_instance_v2_id: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_GENERAL_V2_MARKET_BINDING, market_instance_v2_id],
    )
}

/// Canonical genesis-assisted General V2 Market runtime PDA.
pub fn general_v2_market_runtime_pda(
    program_id: &Pubkey,
    market_binding: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_GENERAL_V2_MARKET_RUNTIME, market_binding],
    )
}

/// Canonical counted General V2 Epoch PDA.
pub fn general_v2_epoch_pda(
    program_id: &Pubkey,
    market_binding: &[u8; 32],
    epoch_index: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_GENERAL_V2_EPOCH,
            market_binding,
            &epoch_index.to_le_bytes(),
        ],
    )
}

/// Canonical General V2 EconomicDomain artifact PDA.
pub fn general_v2_economic_domain_pda(program_id: &Pubkey, epoch: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_GENERAL_V2_ECONOMIC_DOMAIN, epoch])
}

/// Canonical General V2 candidate Window PDA.
pub fn general_v2_window_pda(program_id: &Pubkey, epoch: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_GENERAL_V2_WINDOW, epoch])
}

/// Canonical General V2 root Budget PDA.
pub fn general_v2_budget_pda(program_id: &Pubkey, epoch: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_GENERAL_V2_BUDGET, epoch])
}

/// Canonical General V2 ordinal-owned AdmissionNode PDA.
pub fn general_v2_node_pda(program_id: &Pubkey, epoch: &[u8; 32], ordinal: u64) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_GENERAL_V2_NODE, epoch, &ordinal.to_le_bytes()],
    )
}

/// Canonical General V2 Feed/Stage PDA inherited from its AdmissionNode.
pub fn general_v2_feed_pda(program_id: &Pubkey, node: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_GENERAL_V2_FEED, node])
}

/// Canonical General V2 ClearWork PDA inherited from its AdmissionNode.
pub fn general_v2_work_pda(program_id: &Pubkey, node: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_GENERAL_V2_WORK, node])
}

/// Disabled canonical General V2 resumable ClearWork V3 PDA.
pub fn general_v2_work_v3_pda(program_id: &Pubkey, node: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_GENERAL_V2_WORK_V3, node])
}

/// Canonical General V2 selected settlement-authority PDA.
pub fn general_v2_selected_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    settlement_candidate_id: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_GENERAL_V2_SELECTED, epoch, settlement_candidate_id],
    )
}

/// Canonical disabled General V2 OrderPage V5 PDA.
///
/// The authenticated Epoch owns the MarketRuntime and page-set lifecycle, so
/// the zero-based page index is the only suffix.
pub fn general_v2_order_page_v5_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    page_index: u16,
) -> (Pubkey, u8) {
    let page_index_le = page_index.to_le_bytes();
    find(
        program_id,
        &[SEED_GENERAL_V2_ORDER_PAGE_V5, epoch, &page_index_le],
    )
}

/// Canonical disabled General V2 Reservation V3 PDA.
///
/// The semantic identity already commits MarketRuntime, Epoch, owner,
/// Position generation, and order ID; none is repeated as a second address
/// projection.
pub fn general_v2_reservation_v3_pda(
    program_id: &Pubkey,
    reservation_id: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_GENERAL_V2_RESERVATION_V3, reservation_id],
    )
}

/// Canonical disabled rent-owned General V2 Reservation V9 PDA.
///
/// The fresh V9 semantic identity already commits MarketRuntime, Epoch,
/// owner, Position generation, and order ID. Versions 5 and 7 retain their
/// historical meanings and cannot alias this address family.
pub fn general_v2_reservation_v9_pda(
    program_id: &Pubkey,
    reservation_id: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_GENERAL_V2_RESERVATION_V9, reservation_id],
    )
}

/// Canonical disabled General SettlementReceipt V3 PDA.
pub fn general_v2_receipt_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    settlement_candidate: &[u8; 32],
    slice_index: u16,
) -> (Pubkey, u8) {
    let slice_index_le = slice_index.to_le_bytes();
    find(
        program_id,
        &[
            SEED_GENERAL_V2_RECEIPT,
            epoch,
            settlement_candidate,
            &slice_index_le,
        ],
    )
}

/// Canonical disabled rent-owned General SettlementReceipt V5 PDA.
pub fn general_v2_receipt_v5_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    settlement_candidate: &[u8; 32],
    slice_index: u16,
) -> (Pubkey, u8) {
    let slice_index_le = slice_index.to_le_bytes();
    find(
        program_id,
        &[
            SEED_GENERAL_V2_RECEIPT_V5,
            epoch,
            settlement_candidate,
            &slice_index_le,
        ],
    )
}

/// Canonical disabled presence-explicit owner-settlement address for one
/// selected owner row.
///
/// The withdrawn V1 row used a different seed domain. Keeping V2 on its own
/// address prevents a prefunded or historical V1 account from being promoted
/// into the zero-price-safe successor by changing only its outer version.
pub fn general_v2_owner_settlement_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    settlement_candidate: &[u8; 32],
    owner: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_GENERAL_V2_OWNER_SETTLEMENT,
            epoch,
            settlement_candidate,
            owner,
        ],
    )
}

/// Canonical disabled rent-owned OwnerSettlement V5 PDA.
pub fn general_v2_owner_settlement_v5_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    settlement_candidate: &[u8; 32],
    owner: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_GENERAL_V2_OWNER_SETTLEMENT_V5,
            epoch,
            settlement_candidate,
            owner,
        ],
    )
}

/// Canonical global Position V3 address for one General owner.
pub fn position_v3_pda(
    program_id: &Pubkey,
    market_instance: &[u8; 32],
    owner: &[u8; 32],
    purpose: clutch_retirement::PositionPurposeV3,
    purpose_binding: &[u8; 32],
) -> (Pubkey, u8) {
    let purpose_seed = [u8::from(purpose)];
    find(
        program_id,
        &[
            clutch_retirement::POSITION_V3_PDA_PREFIX,
            market_instance,
            owner,
            &purpose_seed,
            purpose_binding,
        ],
    )
}

/// Canonical purpose-owned Replay V3 address paired with one Position.
pub fn purpose_replay_v3_pda(
    program_id: &Pubkey,
    position: &[u8; 32],
    purpose: clutch_retirement::PositionPurposeV3,
    purpose_binding: &[u8; 32],
) -> (Pubkey, u8) {
    let purpose_seed = [u8::from(purpose)];
    find(
        program_id,
        &[
            clutch_retirement::PURPOSE_REPLAY_V3_PDA_PREFIX,
            position,
            &purpose_seed,
            purpose_binding,
        ],
    )
}

/// Canonical disabled selected fee-record address for one SelectedCandidate.
pub fn general_v2_selected_fee_record_pda(
    program_id: &Pubkey,
    selected_candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_GENERAL_V2_SELECTED_FEE_RECORD, selected_candidate],
    )
}

/// Canonical disabled carry address keyed only by fee record and owner.
pub fn general_v2_owner_fee_carry_pda(
    program_id: &Pubkey,
    fee_record: &[u8; 32],
    owner: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_GENERAL_V2_OWNER_FEE_CARRY, fee_record, owner],
    )
}

/// Canonical disabled temporary payer-allocation address for one owner.
pub fn general_v2_payer_allocation_pda(
    program_id: &Pubkey,
    fee_record: &[u8; 32],
    owner: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_GENERAL_V2_PAYER_ALLOCATION, fee_record, owner],
    )
}

/// Canonical disabled candidate-wide recipient-allocation address.
pub fn general_v2_recipient_allocation_pda(
    program_id: &Pubkey,
    fee_record: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_GENERAL_V2_RECIPIENT_ALLOCATION, fee_record],
    )
}

/// Canonical disabled treasury-ledger address for one selected fee record.
pub fn general_v2_treasury_ledger_pda(program_id: &Pubkey, fee_record: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_GENERAL_V2_TREASURY_LEDGER, fee_record])
}

/// Canonical disabled buyer-first cash-pot address for one final candidate.
pub fn general_v2_settlement_cash_pot_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    settlement_candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_GENERAL_V2_SETTLEMENT_CASH_POT,
            epoch,
            settlement_candidate,
        ],
    )
}

/// Canonical counted General V2 SettlementRoot PDA.
pub fn general_v2_settlement_root_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    settlement_candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_GENERAL_V2_SETTLEMENT_ROOT, epoch, settlement_candidate],
    )
}

/// Canonical immutable-terms address and bump.
///
/// Terms are content-addressed by their own digest and namespaced by the Realm
/// that authored them, so the address is a function of exactly those two facts
/// and never of the market that later binds them.  One terms artifact can
/// therefore be shared by many markets without being re-uploaded.
pub fn terms_pda(program_id: &Pubkey, realm: &[u8; 32], terms: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_TERMS, realm, terms])
}

/// Canonical frozen price-grid address and bump.
///
/// Content-addressed by the grid digest, namespaced by Realm, for the same
/// reason as [`terms_pda`].
pub fn grid_pda(program_id: &Pubkey, realm: &[u8; 32], grid: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_GRID, realm, grid])
}

/// Canonical raw collateral-policy address and bump.
///
/// `profile` is the parent Profile identity recomputed from the policy digest.
/// The raw policy codec has no stored bump; callers authenticate this address
/// and recompute both identities from the exact bytes.
pub fn policy_pda(program_id: &Pubkey, profile: &[u8; 32], digest: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_POLICY, profile, digest])
}

/// Canonical uploader-keyed stage for one exact Dealer policy identity.
pub fn dealer_policy_stage_pda(
    program_id: &Pubkey,
    artifact_kind: u8,
    funder: &[u8; 32],
    policy_id: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_DEALER_POLICY_STAGE,
            &[artifact_kind],
            funder,
            policy_id,
        ],
    )
}

/// Canonical immutable Dealer policy address from the pure-contract recipe.
pub fn dealer_policy_pda(program_id: &Pubkey, policy_id: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DEALER_POLICY, policy_id])
}

/// Canonical immutable Dealer liveness-schedule address.
pub fn dealer_liveness_schedule_pda(program_id: &Pubkey, schedule_id: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DEALER_LIVENESS_SCHEDULE, schedule_id])
}

/// Canonical immutable generic runtime-liveness policy address.
pub fn dealer_runtime_liveness_policy_pda(
    program_id: &Pubkey,
    policy_id: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_DEALER_RUNTIME_LIVENESS_POLICY, policy_id],
    )
}

/// Canonical facility-scoped runtime-liveness compartment address.
pub fn dealer_runtime_liveness_account_pda(
    program_id: &Pubkey,
    facility_id: &[u8; 32],
    compartment: u8,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_DEALER_RUNTIME_LIVENESS_ACCOUNT,
            facility_id,
            &[compartment],
        ],
    )
}

/// Canonical authoritative Dealer StateV2 address.
pub fn dealer_state_v2_pda(program_id: &Pubkey, facility_id: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DEALER_STATE_V2, facility_id])
}

/// Canonical counted funded-dependency address.
pub fn dealer_funded_v2_pda(program_id: &Pubkey, facility_id: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DEALER_FUNDED_V2, facility_id])
}

/// Canonical Dealer LP page V2 address.
pub fn dealer_lp_page_v2_pda(
    program_id: &Pubkey,
    facility_id: &[u8; 32],
    page_ordinal: u32,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_DEALER_LP_PAGE_V2,
            facility_id,
            &page_ordinal.to_le_bytes(),
        ],
    )
}

/// Canonical Dealer LeaseV2 address.
pub fn dealer_lease_v2_pda(
    program_id: &Pubkey,
    facility_id: &[u8; 32],
    generation: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_DEALER_LEASE_V2, facility_id, &generation.to_le_bytes()],
    )
}

/// Canonical Dealer SettlementPotV2 address.
pub fn dealer_pot_v2_pda(
    program_id: &Pubkey,
    facility_id: &[u8; 32],
    generation: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_DEALER_POT_V2, facility_id, &generation.to_le_bytes()],
    )
}

/// Canonical Dealer Epoch-binding V2 address.
pub fn dealer_epoch_v2_pda(
    program_id: &Pubkey,
    facility_id: &[u8; 32],
    generation: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_DEALER_EPOCH_V2, facility_id, &generation.to_le_bytes()],
    )
}

/// Canonical Dealer page-scoped terminal-allocation address.
pub fn dealer_terminal_allocation_pda(
    program_id: &Pubkey,
    facility_id: &[u8; 32],
    page_ordinal: u32,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_DEALER_TERMINAL_ALLOCATION,
            facility_id,
            &page_ordinal.to_le_bytes(),
        ],
    )
}

/// Canonical Dealer singleton ClaimWork address.
pub fn dealer_claim_work_pda(program_id: &Pubkey, facility_id: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DEALER_CLAIM_WORK, facility_id])
}

/// Canonical Dealer permanent root-tombstone address.
pub fn dealer_root_v2_pda(program_id: &Pubkey, facility_id: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DEALER_ROOT_V2, facility_id])
}

/// Canonical Dealer owner-scoped ExitTicket address.
pub fn dealer_exit_ticket_pda(
    program_id: &Pubkey,
    facility_id: &[u8; 32],
    owner: &[u8; 32],
) -> (Pubkey, u8) {
    find(program_id, &[SEED_DEALER_EXIT_TICKET, facility_id, owner])
}

/// Canonical content-addressed Dealer action-work receipt address.
pub fn dealer_action_receipt_pda(program_id: &Pubkey, slot_id: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DEALER_ACTION_RECEIPT, slot_id])
}

/// Canonical full-width batch-policy artifact address.
pub fn batch_policy_pda(program_id: &Pubkey, epoch: &[u8; 32], digest: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_BATCH_POLICY, epoch, digest])
}

/// Canonical immutable successor Product/Series artifact address.
pub fn product_artifact_pda(program_id: &Pubkey, kind: u8, digest: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_PRODUCT_ARTIFACT_V1, &[kind], digest])
}

/// Canonical immutable liveness policy selected by a Source release.
pub fn source_liveness_policy_pda(program_id: &Pubkey, policy_id: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_SOURCE_LIVENESS_POLICY_V1, policy_id])
}

/// Canonical immutable registered-Series address.
pub fn series_registry_pda(program_id: &Pubkey, series: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_SERIES_REGISTRY_V1, series])
}

/// Canonical mutable funding-state address for one registered Series.
pub fn series_funding_pda(program_id: &Pubkey, series: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_SERIES_FUNDING_V1, series])
}

/// Canonical zero-data lamport custody address for one funding component.
///
/// `component` is the exact `SeriesFundingComponentV1` discriminant `0..=4`;
/// the instruction parser must refuse every other byte before deriving it.
pub fn series_lamport_vault_pda(
    program_id: &Pubkey,
    series: &[u8; 32],
    component: u8,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_SERIES_LAMPORT_VAULT_V1, series, &[component]],
    )
}

/// Canonical sole signing authority for one Series' five collateral vaults.
pub fn series_collateral_authority_pda(program_id: &Pubkey, series: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_SERIES_COLLATERAL_AUTHORITY_V1, series])
}

/// Canonical release-selected collateral vault for one funding component.
///
/// Address allocation is owned here; token-account semantic admission remains
/// the collateral adapter's typed boundary.
pub fn series_collateral_vault_pda(
    program_id: &Pubkey,
    series: &[u8; 32],
    component: u8,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_SERIES_COLLATERAL_VAULT_V1, series, &[component]],
    )
}

/// Canonical immutable SourcePlane provenance account for one compiled record.
pub fn source_occurrence_pda(program_id: &Pubkey, source_occurrence_id: &[u8; 32]) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_SOURCE_OCCURRENCE_V1, source_occurrence_id],
    )
}

/// Canonical Product occurrence root for one full-width Market generation.
pub fn product_occurrence_root_pda(
    program_id: &Pubkey,
    market_instance_id: &[u8; 32],
    generation: u64,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_PRODUCT_OCCURRENCE_ROOT_V1,
            market_instance_id,
            &generation.to_le_bytes(),
        ],
    )
}

/// Canonical full-width Resolution V5 account for one Market occurrence.
pub fn resolution_v5_pda(program_id: &Pubkey, market_instance_id: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_RESOLUTION_V5, market_instance_id])
}

/// Canonical DirectBatchPolicy V3 artifact address.
pub fn direct_batch_policy_v3_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    digest: &[u8; 32],
) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_BATCH_POLICY_V3, epoch, digest])
}

/// Canonical direct candidate-window address.
pub fn direct_window_pda(program_id: &Pubkey, epoch: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_WINDOW, epoch])
}

/// Canonical full-width verified direct-candidate address.
pub fn direct_candidate_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_CANDIDATE, epoch, candidate])
}

/// Canonical direct receipt address for the sole direct slice.
pub fn direct_receipt_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_DIRECT_RECEIPT, epoch, candidate, &0u16.to_le_bytes()],
    )
}

/// Canonical closed direct-pot address.
pub fn direct_pot_pda(program_id: &Pubkey, epoch: &[u8; 32], candidate: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_POT, epoch, candidate])
}

/// Canonical Direct V3 candidate-window address.
pub fn direct_window_v3_pda(program_id: &Pubkey, epoch: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_WINDOW_V3, epoch])
}

/// Canonical Direct V3 verified-candidate address.
pub fn direct_candidate_v3_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_CANDIDATE_V3, epoch, candidate])
}

/// Canonical finite WorkBudget address for one Direct V3 Epoch.
pub fn direct_work_v3_pda(program_id: &Pubkey, epoch: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_WORK_V3, epoch])
}

/// Canonical Direct V3 receipt address for the sole selected slice.
pub fn direct_receipt_v3_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_DIRECT_RECEIPT_V3,
            epoch,
            candidate,
            &0u16.to_le_bytes(),
        ],
    )
}

/// Canonical Direct V3 zero-pot address for one selected Candidate.
pub fn direct_pot_v3_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(program_id, &[SEED_DIRECT_POT_V3, epoch, candidate])
}

/// Canonical uploader-scoped staging address and bump.
///
/// Uploader scoping keeps an abandoned partial body from poisoning the one
/// final content-derived address shared by all uploaders. The stage header
/// re-binds every seed component before admitting a write, seal, or abort.
pub fn artifact_stage_pda(
    program_id: &Pubkey,
    uploader: &[u8; 32],
    kind: &[u8; 1],
    context: &[u8; 32],
    digest: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_ARTIFACT_STAGE, uploader, kind, context, digest],
    )
}

/// Canonical resolution-record address and bump.
///
/// Exactly one resolution record per market: a second address for the same
/// market would be a second place a payout could be decided.
pub fn resolution_pda(program_id: &Pubkey, market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_RESOLUTION, market])
}

/// Canonical epoch address and bump.
///
/// The epoch index rather than the epoch identity is the seed, because
/// `clutch_solana_layout::canonical_epoch_id` already derives the identity from
/// exactly `(market, index)`.  Seeding on the index keeps the address derivable
/// by a caller that has not yet fetched the epoch account.
pub fn epoch_pda(program_id: &Pubkey, market: &[u8; 32], epoch_index: u64) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_EPOCH, market, &epoch_index.to_le_bytes()],
    )
}

/// Canonical order-page address and bump.
///
/// The epoch identity already binds the market, so the market is not repeated.
pub fn page_pda(program_id: &Pubkey, epoch: &[u8; 32], page_index: u16) -> (Pubkey, u8) {
    find(program_id, &[SEED_PAGE, epoch, &page_index.to_le_bytes()])
}

/// Canonical general-epoch deadline-window address and bump.
///
/// The same seed tuple as [`epoch_pda`] under its own prefix: one window per
/// epoch, derivable by a caller that has not yet fetched either account.
pub fn epoch_window_pda(program_id: &Pubkey, market: &[u8; 32], epoch_index: u64) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_EPOCH_WINDOW, market, &epoch_index.to_le_bytes()],
    )
}

/// Canonical per-order reservation address and bump.
///
/// The layout-owned reservation digest already commits to market, epoch,
/// owner, Position generation, and order id. Seeding on that one fixed-width
/// identity keeps the PDA tuple short without truncating any coordinate.
pub fn reservation_pda(program_id: &Pubkey, reservation: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_RESERVATION, reservation])
}

/// Canonical candidate-record address and bump.
///
/// A candidate is content-addressed by the digest of its free coordinates, and
/// many candidates compete inside one epoch, so both identities are seeds.
pub fn candidate_pda(program_id: &Pubkey, epoch: &[u8; 32], candidate: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_CANDIDATE, epoch, candidate])
}

/// Canonical candidate-feed address.
pub fn candidate_feed_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    candidate: &[u8; 32],
) -> (Pubkey, u8) {
    find(program_id, &[SEED_CANDIDATE_FEED, epoch, candidate])
}

/// Canonical clearing-checkpoint address and bump.
///
/// Exactly one resumable checkpoint per `(epoch, candidate)` — the same tuple
/// that addresses the candidate record and its feed, because the checkpoint
/// verifies exactly one candidate against one frozen epoch.  The market is
/// bound through the epoch identity rather than repeated, matching
/// [`candidate_pda`].  A PDA rather than a keypair address is the design
/// decision of `docs/implementation/SOLANA_LAYOUT.md`'s staged-creation
/// analysis: a keypair-addressed checkpoint is substitutable, a PDA at this
/// tuple is not.
pub fn clear_work_pda(program_id: &Pubkey, epoch: &[u8; 32], candidate: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_CLEAR_WORK, epoch, candidate])
}

/// Canonical final-pot address and bump.
///
/// One pot per epoch.  The pot names the selected candidate in its bytes rather
/// than in its address, so selecting a candidate cannot move the pot.
pub fn pot_pda(program_id: &Pubkey, epoch: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_POT, epoch])
}

/// Canonical settlement-receipt address and bump.
///
/// One receipt per frozen slice of one candidate, so re-settling a slice
/// collides with the existing account instead of minting a second receipt.
pub fn receipt_pda(
    program_id: &Pubkey,
    epoch: &[u8; 32],
    candidate: &[u8; 32],
    slice_index: u16,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_RECEIPT, epoch, candidate, &slice_index.to_le_bytes()],
    )
}

/// Canonical general funding-ledger address and bump.
///
/// Keyed by the funded account's own address: exactly one recorded funding
/// per machinery account, and a close route can re-derive its ledger from the
/// account it is closing with no other input.
pub fn general_funding_pda(program_id: &Pubkey, target: &Pubkey) -> (Pubkey, u8) {
    find(program_id, &[SEED_GENERAL_FUNDING, &target.to_bytes()])
}

/// Canonical per-Realm revenue-policy record address and bump.
///
/// Exactly one record per Realm, ever: the record is created only inside the
/// same `InitRealm` transition that creates the Realm (D4 — no retrofit),
/// and its absence is the zero-take state.
pub fn revenue_policy_pda(program_id: &Pubkey, realm: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_REVENUE_POLICY, realm])
}

/* ------------------------------------------------------------------------ */
/* Token plane — PROPOSED appends                                            */
/* ------------------------------------------------------------------------ */

/* `docs/implementation/TOKEN2022_PLAN.md` §3.2 records that this module "is
 * missing every token address" and proposes exactly three.  They are appended
 * here, unfrozen like every other seed above, with one correction the plan
 * could not make because it never derived an address: **the proposed
 * `"dragons-clutch:hoard-authority:v1"` is 33 bytes and cannot be a seed.**  A
 * single seed is capped at 32 bytes, so `find_program_address` refuses it
 * outright — on-chain that is a panic, not a refusal code.  The prefix is
 * therefore `"dragons-clutch:hoard-auth:v1"` (28 bytes).  The other two are the
 * plan's, unaltered, at 30 and 29 bytes.
 *
 * The Hoard *authority* and the Hoard *token account* stay two addresses, for
 * the reason the plan gives: collapsing them makes the signing seeds and the
 * account seeds the same bytes. */

/// Outcome-mint seed prefix; 30 bytes.
pub const SEED_OUTCOME_MINT: &[u8] = b"dragons-clutch:outcome-mint:v1";
/// Full-width MarketInstanceV2 outcome-mint seed prefix.
pub const SEED_OUTCOME_MINT_V2: &[u8] = b"dc:outcome-mint:v2";
/// Hoard signing-authority seed prefix; 28 bytes.
///
/// Shortened from the plan's `hoard-authority`, which does not fit a seed.
pub const SEED_HOARD_AUTHORITY: &[u8] = b"dragons-clutch:hoard-auth:v1";
/// Full-width Hoard V2 signing authority.
pub const SEED_HOARD_AUTHORITY_V2: &[u8] = b"dc:hoard-auth:v2";
/// Hoard token-account seed prefix; 29 bytes.
pub const SEED_HOARD_TOKEN: &[u8] = b"dragons-clutch:hoard-token:v1";
/// Full-width Hoard V2 collateral token account.
pub const SEED_HOARD_TOKEN_V2: &[u8] = b"dc:hoard-token:v2";

/// Canonical outcome-mint address and bump.
///
/// One mint per `(market, outcome index)`.  The index rather than the outcome
/// identity is the seed, for the same reason [`epoch_pda`] seeds on the index:
/// the address stays derivable by a caller that has not fetched the market
/// account, and `MarketAccount::outcomes` already binds index to identity.
pub fn outcome_mint_pda(program_id: &Pubkey, market: &[u8; 32], outcome_index: u8) -> (Pubkey, u8) {
    find(program_id, &[SEED_OUTCOME_MINT, market, &[outcome_index]])
}

/// Canonical full-width outcome mint for one MarketInstanceV2 outcome.
///
/// The fresh seed domain prevents a historical lowered Market coordinate from
/// becoming authority merely because its bytes collide with a full content ID.
pub fn outcome_mint_v2_pda(
    program_id: &Pubkey,
    market_instance_v2_id: &[u8; 32],
    outcome_index: u8,
) -> (Pubkey, u8) {
    find(
        program_id,
        &[
            SEED_OUTCOME_MINT_V2,
            market_instance_v2_id,
            &[outcome_index],
        ],
    )
}

/// Canonical Hoard signing-authority address and bump.
///
/// This is the address the program signs *as* to move collateral out of the
/// Hoard token account.  It holds no data and is never a state account.
pub fn hoard_authority_pda(program_id: &Pubkey, market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_HOARD_AUTHORITY, market])
}

/// Canonical full-width Hoard V2 signing authority.
pub fn hoard_authority_v2_pda(
    program_id: &Pubkey,
    market_instance_v2_id: &[u8; 32],
) -> (Pubkey, u8) {
    find(
        program_id,
        &[SEED_HOARD_AUTHORITY_V2, market_instance_v2_id],
    )
}

/// Canonical Hoard token-account address and bump.
///
/// The Token-2022 account [`hoard_authority_pda`] owns.  Distinct from
/// [`hoard_pda`], which is this program's own collateral-accounting state.
pub fn hoard_token_pda(program_id: &Pubkey, market: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_HOARD_TOKEN, market])
}

/// Canonical full-width Hoard V2 collateral token account.
pub fn hoard_token_v2_pda(program_id: &Pubkey, market_instance_v2_id: &[u8; 32]) -> (Pubkey, u8) {
    find(program_id, &[SEED_HOARD_TOKEN_V2, market_instance_v2_id])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A seed longer than 32 bytes is not a refusal code, it is a panic inside
    /// `find_program_address`.  Every prefix this module exports is checked
    /// here rather than by inspection, which is how the plan's proposed
    /// `hoard-authority` prefix was caught at 33 bytes.
    #[test]
    fn every_seed_prefix_fits_one_seed() {
        const PREFIXES: [&[u8]; 43] = [
            SEED_REVENUE_POLICY,
            SEED_EPOCH_WINDOW,
            SEED_REALM,
            SEED_PROFILE,
            SEED_MARKET,
            SEED_HOARD,
            SEED_POSITION,
            SEED_KERNEL,
            SEED_EXTERNAL,
            SEED_REPLAY,
            SEED_SUPPLY,
            SEED_FEED,
            SEED_TERMS,
            SEED_GRID,
            SEED_RESOLUTION,
            SEED_EPOCH,
            SEED_PAGE,
            SEED_RESERVATION,
            SEED_CANDIDATE,
            SEED_CANDIDATE_FEED,
            SEED_CLEAR_WORK,
            SEED_POT,
            SEED_RECEIPT,
            SEED_ARTIFACT_STAGE,
            SEED_POLICY,
            SEED_BATCH_POLICY,
            SEED_DIRECT_BATCH_POLICY_V3,
            SEED_DIRECT_WINDOW,
            SEED_DIRECT_CANDIDATE,
            SEED_DIRECT_RECEIPT,
            SEED_DIRECT_POT,
            SEED_DIRECT_WINDOW_V3,
            SEED_DIRECT_CANDIDATE_V3,
            SEED_DIRECT_WORK_V3,
            SEED_DIRECT_RECEIPT_V3,
            SEED_DIRECT_POT_V3,
            SEED_SOURCE_SPEC,
            SEED_SOURCE_ARCHIVE,
            SEED_RESOLUTION_WORK,
            SEED_RESOLUTION_RESERVE,
            SEED_OUTCOME_MINT,
            SEED_HOARD_AUTHORITY,
            SEED_HOARD_TOKEN,
        ];
        for prefix in PREFIXES {
            assert!(
                prefix.len() <= 32,
                "seed prefix {:?} is {} bytes and cannot be a seed",
                core::str::from_utf8(prefix).unwrap_or("<non-utf8>"),
                prefix.len()
            );
        }
        assert_eq!(SEED_HOARD_AUTHORITY.len(), 28);
        assert_eq!(SEED_OUTCOME_MINT.len(), 30);
        assert_eq!(SEED_HOARD_TOKEN.len(), 29);
        assert_eq!(SEED_BATCH_POLICY.len(), 30);
        assert_eq!(SEED_DIRECT_BATCH_POLICY_V3.len(), 19);
        assert_eq!(SEED_DIRECT_WINDOW.len(), 31);
        assert_eq!(SEED_DIRECT_CANDIDATE.len(), 31);
        assert_eq!(SEED_DIRECT_RECEIPT.len(), 32);
        assert_eq!(SEED_DIRECT_POT.len(), 28);
        assert_eq!(SEED_DIRECT_WINDOW_V3.len(), 19);
        assert_eq!(SEED_DIRECT_CANDIDATE_V3.len(), 22);
        assert_eq!(SEED_DIRECT_WORK_V3.len(), 17);
        assert_eq!(SEED_DIRECT_RECEIPT_V3.len(), 20);
        assert_eq!(SEED_DIRECT_POT_V3.len(), 16);
        assert_eq!(SEED_CLEAR_WORK.len(), 28);
        assert_eq!(SEED_EPOCH_WINDOW.len(), 30);
        // The design-named revenue-policy prefix sits exactly at the cap.
        assert_eq!(SEED_REVENUE_POLICY.len(), 32);
        // The plan's own proposal, kept here as the falsifier: it does not fit.
        assert_eq!(b"dragons-clutch:hoard-authority:v1".len(), 33);
    }

    /// The revenue-policy prefix shares an address space with nothing.
    ///
    /// `SEED_REALM` takes the *same* single-`realm` seed tuple, so prefix
    /// distinctness is exactly what keeps the Realm and its revenue record
    /// two addresses.
    #[test]
    fn the_revenue_policy_prefix_collides_with_nothing() {
        const REGISTRY: [&[u8]; 42] = [
            SEED_REALM,
            SEED_PROFILE,
            SEED_MARKET,
            SEED_HOARD,
            SEED_POSITION,
            SEED_KERNEL,
            SEED_EXTERNAL,
            SEED_REPLAY,
            SEED_SUPPLY,
            SEED_FEED,
            SEED_TERMS,
            SEED_GRID,
            SEED_RESOLUTION,
            SEED_EPOCH,
            SEED_PAGE,
            SEED_RESERVATION,
            SEED_CANDIDATE,
            SEED_CANDIDATE_FEED,
            SEED_CLEAR_WORK,
            SEED_EPOCH_WINDOW,
            SEED_POT,
            SEED_RECEIPT,
            SEED_GENERAL_FUNDING,
            SEED_ARTIFACT_STAGE,
            SEED_POLICY,
            SEED_BATCH_POLICY,
            SEED_DIRECT_BATCH_POLICY_V3,
            SEED_DIRECT_WINDOW,
            SEED_DIRECT_CANDIDATE,
            SEED_DIRECT_RECEIPT,
            SEED_DIRECT_POT,
            SEED_DIRECT_WINDOW_V3,
            SEED_DIRECT_CANDIDATE_V3,
            SEED_DIRECT_WORK_V3,
            SEED_DIRECT_RECEIPT_V3,
            SEED_DIRECT_POT_V3,
            SEED_SOURCE_SPEC,
            SEED_SOURCE_ARCHIVE,
            SEED_RESOLUTION_WORK,
            SEED_RESOLUTION_RESERVE,
            SEED_OUTCOME_MINT,
            SEED_HOARD_AUTHORITY,
        ];
        for old in REGISTRY {
            assert_ne!(SEED_REVENUE_POLICY, old);
        }
        assert_ne!(SEED_REVENUE_POLICY, SEED_HOARD_TOKEN);
    }

    /// The three token prefixes are distinct from each other and from every
    /// prefix that was already here: a shared prefix is a shared address space.
    #[test]
    fn the_token_prefixes_collide_with_nothing() {
        const EXISTING: [&[u8]; 20] = [
            SEED_REALM,
            SEED_PROFILE,
            SEED_MARKET,
            SEED_HOARD,
            SEED_POSITION,
            SEED_KERNEL,
            SEED_EXTERNAL,
            SEED_REPLAY,
            SEED_SUPPLY,
            SEED_FEED,
            SEED_TERMS,
            SEED_GRID,
            SEED_RESOLUTION,
            SEED_EPOCH,
            SEED_PAGE,
            SEED_RESERVATION,
            SEED_CANDIDATE,
            SEED_CANDIDATE_FEED,
            SEED_POT,
            SEED_RECEIPT,
        ];
        const APPENDED: [&[u8]; 3] = [SEED_OUTCOME_MINT, SEED_HOARD_AUTHORITY, SEED_HOARD_TOKEN];
        for added in APPENDED {
            for old in EXISTING {
                assert_ne!(added, old);
            }
        }
        assert_ne!(APPENDED[0], APPENDED[1]);
        assert_ne!(APPENDED[0], APPENDED[2]);
        assert_ne!(APPENDED[1], APPENDED[2]);
        /* `SEED_HOARD` is a strict prefix of `SEED_HOARD_TOKEN` as a *string*,
         * which is harmless: seeds are length-delimited arguments, not a
         * concatenation, so `["dragons-clutch:hoard:v1", m]` and
         * `["dragons-clutch:hoard-token:v1", m]` are different tuples. */
        assert!(SEED_HOARD_TOKEN.starts_with(b"dragons-clutch:hoard"));
    }

    /// Every Direct V3 namespace is disjoint from every historical direct and
    /// generic namespace. Account versions therefore cannot be squatted by a
    /// valid older PDA carrying the same semantic identity.
    #[test]
    fn direct_v3_prefixes_are_pairwise_disjoint_from_the_full_registry() {
        const HISTORICAL: [&[u8]; 35] = [
            SEED_REALM,
            SEED_PROFILE,
            SEED_MARKET,
            SEED_HOARD,
            SEED_POSITION,
            SEED_KERNEL,
            SEED_EXTERNAL,
            SEED_REPLAY,
            SEED_SUPPLY,
            SEED_FEED,
            SEED_TERMS,
            SEED_GRID,
            SEED_RESOLUTION,
            SEED_EPOCH,
            SEED_PAGE,
            SEED_RESERVATION,
            SEED_CANDIDATE,
            SEED_CANDIDATE_FEED,
            SEED_POT,
            SEED_RECEIPT,
            SEED_ARTIFACT_STAGE,
            SEED_POLICY,
            SEED_BATCH_POLICY,
            SEED_DIRECT_BATCH_POLICY_V3,
            SEED_DIRECT_WINDOW,
            SEED_DIRECT_CANDIDATE,
            SEED_DIRECT_RECEIPT,
            SEED_DIRECT_POT,
            SEED_SOURCE_SPEC,
            SEED_SOURCE_ARCHIVE,
            SEED_RESOLUTION_WORK,
            SEED_RESOLUTION_RESERVE,
            SEED_OUTCOME_MINT,
            SEED_HOARD_AUTHORITY,
            SEED_HOARD_TOKEN,
        ];
        const DIRECT_V3: [&[u8]; 5] = [
            SEED_DIRECT_WINDOW_V3,
            SEED_DIRECT_CANDIDATE_V3,
            SEED_DIRECT_WORK_V3,
            SEED_DIRECT_RECEIPT_V3,
            SEED_DIRECT_POT_V3,
        ];
        for (index, added) in DIRECT_V3.iter().enumerate() {
            for old in HISTORICAL {
                assert_ne!(*added, old);
            }
            for later in DIRECT_V3.iter().skip(index + 1) {
                assert_ne!(*added, *later);
            }
        }
    }

    /// The clearing-checkpoint prefix shares an address space with nothing.
    ///
    /// `SEED_CANDIDATE_FEED` and `SEED_CANDIDATE` take the *same* seed tuple
    /// `(epoch, candidate)` that `SEED_CLEAR_WORK` takes, so prefix
    /// distinctness is exactly what keeps the three accounts three addresses.
    #[test]
    fn the_clear_work_prefix_collides_with_nothing() {
        const REGISTRY: [&[u8]; 40] = [
            SEED_REALM,
            SEED_PROFILE,
            SEED_MARKET,
            SEED_HOARD,
            SEED_POSITION,
            SEED_KERNEL,
            SEED_EXTERNAL,
            SEED_REPLAY,
            SEED_SUPPLY,
            SEED_FEED,
            SEED_TERMS,
            SEED_GRID,
            SEED_RESOLUTION,
            SEED_EPOCH,
            SEED_PAGE,
            SEED_RESERVATION,
            SEED_CANDIDATE,
            SEED_CANDIDATE_FEED,
            SEED_POT,
            SEED_RECEIPT,
            SEED_ARTIFACT_STAGE,
            SEED_POLICY,
            SEED_BATCH_POLICY,
            SEED_DIRECT_BATCH_POLICY_V3,
            SEED_DIRECT_WINDOW,
            SEED_DIRECT_CANDIDATE,
            SEED_DIRECT_RECEIPT,
            SEED_DIRECT_POT,
            SEED_DIRECT_WINDOW_V3,
            SEED_DIRECT_CANDIDATE_V3,
            SEED_DIRECT_WORK_V3,
            SEED_DIRECT_RECEIPT_V3,
            SEED_DIRECT_POT_V3,
            SEED_SOURCE_SPEC,
            SEED_SOURCE_ARCHIVE,
            SEED_RESOLUTION_WORK,
            SEED_RESOLUTION_RESERVE,
            SEED_OUTCOME_MINT,
            SEED_HOARD_AUTHORITY,
            SEED_HOARD_TOKEN,
        ];
        for old in REGISTRY {
            assert_ne!(SEED_CLEAR_WORK, old);
        }
    }

    /// The window prefix shares an address space with nothing.
    ///
    /// `SEED_EPOCH` takes the *same* seed tuple `(market, epoch_index)` the
    /// window takes, so prefix distinctness is exactly what keeps the epoch
    /// and its deadline window two addresses.
    #[test]
    fn the_epoch_window_prefix_collides_with_nothing() {
        const REGISTRY: [&[u8]; 41] = [
            SEED_REALM,
            SEED_PROFILE,
            SEED_MARKET,
            SEED_HOARD,
            SEED_POSITION,
            SEED_KERNEL,
            SEED_EXTERNAL,
            SEED_REPLAY,
            SEED_SUPPLY,
            SEED_FEED,
            SEED_TERMS,
            SEED_GRID,
            SEED_RESOLUTION,
            SEED_EPOCH,
            SEED_PAGE,
            SEED_RESERVATION,
            SEED_CANDIDATE,
            SEED_CANDIDATE_FEED,
            SEED_CLEAR_WORK,
            SEED_POT,
            SEED_RECEIPT,
            SEED_ARTIFACT_STAGE,
            SEED_POLICY,
            SEED_BATCH_POLICY,
            SEED_DIRECT_BATCH_POLICY_V3,
            SEED_DIRECT_WINDOW,
            SEED_DIRECT_CANDIDATE,
            SEED_DIRECT_RECEIPT,
            SEED_DIRECT_POT,
            SEED_DIRECT_WINDOW_V3,
            SEED_DIRECT_CANDIDATE_V3,
            SEED_DIRECT_WORK_V3,
            SEED_DIRECT_RECEIPT_V3,
            SEED_DIRECT_POT_V3,
            SEED_SOURCE_SPEC,
            SEED_SOURCE_ARCHIVE,
            SEED_RESOLUTION_WORK,
            SEED_RESOLUTION_RESERVE,
            SEED_OUTCOME_MINT,
            SEED_HOARD_AUTHORITY,
            SEED_HOARD_TOKEN,
        ];
        for old in REGISTRY {
            assert_ne!(SEED_EPOCH_WINDOW, old);
        }
    }

    #[test]
    fn failure_market_root_successor_cannot_alias_the_legacy_root() {
        assert_ne!(SEED_FAILURE_MARKET_ROOT_V2, SEED_FAILURE_EXTERNAL_ROOT);
        let program_id = Pubkey::new_from_array([1; 32]);
        let market = [2; 32];
        assert_ne!(
            failure_market_root_v2_pda(&program_id, &market, 3).0,
            failure_external_root_pda(&program_id, &market, 3).0,
        );
    }
}
