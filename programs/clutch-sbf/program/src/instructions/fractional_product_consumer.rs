// SPDX-License-Identifier: AGPL-3.0-or-later

//! Private Fractional postwrite consumer for Product's current Market root.
//!
//! This module owns no a4/a5/ClaimLedger or Product root truth and has no
//! dispatch route. It projects the exact private Fractional postwrite into
//! Product's default-refusing owner trait; Product alone derives and writes the
//! `0xaa/v2` successor.

use crate::accounts::{require, Outcome};
use crate::error::ClutchError;
use clutch_fractional_redemption_runtime::{
    FractionalFamilyAdmissionReceiptV1, FractionalFamilyTerminalReceiptV1,
    FractionalRedemptionActionV1,
};
use clutch_product_series::{
    ContentId, MarketFoundationAccountGraphV4, MarketFoundationScheduleV4,
    MarketInstanceV2Id, MarketLifecycleRootV3,
};
use clutch_retirement::Identity32V1;
use clutch_solana_layout::product_series::MarketLifecycleRootAccountV3;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::fractional_redemption::{
    AuthenticatedFractionalFamilyAdmissionPostwriteV1,
    AuthenticatedFractionalFamilyTerminalPostwriteV1,
};
use super::product_market_lifecycle_v3_current::authenticate_market_lifecycle_root_v3;
use super::product_series_current::{
    consume_fractional_family_admission_postwrite_v2,
    consume_fractional_family_terminal_postwrite_v2,
    AuthenticatedProductFractionalFamilyAdmissionOwnerV2,
    AuthenticatedProductFractionalFamilyAdmissionV2,
    AuthenticatedProductFractionalFamilyTerminalOwnerV2,
    AuthenticatedProductFractionalFamilyTerminalV2,
};

fn content(identity: Identity32V1) -> ContentId {
    ContentId::from_bytes(identity.bytes())
}

fn pubkey(identity: Identity32V1) -> Pubkey {
    Pubkey::new_from_array(identity.bytes())
}

fn require_terminal_registry_identity(
    action: FractionalRedemptionActionV1,
    expected_release_id: ContentId,
    expected_profile_id: ContentId,
    presented_release_id: ContentId,
    presented_profile_id: ContentId,
) -> Outcome<()> {
    require(
        action == FractionalRedemptionActionV1::CloseEmptyLedger
            && expected_release_id == presented_release_id
            && expected_profile_id == presented_profile_id,
        ClutchError::MismatchedState,
    )
}

struct FractionalAdmissionOwnerV2 {
    postwrite: AuthenticatedFractionalFamilyAdmissionPostwriteV1,
}

impl FractionalAdmissionOwnerV2 {
    const fn admission(&self) -> FractionalFamilyAdmissionReceiptV1 {
        self.postwrite.family_admission()
    }
}

impl AuthenticatedProductFractionalFamilyAdmissionOwnerV2 for FractionalAdmissionOwnerV2 {
    fn admission_receipt_id(&self) -> Outcome<ContentId> {
        Ok(content(self.admission().receipt_id()))
    }

    fn verification_id(&self) -> Outcome<ContentId> {
        Ok(content(self.postwrite.verification_id()))
    }

    fn postwrite_authentication_id(&self) -> Outcome<ContentId> {
        Ok(content(self.postwrite.authentication_id()))
    }

    fn policy_state_id(&self) -> Outcome<ContentId> {
        Ok(content(self.admission().policy_state_id()))
    }

    fn ledger_state_id(&self) -> Outcome<ContentId> {
        Ok(content(self.admission().ledger_state_id()))
    }

    fn claim_ledger_before_id(&self) -> Outcome<ContentId> {
        Ok(content(self.admission().claim_ledger_before_id()))
    }

    fn claim_ledger_after_id(&self) -> Outcome<ContentId> {
        Ok(content(self.admission().claim_ledger_after_id()))
    }

    fn claim_ledger_latch_transition_id(&self) -> Outcome<ContentId> {
        Ok(content(self.admission().latch_transition_id()))
    }

    #[allow(clippy::too_many_arguments)]
    fn authenticate_product_fractional_family_admission_owner_v2(
        &self,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        policy_account: Pubkey,
        policy_state_id: ContentId,
        ledger_account: Pubkey,
        ledger_state_id: ContentId,
        claim_ledger_account: Pubkey,
        claim_ledger_before_id: ContentId,
        claim_ledger_after_id: ContentId,
        claim_ledger_latch_transition_id: ContentId,
        claim_issuance_binding_id: ContentId,
        runtime_release_id: ContentId,
        capability_profile_id: ContentId,
        resolution_account: Pubkey,
        resolution_semantic_id: ContentId,
        resolution_data_id: ContentId,
        native_claim_basis_id: ContentId,
        admission_receipt_id: ContentId,
        verification_id: ContentId,
        postwrite_authentication_id: ContentId,
    ) -> Outcome<()> {
        let admission = self.admission();
        let release = self.postwrite.runtime_release();
        require(
            release.action() == FractionalRedemptionActionV1::Initialize
                && market_instance_id.bytes() == admission.market_instance().bytes()
                && generation == admission.domain_generation()
                && policy_account == pubkey(admission.policy_account())
                && policy_state_id == content(admission.policy_state_id())
                && ledger_account == pubkey(admission.ledger_account())
                && ledger_state_id == content(admission.ledger_state_id())
                && claim_ledger_account == pubkey(admission.claim_ledger_account())
                && claim_ledger_before_id == content(admission.claim_ledger_before_id())
                && claim_ledger_after_id == content(admission.claim_ledger_after_id())
                && claim_ledger_latch_transition_id == content(admission.latch_transition_id())
                && claim_issuance_binding_id == content(admission.claim_issuance_binding())
                && runtime_release_id == content(release.release_id())
                && capability_profile_id == release.capability_profile_id()
                && resolution_account == pubkey(self.postwrite.resolution_account())
                && resolution_semantic_id == content(self.postwrite.resolution_semantic_id())
                && resolution_data_id == content(self.postwrite.resolution_data_id())
                && native_claim_basis_id == content(self.postwrite.native_claim_basis_id())
                && admission_receipt_id == content(admission.receipt_id())
                && verification_id == content(self.postwrite.verification_id())
                && postwrite_authentication_id == content(self.postwrite.authentication_id()),
            ClutchError::MismatchedState,
        )
    }
}

struct FractionalTerminalOwnerV2 {
    postwrite: AuthenticatedFractionalFamilyTerminalPostwriteV1,
}

impl FractionalTerminalOwnerV2 {
    const fn terminal(&self) -> FractionalFamilyTerminalReceiptV1 {
        self.postwrite.family_terminal()
    }
}

impl AuthenticatedProductFractionalFamilyTerminalOwnerV2 for FractionalTerminalOwnerV2 {
    fn terminal_receipt_id(&self) -> Outcome<ContentId> {
        Ok(content(self.terminal().receipt_id()))
    }

    fn verification_id(&self) -> Outcome<ContentId> {
        Ok(content(self.postwrite.verification_id()))
    }

    fn postwrite_authentication_id(&self) -> Outcome<ContentId> {
        Ok(content(self.postwrite.authentication_id()))
    }

    fn policy_terminal_state_id(&self) -> Outcome<ContentId> {
        Ok(content(self.terminal().policy_terminal_state_id()))
    }

    fn ledger_terminal_state_id(&self) -> Outcome<ContentId> {
        Ok(content(self.terminal().ledger_terminal_state_id()))
    }

    fn claim_ledger_post_state_id(&self) -> Outcome<ContentId> {
        Ok(content(self.terminal().claim_ledger_post_state_id()))
    }

    fn claim_ledger_transition_id(&self) -> Outcome<ContentId> {
        Ok(content(self.terminal().claim_ledger_transition_id()))
    }

    fn fractional_release_id(&self) -> Outcome<ContentId> {
        Ok(content(self.terminal().fractional_release_id()))
    }

    fn claim_release_receipt_id(&self) -> Outcome<ContentId> {
        Ok(content(self.postwrite.claim_release_receipt_id()))
    }

    fn rent_disposition_id(&self) -> Outcome<ContentId> {
        Ok(content(self.terminal().rent_disposition_id()))
    }

    #[allow(clippy::too_many_arguments)]
    fn authenticate_product_fractional_family_terminal_owner_v2(
        &self,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        policy_account: Pubkey,
        policy_terminal_state_id: ContentId,
        ledger_account: Pubkey,
        ledger_terminal_state_id: ContentId,
        claim_ledger_account: Pubkey,
        claim_ledger_post_state_id: ContentId,
        claim_ledger_transition_id: ContentId,
        fractional_release_id: ContentId,
        capability_profile_id: ContentId,
        claim_release_receipt_id: ContentId,
        rent_disposition_id: ContentId,
        resolution_account: Pubkey,
        resolution_semantic_id: ContentId,
        resolution_data_id: ContentId,
        native_claim_basis_id: ContentId,
        terminal_receipt_id: ContentId,
        verification_id: ContentId,
        postwrite_authentication_id: ContentId,
    ) -> Outcome<()> {
        let terminal = self.terminal();
        let release = self.postwrite.runtime_release();
        require_terminal_registry_identity(
            release.action(),
            content(release.release_id()),
            release.capability_profile_id(),
            fractional_release_id,
            capability_profile_id,
        )?;
        require(
            market_instance_id.bytes() == terminal.market_instance_id().bytes()
                && generation == terminal.domain_generation()
                && policy_account == pubkey(terminal.policy_account())
                && policy_terminal_state_id == content(terminal.policy_terminal_state_id())
                && ledger_account == pubkey(terminal.ledger_account())
                && ledger_terminal_state_id == content(terminal.ledger_terminal_state_id())
                && claim_ledger_account == pubkey(terminal.claim_ledger_account())
                && claim_ledger_post_state_id == content(terminal.claim_ledger_post_state_id())
                && claim_ledger_transition_id == content(terminal.claim_ledger_transition_id())
                && fractional_release_id == content(terminal.fractional_release_id())
                && claim_release_receipt_id == content(self.postwrite.claim_release_receipt_id())
                && rent_disposition_id == content(terminal.rent_disposition_id())
                && resolution_account == pubkey(self.postwrite.resolution_account())
                && resolution_semantic_id == content(self.postwrite.resolution_semantic_id())
                && resolution_data_id == content(self.postwrite.resolution_data_id())
                && native_claim_basis_id == content(self.postwrite.native_claim_basis_id())
                && terminal_receipt_id == content(terminal.receipt_id())
                && verification_id == content(self.postwrite.verification_id())
                && postwrite_authentication_id == content(self.postwrite.authentication_id()),
            ClutchError::MismatchedState,
        )
    }
}

/// Consume one exact action-1 a4/v3, a5/v1, and ClaimLedger postwrite.
#[allow(clippy::too_many_arguments)]
pub(crate) fn consume_fractional_admission_v2(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    postwrite: AuthenticatedFractionalFamilyAdmissionPostwriteV1,
    schedule: &MarketFoundationScheduleV4,
    graph: &MarketFoundationAccountGraphV4,
    root_before_output: &mut MarketLifecycleRootAccountV3,
    root_successor_output: &mut MarketLifecycleRootV3,
    root_after_output: &mut MarketLifecycleRootAccountV3,
) -> Outcome<AuthenticatedProductFractionalFamilyAdmissionV2> {
    let admission = postwrite.family_admission();
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        MarketInstanceV2Id::from_bytes(admission.market_instance().bytes()),
        admission.domain_generation(),
        true,
        root_before_output,
    )?;
    let owner = FractionalAdmissionOwnerV2 { postwrite };
    let (_, accepted) = consume_fractional_family_admission_postwrite_v2(
        program_id,
        root_account,
        root,
        &owner,
        schedule,
        graph,
        root_successor_output,
        root_after_output,
    )?;
    Ok(accepted)
}

#[cfg(test)]
mod hostile_contract_tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    #[test]
    fn terminal_registry_identity_requires_both_release_and_profile() {
        assert!(require_terminal_registry_identity(
            FractionalRedemptionActionV1::CloseEmptyLedger,
            id(1),
            id(2),
            id(1),
            id(2),
        )
        .is_ok());
        assert!(require_terminal_registry_identity(
            FractionalRedemptionActionV1::CloseEmptyLedger,
            id(1),
            id(2),
            id(1),
            id(3),
        )
        .is_err());
        assert!(require_terminal_registry_identity(
            FractionalRedemptionActionV1::SealClaimsExhausted,
            id(1),
            id(2),
            id(1),
            id(2),
        )
        .is_err());
    }
}

/// Consume one exact action-10 postwrite before a4/v3 and a5/v1 deletion.
#[allow(clippy::too_many_arguments)]
pub(crate) fn consume_fractional_terminal_v2(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    postwrite: AuthenticatedFractionalFamilyTerminalPostwriteV1,
    schedule: &MarketFoundationScheduleV4,
    graph: &MarketFoundationAccountGraphV4,
    root_before_output: &mut MarketLifecycleRootAccountV3,
    root_successor_output: &mut MarketLifecycleRootV3,
    root_after_output: &mut MarketLifecycleRootAccountV3,
) -> Outcome<AuthenticatedProductFractionalFamilyTerminalV2> {
    let terminal = postwrite.family_terminal();
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        MarketInstanceV2Id::from_bytes(terminal.market_instance_id().bytes()),
        terminal.domain_generation(),
        true,
        root_before_output,
    )?;
    let owner = FractionalTerminalOwnerV2 { postwrite };
    let (_, accepted) = consume_fractional_family_terminal_postwrite_v2(
        program_id,
        root_account,
        root,
        &owner,
        schedule,
        graph,
        root_successor_output,
        root_after_output,
    )?;
    Ok(accepted)
}
