// SPDX-License-Identifier: AGPL-3.0-or-later

//! Runtime authentication for externally owned FinalPot liability receipts.
//!
//! Each receipt program remains the sole semantic owner of its compartment.
//! This module only authenticates exact Solana account facts, binds the
//! receipt to the persisted FinalPot, and emits a complete prospective
//! FinalPot postimage. It moves no collateral and accepts no neutral sink.

use clutch_general_v2_contract::{
    FinalPotDischargeKindV1, FinalPotDischargeReceiptV1, GeneralV2FinalPotV1AccountV1,
    FINAL_POT_ACCOUNT_BYTES, FINAL_POT_DISCHARGE_RECEIPT_BODY_V1_BYTES,
};
use clutch_retirement::Identity32V1;

use crate::{AccountViewV2, CanonicalPdaV1, RetirementAdapterErrorV2};

/// Exact read-only receipt capability after owner/PDA/body authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralV2FinalPotDischargeReceiptV1 {
    account: Identity32V1,
    owner_program: Identity32V1,
    receipt: FinalPotDischargeReceiptV1,
}

impl AuthenticatedGeneralV2FinalPotDischargeReceiptV1 {
    /// Canonical receipt PDA retained as the once-only disposition identity.
    pub const fn account(self) -> Identity32V1 {
        self.account
    }
    /// Exact semantic-owner program selected by receipt kind.
    pub const fn owner_program(self) -> Identity32V1 {
        self.owner_program
    }
    /// Totally decoded canonical semantic receipt.
    pub const fn receipt(self) -> FinalPotDischargeReceiptV1 {
        self.receipt
    }
}

/// Authenticate an external FinalPot receipt without trusting a client-side
/// projection. The receipt id is the canonical receipt PDA itself.
pub fn authenticate_general_v2_final_pot_discharge_receipt_v1(
    view: AccountViewV2<'_>,
    canonical_pda: CanonicalPdaV1,
    expected_receipt_program: Identity32V1,
) -> Result<AuthenticatedGeneralV2FinalPotDischargeReceiptV1, RetirementAdapterErrorV2> {
    if view.address != canonical_pda.address() {
        return Err(RetirementAdapterErrorV2::WrongPda);
    }
    if view.is_writable {
        return Err(RetirementAdapterErrorV2::UnexpectedWritable);
    }
    if view.is_executable {
        return Err(RetirementAdapterErrorV2::ExecutableAccount);
    }
    if view.data.len() != FINAL_POT_DISCHARGE_RECEIPT_BODY_V1_BYTES {
        return Err(RetirementAdapterErrorV2::InvalidSchema);
    }
    let receipt = FinalPotDischargeReceiptV1::decode_body(view.data)?;
    if expected_receipt_program.is_zero() || view.owner != expected_receipt_program {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    if receipt.receipt_id != view.address.bytes() {
        return Err(RetirementAdapterErrorV2::WrongPda);
    }
    Ok(AuthenticatedGeneralV2FinalPotDischargeReceiptV1 {
        account: view.address,
        owner_program: view.owner,
        receipt,
    })
}

/// Prospective exact FinalPot write authorized by one external compartment
/// receipt. The external collateral/fee transition must already have produced
/// the canonical receipt; this plan never executes or duplicates it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedGeneralV2FinalPotDischargeV1 {
    final_pot_account: Identity32V1,
    receipt_account: Identity32V1,
    kind: FinalPotDischargeKindV1,
    final_pot_bytes_after: [u8; FINAL_POT_ACCOUNT_BYTES],
    became_terminal: bool,
}

impl PreparedGeneralV2FinalPotDischargeV1 {
    /// Writable explicit-liability FinalPot account.
    pub const fn final_pot_account(self) -> Identity32V1 {
        self.final_pot_account
    }
    /// Read-only canonical external receipt account.
    pub const fn receipt_account(self) -> Identity32V1 {
        self.receipt_account
    }
    /// Disjoint compartment consumed.
    pub const fn kind(self) -> FinalPotDischargeKindV1 {
        self.kind
    }
    /// Complete canonical FinalPot postimage.
    pub const fn final_pot_bytes_after(self) -> [u8; FINAL_POT_ACCOUNT_BYTES] {
        self.final_pot_bytes_after
    }
    /// Whether all liabilities and owner rows are now exhausted.
    pub const fn became_terminal(self) -> bool {
        self.became_terminal
    }
}

/// Bind one authenticated external receipt to one exact writable FinalPot and
/// prepare its once-only latch update.
pub fn prepare_apply_general_v2_final_pot_discharge_receipt_v1(
    final_pot: AccountViewV2<'_>,
    program_id: Identity32V1,
    canonical_final_pot_pda: CanonicalPdaV1,
    receipt: AuthenticatedGeneralV2FinalPotDischargeReceiptV1,
) -> Result<PreparedGeneralV2FinalPotDischargeV1, RetirementAdapterErrorV2> {
    if final_pot.address != canonical_final_pot_pda.address() {
        return Err(RetirementAdapterErrorV2::WrongPda);
    }
    if final_pot.owner != program_id {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    if !final_pot.is_writable {
        return Err(RetirementAdapterErrorV2::NotWritable);
    }
    if final_pot.is_executable {
        return Err(RetirementAdapterErrorV2::ExecutableAccount);
    }
    if final_pot.data.len() != FINAL_POT_ACCOUNT_BYTES {
        return Err(RetirementAdapterErrorV2::InvalidSchema);
    }
    if final_pot.address == receipt.account {
        return Err(clutch_retirement::RetirementErrorV2::AccountAlias.into());
    }
    let mut account = GeneralV2FinalPotV1AccountV1::decode(final_pot.data)?;
    if account.stored_bump != canonical_final_pot_pda.bump() {
        return Err(RetirementAdapterErrorV2::WrongBump);
    }
    account.semantic = account
        .semantic
        .discharge_external_receipt(receipt.receipt)?;
    let became_terminal = account.semantic.retirement_disposition().is_ok();
    let mut final_pot_bytes_after = [0u8; FINAL_POT_ACCOUNT_BYTES];
    account.encode(&mut final_pot_bytes_after)?;
    Ok(PreparedGeneralV2FinalPotDischargeV1 {
        final_pot_account: final_pot.address,
        receipt_account: receipt.account,
        kind: receipt.receipt.kind,
        final_pot_bytes_after,
        became_terminal,
    })
}
