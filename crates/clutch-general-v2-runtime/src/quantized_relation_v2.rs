// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact finite price admission for the production RelationV2 profile.
//!
//! This module is the authority seam between the current finite quantized
//! atom-mixture checker and RelationV2. A caller-provided simplex cannot enter
//! the successor ranking path until the exact production evaluator reconstructs
//! it from a positive integer-coordinate mixture. Continuous moment or Hankel
//! witnesses are not inputs to this profile.

use clutch_batch::relation_v2::{
    price_semantics_digest_v2, verify_economic_candidate_v2, EconomicBookV2,
    EconomicCandidateV2, EconomicDomainV2, PricePreconditionV2, VerifiedEconomicsV2,
};
use clutch_batch::relation_v2_ranking::BestValidSubmittedCandidateV2;
use clutch_batch::score_v2::SelectionUpdateV2;
use clutch_general_v2_contract::{
    economic_domain_digest_v2, quantized_witness_body_digest_v3, CandidateFeedHeaderV2, Id32,
    MarketBindingV1, SettlementCandidateKindV1,
};
use clutch_price_measure::{
    VerifiedQuantizedAtomMixtureV1, QUANTIZED_ATOM_CARATHEODORY_PROFILE_V1,
    QUANTIZED_ATOM_MIXTURE_CERTIFICATE_VERSION_V1,
    QUANTIZED_ATOM_MIXTURE_SEMANTICS_VERSION_V1,
};
use clutch_product_series::{
    MarketGenesisProfileV2, MarketInstancePreimageV2, NativeClaimBasisV1, PriceMeasurePolicyV1,
    ProductTemplateV4, QuantizedBasisSpecV1, QuantizedEdgePolicyV1,
};
use clutch_solana_layout::PriceGridAccount;

use crate::{
    decode_sealed_candidate_feed_v1, hash_parts, score_v2_q_policy_id_v1,
    verify_exact_smooth_atom_mixture_v1, CanonicalSha256, GeneralV2RuntimeError,
    RELATION_V2_POLICY_BODY_V1,
};

/// Domain of the exact finite-certificate RelationV2 policy successor.
pub const QUANTIZED_RELATION_V2_POLICY_DIGEST_DOMAIN_V2: &[u8] =
    b"dragons-clutch/relation-v2-quantized-policy/v2\0";

/// Canonical exact finite-certificate RelationV2 extension body.
///
/// Its policy ID also hashes [`RELATION_V2_POLICY_BODY_V1`] in full. These
/// extension bytes commit the exact atom-mixture schema, evaluator semantics,
/// affine support profile, degree-two/three admission, payout-denominator
/// price scale, and proof-independent candidate identity.
pub const QUANTIZED_RELATION_V2_POLICY_BODY_V2: [u8; 16] = [
    b'D',
    b'C',
    b'R',
    b'V',
    b'2',
    b'A',
    b'M',
    0,
    2,
    QUANTIZED_ATOM_MIXTURE_CERTIFICATE_VERSION_V1,
    QUANTIZED_ATOM_MIXTURE_SEMANTICS_VERSION_V1,
    QUANTIZED_ATOM_CARATHEODORY_PROFILE_V1,
    2,
    3,
    1,
    1,
];

/// Derive the successor policy identity bound into every admitted candidate.
pub fn quantized_relation_v2_policy_id_v2() -> Result<Id32, GeneralV2RuntimeError> {
    Id32::new(hash_parts(&[
        QUANTIZED_RELATION_V2_POLICY_DIGEST_DOMAIN_V2,
        &RELATION_V2_POLICY_BODY_V1,
        &QUANTIZED_RELATION_V2_POLICY_BODY_V2,
    ]))
    .map_err(GeneralV2RuntimeError::Contract)
}

/// Private proof that one exact feed price is in the authenticated quantized
/// payout image selected by its Market, Terms, Basis, and RelationV2 policy.
///
/// This is an in-memory arithmetic capability. It does not authenticate an
/// account by itself, authorize settlement, or claim that the price is fair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedRelationPriceAdmissionV2 {
    candidate_feed: Id32,
    economic_domain_digest: Id32,
    price_body_digest: Id32,
    authority: QuantizedRelationPriceAuthorityV2,
}

/// Private proof that exact price admission also joined the complete immutable
/// Product and canonical PriceGrid tuple.
///
/// This is the capability required by resumed nonempty Work. A basic exact
/// atom certificate is insufficient because it does not by itself authenticate
/// the Product bodies or tick grid selected by the Market.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedRelationProductPriceAdmissionV2 {
    price_admission: QuantizedRelationPriceAdmissionV2,
    market_binding: Id32,
    market_genesis_profile_v2_id: Id32,
    market_instance_v2_id: Id32,
    product_template_id: Id32,
    price_grid_id: Id32,
    price_grid_realm: Id32,
}

/// Private exact-price capability consumed by every successor RelationV2 call.
///
/// The builder and sealed-feed adapter can mint this only from a certificate
/// produced by the finite production atom-mixture verifier. Keeping this type
/// crate-private prevents a raw caller price from reaching the successor
/// ranking implementation through a parallel public constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct QuantizedRelationPriceAuthorityV2 {
    domain: EconomicDomainV2,
    price: PricePreconditionV2,
    certificate: VerifiedQuantizedAtomMixtureV1,
}

impl QuantizedRelationPriceAdmissionV2 {
    /// Authenticated sealed candidate-feed identity.
    pub const fn candidate_feed(&self) -> Id32 {
        self.candidate_feed
    }

    /// Canonical persisted EconomicDomain identity checked by this admission.
    pub const fn economic_domain_digest(&self) -> Id32 {
        self.economic_domain_digest
    }

    /// Canonical exact finite witness-body identity checked by this admission.
    pub const fn price_body_digest(&self) -> Id32 {
        self.price_body_digest
    }

    /// Exact RelationV2 domain carrying the quantized successor policy digest.
    pub const fn domain(&self) -> &EconomicDomainV2 {
        &self.authority.domain
    }

    /// Exact simplex price reconstructed by the finite certificate.
    pub const fn price(&self) -> &PricePreconditionV2 {
        &self.authority.price
    }

    /// Exact production atom-mixture fact retained behind the admission.
    pub const fn certificate(&self) -> VerifiedQuantizedAtomMixtureV1 {
        self.authority.certificate
    }
}

impl QuantizedRelationProductPriceAdmissionV2 {
    /// Exact feed-price admission nested inside the closed Product tuple.
    pub const fn price_admission(&self) -> QuantizedRelationPriceAdmissionV2 {
        self.price_admission
    }

    /// Canonical MarketBinding account identity selected by the Epoch.
    pub const fn market_binding(&self) -> Id32 {
        self.market_binding
    }

    /// Exact Genesis V2 body identity checked by this capability.
    pub const fn market_genesis_profile_v2_id(&self) -> Id32 {
        self.market_genesis_profile_v2_id
    }

    /// Exact MarketInstance V2 body identity checked by this capability.
    pub const fn market_instance_v2_id(&self) -> Id32 {
        self.market_instance_v2_id
    }

    /// Exact ProductTemplate V4 body identity checked by this capability.
    pub const fn product_template_id(&self) -> Id32 {
        self.product_template_id
    }

    /// Canonical PriceGrid body identity checked by this capability.
    pub const fn price_grid_id(&self) -> Id32 {
        self.price_grid_id
    }

    /// Canonical PriceGrid Realm identity checked by this capability.
    pub const fn price_grid_realm(&self) -> Id32 {
        self.price_grid_realm
    }
}

impl QuantizedRelationPriceAuthorityV2 {
    pub(crate) const fn domain(&self) -> &EconomicDomainV2 {
        &self.domain
    }

    pub(crate) const fn price(&self) -> &PricePreconditionV2 {
        &self.price
    }

    pub(crate) const fn certificate(&self) -> VerifiedQuantizedAtomMixtureV1 {
        self.certificate
    }
}

/// A RelationV2 result that can only be constructed from exact quantized price
/// admission and a fully valid submitted candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedQuantizedRelationCandidateV2 {
    admission: QuantizedRelationPriceAdmissionV2,
    economics: VerifiedEconomicsV2,
}

impl VerifiedQuantizedRelationCandidateV2 {
    /// Exact finite price admission used by RelationV2.
    pub const fn price_admission(&self) -> &QuantizedRelationPriceAdmissionV2 {
        &self.admission
    }

    /// Owner-blind RelationV2 economics and domain-bound ScoreV2-Q certificate.
    pub const fn economics(&self) -> &VerifiedEconomicsV2 {
        &self.economics
    }

    /// Proof-independent candidate identity containing the successor policy.
    pub const fn candidate_digest(&self) -> [u8; 32] {
        self.economics.economic_candidate_digest
    }
}

/// Fixed-price fold retaining the best valid submitted exact-quantized
/// RelationV2 candidate encountered, never an optimality claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BestValidQuantizedSubmittedCandidateV2 {
    admission: QuantizedRelationPriceAdmissionV2,
    ranking: BestValidSubmittedCandidateV2,
}

impl BestValidQuantizedSubmittedCandidateV2 {
    /// Verify the first candidate under one exact finite price admission.
    pub fn begin(
        admission: QuantizedRelationPriceAdmissionV2,
        book: EconomicBookV2,
        first_candidate: EconomicCandidateV2,
    ) -> Result<Self, GeneralV2RuntimeError> {
        let ranking = BestValidSubmittedCandidateV2::begin(
            *admission.domain(),
            book,
            *admission.price(),
            first_candidate,
        )?;
        Ok(Self { admission, ranking })
    }

    /// Reverify and rank one more candidate under the same immutable inputs.
    pub fn submit(
        &mut self,
        candidate: EconomicCandidateV2,
    ) -> Result<SelectionUpdateV2, GeneralV2RuntimeError> {
        self.ranking
            .submit(candidate)
            .map_err(GeneralV2RuntimeError::from)
    }

    /// Exact finite price admission shared by every ranked candidate.
    pub const fn price_admission(&self) -> &QuantizedRelationPriceAdmissionV2 {
        &self.admission
    }

    /// Retained best valid submitted candidate witness.
    pub const fn best_candidate(&self) -> &EconomicCandidateV2 {
        self.ranking.best_candidate()
    }

    /// Retained checked RelationV2 economics and ScoreV2-Q certificate.
    pub const fn best_economics(&self) -> &VerifiedEconomicsV2 {
        self.ranking.best_economics()
    }

    /// Number of valid submitted candidates admitted to the fold.
    pub const fn valid_submission_count(&self) -> u64 {
        self.ranking.valid_submission_count()
    }
}

/// Authenticate one sealed feed's exact finite price before RelationV2 work.
///
/// The caller must authenticate the MarketBinding, EconomicDomain, Basis body,
/// and edge-policy registry account before invoking this pure seam. The SBF
/// adapter does so before creating the resumable Work account; subsequent work
/// binds the immutable feed/body/price/policy identities captured here.
#[allow(clippy::too_many_arguments)]
pub fn verify_quantized_relation_price_admission_v2(
    candidate_feed_identity: Id32,
    sealed_candidate_feed: &[u8],
    economic_domain_account: &clutch_general_v2_contract::EconomicDomainV2AccountV1,
    market_binding: &MarketBindingV1,
    native_basis: &NativeClaimBasisV1,
    authenticated_edge_policy: QuantizedEdgePolicyV1,
) -> Result<QuantizedRelationPriceAdmissionV2, GeneralV2RuntimeError> {
    if candidate_feed_identity.is_zero() {
        return Err(GeneralV2RuntimeError::Contract(
            clutch_general_v2_contract::CodecError::ZeroIdentity,
        ));
    }
    let (header, feed) = decode_sealed_candidate_feed_v1(sealed_candidate_feed)?;
    economic_domain_account.validate()?;
    market_binding.validate()?;
    native_basis.validate()?;

    let transcript = economic_domain_account.transcript;
    let relation_policy_id = quantized_relation_v2_policy_id_v2()?;
    let score_policy_id = score_v2_q_policy_id_v1()?;
    let basis_id = native_basis.id()?.bytes();
    let domain_digest = economic_domain_digest_v2(&CanonicalSha256, transcript)?;
    validate_quantized_relation_profile_v2(
        native_basis.basis_degree,
        native_basis.outcome_count,
        transcript.price_scale,
        native_basis.denominator,
    )?;
    if native_basis.edge_policy_registry_value != 1
        || authenticated_edge_policy != QuantizedEdgePolicyV1::Clamp
        || header.candidate_kind != SettlementCandidateKindV1::Direct
        || economic_domain_account.epoch != header.epoch
        || market_binding.market != header.market
        || market_binding.market_instance_v2_id.bytes()
            != transcript.market_instance_v2_id.bytes()
        || market_binding.market_genesis_profile_v2_id.bytes() == [0; 32]
        || market_binding.relation_policy_id != relation_policy_id
        || header.relation_policy_id != relation_policy_id
        || transcript.relation_policy_id != relation_policy_id
        || market_binding.score_policy_id != score_policy_id
        || market_binding.native_claim_basis_id.bytes() != basis_id
        || header.native_claim_basis_id.bytes() != basis_id
        || transcript.native_claim_basis_id.bytes() != basis_id
        || header.price_measure_policy_v1_id != market_binding.price_measure_policy_v1_id
        || transcript.price_measure_policy_v1_id != header.price_measure_policy_v1_id
        || header.economic_domain_digest != domain_digest
        || header.basis_degree != native_basis.basis_degree
        || header.outcome_count != native_basis.outcome_count
        || header.outcome_count != transcript.outcome_count
        || header.outcome_count != market_binding.outcome_count
        || header.price_scale != native_basis.denominator
        || header.price_scale != transcript.price_scale
        || header.price_scale != market_binding.price_scale
        || transcript.coordinate_domain_min >= transcript.coordinate_domain_max
    {
        return Err(GeneralV2RuntimeError::BindingMismatch);
    }

    let projected_basis = QuantizedBasisSpecV1 {
        outcome_count: native_basis.outcome_count,
        degree: native_basis.basis_degree,
        knot_count: native_basis.knot_count,
        uniform_log2_spacing: native_basis.uniform_log2_spacing,
        denominator: native_basis.denominator,
        domain_max: transcript.coordinate_domain_max,
        edge_policy: authenticated_edge_policy,
        knots: native_basis.knots,
    };
    projected_basis
        .validate()
        .map_err(|_| GeneralV2RuntimeError::BindingMismatch)?;

    let domain = EconomicDomainV2 {
        relation_version: transcript.relation_version,
        market_semantics_digest: transcript.market_instance_v2_id.bytes(),
        epoch_semantics_digest: transcript.epoch_semantics_digest.bytes(),
        relation_policy_digest: transcript.relation_policy_id.bytes(),
        price_policy_digest: transcript.price_measure_policy_v1_id.bytes(),
        epoch_index: transcript.epoch_index,
        outcome_count: transcript.outcome_count,
        price_scale: transcript.price_scale,
    };
    let candidate_price_digest = price_semantics_digest_v2(&domain, &feed.prices)?;
    if header.candidate_price_digest.bytes() != candidate_price_digest {
        return Err(GeneralV2RuntimeError::BindingMismatch);
    }
    let price_body_digest = quantized_witness_body_digest_v3(
        &CanonicalSha256,
        candidate_feed_identity,
        sealed_candidate_feed,
        true,
    )?;
    if header.price_body_digest != price_body_digest {
        return Err(GeneralV2RuntimeError::WitnessBodyDigestMismatch);
    }

    let certificate = verify_exact_smooth_atom_mixture_v1(
        market_binding.market,
        market_binding.market_genesis_profile_v2_id.bytes(),
        market_binding.native_claim_basis_id,
        candidate_price_digest,
        transcript.coordinate_domain_min,
        transcript.coordinate_domain_max,
        projected_basis,
        feed.prices,
        header.atom_count,
        header.common_denominator,
        feed.atom_coordinates,
        feed.atom_masses,
    )?;
    let price = PricePreconditionV2 {
        policy_digest: transcript.price_measure_policy_v1_id.bytes(),
        semantic_price_digest: candidate_price_digest,
        prices: feed.prices,
    };
    let authority = bind_quantized_relation_price_certificate_v2(domain, price, certificate)?;
    Ok(QuantizedRelationPriceAdmissionV2 {
        candidate_feed: candidate_feed_identity,
        economic_domain_digest: domain_digest,
        price_body_digest,
        authority,
    })
}

/// Authenticate the complete Product/Grid tuple and exact finite feed price.
///
/// This is the successor nonempty-Work semantic seam. The SBF adapter remains
/// responsible for program ownership, content-addressed artifact accounts,
/// PriceGrid PDA derivation, and the lifecycle accounts that name these bodies.
/// Once those account facts hold, this function prevents Work creation unless
/// one immutable MarketInstance, Genesis coordinate domain, Product basis,
/// price policy, PriceGrid, Relation policy, and exact atom certificate all
/// describe the same price. Success is not settlement or execution authority.
#[allow(clippy::too_many_arguments)]
pub fn verify_quantized_relation_product_price_admission_v2(
    candidate_feed_identity: Id32,
    sealed_candidate_feed: &[u8],
    economic_domain_account: &clutch_general_v2_contract::EconomicDomainV2AccountV1,
    market_binding_identity: Id32,
    market_binding: &MarketBindingV1,
    price_grid: &PriceGridAccount,
    product_template: &ProductTemplateV4,
    native_basis: &NativeClaimBasisV1,
    price_measure_policy: &PriceMeasurePolicyV1,
    genesis: &MarketGenesisProfileV2,
    market_instance: &MarketInstancePreimageV2,
    authenticated_edge_policy: QuantizedEdgePolicyV1,
) -> Result<QuantizedRelationProductPriceAdmissionV2, GeneralV2RuntimeError> {
    if market_binding_identity.is_zero() {
        return Err(GeneralV2RuntimeError::BindingMismatch);
    }
    market_instance.validate_bindings(
        product_template,
        native_basis,
        price_measure_policy,
        genesis,
    )?;
    genesis.validate_partition_bindings(
        native_basis,
        price_measure_policy,
        authenticated_edge_policy,
    )?;

    let transcript = economic_domain_account.transcript;
    let relation_policy_id = quantized_relation_v2_policy_id_v2()?;
    let score_policy_id = score_v2_q_policy_id_v1()?;
    if market_binding.market_genesis_profile_v2_id.bytes() != genesis.id()?.bytes()
        || market_binding.market_instance_v2_id.bytes() != market_instance.id()?.bytes()
        || market_binding.native_claim_basis_id.bytes() != native_basis.id()?.bytes()
        || market_binding.price_measure_policy_v1_id.bytes()
            != price_measure_policy.id()?.bytes()
        || market_binding.relation_policy_id != relation_policy_id
        || market_binding.score_policy_id != score_policy_id
        || genesis.relation_policy_id.bytes() != relation_policy_id.bytes()
        || genesis.score_policy_id.bytes() != score_policy_id.bytes()
        || price_grid.grid.bytes() != genesis.price_grid_id.bytes()
        || price_grid.realm.bytes() != genesis.realm_id.bytes()
        || price_grid.price_scale != market_binding.price_scale
        || transcript.market_instance_v2_id != market_binding.market_instance_v2_id
        || transcript.native_claim_basis_id != market_binding.native_claim_basis_id
        || transcript.price_measure_policy_v1_id != market_binding.price_measure_policy_v1_id
        || transcript.relation_policy_id != relation_policy_id
        || transcript.outcome_count != native_basis.outcome_count
        || transcript.price_scale != native_basis.denominator
        || transcript.coordinate_domain_min != genesis.coordinate_domain_min
        || transcript.coordinate_domain_max != genesis.coordinate_domain_max
    {
        return Err(GeneralV2RuntimeError::BindingMismatch);
    }

    let admission = verify_quantized_relation_price_admission_v2(
        candidate_feed_identity,
        sealed_candidate_feed,
        economic_domain_account,
        market_binding,
        native_basis,
        authenticated_edge_policy,
    )?;
    verify_quantized_relation_price_grid_v2(price_grid, &admission.authority)?;
    Ok(QuantizedRelationProductPriceAdmissionV2 {
        price_admission: admission,
        market_binding: market_binding_identity,
        market_genesis_profile_v2_id: market_binding.market_genesis_profile_v2_id,
        market_instance_v2_id: market_binding.market_instance_v2_id,
        product_template_id: Id32::new(product_template.id()?.bytes())?,
        price_grid_id: Id32::new(price_grid.grid.bytes())?,
        price_grid_realm: Id32::new(price_grid.realm.bytes())?,
    })
}

fn verify_quantized_relation_price_grid_v2(
    price_grid: &PriceGridAccount,
    authority: &QuantizedRelationPriceAuthorityV2,
) -> Result<(), GeneralV2RuntimeError> {
    price_grid.validate()?;
    if price_grid.price_scale != authority.domain().price_scale {
        return Err(GeneralV2RuntimeError::BindingMismatch);
    }
    let mut outcome = 0usize;
    while outcome < usize::from(authority.domain().outcome_count) {
        price_grid.tick_of(authority.price().prices[outcome])?;
        outcome += 1;
    }
    Ok(())
}

/// Bind a finite verifier result to the exact RelationV2 price transcript.
///
/// This crate-private constructor is the only path used by the builder before
/// it ranks a candidate. The public sealed-feed path additionally authenticates
/// Market, Terms, Basis, feed, and body provenance before reaching it.
pub(crate) fn bind_quantized_relation_price_certificate_v2(
    domain: EconomicDomainV2,
    price: PricePreconditionV2,
    certificate: VerifiedQuantizedAtomMixtureV1,
) -> Result<QuantizedRelationPriceAuthorityV2, GeneralV2RuntimeError> {
    let expected_price_digest = price_semantics_digest_v2(&domain, &price.prices)?;
    if domain.relation_policy_digest != quantized_relation_v2_policy_id_v2()?.bytes()
        || price.policy_digest != domain.price_policy_digest
        || price.semantic_price_digest != expected_price_digest
        || certificate.bindings().price_id != expected_price_digest
        || certificate.outcome_count() != domain.outcome_count
        || certificate.payout_denominator() != domain.price_scale
    {
        return Err(GeneralV2RuntimeError::BindingMismatch);
    }
    validate_quantized_relation_profile_v2(
        certificate.basis_degree(),
        certificate.outcome_count(),
        domain.price_scale,
        certificate.payout_denominator(),
    )?;
    Ok(QuantizedRelationPriceAuthorityV2 {
        domain,
        price,
        certificate,
    })
}

fn validate_quantized_relation_profile_v2(
    basis_degree: u8,
    outcome_count: u8,
    price_scale: u64,
    payout_denominator: u64,
) -> Result<(), GeneralV2RuntimeError> {
    if !(2..=3).contains(&basis_degree)
        || outcome_count <= basis_degree
        || !(2..=16).contains(&outcome_count)
        || price_scale == 0
        || price_scale != payout_denominator
    {
        return Err(GeneralV2RuntimeError::BindingMismatch);
    }
    Ok(())
}

/// Verify one owner-blind candidate only after exact finite price admission.
pub fn verify_quantized_relation_candidate_v2(
    admission: QuantizedRelationPriceAdmissionV2,
    book: &EconomicBookV2,
    candidate: &EconomicCandidateV2,
) -> Result<VerifiedQuantizedRelationCandidateV2, GeneralV2RuntimeError> {
    let economics = verify_quantized_relation_economics_v2(&admission.authority, book, candidate)?;
    Ok(VerifiedQuantizedRelationCandidateV2 {
        admission,
        economics,
    })
}

/// Crate-private RelationV2 entry point for the checked builder.
pub(crate) fn verify_quantized_relation_economics_v2(
    authority: &QuantizedRelationPriceAuthorityV2,
    book: &EconomicBookV2,
    candidate: &EconomicCandidateV2,
) -> Result<VerifiedEconomicsV2, GeneralV2RuntimeError> {
    verify_economic_candidate_v2(authority.domain(), book, authority.price(), candidate)
        .map_err(GeneralV2RuntimeError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_batch::relation_v2::{
        verify_economic_candidate_v2, EconomicBookV2, EconomicCandidateV2, EconomicDomainV2,
        PricePreconditionV2, ECONOMIC_RELATION_VERSION_V2,
    };
    use clutch_price_measure::{
        verify_quantized_atom_mixture_v1, BoundQuantizedSplineV1,
        QuantizedAtomMixtureBindingsV1, QuantizedAtomMixtureCertificateV1,
        QuantizedPayoutPriceVectorV1,
    };
    use clutch_solana_layout::{Hash32, MAX_GRID_TICKS};
    use clutch_general_v2_contract::{
        ClearWorkV3AccountV1, ClearWorkVerificationStateV1, DeletableRentOwnerV1,
        MarketBindingV1, SettlementCandidateKindV1, Sha256CheckpointV1,
        PRICE_MEASURE_WITNESS_SCHEMA_V3, QUANTIZED_PRICE_MEASURE_SEMANTICS_V1,
        SHA256_INITIAL_STATE_V1,
    };
    use crate::relation_v2_policy_id_v1;
    use crate::{score_v2_q_policy_id_v1, verify_quantized_clear_work_authority_v2};

    fn exact_price_authority_fixture() -> (
        EconomicDomainV2,
        PricePreconditionV2,
        VerifiedQuantizedAtomMixtureV1,
    ) {
        let mut knots = [0u128; 16];
        knots[..3].copy_from_slice(&[0, 2, 4]);
        let basis = QuantizedBasisSpecV1 {
            outcome_count: 4,
            degree: 2,
            knot_count: 3,
            uniform_log2_spacing: 1,
            denominator: 12,
            domain_max: 4,
            edge_policy: QuantizedEdgePolicyV1::Clamp,
            knots,
        };
        let prices = basis.evaluate(1).unwrap().weights;
        let domain = EconomicDomainV2 {
            relation_version: ECONOMIC_RELATION_VERSION_V2,
            market_semantics_digest: [1; 32],
            epoch_semantics_digest: [2; 32],
            relation_policy_digest: quantized_relation_v2_policy_id_v2().unwrap().bytes(),
            price_policy_digest: [3; 32],
            epoch_index: 9,
            outcome_count: 4,
            price_scale: 12,
        };
        let price_digest = price_semantics_digest_v2(&domain, &prices).unwrap();
        let price = PricePreconditionV2 {
            policy_digest: domain.price_policy_digest,
            semantic_price_digest: price_digest,
            prices,
        };
        let bindings = QuantizedAtomMixtureBindingsV1 {
            market_id: [4; 32],
            terms_id: [5; 32],
            basis_id: [6; 32],
            price_id: price_digest,
        };
        let bound = BoundQuantizedSplineV1 {
            bindings,
            coordinate_domain_min: 0,
            coordinate_domain_max: 4,
            basis,
        };
        let payout_price = QuantizedPayoutPriceVectorV1 {
            price_id: price_digest,
            outcome_count: 4,
            prices,
        };
        let mut coordinates = [0u128; 16];
        coordinates[0] = 1;
        let mut masses = [0u64; 16];
        masses[0] = 1;
        let certificate = QuantizedAtomMixtureCertificateV1::new(
            bindings,
            2,
            4,
            12,
            1,
            1,
            coordinates,
            masses,
        )
        .unwrap();
        let verified =
            verify_quantized_atom_mixture_v1(&bound, &payout_price, &certificate).unwrap();
        (domain, price, verified)
    }

    fn live_id(byte: u8) -> Id32 {
        Id32::new([byte; 32]).unwrap()
    }

    fn retained_work_authority_fixture() -> (
        QuantizedRelationProductPriceAdmissionV2,
        Id32,
        Id32,
        ClearWorkV3AccountV1,
        MarketBindingV1,
    ) {
        let (domain, price, certificate) = exact_price_authority_fixture();
        let candidate_feed = live_id(9);
        let economic_domain_digest = live_id(7);
        let price_body_digest = live_id(8);
        let market_binding_identity = live_id(10);
        let relation_policy_id = Id32::new(domain.relation_policy_digest).unwrap();
        let score_policy_id = score_v2_q_policy_id_v1().unwrap();
        let admission = QuantizedRelationPriceAdmissionV2 {
            candidate_feed,
            economic_domain_digest,
            price_body_digest,
            authority: bind_quantized_relation_price_certificate_v2(
                domain,
                price,
                certificate,
            )
            .unwrap(),
        };
        let product_admission = QuantizedRelationProductPriceAdmissionV2 {
            price_admission: admission,
            market_binding: market_binding_identity,
            market_genesis_profile_v2_id: live_id(5),
            market_instance_v2_id: live_id(1),
            product_template_id: live_id(11),
            price_grid_id: live_id(12),
            price_grid_realm: live_id(13),
        };
        let work = ClearWorkV3AccountV1 {
            epoch: Id32::ZERO,
            node: Id32::ZERO,
            market: live_id(4),
            order_set: Id32::ZERO,
            feed: candidate_feed,
            candidate_bundle_digest: Id32::ZERO,
            settlement_candidate_id: Id32::ZERO,
            base_relation_candidate_id: Id32::ZERO,
            relation_policy_id,
            economic_domain_digest,
            native_claim_basis_id: live_id(6),
            candidate_price_digest: Id32::new(price.semantic_price_digest).unwrap(),
            price_measure_policy_v1_id: live_id(3),
            score_policy_id,
            price_body_digest,
            previous_order_id: Id32::ZERO,
            epoch_generation: 0,
            rent: DeletableRentOwnerV1 {
                payer: Id32::ZERO,
                refundable_principal: 0,
                donation_floor: 0,
            },
            reward_remaining: 0,
            reward_earned: 0,
            slice_count: 0,
            slice_cursor: 0,
            page_count: 0,
            page_cursor: 0,
            outcome_count: 4,
            order_count: 0,
            order_cursor: 0,
            slot_cursor: 0,
            phase: 0,
            candidate_kind: SettlementCandidateKindV1::Direct,
            price_witness_schema: PRICE_MEASURE_WITNESS_SCHEMA_V3,
            quantized_semantics_version: QUANTIZED_PRICE_MEASURE_SEMANTICS_V1,
            stored_bump: 0,
            verification_state: ClearWorkVerificationStateV1::Pending,
            flags: 0,
            sha256: Sha256CheckpointV1 {
                state: SHA256_INITIAL_STATE_V1,
                block: [0; 64],
                block_len: 0,
                total_len: 0,
            },
        };
        let binding = MarketBindingV1 {
            market: live_id(4),
            market_genesis_profile_v2_id: live_id(5),
            market_instance_v2_id: live_id(1),
            series_plan_v5_id: Id32::ZERO,
            series_funding_terms_v2_id: Id32::ZERO,
            relation_policy_id,
            price_measure_policy_v1_id: live_id(3),
            native_claim_basis_id: live_id(6),
            admission_policy_id: Id32::ZERO,
            score_policy_id,
            settlement_policy_id: Id32::ZERO,
            neutral_sink: Id32::ZERO,
            price_scale: 12,
            commit_span_slots: 0,
            reveal_span_slots: 0,
            verification_span_slots: 0,
            bond_lamports: 0,
            invalidity_penalty: 0,
            abandonment_penalty: 0,
            node_cleanup_reward: 0,
            price_check_reward: 0,
            order_reward: 0,
            slice_reward: 0,
            completion_reward: 0,
            work_close_reward: 0,
            feed_close_reward: 0,
            freeze_reward: 0,
            finalize_reward: 0,
            solver_prize: 0,
            root_close_reward: 0,
            relation_version: 2,
            outcome_count: 4,
            basis_degree: 2,
            rank_key_len: 88,
            candidate_kind_mask: 1,
            stored_bump: 0,
            flags: 0,
        };
        (
            product_admission,
            candidate_feed,
            market_binding_identity,
            work,
            binding,
        )
    }

    #[test]
    fn successor_policy_commits_exact_finite_profile_and_is_breaking() {
        assert_eq!(QUANTIZED_RELATION_V2_POLICY_BODY_V2[9], 1);
        assert_eq!(QUANTIZED_RELATION_V2_POLICY_BODY_V2[10], 1);
        assert_eq!(QUANTIZED_RELATION_V2_POLICY_BODY_V2[11], 1);
        assert_eq!(&QUANTIZED_RELATION_V2_POLICY_BODY_V2[12..14], &[2, 3]);
        assert_ne!(
            quantized_relation_v2_policy_id_v2().unwrap(),
            relation_v2_policy_id_v1().unwrap()
        );
    }

    #[test]
    fn resumed_work_rejects_every_call_local_authority_substitution() {
        let (admission, feed, binding_id, work, binding) = retained_work_authority_fixture();
        assert_eq!(
            verify_quantized_clear_work_authority_v2(
                admission,
                feed,
                binding_id,
                &work,
                &binding,
            ),
            Ok(())
        );

        assert_eq!(
            verify_quantized_clear_work_authority_v2(
                admission,
                live_id(14),
                binding_id,
                &work,
                &binding,
            ),
            Err(crate::GeneralV2WorkErrorV1::BindingMismatch)
        );
        assert_eq!(
            verify_quantized_clear_work_authority_v2(
                admission,
                feed,
                live_id(14),
                &work,
                &binding,
            ),
            Err(crate::GeneralV2WorkErrorV1::BindingMismatch)
        );

        let wrong_body_work = ClearWorkV3AccountV1 {
            price_body_digest: live_id(14),
            ..work
        };
        assert_eq!(
            verify_quantized_clear_work_authority_v2(
                admission,
                feed,
                binding_id,
                &wrong_body_work,
                &binding,
            ),
            Err(crate::GeneralV2WorkErrorV1::BindingMismatch)
        );

        let wrong_instance_admission = QuantizedRelationProductPriceAdmissionV2 {
            market_instance_v2_id: live_id(14),
            ..admission
        };
        assert_eq!(
            verify_quantized_clear_work_authority_v2(
                wrong_instance_admission,
                feed,
                binding_id,
                &work,
                &binding,
            ),
            Err(crate::GeneralV2WorkErrorV1::BindingMismatch)
        );

        let wrong_scale_binding = MarketBindingV1 {
            price_scale: 13,
            ..binding
        };
        assert_eq!(
            verify_quantized_clear_work_authority_v2(
                admission,
                feed,
                binding_id,
                &work,
                &wrong_scale_binding,
            ),
            Err(crate::GeneralV2WorkErrorV1::BindingMismatch)
        );
    }

    #[test]
    fn successor_profile_rejects_unproved_degrees_and_scale_substitution() {
        for degree in [0, 1, 4, u8::MAX] {
            assert_eq!(
                validate_quantized_relation_profile_v2(degree, 5, 1_000, 1_000),
                Err(GeneralV2RuntimeError::BindingMismatch)
            );
        }
        assert_eq!(
            validate_quantized_relation_profile_v2(2, 2, 1_000, 1_000),
            Err(GeneralV2RuntimeError::BindingMismatch)
        );
        assert_eq!(
            validate_quantized_relation_profile_v2(3, 4, 999, 1_000),
            Err(GeneralV2RuntimeError::BindingMismatch)
        );
        assert_eq!(
            validate_quantized_relation_profile_v2(2, 16, 1_000, 1_000),
            Ok(())
        );
    }

    #[test]
    fn exact_certificate_is_required_against_the_same_policy_and_price_digest() {
        let (domain, price, certificate) = exact_price_authority_fixture();
        assert!(bind_quantized_relation_price_certificate_v2(domain, price, certificate).is_ok());

        let wrong_policy_domain = EconomicDomainV2 {
            relation_policy_digest: relation_v2_policy_id_v1().unwrap().bytes(),
            ..domain
        };
        assert_eq!(
            bind_quantized_relation_price_certificate_v2(
                wrong_policy_domain,
                price,
                certificate,
            ),
            Err(GeneralV2RuntimeError::BindingMismatch)
        );

        let forged_price_identity = PricePreconditionV2 {
            semantic_price_digest: [9; 32],
            ..price
        };
        assert_eq!(
            bind_quantized_relation_price_certificate_v2(
                domain,
                forged_price_identity,
                certificate,
            ),
            Err(GeneralV2RuntimeError::BindingMismatch)
        );
    }

    #[test]
    fn every_active_exact_price_must_be_a_grid_tick() {
        let (domain, price, certificate) = exact_price_authority_fixture();
        let authority =
            bind_quantized_relation_price_certificate_v2(domain, price, certificate).unwrap();
        let mut dense_ticks = [0u64; MAX_GRID_TICKS];
        let mut tick = 0usize;
        while tick <= 12 {
            dense_ticks[tick] = u64::try_from(tick).unwrap();
            tick += 1;
        }
        let mut admitted_grid = PriceGridAccount {
            grid: Hash32::ZERO,
            realm: Hash32::from_bytes([7; 32]),
            price_scale: 12,
            tick_count: 13,
            ticks: dense_ticks,
            stored_bump: 1,
            flags: 0,
        };
        admitted_grid.grid = admitted_grid.recomputed_grid_id().unwrap();
        assert_eq!(
            verify_quantized_relation_price_grid_v2(&admitted_grid, &authority),
            Ok(())
        );

        let mut endpoint_only = PriceGridAccount {
            tick_count: 2,
            ticks: [0; MAX_GRID_TICKS],
            ..admitted_grid
        };
        endpoint_only.ticks[1] = 12;
        endpoint_only.grid = endpoint_only.recomputed_grid_id().unwrap();
        assert!(matches!(
            verify_quantized_relation_price_grid_v2(&endpoint_only, &authority),
            Err(GeneralV2RuntimeError::PriceGrid(_))
        ));

        let mut wrong_scale = admitted_grid;
        wrong_scale.price_scale = 13;
        wrong_scale.grid = wrong_scale.recomputed_grid_id().unwrap();
        assert_eq!(
            verify_quantized_relation_price_grid_v2(&wrong_scale, &authority),
            Err(GeneralV2RuntimeError::BindingMismatch)
        );
    }

    #[test]
    fn successor_policy_is_bound_into_relation_candidate_identity() {
        let prices = {
            let mut value = [0u64; 16];
            value[..3].copy_from_slice(&[250, 250, 500]);
            value
        };
        let legacy_policy = relation_v2_policy_id_v1().unwrap().bytes();
        let successor_policy = quantized_relation_v2_policy_id_v2().unwrap().bytes();
        let candidate_digest = |relation_policy_digest| {
            let domain = EconomicDomainV2 {
                relation_version: ECONOMIC_RELATION_VERSION_V2,
                market_semantics_digest: [1; 32],
                epoch_semantics_digest: [2; 32],
                relation_policy_digest,
                price_policy_digest: [3; 32],
                epoch_index: 9,
                outcome_count: 3,
                price_scale: 1_000,
            };
            let price = PricePreconditionV2 {
                policy_digest: domain.price_policy_digest,
                semantic_price_digest: price_semantics_digest_v2(&domain, &prices).unwrap(),
                prices,
            };
            verify_economic_candidate_v2(
                &domain,
                &EconomicBookV2::empty(),
                &price,
                &EconomicCandidateV2::EMPTY,
            )
            .unwrap()
            .economic_candidate_digest
        };
        assert_ne!(candidate_digest(legacy_policy), candidate_digest(successor_policy));
    }
}
