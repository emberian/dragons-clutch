// SPDX-License-Identifier: AGPL-3.0-or-later
//! Capability-disabled atomic Product/Failure Resolution V5 composer.
//!
//! This module is deliberately not routed by dispatch. It is the single live
//! writer boundary which joins Product's authenticated active Market root,
//! pinned Series link, and retained slot-10 preallocation to Failure's private
//! exhaustive interval receipt and Collateral's exact Hoard/ClaimLedger
//! postimage verifier. The preallocated Resolution PDA supplies only its
//! separately itemized rent principal; no Recovery work capital, Hoard
//! principal, future fees, or caller-selected funding source participates.

use crate::accounts::{expect_pda, require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::collateral_position_v3::{
    authenticate_market_resolution_activation_postwrite_v5,
    AuthenticatedMarketResolutionActivationPostwriteV5, GeneralMarketLiabilityAuthorityV2,
    RuntimeSha256,
};
use crate::instructions::failure_market_interval_v2::AuthenticatedFailureMarketProductResolutionV2;
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_system_program, SYSTEM_PROGRAM_ID,
};
use crate::instructions::product_artifact::AuthenticatedRegistryCapabilityV3;
use crate::instructions::product_market::{
    authenticate_market_lifecycle_root_v1, authenticate_series_market_link_v1,
    write_market_lifecycle_root_v1, AuthenticatedMarketFoundationPreallocationV2,
    AuthenticatedMarketLifecycleRootV1, AuthenticatedSeriesMarketLinkV1,
};
use crate::instructions::product_series::AuthenticatedCompiledProductSeriesBundleV5;
use crate::seeds;
use clutch_collateral_adapter_v2::{
    prepare_market_resolution_activation_v5, ClaimLedgerV3, HoardV2, Id as CollateralId,
    MarketLiabilityLifecycleV1, ResolutionFinalizationFactsV5, ResolutionPayoutUnitBoundaryV5,
    ResolutionV5, CLAIM_LEDGER_V3_BYTES, HOARD_V2_BYTES, RESOLUTION_V5_BYTES,
};
use clutch_failure_policy_runtime::market_interval_cell_v2::FailureMarketIntervalCellResolutionReceiptV2;
use clutch_product_series::{
    ContentId, MarketFoundationSlotV2, MarketLifecyclePhaseV1, MarketResolutionActivationV1,
    SeriesMarketLinkPhaseV1,
};
use clutch_retirement::{DeletableRentOwnerV1, Identity32V1};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const FAILURE_MARKET_RESOLUTION_ACTIVATION_AUTHENTICATION_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/failure-market-resolution-activation/v5\0";
const FAILURE_MARKET_RESOLUTION_FINALIZATION_EVIDENCE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/failure-market-resolution-finalization-evidence/v5\0";

/// Stable byte committed for the only disposition admitted by this composer.
const RESOLVED_DISPOSITION_BYTE_V2: u8 = 1;
const _: () = assert!(clutch_retirement::MAX_OUTCOMES * 8 == 128);

/// Private same-call proof consumed by the reusable Failure cell writer.
///
/// Construction is possible only after the Resolution, Hoard, ClaimLedger,
/// and Product root postimages have all been hostile-reauthenticated. It
/// retains the complete private Failure receipt so the final cell write cannot
/// substitute another payout, certificate, session, or disposition.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthenticatedFailureMarketResolutionActivationV5 {
    id: ContentId,
    failure_resolution: FailureMarketIntervalCellResolutionReceiptV2,
    product_activation: MarketResolutionActivationV1,
    collateral_postwrite: AuthenticatedMarketResolutionActivationPostwriteV5,
    market_root: Pubkey,
    market_root_authentication_before: ContentId,
    market_root_authentication_after: ContentId,
    series_link: Pubkey,
    series_link_authentication: ContentId,
    slot10_preallocation_id: ContentId,
    finalization_evidence_id: ContentId,
}

impl AuthenticatedFailureMarketResolutionActivationV5 {
    /// Complete same-call authorization identity.
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    /// Product's exact once-only `0xaa` activation postimage.
    pub(crate) const fn product_activation(self) -> MarketResolutionActivationV1 {
        self.product_activation
    }

    /// Collateral's exact Resolution/Hoard/ClaimLedger postwrite proof.
    pub(crate) const fn collateral_postwrite(
        self,
    ) -> AuthenticatedMarketResolutionActivationPostwriteV5 {
        self.collateral_postwrite
    }

    /// Exact Product-retained slot-10 preallocation consumed by the writer.
    pub(crate) const fn slot10_preallocation_id(self) -> ContentId {
        self.slot10_preallocation_id
    }

    /// Exact evidence identity embedded in Resolution V5 and Product `0xaa`.
    pub(crate) const fn finalization_evidence_id(self) -> ContentId {
        self.finalization_evidence_id
    }

    /// Exact shared root and its authenticated pre/post identities.
    pub(crate) const fn market_root(self) -> Pubkey {
        self.market_root
    }

    pub(crate) const fn market_root_authentication_before(self) -> ContentId {
        self.market_root_authentication_before
    }

    pub(crate) const fn market_root_authentication_after(self) -> ContentId {
        self.market_root_authentication_after
    }

    /// Exact read-only initiating link and its live pin authentication.
    pub(crate) const fn series_link(self) -> Pubkey {
        self.series_link
    }

    pub(crate) const fn series_link_authentication(self) -> ContentId {
        self.series_link_authentication
    }
}

impl AuthenticatedFailureMarketProductResolutionV2
    for AuthenticatedFailureMarketResolutionActivationV5
{
    fn authenticate_failure_market_product_resolution(
        &self,
        expected: FailureMarketIntervalCellResolutionReceiptV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        let expected_certificate = expected.verified_payout().certificate();
        let retained_certificate = self.failure_resolution.verified_payout().certificate();
        if expected.id() != self.failure_resolution.id()
            || expected.failure_policy_binding_id()
                != self.failure_resolution.failure_policy_binding_id()
            || expected.facts() != self.failure_resolution.facts()
            || expected_certificate != retained_certificate
            || expected_certificate
                .id()
                .map_err(|_| clutch_failure_policy_runtime::Error::BindingMismatch)?
                .bytes()
                != self.product_activation.product_certificate_id().bytes()
            || expected.id().bytes()
                != self
                    .product_activation
                    .failure_resolution_receipt_id()
                    .bytes()
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Claim the exact retained Resolution V5 preallocation, atomically write the
/// three collateral postimages, advance Product's active root once, and mint
/// the sole private authority accepted by the Failure resolved-cell writer.
///
/// `root_before` and `link_before` are private Product receipts constructed by
/// their semantic owners. This function does not trust their copied values: it
/// hostile-reopens both physical accounts and requires byte-for-byte identical
/// authentication before any write. The link stays read-only and pinned; the
/// Failure cell is written only by the caller after this function returns.
#[allow(clippy::too_many_arguments)]
pub(crate) fn activate_failure_market_resolution_v5<'a, 'root, 'link, 'post>(
    program_id: &Pubkey,
    market_root_account: &AccountInfo<'a>,
    series_link_account: &AccountInfo<'a>,
    resolution_account: &AccountInfo<'a>,
    hoard_account: &AccountInfo<'a>,
    claim_ledger_account: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    root_before: AuthenticatedMarketLifecycleRootV1<'root>,
    link_before: AuthenticatedSeriesMarketLinkV1<'link>,
    registry: AuthenticatedRegistryCapabilityV3,
    bundle: AuthenticatedCompiledProductSeriesBundleV5,
    slot10: AuthenticatedMarketFoundationPreallocationV2,
    liabilities: GeneralMarketLiabilityAuthorityV2,
    failure_resolution: FailureMarketIntervalCellResolutionReceiptV2,
    root_decode_before: &'root mut MarketLifecycleRootAccountV1,
    link_decode_before: &'link mut SeriesMarketLinkAccountV1,
    root_decode_after: &'post mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedFailureMarketResolutionActivationV5> {
    require_system_program(system_program)?;
    require_distinct(&[
        market_root_account.clone(),
        series_link_account.clone(),
        resolution_account.clone(),
        hoard_account.clone(),
        claim_ledger_account.clone(),
        rent_sysvar.clone(),
        system_program.clone(),
    ])?;

    let expected_root_binding = root_before.state().binding();
    let expected_link_binding = link_before.state().binding();
    let live_root = authenticate_market_lifecycle_root_v1(
        program_id,
        market_root_account,
        expected_root_binding.market_instance_id,
        expected_root_binding.generation,
        true,
        root_decode_before,
    )?;
    let live_link = authenticate_series_market_link_v1(
        program_id,
        series_link_account,
        expected_link_binding.series_plan_id,
        expected_link_binding.ordinal,
        expected_link_binding.market_instance_id,
        expected_link_binding.generation,
        *market_root_account.key,
        false,
        link_decode_before,
    )?;
    require(
        live_root.account() == root_before.account()
            && live_root.authentication_id() == root_before.authentication_id()
            && live_root.value() == root_before.value()
            && live_link.account() == link_before.account()
            && live_link.authentication_id() == link_before.authentication_id()
            && live_link.value() == link_before.value(),
        ClutchError::MismatchedState,
    )?;

    let root = live_root.state();
    let root_binding = root.binding();
    let link = *live_link.state();
    let link_binding = link.binding();
    let registry_projection = registry.projection();
    let failure_facts = failure_resolution.facts();
    let verified_payout = failure_resolution.verified_payout();
    let certificate = verified_payout.certificate();
    let payout = verified_payout.payout();
    certificate
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    payout
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let certificate_id = certificate
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    require_current_product_failure_join(
        market_root_account,
        &live_root,
        root,
        root_binding_id,
        &live_link,
        link,
        registry,
        bundle,
        failure_resolution,
        certificate,
        certificate_id,
    )?;
    require(
        root.phase() == MarketLifecyclePhaseV1::Active
            && root.resolution_semantic_id() == ContentId::ZERO
            && root.resolution_data_id() == ContentId::ZERO
            && root.resolution_activation_receipt_id() == ContentId::ZERO
            && link.phase() == SeriesMarketLinkPhaseV1::Active
            && link.active_failure_sessions() == 1
            && failure_facts.session_binding_id.bytes()
                == link.failure_session_transcript_id().bytes()
            && failure_facts.market_instance_id == root_binding.market_instance_id
            && failure_facts.generation == root_binding.generation
            && failure_resolution.failure_policy_binding_id().bytes()
                == root_binding.market_failure_policy_binding_id.bytes()
            && failure_facts.product_certificate_id == certificate_id
            && certificate.product_template_id().bytes()
                == root_binding.product_template_id.bytes()
            && certificate.market_genesis_profile_id().bytes()
                == root_binding.market_genesis_profile_id.bytes()
            && certificate.native_claim_basis_id().bytes()
                == root_binding.native_claim_basis_id.bytes()
            && certificate.price_measure_policy_id().bytes()
                == root_binding.price_measure_policy_id.bytes()
            && certificate.capability_profile_id() == root_binding.capability_profile_id
            && certificate.interval_profile_id().bytes()
                == root_binding.interval_consensus_profile_id.bytes()
            && certificate.source_occurrence_id() == link_binding.source_occurrence_id
            && payout.active_len == root_binding.outcome_count
            && registry_projection.realm_collateral.neutral_lamport_sink
                == root.capital().neutral_lamport_sink,
        ClutchError::MismatchedState,
    )?;

    require_exact_collateral_prestate(
        program_id,
        hoard_account,
        claim_ledger_account,
        liabilities,
        root_binding,
        registry_projection,
    )?;
    require_exact_slot10_preallocation(
        program_id,
        resolution_account,
        rent_sysvar,
        live_root,
        slot10,
    )?;

    let expected_hoard_after_id = HoardV2 {
        lifecycle: MarketLiabilityLifecycleV1::Resolved,
        ..liabilities.hoard
    }
    .semantic_id(&RuntimeSha256)
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_claim_ledger_after_id = ClaimLedgerV3 {
        resolution_account: CollateralId::from_bytes(resolution_account.key.to_bytes()),
        lifecycle: MarketLiabilityLifecycleV1::Resolved,
        ..liabilities.claim_ledger
    }
    .semantic_id(&RuntimeSha256)
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let finalization_evidence_id = derive_finalization_evidence_id_v5(
        live_root,
        live_link,
        registry,
        bundle,
        slot10,
        liabilities,
        expected_hoard_after_id,
        expected_claim_ledger_after_id,
        failure_resolution,
        root_binding_id,
        certificate_id.content_id(),
        resolution_account.key,
        payout.active_len,
        payout.denominator,
        &payout.weights,
    )?;
    let rent = DeletableRentOwnerV1::from_persisted(
        Identity32V1::new(slot10.rent_refund_owner().to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        slot10.principal_lamports(),
        slot10.donation_lamports(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let (expected_resolution, resolution_bump) =
        seeds::resolution_v5_pda(program_id, &root_binding.market_instance_id.bytes());
    let resolution = ResolutionV5::finalized(
        ResolutionFinalizationFactsV5 {
            market_instance_id: CollateralId::from_bytes(root_binding.market_instance_id.bytes()),
            native_claim_basis_id: CollateralId::from_bytes(
                root_binding.native_claim_basis_id.bytes(),
            ),
            finalization_evidence_id: CollateralId::from_bytes(finalization_evidence_id.bytes()),
            outcome_count: payout.active_len,
            payout_denominator: payout.denominator,
            payout_weights: payout.weights,
            generation: root_binding.generation,
            payout_unit_boundary: ResolutionPayoutUnitBoundaryV5::ExactWholeCollateralAtoms,
        },
        resolution_bump,
        rent,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        expected_resolution == *resolution_account.key,
        ClutchError::MismatchedState,
    )?;
    let activation_plan = prepare_market_resolution_activation_v5(
        CollateralId::from_bytes(resolution_account.key.to_bytes()),
        resolution,
        liabilities.hoard,
        liabilities.claim_ledger,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        activation_plan.hoard_after_id() == expected_hoard_after_id
            && activation_plan.claim_ledger_after_id() == expected_claim_ledger_after_id,
        ClutchError::MismatchedState,
    )?;

    let product_activation = MarketResolutionActivationV1::new(
        root_binding,
        ContentId::from_bytes(activation_plan.resolution_id().bytes()),
        ContentId::from_bytes(activation_plan.resolution_data_id().bytes()),
        ContentId::from_bytes(failure_resolution.id().bytes()),
        certificate_id.content_id(),
        finalization_evidence_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let successor = root
        .record_resolution_activation(product_activation)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    write_collateral_activation_postimages_v5(
        program_id,
        system_program,
        resolution_account,
        hoard_account,
        claim_ledger_account,
        root_binding.market_instance_id.bytes(),
        resolution_bump,
        resolution,
        activation_plan.hoard_after(),
        activation_plan.claim_ledger_after(),
        slot10.observed_balance_lamports(),
    )?;
    let collateral_postwrite = authenticate_market_resolution_activation_postwrite_v5(
        program_id,
        liabilities,
        activation_plan,
        resolution_account,
        hoard_account,
        claim_ledger_account,
    )?;
    require(
        collateral_postwrite.plan() == activation_plan
            && collateral_postwrite.liability_authority_receipt_id() == liabilities.receipt_id,
        ClutchError::MismatchedState,
    )?;

    let root_after = write_market_lifecycle_root_v1(
        program_id,
        market_root_account,
        live_root,
        &successor,
        root_decode_after,
    )?;
    require(
        root_after.state().resolution_activation_receipt_id() == product_activation.id()
            && root_after.state().resolution_semantic_id()
                == product_activation.resolution_semantic_id()
            && root_after.state().resolution_data_id() == product_activation.resolution_data_id()
            && root_after.state().transition_sequence()
                == root
                    .transition_sequence()
                    .checked_add(1)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::MismatchedState,
    )?;

    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAILURE_MARKET_RESOLUTION_ACTIVATION_AUTHENTICATION_DOMAIN_V5,
            market_root_account.key.as_ref(),
            &live_root.authentication_id().bytes(),
            &root_after.authentication_id().bytes(),
            series_link_account.key.as_ref(),
            &live_link.authentication_id().bytes(),
            resolution_account.key.as_ref(),
            &slot10.id().bytes(),
            &failure_resolution.id().bytes(),
            &certificate_id.bytes(),
            &finalization_evidence_id.bytes(),
            &product_activation.id().bytes(),
            &activation_plan.receipt_id().bytes(),
            &collateral_postwrite.receipt_id().bytes(),
            &slot10.principal_lamports().to_le_bytes(),
            &slot10.donation_lamports().to_le_bytes(),
            &[RESOLVED_DISPOSITION_BYTE_V2],
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedFailureMarketResolutionActivationV5 {
        id,
        failure_resolution,
        product_activation,
        collateral_postwrite,
        market_root: *market_root_account.key,
        market_root_authentication_before: live_root.authentication_id(),
        market_root_authentication_after: root_after.authentication_id(),
        series_link: *series_link_account.key,
        series_link_authentication: live_link.authentication_id(),
        slot10_preallocation_id: slot10.id(),
        finalization_evidence_id,
    })
}

#[allow(clippy::too_many_arguments)]
fn require_current_product_failure_join(
    market_root_account: &AccountInfo<'_>,
    root: &AuthenticatedMarketLifecycleRootV1<'_>,
    root_state: &clutch_product_series::MarketLifecycleRootV1,
    root_binding_id: ContentId,
    link: &AuthenticatedSeriesMarketLinkV1<'_>,
    link_state: clutch_product_series::SeriesMarketLinkV1,
    registry: AuthenticatedRegistryCapabilityV3,
    bundle: AuthenticatedCompiledProductSeriesBundleV5,
    failure_resolution: FailureMarketIntervalCellResolutionReceiptV2,
    certificate: clutch_product_series::QuantizedIntervalConsensusCertificateV1,
    certificate_id: clutch_product_series::QuantizedIntervalConsensusCertificateV1Id,
) -> Outcome<()> {
    let root_binding = root_state.binding();
    let link_binding = link_state.binding();
    let projection = registry.projection();
    let bundle_value = bundle.bundle();
    require(
        root.is_writable()
            && !link.is_writable()
            && root.account() == *market_root_account.key
            && root_binding
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == root_binding_id
            && link_binding.market_root_account_id.bytes() == market_root_account.key.to_bytes()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && registry.series_plan_id() == link_binding.series_plan_id
            && registry.registry_release_id() == root_binding.registry_release_id
            && registry.capability_profile_id() == root_binding.capability_profile_id
            && projection.registry_release_id == root_binding.registry_release_id
            && projection.capability_profile_id == root_binding.capability_profile_id
            && link_binding.capability_profile_id == root_binding.capability_profile_id
            && bundle_value.registry_release_id == root_binding.registry_release_id
            && bundle_value.capability_profile_id.content_id()
                == root_binding.capability_profile_id
            && bundle_value.series_plan_id == link_binding.series_plan_id
            && bundle_value.product_template_id.content_id() == root_binding.product_template_id
            && bundle_value.native_claim_basis_id.content_id()
                == root_binding.native_claim_basis_id
            && bundle_value.market_genesis_profile_id.content_id()
                == root_binding.market_genesis_profile_id
            && bundle_value.price_measure_policy_id.content_id()
                == root_binding.price_measure_policy_id
            && bundle_value.evidence_only_recovery_policy_id.content_id()
                == root_binding.recovery_policy_id
            && bundle_value.source_release_manifest_id == root_binding.source_release_id
            && bundle_value.source_plane_contract_id == root_binding.source_plane_contract_id
            && bundle_value.source_spec_id == root_binding.source_spec_id
            && link_binding.compiler_output_id.bytes() == bundle.bundle_id().bytes()
            && link_binding.source_release_id == root_binding.source_release_id
            && link_binding.source_plane_contract_id == root_binding.source_plane_contract_id
            && link_binding.source_spec_id == root_binding.source_spec_id
            && link_binding.source_route_id == root_binding.source_route_id
            && link_binding.clock_policy_id == root_binding.clock_policy_id
            && certificate.source_occurrence_id() == link_binding.source_occurrence_id
            && certificate.product_template_id() == bundle_value.product_template_id
            && certificate.market_genesis_profile_id() == bundle_value.market_genesis_profile_id
            && certificate.native_claim_basis_id() == bundle_value.native_claim_basis_id
            && certificate.price_measure_policy_id() == bundle_value.price_measure_policy_id
            && certificate.capability_profile_id() == registry.capability_profile_id()
            && certificate_id.bytes() == failure_resolution.facts().product_certificate_id.bytes(),
        ClutchError::MismatchedState,
    )
}

fn require_exact_collateral_prestate(
    program_id: &Pubkey,
    hoard_account: &AccountInfo<'_>,
    claim_ledger_account: &AccountInfo<'_>,
    liabilities: GeneralMarketLiabilityAuthorityV2,
    root_binding: clutch_product_series::MarketLifecycleBindingV1,
    registry: clutch_product_series::RegistryCapabilityProjectionV2,
) -> Outcome<()> {
    require_program_owned_writable(hoard_account, program_id, HOARD_V2_BYTES)?;
    require_program_owned_writable(claim_ledger_account, program_id, CLAIM_LEDGER_V3_BYTES)?;
    let hoard = HoardV2::decode(&hoard_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let claim_ledger = ClaimLedgerV3::decode(&claim_ledger_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market = root_binding.market_instance_id.bytes();
    expect_pda(
        hoard_account.key,
        seeds::hoard_v2_pda(program_id, &market),
        Some(hoard.stored_bump),
    )?;
    expect_pda(
        claim_ledger_account.key,
        seeds::claim_ledger_v3_pda(program_id, &market),
        Some(claim_ledger.stored_bump),
    )?;
    let hoard_id = hoard
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let claim_id = claim_ledger
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let relation_market = liabilities.market_binding.base();
    let bound_realm = liabilities.bound.realm_bound().realm();
    let bound_release_id = liabilities
        .bound
        .release()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        hoard == liabilities.hoard
            && claim_ledger == liabilities.claim_ledger
            && hoard_id == liabilities.hoard_semantic_id
            && claim_id == liabilities.claim_ledger_semantic_id
            && liabilities
                .market_instance
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == root_binding.market_instance_id
            && relation_market.market_instance_v2_id.bytes() == market
            && relation_market.market_genesis_profile_v2_id.bytes()
                == root_binding.market_genesis_profile_id.bytes()
            && relation_market.native_claim_basis_id.bytes()
                == root_binding.native_claim_basis_id.bytes()
            && relation_market.price_measure_policy_v1_id.bytes()
                == root_binding.price_measure_policy_id.bytes()
            && relation_market.outcome_count == root_binding.outcome_count
            && bound_realm.realm.bytes() == root_binding.realm_id.bytes()
            && bound_realm.profile.bytes() == root_binding.collateral_profile_id.bytes()
            && liabilities.bound.policy_id().bytes() == root_binding.collateral_policy_id.bytes()
            && bound_release_id.bytes() == root_binding.collateral_release_id.bytes()
            && registry.realm_collateral.realm_id == root_binding.realm_id
            && registry.realm_collateral.profile_id == root_binding.collateral_profile_id
            && liabilities.bound.market().collateral_cap_atoms
                == liabilities.market_instance.collateral_cap
            && liabilities.market_instance.collateral_cap
                <= registry.realm_collateral.market_collateral_cap_ceiling
            && liabilities.bound.market().market.bytes() == market,
        ClutchError::MismatchedState,
    )
}

fn require_exact_slot10_preallocation(
    program_id: &Pubkey,
    resolution_account: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    slot10: AuthenticatedMarketFoundationPreallocationV2,
) -> Outcome<()> {
    let binding = root.state().binding();
    let capital = root.state().capital();
    let expected_balance = slot10
        .principal_lamports()
        .checked_add(slot10.donation_lamports())
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let rent = read_rent(rent_sysvar)?;
    let (expected_resolution, _) =
        seeds::resolution_v5_pda(program_id, &binding.market_instance_id.bytes());
    require(
        root.state().phase() == MarketLifecyclePhaseV1::Active
            && slot10.root_account() == root.account()
            && slot10.root_authentication_id() == root.authentication_id()
            && slot10.market_instance_id() == binding.market_instance_id
            && slot10.generation() == binding.generation
            && slot10.slot() == MarketFoundationSlotV2::ResolutionV5
            && slot10.account() == *resolution_account.key
            && slot10.foundation_schedule_id().bytes() == binding.foundation_schedule_id.bytes()
            && slot10.foundation_account_graph_id().bytes()
                == binding.foundation_account_graph_id.bytes()
            && slot10.foundation_transcript_id() == root.state().foundation().transcript_id
            && slot10.rent_refund_owner().to_bytes() == capital.rent_refund_owner.bytes()
            && slot10.neutral_lamport_sink().to_bytes() == capital.neutral_lamport_sink.bytes()
            && slot10.principal_lamports() == rent.minimum_balance(RESOLUTION_V5_BYTES)?
            && slot10.observed_balance_lamports() == expected_balance
            && binding.resolution_account_id.bytes() == resolution_account.key.to_bytes()
            && expected_resolution == *resolution_account.key
            && resolution_account.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && resolution_account.is_writable
            && !resolution_account.is_signer
            && !resolution_account.executable
            && resolution_account.data_len() == 0
            && resolution_account.lamports() == expected_balance,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn derive_finalization_evidence_id_v5(
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    link: AuthenticatedSeriesMarketLinkV1<'_>,
    registry: AuthenticatedRegistryCapabilityV3,
    bundle: AuthenticatedCompiledProductSeriesBundleV5,
    slot10: AuthenticatedMarketFoundationPreallocationV2,
    liabilities: GeneralMarketLiabilityAuthorityV2,
    expected_hoard_after_id: CollateralId,
    expected_claim_ledger_after_id: CollateralId,
    failure_resolution: FailureMarketIntervalCellResolutionReceiptV2,
    root_binding_id: ContentId,
    certificate_id: ContentId,
    resolution_account: &Pubkey,
    outcome_count: u8,
    denominator: u64,
    weights: &[u64; clutch_retirement::MAX_OUTCOMES],
) -> Outcome<ContentId> {
    let failure = failure_resolution.facts();
    let link_state_id = link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let weights_bytes = encode_weights(weights);
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAILURE_MARKET_RESOLUTION_FINALIZATION_EVIDENCE_DOMAIN_V5,
            &root_binding_id.bytes(),
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            link.account().as_ref(),
            &link.authentication_id().bytes(),
            &link_state_id.bytes(),
            registry.series_registry_account().as_ref(),
            registry.program_account().as_ref(),
            registry.programdata_account().as_ref(),
            registry.release_artifact_account().as_ref(),
            registry.profile_artifact_account().as_ref(),
            &registry.registry_release_id().bytes(),
            &registry.capability_profile_id().bytes(),
            bundle.artifact_account().as_ref(),
            &bundle.bundle_id().bytes(),
            &slot10.id().bytes(),
            &slot10.foundation_schedule_id().bytes(),
            &slot10.foundation_account_graph_id().bytes(),
            &slot10.foundation_transcript_id().bytes(),
            &slot10.principal_lamports().to_le_bytes(),
            &slot10.donation_lamports().to_le_bytes(),
            slot10.rent_refund_owner().as_ref(),
            slot10.neutral_lamport_sink().as_ref(),
            &liabilities.receipt_id.bytes(),
            &liabilities.hoard_semantic_id.bytes(),
            &expected_hoard_after_id.bytes(),
            &liabilities.claim_ledger_semantic_id.bytes(),
            &expected_claim_ledger_after_id.bytes(),
            &failure_resolution.id().bytes(),
            &failure_resolution.failure_policy_binding_id().bytes(),
            &failure.cell_before.bytes(),
            &failure.cell_after.bytes(),
            &failure.session_binding_id.bytes(),
            &failure.source_handoff_id.bytes(),
            &failure.terminal_work_id.bytes(),
            &certificate_id.bytes(),
            &failure.last_runtime_work_receipt_id.bytes(),
            &failure.completed_work_calls.to_le_bytes(),
            &failure.exact_reward_lamports.to_le_bytes(),
            resolution_account.as_ref(),
            &[outcome_count],
            &denominator.to_le_bytes(),
            &weights_bytes,
            &[RESOLVED_DISPOSITION_BYTE_V2],
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
fn write_collateral_activation_postimages_v5<'a>(
    program_id: &Pubkey,
    system_program: &AccountInfo<'a>,
    resolution_account: &AccountInfo<'a>,
    hoard_account: &AccountInfo<'a>,
    claim_ledger_account: &AccountInfo<'a>,
    market_instance_id: [u8; 32],
    resolution_bump: u8,
    resolution: ResolutionV5,
    hoard_after: HoardV2,
    claim_ledger_after: ClaimLedgerV3,
    expected_resolution_balance: u64,
) -> Outcome<()> {
    let hoard_lamports_before = hoard_account.lamports();
    let claim_ledger_lamports_before = claim_ledger_account.lamports();
    let bump_seed = [resolution_bump];
    let signer_seeds: [&[u8]; 3] = [seeds::SEED_RESOLUTION_V5, &market_instance_id, &bump_seed];
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(RESOLUTION_V5_BYTES),
        vec![AccountMeta::new(*resolution_account.key, true)],
    );
    invoke_signed(
        &allocate,
        &[resolution_account.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*resolution_account.key, true)],
    );
    invoke_signed(
        &assign,
        &[resolution_account.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        resolution_account.owner == program_id
            && resolution_account.data_len() == RESOLUTION_V5_BYTES
            && resolution_account.lamports() == expected_resolution_balance,
        ClutchError::AccountCreationFailed,
    )?;
    {
        let mut data = resolution_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(
            data.iter().all(|byte| *byte == 0),
            ClutchError::AlreadyInitialized,
        )?;
        resolution
            .encode(&mut data)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    }
    {
        let mut data = hoard_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        hoard_after
            .encode(&mut data)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    }
    {
        let mut data = claim_ledger_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        claim_ledger_after
            .encode(&mut data)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    }
    require(
        resolution_account.lamports() == expected_resolution_balance
            && hoard_account.lamports() == hoard_lamports_before
            && claim_ledger_account.lamports() == claim_ledger_lamports_before,
        ClutchError::MismatchedState,
    )
}

fn require_program_owned_writable(
    account: &AccountInfo<'_>,
    program_id: &Pubkey,
    expected_len: usize,
) -> Outcome<()> {
    require(
        account.owner == program_id
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && account.data_len() == expected_len,
        ClutchError::MismatchedState,
    )
}

fn encode_weights(weights: &[u64; clutch_retirement::MAX_OUTCOMES]) -> [u8; 128] {
    let mut output = [0u8; 128];
    let mut index = 0usize;
    while index < clutch_retirement::MAX_OUTCOMES {
        let start = index * 8;
        output[start..start + 8].copy_from_slice(&weights[index].to_le_bytes());
        index += 1;
    }
    output
}

fn require_live_content_id(id: ContentId) -> Outcome<()> {
    require(id != ContentId::ZERO, ClutchError::MismatchedState)
}
