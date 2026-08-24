// SPDX-License-Identifier: AGPL-3.0-or-later

//! Counted SettlementRoot successor for the exact index plane.
//!
//! The live Root V1 cannot count the locator and adjacency accounts. This
//! breaking wrapper retains the exact Root V1 body and makes those two accounts
//! explicit expected/admitted/live/retired children at the canonical in-place
//! Root PDA.

use crate::{
    CandidateWindowV5AccountV1, CodecError, DeletableRentOwnerV1, GeneralEpochPhaseV1,
    GeneralEpochV6AccountV1, Id32, Reader,
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
    16 + SETTLEMENT_ROOT_ACCOUNT_BYTES + (7 * 32) + 8;
const INDEXED_SETTLEMENT_ROOT_ENVELOPE_BYTES_V1: usize = 16;
const INDEXED_SETTLEMENT_ROOT_SUFFIX_OFFSET_V1: usize =
    INDEXED_SETTLEMENT_ROOT_ENVELOPE_BYTES_V1 + SETTLEMENT_ROOT_ACCOUNT_BYTES;
const INDEXED_SETTLEMENT_ROOT_SUFFIX_BYTES_V1: usize = (7 * 32) + 8;
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
const _: () = assert!(INDEXED_SETTLEMENT_ROOT_BYTES_V1 == 1_228);
const _: () = assert!(
    INDEXED_SETTLEMENT_ROOT_SUFFIX_OFFSET_V1 + INDEXED_SETTLEMENT_ROOT_SUFFIX_BYTES_V1
        == INDEXED_SETTLEMENT_ROOT_BYTES_V1
);
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
    /// Action 39 allocates the 1,228-byte successor directly.
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
#[derive(Debug, Eq, PartialEq)]
pub struct IndexedSettlementRootRentPreparationV1 {
    mode: IndexedSettlementRootRentModeV1,
    root_account: Id32,
    base_before_data_id: Id32,
    base_after_data_id: Id32,
    rent_after: DeletableRentOwnerV1,
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

    /// Account-key-bound identity of the exact source Root body.
    pub const fn base_before_data_id(&self) -> Id32 {
        self.base_before_data_id
    }

    /// Account-key-bound identity of the embedded Root after the rent change.
    pub const fn base_after_data_id(&self) -> Id32 {
        self.base_after_data_id
    }

    /// Zero for direct allocation or 980 for an in-place upgrade.
    pub const fn data_len_before(&self) -> usize {
        self.data_len_before
    }

    /// Exact successor width, always 1,228 bytes.
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
        self.rent_after
    }

    /// Immutable neutral sink which eventually receives every nonprincipal lamport.
    pub const fn neutral_sink(&self) -> Id32 {
        self.neutral_sink
    }

    /// Exact source/poststate/rent/width projector transcript.
    pub const fn projector_id(&self) -> Id32 {
        self.projector_id
    }

    /// Authenticate the exact borrowed source Root and mint one noncopyable
    /// authority for compact indexed-root construction.
    pub fn authenticate_source<'a, B: Sha256BackendV1>(
        self,
        base_before: &'a SettlementRootV1AccountV1,
        backend: &B,
    ) -> Result<AuthenticatedIndexedSettlementRootRentV1<'a>, CodecError> {
        base_before.validate()?;
        if base_before.data_id(backend, self.root_account)? != self.base_before_data_id {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(AuthenticatedIndexedSettlementRootRentV1 {
            preparation: self,
            base_before,
            _private: (),
        })
    }
}

/// Private-field, noncopyable authority joining one compact rent receipt to
/// the exact borrowed source Root used by index construction.
#[derive(Debug)]
pub struct AuthenticatedIndexedSettlementRootRentV1<'a> {
    preparation: IndexedSettlementRootRentPreparationV1,
    base_before: &'a SettlementRootV1AccountV1,
    _private: (),
}

impl AuthenticatedIndexedSettlementRootRentV1<'_> {
    /// Exact borrowed source Root authenticated by the preparation transcript.
    pub const fn base_before(&self) -> &SettlementRootV1AccountV1 {
        self.base_before
    }

    /// Fresh allocation or in-place upgrade.
    pub const fn mode(&self) -> IndexedSettlementRootRentModeV1 {
        self.preparation.mode()
    }

    /// Canonical Root PDA whose version/length changes atomically.
    pub const fn root_account(&self) -> Id32 {
        self.preparation.root_account()
    }

    /// Zero for direct allocation or 980 for an in-place upgrade.
    pub const fn data_len_before(&self) -> usize {
        self.preparation.data_len_before()
    }

    /// Complete observed pre-transition root balance.
    pub const fn root_balance_before_lamports(&self) -> u64 {
        self.preparation.root_balance_before_lamports()
    }

    /// Exact post-transition balance after the payer funds full principal.
    pub const fn root_balance_after_lamports(&self) -> u64 {
        self.preparation.root_balance_after_lamports()
    }

    /// Exact debit from the persisted root rent payer.
    pub const fn payer_debit_lamports(&self) -> u64 {
        self.preparation.payer_debit_lamports()
    }

    /// Authenticated payer balance shared with sibling creation.
    pub const fn payer_balance_before_lamports(&self) -> u64 {
        self.preparation.payer_balance_before_lamports()
    }

    /// Updated full principal and observed hostile-donation floor.
    pub const fn rent_after(&self) -> DeletableRentOwnerV1 {
        self.preparation.rent_after()
    }

    /// Immutable sink for every nonprincipal lamport.
    pub const fn neutral_sink(&self) -> Id32 {
        self.preparation.neutral_sink()
    }

    /// Consume the authenticated rent/source join and stream the exact live
    /// indexed successor without retaining either Root inside the receipt.
    #[allow(clippy::too_many_arguments)]
    pub fn encode_new_live_and_data_id<B: Sha256BackendV1>(
        self,
        locator_account: Id32,
        adjacency_account: Id32,
        plane_id: Id32,
        locator_data_id: Id32,
        adjacency_data_id: Id32,
        selected_feed_data_id: Id32,
        capability_profile_id: Id32,
        backend: &B,
        output: &mut [u8],
    ) -> Result<Id32, CodecError> {
        let base_after = self
            .base_before
            .with_indexed_root_rent(self.preparation.rent_after)?;
        if base_after.data_id(backend, self.preparation.root_account)?
            != self.preparation.base_after_data_id
        {
            return Err(CodecError::MismatchedBinding);
        }
        IndexedSettlementRootV1AccountV1::encode_new_live_and_data_id(
            &base_after,
            locator_account,
            adjacency_account,
            plane_id,
            locator_data_id,
            adjacency_data_id,
            selected_feed_data_id,
            capability_profile_id,
            backend,
            self.preparation.root_account,
            output,
        )
    }
}

const _: () = assert!(
    core::mem::size_of::<IndexedSettlementRootRentPreparationV1>() <= 320
);
const _: () = assert!(
    core::mem::size_of::<AuthenticatedIndexedSettlementRootRentV1<'static>>() <= 352
);

/// Prepare a direct 1,228-byte allocation without a prefund discount.
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
        base,
        base,
        0,
        root_balance_before_lamports,
        root_balance_after_lamports,
        indexed_root_rent_minimum_lamports,
        payer_balance_before_lamports,
        neutral_sink,
        backend,
    )
}

/// Prepare an exact in-place 980-to-1,228-byte root upgrade.
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
        base,
        &base_after,
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
    base_before: &SettlementRootV1AccountV1,
    base_after: &SettlementRootV1AccountV1,
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
    let rent_after = base_after.root_rent();
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
        &rent_after.payer.bytes(),
        &rent_after.refundable_principal.to_le_bytes(),
        &rent_after.donation_floor.to_le_bytes(),
        &neutral_sink.bytes(),
    ]))?;
    Ok(IndexedSettlementRootRentPreparationV1 {
        mode,
        root_account,
        base_before_data_id: before_id,
        base_after_data_id: after_id,
        rent_after,
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
enum IndexedSettlementBaseTransitionV1 {
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
    /// Create and count the unique merge-funded settlement cash pot.
    ActivateMergeCashPot,
    /// Complete one owner finalization.
    CompleteOwnerFinalization {
        /// Exact presence of the fee finalization child.
        fee_receipt_created: bool,
    },
    /// Complete one exact merge payment latch.
    CompleteMergePayment,
    /// Retire one exact terminal Receipt.
    RetireOneReceipt,
    /// Retire one exact finalized owner row.
    RetireOneOwnerRow,
    /// Retire one exact terminal filled Reservation.
    RetireOneReservation,
    /// Retire one exact fee-finalization child.
    RetireOneFeeFinalization,
    /// Begin singleton retirement after all settlement children are accounted.
    BeginRetiring,
    /// Retire the exact settlement cash pot.
    RetireCashPot,
    /// Retire the exact Split/Merge FinalPot.
    RetireFinalPot,
    /// Retire the exact selected fee record.
    RetireFeeRecord,
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
    selected_feed_data_id: Id32,
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
    selected_feed_data_id: Id32,
}

/// Compact terminal close facts decoded from one exact indexed-root body.
///
/// This is structural evidence only. The SBF adapter must still authenticate
/// the program owner, canonical PDA/bump, writable account, MarketBinding, and
/// exact lamport balance before it may close the root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedSettlementRootCloseProjectionV1 {
    terminal: IndexedSettlementRootTerminalProjectionV1,
    fee_record: Id32,
    root_rent: DeletableRentOwnerV1,
    market_binding: Id32,
    stored_bump: u8,
}

impl IndexedSettlementRootCloseProjectionV1 {
    /// Exact terminal receipt and Product occurrence handoff.
    pub const fn terminal(&self) -> &IndexedSettlementRootTerminalProjectionV1 {
        &self.terminal
    }
    /// Canonical selected-fee coordinate that names the durable closure pair.
    ///
    /// This is retained only by the close projection so the final SBF action
    /// can authenticate `0xb9/v2` and `0xb9/v3` before deleting the root. It
    /// does not add a second fee semantic owner to the terminal handoff.
    pub const fn fee_record(&self) -> Id32 {
        self.fee_record
    }
    /// Immutable root rent principal and payer.
    pub const fn root_rent(&self) -> DeletableRentOwnerV1 {
        self.root_rent
    }
    /// MarketBinding account that owns the neutral donation sink.
    pub const fn market_binding(&self) -> Id32 {
        self.market_binding
    }
    /// Canonical indexed-root PDA bump persisted in the base root.
    pub const fn stored_bump(&self) -> u8 {
        self.stored_bump
    }
}

/// Atomically consume one exact terminal indexed-root projection and stream
/// the finalized Epoch successor with its unique selected-root count cleared.
///
/// The immutable finalized Window is the authoritative historical pointer to
/// the selected root. This prevents a terminal projection from another root
/// in the same Epoch from decrementing the count. The runtime adapter must
/// still authenticate the exact Epoch and Window accounts, owners, PDAs,
/// access modes, and unchanged Window bytes before applying the root close.
pub fn encode_retire_indexed_settlement_root_v1(
    terminal: &IndexedSettlementRootCloseProjectionV1,
    epoch_account: Id32,
    epoch: &GeneralEpochV6AccountV1,
    window_account: Id32,
    window: &CandidateWindowV5AccountV1,
    epoch_output: &mut [u8],
) -> Result<(), CodecError> {
    epoch.validate()?;
    window.validate()?;
    let selected = terminal.terminal().base();
    let window = window.base();
    if epoch_account.is_zero()
        || window_account.is_zero()
        || selected.root_account() == epoch_account
        || selected.root_account() == window_account
        || epoch_account == window_account
        || epoch.phase != GeneralEpochPhaseV1::Finalized
        || epoch.selected_candidate_count != 1
        || epoch.window != window_account
        || epoch.market_binding != terminal.market_binding()
        || epoch.market_runtime != selected.market()
        || epoch.market_instance_v2_id != selected.market_instance_v2_id()
        || epoch.generation != selected.epoch_generation()
        || selected.epoch() != epoch_account
        || window.epoch != epoch_account
        || window.market != epoch.market_runtime
        || window.epoch_generation != epoch.generation
        || window.finalized_slot == 0
        || window.selected_candidate_artifact != selected.root_account()
    {
        return Err(CodecError::MismatchedBinding);
    }
    GeneralEpochV6AccountV1 {
        selected_candidate_count: 0,
        ..*epoch
    }
    .encode(epoch_output)
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

    /// Exact retained Feed body authenticated before index retirement.
    pub const fn selected_feed_data_id(&self) -> Id32 {
        self.selected_feed_data_id
    }
}

impl IndexedSettlementRootV1AccountV1 {
    /// Hostile-decode one exact terminal body into the compact facts needed by
    /// the root-close adapter, hashing the supplied bytes without re-encoding
    /// a second indexed-root-sized scratch value.
    pub fn decode_terminal_close_projection<B: Sha256BackendV1>(
        backend: &B,
        root_account: Id32,
        input: &[u8],
    ) -> Result<IndexedSettlementRootCloseProjectionV1, CodecError> {
        if root_account.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        let value = Self::decode(input)?;
        if !value.is_terminal() {
            return Err(CodecError::InvalidState);
        }
        let terminal = IndexedSettlementRootTerminalProjectionV1 {
            base: value.base.terminal_projection(backend, root_account)?,
            indexed_root_data_id: Self::encoded_data_id(backend, root_account, input)?,
            plane_id: value.plane_id,
            locator_data_id: value.locator_data_id,
            adjacency_data_id: value.adjacency_data_id,
            selected_feed_data_id: value.selected_feed_data_id,
        };
        Ok(IndexedSettlementRootCloseProjectionV1 {
            terminal,
            fee_record: value.base.fee_record(),
            root_rent: value.base.root_rent(),
            market_binding: value.base.market_binding(),
            stored_bump: value.base.stored_bump(),
        })
    }

    /// Exact last frontier at which the retained Feed is still readable but
    /// every other base child liability has already been discharged.
    fn at_pre_feed_terminal_frontier(base: &SettlementRootV1AccountV1) -> bool {
        base.at_retained_feed_retirement_frontier()
    }

    /// Atomically introduce a live, already-admitted exact sibling pair.
    ///
    /// The runtime must have derived all seven identities from the complete V5
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
        selected_feed_data_id: Id32,
        capability_profile_id: Id32,
    ) -> Result<Self, CodecError> {
        if base.phase() != SettlementRootPhaseV1::Materializing {
            return Err(CodecError::InvalidState);
        }
        let counts = Self::live_counts();
        Self::validate_components(
            &base,
            locator_account,
            adjacency_account,
            plane_id,
            locator_data_id,
            adjacency_data_id,
            selected_feed_data_id,
            capability_profile_id,
            counts,
            ExactIndexChildrenStateV1::Live,
        )?;
        let value = Self {
            base,
            locator_account,
            adjacency_account,
            plane_id,
            locator_data_id,
            adjacency_data_id,
            selected_feed_data_id,
            capability_profile_id,
            counts,
            state: ExactIndexChildrenStateV1::Live,
        };
        Ok(value)
    }

    const fn live_counts() -> ExactIndexChildCountsV1 {
        ExactIndexChildCountsV1 {
            expected: INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1,
            admitted: INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1,
            live: INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1,
            retired: 0,
        }
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

    /// Full exact retained-Feed byte identity needed after index retirement.
    pub const fn selected_feed_data_id(&self) -> Id32 {
        self.selected_feed_data_id
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

    /// Validate the base root, seven identities, exact count partition, and phase join.
    pub fn validate(&self) -> Result<(), CodecError> {
        Self::validate_components(
            &self.base,
            self.locator_account,
            self.adjacency_account,
            self.plane_id,
            self.locator_data_id,
            self.adjacency_data_id,
            self.selected_feed_data_id,
            self.capability_profile_id,
            self.counts,
            self.state,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_components(
        base: &SettlementRootV1AccountV1,
        locator_account: Id32,
        adjacency_account: Id32,
        plane_id: Id32,
        locator_data_id: Id32,
        adjacency_data_id: Id32,
        selected_feed_data_id: Id32,
        capability_profile_id: Id32,
        counts: ExactIndexChildCountsV1,
        state: ExactIndexChildrenStateV1,
    ) -> Result<(), CodecError> {
        base.validate()?;
        let identities = [
            locator_account,
            adjacency_account,
            plane_id,
            locator_data_id,
            adjacency_data_id,
            selected_feed_data_id,
            capability_profile_id,
        ];
        if identities.iter().any(|identity| identity.is_zero()) {
            return Err(CodecError::ZeroIdentity);
        }
        // Semantic hashes and profile identities occupy independent domains;
        // byte equality between them is not account aliasing and must not make
        // an otherwise valid root unrepresentable. Only physical accounts
        // require pairwise nonaliasing below.
        let physical = [
            locator_account,
            adjacency_account,
            base.market(),
            base.epoch(),
            base.market_binding(),
            base.retained_feed(),
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
        counts.validate(state)?;
        if state == ExactIndexChildrenStateV1::Live
            && base.retained_feed_state() != SettlementRootChildStateV1::Live
        {
            return Err(CodecError::InvalidState);
        }
        if state == ExactIndexChildrenStateV1::Retired {
            match base.phase() {
                SettlementRootPhaseV1::Retiring
                    if Self::at_pre_feed_terminal_frontier(base) => {}
                SettlementRootPhaseV1::Terminal => {}
                _ => return Err(CodecError::InvalidState),
            }
        }
        Ok(())
    }

    /// Apply only an existing checked Root V1 transition while retaining exact
    /// immutable index identity and count ownership.
    fn apply_base_transition(
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
            IndexedSettlementBaseTransitionV1::ActivateMergeCashPot => {
                *crate::prepare_activate_merge_cash_pot_v1(&self.base)?.root()
            }
            IndexedSettlementBaseTransitionV1::CompleteOwnerFinalization {
                fee_receipt_created,
            } => self
                .base
                .complete_owner_finalization(fee_receipt_created)?,
            IndexedSettlementBaseTransitionV1::CompleteMergePayment => {
                self.base.complete_merge_payment()?
            }
            IndexedSettlementBaseTransitionV1::RetireOneReceipt => {
                self.base.retire_one_receipt()?
            }
            IndexedSettlementBaseTransitionV1::RetireOneOwnerRow => {
                self.base.retire_one_owner_row()?
            }
            IndexedSettlementBaseTransitionV1::RetireOneReservation => {
                self.base.retire_one_reservation()?
            }
            IndexedSettlementBaseTransitionV1::RetireOneFeeFinalization => {
                self.base.retire_one_fee_finalization()?
            }
            IndexedSettlementBaseTransitionV1::BeginRetiring => self.base.begin_retiring()?,
            IndexedSettlementBaseTransitionV1::RetireCashPot => self.base.retire_cash_pot()?,
            IndexedSettlementBaseTransitionV1::RetireFinalPot => self.base.retire_final_pot()?,
            IndexedSettlementBaseTransitionV1::RetireFeeRecord => {
                self.base.retire_fee_record()?
            }
            IndexedSettlementBaseTransitionV1::RetirePortfolioPairArchives {
                receipt_count,
            } => self.base.retire_portfolio_pair_archives(receipt_count)?,
        };
        let value = Self { base, ..*self };
        value.validate()?;
        Ok(value)
    }

    /// Count the unique exact Dealer child admitted by its authenticated action.
    pub fn admit_dealer_child(&self) -> Result<Self, CodecError> {
        self.apply_base_transition(IndexedSettlementBaseTransitionV1::AdmitDealerChild)
    }

    /// Count the unique exact Dealer child retired by its authenticated action.
    pub fn retire_dealer_child(&self) -> Result<Self, CodecError> {
        self.apply_base_transition(IndexedSettlementBaseTransitionV1::RetireDealerChild)
    }

    /// Count one authenticated materialization and its exact new endpoints.
    pub fn admit_materialization(
        &self,
        owner_rows_created: u8,
        filled_reservations_admitted: u8,
        merge_receipt: bool,
    ) -> Result<Self, CodecError> {
        self.apply_base_transition(IndexedSettlementBaseTransitionV1::AdmitMaterialization {
            owner_rows_created,
            filled_reservations_admitted,
            merge_receipt,
        })
    }

    /// Count one authenticated zero-fill Reservation release.
    pub fn release_unfilled_reservation(&self) -> Result<Self, CodecError> {
        self.apply_base_transition(IndexedSettlementBaseTransitionV1::ReleaseUnfilledReservation)
    }

    /// Count the unique action-37 merge cash-pot creation.
    pub fn activate_merge_cash_pot(&self) -> Result<Self, CodecError> {
        self.apply_base_transition(IndexedSettlementBaseTransitionV1::ActivateMergeCashPot)
    }

    /// Count one authenticated owner finalization and its exact fee-child bit.
    pub fn complete_owner_finalization(
        &self,
        fee_receipt_created: bool,
    ) -> Result<Self, CodecError> {
        self.apply_base_transition(
            IndexedSettlementBaseTransitionV1::CompleteOwnerFinalization {
                fee_receipt_created,
            },
        )
    }

    /// Count one authenticated merge-payment latch completion.
    pub fn complete_merge_payment(&self) -> Result<Self, CodecError> {
        self.apply_base_transition(IndexedSettlementBaseTransitionV1::CompleteMergePayment)
    }

    /// Count one exact terminal Receipt close without changing index identity.
    pub fn retire_one_receipt(&self) -> Result<Self, CodecError> {
        self.apply_base_transition(IndexedSettlementBaseTransitionV1::RetireOneReceipt)
    }

    /// Count one exact finalized owner-row close without changing index identity.
    pub fn retire_one_owner_row(&self) -> Result<Self, CodecError> {
        self.apply_base_transition(IndexedSettlementBaseTransitionV1::RetireOneOwnerRow)
    }

    /// Count one exact terminal Reservation close without changing index identity.
    pub fn retire_one_reservation(&self) -> Result<Self, CodecError> {
        self.apply_base_transition(IndexedSettlementBaseTransitionV1::RetireOneReservation)
    }

    /// Count one exact fee-finalization child close without changing index identity.
    pub fn retire_one_fee_finalization(&self) -> Result<Self, CodecError> {
        self.apply_base_transition(IndexedSettlementBaseTransitionV1::RetireOneFeeFinalization)
    }

    /// Enter dependency-ordered singleton retirement without changing index identity.
    pub fn begin_retiring(&self) -> Result<Self, CodecError> {
        self.apply_base_transition(IndexedSettlementBaseTransitionV1::BeginRetiring)
    }

    /// Retire the exact settlement cash pot without changing index identity.
    pub fn retire_cash_pot(&self) -> Result<Self, CodecError> {
        self.apply_base_transition(IndexedSettlementBaseTransitionV1::RetireCashPot)
    }

    /// Retire the exact Split/Merge FinalPot without changing index identity.
    pub fn retire_final_pot(&self) -> Result<Self, CodecError> {
        self.apply_base_transition(IndexedSettlementBaseTransitionV1::RetireFinalPot)
    }

    /// Retire the exact selected fee record without changing index identity.
    pub fn retire_fee_record(&self) -> Result<Self, CodecError> {
        self.apply_base_transition(IndexedSettlementBaseTransitionV1::RetireFeeRecord)
    }

    /// Count the complete authenticated receipt archive set for one portfolio pair.
    pub fn retire_portfolio_pair_archives(
        &self,
        receipt_count: u8,
    ) -> Result<Self, CodecError> {
        self.apply_base_transition(
            IndexedSettlementBaseTransitionV1::RetirePortfolioPairArchives { receipt_count },
        )
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

    /// Stream the exact post-index-retirement successor and account-bound ID.
    ///
    /// This preserves the retained Feed as the sole remaining readable child
    /// while avoiding a second indexed-root-sized poststate in the runtime
    /// composer. Both index closes and this root write must share one rollback
    /// domain; this structural encoder grants no close authority by itself.
    pub fn encode_retire_index_children_and_data_id<B: Sha256BackendV1>(
        &self,
        backend: &B,
        root_account: Id32,
        output: &mut [u8],
    ) -> Result<Id32, CodecError> {
        self.validate()?;
        if root_account.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if self.state != ExactIndexChildrenStateV1::Live
            || !Self::at_pre_feed_terminal_frontier(&self.base)
        {
            return Err(CodecError::InvalidState);
        }
        let counts = ExactIndexChildCountsV1 {
            expected: INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1,
            admitted: INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1,
            live: 0,
            retired: INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1,
        };
        Self::encode_components(
            &self.base,
            self.locator_account,
            self.adjacency_account,
            self.plane_id,
            self.locator_data_id,
            self.adjacency_data_id,
            self.selected_feed_data_id,
            self.capability_profile_id,
            counts,
            ExactIndexChildrenStateV1::Retired,
            output,
        )?;
        Self::encoded_data_id(backend, root_account, output)
    }

    /// Retire the already-authenticated retained Feed and finish the base root.
    ///
    /// This succeeds only after both exact-index siblings are retired. The SBF
    /// composer must close the same full-body-authenticated Feed and write this
    /// successor atomically; this structural transition is not close authority.
    pub fn retire_feed_and_finish(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if self.state != ExactIndexChildrenStateV1::Retired {
            return Err(CodecError::InvalidState);
        }
        let base = self.base.retire_retained_feed_and_finish()?;
        let value = Self { base, ..*self };
        value.validate()?;
        Ok(value)
    }

    /// Stream the exact post-Feed terminal successor and its account-bound ID.
    ///
    /// This avoids constructing a second indexed-root-sized value in the SBF
    /// composer. Feed account authentication, close credits, and the root write
    /// must still be composed atomically by the runtime adapter.
    pub fn encode_retire_feed_and_finish_and_data_id<B: Sha256BackendV1>(
        &self,
        backend: &B,
        root_account: Id32,
        output: &mut [u8],
    ) -> Result<Id32, CodecError> {
        self.validate()?;
        if root_account.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if self.state != ExactIndexChildrenStateV1::Retired {
            return Err(CodecError::InvalidState);
        }
        let base = self.base.retire_retained_feed_and_finish()?;
        Self::encode_components(
            &base,
            self.locator_account,
            self.adjacency_account,
            self.plane_id,
            self.locator_data_id,
            self.adjacency_data_id,
            self.selected_feed_data_id,
            self.capability_profile_id,
            self.counts,
            self.state,
            output,
        )?;
        Self::encoded_data_id(backend, root_account, output)
    }

    /// Stream one canonical live successor directly into account memory.
    ///
    /// This is byte-for-byte identical to `new_live(...).encode(...)` without
    /// materializing either the 1,228-byte wrapper or a 980-byte base scratch
    /// array in the caller's frame.
    #[allow(clippy::too_many_arguments)]
    pub fn encode_new_live_into(
        base: &SettlementRootV1AccountV1,
        locator_account: Id32,
        adjacency_account: Id32,
        plane_id: Id32,
        locator_data_id: Id32,
        adjacency_data_id: Id32,
        selected_feed_data_id: Id32,
        capability_profile_id: Id32,
        output: &mut [u8],
    ) -> Result<(), CodecError> {
        if base.phase() != SettlementRootPhaseV1::Materializing {
            return Err(CodecError::InvalidState);
        }
        let counts = Self::live_counts();
        Self::validate_components(
            base,
            locator_account,
            adjacency_account,
            plane_id,
            locator_data_id,
            adjacency_data_id,
            selected_feed_data_id,
            capability_profile_id,
            counts,
            ExactIndexChildrenStateV1::Live,
        )?;
        Self::encode_components(
            base,
            locator_account,
            adjacency_account,
            plane_id,
            locator_data_id,
            adjacency_data_id,
            selected_feed_data_id,
            capability_profile_id,
            counts,
            ExactIndexChildrenStateV1::Live,
            output,
        )
    }

    /// Stream one canonical live successor and return its account-key-bound ID.
    ///
    /// This is the sole contract-owned transcript used by the runtime builder;
    /// it does not allocate a second root-sized buffer.
    #[allow(clippy::too_many_arguments)]
    pub fn encode_new_live_and_data_id<B: Sha256BackendV1>(
        base: &SettlementRootV1AccountV1,
        locator_account: Id32,
        adjacency_account: Id32,
        plane_id: Id32,
        locator_data_id: Id32,
        adjacency_data_id: Id32,
        selected_feed_data_id: Id32,
        capability_profile_id: Id32,
        backend: &B,
        root_account: Id32,
        output: &mut [u8],
    ) -> Result<Id32, CodecError> {
        if root_account.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        Self::encode_new_live_into(
            base,
            locator_account,
            adjacency_account,
            plane_id,
            locator_data_id,
            adjacency_data_id,
            selected_feed_data_id,
            capability_profile_id,
            output,
        )?;
        Self::encoded_data_id(backend, root_account, output)
    }

    fn encoded_data_id<B: Sha256BackendV1>(
        backend: &B,
        root_account: Id32,
        encoded: &[u8],
    ) -> Result<Id32, CodecError> {
        if root_account.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if encoded.len() != INDEXED_SETTLEMENT_ROOT_BYTES_V1 {
            return Err(CodecError::WrongLength);
        }
        Id32::new(backend.sha256(&[
            INDEXED_SETTLEMENT_ROOT_DATA_ID_DOMAIN_V1,
            &root_account.bytes(),
            encoded,
        ]))
    }

    /// Encode the exact reserved successor envelope and nested canonical Root V1.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        Self::encode_components(
            &self.base,
            self.locator_account,
            self.adjacency_account,
            self.plane_id,
            self.locator_data_id,
            self.adjacency_data_id,
            self.selected_feed_data_id,
            self.capability_profile_id,
            self.counts,
            self.state,
            output,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_components(
        base: &SettlementRootV1AccountV1,
        locator_account: Id32,
        adjacency_account: Id32,
        plane_id: Id32,
        locator_data_id: Id32,
        adjacency_data_id: Id32,
        selected_feed_data_id: Id32,
        capability_profile_id: Id32,
        counts: ExactIndexChildCountsV1,
        state: ExactIndexChildrenStateV1,
        output: &mut [u8],
    ) -> Result<(), CodecError> {
        if output.len() != INDEXED_SETTLEMENT_ROOT_BYTES_V1 {
            return Err(CodecError::WrongLength);
        }
        output.fill(0);
        let envelope = output
            .get_mut(..INDEXED_SETTLEMENT_ROOT_ENVELOPE_BYTES_V1)
            .ok_or(CodecError::WrongLength)?;
        envelope[0] = INDEXED_SETTLEMENT_ROOT_ACCOUNT_TAG;
        envelope[1] = INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION;
        envelope[2] = state.code();
        base.encode(
            output
                .get_mut(
                    INDEXED_SETTLEMENT_ROOT_ENVELOPE_BYTES_V1
                        ..INDEXED_SETTLEMENT_ROOT_SUFFIX_OFFSET_V1,
                )
                .ok_or(CodecError::WrongLength)?,
        )?;
        let mut writer = Writer::exact(
            output
                .get_mut(INDEXED_SETTLEMENT_ROOT_SUFFIX_OFFSET_V1..)
                .ok_or(CodecError::WrongLength)?,
            INDEXED_SETTLEMENT_ROOT_SUFFIX_BYTES_V1,
        )?;
        for identity in [
            locator_account,
            adjacency_account,
            plane_id,
            locator_data_id,
            adjacency_data_id,
            selected_feed_data_id,
            capability_profile_id,
        ] {
            writer.bytes(&identity.bytes())?;
        }
        for count in [
            counts.expected,
            counts.admitted,
            counts.live,
            counts.retired,
        ] {
            writer.u8(count)?;
        }
        writer.bytes(&[0; 4])?;
        writer.finish()
    }

    /// Decode only the exact reserved successor schema and rerun every invariant.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        if input.len() != INDEXED_SETTLEMENT_ROOT_BYTES_V1 {
            return Err(CodecError::WrongLength);
        }
        let envelope = input
            .get(..INDEXED_SETTLEMENT_ROOT_ENVELOPE_BYTES_V1)
            .ok_or(CodecError::WrongLength)?;
        if envelope[0] != INDEXED_SETTLEMENT_ROOT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if envelope[1] != INDEXED_SETTLEMENT_ROOT_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let state = ExactIndexChildrenStateV1::decode(envelope[2])?;
        if envelope[3..].iter().any(|byte| *byte != 0) {
            return Err(CodecError::NonCanonicalPadding);
        }
        let base = SettlementRootV1AccountV1::decode(
            input
                .get(
                    INDEXED_SETTLEMENT_ROOT_ENVELOPE_BYTES_V1
                        ..INDEXED_SETTLEMENT_ROOT_SUFFIX_OFFSET_V1,
                )
                .ok_or(CodecError::WrongLength)?,
        )?;
        let mut reader = Reader::exact(
            input
                .get(INDEXED_SETTLEMENT_ROOT_SUFFIX_OFFSET_V1..)
                .ok_or(CodecError::WrongLength)?,
            INDEXED_SETTLEMENT_ROOT_SUFFIX_BYTES_V1,
        )?;
        let locator_account = Id32::new(reader.array()?)?;
        let adjacency_account = Id32::new(reader.array()?)?;
        let plane_id = Id32::new(reader.array()?)?;
        let locator_data_id = Id32::new(reader.array()?)?;
        let adjacency_data_id = Id32::new(reader.array()?)?;
        let selected_feed_data_id = Id32::new(reader.array()?)?;
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
            selected_feed_data_id,
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
        Self::encoded_data_id(backend, root_account, &bytes)
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
            selected_feed_data_id: self.selected_feed_data_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_owner_settlement::VirtualCashDirectionV1;
    use sha2::{Digest, Sha256};

    struct Sha2Backend;

    impl Sha256BackendV1 for Sha2Backend {
        fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
            let mut hash = Sha256::new();
            for part in parts {
                hash.update(part);
            }
            hash.finalize().into()
        }
    }

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
            id(27),
        )
        .unwrap()
    }

    fn indexed_with_base(
        base: SettlementRootV1AccountV1,
    ) -> IndexedSettlementRootV1AccountV1 {
        let mut value = live_indexed_root();
        value.base = base;
        value.validate().unwrap();
        value
    }

    fn assert_suffix_preserved(
        before: &IndexedSettlementRootV1AccountV1,
        after: &IndexedSettlementRootV1AccountV1,
    ) {
        assert_eq!(after.locator_account(), before.locator_account());
        assert_eq!(after.adjacency_account(), before.adjacency_account());
        assert_eq!(after.plane_id(), before.plane_id());
        assert_eq!(after.locator_data_id(), before.locator_data_id());
        assert_eq!(after.adjacency_data_id(), before.adjacency_data_id());
        assert_eq!(after.selected_feed_data_id(), before.selected_feed_data_id());
        assert_eq!(after.capability_profile_id(), before.capability_profile_id());
        assert_eq!(after.index_counts(), before.index_counts());
        assert_eq!(after.index_state(), before.index_state());
    }

    fn close_two_scalar_children(
        value: IndexedSettlementRootV1AccountV1,
    ) -> IndexedSettlementRootV1AccountV1 {
        let next = value.retire_one_receipt().unwrap();
        assert_suffix_preserved(&value, &next);
        let value = next;
        let next = value.retire_one_receipt().unwrap();
        assert_suffix_preserved(&value, &next);
        let value = next;
        let next = value.retire_one_reservation().unwrap();
        assert_suffix_preserved(&value, &next);
        let value = next;
        let next = value.retire_one_reservation().unwrap();
        assert_suffix_preserved(&value, &next);
        let value = next;
        let next = value.retire_one_owner_row().unwrap();
        assert_suffix_preserved(&value, &next);
        let value = next;
        let next = value.retire_one_owner_row().unwrap();
        assert_suffix_preserved(&value, &next);
        next
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
    fn streamed_live_encoder_is_byte_exact_without_root_scratch_values() {
        let indexed = live_indexed_root();
        let mut ordinary = [0u8; INDEXED_SETTLEMENT_ROOT_BYTES_V1];
        indexed.encode(&mut ordinary).unwrap();
        let mut streamed = [0u8; INDEXED_SETTLEMENT_ROOT_BYTES_V1];
        IndexedSettlementRootV1AccountV1::encode_new_live_into(
            indexed.base(),
            indexed.locator_account(),
            indexed.adjacency_account(),
            indexed.plane_id(),
            indexed.locator_data_id(),
            indexed.adjacency_data_id(),
            indexed.selected_feed_data_id(),
            indexed.capability_profile_id(),
            &mut streamed,
        )
        .unwrap();
        assert_eq!(streamed, ordinary);
        assert_eq!(IndexedSettlementRootV1AccountV1::decode(&streamed), Ok(indexed));
        let root_account = id(90);
        let ordinary_id = indexed.data_id(&Sha2Backend, root_account).unwrap();
        let streamed_id = IndexedSettlementRootV1AccountV1::encode_new_live_and_data_id(
            indexed.base(),
            indexed.locator_account(),
            indexed.adjacency_account(),
            indexed.plane_id(),
            indexed.locator_data_id(),
            indexed.adjacency_data_id(),
            indexed.selected_feed_data_id(),
            indexed.capability_profile_id(),
            &Sha2Backend,
            root_account,
            &mut streamed,
        )
        .unwrap();
        let mut transcript = Sha256::new();
        transcript.update(INDEXED_SETTLEMENT_ROOT_DATA_ID_DOMAIN_V1);
        transcript.update(root_account.bytes());
        transcript.update(ordinary);
        assert_eq!(streamed_id, ordinary_id);
        assert_eq!(streamed_id, Id32::new(transcript.finalize().into()).unwrap());
        assert_eq!(
            IndexedSettlementRootV1AccountV1::encode_new_live_into(
                indexed.base(),
                indexed.locator_account(),
                indexed.adjacency_account(),
                indexed.plane_id(),
                indexed.locator_data_id(),
                indexed.adjacency_data_id(),
                indexed.selected_feed_data_id(),
                indexed.capability_profile_id(),
                &mut streamed[..INDEXED_SETTLEMENT_ROOT_BYTES_V1 - 1],
            ),
            Err(CodecError::WrongLength),
        );
        let later = crate::settlement_root::tests::portfolio_settling_root();
        assert_eq!(
            IndexedSettlementRootV1AccountV1::encode_new_live_into(
                &later,
                indexed.locator_account(),
                indexed.adjacency_account(),
                indexed.plane_id(),
                indexed.locator_data_id(),
                indexed.adjacency_data_id(),
                indexed.selected_feed_data_id(),
                indexed.capability_profile_id(),
                &mut streamed,
            ),
            Err(CodecError::InvalidState),
        );
        let before_zero_root = streamed;
        assert_eq!(
            IndexedSettlementRootV1AccountV1::encode_new_live_and_data_id(
                indexed.base(),
                indexed.locator_account(),
                indexed.adjacency_account(),
                indexed.plane_id(),
                indexed.locator_data_id(),
                indexed.adjacency_data_id(),
                indexed.selected_feed_data_id(),
                indexed.capability_profile_id(),
                &Sha2Backend,
                Id32::ZERO,
                &mut streamed,
            ),
            Err(CodecError::ZeroIdentity),
        );
        assert_eq!(streamed, before_zero_root);
    }

    #[test]
    fn compact_rent_receipt_authenticates_source_and_streams_exact_postrent_root() {
        let base_before = crate::settlement_root::tests::materializing_root();
        let root_account = id(20);
        let preparation = prepare_indexed_settlement_root_upgrade_rent_v1(
            &base_before,
            root_account,
            110,
            150,
            50,
            id(27),
            &Sha2Backend,
        )
        .unwrap();
        assert!(core::mem::size_of_val(&preparation) <= 320);
        assert_eq!(
            preparation.base_before_data_id(),
            base_before.data_id(&Sha2Backend, root_account).unwrap(),
        );
        let base_after = base_before
            .with_indexed_root_rent(preparation.rent_after())
            .unwrap();
        assert_eq!(
            preparation.base_after_data_id(),
            base_after.data_id(&Sha2Backend, root_account).unwrap(),
        );
        let expected = IndexedSettlementRootV1AccountV1::new_live(
            base_after,
            id(21),
            id(22),
            id(23),
            id(24),
            id(25),
            id(26),
            id(27),
        )
        .unwrap();
        let authority = preparation
            .authenticate_source(&base_before, &Sha2Backend)
            .unwrap();
        assert_eq!(authority.base_before(), &base_before);
        let mut streamed = [0u8; INDEXED_SETTLEMENT_ROOT_BYTES_V1];
        let streamed_id = authority
            .encode_new_live_and_data_id(
                id(21),
                id(22),
                id(23),
                id(24),
                id(25),
                id(26),
                id(27),
                &Sha2Backend,
                &mut streamed,
            )
            .unwrap();
        let mut expected_bytes = [0u8; INDEXED_SETTLEMENT_ROOT_BYTES_V1];
        expected.encode(&mut expected_bytes).unwrap();
        assert_eq!(streamed, expected_bytes);
        assert_eq!(
            streamed_id,
            expected.data_id(&Sha2Backend, root_account).unwrap(),
        );

        let wrong_preparation = prepare_indexed_settlement_root_upgrade_rent_v1(
            &base_before,
            root_account,
            110,
            150,
            50,
            id(27),
            &Sha2Backend,
        )
        .unwrap();
        assert!(matches!(
            wrong_preparation.authenticate_source(&base_after, &Sha2Backend),
            Err(CodecError::MismatchedBinding),
        ));
    }

    #[test]
    fn named_indexed_transitions_preserve_exact_children_and_match_base_successors() {
        let indexed = live_indexed_root();
        let expected_first = indexed
            .base()
            .admit_materialization_delta(2, 2, false)
            .unwrap();
        let first = indexed.admit_materialization(2, 2, false).unwrap();
        assert_eq!(first.base(), &expected_first);
        assert_eq!(first.locator_account(), indexed.locator_account());
        assert_eq!(first.adjacency_account(), indexed.adjacency_account());
        assert_eq!(first.plane_id(), indexed.plane_id());
        assert_eq!(first.index_counts(), indexed.index_counts());
        assert_eq!(first.index_state(), ExactIndexChildrenStateV1::Live);

        let expected_second = expected_first
            .admit_materialization_delta(0, 0, false)
            .unwrap();
        let second = first.admit_materialization(0, 0, false).unwrap();
        assert_eq!(second.base(), &expected_second);
        assert_eq!(second.index_counts(), indexed.index_counts());
        assert_eq!(second.index_state(), ExactIndexChildrenStateV1::Live);
    }

    #[test]
    fn indexed_retirement_wrappers_preserve_suffix_across_every_root_family() {
        let scalar = close_two_scalar_children(indexed_with_base(
            crate::settlement_root::tests::portfolio_settling_root(),
        ));
        let retiring = scalar.begin_retiring().unwrap();
        assert_suffix_preserved(&scalar, &retiring);
        let cash_retired = retiring.retire_cash_pot().unwrap();
        assert_suffix_preserved(&retiring, &cash_retired);
        assert!(cash_retired.base().at_retained_feed_retirement_frontier());

        let fee = close_two_scalar_children(indexed_with_base(
            crate::settlement_root::tests::fee_settling_root(),
        ));
        let next = fee.retire_one_fee_finalization().unwrap();
        assert_suffix_preserved(&fee, &next);
        let fee = next.retire_one_fee_finalization().unwrap();
        assert_suffix_preserved(&next, &fee);
        let next = fee.begin_retiring().unwrap();
        assert_suffix_preserved(&fee, &next);
        assert_eq!(next.retire_cash_pot(), Err(CodecError::InvalidState));
        let fee = next.retire_fee_record().unwrap();
        assert_suffix_preserved(&next, &fee);
        let next = fee.retire_cash_pot().unwrap();
        assert_suffix_preserved(&fee, &next);
        assert!(next.base().at_retained_feed_retirement_frontier());

        for direction in [VirtualCashDirectionV1::Split, VirtualCashDirectionV1::Merge] {
            let virtual_root = close_two_scalar_children(indexed_with_base(
                crate::settlement_root::tests::virtual_root(direction),
            ));
            let next = virtual_root.begin_retiring().unwrap();
            assert_suffix_preserved(&virtual_root, &next);
            let virtual_root = next.retire_cash_pot().unwrap();
            assert_suffix_preserved(&next, &virtual_root);
            let next = virtual_root.retire_final_pot().unwrap();
            assert_suffix_preserved(&virtual_root, &next);
            assert!(next.base().at_retained_feed_retirement_frontier());
        }

        let dealer = close_two_scalar_children(indexed_with_base(
            crate::settlement_root::tests::dealer_settling_root(),
        ));
        let next = dealer.begin_retiring().unwrap();
        assert_suffix_preserved(&dealer, &next);
        let dealer = next.retire_cash_pot().unwrap();
        assert_suffix_preserved(&next, &dealer);
        let next = dealer.retire_dealer_child().unwrap();
        assert_suffix_preserved(&dealer, &next);
        assert!(next.base().at_retained_feed_retirement_frontier());

        let portfolio = indexed_with_base(
            crate::settlement_root::tests::portfolio_settling_root(),
        );
        let next = portfolio.retire_portfolio_pair_archives(2).unwrap();
        assert_suffix_preserved(&portfolio, &next);
        assert_eq!(next.base().phase(), SettlementRootPhaseV1::Settling);
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
        let root_account = id(90);
        let mut streamed_retired = [0u8; INDEXED_SETTLEMENT_ROOT_BYTES_V1];
        let streamed_retired_id = frontier
            .encode_retire_index_children_and_data_id(
                &Sha2Backend,
                root_account,
                &mut streamed_retired,
            )
            .unwrap();
        let mut ordinary_retired = [0u8; INDEXED_SETTLEMENT_ROOT_BYTES_V1];
        retired.encode(&mut ordinary_retired).unwrap();
        assert_eq!(streamed_retired, ordinary_retired);
        assert_eq!(
            streamed_retired_id,
            retired.data_id(&Sha2Backend, root_account).unwrap(),
        );
        let terminal = retired.retire_feed_and_finish().unwrap();
        assert_eq!(terminal.base(), &crate::settlement_root::tests::terminal_root());
        terminal.validate().unwrap();
        assert_eq!(terminal.selected_feed_data_id(), retired.selected_feed_data_id());
        let mut streamed_terminal = [0u8; INDEXED_SETTLEMENT_ROOT_BYTES_V1];
        let streamed_terminal_id = retired
            .encode_retire_feed_and_finish_and_data_id(
                &Sha2Backend,
                root_account,
                &mut streamed_terminal,
            )
            .unwrap();
        let mut ordinary_terminal = [0u8; INDEXED_SETTLEMENT_ROOT_BYTES_V1];
        terminal.encode(&mut ordinary_terminal).unwrap();
        assert_eq!(streamed_terminal, ordinary_terminal);
        assert_eq!(
            streamed_terminal_id,
            terminal.data_id(&Sha2Backend, root_account).unwrap(),
        );
        let close = IndexedSettlementRootV1AccountV1::decode_terminal_close_projection(
            &Sha2Backend,
            root_account,
            &ordinary_terminal,
        )
        .unwrap();
        assert_eq!(
            close.terminal(),
            &terminal.terminal_projection(&Sha2Backend, root_account).unwrap(),
        );
        assert_eq!(close.root_rent(), terminal.base().root_rent());
        assert_eq!(close.fee_record(), terminal.base().fee_record());
        assert_eq!(close.market_binding(), terminal.base().market_binding());
        assert_eq!(close.stored_bump(), terminal.base().stored_bump());
        let epoch_account = terminal.base().epoch();
        let epoch = GeneralEpochV6AccountV1 {
            market_binding: terminal.base().market_binding(),
            market_runtime: terminal.base().market(),
            market_instance_v2_id: terminal.base().market_instance_v2_id(),
            economic_domain: id(91),
            window: terminal.base().window(),
            budget: id(92),
            order_set: terminal.base().order_set(),
            epoch_index: 1,
            generation: terminal.base().epoch_generation(),
            freeze_deadline_slot: 1,
            frozen_slot: 1,
            candidate_bundle_count: 0,
            work_count: 0,
            selected_candidate_count: 1,
            rent: DeletableRentOwnerV1 {
                payer: id(93),
                refundable_principal: 7,
                donation_floor: 0,
            },
            phase: GeneralEpochPhaseV1::Finalized,
            stored_bump: 1,
            flags: 0,
        };
        let window = CandidateWindowV5AccountV1::new(crate::CandidateWindowV4AccountV1 {
            epoch: epoch_account,
            market: terminal.base().market(),
            relation_policy_id: id(94),
            admission_policy_id: id(95),
            score_policy_id: terminal.base().score_policy_id(),
            freeze_deadline_slot: 1,
            frozen_slot: 1,
            reveal_opens_slot: 2,
            submission_closes_slot: 3,
            verification_closes_slot: 4,
            finalized_slot: 4,
            admission_head: Id32::ZERO,
            best_candidate_node: Id32::ZERO,
            best_settlement_candidate_id: Id32::ZERO,
            selected_candidate_artifact: root_account,
            best_rank_key: [0; crate::SCORE_V2_Q_RANK_CAPACITY],
            admitted_count: 1,
            revealed_count: 1,
            verdict_count: 1,
            valid_verdict_count: 1,
            expired_commitment_count: 0,
            expired_unverified_count: 0,
            live_node_count: 0,
            closed_node_count: 1,
            best_ordinal: 0,
            epoch_generation: terminal.base().epoch_generation(),
            rent: DeletableRentOwnerV1 {
                payer: id(96),
                refundable_principal: 8,
                donation_floor: 0,
            },
            rank_key_len: crate::SCORE_V2_Q_COST_ACTIVE_RANK_BYTES as u8,
            stored_bump: 2,
            flags: 0,
        })
        .unwrap();
        let mut epoch_after = [0u8; crate::GENERAL_EPOCH_ACCOUNT_BYTES];
        encode_retire_indexed_settlement_root_v1(
            &close,
            epoch_account,
            &epoch,
            terminal.base().window(),
            &window,
            &mut epoch_after,
        )
        .unwrap();
        let epoch_after = GeneralEpochV6AccountV1::decode(&epoch_after).unwrap();
        assert_eq!(epoch_after.selected_candidate_count, 0);
        let wrong_window = CandidateWindowV5AccountV1::new(crate::CandidateWindowV4AccountV1 {
            selected_candidate_artifact: id(97),
            ..*window.base()
        })
        .unwrap();
        let mut wrong_epoch_after = [0u8; crate::GENERAL_EPOCH_ACCOUNT_BYTES];
        assert!(encode_retire_indexed_settlement_root_v1(
            &close,
            epoch_account,
            &epoch,
            terminal.base().window(),
            &wrong_window,
            &mut wrong_epoch_after,
        )
        .is_err());
        let mut nonterminal = ordinary_terminal;
        nonterminal[2] = ExactIndexChildrenStateV1::Live.code();
        assert!(IndexedSettlementRootV1AccountV1::decode_terminal_close_projection(
            &Sha2Backend,
            root_account,
            &nonterminal,
        )
        .is_err());

        assert_eq!(
            frontier.retire_feed_and_finish(),
            Err(CodecError::InvalidState),
        );
        assert_eq!(
            terminal.retire_feed_and_finish(),
            Err(CodecError::InvalidState),
        );

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
