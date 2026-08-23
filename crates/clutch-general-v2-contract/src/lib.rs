// SPDX-License-Identifier: AGPL-3.0-or-later
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Fail-closed, fixed-memory contract for the future General V2 adapter.
//!
//! Nothing in this crate is a live Solana route. Account ownership, PDA
//! authentication, SHA-256, rent derivation, clocks, signatures, CPI, and
//! lamport movement remain adapter obligations.

mod codec;
mod epoch_retirement;
mod rank;
mod settlement_accounts;
mod state;

pub use codec::{CodecError, Reader, Writer};
pub use epoch_retirement::*;
pub use rank::{
    encode_score_v2_q_first_admitted_tie_v1, FirstAdmittedTieV1, ScoreV2QComponentsV1,
    SCORE_V2_Q_ACTIVE_RANK_BYTES, SCORE_V2_Q_RANK_CAPACITY,
};
pub use settlement_accounts::*;
pub use state::*;

/// Number of bytes in every persisted identity or digest.
pub const ID_BYTES: usize = 32;
/// Largest active outcome width inherited from RelationV2.
pub const MAX_OUTCOMES: usize = 16;
/// Largest active outcome width in its persisted representation.
pub const MAX_OUTCOMES_U8: u8 = 16;
/// Largest active order width inherited from RelationV2.
pub const MAX_ORDERS: usize = 64;
/// Largest active order width in its persisted representation.
pub const MAX_ORDERS_U8: u8 = 64;
/// Largest settlement-slice witness admitted by the existing settlement seam.
pub const MAX_SLICES: usize = 416;
/// Largest settlement-slice width in its persisted representation.
pub const MAX_SLICES_U16: u16 = 416;
/// Largest quantized price-measure support, one atom per active outcome.
pub const MAX_QUANTIZED_ATOMS: usize = MAX_OUTCOMES;
/// Largest quantized support width in its persisted representation.
pub const MAX_QUANTIZED_ATOMS_U8: u8 = MAX_OUTCOMES_U8;

/// Exact funded-admission commitment hash domain from ADR-0008.
pub const CANDIDATE_COMMITMENT_DOMAIN_V1: &[u8] = b"dragons-clutch/candidate-commitment/v1";
/// Fresh General V2 ordinal-owned admission-node PDA seed domain.
///
/// This deliberately does not reuse ADR-0008's submitter/commitment-derived
/// `candidate-admission-v3` identity. General V2 assigns the one-based ordinal
/// before derivation so duplicate candidates cannot grind a node identity.
pub const CANDIDATE_NODE_SEED_DOMAIN_V1: &[u8] = b"general-candidate-admission:v1";

/// Validated ordered seed tuple for a General V2 AdmissionNode PDA.
///
/// The adapter must pass [`Self::domain`], [`Self::epoch`], and
/// [`Self::ordinal_le`] as three distinct seeds in exactly that order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateNodeSeedTupleV1 {
    epoch: [u8; ID_BYTES],
    ordinal_le: [u8; 8],
}

impl CandidateNodeSeedTupleV1 {
    /// Construct the canonical tuple from an authenticated Epoch PDA and the
    /// Window-assigned one-based admission ordinal.
    pub fn new(epoch: Id32, ordinal: u64) -> Result<Self, CodecError> {
        if epoch.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if ordinal == 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(Self {
            epoch: epoch.bytes(),
            ordinal_le: ordinal.to_le_bytes(),
        })
    }

    /// First seed: the fresh General V2 node domain.
    pub const fn domain(&self) -> &'static [u8] {
        CANDIDATE_NODE_SEED_DOMAIN_V1
    }

    /// Second seed: full authenticated Epoch PDA bytes.
    pub const fn epoch(&self) -> &[u8; ID_BYTES] {
        &self.epoch
    }

    /// Third seed: Window-assigned one-based ordinal in little-endian order.
    pub const fn ordinal_le(&self) -> &[u8; 8] {
        &self.ordinal_le
    }
}
/// Fresh General V2 Window PDA seed domain.
pub const WINDOW_SEED_DOMAIN_V1: &[u8] = b"general-window:v4";
/// Fresh General V2 candidate-feed PDA seed domain.
pub const CANDIDATE_FEED_SEED_DOMAIN_V1: &[u8] = b"candidate-feed:v2";
/// Fresh General V2 ClearWork PDA seed domain.
pub const CLEAR_WORK_SEED_DOMAIN_V1: &[u8] = b"clear-work:v2";
/// Fresh General V2 epoch-budget PDA seed domain.
pub const EPOCH_BUDGET_SEED_DOMAIN_V1: &[u8] = b"candidate-budget:v2";
/// Fresh immutable General V2 Market-binding PDA seed domain.
pub const MARKET_BINDING_SEED_DOMAIN_V1: &[u8] = b"general-market-binding:v1";
/// Fresh canonical EconomicDomainV2 artifact PDA seed domain.
pub const ECONOMIC_DOMAIN_SEED_DOMAIN_V1: &[u8] = b"economic-domain:v2";
/// Fresh selected-candidate settlement-authority PDA seed domain.
pub const SELECTED_CANDIDATE_SEED_DOMAIN_V1: &[u8] = b"selected-candidate:v1";
/// Fresh counted General V2 Epoch PDA seed domain.
pub const EPOCH_SEED_DOMAIN_V1: &[u8] = b"general-epoch:v2";
/// Fresh counted General V2 order-page PDA seed domain.
pub const ORDER_PAGE_SEED_DOMAIN_V1: &[u8] = b"general-order-page:v2";
/// Fresh counted General V2 reservation PDA seed domain.
pub const RESERVATION_SEED_DOMAIN_V1: &[u8] = b"general-reservation:v2";
/// Fresh General V2 receipt PDA seed domain.
pub const RECEIPT_SEED_DOMAIN_V1: &[u8] = b"general-receipt:v2";
/// Fresh General V2 final-pot PDA seed domain.
pub const FINAL_POT_SEED_DOMAIN_V1: &[u8] = b"general-final-pot:v2";
/// Fresh owner-settlement row seed domain.
pub const OWNER_SETTLEMENT_SEED_DOMAIN_V1: &[u8] = b"owner-settlement:v1";

/// Existing Epoch semantic tag, fresh counted General V2 version.
pub const GENERAL_EPOCH_ACCOUNT_TAG: u8 = 11;
/// Settlement-complete counted General V2 Epoch version.
pub const GENERAL_EPOCH_ACCOUNT_VERSION: u8 = 6;
/// Permanent General V2 Epoch tombstone discriminator.
pub const GENERAL_EPOCH_TOMBSTONE_ACCOUNT_TAG: u8 = 0x76;
/// Rent-owner-complete General V2 Epoch tombstone version.
pub const GENERAL_EPOCH_TOMBSTONE_ACCOUNT_VERSION: u8 = 2;
/// Existing Market semantic tag, fresh General V2 runtime version.
pub const MARKET_RUNTIME_ACCOUNT_TAG: u8 = 3;
/// First monotone General V2 Market-runtime version.
pub const MARKET_RUNTIME_ACCOUNT_VERSION: u8 = 3;
/// Corrected globally disjoint General V2 owner-settlement discriminator.
pub const OWNER_SETTLEMENT_ACCOUNT_TAG: u8 = 0x81;
/// First exact owner-settlement outer version.
pub const OWNER_SETTLEMENT_ACCOUNT_VERSION: u8 = 1;
/// Allocation-complete General V2 settlement cash-pot discriminator.
pub const SETTLEMENT_CASH_POT_ACCOUNT_TAG: u8 = 0x87;
/// First exact settlement cash-pot outer version.
pub const SETTLEMENT_CASH_POT_ACCOUNT_VERSION: u8 = 1;
/// Terminal General V2 FinalPot discriminator.
pub const FINAL_POT_ACCOUNT_TAG: u8 = 0x89;
/// First exact FinalPot outer version.
pub const FINAL_POT_ACCOUNT_VERSION: u8 = 1;

/// Existing semantic account tag, fresh successor version: Window.
pub const WINDOW_ACCOUNT_TAG: u8 = 24;
/// Codec version matching the disabled central Window reservation.
pub const WINDOW_ACCOUNT_VERSION: u8 = 4;
/// Existing semantic account tag, fresh successor version: sealed feed.
pub const CANDIDATE_FEED_ACCOUNT_TAG: u8 = 18;
/// Active-width General V2 feed version.
pub const CANDIDATE_FEED_ACCOUNT_VERSION: u8 = 2;
/// Existing semantic account tag, fresh successor version: feed stage.
pub const CANDIDATE_FEED_STAGE_ACCOUNT_TAG: u8 = 25;
/// Active-width General V2 feed-stage version.
pub const CANDIDATE_FEED_STAGE_ACCOUNT_VERSION: u8 = 2;
/// Existing semantic account tag, fresh successor version: ClearWork.
pub const CLEAR_WORK_ACCOUNT_TAG: u8 = 17;
/// Active-width General V2 ClearWork version.
pub const CLEAR_WORK_ACCOUNT_VERSION: u8 = 2;
/// Codec tag matching the disabled central admission-node reservation.
pub const ADMISSION_NODE_ACCOUNT_TAG: u8 = 0x77;
/// First funded admission-node account version.
pub const ADMISSION_NODE_ACCOUNT_VERSION: u8 = 1;
/// Codec tag matching the disabled central epoch-budget reservation.
pub const EPOCH_BUDGET_ACCOUNT_TAG: u8 = 0x78;
/// First epoch-budget account version.
pub const EPOCH_BUDGET_ACCOUNT_VERSION: u8 = 1;
/// Codec tag matching the disabled central Market-binding reservation.
pub const MARKET_BINDING_ACCOUNT_TAG: u8 = 0x79;
/// First immutable Market-binding account version.
pub const MARKET_BINDING_ACCOUNT_VERSION: u8 = 1;
/// Codec projection of the centrally owned Replay-successor account tag.
pub const REPLAY_SUCCESSOR_ACCOUNT_TAG: u8 = 0x7a;
/// First Replay-successor account version.
pub const REPLAY_SUCCESSOR_ACCOUNT_VERSION: u8 = 1;
/// Width projected by the counted-retirement seam for the Replay successor.
pub const REPLAY_SUCCESSOR_ACCOUNT_BYTES: usize = 132;
/// Codec tag matching the disabled central EconomicDomainV2 reservation.
pub const ECONOMIC_DOMAIN_ACCOUNT_TAG: u8 = 0x7b;
/// First canonical EconomicDomainV2 artifact account version.
pub const ECONOMIC_DOMAIN_ACCOUNT_VERSION: u8 = 1;
/// Codec tag matching the disabled central SelectedCandidate reservation.
pub const SELECTED_CANDIDATE_ACCOUNT_TAG: u8 = 0x7c;
/// First selected-candidate settlement-authority account version.
pub const SELECTED_CANDIDATE_ACCOUNT_VERSION: u8 = 1;

/// Immutable ownership note for one account schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountAllocationV1 {
    /// Global account discriminator.
    pub tag: u8,
    /// Exact account schema version.
    pub version: u8,
    /// Human-readable semantic owner.
    pub owner: &'static str,
}

/// Standalone codec coordinates proposed to match the central registry.
///
/// `clutch-solana-layout::registry` remains the sole global allocation owner.
/// The eventual adapter must compile-time/test-check parity before activation;
/// this dependency-free crate does not claim registry authority.
pub const ACCOUNT_ALLOCATIONS_V1: [AccountAllocationV1; 16] = [
    AccountAllocationV1 {
        tag: MARKET_RUNTIME_ACCOUNT_TAG,
        version: MARKET_RUNTIME_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/MarketRuntimeV3AccountV1",
    },
    AccountAllocationV1 {
        tag: GENERAL_EPOCH_ACCOUNT_TAG,
        version: GENERAL_EPOCH_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/GeneralEpochV6AccountV1",
    },
    AccountAllocationV1 {
        tag: GENERAL_EPOCH_TOMBSTONE_ACCOUNT_TAG,
        version: GENERAL_EPOCH_TOMBSTONE_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/GeneralEpochTombstoneV2",
    },
    AccountAllocationV1 {
        tag: OWNER_SETTLEMENT_ACCOUNT_TAG,
        version: OWNER_SETTLEMENT_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/OwnerSettlementV1AccountV1",
    },
    AccountAllocationV1 {
        tag: SETTLEMENT_CASH_POT_ACCOUNT_TAG,
        version: SETTLEMENT_CASH_POT_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/SettlementCashPotV1AccountV1",
    },
    AccountAllocationV1 {
        tag: FINAL_POT_ACCOUNT_TAG,
        version: FINAL_POT_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/GeneralV2FinalPotV1AccountV1",
    },
    AccountAllocationV1 {
        tag: WINDOW_ACCOUNT_TAG,
        version: WINDOW_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/WindowV4",
    },
    AccountAllocationV1 {
        tag: CANDIDATE_FEED_ACCOUNT_TAG,
        version: CANDIDATE_FEED_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/CandidateFeedV2",
    },
    AccountAllocationV1 {
        tag: CANDIDATE_FEED_STAGE_ACCOUNT_TAG,
        version: CANDIDATE_FEED_STAGE_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/CandidateFeedStageV2",
    },
    AccountAllocationV1 {
        tag: CLEAR_WORK_ACCOUNT_TAG,
        version: CLEAR_WORK_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/ClearWorkV2",
    },
    AccountAllocationV1 {
        tag: ADMISSION_NODE_ACCOUNT_TAG,
        version: ADMISSION_NODE_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/AdmissionNodeV3AccountV1",
    },
    AccountAllocationV1 {
        tag: EPOCH_BUDGET_ACCOUNT_TAG,
        version: EPOCH_BUDGET_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/EpochBudgetV2AccountV1",
    },
    AccountAllocationV1 {
        tag: MARKET_BINDING_ACCOUNT_TAG,
        version: MARKET_BINDING_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/MarketBindingV1",
    },
    AccountAllocationV1 {
        tag: REPLAY_SUCCESSOR_ACCOUNT_TAG,
        version: REPLAY_SUCCESSOR_ACCOUNT_VERSION,
        owner: "clutch-retirement + clutch-solana-reference/ReplaySuccessor",
    },
    AccountAllocationV1 {
        tag: ECONOMIC_DOMAIN_ACCOUNT_TAG,
        version: ECONOMIC_DOMAIN_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/EconomicDomainV2AccountV1",
    },
    AccountAllocationV1 {
        tag: SELECTED_CANDIDATE_ACCOUNT_TAG,
        version: SELECTED_CANDIDATE_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/SelectedCandidateV1AccountV1",
    },
];

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_OUTCOMES_U8 == 16);
const _: () = assert!(MAX_ORDERS == 64);
const _: () = assert!(MAX_ORDERS_U8 == 64);
const _: () = assert!(MAX_QUANTIZED_ATOMS == 16);
const _: () = assert!(MAX_QUANTIZED_ATOMS_U8 == 16);
const _: () = assert!(MAX_SLICES_U16 == 416);
