// SPDX-License-Identifier: AGPL-3.0-or-later

//! Disabled counted SettlementRoot successor for the exact index plane.
//!
//! The live Root V1 cannot count the locator and adjacency accounts. This
//! breaking wrapper retains the exact Root V1 body and makes those two accounts
//! explicit expected/admitted/live/retired children. It has no Solana account
//! discriminator, seed, action, or capability today: its eight-byte magic is a
//! reviewable pure-codec envelope, not a deployable route allocation.

use crate::{
    prepare_activate_merge_cash_pot_v1, CodecError, Id32, Reader, SettlementRootPhaseV1,
    SettlementRootTerminalProjectionV1, SettlementRootV1AccountV1, Sha256BackendV1, Writer,
    SETTLEMENT_ROOT_ACCOUNT_BYTES,
};

/// Exact disabled successor magic. This is not a live Solana discriminator.
pub const INDEXED_SETTLEMENT_ROOT_MAGIC_V1: [u8; 8] = *b"DCIXRT01";
/// Exact disabled successor schema.
pub const INDEXED_SETTLEMENT_ROOT_SCHEMA_V1: u8 = 1;
/// Exactly the locator and adjacency siblings are counted.
pub const INDEXED_SETTLEMENT_ROOT_EXPECTED_CHILDREN_V1: u8 = 2;
/// Exact active successor width.
pub const INDEXED_SETTLEMENT_ROOT_BYTES_V1: usize =
    16 + SETTLEMENT_ROOT_ACCOUNT_BYTES + (6 * 32) + 8;
/// Account-key-bound data identity domain for the complete successor bytes.
pub const INDEXED_SETTLEMENT_ROOT_DATA_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-v2/indexed-settlement-root-data/v1\0";

const _: () = assert!(SETTLEMENT_ROOT_ACCOUNT_BYTES == 980);
const _: () = assert!(INDEXED_SETTLEMENT_ROOT_BYTES_V1 == 1_196);

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
    /// Activate the expected merge cash pot.
    ActivateMergeCashPot,
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
        let physical = [
            self.locator_account,
            self.adjacency_account,
            self.base.market(),
            self.base.epoch(),
            self.base.market_binding(),
            self.base.retained_feed(),
        ];
        left = 0;
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
        if self.state == ExactIndexChildrenStateV1::Retired
            && self.base.phase() != SettlementRootPhaseV1::Terminal
        {
            return Err(CodecError::InvalidState);
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
            IndexedSettlementBaseTransitionV1::ActivateMergeCashPot => {
                *prepare_activate_merge_cash_pot_v1(&self.base)?.root()
            }
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

    /// Atomically count both live siblings retired after the base graph is terminal.
    ///
    /// The runtime must close both exact accounts, transfer both rent principals
    /// and donations, and write this successor in one rollback domain.
    pub fn retire_index_children(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if self.state != ExactIndexChildrenStateV1::Live
            || self.base.phase() != SettlementRootPhaseV1::Terminal
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

    /// Encode the exact disabled successor envelope and nested canonical Root V1.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut base = [0u8; SETTLEMENT_ROOT_ACCOUNT_BYTES];
        self.base.encode(&mut base)?;
        let mut writer = Writer::exact(output, INDEXED_SETTLEMENT_ROOT_BYTES_V1)?;
        writer.bytes(&INDEXED_SETTLEMENT_ROOT_MAGIC_V1)?;
        writer.u8(INDEXED_SETTLEMENT_ROOT_SCHEMA_V1)?;
        writer.u8(self.state.code())?;
        writer.u8(0)?;
        writer.bytes(&[0; 5])?;
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

    /// Decode only the exact disabled successor schema and rerun every invariant.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, INDEXED_SETTLEMENT_ROOT_BYTES_V1)?;
        if reader.array::<8>()? != INDEXED_SETTLEMENT_ROOT_MAGIC_V1 {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != INDEXED_SETTLEMENT_ROOT_SCHEMA_V1 {
            return Err(CodecError::WrongVersion);
        }
        let state = ExactIndexChildrenStateV1::decode(reader.u8()?)?;
        if reader.u8()? != 0 || reader.array::<5>()? != [0; 5] {
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
    fn codec_refuses_historical_width_magic_version_and_reserved_bytes() {
        assert_eq!(
            IndexedSettlementRootV1AccountV1::decode(&[0; INDEXED_SETTLEMENT_ROOT_BYTES_V1 - 1]),
            Err(CodecError::WrongLength)
        );
        let mut bytes = [0u8; INDEXED_SETTLEMENT_ROOT_BYTES_V1];
        assert_eq!(
            IndexedSettlementRootV1AccountV1::decode(&bytes),
            Err(CodecError::WrongTag)
        );
        bytes[..8].copy_from_slice(&INDEXED_SETTLEMENT_ROOT_MAGIC_V1);
        bytes[8] = 2;
        assert_eq!(
            IndexedSettlementRootV1AccountV1::decode(&bytes),
            Err(CodecError::WrongVersion)
        );
        bytes[8] = INDEXED_SETTLEMENT_ROOT_SCHEMA_V1;
        bytes[9] = 1;
        bytes[11] = 1;
        assert_eq!(
            IndexedSettlementRootV1AccountV1::decode(&bytes),
            Err(CodecError::NonCanonicalPadding)
        );
    }
}
