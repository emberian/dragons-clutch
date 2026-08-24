// SPDX-License-Identifier: AGPL-3.0-or-later

//! Private Fractional postwrite consumer for the Product Market root.
//!
//! This module owns no a4/a5/ClaimLedger truth and has no dispatch route. It
//! accepts only the private receipts minted by `fractional_redemption` from
//! current writable postimages, joins them to Product's persisted Foundation
//! graph, and delegates the sole `0xaa/v1` mutation to Product's narrow,
//! default-refusing Fractional writers.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use clutch_fractional_redemption_runtime::{
    FractionalRedemptionActionV1, FractionalFamilyAdmissionReceiptV1,
    FractionalFamilyTerminalReceiptV1,
};
use clutch_product_series::{
    ContentId, MarketFamilyStatusV1, MarketFamilyV1, MarketFoundationAccountGraphV2,
    MarketFoundationAccountGraphV2Id, MarketFoundationScheduleV2,
    MarketFoundationScheduleV2Id, MarketFoundationSlotV2, MarketInstanceV2Id,
    MarketLifecyclePhaseV1,
};
use clutch_retirement::Identity32V1;
use clutch_solana_layout::product_series::MarketLifecycleRootAccountV1;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::fractional_redemption::{
    AuthenticatedFractionalFamilyAdmissionPostwriteV1,
    AuthenticatedFractionalFamilyTerminalPostwriteV1,
};
use super::product_market::{
    authenticate_market_lifecycle_root_v1,
    write_authenticated_fractional_family_admission_root_v1,
    write_authenticated_fractional_family_terminal_root_v1,
    AuthenticatedFractionalFamilyAdmissionRootWriteV1,
    AuthenticatedFractionalFamilyTerminalRootWriteV1, AuthenticatedMarketLifecycleRootV1,
};

const ADMISSION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-fractional-consumer/admission/v2\0";
const TERMINAL_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-fractional-consumer/terminal/v2\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductFractionalAdmissionV1 {
    id: ContentId,
}

impl AuthenticatedProductFractionalAdmissionV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductFractionalTerminalV1 {
    id: ContentId,
}

impl AuthenticatedProductFractionalTerminalV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }
}

struct AdmissionRootAuthorityV1 {
    root_account: Pubkey,
    root_semantic_before: ContentId,
    root_data_before: ContentId,
    root_authentication_before: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    fractional_root_id: ContentId,
    sequence: u32,
    receipt_id: ContentId,
    verification_id: ContentId,
    postwrite_authentication_id: ContentId,
}

impl AuthenticatedFractionalFamilyAdmissionRootWriteV1 for AdmissionRootAuthorityV1 {
    #[allow(clippy::too_many_arguments)]
    fn authenticate_fractional_family_admission_root_write_v1(
        &self,
        root_account: Pubkey,
        root_semantic_before: ContentId,
        root_data_before: ContentId,
        root_authentication_before: ContentId,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        fractional_root_id: ContentId,
        family_admission_sequence: u32,
        fractional_admission_receipt_id: ContentId,
        fractional_verification_id: ContentId,
        fractional_postwrite_authentication_id: ContentId,
    ) -> Outcome<()> {
        require(
            root_account == self.root_account
                && root_semantic_before == self.root_semantic_before
                && root_data_before == self.root_data_before
                && root_authentication_before == self.root_authentication_before
                && market_instance_id == self.market_instance_id
                && generation == self.generation
                && fractional_root_id == self.fractional_root_id
                && family_admission_sequence == self.sequence
                && fractional_admission_receipt_id == self.receipt_id
                && fractional_verification_id == self.verification_id
                && fractional_postwrite_authentication_id == self.postwrite_authentication_id,
            ClutchError::MismatchedState,
        )
    }
}

struct TerminalRootAuthorityV1 {
    root_account: Pubkey,
    root_semantic_before: ContentId,
    root_data_before: ContentId,
    root_authentication_before: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    fractional_root_id: ContentId,
    sequence: u32,
    receipt_id: ContentId,
    policy_terminal_state_id: ContentId,
    ledger_terminal_state_id: ContentId,
    verification_id: ContentId,
    postwrite_authentication_id: ContentId,
    claim_release_receipt_id: ContentId,
}

impl AuthenticatedFractionalFamilyTerminalRootWriteV1 for TerminalRootAuthorityV1 {
    #[allow(clippy::too_many_arguments)]
    fn authenticate_fractional_family_terminal_root_write_v1(
        &self,
        root_account: Pubkey,
        root_semantic_before: ContentId,
        root_data_before: ContentId,
        root_authentication_before: ContentId,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        fractional_root_id: ContentId,
        family_terminal_sequence: u32,
        fractional_terminal_receipt_id: ContentId,
        fractional_policy_terminal_state_id: ContentId,
        fractional_ledger_terminal_state_id: ContentId,
        fractional_verification_id: ContentId,
        fractional_postwrite_authentication_id: ContentId,
        claim_release_receipt_id: ContentId,
    ) -> Outcome<()> {
        require(
            root_account == self.root_account
                && root_semantic_before == self.root_semantic_before
                && root_data_before == self.root_data_before
                && root_authentication_before == self.root_authentication_before
                && market_instance_id == self.market_instance_id
                && generation == self.generation
                && fractional_root_id == self.fractional_root_id
                && family_terminal_sequence == self.sequence
                && fractional_terminal_receipt_id == self.receipt_id
                && fractional_policy_terminal_state_id == self.policy_terminal_state_id
                && fractional_ledger_terminal_state_id == self.ledger_terminal_state_id
                && fractional_verification_id == self.verification_id
                && fractional_postwrite_authentication_id == self.postwrite_authentication_id
                && claim_release_receipt_id == self.claim_release_receipt_id,
            ClutchError::MismatchedState,
        )
    }
}

fn content(identity: Identity32V1) -> ContentId {
    ContentId::from_bytes(identity.bytes())
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(id != ContentId::ZERO, ClutchError::MismatchedState)
}

fn authenticate_root<'a>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    output: &'a mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedMarketLifecycleRootV1<'a>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    MarketLifecycleRootAccountV1::decode_into(&data, output)?;
    drop(data);
    authenticate_market_lifecycle_root_v1(
        program_id,
        account,
        market_instance_id,
        generation,
        true,
        output,
    )
}

fn graph_id(
    schedule: &MarketFoundationScheduleV2,
    graph: &MarketFoundationAccountGraphV2,
) -> Outcome<(MarketFoundationScheduleV2Id, MarketFoundationAccountGraphV2Id)> {
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok((schedule_id, graph_id))
}

fn require_foundation_accounts(
    graph: &MarketFoundationAccountGraphV2,
    policy_account: ContentId,
    ledger_account: ContentId,
    claim_ledger_account: ContentId,
) -> Outcome<()> {
    require(
        graph
            .account(MarketFoundationSlotV2::FractionalPolicy)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == policy_account
            && graph
                .account(MarketFoundationSlotV2::FractionalLedger)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == ledger_account
            && graph
                .account(MarketFoundationSlotV2::ClaimLedger)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == claim_ledger_account,
        ClutchError::MismatchedState,
    )
}

/// Consume one exact action-1 a4/v3, a5/v1, and ClaimLedger postwrite.
#[allow(clippy::too_many_arguments)]
pub(crate) fn consume_fractional_admission_v1(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    postwrite: AuthenticatedFractionalFamilyAdmissionPostwriteV1,
    schedule: &MarketFoundationScheduleV2,
    graph: &MarketFoundationAccountGraphV2,
    root_before_output: &mut MarketLifecycleRootAccountV1,
    root_after_output: &mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedProductFractionalAdmissionV1> {
    let admission: FractionalFamilyAdmissionReceiptV1 = postwrite.family_admission();
    let runtime_release = postwrite.runtime_release();
    let market_instance_id = MarketInstanceV2Id::from_bytes(admission.market_instance().bytes());
    let generation = admission.domain_generation();
    let root = authenticate_root(
        program_id,
        root_account,
        market_instance_id,
        generation,
        root_before_output,
    )?;
    let binding = root.state().binding();
    let (schedule_id, foundation_graph_id) = graph_id(schedule, graph)?;
    let policy_account = content(admission.policy_account());
    let ledger_account = content(admission.ledger_account());
    let claim_ledger_account = content(admission.claim_ledger_account());
    let policy_state_id = content(admission.policy_state_id());
    let ledger_state_id = content(admission.ledger_state_id());
    let claim_ledger_before_id = content(admission.claim_ledger_before_id());
    let claim_ledger_after_id = content(admission.claim_ledger_after_id());
    let claim_latch_id = content(admission.latch_transition_id());
    let receipt_id = content(admission.receipt_id());
    let verification_id = content(postwrite.verification_id());
    let postwrite_id = content(postwrite.authentication_id());
    let resolution_account = content(postwrite.resolution_account());
    let resolution_semantic_id = content(postwrite.resolution_semantic_id());
    let resolution_data_id = content(postwrite.resolution_data_id());
    let native_claim_basis_id = content(postwrite.native_claim_basis_id());
    let runtime_release_id = content(runtime_release.release_id());
    let fractional = root
        .state()
        .product_families()
        .family(MarketFamilyV1::Fractional);
    let fractional_root_id = root
        .state()
        .product_families()
        .binding()
        .family_root_id(MarketFamilyV1::Fractional);
    for id in [
        policy_account,
        ledger_account,
        claim_ledger_account,
        policy_state_id,
        ledger_state_id,
        claim_ledger_before_id,
        claim_ledger_after_id,
        claim_latch_id,
        receipt_id,
        verification_id,
        postwrite_id,
        resolution_account,
        resolution_semantic_id,
        resolution_data_id,
        native_claim_basis_id,
        runtime_release_id,
        runtime_release.capability_profile_id(),
    ] {
        require_live(id)?;
    }
    require_foundation_accounts(graph, policy_account, ledger_account, claim_ledger_account)?;
    require(
        runtime_release.action() == FractionalRedemptionActionV1::Initialize
            && root.state().phase() == MarketLifecyclePhaseV1::Active
            && root.is_writable()
            && binding.market_instance_id == market_instance_id
            && binding.generation == generation
            && binding.foundation_schedule_id == schedule_id
            && binding.foundation_account_graph_id == foundation_graph_id
            && graph.market_instance_id == market_instance_id
            && graph.generation == generation
            && fractional_root_id == policy_account
            && binding.claim_issuance_binding_id
                == content(admission.claim_issuance_binding())
            && binding.resolution_account_id == resolution_account
            && root.state().resolution_semantic_id() == resolution_semantic_id
            && root.state().resolution_data_id() == resolution_data_id
            && binding.native_claim_basis_id == native_claim_basis_id
            && binding.registry_release_id == runtime_release_id
            && binding.capability_profile_id == runtime_release.capability_profile_id()
            && fractional.status() == MarketFamilyStatusV1::EnabledNeverFounded
            && fractional.counts().admitted == 0
            && fractional.counts().live == 0
            && fractional.counts().terminal == 0
            && policy_account != ledger_account
            && policy_account != claim_ledger_account
            && ledger_account != claim_ledger_account
            && policy_state_id != ledger_state_id
            && claim_ledger_before_id != claim_ledger_after_id
            && receipt_id != verification_id
            && receipt_id != postwrite_id
            && verification_id != postwrite_id,
        ClutchError::MismatchedState,
    )?;
    let root_semantic_before = root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let sequence = fractional.counts().admitted;
    let authority = AdmissionRootAuthorityV1 {
        root_account: *root_account.key,
        root_semantic_before,
        root_data_before: root.data_id(),
        root_authentication_before: root.authentication_id(),
        market_instance_id,
        generation,
        fractional_root_id,
        sequence,
        receipt_id,
        verification_id,
        postwrite_authentication_id: postwrite_id,
    };
    let root_after = write_authenticated_fractional_family_admission_root_v1(
        program_id,
        root_account,
        root,
        sequence,
        receipt_id,
        verification_id,
        postwrite_id,
        &authority,
        root_after_output,
    )?;
    let after = root_after
        .state()
        .product_families()
        .family(MarketFamilyV1::Fractional);
    require(
        after.status() == MarketFamilyStatusV1::Live
            && after.counts().admitted == 1
            && after.counts().live == 1
            && after.counts().terminal == 0,
        ClutchError::MismatchedState,
    )?;
    let root_semantic_after = root_after
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            ADMISSION_DOMAIN_V2,
            program_id.as_ref(),
            root_account.key.as_ref(),
            &root.authentication_id().bytes(),
            &root_after.authentication_id().bytes(),
            &root_semantic_before.bytes(),
            &root_semantic_after.bytes(),
            &receipt_id.bytes(),
            &verification_id.bytes(),
            &postwrite_id.bytes(),
            &runtime_release.authentication_id().bytes(),
            &resolution_semantic_id.bytes(),
            &claim_latch_id.bytes(),
        ])
        .to_bytes(),
    );
    require_live(id)?;
    Ok(AuthenticatedProductFractionalAdmissionV1 { id })
}

/// Consume one exact action-10 postwrite before a4/v3 and a5/v1 deletion.
#[allow(clippy::too_many_arguments)]
pub(crate) fn consume_fractional_terminal_v1(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    postwrite: AuthenticatedFractionalFamilyTerminalPostwriteV1,
    schedule: &MarketFoundationScheduleV2,
    graph: &MarketFoundationAccountGraphV2,
    root_before_output: &mut MarketLifecycleRootAccountV1,
    root_after_output: &mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedProductFractionalTerminalV1> {
    let terminal: FractionalFamilyTerminalReceiptV1 = postwrite.family_terminal();
    let runtime_release = postwrite.runtime_release();
    let market_instance_id = MarketInstanceV2Id::from_bytes(terminal.market_instance_id().bytes());
    let generation = terminal.domain_generation();
    let root = authenticate_root(
        program_id,
        root_account,
        market_instance_id,
        generation,
        root_before_output,
    )?;
    let binding = root.state().binding();
    let (schedule_id, foundation_graph_id) = graph_id(schedule, graph)?;
    let policy_account = content(terminal.policy_account());
    let ledger_account = content(terminal.ledger_account());
    let claim_ledger_account = content(terminal.claim_ledger_account());
    let policy_terminal_state_id = content(terminal.policy_terminal_state_id());
    let ledger_terminal_state_id = content(terminal.ledger_terminal_state_id());
    let claim_ledger_post_state_id = content(terminal.claim_ledger_post_state_id());
    let claim_ledger_transition_id = content(terminal.claim_ledger_transition_id());
    let rent_disposition_id = content(terminal.rent_disposition_id());
    let receipt_id = content(terminal.receipt_id());
    let verification_id = content(postwrite.verification_id());
    let postwrite_id = content(postwrite.authentication_id());
    let claim_release_receipt_id = content(postwrite.claim_release_receipt_id());
    let resolution_account = content(postwrite.resolution_account());
    let resolution_semantic_id = content(postwrite.resolution_semantic_id());
    let resolution_data_id = content(postwrite.resolution_data_id());
    let native_claim_basis_id = content(postwrite.native_claim_basis_id());
    let runtime_release_id = content(runtime_release.release_id());
    let fractional = root
        .state()
        .product_families()
        .family(MarketFamilyV1::Fractional);
    let fractional_root_id = root
        .state()
        .product_families()
        .binding()
        .family_root_id(MarketFamilyV1::Fractional);
    for id in [
        policy_account,
        ledger_account,
        claim_ledger_account,
        policy_terminal_state_id,
        ledger_terminal_state_id,
        claim_ledger_post_state_id,
        claim_ledger_transition_id,
        rent_disposition_id,
        receipt_id,
        verification_id,
        postwrite_id,
        claim_release_receipt_id,
        resolution_account,
        resolution_semantic_id,
        resolution_data_id,
        native_claim_basis_id,
        runtime_release_id,
        runtime_release.capability_profile_id(),
    ] {
        require_live(id)?;
    }
    require_foundation_accounts(graph, policy_account, ledger_account, claim_ledger_account)?;
    require(
        runtime_release.action() == FractionalRedemptionActionV1::CloseEmptyLedger
            && matches!(
                root.state().phase(),
                MarketLifecyclePhaseV1::Active | MarketLifecyclePhaseV1::Retiring
            )
            && root.is_writable()
            && binding.market_instance_id == market_instance_id
            && binding.generation == generation
            && binding.foundation_schedule_id == schedule_id
            && binding.foundation_account_graph_id == foundation_graph_id
            && graph.market_instance_id == market_instance_id
            && graph.generation == generation
            && fractional_root_id == policy_account
            && binding.resolution_account_id == resolution_account
            && root.state().resolution_semantic_id() == resolution_semantic_id
            && root.state().resolution_data_id() == resolution_data_id
            && binding.native_claim_basis_id == native_claim_basis_id
            && binding.registry_release_id == runtime_release_id
            && binding.capability_profile_id == runtime_release.capability_profile_id()
            && content(terminal.fractional_release_id()) == runtime_release_id
            && fractional.status() == MarketFamilyStatusV1::Live
            && fractional.counts().admitted == 1
            && fractional.counts().live == 1
            && fractional.counts().terminal == 0
            && policy_account != ledger_account
            && policy_account != claim_ledger_account
            && ledger_account != claim_ledger_account
            && policy_terminal_state_id != ledger_terminal_state_id
            && receipt_id != verification_id
            && receipt_id != postwrite_id
            && receipt_id != claim_release_receipt_id
            && verification_id != postwrite_id
            && verification_id != claim_release_receipt_id
            && postwrite_id != claim_release_receipt_id,
        ClutchError::MismatchedState,
    )?;
    let root_semantic_before = root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let sequence = fractional.counts().terminal;
    let authority = TerminalRootAuthorityV1 {
        root_account: *root_account.key,
        root_semantic_before,
        root_data_before: root.data_id(),
        root_authentication_before: root.authentication_id(),
        market_instance_id,
        generation,
        fractional_root_id,
        sequence,
        receipt_id,
        policy_terminal_state_id,
        ledger_terminal_state_id,
        verification_id,
        postwrite_authentication_id: postwrite_id,
        claim_release_receipt_id,
    };
    let root_after = write_authenticated_fractional_family_terminal_root_v1(
        program_id,
        root_account,
        root,
        sequence,
        receipt_id,
        policy_terminal_state_id,
        ledger_terminal_state_id,
        verification_id,
        postwrite_id,
        claim_release_receipt_id,
        &authority,
        root_after_output,
    )?;
    let after = root_after
        .state()
        .product_families()
        .family(MarketFamilyV1::Fractional);
    require(
        after.counts().admitted == 1
            && after.counts().live == 0
            && after.counts().terminal == 1,
        ClutchError::MismatchedState,
    )?;
    let root_semantic_after = root_after
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            TERMINAL_DOMAIN_V2,
            program_id.as_ref(),
            root_account.key.as_ref(),
            &root.authentication_id().bytes(),
            &root_after.authentication_id().bytes(),
            &root_semantic_before.bytes(),
            &root_semantic_after.bytes(),
            &receipt_id.bytes(),
            &verification_id.bytes(),
            &postwrite_id.bytes(),
            &claim_release_receipt_id.bytes(),
            &runtime_release.authentication_id().bytes(),
            &claim_ledger_post_state_id.bytes(),
            &claim_ledger_transition_id.bytes(),
            &rent_disposition_id.bytes(),
        ])
        .to_bytes(),
    );
    require_live(id)?;
    Ok(AuthenticatedProductFractionalTerminalV1 { id })
}

#[cfg(test)]
mod hostile_contract_tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn market(byte: u8) -> MarketInstanceV2Id {
        MarketInstanceV2Id::from_bytes([byte; 32])
    }

    #[test]
    fn admission_authority_accepts_only_the_exact_private_tuple() {
        let authority = AdmissionRootAuthorityV1 {
            root_account: Pubkey::new_from_array([1; 32]),
            root_semantic_before: id(2),
            root_data_before: id(3),
            root_authentication_before: id(4),
            market_instance_id: market(5),
            generation: 6,
            fractional_root_id: id(7),
            sequence: 8,
            receipt_id: id(9),
            verification_id: id(10),
            postwrite_authentication_id: id(11),
        };
        assert!(authority
            .authenticate_fractional_family_admission_root_write_v1(
                authority.root_account,
                authority.root_semantic_before,
                authority.root_data_before,
                authority.root_authentication_before,
                authority.market_instance_id,
                authority.generation,
                authority.fractional_root_id,
                authority.sequence,
                authority.receipt_id,
                authority.verification_id,
                authority.postwrite_authentication_id,
            )
            .is_ok());
        assert!(authority
            .authenticate_fractional_family_admission_root_write_v1(
                authority.root_account,
                authority.root_semantic_before,
                authority.root_data_before,
                authority.root_authentication_before,
                authority.market_instance_id,
                authority.generation,
                authority.fractional_root_id,
                authority.sequence,
                authority.receipt_id,
                id(12),
                authority.postwrite_authentication_id,
            )
            .is_err());
    }

    #[test]
    fn terminal_authority_rejects_claim_release_receipt_substitution() {
        let authority = TerminalRootAuthorityV1 {
            root_account: Pubkey::new_from_array([1; 32]),
            root_semantic_before: id(2),
            root_data_before: id(3),
            root_authentication_before: id(4),
            market_instance_id: market(5),
            generation: 6,
            fractional_root_id: id(7),
            sequence: 8,
            receipt_id: id(9),
            policy_terminal_state_id: id(10),
            ledger_terminal_state_id: id(11),
            verification_id: id(12),
            postwrite_authentication_id: id(13),
            claim_release_receipt_id: id(14),
        };
        assert!(authority
            .authenticate_fractional_family_terminal_root_write_v1(
                authority.root_account,
                authority.root_semantic_before,
                authority.root_data_before,
                authority.root_authentication_before,
                authority.market_instance_id,
                authority.generation,
                authority.fractional_root_id,
                authority.sequence,
                authority.receipt_id,
                authority.policy_terminal_state_id,
                authority.ledger_terminal_state_id,
                authority.verification_id,
                authority.postwrite_authentication_id,
                authority.claim_release_receipt_id,
            )
            .is_ok());
        assert!(authority
            .authenticate_fractional_family_terminal_root_write_v1(
                authority.root_account,
                authority.root_semantic_before,
                authority.root_data_before,
                authority.root_authentication_before,
                authority.market_instance_id,
                authority.generation,
                authority.fractional_root_id,
                authority.sequence,
                authority.receipt_id,
                authority.policy_terminal_state_id,
                authority.ledger_terminal_state_id,
                authority.verification_id,
                authority.postwrite_authentication_id,
                id(15),
            )
            .is_err());
    }
}
