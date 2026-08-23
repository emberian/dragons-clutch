// SPDX-License-Identifier: AGPL-3.0-or-later

//! Disabled counted SettlementRoot successor for the exact index plane.
//!
//! The live Root V1 cannot count the locator and adjacency accounts. This
//! breaking wrapper retains the exact Root V1 body and makes those two accounts
//! explicit expected/admitted/live/retired children. Its central account
//! coordinate and canonical in-place Root PDA are reserved, but every runtime
//! capability remains disabled until the complete SBF transition family lands.

use clutch_owner_settlement::SettlementCashPotV1;

use crate::{
    prepare_activate_merge_cash_pot_v1, CodecError, DeletableRentOwnerV1, Id32, Reader,
    SettlementRootChildStateV1, SettlementRootPhaseV1, SettlementRootSeedTupleV1,
    SettlementRootTerminalProjectionV1, SettlementRootV1AccountV1, Sha256BackendV1, Writer,
    SETTLEMENT_ROOT_ACCOUNT_BYTES, SETTLEMENT_ROOT_ACCOUNT_TAG,
};

/// Central persisted-account discriminator shared with the in-place Root V1.
pub const INDEXED_SETTLEMENT_ROOT_ACCOUNT_TAG: u8 = SETTLEMENT_ROOT_ACCOUNT_TAG;
/// Centrally reserved exact-index Root successor version.
pub const INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION: u8 = 2;
/// Exactly the locator and adjacency siblings are counted.
pub const INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1: u8 = 2;
/// Exact active successor width.
pub const INDEXED_SETTLEMENT_ROOT_BYTES_V1: usize =
    16 + SETTLEMENT_ROOT_ACCOUNT_BYTES + (6 * 32) + 8;
/// Account-key-bound data identity domain for the complete successor bytes.
pub const INDEXED_SETTLEMENT_ROOT_DATA_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/indexed-settlement-root-data/v1\0";
/// Exact fresh-allocation or in-place-upgrade projector transcript domain.
pub const INDEXED_SETTLEMENT_ROOT_RENT_PROJECTOR_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/indexed-settlement-root-rent-projector/v1\0";
/// Canonical one-per-Root frozen-order locator PDA domain.
pub const FROZEN_ORDER_LOCATOR_SEED_DOMAIN_V1: &[u8] =
    b"general-exact-order-locator:v1";
/// Canonical one-per-Root selected-candidate adjacency PDA domain.
pub const CANDIDATE_ORDER_SLICE_INDEX_SEED_DOMAIN_V1: &[u8] =
    b"general-exact-adjacency:v1";

const _: () = assert!(SETTLEMENT_ROOT_ACCOUNT_BYTES == 980);
const _: () = assert!(INDEXED_SETTLEMENT_ROOT_BYTES_V1 == 1_196);
const _: () = assert!(INDEXED_SETTLEMENT_ROOT_ACCOUNT_TAG == 0xa9);
const _: () = assert!(FROZEN_ORDER_LOCATOR_SEED_DOMAIN_V1.len() <= 32);
const _: () = assert!(CANDIDATE_ORDER_SLICE_INDEX_SEED_DOMAIN_V1.len() <= 32);

/// Canonical in-place PDA coordinates retained by the indexed Root successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedSettlementRootSeedTupleV1 {
    base: SettlementRootSeedTupleV1,
}

impl IndexedSettlementRootSeedTupleV1 {
    /// Derive the unchanged Root PDA tuple from exact Epoch and candidate IDs.
    pub fn new(epoch: Id32, candidate: Id32) -> Result<Self, CodecError> {
        Ok(Self {
            base: SettlementRootSeedTupleV1::new(epoch, candidate)?,
        })
    }

    /// Canonical Root seed domain; the successor is an in-place version change.
    pub const fn domain(&self) -> &'static [u8] {
        self.base.domain()
    }

    /// Exact authenticated Epoch PDA seed.
    pub const fn epoch(&self) -> &[u8; 32] {
        self.base.epoch()
    }

    /// Exact stable selected-candidate seed.
    pub const fn candidate(&self) -> &[u8; 32] {
        self.base.candidate()
    }
}

/// Canonical locator child PDA tuple, one-to-one with an indexed Root PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrozenOrderLocatorSeedTupleV1 {
    root: [u8; 32],
}

impl FrozenOrderLocatorSeedTupleV1 {
    /// Bind the unique locator to one nonzero indexed Root PDA.
    pub fn new(root: Id32) -> Result<Self, CodecError> {
        if root.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        Ok(Self { root: root.bytes() })
    }

    /// Fresh non-aliasing locator seed domain.
    pub const fn domain(&self) -> &'static [u8] {
        FROZEN_ORDER_LOCATOR_SEED_DOMAIN_V1
    }

    /// Exact parent indexed Root PDA seed.
    pub const fn root(&self) -> &[u8; 32] {
        &self.root
    }
}

/// Canonical adjacency child PDA tuple, one-to-one with an indexed Root PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateOrderSliceIndexSeedTupleV1 {
    root: [u8; 32],
}

impl CandidateOrderSliceIndexSeedTupleV1 {
    /// Bind the unique adjacency index to one nonzero indexed Root PDA.
    pub fn new(root: Id32) -> Result<Self, CodecError> {
        if root.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        Ok(Self { root: root.bytes() })
    }

    /// Fresh non-aliasing adjacency seed domain.
    pub const fn domain(&self) -> &'static [u8] {
        CANDIDATE_ORDER_SLICE_INDEX_SEED_DOMAIN_V1
    }

    /// Exact parent indexed Root PDA seed.
    pub const fn root(&self) -> &[u8; 32] {
        &self.root
    }
}

/// Whether the reserved indexed root is allocated directly or upgrades V1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum IndexedSettlementRootRentModeV1 {
    /// Action 39 allocates the 1,196-byte successor directly.
    Fresh = 1,
    /// A 980-byte V1 root is atomically reallocated in place.
    Upgrade = 2,
}

impl IndexedSettlementRootRentModeV1 {
    const fn code(self) -> u8 {
        match self {
            Self::Fresh => 1,
            Self::Upgrade => 2,
        }
    }
}

/// Exact root-account rent/allocation preparation consumed by index creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedSettlementRootRentPreparationV1 {
    mode: IndexedSettlementRootRentModeV1,
    root_account: Id32,
    base_before: SettlementRootV1AccountV1,
    base_after: SettlementRootV1AccountV1,
    data_len_before: usize,
    root_balance_before_lamports: u64,
    root_balance_after_lamports: u64,
    payer_debit_lamports: u64,
    payer_balance_before_lamports: u64,
    neutral_sink: Id32,
    projector_id: Id32,
}

impl IndexedSettlementRootRentPreparationV1 {
    /// Fresh allocation or in-place upgrade.
    pub const fn mode(&self) -> IndexedSettlementRootRentModeV1 {
        self.mode
    }

    /// Canonical Root PDA whose version/length changes atomically.
    pub const fn root_account(&self) -> Id32 {
        self.root_account
    }

    /// Exact source Root semantics authenticated before the transition.
    pub const fn base_before(&self) -> &SettlementRootV1AccountV1 {
        &self.base_before
    }

    /// Exact embedded Root semantics with current rent ownership.
    pub const fn base_after(&self) -> &SettlementRootV1AccountV1 {
        &self.base_after
    }

    /// Zero for direct allocation or 980 for an in-place upgrade.
    pub const fn data_len_before(&self) -> usize {
        self.data_len_before
    }

    /// Exact successor width, always 1,196 bytes.
    pub const fn data_len_after(&self) -> usize {
        INDEXED_SETTLEMENT_ROOT_BYTES_V1
    }

    /// Complete observed pre-transition root balance.
    pub const fn root_balance_before_lamports(&self) -> u64 {
        self.root_balance_before_lamports
    }

    /// Exact post-transition balance after the payer funds full principal.
    pub const fn root_balance_after_lamports(&self) -> u64 {
        self.root_balance_after_lamports
    }

    /// Exact debit from the persisted root rent payer.
    pub const fn payer_debit_lamports(&self) -> u64 {
        self.payer_debit_lamports
    }

    /// Authenticated payer balance shared with any sibling creates.
    pub const fn payer_balance_before_lamports(&self) -> u64 {
        self.payer_balance_before_lamports
    }

    /// Updated full principal and observed hostile-donation floor.
    pub const fn rent_after(&self) -> DeletableRentOwnerV1 {
        self.base_after.root_rent()
    }

    /// Immutable neutral sink which eventually receives every nonprincipal lamport.
    pub const fn neutral_sink(&self) -> Id32 {
        self.neutral_sink
    }

    /// Exact source/poststate/rent/width projector transcript.
    pub const fn projector_id(&self) -> Id32 {
        self.projector_id
    }
}

/// Prepare a direct 1,196-byte allocation without a prefund discount.
#[allow(clippy::too_many_arguments)]
pub fn prepare_fresh_indexed_settlement_root_rent_v1<B: Sha256BackendV1>(
    base: &SettlementRootV1AccountV1,
    root_account: Id32,
    root_balance_before_lamports: u64,
    indexed_root_rent_minimum_lamports: u64,
    payer_balance_before_lamports: u64,
    neutral_sink: Id32,
    backend: &B,
) -> Result<IndexedSettlementRootRentPreparationV1, CodecError> {
    base.validate()?;
    let rent = base.root_rent();
    rent.validate()?;
    if base.phase() != SettlementRootPhaseV1::Materializing
        || root_account.is_zero()
        || neutral_sink.is_zero()
        || root_account == neutral_sink
        || root_account == rent.payer
        || neutral_sink == rent.payer
        || indexed_root_rent_minimum_lamports == 0
        || rent.refundable_principal != indexed_root_rent_minimum_lamports
        || rent.donation_floor != root_balance_before_lamports
        || payer_balance_before_lamports < indexed_root_rent_minimum_lamports
    {
        return Err(CodecError::InvalidState);
    }
    let root_balance_after_lamports = root_balance_before_lamports
        .checked_add(indexed_root_rent_minimum_lamports)
        .ok_or(CodecError::ArithmeticOverflow)?;
    prepare_indexed_root_rent_projection_v1(
        IndexedSettlementRootRentModeV1::Fresh,
        root_account,
        *base,
        *base,
        0,
        root_balance_before_lamports,
        root_balance_after_lamports,
        indexed_root_rent_minimum_lamports,
        payer_balance_before_lamports,
        neutral_sink,
        backend,
    )
}

/// Prepare an exact in-place 980-to-1,196-byte root upgrade.
///
/// Hostile prefunding never discounts principal. The persisted V1 payer funds
/// the entire minimum increase; every pre-existing nonprincipal lamport becomes
/// the successor's immutable donation floor.
#[allow(clippy::too_many_arguments)]
pub fn prepare_indexed_settlement_root_upgrade_rent_v1<B: Sha256BackendV1>(
    base: &SettlementRootV1AccountV1,
    root_account: Id32,
    root_balance_before_lamports: u64,
    indexed_root_rent_minimum_lamports: u64,
    payer_balance_before_lamports: u64,
    neutral_sink: Id32,
    backend: &B,
) -> Result<IndexedSettlementRootRentPreparationV1, CodecError> {
    base.validate()?;
    let before = base.root_rent();
    before.validate()?;
    if base.phase() != SettlementRootPhaseV1::Materializing
        || root_account.is_zero()
        || neutral_sink.is_zero()
        || root_account == neutral_sink
        || root_account == before.payer
        || neutral_sink == before.payer
        || indexed_root_rent_minimum_lamports <= before.refundable_principal
    {
        return Err(CodecError::InvalidState);
    }
    let donation_before_lamports = root_balance_before_lamports
        .checked_sub(before.refundable_principal)
        .ok_or(CodecError::InvalidState)?;
    if donation_before_lamports < before.donation_floor {
        return Err(CodecError::InvalidState);
    }
    let payer_debit_lamports = indexed_root_rent_minimum_lamports
        .checked_sub(before.refundable_principal)
        .ok_or(CodecError::ArithmeticOverflow)?;
    if payer_balance_before_lamports < payer_debit_lamports {
        return Err(CodecError::InvalidState);
    }
    let after = DeletableRentOwnerV1 {
        payer: before.payer,
        refundable_principal: indexed_root_rent_minimum_lamports,
        donation_floor: donation_before_lamports,
    };
    let base_after = base.with_indexed_root_rent(after)?;
    let root_balance_after_lamports = root_balance_before_lamports
        .checked_add(payer_debit_lamports)
        .ok_or(CodecError::ArithmeticOverflow)?;
    prepare_indexed_root_rent_projection_v1(
        IndexedSettlementRootRentModeV1::Upgrade,
        root_account,
        *base,
        base_after,
        SETTLEMENT_ROOT_ACCOUNT_BYTES,
        root_balance_before_lamports,
        root_balance_after_lamports,
        payer_debit_lamports,
        payer_balance_before_lamports,
        neutral_sink,
        backend,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_indexed_root_rent_projection_v1<B: Sha256BackendV1>(
    mode: IndexedSettlementRootRentModeV1,
    root_account: Id32,
    base_before: SettlementRootV1AccountV1,
    base_after: SettlementRootV1AccountV1,
    data_len_before: usize,
    root_balance_before_lamports: u64,
    root_balance_after_lamports: u64,
    payer_debit_lamports: u64,
    payer_balance_before_lamports: u64,
    neutral_sink: Id32,
    backend: &B,
) -> Result<IndexedSettlementRootRentPreparationV1, CodecError> {
    let before_id = base_before.data_id(backend, root_account)?;
    let after_id = base_after.data_id(backend, root_account)?;
    let before_len = u64::try_from(data_len_before).map_err(|_| CodecError::InvalidCount)?;
    let after_len = u64::try_from(INDEXED_SETTLEMENT_ROOT_BYTES_V1)
        .map_err(|_| CodecError::InvalidCount)?;
    let rent = base_after.root_rent();
    let projector_id = Id32::new(backend.sha256(&[
        INDEXED_SETTLEMENT_ROOT_RENT_PROJECTOR_DOMAIN_V1,
        &[INDEXED_SETTLEMENT_ROOT_ACCOUNT_TAG, INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION],
        &[mode.code()],
        &root_account.bytes(),
        &before_id.bytes(),
        &after_id.bytes(),
        &before_len.to_le_bytes(),
        &after_len.to_le_bytes(),
        &root_balance_before_lamports.to_le_bytes(),
        &root_balance_after_lamports.to_le_bytes(),
        &payer_debit_lamports.to_le_bytes(),
        &payer_balance_before_lamports.to_le_bytes(),
        &rent.payer.bytes(),
        &rent.refundable_principal.to_le_bytes(),
        &rent.donation_floor.to_le_bytes(),
        &neutral_sink.bytes(),
    ]))?;
    Ok(IndexedSettlementRootRentPreparationV1 {
        mode,
        root_account,
        base_before,
        base_after,
        data_len_before,
        root_balance_before_lamports,
        root_balance_after_lamports,
        payer_debit_lamports,
        payer_balance_before_lamports,
        neutral_sink,
        projector_id,
    })
}

/// Exhaustive lifecycle of the exact two-child index family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExactIndexChildrenStateV1 {
    /// Both exact children were atomically admitted and remain live.
    Live = 1,
    /// Both exact children were atomically closed and counted retired.
    Retired = 2,
}

impl ExactIndexChildrenStateV1 {
    const fn code(self) -> u8 {
        match self {
            Self::Live => 1,
            Self::Retired => 2,
        }
    }

    fn decode(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::Live),
            2 => Ok(Self::Retired),
            _ => Err(CodecError::InvalidState),
        }
    }
}

/// Exact exhaustive child counts owned only by the indexed root successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactIndexChildCountsV1 {
    /// Exact expected child population, always two.
    pub expected: u8,
    /// Children atomically admitted by indexed-root initialization.
    pub admitted: u8,
    /// Admitted children not yet atomically retired.
    pub live: u8,
    /// Admitted children whose rent-owned accounts were atomically closed.
    pub retired: u8,
}

impl ExactIndexChildCountsV1 {
    /// Refuse partial admission, partial close, or arithmetic partitions.
    pub fn validate(self, state: ExactIndexChildrenStateV1) -> Result<(), CodecError> {
        if self.expected != INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1
            || self.admitted != self.expected
            || self
                .live
                .checked_add(self.retired)
                .ok_or(CodecError::ArithmeticOverflow)?
                != self.admitted
        {
            return Err(CodecError::InvalidCount);
        }
        match state {
            ExactIndexChildrenStateV1::Live
                if self.live == self.expected && self.retired == 0 => Ok(()),
            ExactIndexChildrenStateV1::Retired
                if self.live == 0 && self.retired == self.expected => Ok(()),
            ExactIndexChildrenStateV1::Live | ExactIndexChildrenStateV1::Retired => {
                Err(CodecError::InvalidState)
            }
        }
    }
}

/// Existing Root V1 transition selected through the counted successor.
///
/// Each variant calls the existing structural transition rather than accepting
/// an arbitrary caller-authored Root poststate. Index-child identities and
/// counts remain immutable across all variants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexedSettlementBaseTransitionV1 {
    /// Admit the unique exact Dealer child.
    AdmitDealerChild,
    /// Retire the unique exact Dealer child.
    RetireDealerChild,
    /// Admit one exact materialized settlement slice and its new endpoints.
    AdmitMaterialization {
        /// Newly created owner rows, zero through two.
        owner_rows_created: u8,
        /// Newly adopted filled Reservations, zero through two.
        filled_reservations_admitted: u8,
        /// Whether this slice creates one merge-payment latch.
        merge_receipt: bool,
    },
    /// Release one exact zero-fill Reservation.
    ReleaseUnfilledReservation,
    /// Complete one owner finalization.
    CompleteOwnerFinalization {
        /// Exact presence of the fee finalization child.
        fee_receipt_created: bool,
    },
    /// Complete one exact merge payment latch.
    CompleteMergePayment,
    /// Retire the complete archive set for one portfolio pair.
    RetirePortfolioPairArchives {
        /// Entire admitted/live receipt count, bounded by the base root.
        receipt_count: u8,
    },
}

/// Breaking counted-root wrapper for the immutable exact index pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedSettlementRootV1AccountV1 {
    base: SettlementRootV1AccountV1,
    locator_account: Id32,
    adjacency_account: Id32,
    plane_id: Id32,
    locator_data_id: Id32,
    adjacency_data_id: Id32,
    capability_profile_id: Id32,
    counts: ExactIndexChildCountsV1,
    state: ExactIndexChildrenStateV1,
}

/// Terminal receipt that includes both the base graph and retired index pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedSettlementRootTerminalProjectionV1 {
    base: SettlementRootTerminalProjectionV1,
    indexed_root_data_id: Id32,
    plane_id: Id32,
    locator_data_id: Id32,
    adjacency_data_id: Id32,
}

/// Atomic indexed-root plus merge cash-pot activation plan.
///
/// This preserves the base contract's only lawful action-37 transition: the
/// indexed-root postwrite cannot be obtained without the canonical cash-pot
/// body, exact rent owner, account identity, and bump in the same typed plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedActivateMergeCashPotPlanV1 {
    root: IndexedSettlementRootV1AccountV1,
    cash_pot_account: Id32,
    cash_pot: SettlementCashPotV1,
    rent: DeletableRentOwnerV1,
    stored_bump: u8,
}

impl IndexedActivateMergeCashPotPlanV1 {
    /// Indexed root successor latching the singleton cash pot live.
    pub const fn root(&self) -> &IndexedSettlementRootV1AccountV1 {
        &self.root
    }

    /// Exact canonical cash-pot PDA created atomically.
    pub const fn cash_pot_account(&self) -> Id32 {
        self.cash_pot_account
    }

    /// Canonical opening merge cash-pot body.
    pub const fn cash_pot(&self) -> SettlementCashPotV1 {
        self.cash_pot
    }

    /// Exact cash-pot rent/refund/donation owner.
    pub const fn rent(&self) -> DeletableRentOwnerV1 {
        self.rent
    }

    /// Stored canonical cash-pot PDA bump.
    pub const fn stored_bump(&self) -> u8 {
        self.stored_bump
    }
}

impl IndexedSettlementRootTerminalProjectionV1 {
    /// Exact historical Root V1 terminal coordinates.
    pub const fn base(&self) -> &SettlementRootTerminalProjectionV1 {
        &self.base
    }

    /// Account-key-bound exact indexed-root terminal body identity.
    pub const fn indexed_root_data_id(&self) -> Id32 {
        self.indexed_root_data_id
    }

    /// Exact retired plane identity.
    pub const fn plane_id(&self) -> Id32 {
        self.plane_id
    }

    /// Exact retired locator body identity.
    pub const fn locator_data_id(&self) -> Id32 {
        self.locator_data_id
    }

    /// Exact retired adjacency body identity.
    pub const fn adjacency_data_id(&self) -> Id32 {
        self.adjacency_data_id
    }
}

impl IndexedSettlementRootV1AccountV1 {
    /// Exact last frontier at which the retained Feed is still readable but
    /// every other base child liability has already been discharged.
    fn at_pre_feed_terminal_frontier(base: &SettlementRootV1AccountV1) -> bool {
        let counts = base.counts();
        let Some(expected_unfilled) = counts
            .expected_reservations
            .checked_sub(counts.expected_filled_reservations)
        else {
            return false;
        };
        base.phase() == SettlementRootPhaseV1::Retiring
            && counts.admitted_receipts == counts.expected_receipts
            && counts.live_receipts == 0
            && counts.admitted_owner_rows == counts.expected_owner_rows
            && counts.live_owner_rows == 0
            && counts.admitted_reservations == counts.expected_filled_reservations
            && counts.live_reservations == 0
            && counts.released_unfilled_reservations == expected_unfilled
            && counts.completed_owner_finalizations == counts.expected_owner_rows
            && counts.live_fee_finalizations == 0
            && counts.admitted_dealer_children == counts.expected_dealer_children
            && counts.live_dealer_children == 0
            && counts.admitted_merge_payments == counts.expected_merge_payments
            && counts.completed_merge_payments == counts.expected_merge_payments
            && base.cash_pot_state() == SettlementRootChildStateV1::Retired
            && matches!(
                base.final_pot_state(),
                SettlementRootChildStateV1::Absent | SettlementRootChildStateV1::Retired
            )
            && base.retained_feed_state() == SettlementRootChildStateV1::Live
            && matches!(
                base.fee_record_state(),
                SettlementRootChildStateV1::Absent | SettlementRootChildStateV1::Retired
            )
    }

    /// Atomically introduce a live, already-admitted exact sibling pair.
    ///
    /// The runtime must have derived all six identities from the complete V5
    /// page/CandidateFeed traversal and exact account bodies in the same
    /// rollback domain. No partially admitted persisted state is representable.
    #[allow(clippy::too_many_arguments)]
    pub fn new_live(
        base: SettlementRootV1AccountV1,
        locator_account: Id32,
        adjacency_account: Id32,
        plane_id: Id32,
        locator_data_id: Id32,
        adjacency_data_id: Id32,
        capability_profile_id: Id32,
    ) -> Result<Self, CodecError> {
        if base.phase() != SettlementRootPhaseV1::Materializing {
            return Err(CodecError::InvalidState);
        }
        let value = Self {
            base,
            locator_account,
            adjacency_account,
            plane_id,
            locator_data_id,
            adjacency_data_id,
            capability_profile_id,
            counts: ExactIndexChildCountsV1 {
                expected: INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1,
                admitted: INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1,
                live: INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1,
                retired: 0,
            },
            state: ExactIndexChildrenStateV1::Live,
        };
        value.validate()?;
        Ok(value)
    }

    /// Exact historical root semantics and mutable settlement counters.
    pub const fn base(&self) -> &SettlementRootV1AccountV1 {
        &self.base
    }

    /// Exact frozen-order locator child account.
    pub const fn locator_account(&self) -> Id32 {
        self.locator_account
    }

    /// Exact candidate adjacency child account.
    pub const fn adjacency_account(&self) -> Id32 {
        self.adjacency_account
    }

    /// Exact shared index-plane content identity.
    pub const fn plane_id(&self) -> Id32 {
        self.plane_id
    }

    /// Exact active locator body identity.
    pub const fn locator_data_id(&self) -> Id32 {
        self.locator_data_id
    }

    /// Exact active adjacency body identity.
    pub const fn adjacency_data_id(&self) -> Id32 {
        self.adjacency_data_id
    }

    /// Exact Genesis-selected ordered capability profile.
    pub const fn capability_profile_id(&self) -> Id32 {
        self.capability_profile_id
    }

    /// Exact two-child counts.
    pub const fn index_counts(&self) -> ExactIndexChildCountsV1 {
        self.counts
    }

    /// Exact exhaustive index-child lifecycle.
    pub const fn index_state(&self) -> ExactIndexChildrenStateV1 {
        self.state
    }

    /// True only after both the base graph and both index children are terminal.
    pub fn is_terminal(&self) -> bool {
        self.base.phase() == SettlementRootPhaseV1::Terminal
            && self.state == ExactIndexChildrenStateV1::Retired
    }

    /// Validate the base root, six identities, exact count partition, and phase join.
    pub fn validate(&self) -> Result<(), CodecError> {
        self.base.validate()?;
        let identities = [
            self.locator_account,
            self.adjacency_account,
            self.plane_id,
            self.locator_data_id,
            self.adjacency_data_id,
            self.capability_profile_id,
        ];
        if identities.iter().any(|identity| identity.is_zero()) {
            return Err(CodecError::ZeroIdentity);
        }
        // Semantic hashes and profile identities occupy independent domains;
        // byte equality between them is not account aliasing and must not make
        // an otherwise valid root unrepresentable. Only physical accounts
        // require pairwise nonaliasing below.
        let physical = [
            self.locator_account,
            self.adjacency_account,
            self.base.market(),
            self.base.epoch(),
            self.base.market_binding(),
            self.base.retained_feed(),
        ];
        let mut left = 0usize;
        while left < physical.len() {
            let mut right = left + 1;
            while right < physical.len() {
                if physical[left] == physical[right] {
                    return Err(CodecError::MismatchedBinding);
                }
                right += 1;
            }
            left += 1;
        }
        self.counts.validate(self.state)?;
        if self.state == ExactIndexChildrenStateV1::Live
            && self.base.retained_feed_state() != SettlementRootChildStateV1::Live
        {
            return Err(CodecError::InvalidState);
        }
        if self.state == ExactIndexChildrenStateV1::Retired {
            match self.base.phase() {
                SettlementRootPhaseV1::Retiring
                    if Self::at_pre_feed_terminal_frontier(&self.base) => {}
                SettlementRootPhaseV1::Terminal => {}
                _ => return Err(CodecError::InvalidState),
            }
        }
        Ok(())
    }

    /// Apply only an existing checked Root V1 transition while retaining exact
    /// immutable index identity and count ownership.
    pub fn apply_base_transition(
        &self,
        transition: IndexedSettlementBaseTransitionV1,
    ) -> Result<Self, CodecError> {
        self.validate()?;
        if self.state != ExactIndexChildrenStateV1::Live {
            return Err(CodecError::InvalidState);
        }
        let base = match transition {
            IndexedSettlementBaseTransitionV1::AdmitDealerChild => {
                self.base.admit_dealer_child()?
            }
            IndexedSettlementBaseTransitionV1::RetireDealerChild => {
                self.base.retire_dealer_child()?
            }
            IndexedSettlementBaseTransitionV1::AdmitMaterialization {
                owner_rows_created,
                filled_reservations_admitted,
                merge_receipt,
            } => self.base.admit_materialization_delta(
                owner_rows_created,
                filled_reservations_admitted,
                merge_receipt,
            )?,
            IndexedSettlementBaseTransitionV1::ReleaseUnfilledReservation => {
                self.base.release_unfilled_reservation()?
            }
            IndexedSettlementBaseTransitionV1::CompleteOwnerFinalization {
                fee_receipt_created,
            } => self
                .base
                .complete_owner_finalization(fee_receipt_created)?,
            IndexedSettlementBaseTransitionV1::CompleteMergePayment => {
                self.base.complete_merge_payment()?
            }
            IndexedSettlementBaseTransitionV1::RetirePortfolioPairArchives {
                receipt_count,
            } => self.base.retire_portfolio_pair_archives(receipt_count)?,
        };
        let value = Self { base, ..*self };
        value.validate()?;
        Ok(value)
    }

    /// Prepare the sole atomic merge cash-pot activation for the indexed root.
    pub fn prepare_activate_merge_cash_pot(
        &self,
    ) -> Result<IndexedActivateMergeCashPotPlanV1, CodecError> {
        self.validate()?;
        if self.state != ExactIndexChildrenStateV1::Live {
            return Err(CodecError::InvalidState);
        }
        let base = prepare_activate_merge_cash_pot_v1(&self.base)?;
        let root = Self {
            base: *base.root(),
            ..*self
        };
        root.validate()?;
        Ok(IndexedActivateMergeCashPotPlanV1 {
            root,
            cash_pot_account: base.cash_pot_account(),
            cash_pot: base.cash_pot(),
            rent: base.rent(),
            stored_bump: base.stored_bump(),
        })
    }

    /// Atomically count both live siblings retired immediately before Feed retirement.
    ///
    /// The runtime must close both exact accounts, transfer both rent principals
    /// and donations, and write this successor in one rollback domain.
    pub fn retire_index_children(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if self.state != ExactIndexChildrenStateV1::Live
            || !Self::at_pre_feed_terminal_frontier(&self.base)
        {
            return Err(CodecError::InvalidState);
        }
        let value = Self {
            counts: ExactIndexChildCountsV1 {
                expected: INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1,
                admitted: INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1,
                live: 0,
                retired: INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1,
            },
            state: ExactIndexChildrenStateV1::Retired,
            ..*self
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode the exact reserved successor envelope and nested canonical Root V1.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut base = [0u8; SETTLEMENT_ROOT_ACCOUNT_BYTES];
        self.base.encode(&mut base)?;
        let mut writer = Writer::exact(output, INDEXED_SETTLEMENT_ROOT_BYTES_V1)?;
        writer.u8(INDEXED_SETTLEMENT_ROOT_ACCOUNT_TAG)?;
        writer.u8(INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION)?;
        writer.u8(self.state.code())?;
        writer.u8(0)?;
        writer.bytes(&[0; 12])?;
        writer.bytes(&base)?;
        for identity in [
            self.locator_account,
            self.adjacency_account,
            self.plane_id,
            self.locator_data_id,
            self.adjacency_data_id,
            self.capability_profile_id,
        ] {
            writer.bytes(&identity.bytes())?;
        }
        for count in [
            self.counts.expected,
            self.counts.admitted,
            self.counts.live,
            self.counts.retired,
        ] {
            writer.u8(count)?;
        }
        writer.bytes(&[0; 4])?;
        writer.finish()
    }

    /// Decode only the exact reserved successor schema and rerun every invariant.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, INDEXED_SETTLEMENT_ROOT_BYTES_V1)?;
        if reader.u8()? != INDEXED_SETTLEMENT_ROOT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let state = ExactIndexChildrenStateV1::decode(reader.u8()?)?;
        if reader.u8()? != 0 || reader.array::<12>()? != [0; 12] {
            return Err(CodecError::NonCanonicalPadding);
        }
        let base = SettlementRootV1AccountV1::decode(&reader.array::<SETTLEMENT_ROOT_ACCOUNT_BYTES>()?)?;
        let locator_account = Id32::new(reader.array()?)?;
        let adjacency_account = Id32::new(reader.array()?)?;
        let plane_id = Id32::new(reader.array()?)?;
        let locator_data_id = Id32::new(reader.array()?)?;
        let adjacency_data_id = Id32::new(reader.array()?)?;
        let capability_profile_id = Id32::new(reader.array()?)?;
        let counts = ExactIndexChildCountsV1 {
            expected: reader.u8()?,
            admitted: reader.u8()?,
            live: reader.u8()?,
            retired: reader.u8()?,
        };
        if reader.array::<4>()? != [0; 4] {
            return Err(CodecError::NonCanonicalPadding);
        }
        reader.finish()?;
        let value = Self {
            base,
            locator_account,
            adjacency_account,
            plane_id,
            locator_data_id,
            adjacency_data_id,
            capability_profile_id,
            counts,
            state,
        };
        value.validate()?;
        Ok(value)
    }

    /// Account-key-bound content identity of the exact successor bytes.
    pub fn data_id<B: Sha256BackendV1>(
        &self,
        backend: &B,
        root_account: Id32,
    ) -> Result<Id32, CodecError> {
        self.validate()?;
        if root_account.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        let mut bytes = [0u8; INDEXED_SETTLEMENT_ROOT_BYTES_V1];
        self.encode(&mut bytes)?;
        Id32::new(backend.sha256(&[
            INDEXED_SETTLEMENT_ROOT_DATA_ID_DOMAIN_V1,
            &root_account.bytes(),
            &bytes,
        ]))
    }

    /// Base terminal projection, available only after both index siblings retire.
    pub fn terminal_projection<B: Sha256BackendV1>(
        &self,
        backend: &B,
        root_account: Id32,
    ) -> Result<IndexedSettlementRootTerminalProjectionV1, CodecError> {
        self.validate()?;
        if !self.is_terminal() {
            return Err(CodecError::InvalidState);
        }
        Ok(IndexedSettlementRootTerminalProjectionV1 {
            base: self.base.terminal_projection(backend, root_account)?,
            indexed_root_data_id: self.data_id(backend, root_account)?,
            plane_id: self.plane_id,
            locator_data_id: self.locator_data_id,
            adjacency_data_id: self.adjacency_data_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> Id32 {
        Id32::new([value; 32]).unwrap()
    }

    fn live_indexed_root() -> IndexedSettlementRootV1AccountV1 {
        IndexedSettlementRootV1AccountV1::new_live(
            crate::settlement_root::tests::materializing_root(),
            id(21),
            id(22),
            id(23),
            id(24),
            id(25),
            id(26),
        )
        .unwrap()
    }

    #[test]
    fn counts_refuse_partial_admission_and_partial_retirement() {
        assert_eq!(
            ExactIndexChildCountsV1 {
                expected: 2,
                admitted: 1,
                live: 1,
                retired: 0,
            }
            .validate(ExactIndexChildrenStateV1::Live),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            ExactIndexChildCountsV1 {
                expected: 2,
                admitted: 2,
                live: 1,
                retired: 1,
            }
            .validate(ExactIndexChildrenStateV1::Live),
            Err(CodecError::InvalidState)
        );
        assert_eq!(
            ExactIndexChildCountsV1 {
                expected: 2,
                admitted: 2,
                live: 0,
                retired: 2,
            }
            .validate(ExactIndexChildrenStateV1::Retired),
            Ok(())
        );
    }

    #[test]
    fn codec_refuses_historical_width_tag_version_and_reserved_bytes() {
        assert_eq!(
            IndexedSettlementRootV1AccountV1::decode(&[0; INDEXED_SETTLEMENT_ROOT_BYTES_V1 - 1]),
            Err(CodecError::WrongLength)
        );
        let mut bytes = [0u8; INDEXED_SETTLEMENT_ROOT_BYTES_V1];
        assert_eq!(
            IndexedSettlementRootV1AccountV1::decode(&bytes),
            Err(CodecError::WrongTag)
        );
        bytes[0] = INDEXED_SETTLEMENT_ROOT_ACCOUNT_TAG;
        bytes[1] = 3;
        assert_eq!(
            IndexedSettlementRootV1AccountV1::decode(&bytes),
            Err(CodecError::WrongVersion)
        );
        bytes[1] = INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION;
        bytes[2] = 1;
        bytes[4] = 1;
        assert_eq!(
            IndexedSettlementRootV1AccountV1::decode(&bytes),
            Err(CodecError::NonCanonicalPadding)
        );
    }

    #[test]
    fn exact_children_retire_only_at_live_feed_terminal_frontier() {
        let mut early = live_indexed_root();
        early.base = crate::settlement_root::tests::portfolio_settling_root();
        early.validate().unwrap();
        assert_eq!(early.retire_index_children(), Err(CodecError::InvalidState));

        let mut frontier = live_indexed_root();
        frontier.base = crate::settlement_root::tests::pre_feed_terminal_frontier_root();
        frontier.validate().unwrap();
        let retired = frontier.retire_index_children().unwrap();
        assert_eq!(retired.index_state(), ExactIndexChildrenStateV1::Retired);
        retired.validate().unwrap();
        let mut terminal = retired;
        terminal.base = crate::settlement_root::tests::terminal_root();
        terminal.validate().unwrap();

        let mut feed_closed_first = live_indexed_root();
        feed_closed_first.base = crate::settlement_root::tests::terminal_root();
        assert_eq!(feed_closed_first.validate(), Err(CodecError::InvalidState));
        assert_eq!(
            feed_closed_first.retire_index_children(),
            Err(CodecError::InvalidState),
        );

        let mut refeed_wrong_order = terminal;
        refeed_wrong_order.base = crate::settlement_root::tests::materializing_root();
        assert_eq!(refeed_wrong_order.validate(), Err(CodecError::InvalidState));
    }
}
