// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact price, complete-book, and covered-Dealer join for a selected root.

use clutch_batch::dealer_leg_v2::{
    verify_economic_candidate_with_dealer_v2, DealerLegCandidateV2, DealerQuotePreconditionV2,
    VerifiedDealerLegV2,
};
use clutch_batch::portfolio_book_v2::AuthenticatedCompletePortfolioBookV2;
use clutch_batch::relation_v2::{
    price_semantics_digest_v2, EconomicCandidateV2, EconomicDomainV2, PricePreconditionV2,
};
use clutch_general_v2_contract::{
    candidate_bundle_digest_v1, candidate_feed_tail_v2, economic_domain_digest_v2,
    quantized_witness_body_digest_v3, settlement_witness_digest_v1,
    EconomicDomainV2AccountV1, Id32, MarketBindingV2, SettlementCandidateKindV1,
    SettlementRootChildStateV1, SettlementRootPhaseV1, SettlementRootV1AccountV1,
};
use clutch_price_measure::{
    verify_quantized_price_measure_v3_degree_zero, verify_quantized_price_measure_v3_smooth,
    AdapterBindingsV3, PriceVectorV3, QuantizedAtomWitnessV3, VerifiedPriceMeasureV3,
    VerifiedQuantizedAtomMixtureV1,
};
use clutch_product_series::{
    MarketGenesisProfileV2, MarketInstancePreimageV2, NativeClaimBasisV1, PriceMeasurePolicyV1,
    ProductTemplateV4, QuantizedEdgePolicyV1,
};
use clutch_solana_layout::PriceGridAccount;

use crate::{
    decode_sealed_candidate_feed_v1, verify_exact_smooth_atom_mixture_v1, CanonicalSha256,
    GeneralV2RuntimeError, QuantizedBasisProjectionV1,
};

/// Private capability proving that the selected CoveredDealer identity was
/// recomputed from the exact retained Feed, complete frozen book, quote, and
/// quantized price certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedSmoothCoveredDealerCandidateV1 {
    economic_domain_digest: Id32,
    price_measure: VerifiedPriceMeasureV3,
    quantized_atom_mixture: Option<VerifiedQuantizedAtomMixtureV1>,
    dealer_leg: VerifiedDealerLegV2,
}

impl VerifiedSmoothCoveredDealerCandidateV1 {
    /// Canonical EconomicDomainV2 identity checked by the verifier.
    pub const fn economic_domain_digest(&self) -> Id32 {
        self.economic_domain_digest
    }

    /// Exact checked quantized price-measure capability.
    pub const fn price_measure(&self) -> &VerifiedPriceMeasureV3 {
        &self.price_measure
    }

    /// Stronger exact atom-mixture result for degree two or three.
    pub const fn quantized_atom_mixture(&self) -> Option<VerifiedQuantizedAtomMixtureV1> {
        self.quantized_atom_mixture
    }

    /// Exact private covered-Dealer relation capability.
    pub const fn dealer_leg(&self) -> &VerifiedDealerLegV2 {
        &self.dealer_leg
    }
}

/// Reverify one selected CoveredDealer candidate without accepting a caller
/// book, coefficient row, price, verdict, or allocation DTO.
///
/// SettlementRoot remains the selected-candidate/count owner. The complete
/// page capability supplies the owner-blind book, the retained Feed supplies
/// prices/fills/witness bytes, Product supplies immutable policy preimages,
/// and the signed quote supplies only the Dealer proposal. Success proves the
/// final Dealer digest equals the root's final SettlementCandidateId; it does
/// not repeat or replace the root's earlier best-submitted-candidate ordering.
#[allow(clippy::too_many_arguments)]
pub fn verify_smooth_covered_dealer_candidate_v1(
    candidate_feed_identity: Id32,
    sealed_candidate_feed: &[u8],
    settlement_root_account_identity: Id32,
    settlement_root: &SettlementRootV1AccountV1,
    economic_domain_account: &EconomicDomainV2AccountV1,
    market_binding: &MarketBindingV2,
    price_grid: &PriceGridAccount,
    product_template: &ProductTemplateV4,
    native_basis: &NativeClaimBasisV1,
    price_measure_policy: &PriceMeasurePolicyV1,
    genesis: &MarketGenesisProfileV2,
    market_instance: &MarketInstancePreimageV2,
    authenticated_edge_policy: QuantizedEdgePolicyV1,
    book: &AuthenticatedCompletePortfolioBookV2,
    dealer: &DealerLegCandidateV2,
    quote: &DealerQuotePreconditionV2,
) -> Result<VerifiedSmoothCoveredDealerCandidateV1, GeneralV2RuntimeError> {
    if candidate_feed_identity.is_zero() || settlement_root_account_identity.is_zero() {
        return Err(GeneralV2RuntimeError::BindingMismatch);
    }
    let (header, feed) = decode_sealed_candidate_feed_v1(sealed_candidate_feed)?;
    settlement_root.validate()?;
    economic_domain_account.validate()?;
    market_binding.validate()?;
    let relation_binding = market_binding.relation_projection();
    relation_binding.validate()?;
    price_grid.validate()?;
    if header.candidate_kind != SettlementCandidateKindV1::CoveredDealer {
        return Err(GeneralV2RuntimeError::UnsupportedCandidateKind);
    }

    let tail = candidate_feed_tail_v2(sealed_candidate_feed, header)?;
    let observed_settlement_witness = settlement_witness_digest_v1(
        &CanonicalSha256,
        header.base_relation_candidate_id,
        header.slice_count,
        tail.slices_le(),
    )?;
    let observed_candidate_bundle =
        candidate_bundle_digest_v1(&CanonicalSha256, sealed_candidate_feed, true)?;
    let root_counts = settlement_root.counts();
    if observed_settlement_witness != header.settlement_witness_digest
        || observed_settlement_witness != settlement_root.settlement_witness_digest()
        || observed_candidate_bundle != settlement_root.candidate_bundle_digest()
        || settlement_root.phase() != SettlementRootPhaseV1::Materializing
        || settlement_root.retained_feed_state() != SettlementRootChildStateV1::Live
        || root_counts.expected_dealer_children != 1
        || root_counts.admitted_dealer_children != 0
        || root_counts.live_dealer_children != 0
        || settlement_root.retained_feed() != candidate_feed_identity
        || header.epoch != settlement_root.epoch()
        || header.market != settlement_root.market()
        || header.node != settlement_root.source_admission_node()
        || header.order_set != settlement_root.order_set()
        || header.settlement_candidate_id != settlement_root.settlement_candidate_id()
        || header.settlement_witness_digest != settlement_root.settlement_witness_digest()
        || header.epoch_generation != settlement_root.epoch_generation()
        || header.outcome_count != settlement_root.outcome_count()
        || header.order_count != settlement_root.order_count()
        || book.settlement_root_account_id() != settlement_root_account_identity.bytes()
        || book.retained_feed_account_id() != candidate_feed_identity.bytes()
        || book.order_set_digest() != header.order_set.bytes()
        || book.settlement_candidate_id() != header.settlement_candidate_id.bytes()
        || book.settlement_witness_id() != header.settlement_witness_digest.bytes()
        || book.order_count() != header.order_count
    {
        return Err(GeneralV2RuntimeError::BindingMismatch);
    }

    market_instance.validate_bindings(
        product_template,
        native_basis,
        price_measure_policy,
        genesis,
    )?;
    let projected_basis = if native_basis.basis_degree == 0 {
        QuantizedBasisProjectionV1::DegreeZero(
            price_measure_policy.project_degree_zero_table(native_basis, genesis)?,
        )
    } else {
        QuantizedBasisProjectionV1::Smooth(price_measure_policy.project_smooth_basis(
            native_basis,
            genesis,
            authenticated_edge_policy,
        )?)
    };

    let basis_digest = native_basis.id()?.bytes();
    let price_policy_id = price_measure_policy.id()?.bytes();
    let genesis_id = genesis.id()?.bytes();
    let market_instance_id = market_instance.id()?.bytes();
    let transcript = economic_domain_account.transcript;
    let domain_digest = economic_domain_digest_v2(&CanonicalSha256, transcript)?;
    if economic_domain_account.epoch != header.epoch
        || header.relation_policy_id != relation_binding.relation_policy_id
        || header.price_measure_policy_v1_id != relation_binding.price_measure_policy_v1_id
        || header.native_claim_basis_id != relation_binding.native_claim_basis_id
        || header.economic_domain_digest != domain_digest
        || relation_binding.market_genesis_profile_v2_id.bytes() != genesis_id
        || relation_binding.market_instance_v2_id.bytes() != market_instance_id
        || settlement_root.market_instance_v2_id().bytes() != market_instance_id
        || settlement_root.batch_policy_id() != market_binding.batch_policy_id()
        || settlement_root.score_policy_id() != relation_binding.score_policy_id
        || relation_binding.price_measure_policy_v1_id.bytes() != price_policy_id
        || relation_binding.native_claim_basis_id.bytes() != basis_digest
        || relation_binding.price_scale != price_grid.price_scale
        || relation_binding.price_scale != header.price_scale
        || relation_binding.relation_version != transcript.relation_version
        || relation_binding.outcome_count != transcript.outcome_count
        || relation_binding.outcome_count != header.outcome_count
        || relation_binding.outcome_count != native_basis.outcome_count
        || relation_binding.basis_degree != header.basis_degree
        || relation_binding.basis_degree != native_basis.basis_degree
        || relation_binding.candidate_kind_mask & 0b10 == 0
        || transcript.market_instance_v2_id.bytes() != market_instance_id
        || transcript.relation_policy_id != header.relation_policy_id
        || transcript.price_measure_policy_v1_id != header.price_measure_policy_v1_id
        || transcript.native_claim_basis_id != header.native_claim_basis_id
        || transcript.price_scale != header.price_scale
        || transcript.coordinate_domain_min != genesis.coordinate_domain_min
        || transcript.coordinate_domain_max != genesis.coordinate_domain_max
        || price_grid.grid.bytes() != genesis.price_grid_id.bytes()
        || price_grid.realm.bytes() != genesis.realm_id.bytes()
    {
        return Err(GeneralV2RuntimeError::BindingMismatch);
    }

    let mut outcome = 0usize;
    while outcome < usize::from(header.outcome_count) {
        price_grid.tick_of(feed.prices[outcome])?;
        outcome += 1;
    }
    let relation_domain = EconomicDomainV2 {
        relation_version: transcript.relation_version,
        market_semantics_digest: transcript.market_instance_v2_id.bytes(),
        epoch_semantics_digest: transcript.epoch_semantics_digest.bytes(),
        relation_policy_digest: transcript.relation_policy_id.bytes(),
        price_policy_digest: transcript.price_measure_policy_v1_id.bytes(),
        epoch_index: transcript.epoch_index,
        outcome_count: transcript.outcome_count,
        price_scale: transcript.price_scale,
    };
    let candidate_price_digest = price_semantics_digest_v2(&relation_domain, &feed.prices)?;
    if header.candidate_price_digest.bytes() != candidate_price_digest {
        return Err(GeneralV2RuntimeError::BindingMismatch);
    }
    let observed_body_digest = quantized_witness_body_digest_v3(
        &CanonicalSha256,
        candidate_feed_identity,
        sealed_candidate_feed,
        true,
    )?;
    if observed_body_digest != header.price_body_digest {
        return Err(GeneralV2RuntimeError::WitnessBodyDigestMismatch);
    }

    let bindings = AdapterBindingsV3 {
        candidate_feed: candidate_feed_identity.bytes(),
        relation_domain_digest: domain_digest.bytes(),
        basis_digest,
        candidate_price_digest,
        observed_body_digest: observed_body_digest.bytes(),
    };
    let prices = PriceVectorV3 {
        basis_degree: header.basis_degree,
        native_outcome_count: header.outcome_count,
        price_scale: header.price_scale,
        prices: feed.prices,
    };
    let witness = QuantizedAtomWitnessV3 {
        schema_version: header.price_witness_schema,
        quantized_semantics_version: header.quantized_semantics_version,
        candidate_feed: bindings.candidate_feed,
        relation_domain_digest: bindings.relation_domain_digest,
        basis_digest: bindings.basis_digest,
        candidate_price_digest: bindings.candidate_price_digest,
        body_digest: bindings.observed_body_digest,
        basis_degree: header.basis_degree,
        native_outcome_count: header.outcome_count,
        atom_count: header.atom_count,
        common_denominator: header.common_denominator,
        atom_coordinates: feed.atom_coordinates,
        atom_masses: feed.atom_masses,
    };
    price_measure_policy.validate_witness_contract(
        native_basis,
        &prices,
        &witness,
        price_grid.price_scale,
    )?;
    let price_measure = match &projected_basis {
        QuantizedBasisProjectionV1::DegreeZero(table) => {
            verify_quantized_price_measure_v3_degree_zero(&bindings, table, &prices, &witness)?
        }
        QuantizedBasisProjectionV1::Smooth(basis) => {
            verify_quantized_price_measure_v3_smooth(&bindings, basis, &prices, &witness)?
        }
    };
    let quantized_atom_mixture = match projected_basis {
        QuantizedBasisProjectionV1::Smooth(basis) if (2..=3).contains(&basis.degree) => {
            if header.price_scale != basis.denominator {
                return Err(GeneralV2RuntimeError::BindingMismatch);
            }
            Some(verify_exact_smooth_atom_mixture_v1(
                relation_binding.market,
                genesis_id,
                relation_binding.native_claim_basis_id,
                candidate_price_digest,
                genesis.coordinate_domain_min,
                genesis.coordinate_domain_max,
                basis,
                feed.prices,
                header.atom_count,
                header.common_denominator,
                feed.atom_coordinates,
                feed.atom_masses,
            )?)
        }
        _ => None,
    };

    let price_precondition = PricePreconditionV2 {
        policy_digest: transcript.price_measure_policy_v1_id.bytes(),
        semantic_price_digest: candidate_price_digest,
        prices: feed.prices,
    };
    let candidate = EconomicCandidateV2 {
        fills: feed.fills,
        honored_aon_mask: header.honored_aon_mask,
        virtual_split: header.virtual_split,
        virtual_merge: header.virtual_merge,
    };
    let dealer_leg = verify_economic_candidate_with_dealer_v2(
        &relation_domain,
        book.economic_book(),
        &price_precondition,
        &candidate,
        dealer,
        quote,
    )?;
    if quote.upstream_economic_candidate_digest != header.base_relation_candidate_id.bytes()
        || dealer_leg.dealer_economic_candidate_digest()
            != &header.settlement_candidate_id.bytes()
        || dealer_leg.score().digest != header.settlement_candidate_id.bytes()
    {
        return Err(GeneralV2RuntimeError::CandidateIdentityMismatch);
    }
    Ok(VerifiedSmoothCoveredDealerCandidateV1 {
        economic_domain_digest: domain_digest,
        price_measure,
        quantized_atom_mixture,
        dealer_leg,
    })
}
