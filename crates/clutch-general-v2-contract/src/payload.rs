// SPDX-License-Identifier: AGPL-3.0-or-later

//! Strict, allocation-free payload decoders for the empty-book identity lab.

use crate::{
    AuthenticatedOwnerFragmentV1, CodecError, Id32, Reader, SettlementCandidateKindV1,
    SettlementSideV1, ID_BYTES, MAX_ORDERS_U8, MAX_OUTCOMES_U8, MAX_QUANTIZED_ATOMS_U8,
    MAX_SLICES_U16, QUANTIZED_ATOM_BYTES, SETTLEMENT_SLICE_BYTES,
};

/// Largest General V2 action payload under the frozen 402-byte intent ceiling.
pub const MAX_GENERAL_V2_ACTION_PAYLOAD_BYTES: usize = 399;
/// Exact action-2 payload bytes.
pub const INIT_EPOCH_PAYLOAD_BYTES: usize = 48;
/// Exact action-6 payload bytes.
pub const FREEZE_EPOCH_PAYLOAD_BYTES: usize = 32;
/// Exact action-7 payload bytes.
pub const BEGIN_CANDIDATE_PAYLOAD_BYTES: usize = 64;
/// Exact action-8 open-variant payload bytes.
pub const OPEN_CANDIDATE_FEED_PAYLOAD_BYTES: usize = 336;
/// Exact fixed bytes before records in an action-8 segment variant.
pub const WRITE_CANDIDATE_FEED_FIXED_BYTES: usize = 69;
/// Exact payload bytes shared by actions 9, 10, 14, and 32.
pub const EPOCH_NODE_PAYLOAD_BYTES: usize = 64;
/// Exact action-15 payload bytes.
pub const FINALIZE_SELECTION_PAYLOAD_BYTES: usize = 32;
/// Exact action-20 payload bytes.
pub const CLEANUP_CANDIDATE_PAYLOAD_BYTES: usize = 96;
/// Exact action-21 payload bytes.
pub const CLAIM_SOLVER_PAYLOAD_BYTES: usize = 32;
/// Exact action-24 selector payload bytes.
pub const FREEZE_ENTITLEMENT_PAYLOAD_BYTES: usize = 96;
/// Exact action-25 claimed-fragment payload bytes.
pub const ENTITLE_SLICE_PAYLOAD_BYTES: usize = 149;
/// Exact action-26 disabled direct-receipt selector bytes.
pub const CONSUME_DIRECT_RECEIPT_EGGS_PAYLOAD_BYTES: usize = 96;

/// Action-2 `InitEpoch` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitEpochPayloadV1 {
    /// Full Product successor MarketInstanceV2 identity.
    pub market_instance_v2_id: Id32,
    /// Runtime-owned exact next Epoch index.
    pub epoch_index: u64,
    /// Earliest slot at which the Epoch may freeze.
    pub freeze_deadline_slot: u64,
}

impl InitEpochPayloadV1 {
    /// Decode exactly 48 hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, INIT_EPOCH_PAYLOAD_BYTES)?;
        let value = Self {
            market_instance_v2_id: live_id(&mut r)?,
            epoch_index: r.u64()?,
            freeze_deadline_slot: r.u64()?,
        };
        r.finish()?;
        value.validate()?;
        Ok(value)
    }

    /// Validate the nonzero identity and checked-next-index geometry.
    pub fn validate(self) -> Result<(), CodecError> {
        if self.market_instance_v2_id.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if self.freeze_deadline_slot == 0 || self.epoch_index == u64::MAX {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }
}

/// Action-6 `FreezeEpoch` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreezeEpochPayloadV1 {
    /// Canonical recomputed Epoch-semantics identity.
    pub epoch_semantics_id: Id32,
}

impl FreezeEpochPayloadV1 {
    /// Decode exactly 32 hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, FREEZE_EPOCH_PAYLOAD_BYTES)?;
        let value = Self {
            epoch_semantics_id: live_id(&mut r)?,
        };
        r.finish()?;
        Ok(value)
    }
}

/// Action-7 `BeginCandidate` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeginCandidatePayloadV1 {
    /// Parent Epoch PDA.
    pub epoch: Id32,
    /// Exact hidden candidate commitment.
    pub commitment: Id32,
}

impl BeginCandidatePayloadV1 {
    /// Decode exactly 64 hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, BEGIN_CANDIDATE_PAYLOAD_BYTES)?;
        let value = Self {
            epoch: live_id(&mut r)?,
            commitment: live_id(&mut r)?,
        };
        r.finish()?;
        Ok(value)
    }
}

/// Action-8 variant-zero commitment opening and FeedStage dimensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenCandidateFeedPayloadV1 {
    /// Parent Epoch PDA.
    pub epoch: Id32,
    /// Ordinal-derived AdmissionNode PDA.
    pub node: Id32,
    /// Exact 32-byte commitment secret.
    pub secret: [u8; ID_BYTES],
    /// Claimed bundle identity later recomputed from the complete FeedStage.
    pub candidate_bundle_digest: Id32,
    /// Claimed final settlement-candidate identity.
    pub settlement_candidate_id: Id32,
    /// Claimed RelationV2 economic candidate identity.
    pub base_relation_candidate_id: Id32,
    /// Claimed settlement-witness identity.
    pub settlement_witness_digest: Id32,
    /// Claimed RelationV2 price-semantics identity.
    pub candidate_price_digest: Id32,
    /// Claimed canonical V3 witness-body identity.
    pub price_body_digest: Id32,
    /// Virtual complete-set split.
    pub virtual_split: u64,
    /// Virtual complete-set merge.
    pub virtual_merge: u64,
    /// Exact honored-AON bitset.
    pub honored_aon_mask: u64,
    /// Exact integer price simplex scale.
    pub price_scale: u64,
    /// Positive primitive atom-mass denominator.
    pub common_denominator: u64,
    /// Quantized V3 basis degree, zero through three.
    pub basis_degree: u8,
    /// Active outcome count.
    pub outcome_count: u8,
    /// Active order count.
    pub order_count: u8,
    /// Active atom count.
    pub atom_count: u8,
    /// Active settlement-slice count.
    pub slice_count: u16,
    /// Direct or CoveredDealer route.
    pub candidate_kind: SettlementCandidateKindV1,
}

impl OpenCandidateFeedPayloadV1 {
    /// Decode exactly the 336-byte variant-zero payload.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, OPEN_CANDIDATE_FEED_PAYLOAD_BYTES)?;
        if r.u8()? != 0 {
            return Err(CodecError::InvalidState);
        }
        let value = Self {
            epoch: live_id(&mut r)?,
            node: live_id(&mut r)?,
            secret: r.array()?,
            candidate_bundle_digest: live_id(&mut r)?,
            settlement_candidate_id: live_id(&mut r)?,
            base_relation_candidate_id: live_id(&mut r)?,
            settlement_witness_digest: live_id(&mut r)?,
            candidate_price_digest: live_id(&mut r)?,
            price_body_digest: live_id(&mut r)?,
            virtual_split: r.u64()?,
            virtual_merge: r.u64()?,
            honored_aon_mask: r.u64()?,
            price_scale: r.u64()?,
            common_denominator: r.u64()?,
            basis_degree: r.u8()?,
            outcome_count: r.u8()?,
            order_count: r.u8()?,
            atom_count: r.u8()?,
            slice_count: r.u16()?,
            candidate_kind: SettlementCandidateKindV1::from_byte(r.u8()?)?,
        };
        r.finish()?;
        value.validate()?;
        Ok(value)
    }

    /// Validate dimensions and canonical inactive AON bits.
    pub fn validate(self) -> Result<(), CodecError> {
        for id in [
            self.epoch,
            self.node,
            self.candidate_bundle_digest,
            self.settlement_candidate_id,
            self.base_relation_candidate_id,
            self.settlement_witness_digest,
            self.candidate_price_digest,
            self.price_body_digest,
        ] {
            if id.is_zero() {
                return Err(CodecError::ZeroIdentity);
            }
        }
        if self.price_scale == 0
            || self.common_denominator == 0
            || self.basis_degree > 3
            || !(2..=MAX_OUTCOMES_U8).contains(&self.outcome_count)
            || self.outcome_count <= self.basis_degree
            || self.order_count > MAX_ORDERS_U8
            || self.atom_count == 0
            || self.atom_count > self.outcome_count
            || self.atom_count > MAX_QUANTIZED_ATOMS_U8
            || self.slice_count > MAX_SLICES_U16
            || (self.virtual_split != 0 && self.virtual_merge != 0)
            || (self.order_count < 64 && (self.honored_aon_mask >> self.order_count) != 0)
            || (self.candidate_kind == SettlementCandidateKindV1::Direct
                && self.settlement_candidate_id != self.base_relation_candidate_id)
        {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }
}

/// Exact record family of an action-8 segment write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CandidateFeedWriteKindV1 {
    /// Eight-byte little-endian prices.
    Prices = 1,
    /// Eight-byte little-endian fills.
    Fills = 2,
    /// Twenty-four-byte little-endian coordinate/mass atoms.
    QuantizedAtoms = 3,
    /// Thirteen-byte settlement slices.
    SettlementSlices = 4,
}

impl CandidateFeedWriteKindV1 {
    fn from_byte(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::Prices),
            2 => Ok(Self::Fills),
            3 => Ok(Self::QuantizedAtoms),
            4 => Ok(Self::SettlementSlices),
            _ => Err(CodecError::InvalidState),
        }
    }

    /// Exact one-byte tagged-union variant.
    pub const fn to_byte(self) -> u8 {
        match self {
            Self::Prices => 1,
            Self::Fills => 2,
            Self::QuantizedAtoms => 3,
            Self::SettlementSlices => 4,
        }
    }

    /// Exact bytes in one record of this family.
    pub const fn record_bytes(self) -> usize {
        match self {
            Self::Prices | Self::Fills => 8,
            Self::QuantizedAtoms => QUANTIZED_ATOM_BYTES,
            Self::SettlementSlices => SETTLEMENT_SLICE_BYTES,
        }
    }

    fn maximum_total_records(self) -> u16 {
        match self {
            Self::Prices => u16::from(MAX_OUTCOMES_U8),
            Self::Fills => u16::from(MAX_ORDERS_U8),
            Self::QuantizedAtoms => u16::from(MAX_QUANTIZED_ATOMS_U8),
            Self::SettlementSlices => MAX_SLICES_U16,
        }
    }
}

/// Borrowed action-8 segment payload with exact record geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateFeedSegmentPayloadV1<'a> {
    /// Record family selected by the variant byte.
    pub kind: CandidateFeedWriteKindV1,
    /// Parent Epoch PDA.
    pub epoch: Id32,
    /// Ordinal-derived AdmissionNode PDA.
    pub node: Id32,
    /// First sequential record index in this segment.
    pub cursor: u16,
    /// Positive number of exact records.
    pub count: u16,
    /// Exact borrowed record bytes without padding.
    pub records: &'a [u8],
}

impl<'a> CandidateFeedSegmentPayloadV1<'a> {
    /// Decode one strict variable-length segment payload.
    pub fn decode(input: &'a [u8]) -> Result<Self, CodecError> {
        if input.len() < WRITE_CANDIDATE_FEED_FIXED_BYTES
            || input.len() > MAX_GENERAL_V2_ACTION_PAYLOAD_BYTES
        {
            return Err(CodecError::WrongLength);
        }
        let kind = CandidateFeedWriteKindV1::from_byte(input[0])?;
        let mut prefix = Reader::exact(
            &input[..WRITE_CANDIDATE_FEED_FIXED_BYTES],
            WRITE_CANDIDATE_FEED_FIXED_BYTES,
        )?;
        if prefix.u8()? != kind.to_byte() {
            return Err(CodecError::InvalidState);
        }
        let epoch = live_id(&mut prefix)?;
        let node = live_id(&mut prefix)?;
        let cursor = prefix.u16()?;
        let count = prefix.u16()?;
        prefix.finish()?;
        let value = Self {
            kind,
            epoch,
            node,
            cursor,
            count,
            records: &input[WRITE_CANDIDATE_FEED_FIXED_BYTES..],
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate identities, bounded cursor arithmetic, and exact record bytes.
    pub fn validate(self) -> Result<(), CodecError> {
        if self.epoch.is_zero() || self.node.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if self.count == 0 {
            return Err(CodecError::InvalidCount);
        }
        let end = self
            .cursor
            .checked_add(self.count)
            .ok_or(CodecError::ArithmeticOverflow)?;
        if end > self.kind.maximum_total_records() {
            return Err(CodecError::InvalidCount);
        }
        let exact = usize::from(self.count)
            .checked_mul(self.kind.record_bytes())
            .ok_or(CodecError::ArithmeticOverflow)?;
        if self.records.len() != exact {
            return Err(CodecError::WrongLength);
        }
        Ok(())
    }

    /// Require the persisted sequential cursor and return the checked end.
    pub fn require_cursor(self, expected_cursor: u16) -> Result<u16, CodecError> {
        if self.cursor != expected_cursor {
            return Err(CodecError::MismatchedBinding);
        }
        self.cursor
            .checked_add(self.count)
            .ok_or(CodecError::ArithmeticOverflow)
    }
}

/// Strict tagged union for action 8.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)] // no_alloc contract: boxing is forbidden
pub enum WriteCandidateFeedPayloadV1<'a> {
    /// Open the commitment and allocate a FeedStage.
    Open(OpenCandidateFeedPayloadV1),
    /// Write one exact sequential segment.
    Segment(CandidateFeedSegmentPayloadV1<'a>),
}

impl<'a> WriteCandidateFeedPayloadV1<'a> {
    /// Decode variant zero or one of the four exact record families.
    pub fn decode(input: &'a [u8]) -> Result<Self, CodecError> {
        let variant = *input.first().ok_or(CodecError::WrongLength)?;
        if variant == 0 {
            Ok(Self::Open(OpenCandidateFeedPayloadV1::decode(input)?))
        } else {
            Ok(Self::Segment(CandidateFeedSegmentPayloadV1::decode(input)?))
        }
    }
}

/// Shared exact payload of actions 9, 10, 14, 16, and 32.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochNodePayloadV1 {
    /// Parent Epoch PDA.
    pub epoch: Id32,
    /// Ordinal-derived AdmissionNode PDA.
    pub node: Id32,
}

impl EpochNodePayloadV1 {
    /// Decode exactly 64 hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, EPOCH_NODE_PAYLOAD_BYTES)?;
        let value = Self {
            epoch: live_id(&mut r)?,
            node: live_id(&mut r)?,
        };
        r.finish()?;
        Ok(value)
    }
}

/// Action-15 `FinalizeSelection` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizeSelectionPayloadV1 {
    /// Parent Epoch PDA.
    pub epoch: Id32,
}

/// Action-20 `CleanupCandidate` payload.
///
/// `selected_candidate` is the all-zero sentinel exactly when the Epoch has no
/// selected artifact. Otherwise it is the actual SelectedCandidate PDA that
/// the adapter must authenticate and decode. This keeps an optional
/// fixed-position account meta unambiguous without defining another selected
/// pointer beside the Window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CleanupCandidatePayloadV1 {
    /// Parent Epoch PDA.
    pub epoch: Id32,
    /// Reverse-list-head AdmissionNode PDA to clean.
    pub node: Id32,
    /// Actual SelectedCandidate PDA, or canonical all-zero absence.
    pub selected_candidate: Id32,
}

impl CleanupCandidatePayloadV1 {
    /// Decode exactly 96 hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, CLEANUP_CANDIDATE_PAYLOAD_BYTES)?;
        let value = Self {
            epoch: live_id(&mut r)?,
            node: live_id(&mut r)?,
            selected_candidate: Id32::from_bytes(r.array()?),
        };
        r.finish()?;
        value.validate()?;
        Ok(value)
    }

    /// Validate live parents and the optional selected-artifact sentinel.
    pub fn validate(self) -> Result<(), CodecError> {
        if self.epoch.is_zero() || self.node.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if self.epoch == self.node
            || self.selected_candidate == self.epoch
            || self.selected_candidate == self.node
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }
}

/// Action-21 `ClaimSolver` payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimSolverPayloadV1 {
    /// Finalized parent Epoch PDA.
    pub epoch: Id32,
}

impl ClaimSolverPayloadV1 {
    /// Decode exactly 32 hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, CLAIM_SOLVER_PAYLOAD_BYTES)?;
        let value = Self {
            epoch: live_id(&mut r)?,
        };
        r.finish()?;
        Ok(value)
    }
}

/// Action-24 `FreezeEntitlement` identity selector.
///
/// This payload selects one owner row but does not supply its expectation. A
/// future adapter must obtain that expectation only from the complete,
/// authenticated filled-order and selected-fee projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreezeEntitlementPayloadV1 {
    /// Finalized parent Epoch PDA.
    pub epoch: Id32,
    /// Counted SelectedCandidate PDA.
    pub selected_candidate: Id32,
    /// Semantic Position owner identity.
    pub owner: Id32,
}

impl FreezeEntitlementPayloadV1 {
    /// Decode exactly 96 hostile selector bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, FREEZE_ENTITLEMENT_PAYLOAD_BYTES)?;
        let value = Self {
            epoch: live_id(&mut reader)?,
            selected_candidate: live_id(&mut reader)?,
            owner: live_id(&mut reader)?,
        };
        reader.finish()?;
        if value.epoch == value.selected_candidate
            || value.epoch == value.owner
            || value.selected_candidate == value.owner
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(value)
    }
}

/// Action-25 `EntitleSlice` claimed receipt-fragment fields.
///
/// Every field after the four identities is only an equality assertion against
/// a future authenticated receipt and order-membership projection. Decoding
/// this payload never turns caller bytes into settlement semantic truth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntitleSlicePayloadV1 {
    /// Finalized parent Epoch PDA.
    pub epoch: Id32,
    /// Counted SelectedCandidate PDA.
    pub selected_candidate: Id32,
    /// OwnerSettlement envelope PDA.
    pub owner_settlement: Id32,
    /// Receipt PDA whose authenticated body must reproduce every claim below.
    pub receipt: Id32,
    /// SelectedCandidate's exact next global slice index.
    pub slice_index: u16,
    /// Canonical selected order-set index, strictly below 64.
    pub order_index: u8,
    /// Claimed payer/payee side.
    pub side: SettlementSideV1,
    /// Claimed nonzero exact consideration in price units.
    pub consideration_price_units: u128,
    /// Claimed unique order-exhaustion bit.
    pub completes_order: bool,
}

impl EntitleSlicePayloadV1 {
    /// Decode exactly 149 hostile claimed-fragment bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, ENTITLE_SLICE_PAYLOAD_BYTES)?;
        let epoch = live_id(&mut reader)?;
        let selected_candidate = live_id(&mut reader)?;
        let owner_settlement = live_id(&mut reader)?;
        let receipt = live_id(&mut reader)?;
        let slice_index = reader.u16()?;
        let order_index = reader.u8()?;
        let side = match reader.u8()? {
            0 => SettlementSideV1::Buy,
            1 => SettlementSideV1::Sell,
            _ => return Err(CodecError::InvalidState),
        };
        let consideration_price_units = reader.u128()?;
        let completes_order = match reader.u8()? {
            0 => false,
            1 => true,
            _ => return Err(CodecError::InvalidState),
        };
        reader.finish()?;
        let value = Self {
            epoch,
            selected_candidate,
            owner_settlement,
            receipt,
            slice_index,
            order_index,
            side,
            consideration_price_units,
            completes_order,
        };
        let identities = [epoch, selected_candidate, owner_settlement, receipt];
        let mut left = 0usize;
        while left < identities.len() {
            let mut right = left + 1;
            while right < identities.len() {
                if identities[left] == identities[right] {
                    return Err(CodecError::MismatchedBinding);
                }
                right += 1;
            }
            left += 1;
        }
        if order_index >= MAX_ORDERS_U8 || consideration_price_units == 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(value)
    }

    /// Project the claimed upstream fragment after receipt equality checks.
    ///
    /// The return value remains caller-claimed until an adapter proves every
    /// field equal to an authenticated receipt and selected-order membership.
    pub const fn claimed_fragment(self) -> AuthenticatedOwnerFragmentV1 {
        AuthenticatedOwnerFragmentV1 {
            order_index: self.order_index,
            side: self.side,
            consideration_price_units: self.consideration_price_units,
            completes_order: self.completes_order,
        }
    }
}

/// Action-26 `ConsumeDirectReceiptEggs` immutable selector.
///
/// The transition ID is equality-bound to the authenticated direct receipt
/// and its complete pure poststate plan. No economic field is caller-owned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsumeDirectReceiptEggsPayloadV1 {
    /// Counted parent Epoch PDA.
    pub epoch: Id32,
    /// Canonical selected direct-receipt account PDA.
    pub receipt: Id32,
    /// Opaque identity of the complete atomic Egg/reservation/row transition.
    pub settlement_transition_id: Id32,
}

impl ConsumeDirectReceiptEggsPayloadV1 {
    /// Decode exactly 96 hostile selector bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, CONSUME_DIRECT_RECEIPT_EGGS_PAYLOAD_BYTES)?;
        let value = Self {
            epoch: live_id(&mut reader)?,
            receipt: live_id(&mut reader)?,
            settlement_transition_id: live_id(&mut reader)?,
        };
        reader.finish()?;
        if value.epoch == value.receipt
            || value.epoch == value.settlement_transition_id
            || value.receipt == value.settlement_transition_id
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(value)
    }
}

impl FinalizeSelectionPayloadV1 {
    /// Decode exactly 32 hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::exact(input, FINALIZE_SELECTION_PAYLOAD_BYTES)?;
        let value = Self {
            epoch: live_id(&mut r)?,
        };
        r.finish()?;
        Ok(value)
    }
}

/// Strict payload facts for disabled owner-settlement actions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerSettlementPayloadV1 {
    /// Action 24 selector only; creation remains disabled.
    FreezeEntitlement(FreezeEntitlementPayloadV1),
    /// Action 25 claimed receipt fragment only; mutation remains disabled.
    EntitleSlice(EntitleSlicePayloadV1),
}

/// Strict payload fact for the disabled real-ended direct Egg action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectSettlementPayloadV1 {
    /// Action 26 selector only; the SBF route remains disabled.
    ConsumeDirectReceiptEggs(ConsumeDirectReceiptEggsPayloadV1),
}

/// Decode action 26 without adding it to the live-lab union.
pub fn decode_direct_settlement_payload_v1(
    local_action: u8,
    payload: &[u8],
) -> Result<DirectSettlementPayloadV1, CodecError> {
    match local_action {
        26 => Ok(DirectSettlementPayloadV1::ConsumeDirectReceiptEggs(
            ConsumeDirectReceiptEggsPayloadV1::decode(payload)?,
        )),
        _ => Err(CodecError::InvalidState),
    }
}

/// Decode only actions 24 and 25 without adding them to the live-lab union.
pub fn decode_owner_settlement_payload_v1(
    local_action: u8,
    payload: &[u8],
) -> Result<OwnerSettlementPayloadV1, CodecError> {
    match local_action {
        24 => Ok(OwnerSettlementPayloadV1::FreezeEntitlement(
            FreezeEntitlementPayloadV1::decode(payload)?,
        )),
        25 => Ok(OwnerSettlementPayloadV1::EntitleSlice(
            EntitleSlicePayloadV1::decode(payload)?,
        )),
        _ => Err(CodecError::InvalidState),
    }
}

/// Decoded payload for one frozen pure General V2 action contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)] // no_alloc contract: boxing is forbidden
pub enum IdentityLabPayloadV1<'a> {
    /// Action 2.
    InitEpoch(InitEpochPayloadV1),
    /// Action 6.
    FreezeEpoch(FreezeEpochPayloadV1),
    /// Action 7.
    BeginCandidate(BeginCandidatePayloadV1),
    /// Action 8.
    WriteCandidateFeed(WriteCandidateFeedPayloadV1<'a>),
    /// Action 9.
    SealCandidate(EpochNodePayloadV1),
    /// Action 10.
    InitClearWork(EpochNodePayloadV1),
    /// Action 14.
    CompleteCandidateVerification(EpochNodePayloadV1),
    /// Action 15.
    FinalizeSelection(FinalizeSelectionPayloadV1),
    /// Action 16, bounded to an unrevealed committed candidate.
    ExpireCommittedCandidate(EpochNodePayloadV1),
    /// Action 20.
    CleanupCandidate(CleanupCandidatePayloadV1),
    /// Action 21.
    ClaimSolver(ClaimSolverPayloadV1),
    /// Action 32.
    CloseClearWork(EpochNodePayloadV1),
}

/// Decode only a frozen pure General V2 action payload contract.
pub fn decode_identity_lab_payload_v1(
    local_action: u8,
    payload: &[u8],
) -> Result<IdentityLabPayloadV1<'_>, CodecError> {
    if payload.len() > MAX_GENERAL_V2_ACTION_PAYLOAD_BYTES {
        return Err(CodecError::WrongLength);
    }
    match local_action {
        2 => Ok(IdentityLabPayloadV1::InitEpoch(InitEpochPayloadV1::decode(
            payload,
        )?)),
        6 => Ok(IdentityLabPayloadV1::FreezeEpoch(
            FreezeEpochPayloadV1::decode(payload)?,
        )),
        7 => Ok(IdentityLabPayloadV1::BeginCandidate(
            BeginCandidatePayloadV1::decode(payload)?,
        )),
        8 => Ok(IdentityLabPayloadV1::WriteCandidateFeed(
            WriteCandidateFeedPayloadV1::decode(payload)?,
        )),
        9 => Ok(IdentityLabPayloadV1::SealCandidate(
            EpochNodePayloadV1::decode(payload)?,
        )),
        10 => Ok(IdentityLabPayloadV1::InitClearWork(
            EpochNodePayloadV1::decode(payload)?,
        )),
        14 => Ok(IdentityLabPayloadV1::CompleteCandidateVerification(
            EpochNodePayloadV1::decode(payload)?,
        )),
        15 => Ok(IdentityLabPayloadV1::FinalizeSelection(
            FinalizeSelectionPayloadV1::decode(payload)?,
        )),
        16 => Ok(IdentityLabPayloadV1::ExpireCommittedCandidate(
            EpochNodePayloadV1::decode(payload)?,
        )),
        20 => Ok(IdentityLabPayloadV1::CleanupCandidate(
            CleanupCandidatePayloadV1::decode(payload)?,
        )),
        21 => Ok(IdentityLabPayloadV1::ClaimSolver(
            ClaimSolverPayloadV1::decode(payload)?,
        )),
        32 => Ok(IdentityLabPayloadV1::CloseClearWork(
            EpochNodePayloadV1::decode(payload)?,
        )),
        _ => Err(CodecError::InvalidState),
    }
}

fn live_id(reader: &mut Reader<'_>) -> Result<Id32, CodecError> {
    Id32::new(reader.array()?)
}

const _: () = assert!(OPEN_CANDIDATE_FEED_PAYLOAD_BYTES == 1 + (9 * 32) + (5 * 8) + 4 + 2 + 1);
const _: () = assert!(WRITE_CANDIDATE_FEED_FIXED_BYTES == 1 + (2 * 32) + (2 * 2));

#[cfg(test)]
mod tests {
    use super::*;

    fn live(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    #[test]
    fn fixed_payloads_are_exact_and_nonzero() {
        let mut init = [0u8; INIT_EPOCH_PAYLOAD_BYTES];
        init[..32].copy_from_slice(&live(1));
        init[32..40].copy_from_slice(&7u64.to_le_bytes());
        init[40..].copy_from_slice(&99u64.to_le_bytes());
        assert_eq!(InitEpochPayloadV1::decode(&init).unwrap().epoch_index, 7);
        assert_eq!(
            InitEpochPayloadV1::decode(&init[..47]),
            Err(CodecError::WrongLength)
        );
        let mut trailing = [0u8; INIT_EPOCH_PAYLOAD_BYTES + 1];
        trailing[..INIT_EPOCH_PAYLOAD_BYTES].copy_from_slice(&init);
        assert_eq!(
            InitEpochPayloadV1::decode(&trailing),
            Err(CodecError::WrongLength)
        );
        init[0..32].fill(0);
        assert_eq!(
            InitEpochPayloadV1::decode(&init),
            Err(CodecError::ZeroIdentity)
        );

        for (action, len) in [(6, 32usize), (7, 64), (9, 64), (10, 64), (14, 64), (15, 32)] {
            let bytes = [7u8; 64];
            assert!(decode_identity_lab_payload_v1(action, &bytes[..len]).is_ok());
            assert_eq!(
                decode_identity_lab_payload_v1(action, &bytes[..len - 1]),
                Err(CodecError::WrongLength)
            );
        }
        assert_eq!(
            decode_identity_lab_payload_v1(1, &[]),
            Err(CodecError::InvalidState)
        );
    }

    #[test]
    fn feed_open_and_segments_refuse_hostile_geometry() {
        let mut open = [0u8; OPEN_CANDIDATE_FEED_PAYLOAD_BYTES];
        let mut at = 1usize;
        for byte in 1..=9 {
            open[at..at + 32].copy_from_slice(&live(byte));
            at += 32;
        }
        at += 5 * 8;
        open[at] = 1;
        open[at + 1] = 2;
        open[at + 2] = 0;
        open[at + 3] = 1;
        open[at + 4..at + 6].copy_from_slice(&0u16.to_le_bytes());
        open[at + 6] = 0;
        // Price scale and denominator are in the preceding five u64 fields.
        let scalar_at = 1 + 9 * 32;
        open[scalar_at + 24..scalar_at + 32].copy_from_slice(&100u64.to_le_bytes());
        open[scalar_at + 32..scalar_at + 40].copy_from_slice(&1u64.to_le_bytes());
        // Direct final and base IDs must match.
        open[1 + 4 * 32..1 + 5 * 32].copy_from_slice(&live(5));
        open[1 + 5 * 32..1 + 6 * 32].copy_from_slice(&live(5));
        assert!(OpenCandidateFeedPayloadV1::decode(&open).is_ok());
        open[0] = 5;
        assert_eq!(
            OpenCandidateFeedPayloadV1::decode(&open),
            Err(CodecError::InvalidState)
        );

        let mut segment = [0u8; WRITE_CANDIDATE_FEED_FIXED_BYTES + 16];
        segment[0] = CandidateFeedWriteKindV1::Prices.to_byte();
        segment[1..33].copy_from_slice(&live(1));
        segment[33..65].copy_from_slice(&live(2));
        segment[65..67].copy_from_slice(&3u16.to_le_bytes());
        segment[67..69].copy_from_slice(&2u16.to_le_bytes());
        let decoded = CandidateFeedSegmentPayloadV1::decode(&segment).unwrap();
        assert_eq!(decoded.require_cursor(3), Ok(5));
        assert_eq!(
            decoded.require_cursor(2),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            CandidateFeedSegmentPayloadV1::decode(&segment[..84]),
            Err(CodecError::WrongLength)
        );
        assert_eq!(
            CandidateFeedSegmentPayloadV1 {
                records: &decoded.records[..8],
                ..decoded
            }
            .validate(),
            Err(CodecError::WrongLength)
        );
        segment[67..69].fill(0);
        assert_eq!(
            CandidateFeedSegmentPayloadV1::decode(&segment),
            Err(CodecError::InvalidCount)
        );
    }
}
