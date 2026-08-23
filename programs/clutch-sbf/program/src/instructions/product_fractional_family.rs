// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact Fractional postwrite promotion into the shared Product Market root.
//!
//! This module has no dispatch route. It consumes only private receipts minted
//! after hostile a4/a5/ClaimLedger postwrite authentication, joins them to the
//! immutable Product Foundation graph, and delegates the sole `0xaa` mutation
//! to Product's narrow, default-refusing writer.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use clutch_fractional_redemption_runtime::{
    FractionalFamilyTerminalReceiptV1, FractionalRedemptionActionV1,
};
use clutch_product_series::{
    ContentId, MarketFamilyStatusV1, MarketFamilyV1, MarketFoundationAccountGraphV2,
    MarketFoundationScheduleV2, MarketFoundationSlotV2, MarketInstanceV2Id,
    MarketLifecyclePhaseV1,
};
use clutch_solana_layout::product_series::MarketLifecycleRootAccountV1;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::fractional_redemption::AuthenticatedFractionalFamilyTerminalPostwriteV1;
use super::product_market::{
    authenticate_market_lifecycle_root_v1,
    write_authenticated_fractional_family_terminal_root_v1,
    AuthenticatedFractionalFamilyTerminalRootWriteV1, AuthenticatedMarketLifecycleRootV1,
};

const PRODUCT_FRACTIONAL_TERMINAL_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-fractional-terminal/v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductFractionalFamilyTerminalV1 {
    id: ContentId,
    root_account: Pubkey,
    root_authentication_before: ContentId,
    root_authentication_after: ContentId,
    root_semantic_before: ContentId,
    root_semantic_after: ContentId,
    fractional_terminal_receipt_id: ContentId,
    fractional_verification_id: ContentId,
    fractional_postwrite_authentication_id: ContentId,
    policy_terminal_state_id: ContentId,
    ledger_terminal_state_id: ContentId,
    claim_ledger_post_state_id: ContentId,
    claim_ledger_transition_id: ContentId,
    rent_disposition_id: ContentId,
}

impl AuthenticatedProductFractionalFamilyTerminalV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn root_account(self) -> Pubkey {
        self.root_account
    }

    pub(crate) const fn root_authentication_before(self) -> ContentId {
        self.root_authentication_before
    }

    pub(crate) const fn root_authentication_after(self) -> ContentId {
        self.root_authentication_after
    }

    pub(crate) const fn root_semantic_before(self) -> ContentId {
        self.root_semantic_before
    }

    pub(crate) const fn root_semantic_after(self) -> ContentId {
        self.root_semantic_after
    }

    pub(crate) const fn fractional_terminal_receipt_id(self) -> ContentId {
        self.fractional_terminal_receipt_id
    }

    pub(crate) const fn fractional_verification_id(self) -> ContentId {
        self.fractional_verification_id
    }

    pub(crate) const fn fractional_postwrite_authentication_id(self) -> ContentId {
        self.fractional_postwrite_authentication_id
    }

    pub(crate) const fn policy_terminal_state_id(self) -> ContentId {
        self.policy_terminal_state_id
    }

    pub(crate) const fn ledger_terminal_state_id(self) -> ContentId {
        self.ledger_terminal_state_id
    }

    pub(crate) const fn claim_ledger_post_state_id(self) -> ContentId {
        self.claim_ledger_post_state_id
    }

    pub(crate) const fn claim_ledger_transition_id(self) -> ContentId {
        self.claim_ledger_transition_id
    }

    pub(crate) const fn rent_disposition_id(self) -> ContentId {
        self.rent_disposition_id
    }
}

struct FractionalTerminalRootAuthorityV1 {
    root_account: Pubkey,
    root_semantic_before: ContentId,
    root_data_before: ContentId,
    root_authentication_before: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    fractional_root_id: ContentId,
    family_terminal_sequence: u32,
    terminal_receipt_id: ContentId,
    policy_terminal_state_id: ContentId,
    ledger_terminal_state_id: ContentId,
    verification_id: ContentId,
    postwrite_authentication_id: ContentId,
}

impl AuthenticatedFractionalFamilyTerminalRootWriteV1 for FractionalTerminalRootAuthorityV1 {
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
    ) -> Outcome<()> {
        require(
            root_account == self.root_account
                && root_semantic_before == self.root_semantic_before
                && root_data_before == self.root_data_before
                && root_authentication_before == self.root_authentication_before
                && market_instance_id == self.market_instance_id
                && generation == self.generation
                && fractional_root_id == self.fractional_root_id
                && family_terminal_sequence == self.family_terminal_sequence
                && fractional_terminal_receipt_id == self.terminal_receipt_id
                && fractional_policy_terminal_state_id == self.policy_terminal_state_id
                && fractional_ledger_terminal_state_id == self.ledger_terminal_state_id
                && fractional_verification_id == self.verification_id
                && fractional_postwrite_authentication_id == self.postwrite_authentication_id,
            ClutchError::MismatchedState,
        )
    }
}

fn authenticate_root_from_body<'a>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_market: MarketInstanceV2Id,
    expected_generation: u64,
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
        expected_market,
        expected_generation,
        true,
        output,
    )
}

fn content(id: clutch_retirement::Identity32V1) -> ContentId {
    ContentId::from_bytes(id.bytes())
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(id != ContentId::ZERO, ClutchError::MismatchedState)
}

/// Consume one exact Fractional terminal postwrite into `0xaa` before the
/// outer Fractional instruction deletes a4/a5 and applies both rent splits.
#[allow(clippy::too_many_arguments)]
pub(crate) fn consume_fractional_family_terminal_postwrite_v1(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    postwrite: AuthenticatedFractionalFamilyTerminalPostwriteV1,
    schedule: &MarketFoundationScheduleV2,
    graph: &MarketFoundationAccountGraphV2,
    root_before_output: &mut MarketLifecycleRootAccountV1,
    root_after_output: &mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedProductFractionalFamilyTerminalV1> {
    let terminal = postwrite.family_terminal();
    let market_instance_id = MarketInstanceV2Id::from_bytes(terminal.market_instance_id().bytes());
    let generation = terminal.domain_generation();
    let root = authenticate_root_from_body(
        program_id,
        root_account,
        market_instance_id,
        generation,
        root_before_output,
    )?;
    let binding = root.state().binding();
    let root_semantic_before = root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let runtime_release = postwrite.runtime_release();
    let verification_id = content(postwrite.verification_id());
    let postwrite_authentication_id = content(postwrite.authentication_id());
    let terminal_receipt_id = content(terminal.receipt_id());
    let policy_account = content(terminal.policy_account());
    let ledger_account = content(terminal.ledger_account());
    let claim_ledger_account = content(terminal.claim_ledger_account());
    let policy_terminal_state_id = content(terminal.policy_terminal_state_id());
    let ledger_terminal_state_id = content(terminal.ledger_terminal_state_id());
    let claim_ledger_post_state_id = content(terminal.claim_ledger_post_state_id());
    let claim_ledger_transition_id = content(terminal.claim_ledger_transition_id());
    let rent_disposition_id = content(terminal.rent_disposition_id());
    let fractional_release_id = content(terminal.fractional_release_id());
    let resolution_account = content(postwrite.resolution_account());
    let resolution_semantic_id = content(postwrite.resolution_semantic_id());
    let resolution_data_id = content(postwrite.resolution_data_id());
    let native_claim_basis_id = content(postwrite.native_claim_basis_id());
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
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
        verification_id,
        postwrite_authentication_id,
        terminal_receipt_id,
        policy_terminal_state_id,
        ledger_terminal_state_id,
        claim_ledger_post_state_id,
        claim_ledger_transition_id,
        rent_disposition_id,
        fractional_release_id,
        resolution_semantic_id,
        resolution_data_id,
        native_claim_basis_id,
    ] {
        require_live(id)?;
    }
    require(
        matches!(
            root.state().phase(),
            MarketLifecyclePhaseV1::Active | MarketLifecyclePhaseV1::Retiring
        ) && root.is_writable()
            && binding.foundation_schedule_id == schedule_id
            && binding.foundation_account_graph_id == graph_id
            && graph.market_instance_id == binding.market_instance_id
            && graph.generation == binding.generation
            && graph.account(MarketFoundationSlotV2::FractionalPolicy)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == policy_account
            && graph.account(MarketFoundationSlotV2::FractionalLedger)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == ledger_account
            && graph.account(MarketFoundationSlotV2::ClaimLedger)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == claim_ledger_account
            && fractional_root_id == policy_account
            && binding.resolution_account_id == resolution_account
            && root.state().resolution_semantic_id() == resolution_semantic_id
            && root.state().resolution_data_id() == resolution_data_id
            && binding.native_claim_basis_id == native_claim_basis_id
            && runtime_release.action() == FractionalRedemptionActionV1::CloseEmptyLedger
            && content(runtime_release.release_id()) == binding.registry_release_id
            && runtime_release.capability_profile_id() == binding.capability_profile_id
            && fractional_release_id == content(runtime_release.release_id())
            && fractional.status() == MarketFamilyStatusV1::Live
            && fractional.counts().admitted == 1
            && fractional.counts().live == 1
            && policy_account != ledger_account
            && policy_account != claim_ledger_account
            && ledger_account != claim_ledger_account
            && policy_terminal_state_id != ledger_terminal_state_id
            && terminal_receipt_id != verification_id
            && terminal_receipt_id != postwrite_authentication_id
            && verification_id != postwrite_authentication_id,
        ClutchError::MismatchedState,
    )?;
    let family_terminal_sequence = fractional.counts().terminal;
    let authority = FractionalTerminalRootAuthorityV1 {
        root_account: *root_account.key,
        root_semantic_before,
        root_data_before: root.data_id(),
        root_authentication_before: root.authentication_id(),
        market_instance_id,
        generation,
        fractional_root_id,
        family_terminal_sequence,
        terminal_receipt_id,
        policy_terminal_state_id,
        ledger_terminal_state_id,
        verification_id,
        postwrite_authentication_id,
    };
    let root_after = write_authenticated_fractional_family_terminal_root_v1(
        program_id,
        root_account,
        root,
        family_terminal_sequence,
        terminal_receipt_id,
        policy_terminal_state_id,
        ledger_terminal_state_id,
        verification_id,
        postwrite_authentication_id,
        &authority,
        root_after_output,
    )?;
    let root_semantic_after = root_after
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_after = root_after
        .state()
        .product_families()
        .family(MarketFamilyV1::Fractional);
    require(
        terminal_after.counts().admitted == 1
            && terminal_after.counts().live == 0
            && terminal_after.counts().terminal == 1,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_FRACTIONAL_TERMINAL_AUTHENTICATION_DOMAIN_V1,
            program_id.as_ref(),
            root_account.key.as_ref(),
            &root.authentication_id().bytes(),
            &root_after.authentication_id().bytes(),
            &root_semantic_before.bytes(),
            &root_semantic_after.bytes(),
            &terminal_receipt_id.bytes(),
            &verification_id.bytes(),
            &postwrite_authentication_id.bytes(),
            &policy_terminal_state_id.bytes(),
            &ledger_terminal_state_id.bytes(),
            &claim_ledger_post_state_id.bytes(),
            &claim_ledger_transition_id.bytes(),
            &rent_disposition_id.bytes(),
            &binding.market_instance_id.bytes(),
            &binding.generation.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live(id)?;
    Ok(AuthenticatedProductFractionalFamilyTerminalV1 {
        id,
        root_account: *root_account.key,
        root_authentication_before: root.authentication_id(),
        root_authentication_after: root_after.authentication_id(),
        root_semantic_before,
        root_semantic_after,
        fractional_terminal_receipt_id: terminal_receipt_id,
        fractional_verification_id: verification_id,
        fractional_postwrite_authentication_id: postwrite_authentication_id,
        policy_terminal_state_id,
        ledger_terminal_state_id,
        claim_ledger_post_state_id,
        claim_ledger_transition_id,
        rent_disposition_id,
    })
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn authority() -> FractionalTerminalRootAuthorityV1 {
        FractionalTerminalRootAuthorityV1 {
            root_account: Pubkey::new_from_array([1; 32]),
            root_semantic_before: id(2),
            root_data_before: id(3),
            root_authentication_before: id(4),
            market_instance_id: MarketInstanceV2Id::from_bytes([5; 32]),
            generation: 6,
            fractional_root_id: id(7),
            family_terminal_sequence: 0,
            terminal_receipt_id: id(8),
            policy_terminal_state_id: id(9),
            ledger_terminal_state_id: id(10),
            verification_id: id(11),
            postwrite_authentication_id: id(12),
        }
    }

    #[test]
    fn fractional_terminal_authority_refuses_receipt_and_state_substitution() {
        let authority = authority();
        assert!(authority
            .authenticate_fractional_family_terminal_root_write_v1(
                authority.root_account,
                authority.root_semantic_before,
                authority.root_data_before,
                authority.root_authentication_before,
                authority.market_instance_id,
                authority.generation,
                authority.fractional_root_id,
                authority.family_terminal_sequence,
                authority.terminal_receipt_id,
                authority.policy_terminal_state_id,
                authority.ledger_terminal_state_id,
                authority.verification_id,
                authority.postwrite_authentication_id,
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
                authority.family_terminal_sequence,
                id(13),
                authority.policy_terminal_state_id,
                authority.ledger_terminal_state_id,
                authority.verification_id,
                authority.postwrite_authentication_id,
            )
            .is_err());
        assert!(authority
            .authenticate_fractional_family_terminal_root_write_v1(
                authority.root_account,
                authority.root_semantic_before,
                authority.root_data_before,
                authority.root_authentication_before,
                authority.market_instance_id,
                authority.generation,
                authority.fractional_root_id,
                authority.family_terminal_sequence,
                authority.terminal_receipt_id,
                authority.ledger_terminal_state_id,
                authority.policy_terminal_state_id,
                authority.verification_id,
                authority.postwrite_authentication_id,
            )
            .is_err());
    }
}
