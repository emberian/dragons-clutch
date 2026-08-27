// SPDX-License-Identifier: AGPL-3.0-or-later

//! Rent-owned Dealer attachment for one counted CoveredDealer selection.
//!
//! The General settlement root owns only the opaque Dealer-child count. This
//! module owns the Dealer-specific quote, receipt, allocation, fee, Lease, and
//! rent facts. The large quote preimage is authenticated once by the SBF
//! boundary and checked by `clutch-batch`; only its digest and the uniquely
//! derived verdict survive in this account. A public verdict DTO can never
//! mint this account: construction requires [`VerifiedDealerLegV2`].

use clutch_batch::dealer_leg_v2::{
    AggregateDealerTradeV2, DealerCashAllocationV2, DealerCashPolicyV2,
    DealerFacilityBindingV2, DealerFillRowV2, DealerLegCandidateV2, DealerLegVerdictV2,
    DealerQuotePreconditionV2, DealerQuoteRowV2, DealerReceiptV2,
    VerifiedDealerLegRefV2, VerifiedDealerLegV2, EMPTY_DEALER_CASH_ALLOCATION_V2,
    EMPTY_DEALER_FILL_ROW_V2, EMPTY_DEALER_QUOTE_ROW_V2, MAX_DEALER_ROWS_V2,
};
use clutch_batch::portfolio_execution_v2::AuthenticatedSelectedPortfolioOrderV2;
use clutch_batch::Side;
use clutch_general_v2_contract::{
    SettlementRootChildStateV1, SettlementRootPhaseV1, SettlementRootV1AccountV1,
};
use clutch_price_measure::VerifiedPriceMeasureV3;
use sha2::{Digest, Sha256};

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    DealerEpochBindingPhaseV2, DealerEpochBindingV2, DealerPolicyV1, DealerRuntimeActionV1,
    DealerLeaseV2, DealerSelectedFeeRecordBindingV1, DealerStateV2, DeletableRentOwnerV1, Error,
    DealerSettlementSliceV2, FixedCodec, Id, Result, SettlementPotV2,
    DEALER_COVERED_SELECTION_CONTENT_DOMAIN_V1,
    DEALER_QUOTE_ADMISSION_CONTENT_DOMAIN_V1, DELETABLE_RENT_OWNER_BYTES, MAX_OUTCOMES,
};

/// Private authority for one current CoveredDealer settlement row.
///
/// The selected RelationV2 membership capability remains the only path to
/// coefficients, owner, Position incarnation, and dense order index.  The
/// immutable Dealer selection remains the only path to the verified fill and
/// cash allocation.  This value joins those owners; it does not persist a
/// second coefficient or allocation DTO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoveredDealerSettlementRowV1 {
    selection_id: Id,
    selected_fee_binding_digest: Id,
    row_index: u16,
    order_index: u8,
    order_id: Id,
    owner_id: Id,
    position_account_id: Id,
    position_generation: u64,
    side: Side,
    fill_units: u64,
    cash_in_atoms: u64,
    cash_out_atoms: u64,
    native_eggs: [u64; MAX_OUTCOMES],
}

impl CoveredDealerSettlementRowV1 {
    /// Immutable CoveredDealer certificate identity.
    pub const fn selection_id(&self) -> Id {
        self.selection_id
    }

    /// Immutable selected owner-netted fee binding used as Replay evidence.
    pub const fn selected_fee_binding_digest(&self) -> Id {
        self.selected_fee_binding_digest
    }
    /// Canonical Dealer allocation cursor.
    pub const fn row_index(&self) -> u16 {
        self.row_index
    }

    /// Dense RelationV2 order index authenticated through the page set.
    pub const fn order_index(&self) -> u8 {
        self.order_index
    }

    /// Exact immutable order identity.
    pub const fn order_id(&self) -> Id {
        self.order_id
    }

    /// Exact page-authenticated owner.
    pub const fn owner_id(&self) -> Id {
        self.owner_id
    }

    /// Exact current General Position account.
    pub const fn position_account_id(&self) -> Id {
        self.position_account_id
    }

    /// Stable Position incarnation committed by Reservation and page.
    pub const fn position_generation(&self) -> u64 {
        self.position_generation
    }

    /// Exact RelationV2 side.
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Verified Dealer fill units.
    pub const fn fill_units(&self) -> u64 {
        self.fill_units
    }

    /// Position/Reservation assets entering the Pot during Collect.
    pub const fn collect_slice(&self) -> DealerSettlementSliceV2 {
        DealerSettlementSliceV2 {
            start: self.row_index,
            end: self.row_index + 1,
            cash_atoms: match self.side {
                Side::Buy => self.cash_in_atoms,
                Side::Sell => 0,
            },
            eggs: match self.side {
                Side::Buy => [0; MAX_OUTCOMES],
                Side::Sell => self.native_eggs,
            },
        }
    }

    /// Pot assets entering the Position during Deliver.
    pub const fn deliver_slice(&self) -> DealerSettlementSliceV2 {
        DealerSettlementSliceV2 {
            start: self.row_index,
            end: self.row_index + 1,
            cash_atoms: match self.side {
                Side::Buy => 0,
                Side::Sell => self.cash_out_atoms,
            },
            eggs: match self.side {
                Side::Buy => self.native_eggs,
                Side::Sell => [0; MAX_OUTCOMES],
            },
        }
    }
}

/// Indivisible authenticated row asset transition committed by facility Replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoveredDealerRowAssetTransitionV1 {
    action: DealerRuntimeActionV1,
    row: CoveredDealerSettlementRowV1,
    bundle_id: Id,
}

impl CoveredDealerRowAssetTransitionV1 {
    /// Construct only from the joined private row authority and adapter-
    /// rederived exact account postimages.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        action: DealerRuntimeActionV1,
        row: CoveredDealerSettlementRowV1,
        reservation_account_id: Id,
        reservation_pre_data_id: Id,
        reservation_post_data_id: Id,
        position_pre_semantic_id: Id,
        position_post_semantic_id: Id,
        general_replay_pre_semantic_id: Id,
        general_replay_post_semantic_id: Id,
        pot_pre_content_id: Id,
        pot_post_content_id: Id,
    ) -> Result<Self> {
        let action_byte = match action {
            DealerRuntimeActionV1::Collect => 15u8,
            DealerRuntimeActionV1::Deliver => 16u8,
            _ => return Err(Error::InvalidParameter),
        };
        for identity in [
            row.selection_id,
            row.selected_fee_binding_digest,
            row.order_id,
            row.owner_id,
            row.position_account_id,
            reservation_account_id,
            reservation_pre_data_id,
            reservation_post_data_id,
            position_pre_semantic_id,
            position_post_semantic_id,
            general_replay_pre_semantic_id,
            general_replay_post_semantic_id,
            pot_pre_content_id,
            pot_post_content_id,
        ] {
            identity.validate_live()?;
        }
        if reservation_pre_data_id == reservation_post_data_id
            || general_replay_pre_semantic_id == general_replay_post_semantic_id
            || pot_pre_content_id == pot_post_content_id
        {
            return Err(Error::MismatchedBinding);
        }
        let side_byte = match row.side {
            Side::Buy => 0u8,
            Side::Sell => 1u8,
        };
        let slice = match action {
            DealerRuntimeActionV1::Collect => row.collect_slice(),
            DealerRuntimeActionV1::Deliver => row.deliver_slice(),
            _ => return Err(Error::InvalidParameter),
        };
        let mut hasher = Sha256::new();
        hasher.update(b"dragons-clutch/dealer-covered-row-asset-transition/v1\0");
        hasher.update([action_byte, side_byte]);
        hasher.update(row.row_index.to_le_bytes());
        hasher.update([row.order_index]);
        hasher.update(row.fill_units.to_le_bytes());
        for identity in [
            row.selection_id,
            row.selected_fee_binding_digest,
            row.order_id,
            row.owner_id,
            row.position_account_id,
            reservation_account_id,
            reservation_pre_data_id,
            reservation_post_data_id,
            position_pre_semantic_id,
            position_post_semantic_id,
            general_replay_pre_semantic_id,
            general_replay_post_semantic_id,
            pot_pre_content_id,
            pot_post_content_id,
        ] {
            hasher.update(identity.bytes());
        }
        hasher.update(slice.cash_atoms.to_le_bytes());
        for amount in slice.eggs {
            hasher.update(amount.to_le_bytes());
        }
        let bundle_id = Id::from_bytes(hasher.finalize().into());
        bundle_id.validate_live()?;
        Ok(Self {
            action,
            row,
            bundle_id,
        })
    }

    /// Exact asset transition commitment consumed by facility Replay.
    pub const fn bundle_id(&self) -> Id {
        self.bundle_id
    }

    /// Authenticated row authority.
    pub const fn row(&self) -> CoveredDealerSettlementRowV1 {
        self.row
    }

    /// Exact Dealer action.
    pub const fn action(&self) -> DealerRuntimeActionV1 {
        self.action
    }
}

/// Local semantic-body magic for a relayed signed quote admission.
pub const DEALER_QUOTE_ADMISSION_MAGIC_V1: [u8; 8] = *b"DCQADMV1";
/// Exact local quote-admission version.
pub const DEALER_QUOTE_ADMISSION_VERSION_V1: u16 = 1;
/// Exact signed quote-admission body bytes.
pub const DEALER_QUOTE_ADMISSION_BYTES_V1: usize =
    HEADER_BYTES + (9 * 32) + (5 * 8) + 2_568 + 4_040;

/// Local semantic-body magic for the counted Dealer selection attachment.
pub const DEALER_COVERED_SELECTION_MAGIC_V1: [u8; 8] = *b"DCCOVDV1";
/// Exact local selection-attachment version.
pub const DEALER_COVERED_SELECTION_VERSION_V1: u16 = 1;
/// Exact fixed account body bytes, excluding the eight-byte global envelope.
pub const DEALER_COVERED_SELECTION_BYTES_V1: usize =
    HEADER_BYTES + (28 * 32) + (7 * 8) + 16 + (2 * MAX_OUTCOMES * 8)
        + (MAX_DEALER_ROWS_V2 * 64)
        + 16
        + 8
        + DELETABLE_RENT_OWNER_BYTES;

/// Signed, permissionlessly relayable Dealer quote preimage.
///
/// This is data, not signature evidence. The SBF boundary must prove an exact
/// Ed25519 precompile instruction authenticated `admission_id()` under
/// `quote_authority` before it may pass the contained quote to the economic
/// verifier or create a selection attachment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerQuoteAdmissionV1 {
    /// Ordinary Ed25519 public key selected by Dealer policy.
    pub quote_authority: Id,
    /// Exact immutable Dealer policy.
    pub policy_id: Id,
    /// Exact facility identity.
    pub facility_id: Id,
    /// Authoritative Dealer State account.
    pub dealer_state_account_id: Id,
    /// Semantic Dealer Epoch identity.
    pub epoch_id: Id,
    /// Counted General SettlementRoot account.
    pub settlement_root_account_id: Id,
    /// Retained selected Feed account.
    pub retained_feed_account_id: Id,
    /// Final SettlementCandidateId.
    pub settlement_candidate_id: Id,
    /// Exact selected owner-netted fee binding.
    pub selected_fee_binding_digest: Id,
    /// Slot at which the authority issued the quote.
    pub issued_slot: u64,
    /// First slot at which this admission is stale.
    pub expires_slot: u64,
    /// First slot at which collection may no longer begin or advance.
    pub collect_deadline_slot: u64,
    /// First slot at which delivery/finalization may no longer advance.
    pub deliver_deadline_slot: u64,
    /// Active native outcome width.
    pub outcome_count: u8,
    /// Submitted Dealer fills.
    pub dealer: DealerLegCandidateV2,
    /// Proof-independent quote, receipt, and fee envelopes.
    pub quote: DealerQuotePreconditionV2,
}

impl DealerQuoteAdmissionV1 {
    /// Canonical invalid decode target for allocation-owning adapters.
    ///
    /// An SBF adapter can copy this static value directly into heap storage
    /// and call [`Self::decode_into`] without ever placing the 6,948-byte quote
    /// body on a 4-KiB call frame.
    pub const ZEROED: Self = Self {
        quote_authority: Id::ZERO,
        policy_id: Id::ZERO,
        facility_id: Id::ZERO,
        dealer_state_account_id: Id::ZERO,
        epoch_id: Id::ZERO,
        settlement_root_account_id: Id::ZERO,
        retained_feed_account_id: Id::ZERO,
        settlement_candidate_id: Id::ZERO,
        selected_fee_binding_digest: Id::ZERO,
        issued_slot: 0,
        expires_slot: 0,
        collect_deadline_slot: 0,
        deliver_deadline_slot: 0,
        outcome_count: 0,
        dealer: DealerLegCandidateV2 {
            rows: [EMPTY_DEALER_FILL_ROW_V2; MAX_DEALER_ROWS_V2],
            row_count: 0,
        },
        quote: DealerQuotePreconditionV2 {
            upstream_economic_candidate_digest: [0; 32],
            facility: DealerFacilityBindingV2 {
                version: 0,
                facility_semantics_digest: [0; 32],
                policy_semantics_digest: [0; 32],
                pre_generation: 0,
            },
            cash_policy: DealerCashPolicyV2::MinimumGrossHamiltonV1,
            fee_policy_semantics_digest: [0; 32],
            trade: AggregateDealerTradeV2 {
                sell_to_users: [0; MAX_OUTCOMES],
                buy_from_users: [0; MAX_OUTCOMES],
            },
            receipt: DealerReceiptV2 {
                dealer_net_cash_in_atoms: 0,
                dealer_net_cash_out_atoms: 0,
            },
            rows: [EMPTY_DEALER_QUOTE_ROW_V2; MAX_DEALER_ROWS_V2],
            semantic_quote_digest: [0; 32],
        },
    };

    /// Validate identities, lifetime, and canonical row/padding shape.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.quote_authority,
            self.policy_id,
            self.facility_id,
            self.dealer_state_account_id,
            self.epoch_id,
            self.settlement_root_account_id,
            self.retained_feed_account_id,
            self.settlement_candidate_id,
            self.selected_fee_binding_digest,
        ] {
            identity.validate_live()?;
        }
        if self.issued_slot == 0
            || self.issued_slot >= self.expires_slot
            || self.expires_slot > self.collect_deadline_slot
            || self.collect_deadline_slot >= self.deliver_deadline_slot
            || self.outcome_count < 2
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.dealer.row_count == 0
            || usize::from(self.dealer.row_count) > MAX_DEALER_ROWS_V2
            || self.quote.facility.version != 2
            || self.quote.cash_policy != DealerCashPolicyV2::MinimumGrossHamiltonV1
            || self.quote.receipt.dealer_net_cash_in_atoms != 0
                && self.quote.receipt.dealer_net_cash_out_atoms != 0
            || self.quote.semantic_quote_digest == [0; 32]
            || self.quote.upstream_economic_candidate_digest == [0; 32]
            || self.quote.facility.facility_semantics_digest != self.facility_id.bytes()
            || self.quote.facility.policy_semantics_digest != self.policy_id.bytes()
        {
            return Err(Error::InvalidParameter);
        }
        let count = usize::from(self.dealer.row_count);
        let mut row = 0usize;
        while row < MAX_DEALER_ROWS_V2 {
            let fill = self.dealer.rows[row];
            let quote = self.quote.rows[row];
            if row < count {
                if fill.order_id == [0; 32]
                    || fill.dealer_fill_units == 0
                    || quote.order_id != fill.order_id
                    || row > 0 && self.dealer.rows[row - 1].order_id >= fill.order_id
                {
                    return Err(Error::InvalidParameter);
                }
            } else if fill != EMPTY_DEALER_FILL_ROW_V2 || quote != EMPTY_DEALER_QUOTE_ROW_V2 {
                return Err(Error::NonCanonicalPadding);
            }
            row += 1;
        }
        let mut outcome = usize::from(self.outcome_count);
        while outcome < MAX_OUTCOMES {
            if self.quote.trade.sell_to_users[outcome] != 0
                || self.quote.trade.buy_from_users[outcome] != 0
            {
                return Err(Error::NonCanonicalPadding);
            }
            outcome += 1;
        }
        Ok(())
    }

    /// Content identity signed by the policy-selected Ed25519 authority.
    pub fn admission_id(&self) -> Result<Id> {
        self.validate()?;
        let mut hasher = Sha256::new();
        hasher.update(DEALER_QUOTE_ADMISSION_CONTENT_DOMAIN_V1);
        hash_quote_admission(self, &mut hasher);
        let value = Id::from_bytes(hasher.finalize().into());
        value.validate_live()?;
        Ok(value)
    }

    /// Hostile-decode directly into caller-owned storage.
    ///
    /// This is semantically identical to [`FixedCodec::decode`], including
    /// complete postimage validation, but it does not require a by-value
    /// 6,948-byte return object.
    pub fn decode_into(input: &[u8], output: &mut Self) -> Result<()> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_QUOTE_ADMISSION_MAGIC_V1,
            DEALER_QUOTE_ADMISSION_VERSION_V1,
        )?;
        read_quote_admission_into(&mut reader, output)?;
        reader.finish()?;
        output.validate()
    }
}

impl FixedCodec for DealerQuoteAdmissionV1 {
    const ENCODED_LEN: usize = DEALER_QUOTE_ADMISSION_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_QUOTE_ADMISSION_MAGIC_V1,
            DEALER_QUOTE_ADMISSION_VERSION_V1,
        );
        write_quote_admission(self, &mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut value = Self::ZEROED;
        Self::decode_into(input, &mut value)?;
        Ok(value)
    }
}

/// Adapter-authenticated coordinates that are not owned by the Dealer quote.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoveredDealerSelectionContextV1 {
    /// Physical new `0xae/1` account.
    pub selection_account_id: Id,
    /// Exact counted General SettlementRoot account.
    pub settlement_root_account_id: Id,
    /// Exact retained Feed account.
    pub retained_feed_account_id: Id,
    /// Full upstream RelationV2 candidate identity.
    pub upstream_economic_candidate_id: Id,
    /// Exact Feed candidate-bundle identity.
    pub candidate_bundle_digest: Id,
    /// Exact Feed settlement witness.
    pub settlement_witness_digest: Id,
    /// Derived Lease account created in the same instruction.
    pub lease_account_id: Id,
    /// Derived Pot account created in the same instruction.
    pub settlement_pot_account_id: Id,
    /// Current slot.
    pub current_slot: u64,
    /// Stored selection-account bump.
    pub stored_bump: u8,
    /// Exact refundable principal and hostile-prefund disposition.
    pub rent: DeletableRentOwnerV1,
}

/// Immutable Dealer-owned child created atomically with Lease/Pot admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoveredDealerSelectionV1 {
    /// Physical selection account.
    pub selection_account_id: Id,
    /// Dealer policy identity.
    pub policy_id: Id,
    /// Facility identity.
    pub facility_id: Id,
    /// Canonical Position-purpose binding.
    pub facility_position_binding_id: Id,
    /// Authoritative Dealer State account.
    pub dealer_state_account_id: Id,
    /// Full MarketInstanceV2 identity.
    pub market_instance_v2_id: Id,
    /// Semantic General Epoch identity.
    pub epoch_id: Id,
    /// Counted Dealer Epoch-binding account.
    pub epoch_binding_account_id: Id,
    /// Counted General SettlementRoot account.
    pub settlement_root_account_id: Id,
    /// Retained General Feed account.
    pub retained_feed_account_id: Id,
    /// Frozen General order-set identity.
    pub order_set_id: Id,
    /// Final SettlementCandidateId.
    pub settlement_candidate_id: Id,
    /// Upstream RelationV2 candidate identity.
    pub upstream_economic_candidate_id: Id,
    /// Exact Feed bundle identity.
    pub candidate_bundle_digest: Id,
    /// Exact Feed settlement witness.
    pub settlement_witness_digest: Id,
    /// EconomicDomainV2 identity.
    pub economic_domain_id: Id,
    /// Quantized price-certificate body identity.
    pub curve_price_certificate_id: Id,
    /// Ordinary Ed25519 quote authority.
    pub quote_authority: Id,
    /// Exact signed quote-admission identity.
    pub quote_admission_id: Id,
    /// Proof-independent quote identity.
    pub quote_semantics_id: Id,
    /// General-selected batch policy.
    pub batch_policy_id: Id,
    /// General-selected score policy.
    pub score_policy_id: Id,
    /// Selected fee-record account.
    pub selected_fee_record_account_id: Id,
    /// Selected fee-record semantic identity.
    pub selected_fee_record_semantic_id: Id,
    /// Exact selected-fee binding projection.
    pub selected_fee_binding_digest: Id,
    /// Exact revenue policy.
    pub fee_revenue_policy_id: Id,
    /// Lease account created in the same rollback domain.
    pub lease_account_id: Id,
    /// Pot account created in the same rollback domain.
    pub settlement_pot_account_id: Id,
    /// Dealer generation consumed by the Lease.
    pub dealer_generation: u64,
    /// Exact General Epoch generation.
    pub general_epoch_generation: u64,
    /// Window-owned first-admitted ordinal.
    pub selected_ordinal: u64,
    /// Creation slot.
    pub created_slot: u64,
    /// Quote expiry copied from signed evidence.
    pub quote_expires_slot: u64,
    /// Signed collection deadline copied into Lease.
    pub collect_deadline_slot: u64,
    /// Signed delivery/finalization deadline copied into Lease.
    pub deliver_deadline_slot: u64,
    /// Uniquely derived Dealer net-cash receipt.
    pub receipt: DealerReceiptV2,
    /// Uniquely derived aggregate Dealer Egg trade.
    pub trade: AggregateDealerTradeV2,
    /// Uniquely derived per-order cash allocations.
    pub allocations: [DealerCashAllocationV2; MAX_DEALER_ROWS_V2],
    /// Exact sum of selected external fee atoms.
    pub total_external_fee_atoms: u128,
    /// Active native outcome width.
    pub outcome_count: u8,
    /// Active allocation prefix.
    pub allocation_count: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Rent principal owner; never the permissionless relay actor by default.
    pub rent: DeletableRentOwnerV1,
}

impl CoveredDealerSelectionV1 {
    /// Canonical invalid target for heap-first construction by adapters.
    pub const ZEROED: Self = Self {
        selection_account_id: Id::ZERO,
        policy_id: Id::ZERO,
        facility_id: Id::ZERO,
        facility_position_binding_id: Id::ZERO,
        dealer_state_account_id: Id::ZERO,
        market_instance_v2_id: Id::ZERO,
        epoch_id: Id::ZERO,
        epoch_binding_account_id: Id::ZERO,
        settlement_root_account_id: Id::ZERO,
        retained_feed_account_id: Id::ZERO,
        order_set_id: Id::ZERO,
        settlement_candidate_id: Id::ZERO,
        upstream_economic_candidate_id: Id::ZERO,
        candidate_bundle_digest: Id::ZERO,
        settlement_witness_digest: Id::ZERO,
        economic_domain_id: Id::ZERO,
        curve_price_certificate_id: Id::ZERO,
        quote_authority: Id::ZERO,
        quote_admission_id: Id::ZERO,
        quote_semantics_id: Id::ZERO,
        batch_policy_id: Id::ZERO,
        score_policy_id: Id::ZERO,
        selected_fee_record_account_id: Id::ZERO,
        selected_fee_record_semantic_id: Id::ZERO,
        selected_fee_binding_digest: Id::ZERO,
        fee_revenue_policy_id: Id::ZERO,
        lease_account_id: Id::ZERO,
        settlement_pot_account_id: Id::ZERO,
        dealer_generation: 0,
        general_epoch_generation: 0,
        selected_ordinal: 0,
        created_slot: 0,
        quote_expires_slot: 0,
        collect_deadline_slot: 0,
        deliver_deadline_slot: 0,
        receipt: DealerReceiptV2 {
            dealer_net_cash_in_atoms: 0,
            dealer_net_cash_out_atoms: 0,
        },
        trade: AggregateDealerTradeV2 {
            sell_to_users: [0; MAX_OUTCOMES],
            buy_from_users: [0; MAX_OUTCOMES],
        },
        allocations: [EMPTY_DEALER_CASH_ALLOCATION_V2; MAX_DEALER_ROWS_V2],
        total_external_fee_atoms: 0,
        outcome_count: 0,
        allocation_count: 0,
        stored_bump: 0,
        rent: DeletableRentOwnerV1 {
            payer: Id::ZERO,
            neutral_sink: Id::ZERO,
            refundable_principal: 0,
            donation_floor: 0,
        },
    };

    /// Create the sole canonical Dealer attachment from private verifier
    /// capabilities and authenticated General/fee owners.
    #[allow(clippy::too_many_arguments)]
    pub fn from_verified(
        context: CoveredDealerSelectionContextV1,
        policy: &DealerPolicyV1,
        state: &DealerStateV2,
        epoch: &DealerEpochBindingV2,
        root: &SettlementRootV1AccountV1,
        quote_admission: &DealerQuoteAdmissionV1,
        dealer: &VerifiedDealerLegV2,
        price: &VerifiedPriceMeasureV3,
        selected_fee: &DealerSelectedFeeRecordBindingV1,
    ) -> Result<Self> {
        let mut value = Self::ZEROED;
        Self::populate_from_verified_verdict(
            context,
            policy,
            state,
            epoch,
            root,
            quote_admission,
            dealer.verdict(),
            price,
            selected_fee,
            &mut value,
        )?;
        Ok(value)
    }

    /// Create the same attachment from the frame-bounded borrowed Dealer
    /// verifier capability.
    #[allow(clippy::too_many_arguments)]
    pub fn from_verified_ref(
        context: CoveredDealerSelectionContextV1,
        policy: &DealerPolicyV1,
        state: &DealerStateV2,
        epoch: &DealerEpochBindingV2,
        root: &SettlementRootV1AccountV1,
        quote_admission: &DealerQuoteAdmissionV1,
        dealer: &VerifiedDealerLegRefV2<'_>,
        price: &VerifiedPriceMeasureV3,
        selected_fee: &DealerSelectedFeeRecordBindingV1,
    ) -> Result<Self> {
        let mut value = Self::ZEROED;
        Self::from_verified_ref_into(
            context,
            policy,
            state,
            epoch,
            root,
            quote_admission,
            dealer,
            price,
            selected_fee,
            &mut value,
        )?;
        Ok(value)
    }

    /// Construct directly into caller-owned storage from the borrowed
    /// frame-bounded verifier capability.
    #[allow(clippy::too_many_arguments)]
    pub fn from_verified_ref_into(
        context: CoveredDealerSelectionContextV1,
        policy: &DealerPolicyV1,
        state: &DealerStateV2,
        epoch: &DealerEpochBindingV2,
        root: &SettlementRootV1AccountV1,
        quote_admission: &DealerQuoteAdmissionV1,
        dealer: &VerifiedDealerLegRefV2<'_>,
        price: &VerifiedPriceMeasureV3,
        selected_fee: &DealerSelectedFeeRecordBindingV1,
        output: &mut Self,
    ) -> Result<()> {
        Self::populate_from_verified_verdict(
            context,
            policy,
            state,
            epoch,
            root,
            quote_admission,
            dealer.verdict(),
            price,
            selected_fee,
            output,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn populate_from_verified_verdict(
        context: CoveredDealerSelectionContextV1,
        policy: &DealerPolicyV1,
        state: &DealerStateV2,
        epoch: &DealerEpochBindingV2,
        root: &SettlementRootV1AccountV1,
        quote_admission: &DealerQuoteAdmissionV1,
        dealer: &DealerLegVerdictV2,
        price: &VerifiedPriceMeasureV3,
        selected_fee: &DealerSelectedFeeRecordBindingV1,
        output: &mut Self,
    ) -> Result<()> {
        policy.validate()?;
        state.validate_against_policy(policy)?;
        epoch.validate()?;
        root.validate().map_err(|_| Error::MismatchedBinding)?;
        quote_admission.validate()?;
        selected_fee.validate()?;
        context.rent.validate()?;
        for identity in [
            context.selection_account_id,
            context.settlement_root_account_id,
            context.retained_feed_account_id,
            context.upstream_economic_candidate_id,
            context.candidate_bundle_digest,
            context.settlement_witness_digest,
            context.lease_account_id,
            context.settlement_pot_account_id,
        ] {
            identity.validate_live()?;
        }
        let counts = root.counts();
        let final_candidate = Id::from_bytes(dealer.dealer_economic_candidate_digest);
        let quote_semantics = Id::from_bytes(dealer.dealer_quote_semantics_digest);
        let price_bindings = price.bindings();
        let price_body = Id::from_bytes(price.body_digest());
        let selected_fee_digest = selected_fee.binding_digest()?;
        let policy_id = policy.policy_id()?;
        if context.current_slot == 0
            || context.current_slot < quote_admission.issued_slot
            || context.current_slot >= quote_admission.expires_slot
            || quote_admission.deliver_deadline_slot > policy.maturity_slot
            || context.rent.neutral_sink != policy.neutral_sink
            || root.phase() != SettlementRootPhaseV1::Materializing
            || counts.expected_dealer_children != 1
            || counts.admitted_dealer_children != 0
            || counts.live_dealer_children != 0
            || root.retained_feed_state() != SettlementRootChildStateV1::Live
            || epoch.phase != DealerEpochBindingPhaseV2::Bound
            || !epoch.settlement_candidate_id.is_zero()
            || quote_admission.quote_authority != policy.quote_authority
            || quote_admission.policy_id != policy_id
            || quote_admission.facility_id != state.facility_id
            || quote_admission.dealer_state_account_id != epoch.dealer_state_account_id
            || quote_admission.epoch_id != epoch.epoch_id
            || quote_admission.settlement_root_account_id != context.settlement_root_account_id
            || quote_admission.retained_feed_account_id != context.retained_feed_account_id
            || quote_admission.settlement_candidate_id != final_candidate
            || quote_admission.selected_fee_binding_digest != selected_fee_digest
            || quote_admission.quote.upstream_economic_candidate_digest
                != context.upstream_economic_candidate_id.bytes()
            || quote_admission.quote.facility.pre_generation != state.generation
            || quote_admission.quote.fee_policy_semantics_digest != policy.fee_policy_id.bytes()
            || quote_admission.outcome_count != policy.outcome_count
            || quote_semantics.bytes() != quote_admission.quote.semantic_quote_digest
            || final_candidate != Id::from_bytes(root.settlement_candidate_id().bytes())
            || Id::from_bytes(root.market_instance_v2_id().bytes()) != policy.market_instance_v2_id
            || Id::from_bytes(root.epoch().bytes()) != epoch.epoch_account_id
            || Id::from_bytes(root.retained_feed().bytes()) != context.retained_feed_account_id
            || Id::from_bytes(root.order_set().bytes()) == Id::ZERO
            || Id::from_bytes(root.candidate_bundle_digest().bytes())
                != context.candidate_bundle_digest
            || Id::from_bytes(root.settlement_witness_digest().bytes())
                != context.settlement_witness_digest
            || root.epoch_generation() != epoch.general_epoch_generation
            || root.selected_ordinal() == 0
            || root.outcome_count() != policy.outcome_count
            || dealer.outcome_count != policy.outcome_count
            || dealer.allocation_count == 0
            || Id::from_bytes(root.batch_policy_id().bytes()) != selected_fee.batch_policy_id
            || Id::from_bytes(root.score_policy_id().bytes()) == Id::ZERO
            || Id::from_bytes(root.fee_record().bytes()) != selected_fee.fee_record_account_id
            || selected_fee.settlement_candidate_id != final_candidate
            || selected_fee.market_instance_v2_id != policy.market_instance_v2_id
            || selected_fee.epoch_id != epoch.epoch_id
            || selected_fee.revenue_policy_id != policy.fee_policy_id
            || selected_fee.outcome_count != policy.outcome_count
            || Id::from_bytes(price_bindings.candidate_feed) != context.retained_feed_account_id
            || Id::from_bytes(price_bindings.relation_domain_digest) != epoch.economic_domain_id
            || Id::from_bytes(price_bindings.basis_digest) != policy.claim_basis_id
            || price.native_outcome_count() != policy.outcome_count
        {
            return Err(Error::MismatchedBinding);
        }
        let mut user_cash_in_atoms = 0u64;
        let mut user_cash_out_atoms = 0u64;
        let mut row = 0usize;
        while row < usize::from(dealer.allocation_count) {
            user_cash_in_atoms = user_cash_in_atoms
                .checked_add(dealer.allocations[row].user_cash_in_atoms)
                .ok_or(Error::ArithmeticOverflow)?;
            user_cash_out_atoms = user_cash_out_atoms
                .checked_add(dealer.allocations[row].user_cash_out_atoms)
                .ok_or(Error::ArithmeticOverflow)?;
            row += 1;
        }
        let receipt = if user_cash_in_atoms >= user_cash_out_atoms {
            DealerReceiptV2 {
                dealer_net_cash_in_atoms: user_cash_in_atoms - user_cash_out_atoms,
                dealer_net_cash_out_atoms: 0,
            }
        } else {
            DealerReceiptV2 {
                dealer_net_cash_in_atoms: 0,
                dealer_net_cash_out_atoms: user_cash_out_atoms - user_cash_in_atoms,
            }
        };
        if receipt != quote_admission.quote.receipt || dealer.trade != quote_admission.quote.trade {
            return Err(Error::ConservationFailure);
        }
        output.selection_account_id = context.selection_account_id;
        output.policy_id = policy_id;
        output.facility_id = state.facility_id;
        output.facility_position_binding_id = state.facility_position_binding_id;
        output.dealer_state_account_id = epoch.dealer_state_account_id;
        output.market_instance_v2_id = policy.market_instance_v2_id;
        output.epoch_id = epoch.epoch_id;
        output.epoch_binding_account_id = epoch.epoch_binding_account_id;
        output.settlement_root_account_id = context.settlement_root_account_id;
        output.retained_feed_account_id = context.retained_feed_account_id;
        output.order_set_id = Id::from_bytes(root.order_set().bytes());
        output.settlement_candidate_id = final_candidate;
        output.upstream_economic_candidate_id = context.upstream_economic_candidate_id;
        output.candidate_bundle_digest = context.candidate_bundle_digest;
        output.settlement_witness_digest = context.settlement_witness_digest;
        output.economic_domain_id = epoch.economic_domain_id;
        output.curve_price_certificate_id = price_body;
        output.quote_authority = policy.quote_authority;
        output.quote_admission_id = quote_admission.admission_id()?;
        output.quote_semantics_id = quote_semantics;
        output.batch_policy_id = selected_fee.batch_policy_id;
        output.score_policy_id = Id::from_bytes(root.score_policy_id().bytes());
        output.selected_fee_record_account_id = selected_fee.fee_record_account_id;
        output.selected_fee_record_semantic_id = selected_fee.fee_record_semantic_id;
        output.selected_fee_binding_digest = selected_fee_digest;
        output.fee_revenue_policy_id = selected_fee.revenue_policy_id;
        output.lease_account_id = context.lease_account_id;
        output.settlement_pot_account_id = context.settlement_pot_account_id;
        output.dealer_generation = state.generation;
        output.general_epoch_generation = epoch.general_epoch_generation;
        output.selected_ordinal = root.selected_ordinal();
        output.created_slot = context.current_slot;
        output.quote_expires_slot = quote_admission.expires_slot;
        output.collect_deadline_slot = quote_admission.collect_deadline_slot;
        output.deliver_deadline_slot = quote_admission.deliver_deadline_slot;
        output.receipt = receipt;
        output.trade = dealer.trade;
        output.allocations.copy_from_slice(&dealer.allocations);
        output.total_external_fee_atoms = dealer.total_external_fee_atoms;
        output.outcome_count = dealer.outcome_count;
        output.allocation_count = dealer.allocation_count;
        output.stored_bump = context.stored_bump;
        output.rent = context.rent;
        output.validate()
    }

    /// Validate the immutable body without claiming the creation-time
    /// signature, account, General traversal, or price checks were rerun.
    pub fn validate(&self) -> Result<()> {
        for identity in self.identities() {
            identity.validate_live()?;
        }
        self.rent.validate()?;
        if self.dealer_generation == 0
            || self.general_epoch_generation == 0
            || self.selected_ordinal == 0
            || self.created_slot == 0
            || self.created_slot >= self.quote_expires_slot
            || self.quote_expires_slot > self.collect_deadline_slot
            || self.collect_deadline_slot >= self.deliver_deadline_slot
            || self.outcome_count < 2
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.allocation_count == 0
            || usize::from(self.allocation_count) > MAX_DEALER_ROWS_V2
            || self.rent.neutral_sink == self.rent.payer
        {
            return Err(Error::InvalidParameter);
        }
        let mut external_fee_sum = 0u128;
        let mut user_cash_in_atoms = 0u64;
        let mut user_cash_out_atoms = 0u64;
        let count = usize::from(self.allocation_count);
        let mut row = 0usize;
        while row < MAX_DEALER_ROWS_V2 {
            let allocation = self.allocations[row];
            if row < count {
                if allocation.order_id == [0; 32]
                    || allocation.dealer_fill_units == 0
                    || allocation.user_cash_in_atoms != 0 && allocation.user_cash_out_atoms != 0
                    || row > 0 && self.allocations[row - 1].order_id >= allocation.order_id
                {
                    return Err(Error::InvalidParameter);
                }
                external_fee_sum = external_fee_sum
                    .checked_add(u128::from(allocation.external_fee_atoms))
                    .ok_or(Error::ArithmeticOverflow)?;
                user_cash_in_atoms = user_cash_in_atoms
                    .checked_add(allocation.user_cash_in_atoms)
                    .ok_or(Error::ArithmeticOverflow)?;
                user_cash_out_atoms = user_cash_out_atoms
                    .checked_add(allocation.user_cash_out_atoms)
                    .ok_or(Error::ArithmeticOverflow)?;
            } else if allocation != EMPTY_DEALER_CASH_ALLOCATION_V2 {
                return Err(Error::NonCanonicalPadding);
            }
            row += 1;
        }
        if external_fee_sum != self.total_external_fee_atoms {
            return Err(Error::ConservationFailure);
        }
        let expected_receipt = if user_cash_in_atoms >= user_cash_out_atoms {
            DealerReceiptV2 {
                dealer_net_cash_in_atoms: user_cash_in_atoms - user_cash_out_atoms,
                dealer_net_cash_out_atoms: 0,
            }
        } else {
            DealerReceiptV2 {
                dealer_net_cash_in_atoms: 0,
                dealer_net_cash_out_atoms: user_cash_out_atoms - user_cash_in_atoms,
            }
        };
        if expected_receipt != self.receipt {
            return Err(Error::ConservationFailure);
        }
        let mut outcome = usize::from(self.outcome_count);
        while outcome < MAX_OUTCOMES {
            if self.trade.sell_to_users[outcome] != 0 || self.trade.buy_from_users[outcome] != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            outcome += 1;
        }
        Ok(())
    }

    /// Canonical opaque attachment identity consumed by General and Lease.
    pub fn selection_id(&self) -> Result<Id> {
        self.validate()?;
        let mut hasher = Sha256::new();
        hasher.update(DEALER_COVERED_SELECTION_CONTENT_DOMAIN_V1);
        hash_covered_selection(self, &mut hasher);
        let value = Id::from_bytes(hasher.finalize().into());
        value.validate_live()?;
        Ok(value)
    }

    /// Join one canonical Dealer allocation to its authenticated current
    /// RelationV2 page membership.
    ///
    /// `row_index` is the immutable, order-id-sorted Dealer allocation cursor;
    /// it is deliberately not inferred from the dense book index.  Churned V5
    /// pages may leave sparse physical slots, while the complete-book adapter
    /// owns the exact dense RelationV2 projection.
    pub fn authenticate_settlement_row(
        &self,
        membership: &AuthenticatedSelectedPortfolioOrderV2,
        row_index: u16,
    ) -> Result<CoveredDealerSettlementRowV1> {
        self.validate()?;
        let at = usize::from(row_index);
        if at >= usize::from(self.allocation_count) {
            return Err(Error::InvalidParameter);
        }
        let record = membership.record();
        let order = membership.economic_order();
        let allocation = self.allocations[at];
        if record.outcome_count != self.outcome_count
            || record.settlement_root_account_id != self.settlement_root_account_id.bytes()
            || record.retained_feed_account_id != self.retained_feed_account_id.bytes()
            || record.retained_feed_semantic_id != self.candidate_bundle_digest.bytes()
            || record.order_set_digest != self.order_set_id.bytes()
            || record.settlement_candidate_id != self.settlement_candidate_id.bytes()
            || record.settlement_witness_id != self.settlement_witness_digest.bytes()
            || record.economic_candidate_digest != self.upstream_economic_candidate_id.bytes()
            || record.settlement_root_epoch_generation != self.general_epoch_generation
            || record.order_id != allocation.order_id
            || record.selected_fill_units != allocation.dealer_fill_units
            || order.order_id != allocation.order_id
            || order.side != record.side
            || order.quantity < allocation.dealer_fill_units
        {
            return Err(Error::MismatchedBinding);
        }
        match order.side {
            Side::Buy if allocation.user_cash_out_atoms != 0 => {
                return Err(Error::ConservationFailure);
            }
            Side::Sell if allocation.user_cash_in_atoms != 0 => {
                return Err(Error::ConservationFailure);
            }
            Side::Buy | Side::Sell => {}
        }
        let mut native_eggs = [0u64; MAX_OUTCOMES];
        let mut outcome = 0usize;
        while outcome < usize::from(self.outcome_count) {
            native_eggs[outcome] = order.coefficients[outcome]
                .checked_mul(allocation.dealer_fill_units)
                .ok_or(Error::ArithmeticOverflow)?;
            outcome += 1;
        }
        Ok(CoveredDealerSettlementRowV1 {
            selection_id: self.selection_id()?,
            selected_fee_binding_digest: self.selected_fee_binding_digest,
            row_index,
            order_index: record.order_index,
            order_id: Id::from_bytes(record.order_id),
            owner_id: Id::from_bytes(record.owner_id),
            position_account_id: Id::from_bytes(record.position_account_id),
            position_generation: record.position_generation,
            side: record.side,
            fill_units: allocation.dealer_fill_units,
            cash_in_atoms: allocation.user_cash_in_atoms,
            cash_out_atoms: allocation.user_cash_out_atoms,
            native_eggs,
        })
    }

    /// Require one Lease/Pot pair to be the exact executable projection of
    /// this authenticated, persisted selection owner.
    pub fn validate_lease_pot(
        &self,
        lease: &DealerLeaseV2,
        pot: &SettlementPotV2,
        epoch: &DealerEpochBindingV2,
        policy: &DealerPolicyV1,
    ) -> Result<()> {
        self.validate()?;
        lease.validate()?;
        pot.validate_against_lease(lease)?;
        epoch.validate()?;
        policy.validate()?;
        let mut user_cash_in_atoms = 0u64;
        let mut user_cash_out_atoms = 0u64;
        let mut row = 0usize;
        while row < usize::from(self.allocation_count) {
            user_cash_in_atoms = user_cash_in_atoms
                .checked_add(self.allocations[row].user_cash_in_atoms)
                .ok_or(Error::ArithmeticOverflow)?;
            user_cash_out_atoms = user_cash_out_atoms
                .checked_add(self.allocations[row].user_cash_out_atoms)
                .ok_or(Error::ArithmeticOverflow)?;
            row += 1;
        }
        if lease.policy_id != self.policy_id
            || lease.facility_id != self.facility_id
            || lease.facility_position_binding_id != self.facility_position_binding_id
            || lease.dealer_state_account_id != self.dealer_state_account_id
            || lease.market_instance_v2_id != self.market_instance_v2_id
            || lease.epoch_id != self.epoch_id
            || lease.epoch_binding_account_id != self.epoch_binding_account_id
            || lease.settlement_candidate_id != self.settlement_candidate_id
            || lease.settlement_root_account_id != self.settlement_root_account_id
            || lease.covered_dealer_selection_account_id != self.selection_account_id
            || lease.covered_dealer_selection_id != self.selection_id()?
            || lease.upstream_economic_candidate_id != self.upstream_economic_candidate_id
            || lease.quote_id != self.quote_semantics_id
            || lease.dealer_leg_verdict_id != self.settlement_candidate_id
            || lease.curve_price_certificate_id != self.curve_price_certificate_id
            || lease.settlement_rows_root != self.settlement_witness_digest
            || lease.settlement_pot_id != self.settlement_pot_account_id
            || lease.selected_fee_binding_digest != self.selected_fee_binding_digest
            || lease.selected_fee_record_account_id != self.selected_fee_record_account_id
            || lease.selected_fee_record_semantic_id != self.selected_fee_record_semantic_id
            || lease.fee_revenue_policy_id != self.fee_revenue_policy_id
            || lease.pre_generation != self.dealer_generation
            || lease.created_slot != self.created_slot
            || lease.collect_deadline_slot != self.collect_deadline_slot
            || lease.deliver_deadline_slot != self.deliver_deadline_slot
            || lease.outcome_count != self.outcome_count
            || lease.row_count != u16::from(self.allocation_count)
            || epoch.epoch_binding_account_id != self.epoch_binding_account_id
            || epoch.epoch_id != self.epoch_id
            || epoch.general_epoch_generation != self.general_epoch_generation
            || policy.policy_id()? != self.policy_id
            || policy.market_instance_v2_id != self.market_instance_v2_id
            || pot.user_cash_in_atoms != user_cash_in_atoms
            || pot.user_cash_out_atoms != user_cash_out_atoms
            || pot.dealer_net_cash_in_atoms != self.receipt.dealer_net_cash_in_atoms
            || pot.dealer_net_cash_out_atoms != self.receipt.dealer_net_cash_out_atoms
            || pot.facility_buy_eggs != self.trade.buy_from_users
            || pot.facility_sell_eggs != self.trade.sell_to_users
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    fn identities(&self) -> [Id; 28] {
        [
            self.selection_account_id,
            self.policy_id,
            self.facility_id,
            self.facility_position_binding_id,
            self.dealer_state_account_id,
            self.market_instance_v2_id,
            self.epoch_id,
            self.epoch_binding_account_id,
            self.settlement_root_account_id,
            self.retained_feed_account_id,
            self.order_set_id,
            self.settlement_candidate_id,
            self.upstream_economic_candidate_id,
            self.candidate_bundle_digest,
            self.settlement_witness_digest,
            self.economic_domain_id,
            self.curve_price_certificate_id,
            self.quote_authority,
            self.quote_admission_id,
            self.quote_semantics_id,
            self.batch_policy_id,
            self.score_policy_id,
            self.selected_fee_record_account_id,
            self.selected_fee_record_semantic_id,
            self.selected_fee_binding_digest,
            self.fee_revenue_policy_id,
            self.lease_account_id,
            self.settlement_pot_account_id,
        ]
    }
}

impl FixedCodec for CoveredDealerSelectionV1 {
    const ENCODED_LEN: usize = DEALER_COVERED_SELECTION_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_COVERED_SELECTION_MAGIC_V1,
            DEALER_COVERED_SELECTION_VERSION_V1,
        );
        write_covered_selection(self, &mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_COVERED_SELECTION_MAGIC_V1,
            DEALER_COVERED_SELECTION_VERSION_V1,
        )?;
        let value = read_covered_selection(&mut reader)?;
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

fn write_quote_admission(value: &DealerQuoteAdmissionV1, writer: &mut Writer<'_>) {
    for identity in [
        value.quote_authority,
        value.policy_id,
        value.facility_id,
        value.dealer_state_account_id,
        value.epoch_id,
        value.settlement_root_account_id,
        value.retained_feed_account_id,
        value.settlement_candidate_id,
        value.selected_fee_binding_digest,
    ] {
        writer.id(identity);
    }
    writer.u64(value.issued_slot);
    writer.u64(value.expires_slot);
    writer.u64(value.collect_deadline_slot);
    writer.u64(value.deliver_deadline_slot);
    writer.u8(value.outcome_count);
    writer.reserved(7);
    write_dealer_candidate(&value.dealer, writer);
    write_dealer_quote(&value.quote, writer);
}

fn read_quote_admission_into(
    reader: &mut Reader<'_>,
    value: &mut DealerQuoteAdmissionV1,
) -> Result<()> {
    value.quote_authority = reader.id();
    value.policy_id = reader.id();
    value.facility_id = reader.id();
    value.dealer_state_account_id = reader.id();
    value.epoch_id = reader.id();
    value.settlement_root_account_id = reader.id();
    value.retained_feed_account_id = reader.id();
    value.settlement_candidate_id = reader.id();
    value.selected_fee_binding_digest = reader.id();
    value.issued_slot = reader.u64();
    value.expires_slot = reader.u64();
    value.collect_deadline_slot = reader.u64();
    value.deliver_deadline_slot = reader.u64();
    value.outcome_count = reader.u8();
    reader.reserved(7)?;
    read_dealer_candidate_into(reader, &mut value.dealer)?;
    read_dealer_quote_into(reader, &mut value.quote)
}

fn write_dealer_candidate(value: &DealerLegCandidateV2, writer: &mut Writer<'_>) {
    let mut row = 0usize;
    while row < MAX_DEALER_ROWS_V2 {
        writer.bytes(&value.rows[row].order_id);
        writer.u64(value.rows[row].dealer_fill_units);
        row += 1;
    }
    writer.u8(value.row_count);
    writer.reserved(7);
}

fn read_dealer_candidate_into(
    reader: &mut Reader<'_>,
    value: &mut DealerLegCandidateV2,
) -> Result<()> {
    let mut row = 0usize;
    while row < MAX_DEALER_ROWS_V2 {
        value.rows[row] = DealerFillRowV2 {
            order_id: reader.bytes(),
            dealer_fill_units: reader.u64(),
        };
        row += 1;
    }
    value.row_count = reader.u8();
    reader.reserved(7)
}

fn write_dealer_quote(value: &DealerQuotePreconditionV2, writer: &mut Writer<'_>) {
    writer.bytes(&value.upstream_economic_candidate_digest);
    writer.u8(value.facility.version);
    writer.reserved(7);
    writer.bytes(&value.facility.facility_semantics_digest);
    writer.bytes(&value.facility.policy_semantics_digest);
    writer.u64(value.facility.pre_generation);
    writer.u8(cash_policy_byte(value.cash_policy));
    writer.reserved(7);
    writer.bytes(&value.fee_policy_semantics_digest);
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        writer.u64(value.trade.sell_to_users[outcome]);
        outcome += 1;
    }
    outcome = 0;
    while outcome < MAX_OUTCOMES {
        writer.u64(value.trade.buy_from_users[outcome]);
        outcome += 1;
    }
    writer.u64(value.receipt.dealer_net_cash_in_atoms);
    writer.u64(value.receipt.dealer_net_cash_out_atoms);
    let mut row = 0usize;
    while row < MAX_DEALER_ROWS_V2 {
        let value = value.rows[row];
        writer.bytes(&value.order_id);
        writer.u64(value.maximum_cash_in_atoms);
        writer.u64(value.minimum_cash_out_atoms);
        writer.u64(value.external_fee_atoms);
        row += 1;
    }
    writer.bytes(&value.semantic_quote_digest);
}

fn read_dealer_quote_into(
    reader: &mut Reader<'_>,
    value: &mut DealerQuotePreconditionV2,
) -> Result<()> {
    value.upstream_economic_candidate_digest = reader.bytes();
    let version = reader.u8();
    reader.reserved(7)?;
    value.facility = DealerFacilityBindingV2 {
        version,
        facility_semantics_digest: reader.bytes(),
        policy_semantics_digest: reader.bytes(),
        pre_generation: reader.u64(),
    };
    value.cash_policy = match reader.u8() {
        1 => DealerCashPolicyV2::MinimumGrossHamiltonV1,
        _ => return Err(Error::InvalidParameter),
    };
    reader.reserved(7)?;
    value.fee_policy_semantics_digest = reader.bytes();
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        value.trade.sell_to_users[outcome] = reader.u64();
        outcome += 1;
    }
    outcome = 0;
    while outcome < MAX_OUTCOMES {
        value.trade.buy_from_users[outcome] = reader.u64();
        outcome += 1;
    }
    value.receipt = DealerReceiptV2 {
        dealer_net_cash_in_atoms: reader.u64(),
        dealer_net_cash_out_atoms: reader.u64(),
    };
    let mut row = 0usize;
    while row < MAX_DEALER_ROWS_V2 {
        value.rows[row] = DealerQuoteRowV2 {
            order_id: reader.bytes(),
            maximum_cash_in_atoms: reader.u64(),
            minimum_cash_out_atoms: reader.u64(),
            external_fee_atoms: reader.u64(),
        };
        row += 1;
    }
    value.semantic_quote_digest = reader.bytes();
    Ok(())
}

fn write_covered_selection(value: &CoveredDealerSelectionV1, writer: &mut Writer<'_>) {
    for identity in value.identities() {
        writer.id(identity);
    }
    writer.u64(value.dealer_generation);
    writer.u64(value.general_epoch_generation);
    writer.u64(value.selected_ordinal);
    writer.u64(value.created_slot);
    writer.u64(value.quote_expires_slot);
    writer.u64(value.collect_deadline_slot);
    writer.u64(value.deliver_deadline_slot);
    writer.u64(value.receipt.dealer_net_cash_in_atoms);
    writer.u64(value.receipt.dealer_net_cash_out_atoms);
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        writer.u64(value.trade.sell_to_users[outcome]);
        outcome += 1;
    }
    outcome = 0;
    while outcome < MAX_OUTCOMES {
        writer.u64(value.trade.buy_from_users[outcome]);
        outcome += 1;
    }
    let mut row = 0usize;
    while row < MAX_DEALER_ROWS_V2 {
        let value = value.allocations[row];
        writer.bytes(&value.order_id);
        writer.u64(value.dealer_fill_units);
        writer.u64(value.user_cash_in_atoms);
        writer.u64(value.user_cash_out_atoms);
        writer.u64(value.external_fee_atoms);
        row += 1;
    }
    writer.u128(value.total_external_fee_atoms);
    writer.u8(value.outcome_count);
    writer.u8(value.allocation_count);
    writer.u8(value.stored_bump);
    writer.reserved(5);
    value.rent.encode_body(writer);
}

fn read_covered_selection(reader: &mut Reader<'_>) -> Result<CoveredDealerSelectionV1> {
    let mut identities = [Id::ZERO; 28];
    let mut identity = 0usize;
    while identity < identities.len() {
        identities[identity] = reader.id();
        identity += 1;
    }
    let dealer_generation = reader.u64();
    let general_epoch_generation = reader.u64();
    let selected_ordinal = reader.u64();
    let created_slot = reader.u64();
    let quote_expires_slot = reader.u64();
    let collect_deadline_slot = reader.u64();
    let deliver_deadline_slot = reader.u64();
    let receipt = DealerReceiptV2 {
        dealer_net_cash_in_atoms: reader.u64(),
        dealer_net_cash_out_atoms: reader.u64(),
    };
    let mut trade = AggregateDealerTradeV2 {
        sell_to_users: [0; MAX_OUTCOMES],
        buy_from_users: [0; MAX_OUTCOMES],
    };
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        trade.sell_to_users[outcome] = reader.u64();
        outcome += 1;
    }
    outcome = 0;
    while outcome < MAX_OUTCOMES {
        trade.buy_from_users[outcome] = reader.u64();
        outcome += 1;
    }
    let mut allocations = [EMPTY_DEALER_CASH_ALLOCATION_V2; MAX_DEALER_ROWS_V2];
    let mut row = 0usize;
    while row < MAX_DEALER_ROWS_V2 {
        allocations[row] = DealerCashAllocationV2 {
            order_id: reader.bytes(),
            dealer_fill_units: reader.u64(),
            user_cash_in_atoms: reader.u64(),
            user_cash_out_atoms: reader.u64(),
            external_fee_atoms: reader.u64(),
        };
        row += 1;
    }
    let total_external_fee_atoms = reader.u128();
    let outcome_count = reader.u8();
    let allocation_count = reader.u8();
    let stored_bump = reader.u8();
    reader.reserved(5)?;
    Ok(CoveredDealerSelectionV1 {
        selection_account_id: identities[0],
        policy_id: identities[1],
        facility_id: identities[2],
        facility_position_binding_id: identities[3],
        dealer_state_account_id: identities[4],
        market_instance_v2_id: identities[5],
        epoch_id: identities[6],
        epoch_binding_account_id: identities[7],
        settlement_root_account_id: identities[8],
        retained_feed_account_id: identities[9],
        order_set_id: identities[10],
        settlement_candidate_id: identities[11],
        upstream_economic_candidate_id: identities[12],
        candidate_bundle_digest: identities[13],
        settlement_witness_digest: identities[14],
        economic_domain_id: identities[15],
        curve_price_certificate_id: identities[16],
        quote_authority: identities[17],
        quote_admission_id: identities[18],
        quote_semantics_id: identities[19],
        batch_policy_id: identities[20],
        score_policy_id: identities[21],
        selected_fee_record_account_id: identities[22],
        selected_fee_record_semantic_id: identities[23],
        selected_fee_binding_digest: identities[24],
        fee_revenue_policy_id: identities[25],
        lease_account_id: identities[26],
        settlement_pot_account_id: identities[27],
        dealer_generation,
        general_epoch_generation,
        selected_ordinal,
        created_slot,
        quote_expires_slot,
        collect_deadline_slot,
        deliver_deadline_slot,
        receipt,
        trade,
        allocations,
        total_external_fee_atoms,
        outcome_count,
        allocation_count,
        stored_bump,
        rent: DeletableRentOwnerV1::decode_body(reader),
    })
}

fn hash_quote_admission(value: &DealerQuoteAdmissionV1, hasher: &mut Sha256) {
    let mut bytes = [0u8; 12];
    bytes[..8].copy_from_slice(&DEALER_QUOTE_ADMISSION_MAGIC_V1);
    bytes[8..10].copy_from_slice(&DEALER_QUOTE_ADMISSION_VERSION_V1.to_le_bytes());
    hasher.update(bytes);
    for identity in [
        value.quote_authority,
        value.policy_id,
        value.facility_id,
        value.dealer_state_account_id,
        value.epoch_id,
        value.settlement_root_account_id,
        value.retained_feed_account_id,
        value.settlement_candidate_id,
        value.selected_fee_binding_digest,
    ] {
        hasher.update(identity.bytes());
    }
    hasher.update(value.issued_slot.to_le_bytes());
    hasher.update(value.expires_slot.to_le_bytes());
    hasher.update(value.collect_deadline_slot.to_le_bytes());
    hasher.update(value.deliver_deadline_slot.to_le_bytes());
    hasher.update([value.outcome_count]);
    hasher.update([0; 7]);
    hash_dealer_candidate(&value.dealer, hasher);
    hash_dealer_quote(&value.quote, hasher);
}

fn hash_dealer_candidate(value: &DealerLegCandidateV2, hasher: &mut Sha256) {
    let mut row = 0usize;
    while row < MAX_DEALER_ROWS_V2 {
        hasher.update(value.rows[row].order_id);
        hasher.update(value.rows[row].dealer_fill_units.to_le_bytes());
        row += 1;
    }
    hasher.update([value.row_count]);
    hasher.update([0; 7]);
}

fn hash_dealer_quote(value: &DealerQuotePreconditionV2, hasher: &mut Sha256) {
    hasher.update(value.upstream_economic_candidate_digest);
    hasher.update([value.facility.version]);
    hasher.update([0; 7]);
    hasher.update(value.facility.facility_semantics_digest);
    hasher.update(value.facility.policy_semantics_digest);
    hasher.update(value.facility.pre_generation.to_le_bytes());
    hasher.update([cash_policy_byte(value.cash_policy)]);
    hasher.update([0; 7]);
    hasher.update(value.fee_policy_semantics_digest);
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        hasher.update(value.trade.sell_to_users[outcome].to_le_bytes());
        outcome += 1;
    }
    outcome = 0;
    while outcome < MAX_OUTCOMES {
        hasher.update(value.trade.buy_from_users[outcome].to_le_bytes());
        outcome += 1;
    }
    hasher.update(value.receipt.dealer_net_cash_in_atoms.to_le_bytes());
    hasher.update(value.receipt.dealer_net_cash_out_atoms.to_le_bytes());
    let mut row = 0usize;
    while row < MAX_DEALER_ROWS_V2 {
        let value = value.rows[row];
        hasher.update(value.order_id);
        hasher.update(value.maximum_cash_in_atoms.to_le_bytes());
        hasher.update(value.minimum_cash_out_atoms.to_le_bytes());
        hasher.update(value.external_fee_atoms.to_le_bytes());
        row += 1;
    }
    hasher.update(value.semantic_quote_digest);
}

const fn cash_policy_byte(value: DealerCashPolicyV2) -> u8 {
    match value {
        DealerCashPolicyV2::MinimumGrossHamiltonV1 => 1,
    }
}

fn hash_covered_selection(value: &CoveredDealerSelectionV1, hasher: &mut Sha256) {
    let mut bytes = [0u8; 12];
    bytes[..8].copy_from_slice(&DEALER_COVERED_SELECTION_MAGIC_V1);
    bytes[8..10].copy_from_slice(&DEALER_COVERED_SELECTION_VERSION_V1.to_le_bytes());
    hasher.update(bytes);
    for identity in value.identities() {
        hasher.update(identity.bytes());
    }
    for number in [
        value.dealer_generation,
        value.general_epoch_generation,
        value.selected_ordinal,
        value.created_slot,
        value.quote_expires_slot,
        value.collect_deadline_slot,
        value.deliver_deadline_slot,
        value.receipt.dealer_net_cash_in_atoms,
        value.receipt.dealer_net_cash_out_atoms,
    ] {
        hasher.update(number.to_le_bytes());
    }
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        hasher.update(value.trade.sell_to_users[outcome].to_le_bytes());
        outcome += 1;
    }
    outcome = 0;
    while outcome < MAX_OUTCOMES {
        hasher.update(value.trade.buy_from_users[outcome].to_le_bytes());
        outcome += 1;
    }
    let mut row = 0usize;
    while row < MAX_DEALER_ROWS_V2 {
        let value = value.allocations[row];
        hasher.update(value.order_id);
        hasher.update(value.dealer_fill_units.to_le_bytes());
        hasher.update(value.user_cash_in_atoms.to_le_bytes());
        hasher.update(value.user_cash_out_atoms.to_le_bytes());
        hasher.update(value.external_fee_atoms.to_le_bytes());
        row += 1;
    }
    hasher.update(value.total_external_fee_atoms.to_le_bytes());
    hasher.update([value.outcome_count, value.allocation_count, value.stored_bump]);
    hasher.update([0; 5]);
    hasher.update(value.rent.payer.bytes());
    hasher.update(value.rent.neutral_sink.bytes());
    hasher.update(value.rent.refundable_principal.to_le_bytes());
    hasher.update(value.rent.donation_floor.to_le_bytes());
}

#[cfg(test)]
mod covered_row_adversarial_tests {
    use super::*;

    fn id(byte: u8) -> Id {
        Id::from_bytes([byte; 32])
    }

    fn row(side: Side) -> CoveredDealerSettlementRowV1 {
        let mut native_eggs = [0u64; MAX_OUTCOMES];
        native_eggs[0] = 6;
        native_eggs[1] = 9;
        CoveredDealerSettlementRowV1 {
            selection_id: id(1),
            selected_fee_binding_digest: id(2),
            row_index: 3,
            order_index: 5,
            order_id: id(3),
            owner_id: id(4),
            position_account_id: id(5),
            position_generation: 7,
            side,
            fill_units: 3,
            cash_in_atoms: if side == Side::Buy { 11 } else { 0 },
            cash_out_atoms: if side == Side::Sell { 13 } else { 0 },
            native_eggs,
        }
    }

    fn transition(action: DealerRuntimeActionV1, row: CoveredDealerSettlementRowV1) -> Id {
        CoveredDealerRowAssetTransitionV1::new(
            action,
            row,
            id(10),
            id(11),
            id(12),
            id(13),
            id(14),
            id(15),
            id(16),
            id(17),
            id(18),
        )
        .unwrap()
        .bundle_id()
    }

    #[test]
    fn buy_and_sell_slices_have_exact_opposite_asset_boundaries() {
        let buy = row(Side::Buy);
        assert_eq!(buy.collect_slice().cash_atoms, 11);
        assert_eq!(buy.collect_slice().eggs, [0; MAX_OUTCOMES]);
        assert_eq!(buy.deliver_slice().cash_atoms, 0);
        assert_eq!(buy.deliver_slice().eggs[0..2], [6, 9]);

        let sell = row(Side::Sell);
        assert_eq!(sell.collect_slice().cash_atoms, 0);
        assert_eq!(sell.collect_slice().eggs[0..2], [6, 9]);
        assert_eq!(sell.deliver_slice().cash_atoms, 13);
        assert_eq!(sell.deliver_slice().eggs, [0; MAX_OUTCOMES]);
    }

    #[test]
    fn action_side_and_every_postimage_identity_change_the_bundle() {
        let buy = row(Side::Buy);
        let collect = transition(DealerRuntimeActionV1::Collect, buy);
        let deliver = transition(DealerRuntimeActionV1::Deliver, buy);
        let sell = transition(DealerRuntimeActionV1::Collect, row(Side::Sell));
        assert_ne!(collect, deliver);
        assert_ne!(collect, sell);
        let changed = CoveredDealerRowAssetTransitionV1::new(
            DealerRuntimeActionV1::Collect,
            buy,
            id(10),
            id(11),
            id(12),
            id(13),
            id(14),
            id(15),
            id(16),
            id(17),
            id(19),
        )
        .unwrap()
        .bundle_id();
        assert_ne!(collect, changed);
    }

    #[test]
    fn unchanged_reservation_replay_or_pot_postimage_refuses() {
        let buy = row(Side::Buy);
        assert!(CoveredDealerRowAssetTransitionV1::new(
            DealerRuntimeActionV1::Collect,
            buy,
            id(10),
            id(11),
            id(11),
            id(13),
            id(14),
            id(15),
            id(16),
            id(17),
            id(18),
        )
        .is_err());
    }
}

const _: () = assert!(DEALER_QUOTE_ADMISSION_BYTES_V1 == 6_948);
const _: () = assert!(DEALER_COVERED_SELECTION_BYTES_V1 == 5_436);
const _: () = assert!(DEALER_COVERED_SELECTION_BYTES_V1 < 10_240);
