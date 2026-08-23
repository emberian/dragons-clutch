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
//! [`verify_smooth_direct_candidate_v1`] admits only smooth degrees two and
//! three. It verifies one already-submitted candidate and returns a canonical
//! rank. It does not find a candidate, establish global optimality, authorize
//! settlement, authenticate a Solana account owner/PDA, or move assets.
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
    economic_domain_digest_v2, encode_score_v2_q_first_admitted_tie_v1, AdmissionNodeStatusV1,
    AdmissionNodeV3AccountV1, CandidateFeedHeaderV2, CodecError, EconomicDomainV2AccountV1,
    FirstAdmittedTieV1, Id32, MarketBindingV1, ScoreV2QComponentsV1, SettlementCandidateKindV1,
    Sha256BackendV1, CANDIDATE_FEED_HEADER_BYTES, MAX_ORDERS, MAX_OUTCOMES, MAX_QUANTIZED_ATOMS,
    PRICE_MEASURE_WITNESS_SCHEMA_V3, QUANTIZED_ATOM_BYTES, QUANTIZED_PRICE_MEASURE_SEMANTICS_V1,
    SCORE_V2_Q_RANK_CAPACITY,
};
use clutch_price_measure::{
    verify_quantized_price_measure_v3_smooth, AdapterBindingsV3, ErrorV3, PriceVectorV3,
    QuantizedAtomWitnessV3, VerifiedPriceMeasureV3, PRICE_MEASURE_WITNESS_VERSION_V3,
    QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1,
};
use clutch_product_series::{
    Error as ProductError, MarketGenesisProfileV2, MarketInstancePreimageV2, NativeClaimBasisV1,
    PriceMeasurePolicyV1, ProductTemplateV4, QuantizedEdgePolicyV1,
};
use clutch_solana_layout::{CodecError as LayoutError, PriceGridAccount};
use sha2::{Digest, Sha256};

mod builder;

pub use builder::*;

/// SHA-256 domain for the canonical V3 quantized witness body.
///
/// The terminating zero is part of the preimage. A future field or encoding
/// change requires a new domain and schema rather than reinterpretation.
pub const QUANTIZED_WITNESS_BODY_DIGEST_DOMAIN_V1: &[u8] =
    b"dragons-clutch/quantized-price-measure-witness/v3\0";

/// SHA-256 domain for General V2's canonical owner-blind RelationV2 policy.
pub const RELATION_V2_POLICY_DIGEST_DOMAIN_V1: &[u8] = b"dragons-clutch/relation-v2-policy/v1\0";
/// SHA-256 domain for General V2's canonical ScoreV2-Q selection policy.
pub const SCORE_V2_Q_POLICY_DIGEST_DOMAIN_V1: &[u8] = b"dragons-clutch/score-v2-q-policy/v1\0";

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

/// Exact body bytes excluding the digest field itself.
///
/// Layout, in order:
///
/// ```text
/// schema:u8 || quantized_semantics:u8
/// || candidate_feed[32] || relation_domain_digest[32]
/// || basis_digest[32] || candidate_price_digest[32]
/// || basis_degree:u8 || outcome_count:u8 || atom_count:u8
/// || common_denominator_le:u64
/// || 16 * atom_coordinate_le:u128
/// || 16 * atom_mass_le:u64
/// ```
pub const QUANTIZED_WITNESS_BODY_BYTES_V1: usize =
    2 + (4 * 32) + 3 + 8 + (MAX_QUANTIZED_ATOMS * 16) + (MAX_QUANTIZED_ATOMS * 8);

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_ORDERS == 64);
const _: () = assert!(MAX_QUANTIZED_ATOMS == 16);
const _: () = assert!(QUANTIZED_WITNESS_BODY_BYTES_V1 == 525);
const _: () = assert!(RELATION_V2_POLICY_BODY_V1.len() == 24);
const _: () = assert!(SCORE_V2_Q_POLICY_BODY_V1.len() == 16);
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

/// Canonical quantized price-measure witness body before hashing.
///
/// The body binds one nonunique coherence certificate. Its digest authenticates
/// the retained sidecar but never enters RelationV2's economic candidate
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
    pub relation_domain_digest: Id32,
    /// Canonical NativeClaimBasisV1 body identity.
    pub basis_digest: Id32,
    /// Canonical exact candidate-price identity.
    pub candidate_price_digest: Id32,
    /// Smooth basis degree, exactly two or three in this runtime path.
    pub basis_degree: u8,
    /// Active native outcome width.
    pub outcome_count: u8,
    /// Active atom support width.
    pub atom_count: u8,
    /// Primitive positive atom-mass denominator.
    pub common_denominator: u64,
    /// Active sorted coordinates followed by canonical zero padding.
    pub atom_coordinates: [u128; MAX_QUANTIZED_ATOMS],
    /// Active positive masses followed by canonical zero padding.
    pub atom_masses: [u64; MAX_QUANTIZED_ATOMS],
}

impl QuantizedWitnessBodyV1 {
    /// Validate version, smooth shape, support order, primitive mass, and
    /// canonical padding before encoding or hashing.
    pub fn validate(self) -> Result<(), GeneralV2RuntimeError> {
        if self.schema_version != PRICE_MEASURE_WITNESS_VERSION_V3
            || self.quantized_semantics_version != QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1
        {
            return Err(GeneralV2RuntimeError::UnsupportedWitnessVersion);
        }
        if !(2..=3).contains(&self.basis_degree)
            || !(self.basis_degree + 1..=16).contains(&self.outcome_count)
            || self.atom_count == 0
            || self.atom_count > self.outcome_count
            || usize::from(self.atom_count) > MAX_QUANTIZED_ATOMS
            || self.common_denominator == 0
        {
            return Err(GeneralV2RuntimeError::InvalidWitnessShape);
        }
        if self.candidate_feed.is_zero()
            || self.relation_domain_digest.is_zero()
            || self.basis_digest.is_zero()
            || self.candidate_price_digest.is_zero()
        {
            return Err(GeneralV2RuntimeError::BindingMismatch);
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

    /// Encode the exact fixed-width canonical body.
    pub fn encode(self) -> Result<[u8; QUANTIZED_WITNESS_BODY_BYTES_V1], GeneralV2RuntimeError> {
        self.validate()?;
        let mut output = [0u8; QUANTIZED_WITNESS_BODY_BYTES_V1];
        let mut cursor = 0usize;
        put(&mut output, &mut cursor, &[self.schema_version])?;
        put(
            &mut output,
            &mut cursor,
            &[self.quantized_semantics_version],
        )?;
        for id in [
            self.candidate_feed,
            self.relation_domain_digest,
            self.basis_digest,
            self.candidate_price_digest,
        ] {
            put(&mut output, &mut cursor, &id.bytes())?;
        }
        put(
            &mut output,
            &mut cursor,
            &[self.basis_degree, self.outcome_count, self.atom_count],
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.common_denominator.to_le_bytes(),
        )?;
        let mut atom = 0usize;
        while atom < MAX_QUANTIZED_ATOMS {
            put(
                &mut output,
                &mut cursor,
                &self.atom_coordinates[atom].to_le_bytes(),
            )?;
            atom += 1;
        }
        atom = 0;
        while atom < MAX_QUANTIZED_ATOMS {
            put(
                &mut output,
                &mut cursor,
                &self.atom_masses[atom].to_le_bytes(),
            )?;
            atom += 1;
        }
        if cursor != QUANTIZED_WITNESS_BODY_BYTES_V1 {
            return Err(GeneralV2RuntimeError::ArithmeticOverflow);
        }
        Ok(output)
    }

    /// Derive the canonical SHA-256 body identity.
    pub fn digest(self) -> Result<Id32, GeneralV2RuntimeError> {
        let encoded = self.encode()?;
        Id32::new(hash_parts(&[
            QUANTIZED_WITNESS_BODY_DIGEST_DOMAIN_V1,
            &encoded,
        ]))
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

    /// Return the exact checked RelationV2 economics and ScoreV2-Q fields.
    pub const fn economics(&self) -> &VerifiedEconomicsV2 {
        &self.economics
    }

    /// Return the canonical rank that a valid-verdict transition must persist.
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
    /// The owner-blind economic relation refused the candidate.
    Relation(EconomicErrorV2),
    /// This path admits Direct candidates only.
    UnsupportedCandidateKind,
    /// The authenticated AdmissionNode was not in the revealed pre-verdict state.
    InvalidAdmissionState,
    /// A Product, Market, Epoch, domain, feed, or policy identity disagreed.
    BindingMismatch,
    /// This path is deliberately restricted to smooth degree two or three.
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

impl From<EconomicErrorV2> for GeneralV2RuntimeError {
    fn from(value: EconomicErrorV2) -> Self {
        Self::Relation(value)
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

/// Verify one submitted direct General V2 candidate end to end in pure core.
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
    if !(2..=3).contains(&header.basis_degree) {
        return Err(GeneralV2RuntimeError::UnsupportedSmoothDegree);
    }

    market_instance.validate_bindings(
        product_template,
        native_basis,
        price_measure_policy,
        genesis,
    )?;
    let projected_basis = price_measure_policy.project_smooth_basis(
        native_basis,
        genesis,
        authenticated_edge_policy,
    )?;

    let basis_id = native_basis.id()?.bytes();
    let price_policy_id = price_measure_policy.id()?.bytes();
    let genesis_id = genesis.id()?.bytes();
    let market_instance_id = market_instance.id()?.bytes();
    let relation_policy_id = relation_v2_policy_id_v1()?;
    let score_policy_id = score_v2_q_policy_id_v1()?;
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
        || market_binding.native_claim_basis_id.bytes() != basis_id
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

    let witness_body = QuantizedWitnessBodyV1 {
        schema_version: header.price_witness_schema,
        quantized_semantics_version: header.quantized_semantics_version,
        candidate_feed: candidate_feed_identity,
        relation_domain_digest: domain_digest,
        basis_digest: header.native_claim_basis_id,
        candidate_price_digest: header.candidate_price_digest,
        basis_degree: header.basis_degree,
        outcome_count: header.outcome_count,
        atom_count: header.atom_count,
        common_denominator: header.common_denominator,
        atom_coordinates: feed.atom_coordinates,
        atom_masses: feed.atom_masses,
    };
    let observed_body_digest = witness_body.digest()?;
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
    let price_measure =
        verify_quantized_price_measure_v3_smooth(&bindings, &projected_basis, &prices, &witness)?;

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
            certified_risk_flow_atoms: economics.score.risk.certified_risk_flow_atoms,
            cash_equivalent_direct_flow_atoms: economics.score.cash_equivalent_direct_flow_atoms,
            virtual_churn_atoms: economics.score.virtual_churn_atoms,
            settlement_candidate_id: header.settlement_candidate_id,
        },
        FirstAdmittedTieV1 {
            ordinal: admission_node.ordinal,
        },
    )?;
    Ok(VerifiedSmoothDirectCandidateV1 {
        economic_domain_digest: domain_digest,
        price_measure,
        economics,
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
