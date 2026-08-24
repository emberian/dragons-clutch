// SPDX-License-Identifier: AGPL-3.0-or-later

//! Private Fractional postwrite consumer for Product's current Market root.
//!
//! This module owns no a4/a5/ClaimLedger or Product root truth and has no
//! dispatch route. It projects the exact private Fractional postwrite into
//! Product's default-refusing owner trait; Product alone derives and writes the
//! `0xaa/v3` successor.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use clutch_fractional_redemption_runtime::{
    FractionalFamilyAdmissionReceiptV1, FractionalFamilyTerminalReceiptV1,
    FractionalInitializationPlanV1, FractionalRedemptionActionV1,
};
use clutch_product_series::{
    ContentId, MarketFoundationAccountGraphV4, MarketFoundationScheduleV4,
    MarketFamilyV1, MarketInstanceV2Id, MarketLifecycleRootV3,
};
use clutch_retirement::Identity32V1;
use clutch_solana_layout::product_series::MarketLifecycleRootAccountV3;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::fractional_redemption::{
    AuthenticatedFractionalFamilyAdmissionPostwriteV1, AuthenticatedFractionalRuntimeReleaseV1,
    AuthenticatedFractionalFamilyPhysicalTerminalV2,
};
use super::collateral_position_v3::AuthenticatedResolutionV5;
use super::product_market_family_admission_v3_current::{
    commit_product_family_admission_v3, prepare_product_family_admission_v3,
    AuthenticatedProductFamilyAdmissionOwnerV3,
    AuthenticatedProductFamilyAdmissionPlanV3,
    AuthenticatedProductFamilyAdmissionPostwriteV3,
    AuthenticatedProductFamilyAdmissionV3,
};
use super::product_market_family_capability_current::
    AuthenticatedMarketFamilyCapabilityPolicyV1;
use super::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, AuthenticatedMarketLifecycleRootV3,
    AuthenticatedSeriesMarketLinkV3,
};
use super::product_market_replay_current::AuthenticatedMarketLifecycleReplayV2;
use super::product_series_current::{
    consume_fractional_family_admission_postwrite_v2,
    consume_fractional_family_terminal_postwrite_v2,
    AuthenticatedProductFractionalFamilyAdmissionOwnerV2,
    AuthenticatedProductFractionalFamilyAdmissionV2,
    AuthenticatedProductFractionalFamilyTerminalOwnerV2,
    AuthenticatedProductFractionalFamilyTerminalV2,
};

const FRACTIONAL_FAMILY_ADMISSION_PREWRITE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/fractional-family-admission-prewrite/v3\0";
const FRACTIONAL_FAMILY_ADMISSION_POSTWRITE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/fractional-family-admission-postwrite/v3\0";

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

/// Move-only instruction-local owner produced before any a4/a5/ClaimLedger
/// write. It binds the predicted physical postimages to the exact hostile
/// RootV3, replay, family policy, LinkV3, ScheduleV4, and GraphV4 prestates.
#[derive(Debug)]
pub(crate) struct PreparedFractionalFamilyAdmissionV3 {
    id: ContentId,
    program_id: Pubkey,
    root_account: Pubkey,
    root_binding_id: ContentId,
    root_authentication_id: ContentId,
    root_semantic_id: ContentId,
    root_transition_sequence: u64,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    family_policy_id: ContentId,
    family_policy_authentication_id: ContentId,
    family_namespace_anchor_id: ContentId,
    family_admission_sequence: u32,
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
    runtime_release_authentication_id: ContentId,
    resolution_account: Pubkey,
    resolution_semantic_id: ContentId,
    resolution_data_id: ContentId,
    native_claim_basis_id: ContentId,
    fractional_admission_receipt_id: ContentId,
    link_account: Pubkey,
    link_binding_id: ContentId,
    link_authentication_id: ContentId,
    link_semantic_id: ContentId,
    schedule_id: ContentId,
    graph_id: ContentId,
}

impl AuthenticatedProductFamilyAdmissionOwnerV3 for PreparedFractionalFamilyAdmissionV3 {
    fn family(&self) -> Outcome<MarketFamilyV1> { Ok(MarketFamilyV1::Fractional) }

    fn child_account(&self) -> Outcome<Pubkey> { Ok(self.policy_account) }

    fn owner_prewrite_id(&self) -> Outcome<ContentId> { Ok(self.id) }

    #[allow(clippy::too_many_arguments)]
    fn authenticate_product_family_admission_owner_v3(
        &self,
        program_id: &Pubkey,
        root_account: Pubkey,
        root_binding_id: ContentId,
        root_authentication_id: ContentId,
        root_semantic_id: ContentId,
        root_transition_sequence: u64,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        family_policy_id: ContentId,
        family_policy_authentication_id: ContentId,
        family: MarketFamilyV1,
        family_namespace_anchor_id: ContentId,
        family_admission_sequence: u32,
        child_account: Pubkey,
        owner_prewrite_id: ContentId,
    ) -> Outcome<()> {
        require(
            *program_id == self.program_id
                && root_account == self.root_account
                && root_binding_id == self.root_binding_id
                && root_authentication_id == self.root_authentication_id
                && root_semantic_id == self.root_semantic_id
                && root_transition_sequence == self.root_transition_sequence
                && market_instance_id == self.market_instance_id
                && generation == self.generation
                && family_policy_id == self.family_policy_id
                && family_policy_authentication_id == self.family_policy_authentication_id
                && family == MarketFamilyV1::Fractional
                && family_namespace_anchor_id == self.family_namespace_anchor_id
                && family_admission_sequence == self.family_admission_sequence
                && child_account == self.policy_account
                && owner_prewrite_id == self.id,
            ClutchError::MismatchedState,
        )
    }
}

#[derive(Debug)]
struct FractionalFamilyAdmissionPostwriteV3 {
    prepared: PreparedFractionalFamilyAdmissionV3,
    physical: AuthenticatedFractionalFamilyAdmissionPostwriteV1,
}

impl AuthenticatedProductFamilyAdmissionPostwriteV3 for FractionalFamilyAdmissionPostwriteV3 {
    #[allow(clippy::too_many_arguments)]
    fn consume_product_family_admission_postwrite_v3(
        self,
        plan_id: ContentId,
        root_account: Pubkey,
        root_binding_id: ContentId,
        root_semantic_before_id: ContentId,
        _root_semantic_after_id: ContentId,
        root_transition_sequence_before: u64,
        _root_transition_sequence_after: u64,
        family: MarketFamilyV1,
        family_namespace_anchor_id: ContentId,
        _family_prestate_id: ContentId,
        _family_poststate_id: ContentId,
        family_admission_sequence: u32,
        family_admission_receipt_id: ContentId,
        child_account: Pubkey,
        owner_prewrite_id: ContentId,
    ) -> Outcome<ContentId> {
        let admission = self.physical.family_admission();
        let release = self.physical.runtime_release();
        require(
            root_account == self.prepared.root_account
                && root_binding_id == self.prepared.root_binding_id
                && root_semantic_before_id == self.prepared.root_semantic_id
                && root_transition_sequence_before == self.prepared.root_transition_sequence
                && family == MarketFamilyV1::Fractional
                && family_namespace_anchor_id == self.prepared.family_namespace_anchor_id
                && family_admission_sequence == self.prepared.family_admission_sequence
                && child_account == self.prepared.policy_account
                && owner_prewrite_id == self.prepared.id
                && admission.market_instance().bytes() == self.prepared.market_instance_id.bytes()
                && admission.domain_generation() == self.prepared.generation
                && pubkey(admission.policy_account()) == self.prepared.policy_account
                && content(admission.policy_state_id()) == self.prepared.policy_state_id
                && pubkey(admission.ledger_account()) == self.prepared.ledger_account
                && content(admission.ledger_state_id()) == self.prepared.ledger_state_id
                && pubkey(admission.claim_ledger_account()) == self.prepared.claim_ledger_account
                && content(admission.claim_ledger_before_id())
                    == self.prepared.claim_ledger_before_id
                && content(admission.claim_ledger_after_id())
                    == self.prepared.claim_ledger_after_id
                && content(admission.latch_transition_id())
                    == self.prepared.claim_ledger_latch_transition_id
                && content(admission.claim_issuance_binding())
                    == self.prepared.claim_issuance_binding_id
                && content(admission.receipt_id())
                    == self.prepared.fractional_admission_receipt_id
                && content(release.release_id()) == self.prepared.runtime_release_id
                && release.capability_profile_id() == self.prepared.capability_profile_id
                && content(release.authentication_id())
                    == self.prepared.runtime_release_authentication_id
                && pubkey(self.physical.resolution_account())
                    == self.prepared.resolution_account
                && content(self.physical.resolution_semantic_id())
                    == self.prepared.resolution_semantic_id
                && content(self.physical.resolution_data_id())
                    == self.prepared.resolution_data_id
                && content(self.physical.native_claim_basis_id())
                    == self.prepared.native_claim_basis_id,
            ClutchError::MismatchedState,
        )?;
        let id = ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                FRACTIONAL_FAMILY_ADMISSION_POSTWRITE_DOMAIN_V3,
                &plan_id.bytes(),
                &family_admission_receipt_id.bytes(),
                &self.prepared.id.bytes(),
                &content(self.physical.verification_id()).bytes(),
                &content(self.physical.authentication_id()).bytes(),
                &self.prepared.link_binding_id.bytes(),
                &self.prepared.link_authentication_id.bytes(),
                &self.prepared.link_semantic_id.bytes(),
                &self.prepared.schedule_id.bytes(),
                &self.prepared.graph_id.bytes(),
            ])
            .to_bytes(),
        );
        require(id != ContentId::ZERO, ClutchError::MismatchedState)?;
        Ok(id)
    }
}

/// Prepare Product's generic RootV3 admission before any Fractional account
/// mutation. The physical plan contributes only deterministic a4/a5 and
/// ClaimLedger successor facts; all Product truth is hostile-authenticated.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_fractional_admission_v3(
    program_id: &Pubkey,
    root: &AuthenticatedMarketLifecycleRootV3<'_>,
    replay: &AuthenticatedMarketLifecycleReplayV2,
    family_policy: &AuthenticatedMarketFamilyCapabilityPolicyV1,
    link: &AuthenticatedSeriesMarketLinkV3<'_>,
    schedule: &MarketFoundationScheduleV4,
    graph: &MarketFoundationAccountGraphV4,
    runtime_release: &AuthenticatedFractionalRuntimeReleaseV1,
    resolution: &AuthenticatedResolutionV5,
    native_claim_basis_id: ContentId,
    physical_plan: &FractionalInitializationPlanV1,
    successor_output: &mut MarketLifecycleRootV3,
) -> Outcome<(
    AuthenticatedProductFamilyAdmissionPlanV3,
    PreparedFractionalFamilyAdmissionV3,
)> {
    let admission = physical_plan.family_admission;
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let binding = root.binding();
    let family_namespace_anchor_id =
        family_policy.family_namespace_anchors()[MarketFamilyV1::Fractional.index()];
    let family_admission_sequence = root
        .state()
        .product_families()
        .family(MarketFamilyV1::Fractional)
        .counts()
        .admitted;
    let link_semantic_id = ContentId::from_bytes(link.semantic_id().bytes());
    let prepared = PreparedFractionalFamilyAdmissionV3 {
        id: ContentId::ZERO,
        program_id: *program_id,
        root_account: root.account(),
        root_binding_id: root.binding_id(),
        root_authentication_id: root.authentication_id(),
        root_semantic_id: root.semantic_id(),
        root_transition_sequence: root.state().transition_sequence(),
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        family_policy_id: family_policy.policy_id(),
        family_policy_authentication_id: family_policy.id(),
        family_namespace_anchor_id,
        family_admission_sequence,
        policy_account: pubkey(admission.policy_account()),
        policy_state_id: content(admission.policy_state_id()),
        ledger_account: pubkey(admission.ledger_account()),
        ledger_state_id: content(admission.ledger_state_id()),
        claim_ledger_account: pubkey(admission.claim_ledger_account()),
        claim_ledger_before_id: content(admission.claim_ledger_before_id()),
        claim_ledger_after_id: content(admission.claim_ledger_after_id()),
        claim_ledger_latch_transition_id: content(admission.latch_transition_id()),
        claim_issuance_binding_id: content(admission.claim_issuance_binding()),
        runtime_release_id: content(runtime_release.release_id()),
        capability_profile_id: runtime_release.capability_profile_id(),
        runtime_release_authentication_id: content(runtime_release.authentication_id()),
        resolution_account: Pubkey::new_from_array(resolution.account_id.bytes()),
        resolution_semantic_id: ContentId::from_bytes(resolution.semantic_id.bytes()),
        resolution_data_id: ContentId::from_bytes(resolution.data_id.bytes()),
        native_claim_basis_id,
        fractional_admission_receipt_id: content(admission.receipt_id()),
        link_account: link.account(),
        link_binding_id: link.binding_id(),
        link_authentication_id: link.authentication_id(),
        link_semantic_id,
        schedule_id: schedule_id.content_id(),
        graph_id,
    };
    require(
        runtime_release.action() == FractionalRedemptionActionV1::Initialize
            && admission.market_instance().bytes() == binding.market_instance_id.bytes()
            && admission.domain_generation() == binding.generation
            && prepared.policy_account != prepared.ledger_account
            && prepared.policy_account != prepared.claim_ledger_account
            && prepared.ledger_account != prepared.claim_ledger_account
            && prepared.link_account == link.account()
            && link.state().phase()
                == clutch_product_series::SeriesMarketLinkPhaseV3::Active
            && link.binding().market_root_account_id.bytes() == root.account().to_bytes()
            && link.binding().market_instance_id == binding.market_instance_id
            && link.binding().generation == binding.generation
            && binding.foundation_schedule_id == schedule_id
            && binding.foundation_account_graph_id == graph_id
            && graph.market_instance_id == binding.market_instance_id
            && graph.generation == binding.generation
            && graph
                .account(clutch_product_series::MarketFoundationSlotV4::FractionalPolicy)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == prepared.policy_account.to_bytes()
            && graph
                .account(clutch_product_series::MarketFoundationSlotV4::FractionalLedger)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == prepared.ledger_account.to_bytes()
            && graph
                .account(clutch_product_series::MarketFoundationSlotV4::ClaimLedger)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == prepared.claim_ledger_account.to_bytes()
            && prepared.resolution_account.to_bytes()
                == graph
                    .account(clutch_product_series::MarketFoundationSlotV4::ResolutionV5)
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    .bytes(),
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FRACTIONAL_FAMILY_ADMISSION_PREWRITE_DOMAIN_V3,
            program_id.as_ref(),
            prepared.root_account.as_ref(),
            &prepared.root_binding_id.bytes(),
            &prepared.root_authentication_id.bytes(),
            &prepared.root_semantic_id.bytes(),
            &prepared.root_transition_sequence.to_le_bytes(),
            &prepared.market_instance_id.bytes(),
            &prepared.generation.to_le_bytes(),
            &prepared.family_policy_id.bytes(),
            &prepared.family_policy_authentication_id.bytes(),
            &prepared.family_namespace_anchor_id.bytes(),
            &prepared.family_admission_sequence.to_le_bytes(),
            prepared.policy_account.as_ref(),
            &prepared.policy_state_id.bytes(),
            prepared.ledger_account.as_ref(),
            &prepared.ledger_state_id.bytes(),
            prepared.claim_ledger_account.as_ref(),
            &prepared.claim_ledger_before_id.bytes(),
            &prepared.claim_ledger_after_id.bytes(),
            &prepared.claim_ledger_latch_transition_id.bytes(),
            &prepared.claim_issuance_binding_id.bytes(),
            &prepared.runtime_release_id.bytes(),
            &prepared.capability_profile_id.bytes(),
            &prepared.runtime_release_authentication_id.bytes(),
            prepared.resolution_account.as_ref(),
            &prepared.resolution_semantic_id.bytes(),
            &prepared.resolution_data_id.bytes(),
            &prepared.native_claim_basis_id.bytes(),
            &prepared.fractional_admission_receipt_id.bytes(),
            prepared.link_account.as_ref(),
            &prepared.link_binding_id.bytes(),
            &prepared.link_authentication_id.bytes(),
            &prepared.link_semantic_id.bytes(),
            &prepared.schedule_id.bytes(),
            &prepared.graph_id.bytes(),
        ])
        .to_bytes(),
    );
    require(id != ContentId::ZERO, ClutchError::MismatchedState)?;
    let prepared = PreparedFractionalFamilyAdmissionV3 { id, ..prepared };
    let product = prepare_product_family_admission_v3(
        program_id,
        root,
        replay,
        family_policy,
        &prepared,
        successor_output,
    )?;
    Ok((product, prepared))
}

/// Consume the actual a4/a5/ClaimLedger postwrite and persist RootV3 last.
#[allow(clippy::too_many_arguments)]
pub(crate) fn commit_fractional_admission_v3<'next>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    product_plan: AuthenticatedProductFamilyAdmissionPlanV3,
    prepared: PreparedFractionalFamilyAdmissionV3,
    physical: AuthenticatedFractionalFamilyAdmissionPostwriteV1,
    root_before_output: &mut MarketLifecycleRootAccountV3,
    successor_output: &mut MarketLifecycleRootV3,
    rebound_output: &'next mut MarketLifecycleRootAccountV3,
) -> Outcome<(
    AuthenticatedMarketLifecycleRootV3<'next>,
    AuthenticatedProductFamilyAdmissionV3,
)> {
    commit_product_family_admission_v3(
        program_id,
        root_account,
        product_plan,
        FractionalFamilyAdmissionPostwriteV3 { prepared, physical },
        root_before_output,
        successor_output,
        rebound_output,
    )
}

#[cfg(test)]
mod current_admission_contract_tests {
    use super::*;

    #[test]
    fn current_fractional_admission_is_move_only_and_link_evidence_only() {
        let source = include_str!("fractional_product_consumer.rs");
        let start = source
            .find("pub(crate) struct PreparedFractionalFamilyAdmissionV3")
            .expect("current Fractional prewrite");
        let end = source[start..]
            .find("struct FractionalAdmissionOwnerV2")
            .map(|offset| start + offset)
            .expect("current Fractional admission boundary");
        let current = &source[start..end];
        assert!(!current.contains("derive(Clone"));
        assert!(!current.contains("SeriesLinkObligationV3"));
        assert!(current.contains("link_account"));
        assert!(current.contains("link_binding_id"));
        assert!(current.contains("link_authentication_id"));
        assert!(current.contains("link_semantic_id"));
        assert!(current.contains("SeriesMarketLinkPhaseV3::Active"));
        assert!(current.contains("prepare_product_family_admission_v3("));
        assert!(current.contains("commit_product_family_admission_v3("));
    }
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
    terminal: AuthenticatedFractionalFamilyPhysicalTerminalV2,
}

impl FractionalTerminalOwnerV2 {
    const fn terminal(&self) -> FractionalFamilyTerminalReceiptV1 {
        self.terminal.family_terminal()
    }
}

impl AuthenticatedProductFractionalFamilyTerminalOwnerV2 for FractionalTerminalOwnerV2 {
    fn terminal_receipt_id(&self) -> Outcome<ContentId> {
        Ok(self.terminal.id())
    }

    fn verification_id(&self) -> Outcome<ContentId> {
        Ok(content(self.terminal.verification_id()))
    }

    fn postwrite_authentication_id(&self) -> Outcome<ContentId> {
        Ok(content(self.terminal.postwrite_authentication_id()))
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
        Ok(content(self.terminal.claim_release_receipt_id()))
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
        let release = self.terminal.runtime_release();
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
                && claim_release_receipt_id == content(self.terminal.claim_release_receipt_id())
                && rent_disposition_id == content(terminal.rent_disposition_id())
                && resolution_account == pubkey(self.terminal.resolution_account())
                && resolution_semantic_id == content(self.terminal.resolution_semantic_id())
                && resolution_data_id == content(self.terminal.resolution_data_id())
                && native_claim_basis_id == content(self.terminal.native_claim_basis_id())
                && terminal_receipt_id == self.terminal.id()
                && verification_id == content(self.terminal.verification_id())
                && postwrite_authentication_id
                    == content(self.terminal.postwrite_authentication_id()),
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
    link: &AuthenticatedSeriesMarketLinkV3<'_>,
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
        link,
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

    #[test]
    fn product_terminal_authority_uses_the_physical_close_receipt() {
        let source = include_str!("fractional_product_consumer.rs");
        assert!(source.contains("fn terminal_receipt_id(&self) -> Outcome<ContentId> {\n        Ok(self.terminal.id())"));
        assert!(source.contains("terminal_receipt_id == self.terminal.id()"));
        assert!(!source.contains(
            "fn terminal_receipt_id(&self) -> Outcome<ContentId> {\n        Ok(content(self.terminal().receipt_id()))"
        ));
    }
}

/// Consume the move-only action-10 receipt after physical a4/v3 and a5/v1 close.
#[allow(clippy::too_many_arguments)]
pub(crate) fn consume_fractional_terminal_v2(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    terminal: AuthenticatedFractionalFamilyPhysicalTerminalV2,
    link: &AuthenticatedSeriesMarketLinkV3<'_>,
    schedule: &MarketFoundationScheduleV4,
    graph: &MarketFoundationAccountGraphV4,
    root_before_output: &mut MarketLifecycleRootAccountV3,
    root_successor_output: &mut MarketLifecycleRootV3,
    root_after_output: &mut MarketLifecycleRootAccountV3,
) -> Outcome<AuthenticatedProductFractionalFamilyTerminalV2> {
    let family_terminal = terminal.family_terminal();
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        MarketInstanceV2Id::from_bytes(family_terminal.market_instance_id().bytes()),
        family_terminal.domain_generation(),
        true,
        root_before_output,
    )?;
    let owner = FractionalTerminalOwnerV2 { terminal };
    let (_, accepted) = consume_fractional_family_terminal_postwrite_v2(
        program_id,
        root_account,
        root,
        link,
        &owner,
        schedule,
        graph,
        root_successor_output,
        root_after_output,
    )?;
    Ok(accepted)
}
