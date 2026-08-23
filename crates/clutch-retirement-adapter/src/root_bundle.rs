// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_retirement::{
    EpochChildKindV1, GeneralEpochPhaseV2, Identity32V1, LiveGeneralEpochProjectionV2,
    RetirementErrorV2,
};

use crate::{
    project_live_general_epoch_retirement_v2, AuthenticatedAccountV2, GeneralEpochAccountV5,
    RetirementAdapterErrorV2,
};

/// Frozen number of independently counted general-Epoch child classes.
pub const EPOCH_CHILD_CLASS_CAPACITY_V1: usize = 9;

const ALL_CLASS_BITS_V1: u16 = (1u16 << EPOCH_CHILD_CLASS_CAPACITY_V1) - 1;

/// Adapter-issued evidence that one authoritative child-family owner observed
/// its exact class terminal and empty for one parent Epoch generation.
///
/// This is a narrow inter-adapter trust-boundary type. The constructor does
/// not decode any child family: the family-specific adapter must first check
/// its exact owner, PDA, codec, parent identities, terminal economics, and
/// absence/zero-live-count rule. A client projection must never be passed to
/// this constructor. Keeping the fields private prevents a caller from
/// changing the class or parent after that validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedEpochChildClassV1 {
    kind: EpochChildKindV1,
    market: Identity32V1,
    epoch: Identity32V1,
    epoch_generation: u64,
    authenticated_live_count: u32,
}

impl AuthenticatedEpochChildClassV1 {
    /// Mint one class witness only after its authoritative semantic and
    /// runtime adapter has validated terminality and an empty live set.
    ///
    /// A nonzero generation and zero authenticated live count are enforced
    /// here. Parent and class exhaustiveness are checked again by the root
    /// join, against the independently authenticated Epoch V5 bytes.
    pub const fn after_authoritative_terminal_empty_validation(
        kind: EpochChildKindV1,
        market: Identity32V1,
        epoch: Identity32V1,
        epoch_generation: u64,
        authenticated_live_count: u32,
    ) -> Result<Self, RetirementAdapterErrorV2> {
        if epoch_generation == 0 {
            return Err(RetirementAdapterErrorV2::Retirement(
                RetirementErrorV2::WrongGeneration,
            ));
        }
        if authenticated_live_count != 0 {
            return Err(RetirementAdapterErrorV2::Retirement(
                RetirementErrorV2::ChildOutstanding,
            ));
        }
        Ok(Self {
            kind,
            market,
            epoch,
            epoch_generation,
            authenticated_live_count,
        })
    }

    /// Exact counter class validated by the owning adapter.
    pub const fn kind(self) -> EpochChildKindV1 {
        self.kind
    }

    /// Parent Market identity validated by the owning adapter.
    pub const fn market(self) -> Identity32V1 {
        self.market
    }

    /// Parent semantic Epoch identity validated by the owning adapter.
    pub const fn epoch(self) -> Identity32V1 {
        self.epoch
    }

    /// Parent Epoch generation validated by the owning adapter.
    pub const fn epoch_generation(self) -> u64 {
        self.epoch_generation
    }
}

/// Private-field result of joining one authenticated Epoch V5 with exactly
/// one terminal-empty witness for every frozen child class.
///
/// This proves the adapter contract only. It is not a root-close capability:
/// Window admission-ledger authentication, Budget terminal disposition,
/// neutral-sink provenance, rent effects, and SBF rollback remain separate
/// mandatory boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedTerminalEpochRootBundleV1 {
    epoch_account: Identity32V1,
    epoch: LiveGeneralEpochProjectionV2,
    class_bits: u16,
}

impl AuthenticatedTerminalEpochRootBundleV1 {
    /// Canonical runtime address of the authenticated Epoch V5 account.
    pub const fn epoch_account(self) -> Identity32V1 {
        self.epoch_account
    }

    /// Exact decoded parent state bound to all nine class witnesses.
    pub const fn epoch(self) -> LiveGeneralEpochProjectionV2 {
        self.epoch
    }

    /// Number of distinct frozen child classes joined into this witness.
    pub const fn authenticated_class_count(self) -> usize {
        if self.class_bits == ALL_CLASS_BITS_V1 {
            EPOCH_CHILD_CLASS_CAPACITY_V1
        } else {
            0
        }
    }
}

const fn class_bit(kind: EpochChildKindV1) -> u16 {
    match kind {
        EpochChildKindV1::CandidateBundle => 1 << 0,
        EpochChildKindV1::CandidateIndexPage => 1 << 1,
        EpochChildKindV1::CandidateVerdict => 1 << 2,
        EpochChildKindV1::CandidateEscrow => 1 << 3,
        EpochChildKindV1::ClearWorkBundle => 1 << 4,
        EpochChildKindV1::OrderPage => 1 << 5,
        EpochChildKindV1::ReservationArchive => 1 << 6,
        EpochChildKindV1::SettlementReceipt => 1 << 7,
        EpochChildKindV1::FinalPot => 1 << 8,
    }
}

/// Decode one exact authenticated Epoch V5 and exhaustively join all nine
/// terminal-empty child-family witnesses.
///
/// The fixed array forbids variable-length or caller-truncated evidence. The
/// join additionally rejects duplicate classes, which makes omission
/// impossible: nine entries, nine valid enum values, and nine distinct bits
/// must equal the frozen complete mask. Every witness is cross-bound to the
/// authenticated Market, semantic Epoch, and generation. The Epoch itself
/// must be SETTLED or LAPSED and every authoritative count word must be zero.
pub fn authenticate_terminal_epoch_root_bundle_v1(
    authenticated_epoch: AuthenticatedAccountV2<'_>,
    classes: [AuthenticatedEpochChildClassV1; EPOCH_CHILD_CLASS_CAPACITY_V1],
) -> Result<AuthenticatedTerminalEpochRootBundleV1, RetirementAdapterErrorV2> {
    let epoch = project_live_general_epoch_retirement_v2(GeneralEpochAccountV5::decode(
        authenticated_epoch.data(),
    )?)?;
    if !matches!(
        epoch.phase,
        GeneralEpochPhaseV2::Settled | GeneralEpochPhaseV2::Lapsed
    ) {
        return Err(RetirementErrorV2::WrongPhase.into());
    }

    let mut class_bits = 0u16;
    for class in classes {
        if class.market != epoch.market || class.epoch != epoch.epoch {
            return Err(RetirementErrorV2::WrongParent.into());
        }
        if class.epoch_generation != epoch.retirement.epoch_generation {
            return Err(RetirementErrorV2::WrongGeneration.into());
        }

        let bit = class_bit(class.kind);
        if class_bits & bit != 0 {
            return Err(RetirementErrorV2::WrongChildKind.into());
        }
        class_bits |= bit;

        let root_count = epoch.retirement.children.get(class.kind);
        if root_count != 0 || class.authenticated_live_count != 0 {
            return Err(RetirementErrorV2::ChildOutstanding.into());
        }
    }

    if class_bits != ALL_CLASS_BITS_V1 {
        return Err(RetirementErrorV2::WrongChildKind.into());
    }
    if !epoch.retirement.children.is_zero() {
        return Err(RetirementErrorV2::ChildOutstanding.into());
    }

    Ok(AuthenticatedTerminalEpochRootBundleV1 {
        epoch_account: authenticated_epoch.address(),
        epoch,
        class_bits,
    })
}

const _: () = assert!(EPOCH_CHILD_CLASS_CAPACITY_V1 == 9);
const _: () = assert!(ALL_CLASS_BITS_V1 == 0x01ff);
