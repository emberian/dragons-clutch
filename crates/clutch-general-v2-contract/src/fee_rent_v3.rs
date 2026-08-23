// SPDX-License-Identifier: AGPL-3.0-or-later

//! Present-funded rent transition for current owner fee accounts.
//!
//! The current 0x83/v3 carry and 0x84/v2 payer snapshot embed the canonical
//! [`DeletableRentOwnerV1`]. This module derives their one atomic action-38
//! realloc/close disposition from those persisted facts and observed native-
//! lamport balances. It never treats fee atoms, Hoard principal, collateral,
//! future revenue, or a liveness compartment as native rent funding.

use clutch_fee_runtime_contract::terminal::OwnerFeeRentDispositionV2;
use clutch_fee_runtime_contract::Id as FeeId;
use clutch_fee_runtime_contract::codec::{
    FEE_RECORD_ACCOUNT_V1_BYTES, FEE_RECORD_MAGIC_V1, OWNER_FEE_CARRY_ACCOUNT_V1_BYTES,
    OWNER_FEE_CARRY_MAGIC_V1, PAYER_ALLOCATION_ACCOUNT_V1_BYTES, PAYER_ALLOCATION_MAGIC_V1,
    RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES, RECIPIENT_ALLOCATION_MAGIC_V1,
};
use clutch_fee_runtime_contract::terminal::{
    OWNER_FEE_FINALIZATION_BODY_V2_BYTES, OWNER_FEE_FINALIZATION_MAGIC_V2,
    OWNER_FEE_FINALIZATION_VERSION_V2,
};

use crate::{
    CodecError, DeletableRentOwnerV1, Id32, Sha256BackendV1,
    OWNER_FEE_FINALIZATION_ACCOUNT_BYTES_V4,
};

/// Data-ID domain for the rent-owned fee-account transition.
pub const OWNER_FEE_RENT_TRANSITION_DATA_ID_DOMAIN_V3: &[u8] =
    b"dragons-clutch/owner-fee-rent-transition/v3\0";
/// Content domain for the exact reviewed fee semantic schema bundle.
pub const FEE_RUNTIME_SEMANTIC_RELEASE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fee-runtime-semantic-release/v1\0";
/// Full-outer data-ID domain for the exact rent-owned terminal carry account.
pub const OWNER_FEE_FINALIZATION_ACCOUNT_DATA_ID_DOMAIN_V4: &[u8] =
    b"dragons-clutch/owner-fee-finalization-account-data/v4\0";

/// Hash the exact hostile-byte-authenticated 548-byte 0x83/v4 outer account.
pub fn owner_fee_finalization_account_data_id_v4<B: Sha256BackendV1>(
    bytes: &[u8],
    backend: &B,
) -> Result<Id32, CodecError> {
    if bytes.len() != OWNER_FEE_FINALIZATION_ACCOUNT_BYTES_V4 {
        return Err(CodecError::WrongLength);
    }
    Id32::new(backend.sha256(&[
        OWNER_FEE_FINALIZATION_ACCOUNT_DATA_ID_DOMAIN_V4,
        bytes,
    ]))
}

/// Derive the exact fee semantic schema release committed by terminal state.
///
/// This identifies the fixed fee bodies and terminal rounding contract, not
/// an SBF ELF, deployment, URL, or official release manifest. It is derived
/// internally so no caller can substitute a free-form release identity.
pub fn fee_runtime_semantic_release_id_v1<B: Sha256BackendV1>(
    backend: &B,
) -> Result<Id32, CodecError> {
    let fee_record_bytes = u64::try_from(FEE_RECORD_ACCOUNT_V1_BYTES)
        .map_err(|_| CodecError::ArithmeticOverflow)?
        .to_le_bytes();
    let carry_bytes = u64::try_from(OWNER_FEE_CARRY_ACCOUNT_V1_BYTES)
        .map_err(|_| CodecError::ArithmeticOverflow)?
        .to_le_bytes();
    let payer_bytes = u64::try_from(PAYER_ALLOCATION_ACCOUNT_V1_BYTES)
        .map_err(|_| CodecError::ArithmeticOverflow)?
        .to_le_bytes();
    let recipient_bytes = u64::try_from(RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES)
        .map_err(|_| CodecError::ArithmeticOverflow)?
        .to_le_bytes();
    let terminal_version = OWNER_FEE_FINALIZATION_VERSION_V2.to_le_bytes();
    let terminal_bytes = u64::try_from(OWNER_FEE_FINALIZATION_BODY_V2_BYTES)
        .map_err(|_| CodecError::ArithmeticOverflow)?
        .to_le_bytes();
    Id32::new(backend.sha256(&[
        FEE_RUNTIME_SEMANTIC_RELEASE_DOMAIN_V1,
        &FEE_RECORD_MAGIC_V1,
        &fee_record_bytes,
        &OWNER_FEE_CARRY_MAGIC_V1,
        &carry_bytes,
        &PAYER_ALLOCATION_MAGIC_V1,
        &payer_bytes,
        &RECIPIENT_ALLOCATION_MAGIC_V1,
        &recipient_bytes,
        &OWNER_FEE_FINALIZATION_MAGIC_V2,
        &terminal_version,
        &terminal_bytes,
        b"terminal-owner-ceil/canonical-owner-rows/exact-u128-carry",
    ]))
}

/// Exact named identities for one carry realloc and payer close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeRentTransitionAccountsV3 {
    /// Existing 0x83/v3 carry PDA, rewritten in place as 0x83/v4.
    pub carry_account: Id32,
    /// Existing 0x84/v2 payer-allocation PDA, closed atomically.
    pub payer_allocation_account: Id32,
    /// Realm/MarketBinding-derived neutral native-lamport sink.
    pub neutral_sink: Id32,
}

/// One exact native-lamport movement owned by the rent transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeRentTransferV3 {
    source: Id32,
    destination: Id32,
    lamports: u64,
}

impl OwnerFeeRentTransferV3 {
    /// Debited native-lamport account.
    pub const fn source(&self) -> Id32 {
        self.source
    }

    /// Credited native-lamport account.
    pub const fn destination(&self) -> Id32 {
        self.destination
    }

    /// Exact native lamports moved; zero is a canonical no-op.
    pub const fn lamports(&self) -> u64 {
        self.lamports
    }
}

/// Private-construction exact realloc/close plan for current fee accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeRentTransitionPlanV3 {
    semantic: OwnerFeeRentDispositionV2,
    carry_rent_after: DeletableRentOwnerV1,
    carry_top_up: OwnerFeeRentTransferV3,
    payer_principal_refund: OwnerFeeRentTransferV3,
    payer_donation_credit: OwnerFeeRentTransferV3,
}

impl OwnerFeeRentTransitionPlanV3 {
    /// Exact fee-runtime receipt binding derived from persisted rent state.
    pub const fn semantic(&self) -> OwnerFeeRentDispositionV2 {
        self.semantic
    }

    /// Persisted rent compartment for the terminal 0x83/v4 successor.
    pub const fn carry_rent_after(&self) -> DeletableRentOwnerV1 {
        self.carry_rent_after
    }

    /// Exact realloc top-up from the persisted carry rent payer.
    pub const fn carry_top_up(&self) -> OwnerFeeRentTransferV3 {
        self.carry_top_up
    }

    /// Exact payer-account principal refund.
    pub const fn payer_principal_refund(&self) -> OwnerFeeRentTransferV3 {
        self.payer_principal_refund
    }

    /// Exact payer-account donation and hostile-surplus disposition.
    pub const fn payer_donation_credit(&self) -> OwnerFeeRentTransferV3 {
        self.payer_donation_credit
    }

    /// Expected carry balance after the exact realloc top-up.
    pub const fn carry_balance_after_lamports(&self) -> u64 {
        self.semantic.carry_balance_after_lamports
    }

    /// Exact payer balance consumed by its principal/donation split.
    pub const fn payer_balance_before_lamports(&self) -> u64 {
        self.semantic.payer_balance_before_lamports
    }

    /// Canonical transition data identity persisted by the terminal receipt.
    pub const fn data_id(&self) -> Id32 {
        Id32::from_bytes(self.semantic.data_id.0)
    }
}

fn require_live_distinct(values: &[Id32]) -> Result<(), CodecError> {
    let mut left = 0usize;
    while left < values.len() {
        if values[left].is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        let mut right = left + 1;
        while right < values.len() {
            if values[left] == values[right] {
                return Err(CodecError::MismatchedBinding);
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

/// Derive the current fee carry realloc and payer close from persisted rent.
///
/// `carry_terminal_rent_minimum_lamports` must come from the authenticated
/// rent sysvar for the exact 548-byte 0x83/v4 successor. Any balance above
/// persisted refundable principal is donation, including hostile prefunding.
/// Only the persisted carry payer may fund the exact missing minimum; its
/// contribution increases refundable principal rather than donation.
#[allow(clippy::too_many_arguments)]
pub fn prepare_owner_fee_rent_transition_v3<B: Sha256BackendV1>(
    accounts: OwnerFeeRentTransitionAccountsV3,
    carry_rent_before: DeletableRentOwnerV1,
    payer_rent: DeletableRentOwnerV1,
    carry_balance_before_lamports: u64,
    payer_balance_before_lamports: u64,
    carry_terminal_rent_minimum_lamports: u64,
    backend: &B,
) -> Result<OwnerFeeRentTransitionPlanV3, CodecError> {
    carry_rent_before.validate()?;
    payer_rent.validate()?;
    require_live_distinct(&[
        accounts.carry_account,
        accounts.payer_allocation_account,
        accounts.neutral_sink,
    ])?;
    if carry_rent_before.payer == accounts.carry_account
        || carry_rent_before.payer == accounts.payer_allocation_account
        || carry_rent_before.payer == accounts.neutral_sink
        || payer_rent.payer == accounts.carry_account
        || payer_rent.payer == accounts.payer_allocation_account
        || payer_rent.payer == accounts.neutral_sink
        || carry_terminal_rent_minimum_lamports == 0
    {
        return Err(CodecError::MismatchedBinding);
    }
    let carry_donation_before_lamports = carry_balance_before_lamports
        .checked_sub(carry_rent_before.refundable_principal)
        .ok_or(CodecError::InvalidState)?;
    let payer_donation_lamports = payer_balance_before_lamports
        .checked_sub(payer_rent.refundable_principal)
        .ok_or(CodecError::InvalidState)?;
    if carry_donation_before_lamports < carry_rent_before.donation_floor
        || payer_donation_lamports < payer_rent.donation_floor
    {
        return Err(CodecError::InvalidState);
    }
    let carry_top_up_lamports = carry_terminal_rent_minimum_lamports
        .saturating_sub(carry_balance_before_lamports);
    let carry_balance_after_lamports = carry_balance_before_lamports
        .checked_add(carry_top_up_lamports)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let carry_principal_after_lamports = carry_rent_before
        .refundable_principal
        .checked_add(carry_top_up_lamports)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let carry_rent_after = DeletableRentOwnerV1 {
        payer: carry_rent_before.payer,
        refundable_principal: carry_principal_after_lamports,
        donation_floor: carry_donation_before_lamports,
    };
    carry_rent_after.validate()?;

    let carry_balance_before = carry_balance_before_lamports.to_le_bytes();
    let carry_principal_before = carry_rent_before.refundable_principal.to_le_bytes();
    let carry_donation_before = carry_donation_before_lamports.to_le_bytes();
    let carry_terminal_minimum = carry_terminal_rent_minimum_lamports.to_le_bytes();
    let carry_top_up = carry_top_up_lamports.to_le_bytes();
    let carry_balance_after = carry_balance_after_lamports.to_le_bytes();
    let carry_principal_after = carry_principal_after_lamports.to_le_bytes();
    let payer_balance_before = payer_balance_before_lamports.to_le_bytes();
    let payer_principal = payer_rent.refundable_principal.to_le_bytes();
    let payer_donation = payer_donation_lamports.to_le_bytes();
    let data_id = Id32::new(backend.sha256(&[
        OWNER_FEE_RENT_TRANSITION_DATA_ID_DOMAIN_V3,
        &accounts.carry_account.bytes(),
        &accounts.payer_allocation_account.bytes(),
        &carry_rent_before.payer.bytes(),
        &payer_rent.payer.bytes(),
        &accounts.neutral_sink.bytes(),
        &carry_balance_before,
        &carry_principal_before,
        &carry_donation_before,
        &carry_terminal_minimum,
        &carry_top_up,
        &carry_balance_after,
        &carry_principal_after,
        &payer_balance_before,
        &payer_principal,
        &payer_donation,
    ]))?;
    let semantic = OwnerFeeRentDispositionV2 {
        data_id: FeeId(data_id.bytes()),
        carry_account: FeeId(accounts.carry_account.bytes()),
        payer_allocation_account: FeeId(accounts.payer_allocation_account.bytes()),
        carry_rent_refund_owner: FeeId(carry_rent_before.payer.bytes()),
        carry_top_up_payer: FeeId(carry_rent_before.payer.bytes()),
        payer_rent_refund_owner: FeeId(payer_rent.payer.bytes()),
        neutral_sink: FeeId(accounts.neutral_sink.bytes()),
        carry_balance_before_lamports,
        carry_rent_principal_before_lamports: carry_rent_before.refundable_principal,
        carry_donation_before_lamports,
        carry_v2_rent_minimum_lamports: carry_terminal_rent_minimum_lamports,
        carry_top_up_lamports,
        carry_balance_after_lamports,
        carry_rent_principal_after_lamports: carry_principal_after_lamports,
        carry_donation_after_lamports: carry_donation_before_lamports,
        payer_balance_before_lamports,
        payer_rent_principal_lamports: payer_rent.refundable_principal,
        payer_donation_lamports,
    };
    semantic.validate().map_err(|_| CodecError::InvalidState)?;
    Ok(OwnerFeeRentTransitionPlanV3 {
        semantic,
        carry_rent_after,
        carry_top_up: OwnerFeeRentTransferV3 {
            source: carry_rent_before.payer,
            destination: accounts.carry_account,
            lamports: carry_top_up_lamports,
        },
        payer_principal_refund: OwnerFeeRentTransferV3 {
            source: accounts.payer_allocation_account,
            destination: payer_rent.payer,
            lamports: payer_rent.refundable_principal,
        },
        payer_donation_credit: OwnerFeeRentTransferV3 {
            source: accounts.payer_allocation_account,
            destination: accounts.neutral_sink,
            lamports: payer_donation_lamports,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct EchoHash;

    impl Sha256BackendV1 for EchoHash {
        fn sha256(&self, _parts: &[&[u8]]) -> [u8; 32] {
            [99; 32]
        }
    }

    fn id(byte: u8) -> Id32 {
        Id32::from_bytes([byte; 32])
    }

    #[test]
    fn hostile_surplus_stays_donation_and_topup_stays_principal() {
        let plan = prepare_owner_fee_rent_transition_v3(
            OwnerFeeRentTransitionAccountsV3 {
                carry_account: id(1),
                payer_allocation_account: id(2),
                neutral_sink: id(3),
            },
            DeletableRentOwnerV1 {
                payer: id(4),
                refundable_principal: 100,
                donation_floor: 5,
            },
            DeletableRentOwnerV1 {
                payer: id(5),
                refundable_principal: 200,
                donation_floor: 7,
            },
            115,
            227,
            140,
            &EchoHash,
        )
        .unwrap();
        assert_eq!(plan.carry_top_up().lamports(), 25);
        assert_eq!(plan.carry_rent_after().refundable_principal, 125);
        assert_eq!(plan.carry_rent_after().donation_floor, 15);
        assert_eq!(plan.payer_principal_refund().lamports(), 200);
        assert_eq!(plan.payer_donation_credit().lamports(), 27);
    }

    #[test]
    fn hostile_balance_below_persisted_compartments_is_refused() {
        assert_eq!(
            prepare_owner_fee_rent_transition_v3(
                OwnerFeeRentTransitionAccountsV3 {
                    carry_account: id(1),
                    payer_allocation_account: id(2),
                    neutral_sink: id(3),
                },
                DeletableRentOwnerV1 {
                    payer: id(4),
                    refundable_principal: 100,
                    donation_floor: 5,
                },
                DeletableRentOwnerV1 {
                    payer: id(5),
                    refundable_principal: 200,
                    donation_floor: 7,
                },
                104,
                207,
                140,
                &EchoHash,
            ),
            Err(CodecError::InvalidState)
        );
    }
}
