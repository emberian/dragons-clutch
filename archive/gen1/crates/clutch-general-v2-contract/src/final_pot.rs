// SPDX-License-Identifier: AGPL-3.0-or-later

//! Capability-disabled envelope for the combined General V2 FinalPot.
//!
//! The inner settlement contract is the sole owner of the 328-byte cash,
//! native-claim, and selected virtual-inventory-budget semantics. This module
//! adds only the centrally reserved outer coordinate and an explicit adapter
//! join to the counted SelectedCandidate authority.

use crate::{
    AuthenticatedSelectedCandidateV1, CodecError, Id32, Reader, Writer, FINAL_POT_ACCOUNT_BYTES,
    FINAL_POT_ACCOUNT_TAG, FINAL_POT_ACCOUNT_VERSION,
};

pub use clutch_owner_settlement::{
    AuthenticatedFinalPotV1, FinalPotRetirementProjectionV1, VirtualInventoryBudgetV1,
    VirtualInventoryStateV1, VirtualReceiptKindV1, FINAL_POT_BODY_V1_BYTES,
};

/// Adapter-owned authentication facts for one existing FinalPot account.
///
/// PDA derivation, program ownership, writability, and the SelectedCandidate
/// account key/owner checks remain outside this pure crate. Naming every fact
/// here prevents a decoded body or caller-supplied boolean from silently
/// becoming settlement authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalPotAdapterBindingV1<'a> {
    /// Supplied FinalPot PDA.
    pub final_pot: Id32,
    /// Stored bump reproduced by canonical PDA derivation.
    pub derived_bump: u8,
    /// Exact decoded SelectedCandidate account and supplied PDA.
    pub selected: AuthenticatedSelectedCandidateV1<'a>,
    /// True only after deriving FinalPot from the frozen seed tuple.
    pub final_pot_pda_authenticated: bool,
    /// True only after checking the existing FinalPot program owner.
    pub final_pot_program_owner_authenticated: bool,
    /// True only after deriving the supplied SelectedCandidate PDA.
    pub selected_pda_authenticated: bool,
    /// True only after checking the SelectedCandidate program owner.
    pub selected_program_owner_authenticated: bool,
    /// Exact writable bit of the FinalPot account meta.
    pub writable: bool,
}

impl FinalPotAdapterBindingV1<'_> {
    fn validate(self) -> Result<(), CodecError> {
        self.selected.account.validate()?;
        if self.final_pot.is_zero() || self.selected.artifact.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        if !self.final_pot_pda_authenticated
            || !self.final_pot_program_owner_authenticated
            || !self.selected_pda_authenticated
            || !self.selected_program_owner_authenticated
            || !self.writable
        {
            return Err(CodecError::InvalidState);
        }
        let selected = self.selected.account;
        for alias in [
            self.selected.artifact,
            selected.epoch,
            selected.market,
            selected.window,
            selected.market_binding,
            selected.source_admission_node,
            selected.selected_feed,
            selected.order_set,
            selected.settlement_candidate_id,
        ] {
            if self.final_pot == alias {
                return Err(CodecError::MismatchedBinding);
            }
        }
        let _ = crate::FinalPotSeedTupleV1::new(selected.epoch, selected.settlement_candidate_id)?;
        Ok(())
    }
}

/// Capability-disabled outer envelope for the combined FinalPot body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalPotV1AccountV1 {
    /// Exact constructor-checked FinalPot and embedded-budget semantics.
    pub semantic: AuthenticatedFinalPotV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl FinalPotV1AccountV1 {
    /// Validate exact SelectedCandidate, PDA, owner, bump, and body joins.
    pub fn validate_against_selected(
        self,
        binding: FinalPotAdapterBindingV1<'_>,
    ) -> Result<(), CodecError> {
        binding.validate()?;
        let selected = binding.selected.account;
        if self.flags != 0
            || self.stored_bump != binding.derived_bump
            || self.semantic.account != binding.final_pot.bytes()
            || !self.semantic.writable
            || !self.semantic.selected_budget_authenticated
            || self.semantic.epoch != selected.epoch.bytes()
            || self.semantic.market != selected.market.bytes()
            || self.semantic.candidate != selected.settlement_candidate_id.bytes()
        {
            return Err(CodecError::MismatchedBinding);
        }
        if self.semantic.inventory_kind != VirtualReceiptKindV1::None
            && self.semantic.relation_witness_digest != selected.settlement_witness_digest.bytes()
        {
            return Err(CodecError::MismatchedBinding);
        }
        self.semantic
            .encode_body()
            .map_err(|_| CodecError::InvalidState)?;
        Ok(())
    }

    /// Consume the semantic owner's zero-liability retirement projection
    /// after all outer SelectedCandidate/PDA/owner joins pass.
    pub fn retirement_projection(
        self,
        binding: FinalPotAdapterBindingV1<'_>,
    ) -> Result<FinalPotRetirementProjectionV1, CodecError> {
        self.validate_against_selected(binding)?;
        self.semantic
            .retirement_projection()
            .map_err(|_| CodecError::InvalidState)
    }

    /// Encode the exact canonical 332-byte outer account.
    pub fn encode(
        self,
        binding: FinalPotAdapterBindingV1<'_>,
        output: &mut [u8],
    ) -> Result<(), CodecError> {
        self.validate_against_selected(binding)?;
        let body = self
            .semantic
            .encode_body()
            .map_err(|_| CodecError::InvalidState)?;
        let mut writer = Writer::exact(output, FINAL_POT_ACCOUNT_BYTES)?;
        writer.u8(FINAL_POT_ACCOUNT_TAG)?;
        writer.u8(FINAL_POT_ACCOUNT_VERSION)?;
        writer.bytes(&body)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    /// Decode hostile outer bytes only after the adapter authenticates the
    /// exact FinalPot and SelectedCandidate account facts.
    pub fn decode(input: &[u8], binding: FinalPotAdapterBindingV1<'_>) -> Result<Self, CodecError> {
        binding.validate()?;
        let mut reader = Reader::exact(input, FINAL_POT_ACCOUNT_BYTES)?;
        if reader.u8()? != FINAL_POT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != FINAL_POT_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let body: [u8; FINAL_POT_BODY_V1_BYTES] = reader.array()?;
        let semantic = AuthenticatedFinalPotV1::decode_body(
            &body,
            binding.final_pot.bytes(),
            true,
            binding.writable,
        )
        .map_err(|_| CodecError::InvalidState)?;
        let value = Self {
            semantic,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate_against_selected(binding)?;
        Ok(value)
    }
}

const _: () = assert!(FINAL_POT_BODY_V1_BYTES == 328);
const _: () = assert!(FINAL_POT_ACCOUNT_BYTES == 2 + 328 + 2);
