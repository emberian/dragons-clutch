//! Compiler-shaped entrance for a live degree-2/3 Product.
//!
//! A spline Product has an acyclic construction order: its semantic basis ID
//! selects the Product graph, while the finalized linked basis in turn names
//! that graph's exact ResultDomain digest. This module owns that order. It also
//! verifies the offered `DCLTPGT1` certificate against the production spline
//! evaluator before the certificate digest can enter either basis identity.
//!
//! The output is the complete immutable record graph Core Found consumes:
//! Product, ResultDomain, Portfolio, ProductBasisV3, and price-gate coordinates.
//! It does not publish, sign, submit, fund, or mutate an account.

use dclutch_product_payoff_v2_codec::{
    price_gate_v1::{PriceGateCertificateV1, verify_price_gate_v1},
    registry_v3::{GRADED_BASIS_RECORD_SCHEMA_ID_V3, PRICE_GATE_RECORD_SCHEMA_ID_V1},
    runtime_v3::{
        BasisInputV3, BasisKindV3, ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        basis_record_bytes_v3, compile_basis_v3, semantic_basis_preimage_v3,
    },
};
use dclutch_product_runtime_v2::ContentId;
use dclutch_product_runtime_v2_admission::FinalizedRecordCoordinateV2;
use solana_program::{hash::hashv, pubkey::Pubkey};

use crate::{
    CompiledProductRecordsV2, Error, ProductCompilationInputV2, Result, compile_product_records_v2,
    coordinate, digest,
};

const PROVISIONAL_RESULT_DOMAIN_ID_V3: [u8; 32] = [0x53; 32];

/// Irreducible Product graph and spline-basis authoring inputs.
///
/// Basis width is not a caller field. It is the length of `failure_payouts`,
/// then independently checked against `knots.len() - degree - 1` by the
/// ProductBasisV3 compiler. The semantic liability-basis ID and both finalized
/// record digests are derived outputs, never caller assertions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplineProductCompilationInputV3<'a> {
    /// Stable Product semantic identity.
    pub product_id: ContentId,
    /// Exact source coordinate-domain identity.
    pub coordinate_domain_id: ContentId,
    /// Exact result-unit identity.
    pub result_unit_id: ContentId,
    /// Exact Claims basis used by the Product's portfolio.
    pub claim_basis_id: ContentId,
    /// Product-selected representation semantic release.
    pub representation_release_id: ContentId,
    /// Product-selected coordinate-mapping semantic release.
    pub mapping_release_id: ContentId,
    /// Positive common denominator for Product result-domain cuts.
    pub cut_denominator: u64,
    /// Strictly increasing Product result-domain cuts.
    pub cuts: &'a [i128],
    /// Positive common denominator for portfolio coefficients.
    pub portfolio_denominator: u64,
    /// One portfolio coefficient per Product outcome.
    pub coefficients: &'a [u64],
    /// Immutable production spline evaluator release.
    pub evaluator_release_id: ContentId,
    /// Cox-de Boor degree; the live profile admits exactly two or three.
    pub degree: u8,
    /// Whether repeated interior knots are declared by this Product.
    pub interior_multiplicity: bool,
    /// Exact integer payout partition scale.
    pub payout_scale: u64,
    /// Positive common denominator for spline knot numerators.
    pub knot_denominator: u64,
    /// Canonical clamped spline knot numerators.
    pub knots: &'a [i128],
    /// Exact resolution-failure payout partition; its length derives width.
    pub failure_payouts: &'a [u64],
    /// Complete canonical DCLTPGT1 certificate bytes.
    pub price_gate_certificate: &'a [u8],
}

/// Complete derived immutable-record coordinates for one spline Product.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledSplineProductRecordsV3 {
    /// Product, ResultDomain, and Portfolio records and admission request.
    pub product: CompiledProductRecordsV2,
    /// Acyclic semantic basis identity persisted by the Product graph.
    pub semantic_basis_id: ContentId,
    /// Exact Product-linked ProductBasisV3 raw/staging coordinate.
    pub linked_basis: FinalizedRecordCoordinateV2,
    /// Exact verified DCLTPGT1 raw/staging coordinate appended to Core Found.
    pub price_gate: FinalizedRecordCoordinateV2,
    /// Width derived from the canonical spline record.
    pub basis_width: u32,
    /// Decoded certificate returned only after its hull identity was verified.
    pub verified_price_gate: PriceGateCertificateV1,
}

/// Return the exact ProductBasisV3 output width required by this request.
pub fn spline_basis_output_bytes_v3(input: SplineProductCompilationInputV3<'_>) -> Result<usize> {
    let basis_width = input.failure_payouts.len();
    basis_record_bytes_v3(kind(input), basis_width, input.knots.len(), 0)
        .map_err(|_| Error::SplineBasis)
}

/// Compile the Product graph, spline basis, and admitted price-gate pair.
///
/// All candidates are built in private scratch buffers. Caller outputs remain
/// byte-for-byte unchanged unless basis encoding, price-gate hull verification,
/// semantic linkage, Product compilation, and final decoding all succeed.
pub fn compile_spline_product_records_v3(
    registry_program: Pubkey,
    input: SplineProductCompilationInputV3<'_>,
    product_output: &mut [u8],
    domain_output: &mut [u8],
    portfolio_output: &mut [u8],
    linked_basis_output: &mut [u8],
) -> Result<CompiledSplineProductRecordsV3> {
    let expected_basis_bytes = spline_basis_output_bytes_v3(input)?;
    if linked_basis_output.len() != expected_basis_bytes {
        return Err(Error::OutputLength);
    }
    let basis_width = u32::try_from(input.failure_payouts.len()).map_err(|_| Error::SplineBasis)?;
    let certificate_digest = digest(input.price_gate_certificate)?;
    let basis_input = BasisInputV3 {
        kind: kind(input),
        product_id: input.product_id.to_bytes(),
        result_domain_id: PROVISIONAL_RESULT_DOMAIN_ID_V3,
        coordinate_domain_id: input.coordinate_domain_id.to_bytes(),
        result_unit_id: input.result_unit_id.to_bytes(),
        evaluator_release_id: input.evaluator_release_id.to_bytes(),
        basis_width,
        payout_scale: input.payout_scale,
        knot_denominator: input.knot_denominator,
        knots: input.knots,
        terms: &[],
        failure_payouts: input.failure_payouts,
        price_gate_certificate_digest: certificate_digest.to_bytes(),
    };

    let mut provisional_basis = vec![0_u8; expected_basis_bytes];
    compile_basis_v3(basis_input, &mut provisional_basis).map_err(|_| Error::SplineBasis)?;
    let provisional = ProductBasisV3::decode(&provisional_basis).map_err(|_| Error::SplineBasis)?;
    let verified_price_gate = verify_price_gate_v1(
        &provisional,
        input.knot_denominator,
        input.payout_scale,
        input.degree,
        basis_width,
        input.price_gate_certificate,
    )
    .map_err(|_| Error::PriceGate)?;
    let semantic_basis_id = derive_semantic_basis_id(&provisional_basis)?;

    let mut candidate_product = vec![0_u8; product_output.len()];
    let mut candidate_domain = vec![0_u8; domain_output.len()];
    let mut candidate_portfolio = vec![0_u8; portfolio_output.len()];
    let product = compile_product_records_v2(
        registry_program,
        ProductCompilationInputV2 {
            product_id: input.product_id,
            coordinate_domain_id: input.coordinate_domain_id,
            result_unit_id: input.result_unit_id,
            claim_basis_id: input.claim_basis_id,
            liability_basis_id: semantic_basis_id,
            representation_release_id: input.representation_release_id,
            mapping_release_id: input.mapping_release_id,
            cut_denominator: input.cut_denominator,
            cuts: input.cuts,
            portfolio_denominator: input.portfolio_denominator,
            coefficients: input.coefficients,
        },
        &mut candidate_product,
        &mut candidate_domain,
        &mut candidate_portfolio,
    )?;

    let mut candidate_basis = vec![0_u8; expected_basis_bytes];
    compile_basis_v3(
        BasisInputV3 {
            result_domain_id: product.receipt.result_domain.content_digest.to_bytes(),
            ..basis_input
        },
        &mut candidate_basis,
    )
    .map_err(|_| Error::SplineBasis)?;
    let linked = ProductBasisV3::decode(&candidate_basis).map_err(|_| Error::SplineBasis)?;
    if linked.product_id() != input.product_id.to_bytes()
        || linked.result_domain_id() != product.receipt.result_domain.content_digest.to_bytes()
        || linked.coordinate_domain_id() != input.coordinate_domain_id.to_bytes()
        || linked.result_unit_id() != input.result_unit_id.to_bytes()
        || linked.price_gate_certificate_digest_v3() != certificate_digest.to_bytes()
        || derive_semantic_basis_id(&candidate_basis)? != semantic_basis_id
    {
        return Err(Error::CrossRecordMismatch);
    }
    verify_price_gate_v1(
        &linked,
        input.knot_denominator,
        input.payout_scale,
        input.degree,
        basis_width,
        input.price_gate_certificate,
    )
    .map_err(|_| Error::PriceGate)?;

    let linked_basis = coordinate(
        registry_program,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        digest(&candidate_basis)?,
    )?;
    let price_gate = coordinate(
        registry_program,
        PRICE_GATE_RECORD_SCHEMA_ID_V1,
        certificate_digest,
    )?;
    product_output.copy_from_slice(&candidate_product);
    domain_output.copy_from_slice(&candidate_domain);
    portfolio_output.copy_from_slice(&candidate_portfolio);
    linked_basis_output.copy_from_slice(&candidate_basis);
    Ok(CompiledSplineProductRecordsV3 {
        product,
        semantic_basis_id,
        linked_basis,
        price_gate,
        basis_width,
        verified_price_gate,
    })
}

fn kind(input: SplineProductCompilationInputV3<'_>) -> BasisKindV3 {
    BasisKindV3::SplineDegree2To3 {
        degree: input.degree,
        interior_multiplicity: input.interior_multiplicity,
    }
}

fn derive_semantic_basis_id(bytes: &[u8]) -> Result<ContentId> {
    let semantic = semantic_basis_preimage_v3(bytes).map_err(|_| Error::SplineBasis)?;
    ContentId::new(
        hashv(&[
            SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
            semantic.prefix(),
            semantic.suffix(),
        ])
        .to_bytes(),
    )
    .map_err(|_| Error::SplineBasis)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;
    use dclutch_product_payoff_v2_codec::{
        price_gate_v1::{
            PRICE_GATE_ATOM_COUNT_OFFSET_V1, PRICE_GATE_DEGREE_OFFSET_V1,
            PRICE_GATE_DENOMINATORS_OFFSET_V1, PRICE_GATE_MAGIC_OFFSET_V1, PRICE_GATE_MAGIC_V1,
            PRICE_GATE_MASS_OFFSET_V1, PRICE_GATE_NUMERATORS_OFFSET_V1,
            PRICE_GATE_PRICES_OFFSET_V1, PRICE_GATE_PROFILE_OFFSET_V1, PRICE_GATE_PROFILE_V1,
            PRICE_GATE_REQUEST_BYTES_V1, PRICE_GATE_SCALE_OFFSET_V1, PRICE_GATE_SCHEMA_VERSION_V1,
            PRICE_GATE_VERSION_OFFSET_V1, PRICE_GATE_WEIGHTS_OFFSET_V1, PRICE_GATE_WIDTH_OFFSET_V1,
        },
        runtime_v3::ProductBasisV3,
    };
    use dclutch_product_runtime_v2::{portfolio_record_bytes, result_domain_record_bytes};
    use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
    use solana_program::hash::hash;

    fn content(fill: u8) -> ContentId {
        ContentId::new([fill; 32]).expect("content identity")
    }

    fn certificate() -> [u8; PRICE_GATE_REQUEST_BYTES_V1] {
        let mut bytes = [0_u8; PRICE_GATE_REQUEST_BYTES_V1];
        bytes[PRICE_GATE_MAGIC_OFFSET_V1..PRICE_GATE_MAGIC_OFFSET_V1 + 8]
            .copy_from_slice(&PRICE_GATE_MAGIC_V1);
        bytes[PRICE_GATE_VERSION_OFFSET_V1..PRICE_GATE_VERSION_OFFSET_V1 + 2]
            .copy_from_slice(&PRICE_GATE_SCHEMA_VERSION_V1.to_le_bytes());
        bytes[PRICE_GATE_PROFILE_OFFSET_V1..PRICE_GATE_PROFILE_OFFSET_V1 + 2]
            .copy_from_slice(&PRICE_GATE_PROFILE_V1.to_le_bytes());
        bytes[PRICE_GATE_SCALE_OFFSET_V1..PRICE_GATE_SCALE_OFFSET_V1 + 4]
            .copy_from_slice(&7_u32.to_le_bytes());
        bytes[PRICE_GATE_MASS_OFFSET_V1..PRICE_GATE_MASS_OFFSET_V1 + 8]
            .copy_from_slice(&1_u64.to_le_bytes());
        bytes[PRICE_GATE_DEGREE_OFFSET_V1] = 2;
        bytes[PRICE_GATE_WIDTH_OFFSET_V1] = 3;
        bytes[PRICE_GATE_ATOM_COUNT_OFFSET_V1] = 1;
        for (claim, payout) in [1_u64, 4, 2].iter().enumerate() {
            let offset = PRICE_GATE_PRICES_OFFSET_V1 + claim * 8;
            bytes[offset..offset + 8].copy_from_slice(&payout.to_le_bytes());
        }
        bytes[PRICE_GATE_WEIGHTS_OFFSET_V1..PRICE_GATE_WEIGHTS_OFFSET_V1 + 8]
            .copy_from_slice(&1_u64.to_le_bytes());
        bytes[PRICE_GATE_NUMERATORS_OFFSET_V1..PRICE_GATE_NUMERATORS_OFFSET_V1 + 8]
            .copy_from_slice(&3_i64.to_le_bytes());
        bytes[PRICE_GATE_DENOMINATORS_OFFSET_V1..PRICE_GATE_DENOMINATORS_OFFSET_V1 + 4]
            .copy_from_slice(&2_u32.to_le_bytes());
        bytes
    }

    fn input<'a>(
        certificate: &'a [u8],
        knots: &'a [i128],
        failure: &'a [u64],
    ) -> SplineProductCompilationInputV3<'a> {
        SplineProductCompilationInputV3 {
            product_id: content(1),
            coordinate_domain_id: content(2),
            result_unit_id: content(3),
            claim_basis_id: content(4),
            representation_release_id: content(5),
            mapping_release_id: content(6),
            cut_denominator: 1,
            cuts: &[1],
            portfolio_denominator: 1,
            coefficients: &[1, 1, 1],
            evaluator_release_id: content(7),
            degree: 2,
            interior_multiplicity: false,
            payout_scale: 7,
            knot_denominator: 1,
            knots,
            failure_payouts: failure,
            price_gate_certificate: certificate,
        }
    }

    #[test]
    fn compiles_the_exact_graph_and_verified_founding_pair() {
        let certificate = certificate();
        let knots = [0_i128, 0, 0, 3, 3, 3];
        let failure = [0_u64, 0, 7];
        let input = input(&certificate, &knots, &failure);
        let mut product = [0_u8; PRODUCT_RECORD_BYTES_V2];
        let mut domain = vec![0_u8; result_domain_record_bytes(1).expect("domain bytes")];
        let mut portfolio = vec![0_u8; portfolio_record_bytes(3).expect("portfolio bytes")];
        let mut basis = vec![0_u8; spline_basis_output_bytes_v3(input).expect("basis bytes")];
        let registry = Pubkey::new_unique();
        let compiled = compile_spline_product_records_v3(
            registry,
            input,
            &mut product,
            &mut domain,
            &mut portfolio,
            &mut basis,
        )
        .expect("spline Product graph");
        let decoded = ProductBasisV3::decode(&basis).expect("linked basis");
        assert_eq!(compiled.basis_width, 3);
        assert_eq!(compiled.verified_price_gate.active_prices(), &[1, 4, 2]);
        assert_eq!(
            compiled.price_gate.schema_id.to_bytes(),
            PRICE_GATE_RECORD_SCHEMA_ID_V1
        );
        assert_eq!(
            compiled.price_gate.content_digest.to_bytes(),
            hash(&certificate).to_bytes()
        );
        assert_eq!(
            compiled.linked_basis.content_digest.to_bytes(),
            hash(&basis).to_bytes()
        );
        assert_eq!(
            decoded.result_domain_id(),
            compiled
                .product
                .receipt
                .result_domain
                .content_digest
                .to_bytes()
        );
        assert_eq!(
            decoded.price_gate_certificate_digest_v3(),
            compiled.price_gate.content_digest.to_bytes()
        );
        assert_eq!(
            compiled.semantic_basis_id,
            derive_semantic_basis_id(&basis).expect("semantic basis")
        );
    }

    #[test]
    fn forged_gate_refuses_without_mutating_any_output() {
        let mut certificate = certificate();
        certificate[PRICE_GATE_PRICES_OFFSET_V1] = 2;
        let knots = [0_i128, 0, 0, 3, 3, 3];
        let failure = [0_u64, 0, 7];
        let input = input(&certificate, &knots, &failure);
        let mut product = [0xa1_u8; PRODUCT_RECORD_BYTES_V2];
        let mut domain = vec![0xa2_u8; result_domain_record_bytes(1).expect("domain bytes")];
        let mut portfolio = vec![0xa3_u8; portfolio_record_bytes(3).expect("portfolio bytes")];
        let mut basis = vec![0xa4_u8; spline_basis_output_bytes_v3(input).expect("basis bytes")];
        let before = (product, domain.clone(), portfolio.clone(), basis.clone());
        assert_eq!(
            compile_spline_product_records_v3(
                Pubkey::new_unique(),
                input,
                &mut product,
                &mut domain,
                &mut portfolio,
                &mut basis,
            ),
            Err(Error::PriceGate)
        );
        assert_eq!((product, domain, portfolio, basis), before);
    }

    #[test]
    fn knot_width_mismatch_refuses_before_output_mutation() {
        let certificate = certificate();
        let knots = [0_i128, 0, 0, 3, 3];
        let failure = [0_u64, 0, 7];
        let input = input(&certificate, &knots, &failure);
        let mut product = [0xb1_u8; PRODUCT_RECORD_BYTES_V2];
        let mut domain = vec![0xb2_u8; result_domain_record_bytes(1).expect("domain bytes")];
        let mut portfolio = vec![0xb3_u8; portfolio_record_bytes(3).expect("portfolio bytes")];
        let mut basis = vec![0xb4_u8; spline_basis_output_bytes_v3(input).expect("basis bytes")];
        let before = (product, domain.clone(), portfolio.clone(), basis.clone());
        assert_eq!(
            compile_spline_product_records_v3(
                Pubkey::new_unique(),
                input,
                &mut product,
                &mut domain,
                &mut portfolio,
                &mut basis,
            ),
            Err(Error::SplineBasis)
        );
        assert_eq!((product, domain, portfolio, basis), before);
    }
}
