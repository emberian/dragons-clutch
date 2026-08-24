// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dealer State successor that counts the facility-lifetime Product obligation.
//!
//! Every live facility is persisted as `DealerStateV3`. Before the first
//! CoveredDealer admission the Product-obligation partition is canonically
//! empty; the atomic SelectLeaseAndBegin transition fills that partition while
//! creating one `DealerSeriesObligationBindingV3`. Per-Lease settlement mutates
//! only the embedded economic state; the Product obligation survives until
//! facility retirement consumes and closes it.

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    DealerPhaseV2, DealerSeriesObligationBindingV1, DealerSeriesObligationBindingV2,
    DealerSeriesObligationBindingV3,
    DealerSeriesObligationPhaseV1,
    DealerStateV2, DeletableRentOwnerV1, Error, FixedCodec, Id, Result, DEALER_STATE_BYTES_V2,
    DEALER_STATE_CONTENT_DOMAIN_V3, DELETABLE_RENT_OWNER_BYTES,
};

/// Exact local semantic-body magic.
pub const DEALER_STATE_MAGIC_V3: [u8; 8] = *b"DCDSTAT3";
/// Exact local semantic-body version.
pub const DEALER_STATE_VERSION_V3: u16 = 3;
/// Exact canonical body bytes.
pub const DEALER_STATE_BYTES_V3: usize =
    HEADER_BYTES + DEALER_STATE_BYTES_V2 + (2 * 32) + 8 + DELETABLE_RENT_OWNER_BYTES;

/// Sole authoritative Dealer root before, during, and after Product admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerStateV3 {
    /// Exact V2 economic/child state, retained as its sole semantic owner.
    pub base: DealerStateV2,
    /// Last authenticated content identity of the facility Product binding.
    pub series_obligation_binding_id: Id,
    /// Live counted binding account, or zero after exact terminal close.
    pub series_obligation_binding_account_id: Id,
    /// Exhaustive live child count; exactly zero or one.
    pub series_obligation_children: u32,
    /// Separately owned principal for the V2-to-V3 State-account rent delta.
    ///
    /// This cannot be merged into `base.rent`: it is an independently measured
    /// V3-over-V2 principal even when the same founder pays both compartments.
    pub product_upgrade_rent: DeletableRentOwnerV1,
}

impl DealerStateV3 {
    /// Create the sole persisted Dealer root before its first Product
    /// obligation is admitted.
    ///
    /// `product_upgrade_rent` owns the exact V3-over-V2 rent delta from the
    /// founding instruction. Keeping the empty obligation partition inside V3
    /// removes the old caller-selectable `0xaf/v2` authority without inventing
    /// a synthetic Product liability before the facility can trade.
    pub fn founding_unadmitted(
        base: DealerStateV2,
        product_upgrade_rent: DeletableRentOwnerV1,
    ) -> Result<Self> {
        base.validate()?;
        product_upgrade_rent.validate()?;
        if product_upgrade_rent.neutral_sink != base.rent.neutral_sink
            || !matches!(
                base.phase,
                DealerPhaseV2::Funding
                    | DealerPhaseV2::Cancelled
                    | DealerPhaseV2::Trading
                    | DealerPhaseV2::UnwindOnly
            )
        {
            return Err(Error::MismatchedBinding);
        }
        let value = Self {
            base,
            series_obligation_binding_id: Id::ZERO,
            series_obligation_binding_account_id: Id::ZERO,
            series_obligation_children: 0,
            product_upgrade_rent,
        };
        value.validate()?;
        Ok(value)
    }

    /// Admit the first Product obligation into an already-authoritative V3
    /// root after the first lease transition has produced its exact poststate.
    pub fn admit_product_v3(
        self,
        base_after: DealerStateV2,
        binding: &DealerSeriesObligationBindingV3,
    ) -> Result<Self> {
        self.validate()?;
        base_after.validate()?;
        binding.validate()?;
        if self.series_obligation_children != 0
            || !self.series_obligation_binding_id.is_zero()
            || !self.series_obligation_binding_account_id.is_zero()
            || self.base.policy_id != base_after.policy_id
            || self.base.facility_id != base_after.facility_id
            || self.base.facility_position_binding_id != base_after.facility_position_binding_id
            || binding.phase != DealerSeriesObligationPhaseV1::Live
            || binding.key.policy_id != base_after.policy_id
            || binding.key.facility_id != base_after.facility_id
            || binding.key.facility_position_binding_id
                != base_after.facility_position_binding_id
            || base_after.children.leases != 1
            || base_after.children.settlement_pots != 1
            || base_after.active_lease_id.is_zero()
        {
            return Err(Error::MismatchedBinding);
        }
        let next = Self {
            base: base_after,
            series_obligation_binding_id: binding.binding_id()?,
            series_obligation_binding_account_id: binding.key.binding_account_id,
            series_obligation_children: 1,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Whether the exact one-child Product obligation partition is live.
    pub const fn product_obligation_is_live(&self) -> bool {
        self.series_obligation_children == 1
    }

    /// Promote the exact generic Product RootV3/LinkV3 admission post-state.
    pub fn promote_product_v3(
        base: DealerStateV2,
        binding: &DealerSeriesObligationBindingV3,
        product_upgrade_rent: DeletableRentOwnerV1,
    ) -> Result<Self> {
        base.validate()?;
        binding.validate()?;
        product_upgrade_rent.validate()?;
        if binding.phase != DealerSeriesObligationPhaseV1::Live
            || binding.key.policy_id != base.policy_id
            || binding.key.facility_id != base.facility_id
            || binding.key.facility_position_binding_id != base.facility_position_binding_id
            || base.children.leases != 1
            || base.children.settlement_pots != 1
            || base.active_lease_id.is_zero()
            || product_upgrade_rent.neutral_sink != base.rent.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        let value = Self {
            base,
            series_obligation_binding_id: binding.binding_id()?,
            series_obligation_binding_account_id: binding.key.binding_account_id,
            series_obligation_children: 1,
            product_upgrade_rent,
        };
        value.validate()?;
        Ok(value)
    }

    /// Promote the exact current Product RootV2/LinkV2 admission post-state.
    pub fn promote_current(
        base: DealerStateV2,
        binding: &DealerSeriesObligationBindingV2,
        product_upgrade_rent: DeletableRentOwnerV1,
    ) -> Result<Self> {
        base.validate()?;
        binding.validate()?;
        product_upgrade_rent.validate()?;
        if binding.phase != DealerSeriesObligationPhaseV1::Live
            || binding.key.policy_id != base.policy_id
            || binding.key.facility_id != base.facility_id
            || binding.key.facility_position_binding_id != base.facility_position_binding_id
            || base.children.leases != 1
            || base.children.settlement_pots != 1
            || base.active_lease_id.is_zero()
            || product_upgrade_rent.neutral_sink != base.rent.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        let value = Self {
            base,
            series_obligation_binding_id: binding.binding_id()?,
            series_obligation_binding_account_id: binding.key.binding_account_id,
            series_obligation_children: 1,
            product_upgrade_rent,
        };
        value.validate()?;
        Ok(value)
    }

    /// Promote the exact SelectLeaseAndBegin post-state while admitting one
    /// authenticated facility-lifetime Product obligation child.
    pub fn promote(
        base: DealerStateV2,
        binding: &DealerSeriesObligationBindingV1,
        product_upgrade_rent: DeletableRentOwnerV1,
    ) -> Result<Self> {
        base.validate()?;
        binding.validate()?;
        product_upgrade_rent.validate()?;
        if binding.phase != DealerSeriesObligationPhaseV1::Live
            || binding.key.policy_id != base.policy_id
            || binding.key.facility_id != base.facility_id
            || binding.key.facility_position_binding_id != base.facility_position_binding_id
            || base.children.leases != 1
            || base.children.settlement_pots != 1
            || base.active_lease_id.is_zero()
            || product_upgrade_rent.neutral_sink != base.rent.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        let value = Self {
            base,
            series_obligation_binding_id: binding.binding_id()?,
            series_obligation_binding_account_id: binding.key.binding_account_id,
            series_obligation_children: 1,
            product_upgrade_rent,
        };
        value.validate()?;
        Ok(value)
    }

    /// Replace only the V2-owned state after an exact Dealer transition.
    pub fn with_base(self, base: DealerStateV2) -> Result<Self> {
        let next = Self { base, ..self };
        next.validate()?;
        Ok(next)
    }

    /// Commit the authenticated terminal Product postwrite without closing
    /// the still-counted rent-owned binding account.
    pub fn observe_terminal_binding(
        self,
        binding: &DealerSeriesObligationBindingV1,
    ) -> Result<Self> {
        binding.validate()?;
        if self.series_obligation_children != 1
            || binding.phase != DealerSeriesObligationPhaseV1::Terminal
            || binding.key.binding_account_id != self.series_obligation_binding_account_id
            || binding.key.policy_id != self.base.policy_id
            || binding.key.facility_id != self.base.facility_id
            || binding.key.facility_position_binding_id
                != self.base.facility_position_binding_id
            || self.base.phase != DealerPhaseV2::Retiring
            || binding.terminal_state_receipt_id != self.base.terminal_state_receipt_id
        {
            return Err(Error::MismatchedBinding);
        }
        let next = Self {
            series_obligation_binding_id: binding.binding_id()?,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Commit the current Product LinkV2 terminal postwrite while retaining
    /// the counted rent-owned binding account.
    pub fn observe_current_terminal_binding(
        self,
        binding: &DealerSeriesObligationBindingV2,
    ) -> Result<Self> {
        binding.validate()?;
        if self.series_obligation_children != 1
            || binding.phase != DealerSeriesObligationPhaseV1::Terminal
            || binding.key.binding_account_id != self.series_obligation_binding_account_id
            || binding.key.policy_id != self.base.policy_id
            || binding.key.facility_id != self.base.facility_id
            || binding.key.facility_position_binding_id
                != self.base.facility_position_binding_id
            || self.base.phase != DealerPhaseV2::Retiring
            || binding.terminal_state_receipt_id != self.base.terminal_state_receipt_id
        {
            return Err(Error::MismatchedBinding);
        }
        let next = Self {
            series_obligation_binding_id: binding.binding_id()?,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Close the exact terminal binding child after Product has persisted its
    /// terminal link postwrite and rent credits are atomically materialized.
    pub fn close_terminal_binding(
        self,
        binding: &DealerSeriesObligationBindingV1,
    ) -> Result<Self> {
        binding.validate()?;
        if self.series_obligation_children != 1
            || binding.phase != DealerSeriesObligationPhaseV1::Terminal
            || binding.key.binding_account_id != self.series_obligation_binding_account_id
            || binding.binding_id()? != self.series_obligation_binding_id
        {
            return Err(Error::MismatchedBinding);
        }
        let next = Self {
            series_obligation_binding_account_id: Id::ZERO,
            series_obligation_children: 0,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Close the exact current Product LinkV2 terminal binding child.
    pub fn close_current_terminal_binding(
        self,
        binding: &DealerSeriesObligationBindingV2,
    ) -> Result<Self> {
        binding.validate()?;
        if self.series_obligation_children != 1
            || binding.phase != DealerSeriesObligationPhaseV1::Terminal
            || binding.key.binding_account_id != self.series_obligation_binding_account_id
            || binding.binding_id()? != self.series_obligation_binding_id
        {
            return Err(Error::MismatchedBinding);
        }
        let next = Self {
            series_obligation_binding_account_id: Id::ZERO,
            series_obligation_children: 0,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Close the exact live RootV3/LinkV3 binding only after Dealer has physically
    /// terminalized its Position/Replay and the disjoint value owner.
    ///
    /// Product does not authorize this local child deletion. The adapter must
    /// retain the resulting non-Copy Dealer family receipt and pass it directly
    /// to Product's successor terminal writer in the same rollback domain.
    pub fn close_product_v3_live_binding_after_value(
        self,
        binding: &DealerSeriesObligationBindingV3,
        dealer_value_terminal_receipt_id: Id,
    ) -> Result<Self> {
        binding.validate()?;
        dealer_value_terminal_receipt_id.validate_live()?;
        if self.series_obligation_children != 1
            || binding.phase != DealerSeriesObligationPhaseV1::Live
            || binding.key.binding_account_id != self.series_obligation_binding_account_id
            || binding.binding_id()? != self.series_obligation_binding_id
            || self.base.phase != DealerPhaseV2::Retiring
            || self.base.children.facility_positions != 0
            || self.base.children.facility_replays != 0
            || self.base.terminal_state_receipt_id.is_zero()
            || self.base.terminal_replay_semantic_id.is_zero()
        {
            return Err(Error::MismatchedBinding);
        }
        let next = Self {
            series_obligation_binding_account_id: Id::ZERO,
            series_obligation_children: 0,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Validate the combined exhaustive child partition.
    pub fn validate(&self) -> Result<()> {
        self.base.validate()?;
        self.product_upgrade_rent.validate()?;
        if self.product_upgrade_rent.neutral_sink != self.base.rent.neutral_sink {
            return Err(Error::MismatchedBinding);
        }
        match self.series_obligation_children {
            0 => {
                if !self.series_obligation_binding_account_id.is_zero() {
                    return Err(Error::InvalidChildGraph);
                }
                if !self.series_obligation_binding_id.is_zero()
                    && !matches!(self.base.phase, DealerPhaseV2::Retiring | DealerPhaseV2::Closed)
                {
                    return Err(Error::InvalidChildGraph);
                }
            }
            1 => {
                self.series_obligation_binding_id.validate_live()?;
                self.series_obligation_binding_account_id.validate_live()?;
                if self.series_obligation_binding_account_id
                    == self.base.facility_position_account_id
                    || self.series_obligation_binding_account_id
                        == self.base.facility_replay_account_id
                    || self.series_obligation_binding_account_id
                        == self.base.funded_dependencies_account_id
                {
                    return Err(Error::MismatchedBinding);
                }
            }
            _ => return Err(Error::InvalidChildGraph),
        }
        Ok(())
    }

    /// Current authoritative root content identity.
    pub fn state_id(&self) -> Result<Id> {
        self.content_id(DEALER_STATE_CONTENT_DOMAIN_V3)
    }
}

impl FixedCodec for DealerStateV3 {
    const ENCODED_LEN: usize = DEALER_STATE_BYTES_V3;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut base = [0u8; DEALER_STATE_BYTES_V2];
        self.base.encode_into(&mut base)?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&DEALER_STATE_MAGIC_V3, DEALER_STATE_VERSION_V3);
        writer.bytes(&base);
        writer.id(self.series_obligation_binding_id);
        writer.id(self.series_obligation_binding_account_id);
        writer.u32(self.series_obligation_children);
        writer.reserved(4);
        self.product_upgrade_rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(&DEALER_STATE_MAGIC_V3, DEALER_STATE_VERSION_V3)?;
        let base = DealerStateV2::decode(&reader.bytes::<DEALER_STATE_BYTES_V2>())?;
        let value = Self {
            base,
            series_obligation_binding_id: reader.id(),
            series_obligation_binding_account_id: reader.id(),
            series_obligation_children: reader.u32(),
            product_upgrade_rent: DeletableRentOwnerV1 {
                payer: Id::ZERO,
                neutral_sink: Id::ZERO,
                refundable_principal: 0,
                donation_floor: 0,
            },
        };
        reader.reserved(4)?;
        let mut value = value;
        value.product_upgrade_rent = DeletableRentOwnerV1::decode_body(&mut reader);
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(DEALER_STATE_BYTES_V3 == 1_132);
const _: () = assert!(DEALER_STATE_BYTES_V3 <= crate::MAX_SEMANTIC_BODY_BYTES);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DealerPhaseV2, DealerSeriesObligationKeyV1, DeletableRentOwnerV1, RootRentOwnerV1,
        SponsorCapitalDispositionV1, DealerChildCountsV2, MAX_OUTCOMES,
    };

    fn id(byte: u8) -> Id {
        Id::from_bytes([byte; 32])
    }

    fn base() -> DealerStateV2 {
        DealerStateV2 {
            policy_id: id(1),
            facility_id: id(2),
            facility_position_binding_id: id(3),
            facility_position_id: id(4),
            facility_position_account_id: id(5),
            facility_replay_account_id: id(6),
            sponsor: id(7),
            sponsor_refund_recipient: id(8),
            lp_page_head_id: id(9),
            lp_page_set_root: id(10),
            last_lp_owner: id(11),
            active_epoch_id: id(12),
            active_epoch_binding_account_id: id(13),
            active_lease_id: id(14),
            funded_dependencies_id: id(15),
            funded_dependencies_account_id: id(16),
            terminal_position_tombstone_id: Id::ZERO,
            terminal_replay_semantic_id: Id::ZERO,
            terminal_replay_intent_id: Id::ZERO,
            terminal_state_receipt_id: Id::ZERO,
            phase: DealerPhaseV2::Trading,
            sponsor_capital_disposition: SponsorCapitalDispositionV1::Donated,
            outcome_count: 2,
            generation: 1,
            child_sequence: 1,
            total_shares: 1,
            queued_shares: 0,
            terminal_claimed_shares: 0,
            sponsor_capital_atoms: 1,
            net_sold: [0; MAX_OUTCOMES],
            children: DealerChildCountsV2 {
                facility_positions: 1,
                facility_replays: 1,
                lp_pages: 1,
                live_lp_positions: 1,
                exit_tickets: 0,
                unclaimed_lp_positions: 0,
                funded_dependencies: 1,
                epoch_bindings: 1,
                leases: 1,
                settlement_pots: 1,
                terminal_allocations: 0,
                claim_work: 0,
            },
            rent: RootRentOwnerV1 {
                payer: id(17),
                neutral_sink: id(18),
                refundable_live_principal: 1,
                permanent_tombstone_principal: 1,
                donation_floor: 0,
            },
        }
    }

    fn binding() -> DealerSeriesObligationBindingV1 {
        let key = DealerSeriesObligationKeyV1 {
            binding_account_id: id(19),
            policy_id: id(1),
            facility_id: id(2),
            dealer_state_account_id: id(20),
            facility_position_binding_id: id(3),
            market_instance_v2_id: id(21),
            product_market_root_account_id: id(22),
            product_market_binding_id: id(23),
            series_plan_v5_id: id(24),
            series_market_link_account_id: id(25),
            attachment_plan_v4_id: id(26),
            product_generation: 1,
            series_ordinal: 0,
        };
        let pre = id(27);
        DealerSeriesObligationBindingV1::new_live(
            key,
            key.admission_owner_receipt_id(pre, 1).unwrap(),
            id(28),
            pre,
            id(29),
            1,
            DeletableRentOwnerV1 {
                payer: id(30),
                neutral_sink: id(31),
                refundable_principal: 1,
                donation_floor: 0,
            },
        )
        .unwrap()
    }

    #[test]
    fn promotion_round_trip_and_child_close_are_exhaustive() {
        let upgrade_rent = DeletableRentOwnerV1 {
            payer: id(37),
            neutral_sink: id(18),
            refundable_principal: 1,
            donation_floor: 0,
        };
        let state = DealerStateV3::promote(base(), &binding(), upgrade_rent).unwrap();
        let mut bytes = [0u8; DEALER_STATE_BYTES_V3];
        state.encode_into(&mut bytes).unwrap();
        assert_eq!(DealerStateV3::decode(&bytes), Ok(state));

        let live = binding();
        let terminal_state_receipt = id(32);
        let terminal_pre = id(33);
        let owner_receipt = live
            .terminal_owner_receipt_id(terminal_state_receipt, terminal_pre, 2)
            .unwrap();
        let terminal = live
            .terminalize(
                owner_receipt,
                id(34),
                terminal_pre,
                id(35),
                terminal_state_receipt,
                2,
            )
            .unwrap();
        let mut retiring = state.base;
        retiring.phase = DealerPhaseV2::Retiring;
        retiring.facility_position_id = id(38);
        retiring.lp_page_head_id = Id::ZERO;
        retiring.lp_page_set_root = Id::ZERO;
        retiring.active_epoch_id = Id::ZERO;
        retiring.active_epoch_binding_account_id = Id::ZERO;
        retiring.active_lease_id = Id::ZERO;
        retiring.total_shares = 0;
        retiring.children.facility_positions = 0;
        retiring.children.facility_replays = 0;
        retiring.children.lp_pages = 0;
        retiring.children.live_lp_positions = 0;
        retiring.children.epoch_bindings = 0;
        retiring.children.leases = 0;
        retiring.children.settlement_pots = 0;
        retiring.terminal_position_tombstone_id = id(39);
        retiring.terminal_replay_semantic_id = id(40);
        retiring.terminal_replay_intent_id = id(41);
        retiring.terminal_state_receipt_id = terminal_state_receipt;
        let state = state.with_base(retiring).unwrap();
        let observed = state.observe_terminal_binding(&terminal).unwrap();
        assert_eq!(
            observed.close_terminal_binding(&terminal).unwrap().series_obligation_children,
            0
        );
    }

    #[test]
    fn identity_swap_and_noncanonical_count_refuse() {
        let mut wrong = binding();
        wrong.key.facility_id = id(36);
        let upgrade_rent = DeletableRentOwnerV1 {
            payer: id(37),
            neutral_sink: id(18),
            refundable_principal: 1,
            donation_floor: 0,
        };
        assert_eq!(
            DealerStateV3::promote(base(), &wrong, upgrade_rent),
            Err(Error::MismatchedBinding)
        );

        let mut state = DealerStateV3::promote(base(), &binding(), upgrade_rent).unwrap();
        state.series_obligation_children = 2;
        assert_eq!(state.validate(), Err(Error::InvalidChildGraph));
    }
}
