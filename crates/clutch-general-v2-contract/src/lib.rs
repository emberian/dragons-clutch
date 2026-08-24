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
mod candidate_rank_v2;
mod fee_accounts;
mod fee_rent_v3;
mod fee_terminal;
mod final_pot;
mod general_founding_policy_v1;
mod market_binding_v2;
mod market_binding_v3;
mod market_binding_v4;
mod owner_settlement;
mod payload;
mod position_replay;
mod rank;
mod settlement_root;
mod settlement_root_indexed;
mod state;
mod transition;

pub use codec::{CodecError, Reader, Writer};
pub use candidate_rank_v2::*;
pub use fee_accounts::*;
pub use fee_rent_v3::*;
pub use fee_terminal::*;
pub use final_pot::*;
pub use general_founding_policy_v1::*;
pub use market_binding_v2::*;
pub use market_binding_v3::*;
pub use market_binding_v4::*;
pub use owner_settlement::*;
pub use payload::*;
pub use position_replay::*;
pub use rank::{
    encode_score_v2_q_cost_first_admitted_tie_v1,
    encode_score_v2_q_first_admitted_tie_v1, FirstAdmittedTieV1,
    ScoreV2QComponentsV1, ScoreV2QCostComponentsV1, SCORE_V2_Q_ACTIVE_RANK_BYTES,
    SCORE_V2_Q_COST_ACTIVE_RANK_BYTES, SCORE_V2_Q_RANK_CAPACITY,
};
pub use settlement_root::*;
pub use settlement_root_indexed::*;
pub use state::*;
pub use transition::*;

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
/// Resumable RelationV2 ClearWork successor PDA seed domain.
pub const CLEAR_WORK_SEED_DOMAIN_V3: &[u8] = b"clear-work:v3";
/// Fresh General V2 epoch-budget PDA seed domain.
pub const EPOCH_BUDGET_SEED_DOMAIN_V1: &[u8] = b"candidate-budget:v2";
/// Fresh immutable General V2 Market-binding PDA seed domain.
pub const MARKET_BINDING_SEED_DOMAIN_V1: &[u8] = b"general-market-binding:v1";
/// Fresh genesis-assisted General V2 Market-runtime PDA seed domain.
pub const MARKET_RUNTIME_SEED_DOMAIN_V1: &[u8] = b"general-market-runtime:v1";
/// Fresh canonical EconomicDomainV2 artifact PDA seed domain.
pub const ECONOMIC_DOMAIN_SEED_DOMAIN_V1: &[u8] = b"economic-domain:v2";
/// Fresh selected-candidate settlement-authority PDA seed domain.
pub const SELECTED_CANDIDATE_SEED_DOMAIN_V1: &[u8] = b"selected-candidate:v1";
/// Fresh disabled owner-settlement envelope PDA seed domain.
pub const OWNER_SETTLEMENT_SEED_DOMAIN_V1: &[u8] = b"owner-settlement:v1";
/// Presence-explicit owner-settlement successor PDA seed domain.
pub const OWNER_SETTLEMENT_SEED_DOMAIN_V2: &[u8] = b"owner-settlement:v2";
/// Canonical Reservation-handoff owner-settlement PDA seed domain.
pub const OWNER_SETTLEMENT_SEED_DOMAIN_V3: &[u8] = b"owner-settlement:v3";
/// Delivery-complete owner-settlement successor PDA seed domain.
pub const OWNER_SETTLEMENT_SEED_DOMAIN_V4: &[u8] = b"owner-settlement:v4";
/// Sole future rent-owned owner-settlement PDA seed domain.
pub const OWNER_SETTLEMENT_SEED_DOMAIN_V5: &[u8] = b"owner-settlement:v5";
/// Fresh selected composite-fee record PDA seed domain.
pub const SELECTED_FEE_RECORD_SEED_DOMAIN_V1: &[u8] = b"selected-fee-record:v1";
/// Fresh owner-scoped fee carry PDA seed domain.
pub const OWNER_FEE_CARRY_SEED_DOMAIN_V1: &[u8] = b"owner-fee-carry:v1";
/// Fresh temporary owner payer-allocation PDA seed domain.
pub const PAYER_ALLOCATION_SEED_DOMAIN_V1: &[u8] = b"owner-payer-allocation:v1";
/// Fresh temporary candidate-wide recipient-allocation PDA seed domain.
pub const RECIPIENT_ALLOCATION_SEED_DOMAIN_V1: &[u8] = b"candidate-recipient-allocation:v1";
/// Fresh selected-record-scoped treasury ledger PDA seed domain.
pub const TREASURY_LEDGER_SEED_DOMAIN_V1: &[u8] = b"fee-treasury-ledger:v1";
/// Canonical streaming fee-retirement accumulator PDA domain.
pub const FEE_RETIREMENT_ACCUMULATOR_SEED_DOMAIN_V1: &[u8] = b"fee-retire-acc:v1";
/// Durable fee-closure manifest PDA domain.
pub const FEE_CLOSURE_MANIFEST_SEED_DOMAIN_V1: &[u8] = b"fee-close-manifest:v1";
/// Durable candidate-wide fee-terminal receipt PDA domain.
pub const FEE_TERMINAL_RECEIPT_SEED_DOMAIN_V1: &[u8] = b"fee-terminal-receipt:v1";
/// Fresh buyer-first candidate settlement cash-pot PDA seed domain.
pub const SETTLEMENT_CASH_POT_SEED_DOMAIN_V1: &[u8] = b"settlement-cash-pot:v1";
/// Fresh counted General V2 Epoch PDA seed domain.
pub const EPOCH_SEED_DOMAIN_V1: &[u8] = b"general-epoch:v2";
/// Fresh counted General V2 order-page PDA seed domain.
pub const ORDER_PAGE_SEED_DOMAIN_V1: &[u8] = b"general-order-page:v2";
/// Withdrawn General V2 Reservation V3 PDA seed domain.
pub const RESERVATION_SEED_DOMAIN_V1: &[u8] = b"general-reservation:v2";
/// Sole future rent-owned General Reservation V9 PDA seed domain.
pub const RESERVATION_SEED_DOMAIN_V9: &[u8] = b"general-reservation:v9";
/// Superseded provisional General V2 receipt seed domain.
pub const RECEIPT_SEED_DOMAIN_V1: &[u8] = b"general-receipt:v2";
/// Canonical General SettlementReceipt V3 PDA seed domain.
pub const RECEIPT_SEED_DOMAIN_V3: &[u8] = b"general-receipt:v3";
/// Canonical General SettlementReceipt V4 PDA seed domain.
/// V3 remains withdrawn and never aliases this fresh address family.
pub const RECEIPT_SEED_DOMAIN_V4: &[u8] = b"general-receipt:v4";
/// Sole future rent-owned General SettlementReceipt V5 PDA seed domain.
pub const RECEIPT_SEED_DOMAIN_V5: &[u8] = b"general-receipt:v5";
/// Fresh General V2 final-pot PDA seed domain.
pub const FINAL_POT_SEED_DOMAIN_V1: &[u8] = b"general-final-pot:v2";

/// Validated ordered seed tuple for one General OrderPage V5 PDA.
///
/// The page index is the only suffix because the authenticated Epoch already
/// binds the MarketRuntime and frozen order-set lifecycle. The exact V5 body
/// owns its page count, order count, and generation-bearing slot digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOrderPageSeedTupleV5 {
    epoch: [u8; ID_BYTES],
    page_index_le: [u8; 2],
}

impl GeneralOrderPageSeedTupleV5 {
    /// Construct the canonical page tuple.
    pub fn new(epoch: Id32, page_index: u16) -> Result<Self, CodecError> {
        if epoch.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        Ok(Self {
            epoch: epoch.bytes(),
            page_index_le: page_index.to_le_bytes(),
        })
    }

    /// First seed: the fresh General page domain.
    pub const fn domain(&self) -> &'static [u8] {
        ORDER_PAGE_SEED_DOMAIN_V1
    }

    /// Second seed: full authenticated Epoch PDA bytes.
    pub const fn epoch(&self) -> &[u8; ID_BYTES] {
        &self.epoch
    }

    /// Third seed: zero-based page index in little-endian order.
    pub const fn page_index_le(&self) -> &[u8; 2] {
        &self.page_index_le
    }
}

/// Validated ordered seed tuple for one canonical General Reservation V3 PDA.
///
/// The semantic Reservation identity already commits MarketRuntime, Epoch,
/// owner, Position generation, and order ID. Repeating any of those
/// coordinates as a seed would create a second identity projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReservationSeedTupleV3 {
    reservation_id: [u8; ID_BYTES],
}

impl GeneralReservationSeedTupleV3 {
    /// Construct the canonical Reservation tuple.
    pub fn new(reservation_id: Id32) -> Result<Self, CodecError> {
        if reservation_id.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        Ok(Self {
            reservation_id: reservation_id.bytes(),
        })
    }

    /// First seed: the fresh General Reservation domain.
    pub const fn domain(&self) -> &'static [u8] {
        RESERVATION_SEED_DOMAIN_V1
    }

    /// Second seed: canonical semantic Reservation identity.
    pub const fn reservation_id(&self) -> &[u8; ID_BYTES] {
        &self.reservation_id
    }
}

/// Validated ordered seed tuple for one rent-owned General Reservation V9 PDA.
///
/// The V9 semantic identity uses a fresh domain and already commits every
/// order/owner/generation coordinate. The PDA therefore needs only that exact
/// identity and the fresh non-aliasing address domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReservationSeedTupleV9 {
    reservation_id: [u8; ID_BYTES],
}

impl GeneralReservationSeedTupleV9 {
    /// Construct the sole future General Reservation tuple.
    pub fn new(reservation_id: Id32) -> Result<Self, CodecError> {
        if reservation_id.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        Ok(Self {
            reservation_id: reservation_id.bytes(),
        })
    }

    /// First seed: the fresh V9 Reservation domain.
    pub const fn domain(&self) -> &'static [u8] {
        RESERVATION_SEED_DOMAIN_V9
    }

    /// Second seed: canonical V9 semantic Reservation identity.
    pub const fn reservation_id(&self) -> &[u8; ID_BYTES] {
        &self.reservation_id
    }
}

/// Validated ordered seed tuple for the sole future OwnerSettlement V3 PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementSeedTupleV3 {
    epoch: [u8; ID_BYTES],
    settlement_candidate: [u8; ID_BYTES],
    owner: [u8; ID_BYTES],
}

impl OwnerSettlementSeedTupleV3 {
    /// Construct the canonical owner-row tuple.
    pub fn new(
        epoch: Id32,
        settlement_candidate: Id32,
        owner: Id32,
    ) -> Result<Self, CodecError> {
        if epoch.is_zero() || settlement_candidate.is_zero() || owner.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if epoch == settlement_candidate || epoch == owner || settlement_candidate == owner {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(Self {
            epoch: epoch.bytes(),
            settlement_candidate: settlement_candidate.bytes(),
            owner: owner.bytes(),
        })
    }

    /// First seed: fresh non-aliasing V3 owner-row domain.
    pub const fn domain(&self) -> &'static [u8] {
        OWNER_SETTLEMENT_SEED_DOMAIN_V3
    }

    /// Second seed: full authenticated Epoch PDA bytes.
    pub const fn epoch(&self) -> &[u8; ID_BYTES] {
        &self.epoch
    }

    /// Third seed: final selected SettlementCandidate identity.
    pub const fn settlement_candidate(&self) -> &[u8; ID_BYTES] {
        &self.settlement_candidate
    }

    /// Fourth seed: semantic Position owner.
    pub const fn owner(&self) -> &[u8; ID_BYTES] {
        &self.owner
    }
}

/// Validated ordered seed tuple for the delivery-complete OwnerSettlement V4 PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementSeedTupleV4 {
    epoch: [u8; ID_BYTES],
    settlement_candidate: [u8; ID_BYTES],
    owner: [u8; ID_BYTES],
}

impl OwnerSettlementSeedTupleV4 {
    /// Construct the canonical owner-row tuple.
    pub fn new(
        epoch: Id32,
        settlement_candidate: Id32,
        owner: Id32,
    ) -> Result<Self, CodecError> {
        if epoch.is_zero() || settlement_candidate.is_zero() || owner.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if epoch == settlement_candidate || epoch == owner || settlement_candidate == owner {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(Self {
            epoch: epoch.bytes(),
            settlement_candidate: settlement_candidate.bytes(),
            owner: owner.bytes(),
        })
    }

    /// First seed: fresh non-aliasing V4 owner-row domain.
    pub const fn domain(&self) -> &'static [u8] {
        OWNER_SETTLEMENT_SEED_DOMAIN_V4
    }

    /// Second seed: full authenticated Epoch PDA bytes.
    pub const fn epoch(&self) -> &[u8; ID_BYTES] {
        &self.epoch
    }

    /// Third seed: final selected SettlementCandidate identity.
    pub const fn settlement_candidate(&self) -> &[u8; ID_BYTES] {
        &self.settlement_candidate
    }

    /// Fourth seed: semantic Position owner.
    pub const fn owner(&self) -> &[u8; ID_BYTES] {
        &self.owner
    }
}

/// Validated ordered seed tuple for the rent-owned OwnerSettlement V5 PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementSeedTupleV5 {
    epoch: [u8; ID_BYTES],
    settlement_candidate: [u8; ID_BYTES],
    owner: [u8; ID_BYTES],
}

impl OwnerSettlementSeedTupleV5 {
    /// Construct the canonical rent-owned owner-row tuple.
    pub fn new(epoch: Id32, settlement_candidate: Id32, owner: Id32) -> Result<Self, CodecError> {
        if epoch.is_zero() || settlement_candidate.is_zero() || owner.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if epoch == settlement_candidate || epoch == owner || settlement_candidate == owner {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(Self {
            epoch: epoch.bytes(),
            settlement_candidate: settlement_candidate.bytes(),
            owner: owner.bytes(),
        })
    }

    /// First seed: fresh non-aliasing V5 owner-row domain.
    pub const fn domain(&self) -> &'static [u8] {
        OWNER_SETTLEMENT_SEED_DOMAIN_V5
    }

    /// Second seed: full authenticated Epoch PDA bytes.
    pub const fn epoch(&self) -> &[u8; ID_BYTES] {
        &self.epoch
    }

    /// Third seed: final settlement-candidate identity.
    pub const fn settlement_candidate(&self) -> &[u8; ID_BYTES] {
        &self.settlement_candidate
    }

    /// Fourth seed: semantic Position owner.
    pub const fn owner(&self) -> &[u8; ID_BYTES] {
        &self.owner
    }
}

/// Validated ordered seed tuple for the genesis-assisted MarketRuntime PDA.
///
/// The runtime is anchored only to the immutable MarketBinding PDA. The full
/// MarketInstanceV2 identity is authenticated in both account bodies rather
/// than truncated into a seed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketRuntimeSeedTupleV1 {
    market_binding: [u8; ID_BYTES],
}

impl MarketRuntimeSeedTupleV1 {
    /// Construct the canonical tuple from an authenticated MarketBinding PDA.
    pub fn new(market_binding: Id32) -> Result<Self, CodecError> {
        if market_binding.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        Ok(Self {
            market_binding: market_binding.bytes(),
        })
    }

    /// First seed: the fresh General V2 Market-runtime domain.
    pub const fn domain(&self) -> &'static [u8] {
        MARKET_RUNTIME_SEED_DOMAIN_V1
    }

    /// Second seed: full authenticated MarketBinding PDA bytes.
    pub const fn market_binding(&self) -> &[u8; ID_BYTES] {
        &self.market_binding
    }
}

/// Validated ordered seed tuple for a General V2 Epoch PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochSeedTupleV1 {
    market_binding: [u8; ID_BYTES],
    epoch_index_le: [u8; 8],
}

impl EpochSeedTupleV1 {
    /// Construct the canonical tuple from the immutable binding and exact
    /// runtime-owned next Epoch index.
    pub fn new(market_binding: Id32, epoch_index: u64) -> Result<Self, CodecError> {
        if market_binding.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        Ok(Self {
            market_binding: market_binding.bytes(),
            epoch_index_le: epoch_index.to_le_bytes(),
        })
    }

    /// First seed: the fresh counted General V2 Epoch domain.
    pub const fn domain(&self) -> &'static [u8] {
        EPOCH_SEED_DOMAIN_V1
    }

    /// Second seed: full authenticated MarketBinding PDA bytes.
    pub const fn market_binding(&self) -> &[u8; ID_BYTES] {
        &self.market_binding
    }

    /// Third seed: exact runtime-owned Epoch index in little-endian order.
    pub const fn epoch_index_le(&self) -> &[u8; 8] {
        &self.epoch_index_le
    }
}

/// Validated ordered seed tuple for one owner-settlement envelope PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementSeedTupleV1 {
    epoch: [u8; ID_BYTES],
    settlement_candidate: [u8; ID_BYTES],
    owner: [u8; ID_BYTES],
}

/// Validated ordered seed tuple for the withdrawn V2 owner-settlement row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementSeedTupleV2 {
    epoch: [u8; ID_BYTES],
    settlement_candidate: [u8; ID_BYTES],
    owner: [u8; ID_BYTES],
}

/// Validated ordered seed tuple for the canonical V3 owner-settlement row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementSeedTupleV3 {
    epoch: [u8; ID_BYTES],
    settlement_candidate: [u8; ID_BYTES],
    owner: [u8; ID_BYTES],
}

/// Validated ordered seed tuple for the one-to-one General V2 FinalPot PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalPotSeedTupleV1 {
    epoch: [u8; ID_BYTES],
    settlement_candidate: [u8; ID_BYTES],
}

/// Validated ordered seed tuple for one General SettlementReceipt V3 PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptSeedTupleV3 {
    epoch: [u8; ID_BYTES],
    settlement_candidate: [u8; ID_BYTES],
    slice_index_le: [u8; 2],
}

/// Validated ordered seed tuple for one canonical General SettlementReceipt
/// V4 PDA. Its coordinates match V3 structurally but its domain is disjoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptSeedTupleV4 {
    epoch: [u8; ID_BYTES],
    settlement_candidate: [u8; ID_BYTES],
    slice_index_le: [u8; 2],
}

/// Validated ordered seed tuple for one rent-owned SettlementReceipt V5 PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementReceiptSeedTupleV5 {
    epoch: [u8; ID_BYTES],
    settlement_candidate: [u8; ID_BYTES],
    slice_index_le: [u8; 2],
}

impl SettlementReceiptSeedTupleV3 {
    /// Construct the canonical tuple from authenticated selection facts.
    pub fn new(
        epoch: Id32,
        settlement_candidate: Id32,
        slice_index: u16,
    ) -> Result<Self, CodecError> {
        if epoch.is_zero() || settlement_candidate.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if epoch == settlement_candidate {
            return Err(CodecError::MismatchedBinding);
        }
        if slice_index >= MAX_SLICES_U16 {
            return Err(CodecError::InvalidCount);
        }
        Ok(Self {
            epoch: epoch.bytes(),
            settlement_candidate: settlement_candidate.bytes(),
            slice_index_le: slice_index.to_le_bytes(),
        })
    }

    /// First seed: the non-aliasing V3 receipt domain.
    pub const fn domain(&self) -> &'static [u8] {
        RECEIPT_SEED_DOMAIN_V3
    }

    /// Second seed: full authenticated counted Epoch PDA bytes.
    pub const fn epoch(&self) -> &[u8; ID_BYTES] {
        &self.epoch
    }

    /// Third seed: stable final SettlementCandidate identity.
    pub const fn settlement_candidate(&self) -> &[u8; ID_BYTES] {
        &self.settlement_candidate
    }

    /// Fourth seed: exact selected slice index in little-endian order.
    pub const fn slice_index_le(&self) -> &[u8; 2] {
        &self.slice_index_le
    }
}

impl SettlementReceiptSeedTupleV4 {
    /// Construct the canonical tuple from authenticated selection facts.
    pub fn new(
        epoch: Id32,
        settlement_candidate: Id32,
        slice_index: u16,
    ) -> Result<Self, CodecError> {
        if epoch.is_zero() || settlement_candidate.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if epoch == settlement_candidate {
            return Err(CodecError::MismatchedBinding);
        }
        if slice_index >= MAX_SLICES_U16 {
            return Err(CodecError::InvalidCount);
        }
        Ok(Self {
            epoch: epoch.bytes(),
            settlement_candidate: settlement_candidate.bytes(),
            slice_index_le: slice_index.to_le_bytes(),
        })
    }

    /// First seed: the non-aliasing V4 receipt domain.
    pub const fn domain(&self) -> &'static [u8] {
        RECEIPT_SEED_DOMAIN_V4
    }

    /// Second seed: full authenticated counted Epoch PDA bytes.
    pub const fn epoch(&self) -> &[u8; ID_BYTES] {
        &self.epoch
    }

    /// Third seed: stable final SettlementCandidate identity.
    pub const fn settlement_candidate(&self) -> &[u8; ID_BYTES] {
        &self.settlement_candidate
    }

    /// Fourth seed: exact selected slice index in little-endian order.
    pub const fn slice_index_le(&self) -> &[u8; 2] {
        &self.slice_index_le
    }
}

impl SettlementReceiptSeedTupleV5 {
    /// Construct the canonical tuple from authenticated settlement-root facts.
    pub fn new(
        epoch: Id32,
        settlement_candidate: Id32,
        slice_index: u16,
    ) -> Result<Self, CodecError> {
        if epoch.is_zero() || settlement_candidate.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if epoch == settlement_candidate {
            return Err(CodecError::MismatchedBinding);
        }
        if slice_index >= MAX_SLICES_U16 {
            return Err(CodecError::InvalidCount);
        }
        Ok(Self {
            epoch: epoch.bytes(),
            settlement_candidate: settlement_candidate.bytes(),
            slice_index_le: slice_index.to_le_bytes(),
        })
    }

    /// First seed: fresh non-aliasing V5 receipt domain.
    pub const fn domain(&self) -> &'static [u8] {
        RECEIPT_SEED_DOMAIN_V5
    }

    /// Second seed: full authenticated counted Epoch PDA bytes.
    pub const fn epoch(&self) -> &[u8; ID_BYTES] {
        &self.epoch
    }

    /// Third seed: stable final SettlementCandidate identity.
    pub const fn settlement_candidate(&self) -> &[u8; ID_BYTES] {
        &self.settlement_candidate
    }

    /// Fourth seed: exact selected slice index in little-endian order.
    pub const fn slice_index_le(&self) -> &[u8; 2] {
        &self.slice_index_le
    }
}

impl FinalPotSeedTupleV1 {
    /// Construct the canonical tuple from the counted Epoch and stable final
    /// settlement-candidate identity stored by SelectedCandidate.
    pub fn new(epoch: Id32, settlement_candidate: Id32) -> Result<Self, CodecError> {
        if epoch.is_zero() || settlement_candidate.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if epoch == settlement_candidate {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(Self {
            epoch: epoch.bytes(),
            settlement_candidate: settlement_candidate.bytes(),
        })
    }

    /// First seed: the fresh General V2 FinalPot domain.
    pub const fn domain(&self) -> &'static [u8] {
        FINAL_POT_SEED_DOMAIN_V1
    }

    /// Second seed: full authenticated Epoch PDA bytes.
    pub const fn epoch(&self) -> &[u8; ID_BYTES] {
        &self.epoch
    }

    /// Third seed: stable final RelationV2 settlement-candidate identity.
    pub const fn settlement_candidate(&self) -> &[u8; ID_BYTES] {
        &self.settlement_candidate
    }
}

impl OwnerSettlementSeedTupleV1 {
    /// Construct the canonical tuple from Epoch, final candidate, and owner.
    pub fn new(epoch: Id32, settlement_candidate: Id32, owner: Id32) -> Result<Self, CodecError> {
        if epoch.is_zero() || settlement_candidate.is_zero() || owner.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if epoch == settlement_candidate || epoch == owner || settlement_candidate == owner {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(Self {
            epoch: epoch.bytes(),
            settlement_candidate: settlement_candidate.bytes(),
            owner: owner.bytes(),
        })
    }

    /// First seed: the fresh owner-settlement domain.
    pub const fn domain(&self) -> &'static [u8] {
        OWNER_SETTLEMENT_SEED_DOMAIN_V1
    }

    /// Second seed: full authenticated parent Epoch PDA bytes.
    pub const fn epoch(&self) -> &[u8; ID_BYTES] {
        &self.epoch
    }

    /// Third seed: final selected RelationV2 settlement candidate identity.
    pub const fn settlement_candidate(&self) -> &[u8; ID_BYTES] {
        &self.settlement_candidate
    }

    /// Fourth seed: full semantic Position-owner identity bytes.
    pub const fn owner(&self) -> &[u8; ID_BYTES] {
        &self.owner
    }
}

impl OwnerSettlementSeedTupleV2 {
    /// Construct the one-to-one tuple from Epoch, final candidate, and owner.
    pub fn new(epoch: Id32, settlement_candidate: Id32, owner: Id32) -> Result<Self, CodecError> {
        if epoch.is_zero() || settlement_candidate.is_zero() || owner.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if epoch == settlement_candidate || epoch == owner || settlement_candidate == owner {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(Self {
            epoch: epoch.bytes(),
            settlement_candidate: settlement_candidate.bytes(),
            owner: owner.bytes(),
        })
    }

    /// First seed: the non-aliasing presence-explicit V2 domain.
    pub const fn domain(&self) -> &'static [u8] {
        OWNER_SETTLEMENT_SEED_DOMAIN_V2
    }

    /// Second seed: full authenticated parent Epoch PDA bytes.
    pub const fn epoch(&self) -> &[u8; ID_BYTES] {
        &self.epoch
    }

    /// Third seed: stable final RelationV2 settlement candidate identity.
    pub const fn settlement_candidate(&self) -> &[u8; ID_BYTES] {
        &self.settlement_candidate
    }

    /// Fourth seed: full semantic Position-owner identity bytes.
    pub const fn owner(&self) -> &[u8; ID_BYTES] {
        &self.owner
    }
}

impl OwnerSettlementSeedTupleV3 {
    /// Construct the one-to-one tuple from Epoch, final candidate, and owner.
    pub fn new(epoch: Id32, settlement_candidate: Id32, owner: Id32) -> Result<Self, CodecError> {
        if epoch.is_zero() || settlement_candidate.is_zero() || owner.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if epoch == settlement_candidate || epoch == owner || settlement_candidate == owner {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(Self {
            epoch: epoch.bytes(),
            settlement_candidate: settlement_candidate.bytes(),
            owner: owner.bytes(),
        })
    }

    /// First seed: the canonical non-aliasing V3 domain.
    pub const fn domain(&self) -> &'static [u8] {
        OWNER_SETTLEMENT_SEED_DOMAIN_V3
    }

    /// Second seed: full authenticated parent Epoch PDA bytes.
    pub const fn epoch(&self) -> &[u8; ID_BYTES] {
        &self.epoch
    }

    /// Third seed: stable final RelationV2 settlement candidate identity.
    pub const fn settlement_candidate(&self) -> &[u8; ID_BYTES] {
        &self.settlement_candidate
    }

    /// Fourth seed: full semantic Position-owner identity bytes.
    pub const fn owner(&self) -> &[u8; ID_BYTES] {
        &self.owner
    }
}

/// Existing semantic account tag, fresh successor version: Window.
pub const WINDOW_ACCOUNT_TAG: u8 = 24;
/// Codec version matching the disabled central Window reservation.
pub const WINDOW_ACCOUNT_VERSION: u8 = 4;
/// Full 96-byte cost-aware Window successor version.
pub const WINDOW_ACCOUNT_VERSION_V2: u8 = 5;
/// Existing Market semantic tag, fresh General V2 runtime version.
pub const MARKET_RUNTIME_ACCOUNT_TAG: u8 = 3;
/// First RelationV2-native General Market-runtime schema.
pub const MARKET_RUNTIME_ACCOUNT_VERSION: u8 = 3;
/// Existing Epoch semantic tag, fresh counted General V2 version.
pub const GENERAL_EPOCH_ACCOUNT_TAG: u8 = 11;
/// First RelationV2-native counted General Epoch schema.
pub const GENERAL_EPOCH_ACCOUNT_VERSION: u8 = 6;
/// Existing Reservation semantic tag, fresh rent-owned General version.
pub const GENERAL_RESERVATION_ACCOUNT_TAG_V9: u8 = 0x13;
/// Sole future rent-owned General Reservation version.
pub const GENERAL_RESERVATION_ACCOUNT_VERSION_V9: u8 = 9;
/// Exact rent-owned General Reservation bytes.
pub const GENERAL_RESERVATION_ACCOUNT_BYTES_V9: usize = 666;
/// Fresh disabled General V2 owner-settlement envelope tag.
pub const OWNER_SETTLEMENT_ACCOUNT_TAG: u8 = 0x81;
/// Withdrawn non-aliasing first owner-settlement envelope version.
pub const OWNER_SETTLEMENT_ACCOUNT_VERSION_V1: u8 = 1;
/// Withdrawn presence-explicit owner-settlement envelope version.
pub const OWNER_SETTLEMENT_ACCOUNT_VERSION_V2: u8 = 2;
/// Canonical Reservation-handoff owner-settlement envelope version.
pub const OWNER_SETTLEMENT_ACCOUNT_VERSION_V3: u8 = 3;
/// Delivery-complete owner-settlement envelope version.
pub const OWNER_SETTLEMENT_ACCOUNT_VERSION_V4: u8 = 4;
/// Sole future rent-owned owner-settlement envelope version.
pub const OWNER_SETTLEMENT_ACCOUNT_VERSION_V5: u8 = 5;
/// Historical compatibility alias for the withdrawn V4 envelope.
pub const OWNER_SETTLEMENT_ACCOUNT_VERSION: u8 = OWNER_SETTLEMENT_ACCOUNT_VERSION_V4;
/// Exact historical outer owner-settlement account bytes.
pub const OWNER_SETTLEMENT_ACCOUNT_BYTES: usize = 292;
/// Exact rent-owned V5 outer owner-settlement account bytes.
pub const OWNER_SETTLEMENT_ACCOUNT_BYTES_V5: usize = 340;
/// Fresh disabled selected composite-fee record envelope tag.
pub const SELECTED_FEE_RECORD_ACCOUNT_TAG: u8 = 0x82;
/// First selected composite-fee record envelope version.
pub const SELECTED_FEE_RECORD_ACCOUNT_VERSION: u8 = 1;
/// Exact selected composite-fee record outer bytes.
pub const SELECTED_FEE_RECORD_ACCOUNT_BYTES: usize = 340;
/// Current rent-owned RevenuePolicyV2 selected-fee record version.
pub const SELECTED_FEE_RECORD_ACCOUNT_VERSION_V2: u8 = 2;
/// Exact rent-owned RevenuePolicyV2 selected-fee record outer bytes.
pub const SELECTED_FEE_RECORD_ACCOUNT_BYTES_V2: usize = 388;
/// Fresh disabled owner fee-carry envelope tag.
pub const OWNER_FEE_CARRY_ACCOUNT_TAG: u8 = 0x83;
/// First owner fee-carry envelope version.
pub const OWNER_FEE_CARRY_ACCOUNT_VERSION: u8 = 1;
/// Exact owner fee-carry outer bytes.
pub const OWNER_FEE_CARRY_ACCOUNT_BYTES: usize = 132;
/// In-place terminal successor version at the same owner fee-carry PDA.
pub const OWNER_FEE_FINALIZATION_ACCOUNT_VERSION: u8 = 2;
/// Exact terminal fee-finalization outer bytes.
pub const OWNER_FEE_FINALIZATION_ACCOUNT_BYTES: usize = 500;
/// Sole future rent-owned live carry version at the unchanged owner fee-carry PDA.
pub const OWNER_FEE_CARRY_ACCOUNT_VERSION_V3: u8 = 3;
/// Exact rent-owned live carry outer bytes.
pub const OWNER_FEE_CARRY_ACCOUNT_BYTES_V3: usize = 180;
/// Sole future rent-owned terminal successor at the unchanged carry PDA.
pub const OWNER_FEE_FINALIZATION_ACCOUNT_VERSION_V4: u8 = 4;
/// Exact rent-owned terminal fee-finalization outer bytes.
pub const OWNER_FEE_FINALIZATION_ACCOUNT_BYTES_V4: usize = 548;
/// Fresh disabled owner payer-allocation envelope tag.
pub const PAYER_ALLOCATION_ACCOUNT_TAG: u8 = 0x84;
/// First owner payer-allocation envelope version.
pub const PAYER_ALLOCATION_ACCOUNT_VERSION: u8 = 1;
/// Exact owner payer-allocation outer bytes.
pub const PAYER_ALLOCATION_ACCOUNT_BYTES: usize = 2_684;
/// Sole future rent-owned payer-allocation envelope version.
pub const PAYER_ALLOCATION_ACCOUNT_VERSION_V2: u8 = 2;
/// Exact rent-owned payer-allocation outer bytes.
pub const PAYER_ALLOCATION_ACCOUNT_BYTES_V2: usize = 2_732;
/// Candidate-wide recipient-allocation envelope tag.
pub const RECIPIENT_ALLOCATION_ACCOUNT_TAG: u8 = 0x85;
/// First candidate-wide recipient-allocation envelope version.
pub const RECIPIENT_ALLOCATION_ACCOUNT_VERSION: u8 = 1;
/// Exact candidate-wide recipient-allocation outer bytes.
pub const RECIPIENT_ALLOCATION_ACCOUNT_BYTES: usize = 2_644;
/// Historical complete-book-certified recipient version; decode-only.
pub const RECIPIENT_ALLOCATION_ACCOUNT_VERSION_V2: u8 = 2;
/// Exact rent-owned certified recipient-allocation outer bytes.
pub const RECIPIENT_ALLOCATION_ACCOUNT_BYTES_V2: usize = 2_764;
/// Current rent-owned V2 weight-stream-certified recipient version.
pub const RECIPIENT_ALLOCATION_ACCOUNT_VERSION_V3: u8 = 3;
/// Exact 52-byte-outer plus 2,744-byte semantic account width.
pub const RECIPIENT_ALLOCATION_ACCOUNT_BYTES_V3: usize = 2_796;
/// Fresh disabled selected-record treasury-ledger envelope tag.
pub const TREASURY_LEDGER_ACCOUNT_TAG: u8 = 0x86;
/// First selected-record treasury-ledger envelope version.
pub const TREASURY_LEDGER_ACCOUNT_VERSION: u8 = 1;
/// Exact selected-record treasury-ledger outer bytes.
pub const TREASURY_LEDGER_ACCOUNT_BYTES: usize = 148;
/// Current rent-owned RevenuePolicyV2 treasury-ledger version.
pub const TREASURY_LEDGER_ACCOUNT_VERSION_V2: u8 = 2;
/// Exact rent-owned RevenuePolicyV2 treasury-ledger outer bytes.
pub const TREASURY_LEDGER_ACCOUNT_BYTES_V2: usize = 196;
/// Compact fee retirement/terminal account family tag.
pub const FEE_RETIREMENT_ACCOUNT_TAG: u8 = 0xb9;
/// Durable candidate-wide closure-manifest version.
pub const FEE_RETIREMENT_CLOSURE_MANIFEST_ACCOUNT_VERSION: u8 = 2;
/// Durable candidate-wide fee-terminal version.
pub const FEE_RETIREMENT_TERMINAL_ACCOUNT_VERSION: u8 = 3;
/// Exact rent-owned closure-manifest width.
pub const FEE_RETIREMENT_ACCOUNT_BYTES_V2: usize = 580;
/// Exact rent-owned terminal-receipt width.
pub const FEE_RETIREMENT_ACCOUNT_BYTES_V3: usize = 596;
/// Fresh disabled buyer-first settlement cash-pot envelope tag.
pub const SETTLEMENT_CASH_POT_ACCOUNT_TAG: u8 = 0x87;
/// First buyer-first settlement cash-pot envelope version.
pub const SETTLEMENT_CASH_POT_ACCOUNT_VERSION: u8 = 1;
/// Exact buyer-first settlement cash-pot outer bytes.
pub const SETTLEMENT_CASH_POT_ACCOUNT_BYTES: usize = 260;
/// Fresh disabled General V2 FinalPot envelope tag.
pub const FINAL_POT_ACCOUNT_TAG: u8 = 0x89;
/// First combined FinalPot and virtual-inventory-budget envelope version.
pub const FINAL_POT_ACCOUNT_VERSION: u8 = 1;
/// Exact combined FinalPot outer bytes.
pub const FINAL_POT_ACCOUNT_BYTES: usize = 332;
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
/// Resumable RelationV2 ClearWork successor version.
pub const CLEAR_WORK_ACCOUNT_VERSION_V3: u8 = 3;
/// Codec tag matching the disabled central admission-node reservation.
pub const ADMISSION_NODE_ACCOUNT_TAG: u8 = 0x77;
/// First funded admission-node account version.
pub const ADMISSION_NODE_ACCOUNT_VERSION: u8 = 1;
/// Cost-certificate-bearing AdmissionNode successor version.
pub const ADMISSION_NODE_ACCOUNT_VERSION_V2: u8 = 2;
/// Exact cost-certificate-bearing AdmissionNode bytes.
pub const ADMISSION_NODE_ACCOUNT_BYTES_V2: usize = 775;
/// Codec tag matching the disabled central epoch-budget reservation.
pub const EPOCH_BUDGET_ACCOUNT_TAG: u8 = 0x78;
/// First epoch-budget account version.
pub const EPOCH_BUDGET_ACCOUNT_VERSION: u8 = 1;
/// Codec tag matching the disabled central Market-binding reservation.
pub const MARKET_BINDING_ACCOUNT_TAG: u8 = 0x79;
/// First immutable Market-binding account version.
pub const MARKET_BINDING_ACCOUNT_VERSION: u8 = 1;
/// Owner-net candidate-cost Market-binding successor version.
pub const MARKET_BINDING_ACCOUNT_VERSION_V2: u8 = 2;
/// Exact candidate-cost Market-binding successor bytes.
pub const MARKET_BINDING_ACCOUNT_BYTES_V2: usize = 572;
/// Historical BundleV5/AttachmentV4 Product-authorized Market-binding version.
pub const MARKET_BINDING_ACCOUNT_VERSION_V3: u8 = 3;
/// Exact historical Product-family-authorized, rent-owned Market-binding bytes.
pub const MARKET_BINDING_ACCOUNT_BYTES_V3: usize = 952;
/// Historical RootV2/LinkV2 Product/Revenue-authorized Market-binding version.
pub const MARKET_BINDING_ACCOUNT_VERSION_V4: u8 = 4;
/// Exact historical Product/Revenue-authorized, rent-owned Market-binding bytes.
pub const MARKET_BINDING_ACCOUNT_BYTES_V4: usize = 1_304;
/// Current RootV3/LinkV3/FundingV5 Product/Revenue-authorized version.
pub const MARKET_BINDING_ACCOUNT_VERSION_V5: u8 = 5;
/// Exact current Product/Revenue-authorized, rent-owned Market-binding bytes.
pub const MARKET_BINDING_ACCOUNT_BYTES_V5: usize = 1_368;
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
/// this standalone pure crate does not claim registry authority.
pub const ACCOUNT_ALLOCATIONS_V1: [AccountAllocationV1; 40] = [
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
        tag: GENERAL_RESERVATION_ACCOUNT_TAG_V9,
        version: GENERAL_RESERVATION_ACCOUNT_VERSION_V9,
        owner: "clutch-solana-layout/ReservationAccountV9",
    },
    AccountAllocationV1 {
        tag: OWNER_SETTLEMENT_ACCOUNT_TAG,
        version: OWNER_SETTLEMENT_ACCOUNT_VERSION_V1,
        owner: "clutch-general-v2-contract/OwnerSettlementV1AccountV1",
    },
    AccountAllocationV1 {
        tag: OWNER_SETTLEMENT_ACCOUNT_TAG,
        version: OWNER_SETTLEMENT_ACCOUNT_VERSION_V2,
        owner: "clutch-general-v2-contract/OwnerSettlementV2AccountV1",
    },
    AccountAllocationV1 {
        tag: OWNER_SETTLEMENT_ACCOUNT_TAG,
        version: OWNER_SETTLEMENT_ACCOUNT_VERSION_V3,
        owner: "clutch-general-v2-contract/OwnerSettlementV3AccountV1",
    },
    AccountAllocationV1 {
        tag: OWNER_SETTLEMENT_ACCOUNT_TAG,
        version: OWNER_SETTLEMENT_ACCOUNT_VERSION_V4,
        owner: "clutch-general-v2-contract/OwnerSettlementV4AccountV1",
    },
    AccountAllocationV1 {
        tag: OWNER_SETTLEMENT_ACCOUNT_TAG,
        version: OWNER_SETTLEMENT_ACCOUNT_VERSION_V5,
        owner: "clutch-general-v2-contract/OwnerSettlementV5AccountV1",
    },
    AccountAllocationV1 {
        tag: SELECTED_FEE_RECORD_ACCOUNT_TAG,
        version: SELECTED_FEE_RECORD_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/SelectedFeeRecordV1AccountV1",
    },
    AccountAllocationV1 {
        tag: SELECTED_FEE_RECORD_ACCOUNT_TAG,
        version: SELECTED_FEE_RECORD_ACCOUNT_VERSION_V2,
        owner: "clutch-general-v2-contract/SelectedFeeRecordV2AccountV1",
    },
    AccountAllocationV1 {
        tag: OWNER_FEE_CARRY_ACCOUNT_TAG,
        version: OWNER_FEE_CARRY_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/OwnerFeeCarryV1AccountV1",
    },
    AccountAllocationV1 {
        tag: OWNER_FEE_CARRY_ACCOUNT_TAG,
        version: OWNER_FEE_FINALIZATION_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/OwnerFeeFinalizationV2AccountV1",
    },
    AccountAllocationV1 {
        tag: OWNER_FEE_CARRY_ACCOUNT_TAG,
        version: OWNER_FEE_CARRY_ACCOUNT_VERSION_V3,
        owner: "clutch-general-v2-contract/OwnerFeeCarryV3AccountV1",
    },
    AccountAllocationV1 {
        tag: OWNER_FEE_CARRY_ACCOUNT_TAG,
        version: OWNER_FEE_FINALIZATION_ACCOUNT_VERSION_V4,
        owner: "clutch-general-v2-contract/OwnerFeeFinalizationV4AccountV1",
    },
    AccountAllocationV1 {
        tag: PAYER_ALLOCATION_ACCOUNT_TAG,
        version: PAYER_ALLOCATION_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/PayerAllocationV1AccountV1",
    },
    AccountAllocationV1 {
        tag: PAYER_ALLOCATION_ACCOUNT_TAG,
        version: PAYER_ALLOCATION_ACCOUNT_VERSION_V2,
        owner: "clutch-general-v2-contract/PayerAllocationV2AccountV1",
    },
    AccountAllocationV1 {
        tag: RECIPIENT_ALLOCATION_ACCOUNT_TAG,
        version: RECIPIENT_ALLOCATION_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/RecipientAllocationV1AccountV1",
    },
    AccountAllocationV1 {
        tag: RECIPIENT_ALLOCATION_ACCOUNT_TAG,
        version: RECIPIENT_ALLOCATION_ACCOUNT_VERSION_V2,
        owner: "clutch-general-v2-contract/RecipientAllocationV2AccountV1",
    },
    AccountAllocationV1 {
        tag: RECIPIENT_ALLOCATION_ACCOUNT_TAG,
        version: RECIPIENT_ALLOCATION_ACCOUNT_VERSION_V3,
        owner: "clutch-fee-runtime-contract/CertifiedRecipientAllocationV3",
    },
    AccountAllocationV1 {
        tag: TREASURY_LEDGER_ACCOUNT_TAG,
        version: TREASURY_LEDGER_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/TreasuryLedgerV1AccountV1",
    },
    AccountAllocationV1 {
        tag: TREASURY_LEDGER_ACCOUNT_TAG,
        version: TREASURY_LEDGER_ACCOUNT_VERSION_V2,
        owner: "clutch-general-v2-contract/TreasuryLedgerV2AccountV1",
    },
    AccountAllocationV1 {
        tag: SETTLEMENT_CASH_POT_ACCOUNT_TAG,
        version: SETTLEMENT_CASH_POT_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/SettlementCashPotV1AccountV1",
    },
    AccountAllocationV1 {
        tag: FINAL_POT_ACCOUNT_TAG,
        version: FINAL_POT_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/FinalPotV1AccountV1",
    },
    AccountAllocationV1 {
        tag: WINDOW_ACCOUNT_TAG,
        version: WINDOW_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/WindowV4",
    },
    AccountAllocationV1 {
        tag: WINDOW_ACCOUNT_TAG,
        version: WINDOW_ACCOUNT_VERSION_V2,
        owner: "clutch-general-v2-contract/CandidateWindowV5AccountV1",
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
        tag: CLEAR_WORK_ACCOUNT_TAG,
        version: CLEAR_WORK_ACCOUNT_VERSION_V3,
        owner: "clutch-general-v2-contract/ClearWorkV3AccountV1",
    },
    AccountAllocationV1 {
        tag: ADMISSION_NODE_ACCOUNT_TAG,
        version: ADMISSION_NODE_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/AdmissionNodeV3AccountV1",
    },
    AccountAllocationV1 {
        tag: ADMISSION_NODE_ACCOUNT_TAG,
        version: ADMISSION_NODE_ACCOUNT_VERSION_V2,
        owner: "clutch-general-v2-contract/AdmissionNodeV4AccountV1",
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
        tag: MARKET_BINDING_ACCOUNT_TAG,
        version: MARKET_BINDING_ACCOUNT_VERSION_V2,
        owner: "clutch-general-v2-contract/MarketBindingV2",
    },
    AccountAllocationV1 {
        tag: MARKET_BINDING_ACCOUNT_TAG,
        version: MARKET_BINDING_ACCOUNT_VERSION_V3,
        owner: "clutch-general-v2-contract/MarketBindingV3",
    },
    AccountAllocationV1 {
        tag: MARKET_BINDING_ACCOUNT_TAG,
        version: MARKET_BINDING_ACCOUNT_VERSION_V4,
        owner: "clutch-general-v2-contract/MarketBindingV4",
    },
    AccountAllocationV1 {
        tag: MARKET_BINDING_ACCOUNT_TAG,
        version: MARKET_BINDING_ACCOUNT_VERSION_V5,
        owner: "clutch-general-v2-contract/MarketBindingV5",
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
    AccountAllocationV1 {
        tag: SETTLEMENT_ROOT_ACCOUNT_TAG,
        version: SETTLEMENT_ROOT_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/SettlementRootV1AccountV1",
    },
    AccountAllocationV1 {
        tag: INDEXED_SETTLEMENT_ROOT_ACCOUNT_TAG,
        version: INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/IndexedSettlementRootV1AccountV1",
    },
    AccountAllocationV1 {
        tag: FEE_RETIREMENT_ACCOUNT_TAG,
        version: FEE_RETIREMENT_CLOSURE_MANIFEST_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/FeeClosureManifestV2AccountV1",
    },
    AccountAllocationV1 {
        tag: FEE_RETIREMENT_ACCOUNT_TAG,
        version: FEE_RETIREMENT_TERMINAL_ACCOUNT_VERSION,
        owner: "clutch-general-v2-contract/FeeRecordTerminalV3AccountV1",
    },
];

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_OUTCOMES_U8 == 16);
const _: () = assert!(MAX_ORDERS == 64);
const _: () = assert!(MAX_ORDERS_U8 == 64);
const _: () = assert!(MAX_QUANTIZED_ATOMS == 16);
const _: () = assert!(MAX_QUANTIZED_ATOMS_U8 == 16);
const _: () = assert!(MAX_SLICES_U16 == 416);
const _: () = assert!(FEE_RETIREMENT_ACCUMULATOR_SEED_DOMAIN_V1.len() <= 32);
const _: () = assert!(FEE_CLOSURE_MANIFEST_SEED_DOMAIN_V1.len() <= 32);
const _: () = assert!(FEE_TERMINAL_RECEIPT_SEED_DOMAIN_V1.len() <= 32);

#[cfg(test)]
mod seed_tests {
    use super::*;

    fn id(byte: u8) -> Id32 {
        Id32::new([byte; ID_BYTES]).unwrap()
    }

    #[test]
    fn runtime_and_epoch_seed_tuples_are_exact_and_ordered() {
        let binding = id(7);
        let runtime = MarketRuntimeSeedTupleV1::new(binding).unwrap();
        assert_eq!(runtime.domain(), b"general-market-runtime:v1");
        assert_eq!(runtime.market_binding(), &[7; ID_BYTES]);

        let epoch = EpochSeedTupleV1::new(binding, 0x0102_0304_0506_0708).unwrap();
        assert_eq!(epoch.domain(), b"general-epoch:v2");
        assert_eq!(epoch.market_binding(), &[7; ID_BYTES]);
        assert_eq!(epoch.epoch_index_le(), &[8, 7, 6, 5, 4, 3, 2, 1]);
        let final_pot = FinalPotSeedTupleV1::new(id(8), id(9)).unwrap();
        assert_eq!(final_pot.domain(), b"general-final-pot:v2");
        assert_eq!(final_pot.epoch(), &[8; ID_BYTES]);
        assert_eq!(final_pot.settlement_candidate(), &[9; ID_BYTES]);
        let receipt = SettlementReceiptSeedTupleV3::new(id(8), id(9), 0x0102).unwrap();
        assert_eq!(receipt.domain(), b"general-receipt:v3");
        assert_eq!(receipt.epoch(), &[8; ID_BYTES]);
        assert_eq!(receipt.settlement_candidate(), &[9; ID_BYTES]);
        assert_eq!(receipt.slice_index_le(), &[2, 1]);
        let receipt_v4 = SettlementReceiptSeedTupleV4::new(id(8), id(9), 0x0102).unwrap();
        assert_eq!(receipt_v4.domain(), b"general-receipt:v4");
        assert_eq!(receipt_v4.epoch(), &[8; ID_BYTES]);
        assert_eq!(receipt_v4.settlement_candidate(), &[9; ID_BYTES]);
        assert_eq!(receipt_v4.slice_index_le(), &[2, 1]);
        assert_ne!(receipt.domain(), receipt_v4.domain());
        let receipt_v5 = SettlementReceiptSeedTupleV5::new(id(8), id(9), 0x0102).unwrap();
        assert_eq!(receipt_v5.domain(), b"general-receipt:v5");
        assert_eq!(receipt_v5.epoch(), &[8; ID_BYTES]);
        assert_eq!(receipt_v5.settlement_candidate(), &[9; ID_BYTES]);
        assert_eq!(receipt_v5.slice_index_le(), &[2, 1]);
        assert_ne!(receipt_v4.domain(), receipt_v5.domain());
        let page = GeneralOrderPageSeedTupleV5::new(id(8), 0x0304).unwrap();
        assert_eq!(page.domain(), b"general-order-page:v2");
        assert_eq!(page.epoch(), &[8; ID_BYTES]);
        assert_eq!(page.page_index_le(), &[4, 3]);
        let reservation = GeneralReservationSeedTupleV3::new(id(10)).unwrap();
        assert_eq!(reservation.domain(), b"general-reservation:v2");
        assert_eq!(reservation.reservation_id(), &[10; ID_BYTES]);
        let reservation_v9 = GeneralReservationSeedTupleV9::new(id(10)).unwrap();
        assert_eq!(reservation_v9.domain(), b"general-reservation:v9");
        assert_eq!(reservation_v9.reservation_id(), &[10; ID_BYTES]);
        assert_ne!(reservation.domain(), reservation_v9.domain());
        let owner_row = OwnerSettlementSeedTupleV3::new(id(8), id(9), id(11)).unwrap();
        assert_eq!(owner_row.domain(), b"owner-settlement:v3");
        assert_eq!(owner_row.epoch(), &[8; ID_BYTES]);
        assert_eq!(owner_row.settlement_candidate(), &[9; ID_BYTES]);
        assert_eq!(owner_row.owner(), &[11; ID_BYTES]);
        let owner_row_v5 = OwnerSettlementSeedTupleV5::new(id(8), id(9), id(11)).unwrap();
        assert_eq!(owner_row_v5.domain(), b"owner-settlement:v5");
        assert_eq!(owner_row_v5.epoch(), &[8; ID_BYTES]);
        assert_eq!(owner_row_v5.settlement_candidate(), &[9; ID_BYTES]);
        assert_eq!(owner_row_v5.owner(), &[11; ID_BYTES]);
        assert_ne!(owner_row.domain(), owner_row_v5.domain());
        assert_ne!(
            epoch,
            EpochSeedTupleV1::new(binding, 0x0102_0304_0506_0709).unwrap()
        );
        assert_eq!(
            MarketRuntimeSeedTupleV1::new(Id32::ZERO),
            Err(CodecError::ZeroIdentity)
        );
        assert_eq!(
            EpochSeedTupleV1::new(Id32::ZERO, 1),
            Err(CodecError::ZeroIdentity)
        );
        assert_eq!(
            SettlementReceiptSeedTupleV3::new(id(8), id(9), MAX_SLICES_U16),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            SettlementReceiptSeedTupleV4::new(id(8), id(9), MAX_SLICES_U16),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            SettlementReceiptSeedTupleV5::new(id(8), id(9), MAX_SLICES_U16),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            GeneralOrderPageSeedTupleV5::new(Id32::ZERO, 0),
            Err(CodecError::ZeroIdentity)
        );
        assert_eq!(
            GeneralReservationSeedTupleV3::new(Id32::ZERO),
            Err(CodecError::ZeroIdentity)
        );
        assert_eq!(
            GeneralReservationSeedTupleV9::new(Id32::ZERO),
            Err(CodecError::ZeroIdentity)
        );
        assert_eq!(
            OwnerSettlementSeedTupleV3::new(id(8), id(8), id(11)),
            Err(CodecError::MismatchedBinding)
        );
    }
}
