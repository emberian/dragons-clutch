// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact counted-retirement receipt projection into the existing liveness owner.
//!
//! This module emits only authenticated receipt observations.  It neither
//! plans nor executes lamport movement; `clutch-liveness::plan_runtime_transition_v1`
//! remains the sole owner of keeper payment, work/rent refund, donation
//! disposition, and physical liveness-account closure.

use clutch_liveness::{
    runtime_adapter_v1::{
        RuntimeReceiptKindV1, RuntimeReceiptObservationV1,
    },
    runtime_v1::RuntimeCompartmentKindV1,
    Id,
};
use clutch_retirement::Identity32V1;

use crate::{
    AuthenticatedGeneralV2FinalPotTerminalV1, AuthenticatedGeneralV2TerminalEpochV1,
    CanonicalPdaV1,
};

/// Exact bytes in one canonical external counted-retirement receipt.
pub const RETIREMENT_RECEIPT_ACCOUNT_BYTES_V1: usize = 168;
/// Local receipt-body magic; not a global account discriminator.
pub const RETIREMENT_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCRTRC01";
/// Exact local receipt-body version.
pub const RETIREMENT_RECEIPT_VERSION_V1: u16 = 1;

/// Receipt-specific fail-closed error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetirementReceiptErrorV1 {
    /// Receipt account was not the canonical derived PDA.
    WrongPda,
    /// Receipt account owner did not equal the configured receipt program.
    WrongOwner,
    /// Receipt evidence must be supplied read-only.
    UnexpectedWritable,
    /// Receipt evidence cannot be executable.
    ExecutableAccount,
    /// Exact body length differed.
    WrongLength,
    /// Local magic differed.
    WrongMagic,
    /// Local version differed.
    WrongVersion,
    /// Enum byte was outside the canonical set.
    InvalidKind,
    /// Reserved bytes or work/terminal geometry were noncanonical.
    NonCanonical,
    /// A required identity was zero or two independent roles aliased.
    InvalidIdentity,
    /// A family terminal capability and receipt disagreed.
    BindingMismatch,
}

/// Read-only runtime facts for one receipt account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementReceiptAccountViewV1<'a> {
    /// Presented receipt PDA.
    pub address: Identity32V1,
    /// Presented account owner program.
    pub owner_program: Identity32V1,
    /// Exact local receipt body.
    pub data: &'a [u8],
    /// Receipt inputs must not be writable.
    pub is_writable: bool,
    /// Receipt accounts cannot be executable.
    pub is_executable: bool,
}

/// Canonical external counted-retirement receipt body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementReceiptV1 {
    /// Work, successful terminal, or failed terminal observation.
    pub receipt_kind: RuntimeReceiptKindV1,
    /// Content identity preventing once-only replay.
    pub receipt_id: Identity32V1,
    /// Semantic account/family whose work completed.
    pub semantic_owner: Identity32V1,
    /// Whole protocol lifecycle identity.
    pub lifecycle_id: Identity32V1,
    /// Exact retirement quote-schedule content identity.
    pub quote_schedule_id: Identity32V1,
    /// Nonzero parent generation.
    pub generation: u64,
    /// One-based call ordinal for work; zero for terminal receipts.
    pub call_ordinal: u32,
    /// Exact work-call ceiling; zero for terminal receipts.
    pub call_ceiling_lamports: u64,
}

impl RetirementReceiptV1 {
    fn validate(self) -> Result<(), RetirementReceiptErrorV1> {
        if self.generation == 0 {
            return Err(RetirementReceiptErrorV1::NonCanonical);
        }
        let identities = [
            self.receipt_id,
            self.semantic_owner,
            self.lifecycle_id,
            self.quote_schedule_id,
        ];
        let mut left = 0usize;
        while left < identities.len() {
            let mut right = left + 1;
            while right < identities.len() {
                if identities[left] == identities[right]
                    && !matches!(
                        (left, right),
                        (1, 2) // a root may own its whole lifecycle
                    )
                {
                    return Err(RetirementReceiptErrorV1::InvalidIdentity);
                }
                right += 1;
            }
            left += 1;
        }
        match self.receipt_kind {
            RuntimeReceiptKindV1::WorkCompleted => {
                if self.call_ordinal == 0 || self.call_ceiling_lamports == 0 {
                    return Err(RetirementReceiptErrorV1::NonCanonical);
                }
            }
            RuntimeReceiptKindV1::TerminalSuccess | RuntimeReceiptKindV1::TerminalFailure => {
                if self.call_ordinal != 0 || self.call_ceiling_lamports != 0 {
                    return Err(RetirementReceiptErrorV1::NonCanonical);
                }
            }
        }
        Ok(())
    }

    /// Decode and totally validate one exact hostile receipt body.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementReceiptErrorV1> {
        if input.len() != RETIREMENT_RECEIPT_ACCOUNT_BYTES_V1 {
            return Err(RetirementReceiptErrorV1::WrongLength);
        }
        if input[..8] != RETIREMENT_RECEIPT_MAGIC_V1 {
            return Err(RetirementReceiptErrorV1::WrongMagic);
        }
        if read_u16(input, 8) != RETIREMENT_RECEIPT_VERSION_V1 {
            return Err(RetirementReceiptErrorV1::WrongVersion);
        }
        let receipt_kind = match input[10] {
            0 => RuntimeReceiptKindV1::WorkCompleted,
            1 => RuntimeReceiptKindV1::TerminalSuccess,
            2 => RuntimeReceiptKindV1::TerminalFailure,
            _ => return Err(RetirementReceiptErrorV1::InvalidKind),
        };
        if input[11..16] != [0; 5] || input[156..160] != [0; 4] {
            return Err(RetirementReceiptErrorV1::NonCanonical);
        }
        let value = Self {
            receipt_kind,
            receipt_id: read_id(input, 16)?,
            semantic_owner: read_id(input, 48)?,
            lifecycle_id: read_id(input, 80)?,
            quote_schedule_id: read_id(input, 112)?,
            generation: read_u64(input, 144),
            call_ordinal: read_u32(input, 152),
            call_ceiling_lamports: read_u64(input, 160),
        };
        value.validate()?;
        Ok(value)
    }
}

/// Runtime/PDA-authenticated retirement receipt and its exact liveness output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRetirementReceiptV1 {
    receipt: RetirementReceiptV1,
    observation: RuntimeReceiptObservationV1,
}

impl AuthenticatedRetirementReceiptV1 {
    /// Canonical decoded receipt.
    pub const fn receipt(self) -> RetirementReceiptV1 {
        self.receipt
    }

    /// Exact input for the existing liveness planner.
    pub const fn observation(self) -> RuntimeReceiptObservationV1 {
        self.observation
    }
}

/// Authenticate owner, PDA, access, exact bytes, and project to liveness.
pub fn authenticate_retirement_receipt_v1(
    view: RetirementReceiptAccountViewV1<'_>,
    canonical_pda: CanonicalPdaV1,
    expected_receipt_program: Identity32V1,
) -> Result<AuthenticatedRetirementReceiptV1, RetirementReceiptErrorV1> {
    if view.address != canonical_pda.address() {
        return Err(RetirementReceiptErrorV1::WrongPda);
    }
    if view.owner_program != expected_receipt_program {
        return Err(RetirementReceiptErrorV1::WrongOwner);
    }
    if view.is_writable {
        return Err(RetirementReceiptErrorV1::UnexpectedWritable);
    }
    if view.is_executable {
        return Err(RetirementReceiptErrorV1::ExecutableAccount);
    }
    let receipt = RetirementReceiptV1::decode(view.data)?;
    if view.address == receipt.receipt_id
        || view.address == receipt.semantic_owner
        || view.address == receipt.lifecycle_id
        || view.address == receipt.quote_schedule_id
    {
        return Err(RetirementReceiptErrorV1::InvalidIdentity);
    }
    let observation = RuntimeReceiptObservationV1 {
        receipt_account_id: liveness_id(view.address),
        receipt_account_owner_program_id: liveness_id(view.owner_program),
        receipt_id: liveness_id(receipt.receipt_id),
        receipt_kind: receipt.receipt_kind,
        compartment_kind: RuntimeCompartmentKindV1::Retirement,
        semantic_owner: liveness_id(receipt.semantic_owner),
        lifecycle_id: liveness_id(receipt.lifecycle_id),
        quote_schedule_id: liveness_id(receipt.quote_schedule_id),
        generation: receipt.generation,
        call_ordinal: receipt.call_ordinal,
        call_ceiling_lamports: receipt.call_ceiling_lamports,
    };
    Ok(AuthenticatedRetirementReceiptV1 {
        receipt,
        observation,
    })
}

/// Bind a successful terminal receipt to one exact FinalPot capability.
pub fn bind_general_v2_final_pot_terminal_receipt_v1(
    terminal: AuthenticatedGeneralV2FinalPotTerminalV1,
    receipt: AuthenticatedRetirementReceiptV1,
) -> Result<RuntimeReceiptObservationV1, RetirementReceiptErrorV1> {
    let terminal = terminal.terminal();
    let body = receipt.receipt;
    if body.receipt_kind != RuntimeReceiptKindV1::TerminalSuccess
        || body.semantic_owner != terminal.account()
        || body.lifecycle_id != terminal.parent().epoch_account()
        || body.generation != terminal.parent().epoch_generation()
    {
        return Err(RetirementReceiptErrorV1::BindingMismatch);
    }
    Ok(receipt.observation)
}

/// Bind a successful root/tombstone receipt to one exact terminal Epoch.
pub fn bind_general_v2_epoch_terminal_receipt_v1(
    terminal: AuthenticatedGeneralV2TerminalEpochV1,
    receipt: AuthenticatedRetirementReceiptV1,
) -> Result<RuntimeReceiptObservationV1, RetirementReceiptErrorV1> {
    let parent = terminal.parent();
    let body = receipt.receipt;
    if body.receipt_kind != RuntimeReceiptKindV1::TerminalSuccess
        || body.semantic_owner != parent.epoch_account()
        || body.lifecycle_id != parent.epoch_account()
        || body.generation != parent.epoch_generation()
    {
        return Err(RetirementReceiptErrorV1::BindingMismatch);
    }
    Ok(receipt.observation)
}

fn liveness_id(value: Identity32V1) -> Id {
    Id::from_bytes(value.bytes())
}

fn read_id(input: &[u8], at: usize) -> Result<Identity32V1, RetirementReceiptErrorV1> {
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&input[at..at + 32]);
    Identity32V1::new(bytes).map_err(|_| RetirementReceiptErrorV1::InvalidIdentity)
}

fn read_u16(input: &[u8], at: usize) -> u16 {
    let mut bytes = [0u8; 2];
    bytes.copy_from_slice(&input[at..at + 2]);
    u16::from_le_bytes(bytes)
}

fn read_u32(input: &[u8], at: usize) -> u32 {
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&input[at..at + 4]);
    u32::from_le_bytes(bytes)
}

fn read_u64(input: &[u8], at: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&input[at..at + 8]);
    u64::from_le_bytes(bytes)
}
