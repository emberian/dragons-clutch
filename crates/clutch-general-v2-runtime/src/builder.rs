// SPDX-License-Identifier: AGPL-3.0-or-later

//! Deterministic bounded construction of honest General V2 submissions.
//!
//! This module is an untrusted searcher wrapped around the exact checkers in
//! this crate. It never claims that a returned candidate is globally optimal.
//! The returned status distinguishes complete enumeration of the caller's
//! named bounded heuristic family from a search cut short by an explicit work
//! budget.

use core::cmp::min;

use clutch_batch::relation_v2::{
    price_semantics_digest_v2, verify_economic_candidate_v2, EconomicBookV2, EconomicCandidateV2,
    EconomicDomainV2, EconomicErrorV2, EconomicOrderV2, PricePreconditionV2, VerifiedEconomicsV2,
};
use clutch_batch::{PartialPolicy, Side};
use clutch_general_v2_contract::{
    candidate_feed_account_len, economic_domain_digest_v2, AdmissionNodeStatusV1,
    AdmissionNodeV3AccountV1, CandidateFeedHeaderV2, CodecError, DeletableRentOwnerV1,
    EconomicDomainV2AccountV1, GeneralOrderPageSeedTupleV5, Id32, MarketBindingV1,
    SettlementCandidateKindV1,
    CANDIDATE_FEED_HEADER_BYTES, MAX_ORDERS, MAX_OUTCOMES, MAX_QUANTIZED_ATOMS,
    PRICE_MEASURE_WITNESS_SCHEMA_V3, QUANTIZED_PRICE_MEASURE_SEMANTICS_V1, SETTLEMENT_SLICE_BYTES,
};
use clutch_price_measure::{
    verify_quantized_price_measure_v3_degree_zero, verify_quantized_price_measure_v3_smooth,
    AdapterBindingsV3, ErrorV3, PriceVectorV3, QuantizedAtomWitnessV3, VerifiedPriceMeasureV3,
    QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1,
};
use clutch_product_series::{
    Error as ProductError, MarketGenesisProfileV2, MarketInstancePreimageV2, NativeClaimBasisV1,
    PriceMeasurePolicyV1, ProductTemplateV4, QuantizedEdgePolicyV1,
};
use clutch_solana_layout::{
    order_page_v5::{verify_page_set_v5_streaming, verify_page_v5, OrderSlotCursorV5},
    stream, CodecError as LayoutError, OrderSlot, PriceGridAccount, MAX_ORDER_PAGES,
};

use crate::{
    relation_v2_policy_id_v1, score_v2_q_policy_id_v1, CanonicalSha256, GeneralV2RuntimeError,
    QuantizedBasisProjectionV1, QuantizedWitnessBodyV1,
};

/// Largest coordinate sample retained by the fixed-memory searcher.
pub const MAX_BUILDER_COORDINATES_V1: usize = 64;

/// Search-report bit: the atom-measure cap left declared measures unvisited.
pub const SEARCH_TRUNCATED_ATOM_MEASURES_V1: u8 = 1 << 0;
/// Search-report bit: a per-price fill cap left declared fills unvisited.
pub const SEARCH_TRUNCATED_FILL_WITNESSES_V1: u8 = 1 << 1;

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_ORDERS == 64);
const _: () = assert!(MAX_QUANTIZED_ATOMS == 16);

/// One canonical primitive finite measure proposed to the exact V3 checker.
///
/// This is not a proof or accepted witness by construction. The builder checks
/// its shape, evaluates every atom through the Product-selected basis, derives
/// exact integer prices, and then runs the authoritative V3 checker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantizedAtomProposalV1 {
    /// Active sorted support width.
    pub atom_count: u8,
    /// Primitive positive mass denominator.
    pub common_denominator: u64,
    /// Active strictly increasing coordinates followed by zero padding.
    pub atom_coordinates: [u128; MAX_QUANTIZED_ATOMS],
    /// Active positive masses followed by zero padding.
    pub atom_masses: [u64; MAX_QUANTIZED_ATOMS],
}

impl QuantizedAtomProposalV1 {
    /// Construct the canonical unit mass at one integer coordinate.
    pub const fn singleton(coordinate: u128) -> Self {
        let mut atom_coordinates = [0u128; MAX_QUANTIZED_ATOMS];
        let mut atom_masses = [0u64; MAX_QUANTIZED_ATOMS];
        atom_coordinates[0] = coordinate;
        atom_masses[0] = 1;
        Self {
            atom_count: 1,
            common_denominator: 1,
            atom_coordinates,
            atom_masses,
        }
    }

    fn primitive_pair(
        left_coordinate: u128,
        right_coordinate: u128,
        left_mass: u64,
        denominator: u64,
    ) -> Result<Self, CandidateBuilderErrorV1> {
        if left_coordinate >= right_coordinate
            || left_mass == 0
            || left_mass >= denominator
            || gcd_u64(left_mass, denominator) != 1
        {
            return Err(CandidateBuilderErrorV1::InvalidAtomProposal);
        }
        let mut atom_coordinates = [0u128; MAX_QUANTIZED_ATOMS];
        let mut atom_masses = [0u64; MAX_QUANTIZED_ATOMS];
        atom_coordinates[0] = left_coordinate;
        atom_coordinates[1] = right_coordinate;
        atom_masses[0] = left_mass;
        atom_masses[1] = denominator - left_mass;
        Ok(Self {
            atom_count: 2,
            common_denominator: denominator,
            atom_coordinates,
            atom_masses,
        })
    }
}

/// Deterministic finite search family selected by an offchain solver.
///
/// The family consists of every supplied atom proposal, singleton measures at
/// the first `maximum_coordinates` points of the exact knot interval under the
/// named stride (including the last knot when capacity remains), and primitive
/// two-point mixtures over those samples through the named denominator. For
/// each price it tries the empty fill, every maximal exactly balanced buy/sell
/// pair, and then zero/minimum/full fills over the named order prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundedSearchPlanV1 {
    /// Positive integer stride used to sample the exact closed knot interval.
    pub coordinate_stride: u128,
    /// Number of sampled coordinates retained, in `1..=64`.
    pub maximum_coordinates: u8,
    /// Largest primitive pair denominator; zero or one disables pair search.
    /// It may not exceed the Product-selected witness-denominator limit.
    pub maximum_pair_denominator: u64,
    /// Hard cap on supplied and generated atom measures considered.
    pub maximum_atom_measures: u32,
    /// Active order prefix allowed nonzero fill options, in `0..=64`.
    pub fill_order_limit: u8,
    /// Hard cap on fill vectors considered for each coherent grid price.
    pub maximum_fill_witnesses_per_price: u32,
}

impl BoundedSearchPlanV1 {
    /// Refuse a zero work budget or a fixed-capacity overflow.
    pub fn validate(self) -> Result<(), CandidateBuilderErrorV1> {
        if self.coordinate_stride == 0
            || self.maximum_coordinates == 0
            || usize::from(self.maximum_coordinates) > MAX_BUILDER_COORDINATES_V1
            || self.maximum_atom_measures == 0
            || usize::from(self.fill_order_limit) > MAX_ORDERS
            || self.maximum_fill_witnesses_per_price == 0
        {
            return Err(CandidateBuilderErrorV1::InvalidSearchPlan);
        }
        Ok(())
    }
}

/// Whether every member of the explicitly declared bounded heuristic family
/// was visited.
///
/// Neither variant makes a statement about the full clearing problem. Even
/// `CompleteForDeclaredFamily` is not an optimality certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchCompletenessV1 {
    /// Every atom measure and fill vector named by the plan was visited.
    CompleteForDeclaredFamily,
    /// One or more explicit work caps stopped the declared enumeration early.
    IncompleteAtWorkLimit,
}

/// Factual work and coverage report for one bounded construction run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateSearchReportV1 {
    /// Completion only relative to [`BoundedSearchPlanV1`].
    pub completeness: SearchCompletenessV1,
    /// Named `SEARCH_TRUNCATED_*_V1` bits explaining incomplete enumeration.
    pub truncation_mask: u8,
    /// Caller proposals actually considered.
    pub supplied_atom_measures_considered: u32,
    /// Coordinates sampled from the exact closed knot interval.
    pub sampled_coordinate_count: u8,
    /// Whether the stride visited every integer coordinate in the interval.
    pub full_integer_coordinate_interval: bool,
    /// Total supplied and generated atom measures considered.
    pub atom_measures_considered: u32,
    /// Measures that produced an exact integer simplex price.
    pub exact_price_vectors: u32,
    /// Exact price vectors whose every component was a PriceGrid member.
    pub grid_price_vectors: u32,
    /// Fill witnesses explored, including cheaply rejected imbalance shapes.
    pub fill_witnesses_considered: u64,
    /// Fill witnesses accepted by RelationV2.
    pub valid_submitted_candidates: u64,
    /// Active order prefix assigned nonzero options by the search family.
    pub fill_order_limit: u8,
    /// Largest pair denominator named by the plan.
    pub maximum_pair_denominator: u64,
}

impl CandidateSearchReportV1 {
    fn empty(plan: BoundedSearchPlanV1) -> Self {
        Self {
            completeness: SearchCompletenessV1::CompleteForDeclaredFamily,
            truncation_mask: 0,
            supplied_atom_measures_considered: 0,
            sampled_coordinate_count: 0,
            full_integer_coordinate_interval: false,
            atom_measures_considered: 0,
            exact_price_vectors: 0,
            grid_price_vectors: 0,
            fill_witnesses_considered: 0,
            valid_submitted_candidates: 0,
            fill_order_limit: plan.fill_order_limit,
            maximum_pair_denominator: plan.maximum_pair_denominator,
        }
    }

    fn truncate(&mut self, reason: u8) {
        self.truncation_mask |= reason;
        self.completeness = SearchCompletenessV1::IncompleteAtWorkLimit;
    }
}

/// Authenticated inputs shared by every price and fill explored in one run.
#[derive(Clone, Copy, Debug)]
pub struct SmoothDirectBuilderInputsV1<'a> {
    /// Prospective canonical feed account identity used by the V3 body digest.
    pub candidate_feed_identity: Id32,
    /// Immutable General V2 EconomicDomain owner.
    pub economic_domain_account: &'a EconomicDomainV2AccountV1,
    /// Immutable General V2 Market/Product/policy join.
    pub market_binding: &'a MarketBindingV1,
    /// Frozen exact PriceGrid.
    pub price_grid: &'a PriceGridAccount,
    /// Immutable Product template.
    pub product_template: &'a ProductTemplateV4,
    /// Product-owned native claim basis.
    pub native_basis: &'a NativeClaimBasisV1,
    /// Product-selected exact price-measure policy.
    pub price_measure_policy: &'a PriceMeasurePolicyV1,
    /// Immutable market Genesis profile.
    pub genesis: &'a MarketGenesisProfileV2,
    /// Immutable recurring market instance.
    pub market_instance: &'a MarketInstancePreimageV2,
    /// Registry-authenticated edge policy selected by the basis.
    pub authenticated_edge_policy: QuantizedEdgePolicyV1,
    /// Canonical owner-blind projection of one authenticated frozen order set.
    pub book_projection: &'a OwnerBlindBookProjectionV1,
}

/// One checker-accepted candidate chosen from a named bounded search family.
///
/// Fields are private so callers cannot replace the exact price, atoms,
/// RelationV2 identity, or recomputed economics before canonical feed encoding.
/// The search report is evidence only about work performed, never authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuiltDirectCandidateV1 {
    price: PriceVectorV3,
    atom_proposal: QuantizedAtomProposalV1,
    witness_body: QuantizedWitnessBodyV1,
    price_measure: VerifiedPriceMeasureV3,
    economic_candidate: EconomicCandidateV2,
    economics: VerifiedEconomicsV2,
    economic_domain_digest: Id32,
    price_measure_policy_id: Id32,
    market_binding: MarketBindingV1,
    market: Id32,
    epoch: Id32,
    order_set: Id32,
    order_count: u8,
    search_report: CandidateSearchReportV1,
}

impl BuiltDirectCandidateV1 {
    /// Exact integer grid price selected by the bounded builder.
    pub const fn price(&self) -> &PriceVectorV3 {
        &self.price
    }

    /// Primitive exact quantized atoms emitted by the builder.
    pub const fn atom_proposal(&self) -> &QuantizedAtomProposalV1 {
        &self.atom_proposal
    }

    /// Canonical V3 witness body whose digest enters CandidateFeedV2.
    pub const fn witness_body(&self) -> &QuantizedWitnessBodyV1 {
        &self.witness_body
    }

    /// Exact V3 checker result for the retained witness body.
    pub const fn price_measure(&self) -> &VerifiedPriceMeasureV3 {
        &self.price_measure
    }

    /// Exact fills, AON mask, and virtual conversion checked by RelationV2.
    pub const fn economic_candidate(&self) -> &EconomicCandidateV2 {
        &self.economic_candidate
    }

    /// Recomputed RelationV2 economics, candidate identity, and ScoreV2-Q.
    pub const fn economics(&self) -> &VerifiedEconomicsV2 {
        &self.economics
    }

    /// Authenticated parent market of the projected frozen book.
    pub const fn market(&self) -> Id32 {
        self.market
    }

    /// Authenticated parent epoch of the projected frozen book.
    pub const fn epoch(&self) -> Id32 {
        self.epoch
    }

    /// Authenticated frozen order-set identity used by RelationV2.
    pub const fn order_set(&self) -> Id32 {
        self.order_set
    }

    /// Factual coverage and work counts for the bounded heuristic search.
    pub const fn search_report(&self) -> &CandidateSearchReportV1 {
        &self.search_report
    }

    /// Canonical exact-price identity consumed by CandidateFeedV2.
    pub const fn candidate_price_digest(&self) -> Id32 {
        self.witness_body.candidate_price_digest
    }

    /// Canonical witness-body identity consumed by CandidateFeedV2.
    pub fn price_body_digest(&self) -> Result<Id32, CandidateBuilderErrorV1> {
        self.witness_body
            .digest()
            .map_err(CandidateBuilderErrorV1::Runtime)
    }

    /// Canonical RelationV2 candidate identity for a Direct submission.
    pub fn base_relation_candidate_id(&self) -> Result<Id32, CandidateBuilderErrorV1> {
        Id32::new(self.economics.economic_candidate_digest)
            .map_err(CandidateBuilderErrorV1::Contract)
    }

    /// Encode one exact active-width sealed CandidateFeedV2 account.
    ///
    /// Prices, fills, atoms, IDs, and policy fields come only from checked
    /// builder state and authenticated General accounts. Settlement-slice bytes
    /// remain a separately owned boundary: this function validates their
    /// CandidateFeedV2 syntax and binds the digest already authenticated by the
    /// revealed AdmissionNode, but does not claim to derive or verify their
    /// economic decomposition.
    #[allow(clippy::too_many_arguments)]
    pub fn encode_sealed_candidate_feed_v2(
        &self,
        output: &mut [u8],
        candidate_feed_identity: Id32,
        admission_node: &AdmissionNodeV3AccountV1,
        market_binding: &MarketBindingV1,
        economic_domain_account: &EconomicDomainV2AccountV1,
        settlement_tail: SettlementTailV1<'_>,
        rent: DeletableRentOwnerV1,
        stored_bump: u8,
    ) -> Result<CandidateFeedHeaderV2, CandidateBuilderErrorV1> {
        admission_node.validate()?;
        market_binding.validate()?;
        economic_domain_account.validate()?;
        rent.validate()?;
        if admission_node.status != AdmissionNodeStatusV1::Revealed
            || candidate_feed_identity != self.witness_body.candidate_feed
            || *market_binding != self.market_binding
            || admission_node.epoch != self.epoch
            || admission_node.market != self.market
            || admission_node.epoch != economic_domain_account.epoch
            || admission_node.market != market_binding.market
            || admission_node.relation_policy_id != relation_v2_policy_id_v1()?
            || admission_node.score_policy_id != score_v2_q_policy_id_v1()?
            || admission_node.admission_policy_id != market_binding.admission_policy_id
            || market_binding.relation_policy_id != admission_node.relation_policy_id
            || market_binding.score_policy_id != admission_node.score_policy_id
            || market_binding.price_measure_policy_v1_id != self.price_measure_policy_id
            || market_binding.native_claim_basis_id != self.witness_body.basis_digest
            || market_binding.price_scale != self.price.price_scale
            || market_binding.outcome_count != self.price.native_outcome_count
            || market_binding.basis_degree != self.price.basis_degree
            || economic_domain_account.transcript.market_instance_v2_id
                != market_binding.market_instance_v2_id
            || economic_domain_account.transcript.relation_policy_id
                != market_binding.relation_policy_id
            || economic_domain_account
                .transcript
                .price_measure_policy_v1_id
                != market_binding.price_measure_policy_v1_id
            || economic_domain_account.transcript.native_claim_basis_id
                != market_binding.native_claim_basis_id
            || economic_domain_account.transcript.outcome_count != market_binding.outcome_count
            || economic_domain_account.transcript.price_scale != market_binding.price_scale
            || economic_domain_digest_v2(&CanonicalSha256, economic_domain_account.transcript)?
                != self.economic_domain_digest
        {
            return Err(CandidateBuilderErrorV1::BindingMismatch);
        }
        let base_candidate = self.base_relation_candidate_id()?;
        if admission_node.candidate_kind != SettlementCandidateKindV1::Direct
            || admission_node.base_relation_candidate_id != base_candidate
            || admission_node.settlement_candidate_id != base_candidate
        {
            return Err(CandidateBuilderErrorV1::BindingMismatch);
        }

        settlement_tail.validate()?;
        let needs_settlement = self.economic_candidate.virtual_split != 0
            || self.economic_candidate.virtual_merge != 0
            || self.economic_candidate.fills[..usize::from(self.order_count)]
                .iter()
                .any(|fill| *fill != 0);
        if needs_settlement != (settlement_tail.slice_count != 0) {
            return Err(CandidateBuilderErrorV1::SettlementTailMismatch);
        }
        let expected_len = candidate_feed_account_len(
            self.price.native_outcome_count,
            self.order_count,
            self.atom_proposal.atom_count,
            settlement_tail.slice_count,
        )?;
        if output.len() != expected_len {
            return Err(CandidateBuilderErrorV1::OutputLengthMismatch);
        }

        let header = CandidateFeedHeaderV2 {
            epoch: admission_node.epoch,
            node: admission_node.node,
            market: admission_node.market,
            order_set: self.order_set,
            relation_policy_id: admission_node.relation_policy_id,
            economic_domain_digest: self.economic_domain_digest,
            native_claim_basis_id: self.witness_body.basis_digest,
            candidate_price_digest: self.witness_body.candidate_price_digest,
            price_measure_policy_v1_id: self.price_measure_policy_id,
            settlement_candidate_id: base_candidate,
            base_relation_candidate_id: base_candidate,
            settlement_witness_digest: admission_node.settlement_witness_digest,
            price_body_digest: self.price_body_digest()?,
            epoch_generation: admission_node.epoch_generation,
            virtual_split: self.economic_candidate.virtual_split,
            virtual_merge: self.economic_candidate.virtual_merge,
            honored_aon_mask: self.economic_candidate.honored_aon_mask,
            price_scale: self.price.price_scale,
            common_denominator: self.atom_proposal.common_denominator,
            close_reward_lamports: market_binding.feed_close_reward,
            basis_degree: self.price.basis_degree,
            outcome_count: self.price.native_outcome_count,
            order_count: self.order_count,
            atom_count: self.atom_proposal.atom_count,
            slice_count: settlement_tail.slice_count,
            prices_written: self.price.native_outcome_count,
            fills_written: self.order_count,
            atoms_written: self.atom_proposal.atom_count,
            slices_written: settlement_tail.slice_count,
            candidate_kind: SettlementCandidateKindV1::Direct,
            price_witness_schema: PRICE_MEASURE_WITNESS_SCHEMA_V3,
            quantized_semantics_version: QUANTIZED_PRICE_MEASURE_SEMANTICS_V1,
            rent,
            stored_bump,
            flags: 0,
        };
        header.encode(&mut output[..CANDIDATE_FEED_HEADER_BYTES], true)?;
        let mut cursor = CANDIDATE_FEED_HEADER_BYTES;
        write_u64_prefix(
            output,
            &mut cursor,
            &self.price.prices,
            usize::from(self.price.native_outcome_count),
        )?;
        write_u64_prefix(
            output,
            &mut cursor,
            &self.economic_candidate.fills,
            usize::from(self.order_count),
        )?;
        let mut atom = 0usize;
        while atom < usize::from(self.atom_proposal.atom_count) {
            put_bytes(
                output,
                &mut cursor,
                &self.atom_proposal.atom_coordinates[atom].to_le_bytes(),
            )?;
            put_bytes(
                output,
                &mut cursor,
                &self.atom_proposal.atom_masses[atom].to_le_bytes(),
            )?;
            atom += 1;
        }
        put_bytes(output, &mut cursor, settlement_tail.encoded_slices)?;
        if cursor != output.len() {
            return Err(CandidateBuilderErrorV1::OutputLengthMismatch);
        }
        let decoded = CandidateFeedHeaderV2::decode_account(output, true)?;
        if decoded != header {
            return Err(CandidateBuilderErrorV1::BindingMismatch);
        }
        let observed_body_digest = clutch_general_v2_contract::quantized_witness_body_digest_v3(
            &CanonicalSha256,
            candidate_feed_identity,
            output,
            true,
        )?;
        if observed_body_digest != header.price_body_digest {
            return Err(CandidateBuilderErrorV1::BindingMismatch);
        }
        Ok(header)
    }
}

/// Separately owned settlement-slice tail supplied to canonical feed encoding.
///
/// The General feed codec checks exact width and record syntax. An independent
/// settlement builder/checker must derive the bytes and authenticate the digest
/// stored in the AdmissionNode; this type intentionally does neither.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementTailV1<'a> {
    /// Active slice count.
    pub slice_count: u16,
    /// Exact concatenation of thirteen-byte CandidateFeedV2 slice records.
    pub encoded_slices: &'a [u8],
}

impl SettlementTailV1<'_> {
    fn validate(self) -> Result<(), CandidateBuilderErrorV1> {
        let expected = usize::from(self.slice_count)
            .checked_mul(SETTLEMENT_SLICE_BYTES)
            .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
        if self.encoded_slices.len() != expected {
            return Err(CandidateBuilderErrorV1::SettlementTailMismatch);
        }
        Ok(())
    }
}

/// Deterministic refusal set for General V2 candidate construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateBuilderErrorV1 {
    /// Existing General V2 pure runtime refused a derived value.
    Runtime(GeneralV2RuntimeError),
    /// General V2 contract/account codec refused a binding or serialization.
    Contract(CodecError),
    /// Solana layout/page/grid codec refused a projection input.
    Layout(LayoutError),
    /// Product/Series refused an immutable artifact or projection.
    Product(ProductError),
    /// Exact V3 price-measure checker refused a derived witness.
    PriceMeasure(ErrorV3),
    /// Owner-blind RelationV2 refused a derived candidate.
    Relation(EconomicErrorV2),
    /// A search work cap or fixed-capacity parameter was malformed.
    InvalidSearchPlan,
    /// A supplied atom proposal was not primitive, sorted, padded, or in range.
    InvalidAtomProposal,
    /// No explored atom measure produced an exact all-grid price.
    NoCoherentGridPrice,
    /// No explored fill witness was accepted by RelationV2.
    NoValidSubmittedCandidate,
    /// Frozen page identities disagreed with the expected General V2 binding.
    BindingMismatch,
    /// A checked exact arithmetic operation overflowed.
    ArithmeticOverflow,
    /// CandidateFeedV2 output did not have its exact active-width length.
    OutputLengthMismatch,
    /// Settlement tail width or empty/nonempty state disagreed with the fills.
    SettlementTailMismatch,
}

impl From<GeneralV2RuntimeError> for CandidateBuilderErrorV1 {
    fn from(value: GeneralV2RuntimeError) -> Self {
        Self::Runtime(value)
    }
}

impl From<CodecError> for CandidateBuilderErrorV1 {
    fn from(value: CodecError) -> Self {
        Self::Contract(value)
    }
}

impl From<LayoutError> for CandidateBuilderErrorV1 {
    fn from(value: LayoutError) -> Self {
        Self::Layout(value)
    }
}

impl From<ProductError> for CandidateBuilderErrorV1 {
    fn from(value: ProductError) -> Self {
        Self::Product(value)
    }
}

impl From<ErrorV3> for CandidateBuilderErrorV1 {
    fn from(value: ErrorV3) -> Self {
        Self::PriceMeasure(value)
    }
}

impl From<EconomicErrorV2> for CandidateBuilderErrorV1 {
    fn from(value: EconomicErrorV2) -> Self {
        Self::Relation(value)
    }
}

pub(crate) fn relation_domain_from_account(
    account: &EconomicDomainV2AccountV1,
) -> Result<EconomicDomainV2, CandidateBuilderErrorV1> {
    account.validate()?;
    let transcript = account.transcript;
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
    domain.validate()?;
    Ok(domain)
}

/// Structurally authenticated owner-blind projection of one frozen page set.
///
/// Identity fields are private so the solver and CandidateFeed encoder cannot
/// be given a different market, epoch, or order set than the pages projected
/// into the economic book.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerBlindBookProjectionV1 {
    market_binding: MarketBindingV1,
    market: Id32,
    epoch: Id32,
    order_set: Id32,
    domain: EconomicDomainV2,
    economic_domain_digest: Id32,
    price_grid_id: Id32,
    realm: Id32,
    book: EconomicBookV2,
    order_membership: [FrozenOrderMembershipV1; MAX_ORDERS],
}

/// One SBF-authenticated General OrderPage V5 presented to the pure builder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOrderPageInputV5<'a> {
    /// Program-owned page PDA authenticated by the live adapter.
    pub account: Id32,
    /// Exact hostile tag-8/version-5 page body.
    pub body: &'a [u8],
}

/// Frozen General book successor carrying immutable placement generations.
///
/// The inner owner-blind book remains the sole RelationV2 projection. This
/// successor adds only page/PDA coordinates and the Position generation frozen
/// by action 3, so settlement can derive Reservation and General Position
/// identities without rereading every mutable account on every slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerBlindBookProjectionV2 {
    base: OwnerBlindBookProjectionV1,
    page_accounts: [Id32; MAX_ORDER_PAGES],
    page_count: u8,
    position_generations: [u64; MAX_ORDERS],
    order_page_accounts: [Id32; MAX_ORDERS],
    order_page_indices: [u16; MAX_ORDERS],
}

impl OwnerBlindBookProjectionV2 {
    /// Exact RelationV2 owner-blind projection over the same V5 pages.
    pub const fn base(&self) -> &OwnerBlindBookProjectionV1 {
        &self.base
    }

    /// Number of exact frozen V5 pages in this projection.
    pub const fn page_count(&self) -> u8 {
        self.page_count
    }

    /// Program-owned page identity at one canonical page index.
    pub fn page_account(&self, page_index: u16) -> Option<Id32> {
        if usize::from(page_index) < usize::from(self.page_count) {
            Some(self.page_accounts[usize::from(page_index)])
        } else {
            None
        }
    }

    /// Typed page PDA tuple the adapter must rederive.
    pub fn page_seed(&self, page_index: u16) -> Option<GeneralOrderPageSeedTupleV5> {
        self.page_account(page_index)?;
        GeneralOrderPageSeedTupleV5::new(self.base.epoch, page_index).ok()
    }

    /// Immutable Position generation frozen beside one dense live order.
    pub fn position_generation(&self, order_index: u8) -> Option<u64> {
        if order_index < self.base.book.len {
            Some(self.position_generations[usize::from(order_index)])
        } else {
            None
        }
    }

    /// V5 page account containing one dense live order.
    pub fn order_page_account(&self, order_index: u8) -> Option<Id32> {
        if order_index < self.base.book.len {
            Some(self.order_page_accounts[usize::from(order_index)])
        } else {
            None
        }
    }

    /// Canonical V5 page index containing one dense live order.
    pub fn order_page_index(&self, order_index: u8) -> Option<u16> {
        if order_index < self.base.book.len {
            Some(self.order_page_indices[usize::from(order_index)])
        } else {
            None
        }
    }
}

/// Original frozen order family retained for reservation authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FrozenOrderKindV1 {
    /// One sparse single-outcome order.
    Single = 1,
    /// One coefficient-vector portfolio order.
    Portfolio = 2,
}

impl FrozenOrderKindV1 {
    /// Frozen wire discriminator retained in owner/order-set transcripts.
    pub const fn code(self) -> u8 {
        match self {
            Self::Single => 1,
            Self::Portfolio => 2,
        }
    }
}

/// Checked ownership and replay identity of one live projected order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrozenOrderMembershipV1 {
    owner: Id32,
    order_id: Id32,
    generation: u64,
    kind: FrozenOrderKindV1,
    slot: OrderSlot,
}

impl FrozenOrderMembershipV1 {
    const EMPTY: Self = Self {
        owner: Id32::ZERO,
        order_id: Id32::ZERO,
        generation: 0,
        kind: FrozenOrderKindV1::Single,
        slot: OrderSlot::Empty,
    };

    /// Semantic Position owner retained from the authenticated page record.
    pub const fn owner(&self) -> Id32 {
        self.owner
    }

    /// Canonical positional order identity retained from the page record.
    pub const fn order_id(&self) -> Id32 {
        self.order_id
    }

    /// Replay generation retained from the page record.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Original single or portfolio family.
    pub const fn kind(&self) -> FrozenOrderKindV1 {
        self.kind
    }

    pub(crate) const fn slot(&self) -> &OrderSlot {
        &self.slot
    }
}

impl OwnerBlindBookProjectionV1 {
    /// Exact immutable MarketBinding body used by the projection.
    pub const fn market_binding(&self) -> &MarketBindingV1 {
        &self.market_binding
    }

    /// Authenticated parent market identity.
    pub const fn market(&self) -> Id32 {
        self.market
    }

    /// Authenticated parent epoch identity.
    pub const fn epoch(&self) -> Id32 {
        self.epoch
    }

    /// Authenticated frozen order-set identity.
    pub const fn order_set(&self) -> Id32 {
        self.order_set
    }

    /// Exact RelationV2 domain used while projecting order semantics.
    pub const fn domain(&self) -> &EconomicDomainV2 {
        &self.domain
    }

    /// Canonical digest of the complete persisted EconomicDomain transcript.
    pub const fn economic_domain_digest(&self) -> Id32 {
        self.economic_domain_digest
    }

    /// Authenticated PriceGrid content identity used for page admission.
    pub const fn price_grid_id(&self) -> Id32 {
        self.price_grid_id
    }

    /// Realm identity carried by the authenticated PriceGrid.
    pub const fn realm(&self) -> Id32 {
        self.realm
    }

    /// Canonical RelationV2 projection, with owner/replay labels absent.
    pub const fn book(&self) -> &EconomicBookV2 {
        &self.book
    }

    /// Checked membership row at one dense live-order index.
    pub fn order_membership(&self, index: u8) -> Option<&FrozenOrderMembershipV1> {
        if index < self.book.len {
            Some(&self.order_membership[usize::from(index)])
        } else {
            None
        }
    }
}

/// Project a structurally authenticated frozen page set into RelationV2's one
/// owner-blind economic book and retain its checked General identities.
///
/// Owner and replay-generation bytes are excluded from the RelationV2 book but
/// retained behind the projection's private membership table for the separate
/// owner-aware settlement join. Stored positional order IDs remain the
/// canonical sorted identities, so tombstones leave gaps without renumbering
/// live records. Single-Egg orders become sparse coefficient vectors;
/// portfolio cash bounds are converted once to exact price units by checked
/// multiplication with the domain scale.
#[allow(clippy::too_many_arguments)]
pub fn project_owner_blind_book_v2(
    pages: &[&[u8]],
    expected_order_set: Id32,
    economic_domain_account: &EconomicDomainV2AccountV1,
    market_binding: &MarketBindingV1,
    price_grid: &PriceGridAccount,
) -> Result<OwnerBlindBookProjectionV1, CandidateBuilderErrorV1> {
    economic_domain_account.validate()?;
    market_binding.validate()?;
    price_grid.validate()?;
    let domain = relation_domain_from_account(economic_domain_account)?;
    let domain_digest =
        economic_domain_digest_v2(&CanonicalSha256, economic_domain_account.transcript)?;
    if expected_order_set.is_zero()
        || domain.price_scale != price_grid.price_scale
        || market_binding.price_scale != domain.price_scale
        || market_binding.outcome_count != domain.outcome_count
        || market_binding.relation_version != domain.relation_version
        || market_binding.market_instance_v2_id.bytes() != domain.market_semantics_digest
        || market_binding.relation_policy_id.bytes() != domain.relation_policy_digest
        || market_binding.price_measure_policy_v1_id.bytes() != domain.price_policy_digest
        || market_binding.native_claim_basis_id
            != economic_domain_account.transcript.native_claim_basis_id
        || market_binding.relation_policy_id != relation_v2_policy_id_v1()?
        || market_binding.score_policy_id != score_v2_q_policy_id_v1()?
    {
        return Err(CandidateBuilderErrorV1::BindingMismatch);
    }
    let observed_order_set = stream::verify_page_set(pages)?;
    if observed_order_set.bytes() != expected_order_set.bytes() {
        return Err(CandidateBuilderErrorV1::BindingMismatch);
    }

    let mut page = 0usize;
    while page < pages.len() {
        let header = stream::verify_page_on_grid(pages[page], price_grid)?;
        if header.market.bytes() != market_binding.market.bytes()
            || header.epoch.bytes() != economic_domain_account.epoch.bytes()
            || header.order_set.bytes() != expected_order_set.bytes()
        {
            return Err(CandidateBuilderErrorV1::BindingMismatch);
        }
        page += 1;
    }

    let mut book = EconomicBookV2::empty();
    let mut order_membership = [FrozenOrderMembershipV1::EMPTY; MAX_ORDERS];
    page = 0;
    while page < pages.len() {
        let header = stream::OrderPageHeader::decode(pages[page])?;
        let mut cursor = stream::OrderSlotCursor::new(pages[page])?;
        let mut slot_index = 0usize;
        while slot_index < usize::from(header.order_count) {
            let slot = cursor
                .next_slot()
                .ok_or(CandidateBuilderErrorV1::Layout(LayoutError::Truncated))??;
            if let Some((order, membership)) = project_owner_blind_slot(slot, &domain)? {
                let at = usize::from(book.len);
                if at >= MAX_ORDERS {
                    return Err(CandidateBuilderErrorV1::Relation(
                        EconomicErrorV2::TooManyOrders,
                    ));
                }
                book.orders[at] = order;
                order_membership[at] = membership;
                book.len = book
                    .len
                    .checked_add(1)
                    .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
            }
            slot_index += 1;
        }
        page += 1;
    }
    book.validate(&domain)?;
    Ok(OwnerBlindBookProjectionV1 {
        market_binding: *market_binding,
        market: market_binding.market,
        epoch: economic_domain_account.epoch,
        order_set: expected_order_set,
        domain,
        economic_domain_digest: domain_digest,
        price_grid_id: Id32::new(price_grid.grid.bytes())?,
        realm: Id32::new(price_grid.realm.bytes())?,
        book,
        order_membership,
    })
}

/// Project canonical General OrderPage V5 bytes into the owner-blind book and
/// retain each placement's immutable Position generation.
///
/// V5 is a breaking page successor: this function accepts no historical V4
/// page. The complete page set is verified under its generation-bearing digest
/// and canonical order-set fold before any RelationV2 order is emitted. Page
/// account PDAs remain an adapter obligation, represented by exact typed seed
/// tuples in the returned private projection.
#[allow(clippy::too_many_arguments)]
pub fn project_owner_blind_book_v3(
    pages: &[GeneralOrderPageInputV5<'_>],
    expected_order_set: Id32,
    economic_domain_account: &EconomicDomainV2AccountV1,
    market_binding: &MarketBindingV1,
    price_grid: &PriceGridAccount,
) -> Result<OwnerBlindBookProjectionV2, CandidateBuilderErrorV1> {
    economic_domain_account.validate()?;
    market_binding.validate()?;
    price_grid.validate()?;
    let domain = relation_domain_from_account(economic_domain_account)?;
    let domain_digest =
        economic_domain_digest_v2(&CanonicalSha256, economic_domain_account.transcript)?;
    if pages.is_empty()
        || pages.len() > MAX_ORDER_PAGES
        || expected_order_set.is_zero()
        || domain.price_scale != price_grid.price_scale
        || market_binding.price_scale != domain.price_scale
        || market_binding.outcome_count != domain.outcome_count
        || market_binding.relation_version != domain.relation_version
        || market_binding.market_instance_v2_id.bytes() != domain.market_semantics_digest
        || market_binding.relation_policy_id.bytes() != domain.relation_policy_digest
        || market_binding.price_measure_policy_v1_id.bytes() != domain.price_policy_digest
        || market_binding.native_claim_basis_id
            != economic_domain_account.transcript.native_claim_basis_id
        || market_binding.relation_policy_id != relation_v2_policy_id_v1()?
        || market_binding.score_policy_id != score_v2_q_policy_id_v1()?
    {
        return Err(CandidateBuilderErrorV1::BindingMismatch);
    }

    let mut bodies: [&[u8]; MAX_ORDER_PAGES] = [&[]; MAX_ORDER_PAGES];
    let mut page_accounts = [Id32::ZERO; MAX_ORDER_PAGES];
    let mut page = 0usize;
    while page < pages.len() {
        let input = pages[page];
        if input.account.is_zero()
            || input.account == market_binding.market
            || input.account == economic_domain_account.epoch
            || input.account == expected_order_set
        {
            return Err(CandidateBuilderErrorV1::BindingMismatch);
        }
        let mut prior = 0usize;
        while prior < page {
            if page_accounts[prior] == input.account {
                return Err(CandidateBuilderErrorV1::BindingMismatch);
            }
            prior += 1;
        }
        bodies[page] = input.body;
        page_accounts[page] = input.account;
        page += 1;
    }
    let observed_order_set = verify_page_set_v5_streaming(&bodies[..pages.len()])?;
    if observed_order_set.bytes() != expected_order_set.bytes() {
        return Err(CandidateBuilderErrorV1::BindingMismatch);
    }

    let mut book = EconomicBookV2::empty();
    let mut order_membership = [FrozenOrderMembershipV1::EMPTY; MAX_ORDERS];
    let mut position_generations = [0u64; MAX_ORDERS];
    let mut order_page_accounts = [Id32::ZERO; MAX_ORDERS];
    let mut order_page_indices = [0u16; MAX_ORDERS];
    page = 0;
    while page < pages.len() {
        let header = verify_page_v5(pages[page].body)?;
        if header.market.bytes() != market_binding.market.bytes()
            || header.epoch.bytes() != economic_domain_account.epoch.bytes()
            || header.order_set.bytes() != expected_order_set.bytes()
            || usize::from(header.page_index) != page
            || GeneralOrderPageSeedTupleV5::new(
                economic_domain_account.epoch,
                header.page_index,
            )
            .is_err()
        {
            return Err(CandidateBuilderErrorV1::BindingMismatch);
        }
        let mut cursor = OrderSlotCursorV5::new(pages[page].body)?;
        let mut slot_index = 0usize;
        while slot_index < usize::from(header.order_count) {
            let verified = cursor
                .next_slot()
                .ok_or(CandidateBuilderErrorV1::Layout(LayoutError::Truncated))??;
            match verified.slot {
                OrderSlot::Single(record) => {
                    price_grid.tick_of(record.limit)?;
                }
                OrderSlot::Portfolio(record) => {
                    record.validate_on_scale(price_grid.price_scale)?;
                }
                OrderSlot::Tombstone(_) => {}
                OrderSlot::Empty => {
                    return Err(CandidateBuilderErrorV1::Layout(LayoutError::ZeroIdentity))
                }
            }
            if let Some((order, membership)) = project_owner_blind_slot(verified.slot, &domain)? {
                let at = usize::from(book.len);
                if verified.position_generation == 0 {
                    return Err(CandidateBuilderErrorV1::BindingMismatch);
                }
                if at >= MAX_ORDERS {
                    return Err(CandidateBuilderErrorV1::Relation(
                        EconomicErrorV2::TooManyOrders,
                    ));
                }
                book.orders[at] = order;
                order_membership[at] = membership;
                position_generations[at] = verified.position_generation;
                order_page_accounts[at] = pages[page].account;
                order_page_indices[at] = header.page_index;
                book.len = book
                    .len
                    .checked_add(1)
                    .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
            }
            slot_index += 1;
        }
        page += 1;
    }
    book.validate(&domain)?;
    let base = OwnerBlindBookProjectionV1 {
        market_binding: *market_binding,
        market: market_binding.market,
        epoch: economic_domain_account.epoch,
        order_set: expected_order_set,
        domain,
        economic_domain_digest: domain_digest,
        price_grid_id: Id32::new(price_grid.grid.bytes())?,
        realm: Id32::new(price_grid.realm.bytes())?,
        book,
        order_membership,
    };
    Ok(OwnerBlindBookProjectionV2 {
        base,
        page_accounts,
        page_count: u8::try_from(pages.len())
            .map_err(|_| CandidateBuilderErrorV1::ArithmeticOverflow)?,
        position_generations,
        order_page_accounts,
        order_page_indices,
    })
}

/// Search exact finite atom measures and owner-blind fills, returning the best
/// valid submitted candidate encountered under ScoreV2-Q.
///
/// The authoritative V3 and RelationV2 checkers validate every retained result.
/// Search order is supplied proposals, sampled singleton measures, then sampled
/// primitive pairs in coordinate/denominator/mass order. Fill search visits the
/// empty candidate, maximal exact buy/sell ratio pairs, then the declared
/// zero/minimum/full Cartesian family. This is a deterministic builder, not an
/// optimality algorithm.
pub fn build_best_valid_submitted_candidate_v1(
    inputs: SmoothDirectBuilderInputsV1<'_>,
    supplied_atom_proposals: &[QuantizedAtomProposalV1],
    plan: BoundedSearchPlanV1,
) -> Result<BuiltDirectCandidateV1, CandidateBuilderErrorV1> {
    plan.validate()?;
    let context = PreparedBuilderContextV1::new(inputs)?;
    if plan.maximum_pair_denominator > context.price_measure_policy.maximum_witness_denominator {
        return Err(CandidateBuilderErrorV1::InvalidSearchPlan);
    }
    let mut report = CandidateSearchReportV1::empty(plan);
    report.fill_order_limit = min(plan.fill_order_limit, context.book.len);
    let mut best: Option<BuiltDirectCandidateV1> = None;
    let mut saw_grid_price = false;

    let mut proposal = 0usize;
    while proposal < supplied_atom_proposals.len() {
        if report.atom_measures_considered >= plan.maximum_atom_measures {
            report.truncate(SEARCH_TRUNCATED_ATOM_MEASURES_V1);
            break;
        }
        report.supplied_atom_measures_considered = report
            .supplied_atom_measures_considered
            .checked_add(1)
            .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
        consider_atom_measure(
            &context,
            supplied_atom_proposals[proposal],
            plan,
            &mut report,
            &mut saw_grid_price,
            &mut best,
        )?;
        proposal += 1;
    }

    let (coordinates, coordinate_count, full_interval) = sample_coordinates(&context.basis, plan)?;
    report.sampled_coordinate_count = coordinate_count;
    report.full_integer_coordinate_interval = full_interval;

    let mut coordinate = 0usize;
    while coordinate < usize::from(coordinate_count) {
        if report.atom_measures_considered >= plan.maximum_atom_measures {
            report.truncate(SEARCH_TRUNCATED_ATOM_MEASURES_V1);
            break;
        }
        consider_atom_measure(
            &context,
            QuantizedAtomProposalV1::singleton(coordinates[coordinate]),
            plan,
            &mut report,
            &mut saw_grid_price,
            &mut best,
        )?;
        coordinate += 1;
    }

    if plan.maximum_pair_denominator >= 2 {
        let mut left = 0usize;
        'pairs: while left < usize::from(coordinate_count) {
            let mut right = left + 1;
            while right < usize::from(coordinate_count) {
                let mut denominator = 2u64;
                while denominator <= plan.maximum_pair_denominator {
                    let mut left_mass = 1u64;
                    while left_mass < denominator {
                        if gcd_u64(left_mass, denominator) == 1 {
                            if report.atom_measures_considered >= plan.maximum_atom_measures {
                                report.truncate(SEARCH_TRUNCATED_ATOM_MEASURES_V1);
                                break 'pairs;
                            }
                            let atoms = QuantizedAtomProposalV1::primitive_pair(
                                coordinates[left],
                                coordinates[right],
                                left_mass,
                                denominator,
                            )?;
                            consider_atom_measure(
                                &context,
                                atoms,
                                plan,
                                &mut report,
                                &mut saw_grid_price,
                                &mut best,
                            )?;
                        }
                        left_mass += 1;
                    }
                    if denominator == plan.maximum_pair_denominator {
                        break;
                    }
                    denominator += 1;
                }
                right += 1;
            }
            left += 1;
        }
    }

    let mut selected = match best {
        Some(candidate) => candidate,
        None if saw_grid_price => return Err(CandidateBuilderErrorV1::NoValidSubmittedCandidate),
        None => return Err(CandidateBuilderErrorV1::NoCoherentGridPrice),
    };
    selected.search_report = report;
    Ok(selected)
}

#[derive(Clone, Copy, Debug)]
struct PreparedBuilderContextV1<'a> {
    candidate_feed_identity: Id32,
    market_binding: MarketBindingV1,
    market: Id32,
    epoch: Id32,
    order_set: Id32,
    domain: EconomicDomainV2,
    domain_digest: Id32,
    terms_id: [u8; 32],
    basis_digest: Id32,
    coordinate_domain_min: u128,
    coordinate_domain_max: u128,
    price_measure_policy_id: Id32,
    basis: QuantizedBasisProjectionV1,
    native_basis: &'a NativeClaimBasisV1,
    price_measure_policy: &'a PriceMeasurePolicyV1,
    price_grid: &'a PriceGridAccount,
    book: &'a EconomicBookV2,
}

impl QuantizedBasisProjectionV1 {
    fn coordinate_bounds(&self) -> (u128, u128) {
        match self {
            Self::DegreeZero(table) => (table.domain_min, table.domain_max),
            Self::Smooth(basis) => (
                basis.knots[0],
                basis.knots[usize::from(basis.knot_count) - 1],
            ),
        }
    }

    const fn degree(&self) -> u8 {
        match self {
            Self::DegreeZero(_) => 0,
            Self::Smooth(basis) => basis.degree,
        }
    }

    const fn payout_denominator(&self) -> u64 {
        match self {
            Self::DegreeZero(table) => table.payout_denominator,
            Self::Smooth(basis) => basis.denominator,
        }
    }

    fn evaluate(&self, coordinate: u128) -> Result<[u64; MAX_OUTCOMES], CandidateBuilderErrorV1> {
        match self {
            Self::DegreeZero(table) => Ok(table.evaluate(coordinate)?.weights),
            Self::Smooth(basis) => basis
                .evaluate(coordinate)
                .map(|weights| weights.weights)
                .map_err(|_| CandidateBuilderErrorV1::InvalidAtomProposal),
        }
    }
}

impl<'a> PreparedBuilderContextV1<'a> {
    fn new(inputs: SmoothDirectBuilderInputsV1<'a>) -> Result<Self, CandidateBuilderErrorV1> {
        if inputs.candidate_feed_identity.is_zero() {
            return Err(CandidateBuilderErrorV1::Contract(CodecError::ZeroIdentity));
        }
        inputs.economic_domain_account.validate()?;
        inputs.market_binding.validate()?;
        inputs.price_grid.validate()?;
        inputs.market_instance.validate_bindings(
            inputs.product_template,
            inputs.native_basis,
            inputs.price_measure_policy,
            inputs.genesis,
        )?;
        let basis = if inputs.native_basis.basis_degree == 0 {
            QuantizedBasisProjectionV1::DegreeZero(
                inputs
                    .price_measure_policy
                    .project_degree_zero_table(inputs.native_basis, inputs.genesis)?,
            )
        } else {
            QuantizedBasisProjectionV1::Smooth(inputs.price_measure_policy.project_smooth_basis(
                inputs.native_basis,
                inputs.genesis,
                inputs.authenticated_edge_policy,
            )?)
        };
        let basis_digest = Id32::new(inputs.native_basis.id()?.bytes())?;
        let price_measure_policy_id = Id32::new(inputs.price_measure_policy.id()?.bytes())?;
        let market_instance_id = inputs.market_instance.id()?.bytes();
        let genesis_id = inputs.genesis.id()?.bytes();
        let relation_policy_id = relation_v2_policy_id_v1()?;
        let score_policy_id = score_v2_q_policy_id_v1()?;
        let transcript = inputs.economic_domain_account.transcript;
        let domain_digest = economic_domain_digest_v2(&CanonicalSha256, transcript)?;
        let domain = relation_domain_from_account(inputs.economic_domain_account)?;
        if inputs.market_binding.market_genesis_profile_v2_id.bytes() != genesis_id
            || inputs.market_binding.market_instance_v2_id.bytes() != market_instance_id
            || inputs.market_binding.relation_policy_id != relation_policy_id
            || inputs.market_binding.score_policy_id != score_policy_id
            || inputs.genesis.relation_policy_id.bytes() != relation_policy_id.bytes()
            || inputs.genesis.score_policy_id.bytes() != score_policy_id.bytes()
            || inputs.market_binding.price_measure_policy_v1_id != price_measure_policy_id
            || inputs.market_binding.native_claim_basis_id != basis_digest
            || inputs.market_binding.price_scale != inputs.price_grid.price_scale
            || inputs.market_binding.price_scale != transcript.price_scale
            || inputs.market_binding.relation_version != transcript.relation_version
            || inputs.market_binding.outcome_count != transcript.outcome_count
            || inputs.market_binding.outcome_count != inputs.native_basis.outcome_count
            || inputs.market_binding.basis_degree != inputs.native_basis.basis_degree
            || inputs.market_binding.candidate_kind_mask & 0b01 == 0
            || inputs.book_projection.market_binding != *inputs.market_binding
            || inputs.book_projection.market != inputs.market_binding.market
            || inputs.book_projection.epoch != inputs.economic_domain_account.epoch
            || inputs.book_projection.domain != domain
            || inputs.book_projection.economic_domain_digest != domain_digest
            || inputs.book_projection.price_grid_id.bytes() != inputs.price_grid.grid.bytes()
            || inputs.book_projection.realm.bytes() != inputs.price_grid.realm.bytes()
            || transcript.market_instance_v2_id.bytes() != market_instance_id
            || transcript.relation_policy_id != relation_policy_id
            || transcript.price_measure_policy_v1_id != price_measure_policy_id
            || transcript.native_claim_basis_id != basis_digest
            || transcript.coordinate_domain_min != inputs.genesis.coordinate_domain_min
            || transcript.coordinate_domain_max != inputs.genesis.coordinate_domain_max
            || inputs.price_grid.grid.bytes() != inputs.genesis.price_grid_id.bytes()
            || inputs.price_grid.realm.bytes() != inputs.genesis.realm_id.bytes()
            || ((2..=3).contains(&basis.degree())
                && inputs.market_binding.price_scale != basis.payout_denominator())
        {
            return Err(CandidateBuilderErrorV1::BindingMismatch);
        }
        inputs.book_projection.book.validate(&domain)?;
        Ok(Self {
            candidate_feed_identity: inputs.candidate_feed_identity,
            market_binding: *inputs.market_binding,
            market: inputs.book_projection.market,
            epoch: inputs.book_projection.epoch,
            order_set: inputs.book_projection.order_set,
            domain,
            domain_digest,
            terms_id: genesis_id,
            basis_digest,
            coordinate_domain_min: inputs.genesis.coordinate_domain_min,
            coordinate_domain_max: inputs.genesis.coordinate_domain_max,
            price_measure_policy_id,
            basis,
            native_basis: inputs.native_basis,
            price_measure_policy: inputs.price_measure_policy,
            price_grid: inputs.price_grid,
            book: &inputs.book_projection.book,
        })
    }
}

pub(crate) fn project_owner_blind_slot(
    slot: OrderSlot,
    domain: &EconomicDomainV2,
) -> Result<Option<(EconomicOrderV2, FrozenOrderMembershipV1)>, CandidateBuilderErrorV1> {
    let mut coefficients = [0u64; MAX_OUTCOMES];
    let (order, membership) = match slot {
        OrderSlot::Empty => return Err(CandidateBuilderErrorV1::Layout(LayoutError::ZeroIdentity)),
        OrderSlot::Tombstone(_) => return Ok(None),
        OrderSlot::Single(record) => {
            if record.outcome >= domain.outcome_count {
                return Err(CandidateBuilderErrorV1::BindingMismatch);
            }
            coefficients[usize::from(record.outcome)] = 1;
            (
                EconomicOrderV2 {
                    order_id: record.order_id.bytes(),
                    side: side(record.side)?,
                    coefficients,
                    quantity: record.quantity,
                    minimum_fill: record.minimum_fill,
                    partial_policy: partial(record.flags)?,
                    expiry_epoch: record.expiry_epoch,
                    limit_value_price_units_per_unit: u128::from(record.limit),
                },
                FrozenOrderMembershipV1 {
                    owner: Id32::new(record.owner.bytes())?,
                    order_id: Id32::new(record.order_id.bytes())?,
                    generation: record.generation,
                    kind: FrozenOrderKindV1::Single,
                    slot,
                },
            )
        }
        OrderSlot::Portfolio(record) => {
            if record.active_len > domain.outcome_count {
                return Err(CandidateBuilderErrorV1::BindingMismatch);
            }
            let limit = u128::from(record.limit_collateral_per_lot)
                .checked_mul(u128::from(domain.price_scale))
                .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
            (
                EconomicOrderV2 {
                    order_id: record.order_id.bytes(),
                    side: side(record.side)?,
                    coefficients: record.coefficients,
                    quantity: record.lots,
                    minimum_fill: record.minimum_fill_lots,
                    partial_policy: partial(record.flags)?,
                    expiry_epoch: record.expiry_epoch,
                    limit_value_price_units_per_unit: limit,
                },
                FrozenOrderMembershipV1 {
                    owner: Id32::new(record.owner.bytes())?,
                    order_id: Id32::new(record.order_id.bytes())?,
                    generation: record.generation,
                    kind: FrozenOrderKindV1::Portfolio,
                    slot,
                },
            )
        }
    };
    Ok(Some((order, membership)))
}

fn side(value: u8) -> Result<Side, CandidateBuilderErrorV1> {
    match value {
        0 => Ok(Side::Buy),
        1 => Ok(Side::Sell),
        _ => Err(CandidateBuilderErrorV1::Layout(LayoutError::InvalidEnum)),
    }
}

fn partial(flags: u8) -> Result<PartialPolicy, CandidateBuilderErrorV1> {
    match flags {
        0 => Ok(PartialPolicy::Allow),
        1 => Ok(PartialPolicy::AllOrNone),
        _ => Err(CandidateBuilderErrorV1::Layout(LayoutError::InvalidEnum)),
    }
}

fn sample_coordinates(
    basis: &QuantizedBasisProjectionV1,
    plan: BoundedSearchPlanV1,
) -> Result<([u128; MAX_BUILDER_COORDINATES_V1], u8, bool), CandidateBuilderErrorV1> {
    let (first, last) = basis.coordinate_bounds();
    let capacity = usize::from(plan.maximum_coordinates);
    let mut output = [0u128; MAX_BUILDER_COORDINATES_V1];
    let mut count = 0usize;
    let mut coordinate = first;
    while count < capacity {
        output[count] = coordinate;
        count += 1;
        if coordinate == last {
            break;
        }
        let remaining = last - coordinate;
        if plan.coordinate_stride >= remaining {
            if count < capacity {
                coordinate = last;
                continue;
            }
            break;
        }
        coordinate += plan.coordinate_stride;
    }
    let interval_width = last
        .checked_sub(first)
        .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
    let integer_count = interval_width
        .checked_add(1)
        .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
    let count = u8::try_from(count).map_err(|_| CandidateBuilderErrorV1::ArithmeticOverflow)?;
    let full_interval = plan.coordinate_stride == 1 && integer_count == u128::from(count);
    Ok((output, count, full_interval))
}

fn consider_atom_measure(
    context: &PreparedBuilderContextV1<'_>,
    atoms: QuantizedAtomProposalV1,
    plan: BoundedSearchPlanV1,
    report: &mut CandidateSearchReportV1,
    saw_grid_price: &mut bool,
    best: &mut Option<BuiltDirectCandidateV1>,
) -> Result<(), CandidateBuilderErrorV1> {
    report.atom_measures_considered = report
        .atom_measures_considered
        .checked_add(1)
        .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
    validate_atom_proposal(context, atoms)?;
    let Some(price) = derive_exact_price(context, atoms)? else {
        return Ok(());
    };
    report.exact_price_vectors = report
        .exact_price_vectors
        .checked_add(1)
        .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
    let mut outcome = 0usize;
    while outcome < usize::from(context.domain.outcome_count) {
        if context.price_grid.tick_of(price.prices[outcome]).is_err() {
            return Ok(());
        }
        outcome += 1;
    }
    *saw_grid_price = true;
    report.grid_price_vectors = report
        .grid_price_vectors
        .checked_add(1)
        .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;

    let candidate_price_digest = price_semantics_digest_v2(&context.domain, &price.prices)?;
    let witness_body = QuantizedWitnessBodyV1 {
        schema_version: PRICE_MEASURE_WITNESS_SCHEMA_V3,
        quantized_semantics_version: QUANTIZED_PRICE_MEASURE_SEMANTICS_V1,
        candidate_feed: context.candidate_feed_identity,
        economic_domain_digest: context.domain_digest,
        basis_digest: context.basis_digest,
        candidate_price_digest: Id32::new(candidate_price_digest)?,
        price_scale: price.price_scale,
        basis_degree: price.basis_degree,
        outcome_count: price.native_outcome_count,
        atom_count: atoms.atom_count,
        common_denominator: atoms.common_denominator,
        prices: price.prices,
        atom_coordinates: atoms.atom_coordinates,
        atom_masses: atoms.atom_masses,
    };
    let body_digest = witness_body.digest()?;
    let bindings = AdapterBindingsV3 {
        candidate_feed: context.candidate_feed_identity.bytes(),
        relation_domain_digest: context.domain_digest.bytes(),
        basis_digest: context.basis_digest.bytes(),
        candidate_price_digest,
        observed_body_digest: body_digest.bytes(),
    };
    let witness = QuantizedAtomWitnessV3 {
        schema_version: PRICE_MEASURE_WITNESS_SCHEMA_V3,
        quantized_semantics_version: QUANTIZED_PRICE_MEASURE_SEMANTICS_V1,
        candidate_feed: bindings.candidate_feed,
        relation_domain_digest: bindings.relation_domain_digest,
        basis_digest: bindings.basis_digest,
        candidate_price_digest: bindings.candidate_price_digest,
        body_digest: bindings.observed_body_digest,
        basis_degree: price.basis_degree,
        native_outcome_count: price.native_outcome_count,
        atom_count: atoms.atom_count,
        common_denominator: atoms.common_denominator,
        atom_coordinates: atoms.atom_coordinates,
        atom_masses: atoms.atom_masses,
    };
    context.price_measure_policy.validate_witness_contract(
        context.native_basis,
        &price,
        &witness,
        context.price_grid.price_scale,
    )?;
    let price_measure = match &context.basis {
        QuantizedBasisProjectionV1::DegreeZero(table) => {
            verify_quantized_price_measure_v3_degree_zero(&bindings, table, &price, &witness)?
        }
        QuantizedBasisProjectionV1::Smooth(basis) => {
            verify_quantized_price_measure_v3_smooth(&bindings, basis, &price, &witness)?
        }
    };
    if let QuantizedBasisProjectionV1::Smooth(basis) = context.basis {
        if (2..=3).contains(&basis.degree) {
            crate::verify_exact_smooth_atom_mixture_v1(
                context.market,
                context.terms_id,
                context.basis_digest,
                candidate_price_digest,
                context.coordinate_domain_min,
                context.coordinate_domain_max,
                basis,
                price.prices,
                atoms.atom_count,
                atoms.common_denominator,
                atoms.atom_coordinates,
                atoms.atom_masses,
            )?;
        }
    }
    search_fills(
        context,
        price,
        atoms,
        witness_body,
        price_measure,
        plan,
        report,
        best,
    )
}

fn validate_atom_proposal(
    context: &PreparedBuilderContextV1<'_>,
    atoms: QuantizedAtomProposalV1,
) -> Result<(), CandidateBuilderErrorV1> {
    let count = usize::from(atoms.atom_count);
    if count == 0
        || count > usize::from(context.domain.outcome_count)
        || count > MAX_QUANTIZED_ATOMS
        || atoms.common_denominator == 0
        || atoms.common_denominator > context.price_measure_policy.maximum_witness_denominator
    {
        return Err(CandidateBuilderErrorV1::InvalidAtomProposal);
    }
    let (first, last) = context.basis.coordinate_bounds();
    let mut sum = 0u64;
    let mut divisor = atoms.common_denominator;
    let mut atom = 0usize;
    while atom < MAX_QUANTIZED_ATOMS {
        if atom < count {
            if atoms.atom_coordinates[atom] < first
                || atoms.atom_coordinates[atom] > last
                || (atom != 0 && atoms.atom_coordinates[atom] <= atoms.atom_coordinates[atom - 1])
                || atoms.atom_masses[atom] == 0
            {
                return Err(CandidateBuilderErrorV1::InvalidAtomProposal);
            }
            sum = sum
                .checked_add(atoms.atom_masses[atom])
                .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
            divisor = gcd_u64(divisor, atoms.atom_masses[atom]);
        } else if atoms.atom_coordinates[atom] != 0 || atoms.atom_masses[atom] != 0 {
            return Err(CandidateBuilderErrorV1::InvalidAtomProposal);
        }
        atom += 1;
    }
    if sum != atoms.common_denominator || divisor != 1 {
        return Err(CandidateBuilderErrorV1::InvalidAtomProposal);
    }
    Ok(())
}

fn derive_exact_price(
    context: &PreparedBuilderContextV1<'_>,
    atoms: QuantizedAtomProposalV1,
) -> Result<Option<PriceVectorV3>, CandidateBuilderErrorV1> {
    let mut accumulators = [0u128; MAX_OUTCOMES];
    let mut atom = 0usize;
    while atom < usize::from(atoms.atom_count) {
        let weights = context.basis.evaluate(atoms.atom_coordinates[atom])?;
        let mut outcome = 0usize;
        while outcome < usize::from(context.domain.outcome_count) {
            let term = u128::from(weights[outcome])
                .checked_mul(u128::from(atoms.atom_masses[atom]))
                .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
            accumulators[outcome] = accumulators[outcome]
                .checked_add(term)
                .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
            outcome += 1;
        }
        atom += 1;
    }
    let denominator = u128::from(context.basis.payout_denominator())
        .checked_mul(u128::from(atoms.common_denominator))
        .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
    let mut prices = [0u64; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < usize::from(context.domain.outcome_count) {
        let Some(value) = mul_div_exact_u64(
            accumulators[outcome],
            context.domain.price_scale,
            denominator,
        )?
        else {
            return Ok(None);
        };
        prices[outcome] = value;
        outcome += 1;
    }
    Ok(Some(PriceVectorV3 {
        basis_degree: context.basis.degree(),
        native_outcome_count: context.domain.outcome_count,
        price_scale: context.domain.price_scale,
        prices,
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FillOptionsV1 {
    values: [u64; 3],
    len: u8,
}

impl FillOptionsV1 {
    const ZERO: Self = Self {
        values: [0; 3],
        len: 1,
    };
}

#[allow(clippy::too_many_arguments)]
fn search_fills(
    context: &PreparedBuilderContextV1<'_>,
    price: PriceVectorV3,
    atoms: QuantizedAtomProposalV1,
    witness_body: QuantizedWitnessBodyV1,
    price_measure: VerifiedPriceMeasureV3,
    plan: BoundedSearchPlanV1,
    report: &mut CandidateSearchReportV1,
    best: &mut Option<BuiltDirectCandidateV1>,
) -> Result<(), CandidateBuilderErrorV1> {
    let searched_orders = min(
        usize::from(plan.fill_order_limit),
        usize::from(context.book.len),
    );
    let mut options = [FillOptionsV1::ZERO; MAX_ORDERS];
    let mut order = 0usize;
    while order < searched_orders {
        options[order] = fill_options(context.book.orders[order], &price)?;
        order += 1;
    }
    let mut digits = [0u8; MAX_ORDERS];
    let precondition = PricePreconditionV2 {
        policy_digest: context.domain.price_policy_digest,
        semantic_price_digest: witness_body.candidate_price_digest.bytes(),
        prices: price.prices,
    };
    let mut considered_for_price = 0u32;
    consider_fill_witness(
        context,
        price,
        atoms,
        witness_body,
        price_measure,
        EconomicCandidateV2::EMPTY,
        &precondition,
        report,
        best,
    )?;
    considered_for_price = 1;

    let mut buy = 0usize;
    let mut fill_search_truncated = false;
    'balanced_pairs: while buy < searched_orders {
        if context.book.orders[buy].side == Side::Buy && options[buy].len > 1 {
            let mut sell = 0usize;
            while sell < searched_orders {
                if context.book.orders[sell].side == Side::Sell && options[sell].len > 1 {
                    if let Some(candidate) = maximal_balanced_pair_candidate(
                        context.book.orders[buy],
                        buy,
                        context.book.orders[sell],
                        sell,
                        context.domain.outcome_count,
                    )? {
                        if considered_for_price >= plan.maximum_fill_witnesses_per_price {
                            report.truncate(SEARCH_TRUNCATED_FILL_WITNESSES_V1);
                            fill_search_truncated = true;
                            break 'balanced_pairs;
                        }
                        consider_fill_witness(
                            context,
                            price,
                            atoms,
                            witness_body,
                            price_measure,
                            candidate,
                            &precondition,
                            report,
                            best,
                        )?;
                        considered_for_price += 1;
                    }
                }
                sell += 1;
            }
        }
        buy += 1;
    }

    if !fill_search_truncated && increment_fill_digits(&mut digits, &options, searched_orders) {
        loop {
            if considered_for_price >= plan.maximum_fill_witnesses_per_price {
                report.truncate(SEARCH_TRUNCATED_FILL_WITNESSES_V1);
                break;
            }
            let mut candidate = EconomicCandidateV2::EMPTY;
            order = 0;
            while order < searched_orders {
                let fill = options[order].values[usize::from(digits[order])];
                candidate.fills[order] = fill;
                if context.book.orders[order].partial_policy == PartialPolicy::AllOrNone
                    && fill != 0
                {
                    candidate.honored_aon_mask |= 1u64 << order;
                }
                order += 1;
            }
            consider_fill_witness(
                context,
                price,
                atoms,
                witness_body,
                price_measure,
                candidate,
                &precondition,
                report,
                best,
            )?;
            considered_for_price += 1;
            if !increment_fill_digits(&mut digits, &options, searched_orders) {
                break;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn consider_fill_witness(
    context: &PreparedBuilderContextV1<'_>,
    price: PriceVectorV3,
    atoms: QuantizedAtomProposalV1,
    witness_body: QuantizedWitnessBodyV1,
    price_measure: VerifiedPriceMeasureV3,
    mut candidate: EconomicCandidateV2,
    precondition: &PricePreconditionV2,
    report: &mut CandidateSearchReportV1,
    best: &mut Option<BuiltDirectCandidateV1>,
) -> Result<(), CandidateBuilderErrorV1> {
    report.fill_witnesses_considered = report
        .fill_witnesses_considered
        .checked_add(1)
        .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
    let Some((split, merge)) = canonical_virtual_conversion(context, &candidate)? else {
        return Ok(());
    };
    candidate.virtual_split = split;
    candidate.virtual_merge = merge;
    let Ok(economics) =
        verify_economic_candidate_v2(&context.domain, context.book, precondition, &candidate)
    else {
        return Ok(());
    };
    report.valid_submitted_candidates = report
        .valid_submitted_candidates
        .checked_add(1)
        .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
    let replace = best
        .as_ref()
        .map(|current| economics.score.is_better_than(&current.economics.score))
        .unwrap_or(true);
    if replace {
        *best = Some(BuiltDirectCandidateV1 {
            price,
            atom_proposal: atoms,
            witness_body,
            price_measure,
            economic_candidate: candidate,
            economics,
            economic_domain_digest: context.domain_digest,
            price_measure_policy_id: context.price_measure_policy_id,
            market_binding: context.market_binding,
            market: context.market,
            epoch: context.epoch,
            order_set: context.order_set,
            order_count: context.book.len,
            search_report: *report,
        });
    }
    Ok(())
}

fn maximal_balanced_pair_candidate(
    buy: EconomicOrderV2,
    buy_index: usize,
    sell: EconomicOrderV2,
    sell_index: usize,
    outcome_count: u8,
) -> Result<Option<EconomicCandidateV2>, CandidateBuilderErrorV1> {
    let mut buy_step = 0u64;
    let mut sell_step = 0u64;
    let mut outcome = 1usize;
    while outcome < usize::from(outcome_count) {
        let (buy_sign, buy_difference) =
            signed_difference(buy.coefficients[outcome], buy.coefficients[0]);
        let (sell_sign, sell_difference) =
            signed_difference(sell.coefficients[outcome], sell.coefficients[0]);
        if buy_difference == 0 && sell_difference == 0 {
            outcome += 1;
            continue;
        }
        if buy_difference == 0 || sell_difference == 0 || buy_sign != sell_sign {
            return Ok(None);
        }
        if buy_step == 0 {
            let divisor = gcd_u64(buy_difference, sell_difference);
            buy_step = sell_difference / divisor;
            sell_step = buy_difference / divisor;
        } else if u128::from(buy_step) * u128::from(buy_difference)
            != u128::from(sell_step) * u128::from(sell_difference)
        {
            return Ok(None);
        }
        outcome += 1;
    }

    let (buy_fill, sell_fill) = if buy_step == 0 {
        (buy.quantity, sell.quantity)
    } else {
        let mut multiple = min(buy.quantity / buy_step, sell.quantity / sell_step);
        if buy.partial_policy == PartialPolicy::AllOrNone {
            if buy.quantity % buy_step != 0 {
                return Ok(None);
            }
            multiple = buy.quantity / buy_step;
        }
        if sell.partial_policy == PartialPolicy::AllOrNone {
            if sell.quantity % sell_step != 0 {
                return Ok(None);
            }
            let sell_multiple = sell.quantity / sell_step;
            if buy.partial_policy == PartialPolicy::AllOrNone && sell_multiple != multiple {
                return Ok(None);
            }
            multiple = sell_multiple;
        }
        let buy_fill = match buy_step.checked_mul(multiple) {
            Some(value) if value <= buy.quantity => value,
            _ => return Ok(None),
        };
        let sell_fill = match sell_step.checked_mul(multiple) {
            Some(value) if value <= sell.quantity => value,
            _ => return Ok(None),
        };
        (buy_fill, sell_fill)
    };
    if buy_fill == 0
        || sell_fill == 0
        || buy_fill < buy.minimum_fill
        || sell_fill < sell.minimum_fill
        || (buy.partial_policy == PartialPolicy::AllOrNone && buy_fill != buy.quantity)
        || (sell.partial_policy == PartialPolicy::AllOrNone && sell_fill != sell.quantity)
    {
        return Ok(None);
    }
    let mut candidate = EconomicCandidateV2::EMPTY;
    candidate.fills[buy_index] = buy_fill;
    candidate.fills[sell_index] = sell_fill;
    if buy.partial_policy == PartialPolicy::AllOrNone {
        candidate.honored_aon_mask |= 1u64 << buy_index;
    }
    if sell.partial_policy == PartialPolicy::AllOrNone {
        candidate.honored_aon_mask |= 1u64 << sell_index;
    }
    Ok(Some(candidate))
}

const fn signed_difference(left: u64, right: u64) -> (i8, u64) {
    if left > right {
        (1, left - right)
    } else if left < right {
        (-1, right - left)
    } else {
        (0, 0)
    }
}

fn fill_options(
    order: EconomicOrderV2,
    price: &PriceVectorV3,
) -> Result<FillOptionsV1, CandidateBuilderErrorV1> {
    let mut unit_value = 0u128;
    let mut outcome = 0usize;
    while outcome < usize::from(price.native_outcome_count) {
        unit_value = unit_value
            .checked_add(
                u128::from(order.coefficients[outcome])
                    .checked_mul(u128::from(price.prices[outcome]))
                    .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?,
            )
            .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
        outcome += 1;
    }
    let eligible = match order.side {
        Side::Buy => unit_value <= order.limit_value_price_units_per_unit,
        Side::Sell => unit_value >= order.limit_value_price_units_per_unit,
    };
    if !eligible {
        return Ok(FillOptionsV1::ZERO);
    }
    if order.partial_policy == PartialPolicy::AllOrNone {
        return Ok(FillOptionsV1 {
            values: [0, order.quantity, 0],
            len: 2,
        });
    }
    let minimum = if order.minimum_fill == 0 {
        1
    } else {
        order.minimum_fill
    };
    if minimum == order.quantity {
        Ok(FillOptionsV1 {
            values: [0, order.quantity, 0],
            len: 2,
        })
    } else {
        Ok(FillOptionsV1 {
            values: [0, minimum, order.quantity],
            len: 3,
        })
    }
}

fn increment_fill_digits(
    digits: &mut [u8; MAX_ORDERS],
    options: &[FillOptionsV1; MAX_ORDERS],
    searched_orders: usize,
) -> bool {
    let mut order = 0usize;
    while order < searched_orders {
        let next = digits[order] + 1;
        if next < options[order].len {
            digits[order] = next;
            return true;
        }
        digits[order] = 0;
        order += 1;
    }
    false
}

fn canonical_virtual_conversion(
    context: &PreparedBuilderContextV1<'_>,
    candidate: &EconomicCandidateV2,
) -> Result<Option<(u64, u64)>, CandidateBuilderErrorV1> {
    let mut buy = [0u64; MAX_OUTCOMES];
    let mut sell = [0u64; MAX_OUTCOMES];
    let mut order = 0usize;
    while order < usize::from(context.book.len) {
        let fill = candidate.fills[order];
        let target = match context.book.orders[order].side {
            Side::Buy => &mut buy,
            Side::Sell => &mut sell,
        };
        let mut outcome = 0usize;
        while outcome < usize::from(context.domain.outcome_count) {
            let term = match context.book.orders[order].coefficients[outcome].checked_mul(fill) {
                Some(value) => value,
                None => return Ok(None),
            };
            target[outcome] = match target[outcome].checked_add(term) {
                Some(value) => value,
                None => return Ok(None),
            };
            outcome += 1;
        }
        order += 1;
    }
    let split_side = buy[0] >= sell[0];
    let difference = if split_side {
        buy[0] - sell[0]
    } else {
        sell[0] - buy[0]
    };
    let mut outcome = 1usize;
    while outcome < usize::from(context.domain.outcome_count) {
        let same = if split_side {
            buy[outcome] >= sell[outcome] && buy[outcome] - sell[outcome] == difference
        } else {
            sell[outcome] >= buy[outcome] && sell[outcome] - buy[outcome] == difference
        };
        if !same {
            return Ok(None);
        }
        outcome += 1;
    }
    if difference == 0 {
        Ok(Some((0, 0)))
    } else if split_side {
        Ok(Some((difference, 0)))
    } else {
        Ok(Some((0, difference)))
    }
}

fn mul_div_exact_u64(
    left: u128,
    right: u64,
    denominator: u128,
) -> Result<Option<u64>, CandidateBuilderErrorV1> {
    if denominator == 0 {
        return Err(CandidateBuilderErrorV1::ArithmeticOverflow);
    }
    let right = u128::from(right);
    let first = gcd_u128(left, denominator);
    let reduced_left = left / first;
    let remaining_denominator = denominator / first;
    let second = gcd_u128(right, remaining_denominator);
    let reduced_right = right / second;
    if remaining_denominator / second != 1 {
        return Ok(None);
    }
    let value = reduced_left
        .checked_mul(reduced_right)
        .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
    Ok(u64::try_from(value).ok())
}

fn write_u64_prefix<const N: usize>(
    output: &mut [u8],
    cursor: &mut usize,
    values: &[u64; N],
    count: usize,
) -> Result<(), CandidateBuilderErrorV1> {
    if count > N {
        return Err(CandidateBuilderErrorV1::OutputLengthMismatch);
    }
    let mut index = 0usize;
    while index < count {
        put_bytes(output, cursor, &values[index].to_le_bytes())?;
        index += 1;
    }
    Ok(())
}

fn put_bytes(
    output: &mut [u8],
    cursor: &mut usize,
    value: &[u8],
) -> Result<(), CandidateBuilderErrorV1> {
    let end = cursor
        .checked_add(value.len())
        .ok_or(CandidateBuilderErrorV1::ArithmeticOverflow)?;
    let target = output
        .get_mut(*cursor..end)
        .ok_or(CandidateBuilderErrorV1::OutputLengthMismatch)?;
    target.copy_from_slice(value);
    *cursor = end;
    Ok(())
}

const fn gcd_u64(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

const fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

const _: () =
    assert!(QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1 == QUANTIZED_PRICE_MEASURE_SEMANTICS_V1);
