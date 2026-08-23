//! Offline exact price compilation over one canonical Product basis.
//!
//! This module is authority-neutral host tooling. It joins the Product
//! compiler's checked Basis/Terms projection to the allocation-free exact atom
//! solver and emits two canonical artifacts: the production verifier's exact
//! fixed-width certificate, when one exists, and a fixed-width work manifest
//! for every terminal search outcome. It does not authenticate any repeated
//! identity, register a Product, select a candidate, or authorize an onchain
//! route.

use clutch_price_measure::{
    solve_quantized_atom_hull_v1, verify_quantized_atom_mixture_v1,
    BoundQuantizedSplineV1, QuantizedAtomAllSupportSolverOutcomeV1,
    QuantizedAtomAllSupportSolverPlanV1,
    QuantizedAtomMixtureBindingsV1, QuantizedAtomMixtureCertificateV1,
    QuantizedAtomSearchCoordinatesV1, QuantizedAtomSolverErrorV1,
    QuantizedPayoutPriceVectorV1, MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1,
    QUANTIZED_ATOM_MIXTURE_CERTIFICATE_BYTES_V1,
};
use clutch_product_series::{
    CompiledProductSeriesBundleV5, CompiledProductSeriesBundleV5Id, ContentId,
    Error as ProductError, MAX_OUTCOMES,
};
use sha2::{Digest, Sha256};

use crate::production::{CompiledProductionPayoffV1, ProductionCompilerError};

/// Exact byte width of the deterministic work manifest.
pub const EXACT_MARKET_WORK_MANIFEST_BYTES_V1: usize = 1_640;
/// Exact byte width of a successful production atom certificate.
pub const EXACT_MARKET_CERTIFICATE_BYTES_V1: usize =
    QUANTIZED_ATOM_MIXTURE_CERTIFICATE_BYTES_V1;
/// Domain separating the manifest's authority-neutral content checksum.
pub const EXACT_MARKET_WORK_MANIFEST_DOMAIN_V1: &[u8] =
    b"dragons-clutch/exact-market-work-manifest/v1";
/// Domain separating the certificate transport checksum in the manifest.
pub const EXACT_MARKET_CERTIFICATE_OUTPUT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/exact-market-certificate-output/v1";
/// Exact byte width of the BundleV5-bound operator sidecar.
pub const EXACT_MARKET_BUNDLE_SIDECAR_BYTES_V1: usize = 176;
/// Domain separating the current BundleV5-bound operator sidecar identity.
pub const EXACT_MARKET_BUNDLE_SIDECAR_DOMAIN_V1: &[u8] =
    b"dragons-clutch/exact-market-bundle-sidecar/v1";
/// Current immutable Product compiler bundle artifact kind.
pub const COMPILED_PRODUCT_SERIES_BUNDLE_V5_ARTIFACT_KIND: u8 = 60;

const EXACT_MARKET_WORK_MANIFEST_MAGIC_V1: [u8; 8] = *b"DCEMWV1\0";
const EXACT_MARKET_WORK_MANIFEST_SCHEMA_V1: u16 = 1;
const EXACT_MARKET_SOLVER_SEMANTICS_V1: u16 = 1;
const EXACT_MARKET_BUNDLE_SIDECAR_MAGIC_V1: [u8; 8] = *b"DCEMSV1\0";
const EXACT_MARKET_BUNDLE_SIDECAR_SCHEMA_V1: u16 = 1;

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1 == 64);

/// Whether a search enumerated the complete integer Terms domain or only the
/// exact caller-declared coordinate subset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactMarketCoordinateCoverageV1 {
    /// Every integer coordinate from the inclusive Terms minimum through its
    /// inclusive maximum was declared.
    FullIntegerDomain,
    /// Only the exact coordinates recorded in the manifest were searched.
    DeclaredCoordinateSubset,
}

impl ExactMarketCoordinateCoverageV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::FullIntegerDomain => 1,
            Self::DeclaredCoordinateSubset => 2,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, ExactMarketManifestErrorV1> {
        match tag {
            1 => Ok(Self::FullIntegerDomain),
            2 => Ok(Self::DeclaredCoordinateSubset),
            _ => Err(ExactMarketManifestErrorV1::InvalidDiscriminant),
        }
    }
}

/// Exact terminal classification from the all-support finite search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactMarketSearchOutcomeV1 {
    /// A canonical certificate was found and independently reverified.
    Solved,
    /// The entire declared-coordinate hull was exhausted without an exact
    /// representable certificate.
    Unsupported,
    /// At least one exact positive solution exceeded the V1 `u64` mass
    /// representation profile.
    OutOfProfile,
    /// The named support family stopped at the caller's deterministic budget.
    WorkTruncated,
}

impl ExactMarketSearchOutcomeV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::Solved => 1,
            Self::Unsupported => 2,
            Self::OutOfProfile => 3,
            Self::WorkTruncated => 4,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, ExactMarketManifestErrorV1> {
        match tag {
            1 => Ok(Self::Solved),
            2 => Ok(Self::Unsupported),
            3 => Ok(Self::OutOfProfile),
            4 => Ok(Self::WorkTruncated),
            _ => Err(ExactMarketManifestErrorV1::InvalidDiscriminant),
        }
    }
}

/// Fixed-capacity, authority-neutral request for one exact price search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactMarketCompilerRequestV1 {
    market_id: ContentId,
    product_terms_id: ContentId,
    price_id: ContentId,
    price_count: u8,
    prices: [u64; MAX_OUTCOMES],
    coordinates: QuantizedAtomSearchCoordinatesV1,
    maximum_subset_evaluations_per_support: u64,
}

impl ExactMarketCompilerRequestV1 {
    /// Construct a request from exact active prices and strictly increasing
    /// coordinates. Inactive fixed-capacity cells are canonical zeroes.
    pub fn new(
        market_id: ContentId,
        product_terms_id: ContentId,
        price_id: ContentId,
        active_prices: &[u64],
        active_coordinates: &[u128],
        maximum_subset_evaluations_per_support: u64,
    ) -> Result<Self, ExactMarketCompilerErrorV1> {
        if market_id.is_zero() || product_terms_id.is_zero() || price_id.is_zero() {
            return Err(ExactMarketCompilerErrorV1::InvalidIdentity);
        }
        if active_prices.is_empty() || active_prices.len() > MAX_OUTCOMES {
            return Err(ExactMarketCompilerErrorV1::InvalidPriceWidth);
        }
        let mut prices = [0_u64; MAX_OUTCOMES];
        prices[..active_prices.len()].copy_from_slice(active_prices);
        let price_count = u8::try_from(active_prices.len())
            .map_err(|_| ExactMarketCompilerErrorV1::InvalidPriceWidth)?;
        let mut coordinate_body = [0_u128; MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1];
        if active_coordinates.is_empty()
            || active_coordinates.len() > MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1
        {
            return Err(ExactMarketCompilerErrorV1::InvalidCoordinateWidth);
        }
        coordinate_body[..active_coordinates.len()].copy_from_slice(active_coordinates);
        let coordinate_count = u8::try_from(active_coordinates.len())
            .map_err(|_| ExactMarketCompilerErrorV1::InvalidCoordinateWidth)?;
        let coordinates = QuantizedAtomSearchCoordinatesV1::new(
            coordinate_count,
            coordinate_body,
        )?;
        QuantizedAtomAllSupportSolverPlanV1::new(
            maximum_subset_evaluations_per_support,
        )?;
        Ok(Self {
            market_id,
            product_terms_id,
            price_id,
            price_count,
            prices,
            coordinates,
            maximum_subset_evaluations_per_support,
        })
    }

    /// Repeated untrusted Market identity placed in a successful certificate.
    pub const fn market_id(&self) -> ContentId {
        self.market_id
    }

    /// Complete Product Terms identity checked against compiler evidence.
    pub const fn product_terms_id(&self) -> ContentId {
        self.product_terms_id
    }

    /// Repeated untrusted exact-price identity.
    pub const fn price_id(&self) -> ContentId {
        self.price_id
    }

    /// Exact active price width supplied by the caller.
    pub const fn price_count(&self) -> u8 {
        self.price_count
    }

    /// Exact active prices followed by canonical zero padding.
    pub const fn prices(&self) -> &[u64; MAX_OUTCOMES] {
        &self.prices
    }

    /// Canonical finite coordinate declaration.
    pub const fn coordinates(&self) -> QuantizedAtomSearchCoordinatesV1 {
        self.coordinates
    }

    /// Uniform deterministic subset budget for every support of size at least
    /// two.
    pub const fn maximum_subset_evaluations_per_support(&self) -> u64 {
        self.maximum_subset_evaluations_per_support
    }
}

/// Canonical deterministic provenance for one bounded exact inverse search.
///
/// This is compiler evidence, not a Product semantic owner. In particular, an
/// `Unsupported` outcome is a complete finite-hull negative only when
/// [`Self::is_complete_full_domain_negative`] is true. No outcome claims a
/// unique price, fair value, or optimization result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactMarketWorkManifestV1 {
    outcome: ExactMarketSearchOutcomeV1,
    coverage: ExactMarketCoordinateCoverageV1,
    coordinate_count: u8,
    outcome_count: u8,
    basis_degree: u8,
    solution_support: u8,
    exhausted_through_support: u8,
    truncated_support: u8,
    maximum_subset_evaluations_per_support: u64,
    payout_denominator: u64,
    market_id: ContentId,
    product_terms_id: ContentId,
    native_claim_basis_id: ContentId,
    price_id: ContentId,
    coordinate_domain_min: u128,
    coordinate_domain_max: u128,
    prices: [u64; MAX_OUTCOMES],
    coordinates: [u128; MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1],
    evaluations_by_support: [u64; MAX_OUTCOMES],
    exact_but_unrepresentable_by_support: [u64; MAX_OUTCOMES],
    certificate_output_id: ContentId,
}

impl ExactMarketWorkManifestV1 {
    /// Terminal search classification.
    pub const fn outcome(&self) -> ExactMarketSearchOutcomeV1 {
        self.outcome
    }

    /// Explicit full-domain versus declared-subset qualification.
    pub const fn coverage(&self) -> ExactMarketCoordinateCoverageV1 {
        self.coverage
    }

    /// Number of declared integer coordinates.
    pub const fn coordinate_count(&self) -> u8 {
        self.coordinate_count
    }

    /// Active Product outcome width and affine support bound.
    pub const fn outcome_count(&self) -> u8 {
        self.outcome_count
    }

    /// Product smooth basis degree.
    pub const fn basis_degree(&self) -> u8 {
        self.basis_degree
    }

    /// Successful certificate support, or zero for non-solved outcomes.
    pub const fn solution_support(&self) -> u8 {
        self.solution_support
    }

    /// Largest fully exhausted support family.
    pub const fn exhausted_through_support(&self) -> u8 {
        self.exhausted_through_support
    }

    /// Budget-truncated support family, or zero.
    pub const fn truncated_support(&self) -> u8 {
        self.truncated_support
    }

    /// Uniform per-support subset work bound.
    pub const fn maximum_subset_evaluations_per_support(&self) -> u64 {
        self.maximum_subset_evaluations_per_support
    }

    /// Exact Product payout denominator and price scale.
    pub const fn payout_denominator(&self) -> u64 {
        self.payout_denominator
    }

    /// Repeated untrusted Market identity.
    pub const fn market_id(&self) -> ContentId {
        self.market_id
    }

    /// Product Terms identity used to verify the compiler output.
    pub const fn product_terms_id(&self) -> ContentId {
        self.product_terms_id
    }

    /// Compiler-derived Product Basis identity.
    pub const fn native_claim_basis_id(&self) -> ContentId {
        self.native_claim_basis_id
    }

    /// Repeated untrusted exact-price identity.
    pub const fn price_id(&self) -> ContentId {
        self.price_id
    }

    /// Inclusive complete Product Terms-domain minimum.
    pub const fn coordinate_domain_min(&self) -> u128 {
        self.coordinate_domain_min
    }

    /// Inclusive complete Product Terms-domain maximum.
    pub const fn coordinate_domain_max(&self) -> u128 {
        self.coordinate_domain_max
    }

    /// Exact active price vector followed by zero padding.
    pub const fn prices(&self) -> &[u64; MAX_OUTCOMES] {
        &self.prices
    }

    /// Exact coordinate declaration followed by zero padding.
    pub const fn coordinates(
        &self,
    ) -> &[u128; MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1] {
        &self.coordinates
    }

    /// Deterministic subsets visited at a support in `1..=outcome_count`.
    pub fn evaluations_for_support(&self, support: u8) -> Option<u64> {
        if support == 0 || support > self.outcome_count {
            None
        } else {
            Some(self.evaluations_by_support[usize::from(support - 1)])
        }
    }

    /// Exact positive solutions outside the `u64` mass profile at a support in
    /// `1..=outcome_count`.
    pub fn exact_but_unrepresentable_for_support(&self, support: u8) -> Option<u64> {
        if support == 0 || support > self.outcome_count {
            None
        } else {
            Some(self.exact_but_unrepresentable_by_support[usize::from(support - 1)])
        }
    }

    /// Domain-separated checksum of the exact certificate output, or zero
    /// when no certificate was emitted.
    pub const fn certificate_output_id(&self) -> ContentId {
        self.certificate_output_id
    }

    /// True only for an exhaustive negative over every integer coordinate in
    /// the complete Terms domain and every affine support size.
    pub const fn is_complete_full_domain_negative(&self) -> bool {
        matches!(self.outcome, ExactMarketSearchOutcomeV1::Unsupported)
            && matches!(
                self.coverage,
                ExactMarketCoordinateCoverageV1::FullIntegerDomain
            )
            && self.exhausted_through_support == self.outcome_count
            && self.truncated_support == 0
    }

    /// Encode the unique fixed-width manifest body.
    pub fn encode_into(
        &self,
        output: &mut [u8; EXACT_MARKET_WORK_MANIFEST_BYTES_V1],
    ) -> Result<(), ExactMarketManifestErrorV1> {
        self.validate()?;
        let mut cursor = 0_usize;
        put(output, &mut cursor, &EXACT_MARKET_WORK_MANIFEST_MAGIC_V1)?;
        put(
            output,
            &mut cursor,
            &EXACT_MARKET_WORK_MANIFEST_SCHEMA_V1.to_le_bytes(),
        )?;
        put(
            output,
            &mut cursor,
            &EXACT_MARKET_SOLVER_SEMANTICS_V1.to_le_bytes(),
        )?;
        put(
            output,
            &mut cursor,
            &[
                self.outcome.tag(),
                self.coverage.tag(),
                self.coordinate_count,
                self.outcome_count,
                self.basis_degree,
                self.solution_support,
                self.exhausted_through_support,
                self.truncated_support,
            ],
        )?;
        put(output, &mut cursor, &[0; 4])?;
        put(
            output,
            &mut cursor,
            &self.maximum_subset_evaluations_per_support.to_le_bytes(),
        )?;
        put(
            output,
            &mut cursor,
            &self.payout_denominator.to_le_bytes(),
        )?;
        for id in [
            self.market_id,
            self.product_terms_id,
            self.native_claim_basis_id,
            self.price_id,
        ] {
            put(output, &mut cursor, &id.bytes())?;
        }
        put(
            output,
            &mut cursor,
            &self.coordinate_domain_min.to_le_bytes(),
        )?;
        put(
            output,
            &mut cursor,
            &self.coordinate_domain_max.to_le_bytes(),
        )?;
        for price in self.prices {
            put(output, &mut cursor, &price.to_le_bytes())?;
        }
        for coordinate in self.coordinates {
            put(output, &mut cursor, &coordinate.to_le_bytes())?;
        }
        for evaluations in self.evaluations_by_support {
            put(output, &mut cursor, &evaluations.to_le_bytes())?;
        }
        for count in self.exact_but_unrepresentable_by_support {
            put(output, &mut cursor, &count.to_le_bytes())?;
        }
        put(
            output,
            &mut cursor,
            &self.certificate_output_id.bytes(),
        )?;
        if cursor != output.len() {
            return Err(ExactMarketManifestErrorV1::InternalInvariant);
        }
        Ok(())
    }

    /// Decode and fully validate one hostile exact-width manifest body.
    pub fn decode(input: &[u8]) -> Result<Self, ExactMarketManifestErrorV1> {
        if input.len() != EXACT_MARKET_WORK_MANIFEST_BYTES_V1 {
            return Err(ExactMarketManifestErrorV1::InvalidLength);
        }
        let mut reader = Reader::new(input);
        if reader.take::<8>()? != EXACT_MARKET_WORK_MANIFEST_MAGIC_V1
            || reader.u16()? != EXACT_MARKET_WORK_MANIFEST_SCHEMA_V1
            || reader.u16()? != EXACT_MARKET_SOLVER_SEMANTICS_V1
        {
            return Err(ExactMarketManifestErrorV1::InvalidDiscriminant);
        }
        let outcome = ExactMarketSearchOutcomeV1::from_tag(reader.u8()?)?;
        let coverage = ExactMarketCoordinateCoverageV1::from_tag(reader.u8()?)?;
        let coordinate_count = reader.u8()?;
        let outcome_count = reader.u8()?;
        let basis_degree = reader.u8()?;
        let solution_support = reader.u8()?;
        let exhausted_through_support = reader.u8()?;
        let truncated_support = reader.u8()?;
        if reader.take::<4>()? != [0; 4] {
            return Err(ExactMarketManifestErrorV1::NonCanonicalPadding);
        }
        let maximum_subset_evaluations_per_support = reader.u64()?;
        let payout_denominator = reader.u64()?;
        let market_id = ContentId::from_bytes(reader.take::<32>()?);
        let product_terms_id = ContentId::from_bytes(reader.take::<32>()?);
        let native_claim_basis_id = ContentId::from_bytes(reader.take::<32>()?);
        let price_id = ContentId::from_bytes(reader.take::<32>()?);
        let coordinate_domain_min = reader.u128()?;
        let coordinate_domain_max = reader.u128()?;
        let mut prices = [0_u64; MAX_OUTCOMES];
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            prices[index] = reader.u64()?;
            index += 1;
        }
        let mut coordinates = [0_u128; MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1];
        index = 0;
        while index < MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1 {
            coordinates[index] = reader.u128()?;
            index += 1;
        }
        let mut evaluations_by_support = [0_u64; MAX_OUTCOMES];
        index = 0;
        while index < MAX_OUTCOMES {
            evaluations_by_support[index] = reader.u64()?;
            index += 1;
        }
        let mut exact_but_unrepresentable_by_support = [0_u64; MAX_OUTCOMES];
        index = 0;
        while index < MAX_OUTCOMES {
            exact_but_unrepresentable_by_support[index] = reader.u64()?;
            index += 1;
        }
        let certificate_output_id = ContentId::from_bytes(reader.take::<32>()?);
        if !reader.done() {
            return Err(ExactMarketManifestErrorV1::InvalidLength);
        }
        let value = Self {
            outcome,
            coverage,
            coordinate_count,
            outcome_count,
            basis_degree,
            solution_support,
            exhausted_through_support,
            truncated_support,
            maximum_subset_evaluations_per_support,
            payout_denominator,
            market_id,
            product_terms_id,
            native_claim_basis_id,
            price_id,
            coordinate_domain_min,
            coordinate_domain_max,
            prices,
            coordinates,
            evaluations_by_support,
            exact_but_unrepresentable_by_support,
            certificate_output_id,
        };
        value.validate()?;
        let mut canonical = [0_u8; EXACT_MARKET_WORK_MANIFEST_BYTES_V1];
        value.encode_into(&mut canonical)?;
        if canonical.as_slice() != input {
            return Err(ExactMarketManifestErrorV1::NonCanonicalPadding);
        }
        Ok(value)
    }

    /// Domain-separated identity of the exact canonical manifest bytes.
    pub fn content_id(&self) -> Result<ContentId, ExactMarketManifestErrorV1> {
        let mut body = [0_u8; EXACT_MARKET_WORK_MANIFEST_BYTES_V1];
        self.encode_into(&mut body)?;
        Ok(domain_id(EXACT_MARKET_WORK_MANIFEST_DOMAIN_V1, &body))
    }

    fn validate(&self) -> Result<(), ExactMarketManifestErrorV1> {
        if self.market_id.is_zero()
            || self.product_terms_id.is_zero()
            || self.native_claim_basis_id.is_zero()
            || self.price_id.is_zero()
        {
            return Err(ExactMarketManifestErrorV1::InvalidIdentity);
        }
        if (self.basis_degree != 2 && self.basis_degree != 3)
            || self.outcome_count <= self.basis_degree
            || usize::from(self.outcome_count) > MAX_OUTCOMES
            || self.payout_denominator == 0
            || self.maximum_subset_evaluations_per_support == 0
            || self.coordinate_domain_min >= self.coordinate_domain_max
        {
            return Err(ExactMarketManifestErrorV1::InvalidShape);
        }
        let active_outcomes = usize::from(self.outcome_count);
        let mut price_sum = 0_u128;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            if index < active_outcomes {
                price_sum = price_sum
                    .checked_add(u128::from(self.prices[index]))
                    .ok_or(ExactMarketManifestErrorV1::ArithmeticOverflow)?;
            } else if self.prices[index] != 0 {
                return Err(ExactMarketManifestErrorV1::NonCanonicalPadding);
            }
            index += 1;
        }
        if price_sum != u128::from(self.payout_denominator) {
            return Err(ExactMarketManifestErrorV1::InvalidPriceSimplex);
        }
        validate_coordinates(self)?;
        validate_report(self)
    }
}

/// Canonical output of the authority-neutral exact market compiler join.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledExactMarketV1 {
    /// Exact deterministic work and coverage facts.
    pub manifest: ExactMarketWorkManifestV1,
    /// Unique exact bytes of `manifest`.
    pub manifest_bytes: [u8; EXACT_MARKET_WORK_MANIFEST_BYTES_V1],
    /// Domain-separated manifest content identity.
    pub manifest_id: ContentId,
    /// Exact production verifier body for a solved result; absent otherwise.
    pub certificate_bytes: Option<[u8; EXACT_MARKET_CERTIFICATE_BYTES_V1]>,
}

impl CompiledExactMarketV1 {
    /// Recompute all manifest bytes/IDs and pass any emitted certificate
    /// through the bounded hostile decoder and production verifier.
    pub fn verify(
        &self,
        compiled_payoff: &CompiledProductionPayoffV1,
    ) -> Result<(), ExactMarketCompilerErrorV1> {
        self.manifest.validate()?;
        compiled_payoff.verify(self.manifest.product_terms_id)?;
        let basis = compiled_payoff
            .smooth_basis
            .ok_or(ExactMarketCompilerErrorV1::UnsupportedBasisProfile)?;
        if compiled_payoff.native_claim_basis_id.content_id()
            != self.manifest.native_claim_basis_id
            || compiled_payoff.coordinate_domain_min != self.manifest.coordinate_domain_min
            || compiled_payoff.coordinate_domain_max != self.manifest.coordinate_domain_max
            || basis.degree != self.manifest.basis_degree
            || basis.outcome_count != self.manifest.outcome_count
            || basis.denominator != self.manifest.payout_denominator
        {
            return Err(ExactMarketCompilerErrorV1::OutputMismatch);
        }
        let mut expected_manifest = [0_u8; EXACT_MARKET_WORK_MANIFEST_BYTES_V1];
        self.manifest.encode_into(&mut expected_manifest)?;
        if expected_manifest != self.manifest_bytes
            || self.manifest.content_id()? != self.manifest_id
        {
            return Err(ExactMarketCompilerErrorV1::OutputMismatch);
        }
        let bound = bound_from_manifest(&self.manifest, basis);
        let prices = prices_from_manifest(&self.manifest);
        match self.certificate_bytes {
            Some(bytes) => {
                if self.manifest.outcome != ExactMarketSearchOutcomeV1::Solved
                    || certificate_output_id(&bytes) != self.manifest.certificate_output_id
                {
                    return Err(ExactMarketCompilerErrorV1::OutputMismatch);
                }
                let certificate = QuantizedAtomMixtureCertificateV1::decode(&bytes)
                    .map_err(QuantizedAtomSolverErrorV1::from)?;
                let verified = verify_quantized_atom_mixture_v1(&bound, &prices, &certificate)
                    .map_err(QuantizedAtomSolverErrorV1::from)?;
                if verified.witness_count() != self.manifest.solution_support {
                    return Err(ExactMarketCompilerErrorV1::OutputMismatch);
                }
            }
            None => {
                if self.manifest.outcome == ExactMarketSearchOutcomeV1::Solved
                    || !self.manifest.certificate_output_id.is_zero()
                {
                    return Err(ExactMarketCompilerErrorV1::OutputMismatch);
                }
            }
        }
        Ok(())
    }
}

/// Canonical compiler sidecar joining one exact search to the current Product
/// BundleV5 artifact coordinate.
///
/// The all-zero artifact context is the exact globally content-addressed
/// Product artifact context accepted by the uploader. The adapter derives the
/// final PDA from its authenticated program ID, kind 60, and `bundle_v5_id`;
/// neither this sidecar nor the offline compiler authenticates that PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactMarketBundleSidecarV1 {
    bundle_artifact_context: ContentId,
    bundle_v5_id: CompiledProductSeriesBundleV5Id,
    work_manifest_id: ContentId,
    certificate_output_id: ContentId,
    market_id: ContentId,
}

impl ExactMarketBundleSidecarV1 {
    /// Current immutable artifact kind; exactly 60 / BundleV5.
    pub const fn bundle_artifact_kind(&self) -> u8 {
        COMPILED_PRODUCT_SERIES_BUNDLE_V5_ARTIFACT_KIND
    }

    /// Exact globally content-addressed artifact context; always all-zero.
    pub const fn bundle_artifact_context(&self) -> ContentId {
        self.bundle_artifact_context
    }

    /// Typed identity of the complete current Product BundleV5 graph.
    pub const fn bundle_v5_id(&self) -> CompiledProductSeriesBundleV5Id {
        self.bundle_v5_id
    }

    /// Identity of the exact search work/coverage manifest.
    pub const fn work_manifest_id(&self) -> ContentId {
        self.work_manifest_id
    }

    /// Certificate transport checksum, or zero for a non-solved search.
    pub const fn certificate_output_id(&self) -> ContentId {
        self.certificate_output_id
    }

    /// Market identity repeated by the exact certificate and manifest.
    pub const fn market_id(&self) -> ContentId {
        self.market_id
    }

    /// Encode the unique fixed-width sidecar body.
    pub fn encode_into(
        &self,
        output: &mut [u8; EXACT_MARKET_BUNDLE_SIDECAR_BYTES_V1],
    ) -> Result<(), ExactMarketManifestErrorV1> {
        self.validate()?;
        let mut cursor = 0_usize;
        put(output, &mut cursor, &EXACT_MARKET_BUNDLE_SIDECAR_MAGIC_V1)?;
        put(
            output,
            &mut cursor,
            &EXACT_MARKET_BUNDLE_SIDECAR_SCHEMA_V1.to_le_bytes(),
        )?;
        put(
            output,
            &mut cursor,
            &EXACT_MARKET_SOLVER_SEMANTICS_V1.to_le_bytes(),
        )?;
        put(
            output,
            &mut cursor,
            &[COMPILED_PRODUCT_SERIES_BUNDLE_V5_ARTIFACT_KIND],
        )?;
        put(output, &mut cursor, &[0; 3])?;
        for id in [
            self.bundle_artifact_context,
            self.bundle_v5_id.content_id(),
            self.work_manifest_id,
            self.certificate_output_id,
            self.market_id,
        ] {
            put(output, &mut cursor, &id.bytes())?;
        }
        if cursor != output.len() {
            return Err(ExactMarketManifestErrorV1::InternalInvariant);
        }
        Ok(())
    }

    /// Decode and structurally validate one hostile exact-width sidecar.
    pub fn decode(input: &[u8]) -> Result<Self, ExactMarketManifestErrorV1> {
        if input.len() != EXACT_MARKET_BUNDLE_SIDECAR_BYTES_V1 {
            return Err(ExactMarketManifestErrorV1::InvalidLength);
        }
        let mut reader = Reader::new(input);
        if reader.take::<8>()? != EXACT_MARKET_BUNDLE_SIDECAR_MAGIC_V1
            || reader.u16()? != EXACT_MARKET_BUNDLE_SIDECAR_SCHEMA_V1
            || reader.u16()? != EXACT_MARKET_SOLVER_SEMANTICS_V1
            || reader.u8()? != COMPILED_PRODUCT_SERIES_BUNDLE_V5_ARTIFACT_KIND
        {
            return Err(ExactMarketManifestErrorV1::InvalidDiscriminant);
        }
        if reader.take::<3>()? != [0; 3] {
            return Err(ExactMarketManifestErrorV1::NonCanonicalPadding);
        }
        let value = Self {
            bundle_artifact_context: ContentId::from_bytes(reader.take::<32>()?),
            bundle_v5_id: CompiledProductSeriesBundleV5Id::from_bytes(reader.take::<32>()?),
            work_manifest_id: ContentId::from_bytes(reader.take::<32>()?),
            certificate_output_id: ContentId::from_bytes(reader.take::<32>()?),
            market_id: ContentId::from_bytes(reader.take::<32>()?),
        };
        if !reader.done() {
            return Err(ExactMarketManifestErrorV1::InvalidLength);
        }
        value.validate()?;
        let mut canonical = [0_u8; EXACT_MARKET_BUNDLE_SIDECAR_BYTES_V1];
        value.encode_into(&mut canonical)?;
        if canonical.as_slice() != input {
            return Err(ExactMarketManifestErrorV1::NonCanonicalPadding);
        }
        Ok(value)
    }

    /// Domain-separated identity of the exact sidecar bytes.
    pub fn content_id(&self) -> Result<ContentId, ExactMarketManifestErrorV1> {
        let mut body = [0_u8; EXACT_MARKET_BUNDLE_SIDECAR_BYTES_V1];
        self.encode_into(&mut body)?;
        Ok(domain_id(EXACT_MARKET_BUNDLE_SIDECAR_DOMAIN_V1, &body))
    }

    /// Reopen the complete Product compiler output, exact search output, and
    /// every link stored by this sidecar.
    pub fn verify(
        &self,
        compiled_payoff: &CompiledProductionPayoffV1,
        bundle: &CompiledProductSeriesBundleV5,
        exact_market: &CompiledExactMarketV1,
    ) -> Result<(), ExactMarketCompilerErrorV1> {
        self.validate()?;
        exact_market.verify(compiled_payoff)?;
        let expected_bundle_id = bundle.id()?;
        if bundle.native_claim_basis_id != compiled_payoff.native_claim_basis_id
            || bundle.native_claim_basis_id.content_id()
                != exact_market.manifest.native_claim_basis_id
            || bundle.market_genesis_profile_id.content_id()
                != exact_market.manifest.product_terms_id
            || self.bundle_v5_id != expected_bundle_id
            || self.work_manifest_id != exact_market.manifest_id
            || self.certificate_output_id
                != exact_market.manifest.certificate_output_id
            || self.market_id != exact_market.manifest.market_id
        {
            return Err(ExactMarketCompilerErrorV1::OutputMismatch);
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), ExactMarketManifestErrorV1> {
        if !self.bundle_artifact_context.is_zero() {
            return Err(ExactMarketManifestErrorV1::NonCanonicalPadding);
        }
        if self.bundle_v5_id.content_id().is_zero()
            || self.work_manifest_id.is_zero()
            || self.market_id.is_zero()
        {
            return Err(ExactMarketManifestErrorV1::InvalidIdentity);
        }
        Ok(())
    }
}

/// Bind one exact search output to the sole current Product compiler graph.
pub fn bind_exact_market_bundle_v5(
    compiled_payoff: &CompiledProductionPayoffV1,
    bundle: &CompiledProductSeriesBundleV5,
    exact_market: &CompiledExactMarketV1,
) -> Result<ExactMarketBundleSidecarV1, ExactMarketCompilerErrorV1> {
    exact_market.verify(compiled_payoff)?;
    if bundle.native_claim_basis_id != compiled_payoff.native_claim_basis_id
        || bundle.native_claim_basis_id.content_id()
            != exact_market.manifest.native_claim_basis_id
        || bundle.market_genesis_profile_id.content_id()
            != exact_market.manifest.product_terms_id
    {
        return Err(ExactMarketCompilerErrorV1::OutputMismatch);
    }
    let value = ExactMarketBundleSidecarV1 {
        bundle_artifact_context: ContentId::ZERO,
        bundle_v5_id: bundle.id()?,
        work_manifest_id: exact_market.manifest_id,
        certificate_output_id: exact_market.manifest.certificate_output_id,
        market_id: exact_market.manifest.market_id,
    };
    value.verify(compiled_payoff, bundle, exact_market)?;
    Ok(value)
}

/// Deterministic refusal from the offline Product-to-price compiler join.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactMarketCompilerErrorV1 {
    /// A repeated Market, Terms, or price identity was zero.
    InvalidIdentity,
    /// The active exact price vector was empty or wider than Product.
    InvalidPriceWidth,
    /// The declared coordinate set was empty or wider than the solver profile.
    InvalidCoordinateWidth,
    /// The Product output was categorical, degree one, or otherwise outside
    /// the exact V1 positive-mixture profile.
    UnsupportedBasisProfile,
    /// Product compilation evidence or projection refused.
    Production(ProductionCompilerError),
    /// Exact fixed-capacity solver input or arithmetic refused.
    Solver(QuantizedAtomSolverErrorV1),
    /// Canonical work-manifest construction or decoding refused.
    Manifest(ExactMarketManifestErrorV1),
    /// Current Product BundleV5 construction or identity refused.
    Product(ProductError),
    /// Recomputed output disagreed with stored bytes or IDs.
    OutputMismatch,
}

impl From<ProductionCompilerError> for ExactMarketCompilerErrorV1 {
    fn from(value: ProductionCompilerError) -> Self {
        Self::Production(value)
    }
}

impl From<QuantizedAtomSolverErrorV1> for ExactMarketCompilerErrorV1 {
    fn from(value: QuantizedAtomSolverErrorV1) -> Self {
        Self::Solver(value)
    }
}

impl From<ExactMarketManifestErrorV1> for ExactMarketCompilerErrorV1 {
    fn from(value: ExactMarketManifestErrorV1) -> Self {
        Self::Manifest(value)
    }
}

impl From<ProductError> for ExactMarketCompilerErrorV1 {
    fn from(value: ProductError) -> Self {
        Self::Product(value)
    }
}

/// Hostile manifest refusal set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactMarketManifestErrorV1 {
    /// Body length differed from the frozen exact width.
    InvalidLength,
    /// Magic, schema, solver semantics, outcome, or coverage tag differed.
    InvalidDiscriminant,
    /// A repeated live identity was zero.
    InvalidIdentity,
    /// Basis shape, domain, denominator, or work bound was invalid.
    InvalidShape,
    /// Active prices did not sum exactly to the Product payout denominator.
    InvalidPriceSimplex,
    /// Coordinates were not a canonical in-domain strictly increasing prefix.
    InvalidCoordinates,
    /// The declared coverage tag disagreed with the exact coordinate set.
    CoverageMismatch,
    /// Search counters, terminal support, or certificate presence disagreed.
    InvalidReport,
    /// Reserved or inactive fixed-capacity cells were nonzero.
    NonCanonicalPadding,
    /// Checked counter or byte arithmetic overflowed.
    ArithmeticOverflow,
    /// A supposedly unreachable fixed-width invariant differed.
    InternalInvariant,
}

/// Compile one exact payout-denominator-scale price against a previously
/// compiled canonical Product basis.
///
/// The function searches every support size through `outcome_count`, within
/// the exact caller-declared coordinates and per-support work bound. A solved
/// certificate is passed through the same bounded decoder/verifier available
/// to an onchain adapter before it is returned. Negative and truncated results
/// emit only the work manifest.
pub fn compile_exact_market_v1(
    compiled_payoff: &CompiledProductionPayoffV1,
    request: ExactMarketCompilerRequestV1,
) -> Result<CompiledExactMarketV1, ExactMarketCompilerErrorV1> {
    compiled_payoff.verify(request.product_terms_id)?;
    let basis = compiled_payoff
        .smooth_basis
        .ok_or(ExactMarketCompilerErrorV1::UnsupportedBasisProfile)?;
    if basis.degree != 2 && basis.degree != 3 {
        return Err(ExactMarketCompilerErrorV1::UnsupportedBasisProfile);
    }
    if usize::from(basis.outcome_count) > MAX_OUTCOMES
        || request.price_count != basis.outcome_count
        || request.prices[usize::from(basis.outcome_count)..]
            .iter()
            .any(|price| *price != 0)
    {
        return Err(ExactMarketCompilerErrorV1::InvalidPriceWidth);
    }
    let bindings = QuantizedAtomMixtureBindingsV1 {
        market_id: request.market_id.bytes(),
        terms_id: request.product_terms_id.bytes(),
        basis_id: compiled_payoff.native_claim_basis_id.bytes(),
        price_id: request.price_id.bytes(),
    };
    let bound = BoundQuantizedSplineV1 {
        bindings,
        coordinate_domain_min: compiled_payoff.coordinate_domain_min,
        coordinate_domain_max: compiled_payoff.coordinate_domain_max,
        basis,
    };
    let prices = QuantizedPayoutPriceVectorV1 {
        price_id: request.price_id.bytes(),
        outcome_count: basis.outcome_count,
        prices: request.prices,
    };
    let plan = QuantizedAtomAllSupportSolverPlanV1::new(
        request.maximum_subset_evaluations_per_support,
    )?;
    let solver_outcome = solve_quantized_atom_hull_v1(
        &bound,
        &prices,
        request.coordinates,
        plan,
    )?;
    let (outcome, report, certificate_bytes, solution_support) = match solver_outcome {
        QuantizedAtomAllSupportSolverOutcomeV1::Solved(solution) => {
            let mut bytes = [0_u8; EXACT_MARKET_CERTIFICATE_BYTES_V1];
            solution
                .certificate()
                .encode_into(&mut bytes)
                .map_err(QuantizedAtomSolverErrorV1::from)?;
            let decoded = QuantizedAtomMixtureCertificateV1::decode(&bytes)
                .map_err(QuantizedAtomSolverErrorV1::from)?;
            let verified = verify_quantized_atom_mixture_v1(&bound, &prices, &decoded)
                .map_err(QuantizedAtomSolverErrorV1::from)?;
            (
                ExactMarketSearchOutcomeV1::Solved,
                solution.report(),
                Some(bytes),
                verified.witness_count(),
            )
        }
        QuantizedAtomAllSupportSolverOutcomeV1::Unsupported(report) => (
            ExactMarketSearchOutcomeV1::Unsupported,
            report,
            None,
            0,
        ),
        QuantizedAtomAllSupportSolverOutcomeV1::OutOfProfile(report) => (
            ExactMarketSearchOutcomeV1::OutOfProfile,
            report,
            None,
            0,
        ),
        QuantizedAtomAllSupportSolverOutcomeV1::WorkTruncated(report) => (
            ExactMarketSearchOutcomeV1::WorkTruncated,
            report,
            None,
            0,
        ),
    };
    let coverage = if report.covers_full_integer_domain() {
        ExactMarketCoordinateCoverageV1::FullIntegerDomain
    } else {
        ExactMarketCoordinateCoverageV1::DeclaredCoordinateSubset
    };
    let mut evaluations_by_support = [0_u64; MAX_OUTCOMES];
    let mut exact_but_unrepresentable_by_support = [0_u64; MAX_OUTCOMES];
    let mut support = 1_u8;
    while support <= report.outcome_count() {
        let index = usize::from(support - 1);
        evaluations_by_support[index] = report
            .evaluations_for_support(support)
            .ok_or(ExactMarketCompilerErrorV1::OutputMismatch)?;
        exact_but_unrepresentable_by_support[index] = report
            .exact_but_unrepresentable_for_support(support)
            .ok_or(ExactMarketCompilerErrorV1::OutputMismatch)?;
        support = support
            .checked_add(1)
            .ok_or(ExactMarketCompilerErrorV1::OutputMismatch)?;
    }
    let certificate_output_id = certificate_bytes
        .as_ref()
        .map_or(ContentId::ZERO, |bytes| certificate_output_id(bytes));
    let manifest = ExactMarketWorkManifestV1 {
        outcome,
        coverage,
        coordinate_count: report.coordinate_count(),
        outcome_count: report.outcome_count(),
        basis_degree: basis.degree,
        solution_support,
        exhausted_through_support: report.exhausted_through_support(),
        truncated_support: report.truncated_support(),
        maximum_subset_evaluations_per_support: report
            .maximum_subset_evaluations_per_support(),
        payout_denominator: basis.denominator,
        market_id: request.market_id,
        product_terms_id: request.product_terms_id,
        native_claim_basis_id: compiled_payoff.native_claim_basis_id.content_id(),
        price_id: request.price_id,
        coordinate_domain_min: compiled_payoff.coordinate_domain_min,
        coordinate_domain_max: compiled_payoff.coordinate_domain_max,
        prices: request.prices,
        coordinates: *request.coordinates.coordinates(),
        evaluations_by_support,
        exact_but_unrepresentable_by_support,
        certificate_output_id,
    };
    let mut manifest_bytes = [0_u8; EXACT_MARKET_WORK_MANIFEST_BYTES_V1];
    manifest.encode_into(&mut manifest_bytes)?;
    if ExactMarketWorkManifestV1::decode(&manifest_bytes)? != manifest {
        return Err(ExactMarketCompilerErrorV1::OutputMismatch);
    }
    let manifest_id = manifest.content_id()?;
    let value = CompiledExactMarketV1 {
        manifest,
        manifest_bytes,
        manifest_id,
        certificate_bytes,
    };
    value.verify(compiled_payoff)?;
    Ok(value)
}

fn validate_coordinates(
    manifest: &ExactMarketWorkManifestV1,
) -> Result<(), ExactMarketManifestErrorV1> {
    let active = usize::from(manifest.coordinate_count);
    if active == 0 || active > MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1 {
        return Err(ExactMarketManifestErrorV1::InvalidCoordinates);
    }
    let mut index = 0_usize;
    while index < MAX_QUANTIZED_ATOM_SOLVER_COORDINATES_V1 {
        let coordinate = manifest.coordinates[index];
        if index < active {
            if coordinate < manifest.coordinate_domain_min
                || coordinate > manifest.coordinate_domain_max
                || (index != 0 && coordinate <= manifest.coordinates[index - 1])
            {
                return Err(ExactMarketManifestErrorV1::InvalidCoordinates);
            }
        } else if coordinate != 0 {
            return Err(ExactMarketManifestErrorV1::NonCanonicalPadding);
        }
        index += 1;
    }
    let full = is_full_integer_domain(manifest)?;
    if full != matches!(
        manifest.coverage,
        ExactMarketCoordinateCoverageV1::FullIntegerDomain
    ) {
        return Err(ExactMarketManifestErrorV1::CoverageMismatch);
    }
    Ok(())
}

fn is_full_integer_domain(
    manifest: &ExactMarketWorkManifestV1,
) -> Result<bool, ExactMarketManifestErrorV1> {
    let width = manifest
        .coordinate_domain_max
        .checked_sub(manifest.coordinate_domain_min)
        .and_then(|difference| difference.checked_add(1))
        .ok_or(ExactMarketManifestErrorV1::ArithmeticOverflow)?;
    if width != u128::from(manifest.coordinate_count) {
        return Ok(false);
    }
    let mut index = 0_usize;
    while index < usize::from(manifest.coordinate_count) {
        let offset = u128::try_from(index)
            .map_err(|_| ExactMarketManifestErrorV1::ArithmeticOverflow)?;
        let expected = manifest
            .coordinate_domain_min
            .checked_add(offset)
            .ok_or(ExactMarketManifestErrorV1::ArithmeticOverflow)?;
        if manifest.coordinates[index] != expected {
            return Ok(false);
        }
        index += 1;
    }
    Ok(true)
}

fn validate_report(
    manifest: &ExactMarketWorkManifestV1,
) -> Result<(), ExactMarketManifestErrorV1> {
    let active_outcomes = usize::from(manifest.outcome_count);
    let mut support = 1_usize;
    while support <= MAX_OUTCOMES {
        let evaluations = manifest.evaluations_by_support[support - 1];
        let unrepresentable = manifest.exact_but_unrepresentable_by_support[support - 1];
        if support > active_outcomes {
            if evaluations != 0 || unrepresentable != 0 {
                return Err(ExactMarketManifestErrorV1::NonCanonicalPadding);
            }
        } else if unrepresentable > evaluations || (support == 1 && unrepresentable != 0) {
            return Err(ExactMarketManifestErrorV1::InvalidReport);
        }
        support += 1;
    }
    match manifest.outcome {
        ExactMarketSearchOutcomeV1::Solved => {
            if manifest.solution_support == 0
                || manifest.solution_support > manifest.outcome_count
                || manifest.exhausted_through_support
                    != manifest.solution_support.saturating_sub(1)
                || manifest.truncated_support != 0
                || manifest.certificate_output_id.is_zero()
            {
                return Err(ExactMarketManifestErrorV1::InvalidReport);
            }
            validate_completed_supports(manifest, manifest.exhausted_through_support)?;
            let found_support = usize::from(manifest.solution_support);
            let visited = manifest.evaluations_by_support[found_support - 1];
            let family = combinations(
                usize::from(manifest.coordinate_count),
                found_support,
            )?;
            if visited == 0
                || visited > family
                || (found_support >= 2
                    && visited > manifest.maximum_subset_evaluations_per_support)
            {
                return Err(ExactMarketManifestErrorV1::InvalidReport);
            }
            validate_zero_support_suffix(manifest, found_support + 1)?;
        }
        ExactMarketSearchOutcomeV1::Unsupported
        | ExactMarketSearchOutcomeV1::OutOfProfile => {
            if manifest.solution_support != 0
                || manifest.exhausted_through_support != manifest.outcome_count
                || manifest.truncated_support != 0
                || !manifest.certificate_output_id.is_zero()
            {
                return Err(ExactMarketManifestErrorV1::InvalidReport);
            }
            validate_completed_supports(manifest, manifest.outcome_count)?;
            let mut saw_unrepresentable = false;
            support = 1;
            while support <= active_outcomes {
                saw_unrepresentable |=
                    manifest.exact_but_unrepresentable_by_support[support - 1] != 0;
                support += 1;
            }
            if saw_unrepresentable
                != matches!(manifest.outcome, ExactMarketSearchOutcomeV1::OutOfProfile)
            {
                return Err(ExactMarketManifestErrorV1::InvalidReport);
            }
        }
        ExactMarketSearchOutcomeV1::WorkTruncated => {
            if manifest.solution_support != 0
                || manifest.truncated_support < 2
                || manifest.truncated_support > manifest.outcome_count
                || manifest.exhausted_through_support
                    != manifest.truncated_support.saturating_sub(1)
                || !manifest.certificate_output_id.is_zero()
            {
                return Err(ExactMarketManifestErrorV1::InvalidReport);
            }
            validate_completed_supports(manifest, manifest.exhausted_through_support)?;
            let truncated = usize::from(manifest.truncated_support);
            let family = combinations(usize::from(manifest.coordinate_count), truncated)?;
            if manifest.evaluations_by_support[truncated - 1]
                != manifest.maximum_subset_evaluations_per_support
                || manifest.maximum_subset_evaluations_per_support >= family
            {
                return Err(ExactMarketManifestErrorV1::InvalidReport);
            }
            validate_zero_support_suffix(manifest, truncated + 1)?;
        }
    }
    Ok(())
}

fn validate_completed_supports(
    manifest: &ExactMarketWorkManifestV1,
    completed_through: u8,
) -> Result<(), ExactMarketManifestErrorV1> {
    let mut support = 1_usize;
    while support <= usize::from(completed_through) {
        let expected = combinations(usize::from(manifest.coordinate_count), support)?;
        if manifest.evaluations_by_support[support - 1] != expected {
            return Err(ExactMarketManifestErrorV1::InvalidReport);
        }
        support += 1;
    }
    Ok(())
}

fn validate_zero_support_suffix(
    manifest: &ExactMarketWorkManifestV1,
    first_zero_support: usize,
) -> Result<(), ExactMarketManifestErrorV1> {
    let mut support = first_zero_support;
    while support <= usize::from(manifest.outcome_count) {
        if manifest.evaluations_by_support[support - 1] != 0
            || manifest.exact_but_unrepresentable_by_support[support - 1] != 0
        {
            return Err(ExactMarketManifestErrorV1::InvalidReport);
        }
        support += 1;
    }
    Ok(())
}

fn combinations(n: usize, k: usize) -> Result<u64, ExactMarketManifestErrorV1> {
    if k > n {
        return Ok(0);
    }
    let reduced = core::cmp::min(k, n - k);
    let mut value = 1_u128;
    let mut index = 1_usize;
    while index <= reduced {
        let numerator = n
            .checked_sub(reduced)
            .and_then(|base| base.checked_add(index))
            .ok_or(ExactMarketManifestErrorV1::ArithmeticOverflow)?;
        value = value
            .checked_mul(
                u128::try_from(numerator)
                    .map_err(|_| ExactMarketManifestErrorV1::ArithmeticOverflow)?,
            )
            .ok_or(ExactMarketManifestErrorV1::ArithmeticOverflow)?;
        value /= u128::try_from(index)
            .map_err(|_| ExactMarketManifestErrorV1::ArithmeticOverflow)?;
        index += 1;
    }
    u64::try_from(value).map_err(|_| ExactMarketManifestErrorV1::ArithmeticOverflow)
}

fn bound_from_manifest(
    manifest: &ExactMarketWorkManifestV1,
    basis: clutch_bspline::BasisSpec,
) -> BoundQuantizedSplineV1 {
    BoundQuantizedSplineV1 {
        bindings: QuantizedAtomMixtureBindingsV1 {
            market_id: manifest.market_id.bytes(),
            terms_id: manifest.product_terms_id.bytes(),
            basis_id: manifest.native_claim_basis_id.bytes(),
            price_id: manifest.price_id.bytes(),
        },
        coordinate_domain_min: manifest.coordinate_domain_min,
        coordinate_domain_max: manifest.coordinate_domain_max,
        basis,
    }
}

fn prices_from_manifest(
    manifest: &ExactMarketWorkManifestV1,
) -> QuantizedPayoutPriceVectorV1 {
    QuantizedPayoutPriceVectorV1 {
        price_id: manifest.price_id.bytes(),
        outcome_count: manifest.outcome_count,
        prices: manifest.prices,
    }
}

fn certificate_output_id(
    bytes: &[u8; EXACT_MARKET_CERTIFICATE_BYTES_V1],
) -> ContentId {
    domain_id(EXACT_MARKET_CERTIFICATE_OUTPUT_DOMAIN_V1, bytes)
}

fn domain_id(domain: &[u8], body: &[u8]) -> ContentId {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(body);
    ContentId::from_bytes(hasher.finalize().into())
}

fn put(
    output: &mut [u8],
    cursor: &mut usize,
    bytes: &[u8],
) -> Result<(), ExactMarketManifestErrorV1> {
    let end = cursor
        .checked_add(bytes.len())
        .ok_or(ExactMarketManifestErrorV1::ArithmeticOverflow)?;
    let target = output
        .get_mut(*cursor..end)
        .ok_or(ExactMarketManifestErrorV1::InvalidLength)?;
    target.copy_from_slice(bytes);
    *cursor = end;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], ExactMarketManifestErrorV1> {
        let end = self
            .cursor
            .checked_add(N)
            .ok_or(ExactMarketManifestErrorV1::ArithmeticOverflow)?;
        let bytes = self
            .bytes
            .get(self.cursor..end)
            .ok_or(ExactMarketManifestErrorV1::InvalidLength)?;
        let value = <[u8; N]>::try_from(bytes)
            .map_err(|_| ExactMarketManifestErrorV1::InvalidLength)?;
        self.cursor = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, ExactMarketManifestErrorV1> {
        Ok(self.take::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ExactMarketManifestErrorV1> {
        Ok(u16::from_le_bytes(self.take::<2>()?))
    }

    fn u64(&mut self) -> Result<u64, ExactMarketManifestErrorV1> {
        Ok(u64::from_le_bytes(self.take::<8>()?))
    }

    fn u128(&mut self) -> Result<u128, ExactMarketManifestErrorV1> {
        Ok(u128::from_le_bytes(self.take::<16>()?))
    }

    const fn done(&self) -> bool {
        self.cursor == self.bytes.len()
    }
}
