// SPDX-License-Identifier: AGPL-3.0-or-later
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Executable pure-core join for a direct General V2 candidate.
//!
//! This crate closes the semantic gap between five existing owners without
//! becoming a second owner of any of them:
//!
//! - Product owns the immutable basis, price-measure policy, Genesis profile,
//!   and full-width market identity.
//! - The General V2 contract owns the sealed candidate-feed and rank codecs.
//! - The price-measure checker owns exact coherence against the quantized
//!   integer-coordinate payout map.
//! - RelationV2 owns owner-blind order admission, conservation, and the final
//!   economic candidate digest.
//! - ScoreV2-Q owns the quotient-risk comparison fields.
//! - This join owns only the canonical fixed policy identities selecting those
//!   already-owned semantics for General V2.
//!
//! [`verify_smooth_direct_candidate_v1`] retains its original API name but now
//! admits the complete Product-selected V3 quantized family: mapped degree
//! zero and smooth degrees one through three. It verifies one already-submitted
//! candidate and returns a canonical rank. It does not establish global
//! optimality, authorize settlement, authenticate a Solana account owner/PDA,
//! or move assets.
//!
//! There is exactly one payout rounding boundary: the Product-selected
//! largest-remainder/lowest-outcome-index quantizer executed by the immutable
//! basis evaluator. Candidate prices are already exact integer grid members;
//! the certificate and RelationV2 perform no further rounding.

use clutch_batch::relation_v2::{
    price_semantics_digest_v2, verify_economic_candidate_v2, EconomicBookV2, EconomicCandidateV2,
    EconomicDomainV2, EconomicErrorV2, PricePreconditionV2, VerifiedEconomicsV2,
};
use clutch_general_v2_contract::{
    candidate_bundle_digest_v1, candidate_feed_tail_v2, economic_domain_digest_v2,
    encode_score_v2_q_cost_first_admitted_tie_v1,
    encode_score_v2_q_first_admitted_tie_v1, quantized_witness_body_digest_v3,
    quantized_witness_parts_digest_v3, settlement_witness_digest_v1, AdmissionNodeStatusV1,
    AdmissionNodeV3AccountV1, CandidateFeedHeaderV2, CodecError, EconomicDomainV2AccountV1,
    FirstAdmittedTieV1, Id32, MarketBindingV1, ScoreV2QComponentsV1,
    ScoreV2QCostComponentsV1, SettlementCandidateKindV1,
    Sha256BackendV1, CANDIDATE_FEED_HEADER_BYTES, MAX_ORDERS, MAX_OUTCOMES, MAX_QUANTIZED_ATOMS,
    PRICE_MEASURE_WITNESS_SCHEMA_V3, QUANTIZED_ATOM_BYTES, QUANTIZED_PRICE_MEASURE_SEMANTICS_V1,
    QUANTIZED_WITNESS_BODY_V3_FIXED_BYTES, SCORE_V2_Q_RANK_CAPACITY,
};
use clutch_price_measure::{
    verify_quantized_atom_mixture_v1, verify_quantized_price_measure_v3_degree_zero,
    verify_quantized_price_measure_v3_smooth, AdapterBindingsV3, BoundQuantizedSplineV1,
    DegreeZeroPayoutTableV3, ErrorV1 as AtomMixtureErrorV1, ErrorV3, PriceVectorV3,
    QuantizedAtomMixtureBindingsV1, QuantizedAtomMixtureCertificateV1, QuantizedAtomWitnessV3,
    QuantizedPayoutPriceVectorV1, VerifiedPriceMeasureV3, VerifiedQuantizedAtomMixtureV1,
    PRICE_MEASURE_WITNESS_VERSION_V3, QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1,
};
use clutch_product_series::{
    Error as ProductError, MarketGenesisProfileV2, MarketInstancePreimageV2, NativeClaimBasisV1,
    PriceMeasurePolicyV1, ProductTemplateV4, QuantizedBasisSpecV1, QuantizedEdgePolicyV1,
};
use clutch_solana_layout::{CodecError as LayoutError, PriceGridAccount};
use sha2::{Digest, Sha256};

mod builder;
mod candidate_cost;
mod settlement;
mod settlement_root_projection;
mod work;

pub use builder::*;
pub use candidate_cost::*;
pub use settlement::*;
pub use settlement_root_projection::*;
pub use work::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QuantizedBasisProjectionV1 {
    DegreeZero(DegreeZeroPayoutTableV3),
    Smooth(QuantizedBasisSpecV1),
}

/// SHA-256 domain for General V2's canonical owner-blind RelationV2 policy.
pub const RELATION_V2_POLICY_DIGEST_DOMAIN_V1: &[u8] = b"dragons-clutch/relation-v2-policy/v1\0";
/// SHA-256 domain for General V2's canonical ScoreV2-Q selection policy.
pub const SCORE_V2_Q_POLICY_DIGEST_DOMAIN_V1: &[u8] = b"dragons-clutch/score-v2-q-policy/v1\0";
/// SHA-256 domain for the owner-net cost-aware ScoreV2-Q successor.
pub const SCORE_V2_Q_COST_POLICY_DIGEST_DOMAIN_V1: &[u8] =
    b"dragons-clutch/score-v2-q-cost-policy/v1\0";

/// Exact canonical RelationV2 policy body.
///
/// The bytes freeze, in order: magic, policy schema, relation version,
/// owner-blind normalization, maximum outcome/order widths, coefficient/fill/
/// limit integer widths, partial-fill semantics, canonical virtual conversion,
/// full-input candidate digest semantics, and zero reserved bytes.
pub const RELATION_V2_POLICY_BODY_V1: [u8; 24] = [
    b'D', b'C', b'R', b'E', b'L', b'V', b'2', 0, 1, 2, 0, 16, 64, 8, 8, 16, 1, 1, 1, 0, 0, 0, 0, 0,
];

/// Exact canonical ScoreV2-Q policy body.
///
/// The bytes freeze, in order: magic, policy schema, score arithmetic version,
/// owner-blind normalization, range objective, minimum direct complete-set
/// tie, minimum virtual-churn tie, smaller full candidate-digest tie, and
/// Window-owned first-admitted exact-duplicate tie.
pub const SCORE_V2_Q_POLICY_BODY_V1: [u8; 16] = [
    b'D', b'C', b'S', b'V', b'2', b'Q', b'1', 0, 1, 2, 0, 0, 0, 0, 0, 0,
];
/// Exact canonical owner-net cost-aware ScoreV2-Q successor body.
///
/// The final bytes commit to owner aggregation, signed payoff netting,
/// complete-set quotienting, exact state-price valuation, terminal-owner
/// ceiling, insertion after churn, and absence of fee/identity/optimality
/// claims.
pub const SCORE_V2_Q_COST_POLICY_BODY_V1: [u8; 24] = [
    b'D', b'C', b'S', b'V', b'2', b'Q', b'C', 0, 1, 2, 1, 1, 1, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0,
    0,
];

/// Maximum exact active-width bytes in the contract-owned V3 witness body.
pub const QUANTIZED_WITNESS_BODY_MAX_BYTES_V1: usize = QUANTIZED_WITNESS_BODY_V3_FIXED_BYTES
    + (MAX_OUTCOMES * 8)
    + (MAX_QUANTIZED_ATOMS * QUANTIZED_ATOM_BYTES);

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_ORDERS == 64);
const _: () = assert!(MAX_QUANTIZED_ATOMS == 16);
const _: () = assert!(QUANTIZED_WITNESS_BODY_MAX_BYTES_V1 == 661);
const _: () = assert!(RELATION_V2_POLICY_BODY_V1.len() == 24);
const _: () = assert!(SCORE_V2_Q_POLICY_BODY_V1.len() == 16);
const _: () = assert!(SCORE_V2_Q_COST_POLICY_BODY_V1.len() == 24);
const _: () = assert!(PRICE_MEASURE_WITNESS_SCHEMA_V3 == PRICE_MEASURE_WITNESS_VERSION_V3);
const _: () =
    assert!(QUANTIZED_PRICE_MEASURE_SEMANTICS_V1 == QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1);

/// Derive General V2's one canonical owner-blind RelationV2 policy identity.
pub fn relation_v2_policy_id_v1() -> Result<Id32, GeneralV2RuntimeError> {
    Id32::new(hash_parts(&[
        RELATION_V2_POLICY_DIGEST_DOMAIN_V1,
        &RELATION_V2_POLICY_BODY_V1,
    ]))
    .map_err(GeneralV2RuntimeError::Contract)
}

/// Derive General V2's one canonical ScoreV2-Q plus first-admitted policy ID.
pub fn score_v2_q_policy_id_v1() -> Result<Id32, GeneralV2RuntimeError> {
    Id32::new(hash_parts(&[
        SCORE_V2_Q_POLICY_DIGEST_DOMAIN_V1,
        &SCORE_V2_Q_POLICY_BODY_V1,
    ]))
    .map_err(GeneralV2RuntimeError::Contract)
}

/// Derive the breaking owner-net cost-aware ScoreV2-Q policy identity.
pub fn score_v2_q_cost_policy_id_v1() -> Result<Id32, GeneralV2RuntimeError> {
    Id32::new(hash_parts(&[
        SCORE_V2_Q_COST_POLICY_DIGEST_DOMAIN_V1,
        &SCORE_V2_Q_COST_POLICY_BODY_V1,
    ]))
    .map_err(GeneralV2RuntimeError::Contract)
}

/// The relation-relevant projection of one sealed General V2 candidate feed.
///
/// Settlement slices remain in the sealed account and are validated by the
/// General V2 feed codec. They are deliberately not duplicated here because
/// they do not define price coherence, RelationV2 economics, or ScoreV2-Q.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateFeedEconomicsV1 {
    /// Active exact prices followed by canonical zero padding.
    pub prices: [u64; MAX_OUTCOMES],
    /// Active order fills followed by canonical zero padding.
    pub fills: [u64; MAX_ORDERS],
    /// Active sorted atom coordinates followed by canonical zero padding.
    pub atom_coordinates: [u128; MAX_QUANTIZED_ATOMS],
    /// Active positive atom masses followed by canonical zero padding.
    pub atom_masses: [u64; MAX_QUANTIZED_ATOMS],
}

/// Typed canonical quantized price-measure witness body before hashing.
///
/// The contract owns its active-width transcript. This runtime retains typed
/// fixed-capacity fields while the exact serialization remains the price and
/// atom tails of `CandidateFeedV2`. Its digest authenticates one nonunique
/// coherence certificate but never enters RelationV2's economic candidate
/// digest or ScoreV2-Q rank.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedWitnessBodyV1 {
    /// Exact witness schema, currently V3.
    pub schema_version: u8,
    /// Exact quantized evaluator/rounding semantics, currently V1.
    pub quantized_semantics_version: u8,
    /// Authenticated sealed candidate-feed identity.
    pub candidate_feed: Id32,
    /// Canonical EconomicDomainV2 artifact digest.
    pub economic_domain_digest: Id32,
    /// Canonical NativeClaimBasisV1 body identity.
    pub basis_digest: Id32,
    /// Canonical exact candidate-price identity.
    pub candidate_price_digest: Id32,
    /// Exact integer simplex scale.
    pub price_scale: u64,
    /// Quantized basis degree, zero through three.
    pub basis_degree: u8,
    /// Active native outcome width.
    pub outcome_count: u8,
    /// Active atom support width.
    pub atom_count: u8,
    /// Primitive positive atom-mass denominator.
    pub common_denominator: u64,
    /// Active exact prices followed by canonical zero padding.
    pub prices: [u64; MAX_OUTCOMES],
    /// Active sorted coordinates followed by canonical zero padding.
    pub atom_coordinates: [u128; MAX_QUANTIZED_ATOMS],
    /// Active positive masses followed by canonical zero padding.
    pub atom_masses: [u64; MAX_QUANTIZED_ATOMS],
}

impl QuantizedWitnessBodyV1 {
    /// Validate version, quantized basis shape, support order, primitive mass, and
    /// canonical padding before encoding or hashing.
    pub fn validate(self) -> Result<(), GeneralV2RuntimeError> {
        if self.schema_version != PRICE_MEASURE_WITNESS_VERSION_V3
            || self.quantized_semantics_version != QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1
        {
            return Err(GeneralV2RuntimeError::UnsupportedWitnessVersion);
        }
        if self.basis_degree > 3
            || !(2..=16).contains(&self.outcome_count)
            || self.outcome_count <= self.basis_degree
            || self.atom_count == 0
            || self.atom_count > self.outcome_count
            || usize::from(self.atom_count) > MAX_QUANTIZED_ATOMS
            || self.price_scale == 0
            || self.common_denominator == 0
        {
            return Err(GeneralV2RuntimeError::InvalidWitnessShape);
        }
        if self.candidate_feed.is_zero()
            || self.economic_domain_digest.is_zero()
            || self.basis_digest.is_zero()
            || self.candidate_price_digest.is_zero()
        {
            return Err(GeneralV2RuntimeError::BindingMismatch);
        }
        let mut price_sum = 0u64;
        let mut outcome = 0usize;
        while outcome < MAX_OUTCOMES {
            let price = self.prices[outcome];
            if outcome < usize::from(self.outcome_count) {
                price_sum = price_sum
                    .checked_add(price)
                    .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?;
            } else if price != 0 {
                return Err(GeneralV2RuntimeError::NonCanonicalWitnessPadding);
            }
            outcome += 1;
        }
        if price_sum != self.price_scale {
            return Err(GeneralV2RuntimeError::InvalidWitnessShape);
        }
        let mut prior = 0u128;
        let mut mass_sum = 0u64;
        let mut divisor = self.common_denominator;
        let mut atom = 0usize;
        while atom < MAX_QUANTIZED_ATOMS {
            let coordinate = self.atom_coordinates[atom];
            let mass = self.atom_masses[atom];
            if atom < usize::from(self.atom_count) {
                if mass == 0 || (atom != 0 && coordinate <= prior) {
                    return Err(GeneralV2RuntimeError::InvalidWitnessShape);
                }
                prior = coordinate;
                mass_sum = mass_sum
                    .checked_add(mass)
                    .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?;
                divisor = gcd(divisor, mass);
            } else if coordinate != 0 || mass != 0 {
                return Err(GeneralV2RuntimeError::NonCanonicalWitnessPadding);
            }
            atom += 1;
        }
        if mass_sum != self.common_denominator || divisor != 1 {
            return Err(GeneralV2RuntimeError::InvalidWitnessShape);
        }
        Ok(())
    }

    /// Return the exact active-width contract transcript length.
    pub fn encoded_len(self) -> Result<usize, GeneralV2RuntimeError> {
        self.validate()?;
        QUANTIZED_WITNESS_BODY_V3_FIXED_BYTES
            .checked_add(
                usize::from(self.outcome_count)
                    .checked_mul(8)
                    .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?,
            )
            .and_then(|value| {
                usize::from(self.atom_count)
                    .checked_mul(QUANTIZED_ATOM_BYTES)
                    .and_then(|atom_bytes| value.checked_add(atom_bytes))
            })
            .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)
    }

    /// Encode the exact active-width contract transcript into caller storage.
    pub fn encode(self, output: &mut [u8]) -> Result<(), GeneralV2RuntimeError> {
        let expected = self.encoded_len()?;
        if output.len() != expected {
            return Err(GeneralV2RuntimeError::InvalidWitnessShape);
        }
        let mut cursor = 0usize;
        for id in [
            self.candidate_feed,
            self.economic_domain_digest,
            self.basis_digest,
            self.candidate_price_digest,
        ] {
            put(output, &mut cursor, &id.bytes())?;
        }
        put(output, &mut cursor, &self.price_scale.to_le_bytes())?;
        put(output, &mut cursor, &self.common_denominator.to_le_bytes())?;
        put(output, &mut cursor, &[self.schema_version])?;
        put(output, &mut cursor, &[self.quantized_semantics_version])?;
        put(output, &mut cursor, &[self.basis_degree])?;
        put(output, &mut cursor, &[self.outcome_count])?;
        put(output, &mut cursor, &[self.atom_count])?;
        let mut outcome = 0usize;
        while outcome < usize::from(self.outcome_count) {
            put(output, &mut cursor, &self.prices[outcome].to_le_bytes())?;
            outcome += 1;
        }
        let mut atom = 0usize;
        while atom < usize::from(self.atom_count) {
            put(
                output,
                &mut cursor,
                &self.atom_coordinates[atom].to_le_bytes(),
            )?;
            put(output, &mut cursor, &self.atom_masses[atom].to_le_bytes())?;
            atom += 1;
        }
        if cursor != expected {
            return Err(GeneralV2RuntimeError::ArithmeticOverflow);
        }
        Ok(())
    }

    /// Derive the contract-owned canonical SHA-256 body identity.
    pub fn digest(self) -> Result<Id32, GeneralV2RuntimeError> {
        self.validate()?;
        let mut prices_le = [0u8; MAX_OUTCOMES * 8];
        let mut atoms_le = [0u8; MAX_QUANTIZED_ATOMS * QUANTIZED_ATOM_BYTES];
        let mut outcome = 0usize;
        while outcome < usize::from(self.outcome_count) {
            let at = outcome * 8;
            prices_le[at..at + 8].copy_from_slice(&self.prices[outcome].to_le_bytes());
            outcome += 1;
        }
        let mut atom = 0usize;
        while atom < usize::from(self.atom_count) {
            let at = atom * QUANTIZED_ATOM_BYTES;
            atoms_le[at..at + 16].copy_from_slice(&self.atom_coordinates[atom].to_le_bytes());
            atoms_le[at + 16..at + QUANTIZED_ATOM_BYTES]
                .copy_from_slice(&self.atom_masses[atom].to_le_bytes());
            atom += 1;
        }
        quantized_witness_parts_digest_v3(
            &CanonicalSha256,
            self.candidate_feed,
            self.economic_domain_digest,
            self.basis_digest,
            self.candidate_price_digest,
            self.price_scale,
            self.common_denominator,
            self.schema_version,
            self.quantized_semantics_version,
            self.basis_degree,
            self.outcome_count,
            self.atom_count,
            &prices_le[..usize::from(self.outcome_count) * 8],
            &atoms_le[..usize::from(self.atom_count) * QUANTIZED_ATOM_BYTES],
        )
        .map_err(GeneralV2RuntimeError::Contract)
    }
}

/// Successful executable composition of price coherence, RelationV2, and
/// ScoreV2-Q ranking.
///
/// This value is an in-memory pure-core result. It is not a persisted verdict,
/// settlement authority, proof of global optimality, or release evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedSmoothDirectCandidateV1 {
    /// Canonical EconomicDomainV2 artifact identity.
    economic_domain_digest: Id32,
    /// Recomputed exact runtime price-coherence summary.
    price_measure: VerifiedPriceMeasureV3,
    /// Stronger payout-denominator-scale certificate for degrees two and three.
    ///
    /// Degree zero and one use their exact V3 finite/mapped checker and are
    /// outside the deliberately narrow atom-mixture V1 theorem.
    quantized_atom_mixture: Option<VerifiedQuantizedAtomMixtureV1>,
    /// Recomputed owner-blind relation economics and ScoreV2-Q fields.
    economics: VerifiedEconomicsV2,
    /// Canonical descending ScoreV2-Q plus first-admitted duplicate tie.
    rank_key: [u8; SCORE_V2_Q_RANK_CAPACITY],
}

impl VerifiedSmoothDirectCandidateV1 {
    /// Return the canonical EconomicDomainV2 identity checked by this verdict.
    pub const fn economic_domain_digest(&self) -> Id32 {
        self.economic_domain_digest
    }

    /// Return the exact checked quantized price-measure summary.
    pub const fn price_measure(&self) -> &VerifiedPriceMeasureV3 {
        &self.price_measure
    }

    /// Return the exact positive atom-mixture fact for degree two or three.
    ///
    /// `None` means the Product selected degree zero or one, for which the
    /// atom-mixture V1 profile makes no claim.
    pub const fn quantized_atom_mixture(&self) -> Option<VerifiedQuantizedAtomMixtureV1> {
        self.quantized_atom_mixture
    }

    /// Return the exact checked RelationV2 economics and ScoreV2-Q fields.
    pub const fn economics(&self) -> &VerifiedEconomicsV2 {
        &self.economics
    }

    /// Return the canonical rank that a valid-verdict transition must persist.
    pub const fn rank_key(&self) -> &[u8; SCORE_V2_Q_RANK_CAPACITY] {
        &self.rank_key
    }
}

/// Successful cost-aware successor of the existing Direct-candidate verifier.
///
/// The embedded V1 verdict retains exact price and RelationV2 ownership. The
/// additional private certificate is derived from the same candidate plus the
/// frozen owner membership and a content-addressed immutable batch policy.
/// This result is not a fee assessment, selection authorization, or optimality
/// certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedCostedSmoothDirectCandidateV1 {
    verified_candidate: VerifiedSmoothDirectCandidateV1,
    cost_certificate: CandidateCostCertificateV1,
    rank_key: [u8; SCORE_V2_Q_RANK_CAPACITY],
}

impl VerifiedCostedSmoothDirectCandidateV1 {
    /// Exact checked RelationV2 economics without exposing the internal V1
    /// rank representation used during checker reuse.
    pub const fn economics(&self) -> &VerifiedEconomicsV2 {
        &self.verified_candidate.economics
    }

    /// Exact checked price-measure summary.
    pub const fn price_measure(&self) -> &VerifiedPriceMeasureV3 {
        &self.verified_candidate.price_measure
    }

    /// Owner-net, complete-set-quotiented preselection cost certificate.
    pub const fn cost_certificate(&self) -> &CandidateCostCertificateV1 {
        &self.cost_certificate
    }

    /// Breaking cost-aware rank successor consumed by a future action 14/15.
    pub const fn rank_key(&self) -> &[u8; SCORE_V2_Q_RANK_CAPACITY] {
        &self.rank_key
    }
}

/// Deterministic refusal set for the General V2 pure runtime join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralV2RuntimeError {
    /// A General V2 account/identity contract refused the input.
    Contract(CodecError),
    /// A Product/Series artifact or immutable join refused the input.
    Product(ProductError),
    /// The canonical PriceGrid account or tick membership check refused.
    PriceGrid(LayoutError),
    /// The exact quantized price-measure checker refused the certificate.
    PriceMeasure(ErrorV3),
    /// The exact positive degree-two/three atom-mixture checker refused.
    AtomMixture(AtomMixtureErrorV1),
    /// The owner-blind economic relation refused the candidate.
    Relation(EconomicErrorV2),
    /// Owner membership, immutable policy, or exact cost derivation refused.
    CandidateCost(CandidateCostErrorV1),
    /// This path admits Direct candidates only.
    UnsupportedCandidateKind,
    /// The authenticated AdmissionNode was not in the revealed pre-verdict state.
    InvalidAdmissionState,
    /// A Product, Market, Epoch, domain, feed, or policy identity disagreed.
    BindingMismatch,
    /// An authenticated basis was outside the V3 degree-zero-through-three domain.
    UnsupportedSmoothDegree,
    /// Witness schema or quantized semantic version was not the frozen pair.
    UnsupportedWitnessVersion,
    /// Witness width, atom order, mass, or primitive denominator was invalid.
    InvalidWitnessShape,
    /// An inactive witness slot was not canonical zero padding.
    NonCanonicalWitnessPadding,
    /// The recomputed canonical witness body digest disagreed with the feed.
    WitnessBodyDigestMismatch,
    /// The recomputed RelationV2 identity disagreed with the submitted final ID.
    CandidateIdentityMismatch,
    /// A checked offset, sum, or encoding operation overflowed.
    ArithmeticOverflow,
}

impl From<CodecError> for GeneralV2RuntimeError {
    fn from(value: CodecError) -> Self {
        Self::Contract(value)
    }
}

impl From<ProductError> for GeneralV2RuntimeError {
    fn from(value: ProductError) -> Self {
        Self::Product(value)
    }
}

impl From<LayoutError> for GeneralV2RuntimeError {
    fn from(value: LayoutError) -> Self {
        Self::PriceGrid(value)
    }
}

impl From<ErrorV3> for GeneralV2RuntimeError {
    fn from(value: ErrorV3) -> Self {
        Self::PriceMeasure(value)
    }
}

impl From<AtomMixtureErrorV1> for GeneralV2RuntimeError {
    fn from(value: AtomMixtureErrorV1) -> Self {
        Self::AtomMixture(value)
    }
}

impl From<EconomicErrorV2> for GeneralV2RuntimeError {
    fn from(value: EconomicErrorV2) -> Self {
        Self::Relation(value)
    }
}

impl From<CandidateCostErrorV1> for GeneralV2RuntimeError {
    fn from(value: CandidateCostErrorV1) -> Self {
        Self::CandidateCost(value)
    }
}

/// Decode the exact relation-relevant projection of a sealed candidate feed.
///
/// The authoritative General V2 decoder first validates the complete active-
/// width account, including settlement slices. This function then copies the
/// price, fill, and atom prefixes into fixed arrays with canonical zero
/// padding. It never accepts a stage account.
pub fn decode_sealed_candidate_feed_v1(
    input: &[u8],
) -> Result<(CandidateFeedHeaderV2, CandidateFeedEconomicsV1), GeneralV2RuntimeError> {
    let header = CandidateFeedHeaderV2::decode_account(input, true)?;
    let prices_at = CANDIDATE_FEED_HEADER_BYTES;
    let fills_at = prices_at
        .checked_add(
            usize::from(header.outcome_count)
                .checked_mul(8)
                .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?,
        )
        .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?;
    let atoms_at = fills_at
        .checked_add(
            usize::from(header.order_count)
                .checked_mul(8)
                .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?,
        )
        .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?;

    let mut body = CandidateFeedEconomicsV1 {
        prices: [0; MAX_OUTCOMES],
        fills: [0; MAX_ORDERS],
        atom_coordinates: [0; MAX_QUANTIZED_ATOMS],
        atom_masses: [0; MAX_QUANTIZED_ATOMS],
    };
    let mut index = 0usize;
    while index < usize::from(header.outcome_count) {
        let at = prices_at
            .checked_add(
                index
                    .checked_mul(8)
                    .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?,
            )
            .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?;
        body.prices[index] = read_u64(input, at)?;
        index += 1;
    }
    index = 0;
    while index < usize::from(header.order_count) {
        let at = fills_at
            .checked_add(
                index
                    .checked_mul(8)
                    .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?,
            )
            .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?;
        body.fills[index] = read_u64(input, at)?;
        index += 1;
    }
    index = 0;
    while index < usize::from(header.atom_count) {
        let at = atoms_at
            .checked_add(
                index
                    .checked_mul(QUANTIZED_ATOM_BYTES)
                    .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?,
            )
            .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?;
        body.atom_coordinates[index] = read_u128(input, at)?;
        body.atom_masses[index] = read_u64(
            input,
            at.checked_add(16)
                .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?,
        )?;
        index += 1;
    }
    Ok((header, body))
}

/// Reproject the existing General feed atoms into the stronger exact positive
/// certificate for one degree-two or degree-three Product basis.
///
/// The inputs are not self-authenticating. Callers in this crate invoke this
/// helper only after joining the exact MarketBinding, Genesis V2,
/// NativeClaimBasis, edge-registry selector, candidate-price digest, and feed
/// body that own them. Genesis V2 is the complete immutable Terms identity for
/// this projection because it owns the coordinate domain and selects every
/// policy joined by MarketBinding. No second atom body or account is created.
#[allow(clippy::too_many_arguments)]
pub(crate) fn verify_exact_smooth_atom_mixture_v1(
    market_id: Id32,
    terms_id: [u8; 32],
    basis_id: Id32,
    candidate_price_id: [u8; 32],
    coordinate_domain_min: u128,
    coordinate_domain_max: u128,
    basis: QuantizedBasisSpecV1,
    prices: [u64; MAX_OUTCOMES],
    atom_count: u8,
    common_denominator: u64,
    atom_coordinates: [u128; MAX_QUANTIZED_ATOMS],
    atom_masses: [u64; MAX_QUANTIZED_ATOMS],
) -> Result<VerifiedQuantizedAtomMixtureV1, GeneralV2RuntimeError> {
    let bindings = QuantizedAtomMixtureBindingsV1 {
        market_id: market_id.bytes(),
        terms_id,
        basis_id: basis_id.bytes(),
        price_id: candidate_price_id,
    };
    let bound = BoundQuantizedSplineV1 {
        bindings,
        coordinate_domain_min,
        coordinate_domain_max,
        basis,
    };
    let prices = QuantizedPayoutPriceVectorV1 {
        price_id: candidate_price_id,
        outcome_count: basis.outcome_count,
        prices,
    };
    let certificate = QuantizedAtomMixtureCertificateV1::new(
        bindings,
        basis.degree,
        basis.outcome_count,
        basis.denominator,
        common_denominator,
        atom_count,
        atom_coordinates,
        atom_masses,
    )?;
    verify_quantized_atom_mixture_v1(&bound, &prices, &certificate)
        .map_err(GeneralV2RuntimeError::from)
}

/// Verify one submitted quantized Direct General V2 candidate end to end.
///
/// Refusal order is: sealed-feed codec; contract/Product/grid/domain bindings;
/// canonical witness-body identity; exact quantized price coherence; owner-
/// blind RelationV2; submitted candidate identity; then canonical rank.
/// Success means this is one valid submitted candidate, not the optimal
/// clearing and not a settlement authorization.
#[allow(clippy::too_many_arguments)]
pub fn verify_smooth_direct_candidate_v1(
    candidate_feed_identity: Id32,
    sealed_candidate_feed: &[u8],
    admission_node: &AdmissionNodeV3AccountV1,
    economic_domain_account: &EconomicDomainV2AccountV1,
    market_binding: &MarketBindingV1,
    price_grid: &PriceGridAccount,
    product_template: &ProductTemplateV4,
    native_basis: &NativeClaimBasisV1,
    price_measure_policy: &PriceMeasurePolicyV1,
    genesis: &MarketGenesisProfileV2,
    market_instance: &MarketInstancePreimageV2,
    authenticated_edge_policy: QuantizedEdgePolicyV1,
    book: &EconomicBookV2,
) -> Result<VerifiedSmoothDirectCandidateV1, GeneralV2RuntimeError> {
    verify_smooth_direct_candidate_with_score_policy_v1(
        candidate_feed_identity,
        sealed_candidate_feed,
        admission_node,
        economic_domain_account,
        market_binding,
        price_grid,
        product_template,
        native_basis,
        price_measure_policy,
        genesis,
        market_instance,
        authenticated_edge_policy,
        book,
        score_v2_q_policy_id_v1()?,
    )
}

#[allow(clippy::too_many_arguments)]
fn verify_smooth_direct_candidate_with_score_policy_v1(
    candidate_feed_identity: Id32,
    sealed_candidate_feed: &[u8],
    admission_node: &AdmissionNodeV3AccountV1,
    economic_domain_account: &EconomicDomainV2AccountV1,
    market_binding: &MarketBindingV1,
    price_grid: &PriceGridAccount,
    product_template: &ProductTemplateV4,
    native_basis: &NativeClaimBasisV1,
    price_measure_policy: &PriceMeasurePolicyV1,
    genesis: &MarketGenesisProfileV2,
    market_instance: &MarketInstancePreimageV2,
    authenticated_edge_policy: QuantizedEdgePolicyV1,
    book: &EconomicBookV2,
    expected_score_policy_id: Id32,
) -> Result<VerifiedSmoothDirectCandidateV1, GeneralV2RuntimeError> {
    if candidate_feed_identity.is_zero() {
        return Err(GeneralV2RuntimeError::Contract(CodecError::ZeroIdentity));
    }
    let (header, feed) = decode_sealed_candidate_feed_v1(sealed_candidate_feed)?;
    admission_node.validate()?;
    economic_domain_account.validate()?;
    market_binding.validate()?;
    price_grid.validate()?;
    if header.candidate_kind != SettlementCandidateKindV1::Direct {
        return Err(GeneralV2RuntimeError::UnsupportedCandidateKind);
    }
    if admission_node.status != AdmissionNodeStatusV1::Revealed {
        return Err(GeneralV2RuntimeError::InvalidAdmissionState);
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
    if observed_settlement_witness != header.settlement_witness_digest
        || observed_settlement_witness != admission_node.settlement_witness_digest
        || observed_candidate_bundle != admission_node.candidate_bundle_digest
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
    let relation_policy_id = relation_v2_policy_id_v1()?;
    let score_policy_id = expected_score_policy_id;
    let transcript = economic_domain_account.transcript;
    let domain_digest = economic_domain_digest_v2(&CanonicalSha256, transcript)?;

    if economic_domain_account.epoch != header.epoch
        || admission_node.node != header.node
        || admission_node.epoch != header.epoch
        || admission_node.market != header.market
        || admission_node.relation_policy_id != header.relation_policy_id
        || admission_node.score_policy_id != score_policy_id
        || admission_node.admission_policy_id != market_binding.admission_policy_id
        || admission_node.epoch_generation != header.epoch_generation
        || admission_node.candidate_kind != header.candidate_kind
        || admission_node.settlement_candidate_id != header.settlement_candidate_id
        || admission_node.base_relation_candidate_id != header.base_relation_candidate_id
        || admission_node.settlement_witness_digest != header.settlement_witness_digest
        || header.market != market_binding.market
        || header.relation_policy_id != market_binding.relation_policy_id
        || header.price_measure_policy_v1_id != market_binding.price_measure_policy_v1_id
        || header.native_claim_basis_id != market_binding.native_claim_basis_id
        || header.economic_domain_digest != domain_digest
        || header.order_count != book.len
        || market_binding.market_genesis_profile_v2_id.bytes() != genesis_id
        || market_binding.market_instance_v2_id.bytes() != market_instance_id
        || market_binding.relation_policy_id != relation_policy_id
        || genesis.relation_policy_id.bytes() != relation_policy_id.bytes()
        || transcript.relation_policy_id != relation_policy_id
        || header.relation_policy_id != relation_policy_id
        || market_binding.score_policy_id != score_policy_id
        || genesis.score_policy_id.bytes() != score_policy_id.bytes()
        || market_binding.price_measure_policy_v1_id.bytes() != price_policy_id
        || market_binding.native_claim_basis_id.bytes() != basis_digest
        || market_binding.price_scale != price_grid.price_scale
        || market_binding.price_scale != header.price_scale
        || market_binding.relation_version != transcript.relation_version
        || market_binding.outcome_count != transcript.outcome_count
        || market_binding.outcome_count != header.outcome_count
        || market_binding.outcome_count != native_basis.outcome_count
        || market_binding.basis_degree != header.basis_degree
        || market_binding.basis_degree != native_basis.basis_degree
        || market_binding.candidate_kind_mask & 0b01 == 0
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
                market_binding.market,
                genesis_id,
                market_binding.native_claim_basis_id,
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
    let economics =
        verify_economic_candidate_v2(&relation_domain, book, &price_precondition, &candidate)?;
    if economics.economic_candidate_digest != header.base_relation_candidate_id.bytes()
        || economics.economic_candidate_digest != header.settlement_candidate_id.bytes()
    {
        return Err(GeneralV2RuntimeError::CandidateIdentityMismatch);
    }

    let rank_key = encode_score_v2_q_first_admitted_tie_v1(
        ScoreV2QComponentsV1 {
            certified_risk_flow_atoms: economics.score.score().risk.certified_risk_flow_atoms,
            cash_equivalent_direct_flow_atoms: economics
                .score
                .score()
                .cash_equivalent_direct_flow_atoms,
            virtual_churn_atoms: economics.score.score().virtual_churn_atoms,
            settlement_candidate_id: header.settlement_candidate_id,
        },
        FirstAdmittedTieV1 {
            ordinal: admission_node.ordinal,
        },
    )?;
    Ok(VerifiedSmoothDirectCandidateV1 {
        economic_domain_digest: domain_digest,
        price_measure,
        quantized_atom_mixture,
        economics,
        rank_key,
    })
}

/// Verify one Direct candidate and add its owner-net preselection cost tie.
///
/// This successor takes the private owner-bearing page projection rather than
/// a caller-provided owner table. It reuses the V1 price/RelationV2 verifier,
/// exact-joins that verdict back to the same frozen book, derives the cost
/// certificate, then uses the breaking rank encoder. Current SBF actions remain
/// capability-disabled until their immutable Market binding names the batch
/// policy identity and their comparison state explicitly selects this rank
/// version.
#[allow(clippy::too_many_arguments)]
pub fn verify_smooth_direct_candidate_costed_v1(
    candidate_feed_identity: Id32,
    sealed_candidate_feed: &[u8],
    admission_node: &clutch_general_v2_contract::AdmissionNodeV4AccountV1,
    economic_domain_account: &EconomicDomainV2AccountV1,
    market_binding: &clutch_general_v2_contract::MarketBindingV2,
    price_grid: &PriceGridAccount,
    product_template: &ProductTemplateV4,
    native_basis: &NativeClaimBasisV1,
    price_measure_policy: &PriceMeasurePolicyV1,
    genesis: &MarketGenesisProfileV2,
    market_instance: &MarketInstancePreimageV2,
    authenticated_edge_policy: QuantizedEdgePolicyV1,
    owner_projection: &OwnerBlindBookProjectionV2,
    cost_policy: &CandidateCostPolicyV1,
) -> Result<VerifiedCostedSmoothDirectCandidateV1, GeneralV2RuntimeError> {
    market_binding.validate()?;
    admission_node.validate()?;
    cost_policy.binds_market(market_binding)?;
    let relation_binding = market_binding.relation_projection();
    let relation_node = admission_node.base();
    let base = owner_projection.base();
    let cost_score_policy_id = score_v2_q_cost_policy_id_v1()?;
    let verified_candidate = verify_smooth_direct_candidate_with_score_policy_v1(
        candidate_feed_identity,
        sealed_candidate_feed,
        relation_node,
        economic_domain_account,
        &relation_binding,
        price_grid,
        product_template,
        native_basis,
        price_measure_policy,
        genesis,
        market_instance,
        authenticated_edge_policy,
        base.book(),
        cost_score_policy_id,
    )?;
    let (header, feed) = decode_sealed_candidate_feed_v1(sealed_candidate_feed)?;
    let domain = base.domain();
    let transcript = economic_domain_account.transcript;
    if base.market_binding() != &relation_binding
        || base.market() != header.market
        || base.epoch() != header.epoch
        || base.order_set() != header.order_set
        || base.economic_domain_digest() != header.economic_domain_digest
        || domain.market_semantics_digest != transcript.market_instance_v2_id.bytes()
        || domain.epoch_semantics_digest != transcript.epoch_semantics_digest.bytes()
        || domain.relation_policy_digest != transcript.relation_policy_id.bytes()
        || domain.price_policy_digest != transcript.price_measure_policy_v1_id.bytes()
        || domain.epoch_index != transcript.epoch_index
        || domain.outcome_count != transcript.outcome_count
        || domain.price_scale != transcript.price_scale
        || relation_binding.score_policy_id != cost_score_policy_id
    {
        return Err(GeneralV2RuntimeError::BindingMismatch);
    }
    let price = PricePreconditionV2 {
        policy_digest: transcript.price_measure_policy_v1_id.bytes(),
        semantic_price_digest: header.candidate_price_digest.bytes(),
        prices: feed.prices,
    };
    let candidate = EconomicCandidateV2 {
        fills: feed.fills,
        honored_aon_mask: header.honored_aon_mask,
        virtual_split: header.virtual_split,
        virtual_merge: header.virtual_merge,
    };
    let cost_certificate = derive_candidate_cost_certificate_v1(
        owner_projection,
        &price,
        &candidate,
        verified_candidate.economics(),
        cost_policy,
    )?;
    let economics = verified_candidate.economics();
    let rank_key = encode_score_v2_q_cost_first_admitted_tie_v1(
        ScoreV2QCostComponentsV1 {
            score: ScoreV2QComponentsV1 {
                certified_risk_flow_atoms: economics.score.score().risk.certified_risk_flow_atoms,
                cash_equivalent_direct_flow_atoms: economics
                    .score
                    .score()
                    .cash_equivalent_direct_flow_atoms,
                virtual_churn_atoms: economics.score.score().virtual_churn_atoms,
                settlement_candidate_id: header.settlement_candidate_id,
            },
            owner_net_cost_atoms: cost_certificate.owner_net_cost_atoms(),
        },
        FirstAdmittedTieV1 {
            ordinal: relation_node.ordinal,
        },
    )?;
    Ok(VerifiedCostedSmoothDirectCandidateV1 {
        verified_candidate,
        cost_certificate,
        rank_key,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CanonicalSha256;

impl Sha256BackendV1 for CanonicalSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
        hash_parts(parts)
    }
}

fn hash_parts(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    let mut part = 0usize;
    while part < parts.len() {
        hasher.update(parts[part]);
        part += 1;
    }
    hasher.finalize().into()
}

fn put(output: &mut [u8], cursor: &mut usize, value: &[u8]) -> Result<(), GeneralV2RuntimeError> {
    let end = cursor
        .checked_add(value.len())
        .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?;
    let target = output
        .get_mut(*cursor..end)
        .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?;
    target.copy_from_slice(value);
    *cursor = end;
    Ok(())
}

fn read_u64(input: &[u8], at: usize) -> Result<u64, GeneralV2RuntimeError> {
    let end = at
        .checked_add(8)
        .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?;
    let bytes: [u8; 8] = input
        .get(at..end)
        .ok_or(GeneralV2RuntimeError::Contract(CodecError::WrongLength))?
        .try_into()
        .map_err(|_| GeneralV2RuntimeError::Contract(CodecError::WrongLength))?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_u128(input: &[u8], at: usize) -> Result<u128, GeneralV2RuntimeError> {
    let end = at
        .checked_add(16)
        .ok_or(GeneralV2RuntimeError::ArithmeticOverflow)?;
    let bytes: [u8; 16] = input
        .get(at..end)
        .ok_or(GeneralV2RuntimeError::Contract(CodecError::WrongLength))?
        .try_into()
        .map_err(|_| GeneralV2RuntimeError::Contract(CodecError::WrongLength))?;
    Ok(u128::from_le_bytes(bytes))
}

const fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
