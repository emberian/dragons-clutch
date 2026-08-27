//! Canonical covered-dealer composition for owner-blind RelationV2.
//!
//! This module closes an otherwise-unbalanced [`crate::relation_v2`]
//! candidate with one aggregate covered-dealer leg. The dealer flow is derived
//! from user flow and the virtual split-or-merge; a candidate cannot choose a
//! gross same-outcome round trip. The submitted leg contains only immutable
//! order IDs and dealer-filled units. Receipt, residual envelopes, exact fees,
//! and facility generation arrive through one proof-independent semantic quote
//! precondition whose upstream authentication remains an adapter obligation.
//! No cash allocation, proof body, price witness, account, signer, or SBF
//! representation is a candidate coordinate.
//!
//! `MinimumGrossHamiltonV1` first selects the least gross payer and receiver
//! totals compatible with the exact aggregate dealer receipt and every seller
//! minimum. It allocates payer cash pro rata by residual buyer caps, gives each
//! seller its residual minimum, then allocates any forced excess payout pro
//! rata by exact native Egg atoms. Equal remainders prefer the smaller immutable
//! order identity. Fees are carried and summed separately; they never enter the
//! dealer receipt conservation equation.

use crate::relation_v1::MAX_OUTCOMES;
use crate::relation_v2::{
    derive_unbalanced_economics_v2, EconomicBookV2, EconomicCandidateV2, EconomicDomainV2,
    EconomicErrorV2, PricePreconditionV2, Sha256V2,
};
use crate::score_v2::{
    score_candidate_v2, CandidateDeltaV2, NormalizationPolicyV2, RiskObjectiveV2, ScoreErrorV2,
    ScoreV2,
};
use crate::{Side, MAX_ORDERS};

/// Semantic version of the first RelationV2 covered-dealer join.
pub const DEALER_LEG_VERSION_V2: u8 = 2;
/// Maximum immutable order rows in one aggregate dealer leg.
///
/// This is the RelationV2 order capacity, not a facility LP-roster bound.
/// Runtime adapters may stream the rows without narrowing this pure relation.
pub const MAX_DEALER_ROWS_V2: usize = MAX_ORDERS;

const DEALER_ECONOMIC_DIGEST_DOMAIN_V2: &[u8] = b"dragons-clutch/dealer-economic-candidate/v2\0";
const DEALER_QUOTE_SEMANTICS_DIGEST_DOMAIN_V2: &[u8] =
    b"dragons-clutch/dealer-quote-semantics/v2\0";

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_ORDERS == 64);

/// Frozen per-user cash-allocation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerCashPolicyV2 {
    /// Minimum gross cash, followed by exact Hamilton allocations.
    MinimumGrossHamiltonV1 = 1,
}

/// Immutable facility semantics consumed by one candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFacilityBindingV2 {
    /// Exact dealer-leg semantic version.
    pub version: u8,
    /// Content identity of the facility, never an account key in this crate.
    pub facility_semantics_digest: [u8; 32],
    /// Content identity of its immutable covered-dealer policy.
    pub policy_semantics_digest: [u8; 32],
    /// Exact state generation consumed by the proposed transition.
    pub pre_generation: u64,
}

/// Exact net cash transition quoted by the covered-dealer kernel.
///
/// These are not gross user transfers. After offsetting all derived user cash
/// in against all derived user cash out, exactly one direction may be nonzero:
///
/// `sum(user cash in) + dealer net cash out`
/// `= sum(user cash out) + dealer net cash in`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerReceiptV2 {
    /// Net cash retained by the dealer after offsetting user payouts.
    pub dealer_net_cash_in_atoms: u64,
    /// Net cash supplied by the dealer after offsetting user payments.
    pub dealer_net_cash_out_atoms: u64,
}

/// One immutable order's submitted dealer-fill coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFillRowV2 {
    /// Exact RelationV2 order identity.
    pub order_id: [u8; 32],
    /// Filled order units routed to the dealer.
    pub dealer_fill_units: u64,
}

/// Canonical unused dealer-fill row.
pub const EMPTY_DEALER_FILL_ROW_V2: DealerFillRowV2 = DealerFillRowV2 {
    order_id: [0; 32],
    dealer_fill_units: 0,
};

/// One upstream-quoted order envelope aligned to a dealer-fill row.
///
/// The upstream fee projection supplies `maximum_cash_in_atoms` and
/// `minimum_cash_out_atoms` after all non-dealer consideration and quoted fees.
/// `external_fee_atoms` is digest-bound economic content but is never added to
/// or subtracted from dealer cash. This layer establishes no fee recipient,
/// funding, custody, or transfer conservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerQuoteRowV2 {
    /// Exact RelationV2 order identity; must match the fill row at this index.
    pub order_id: [u8; 32],
    /// Residual maximum a buyer may pay to the dealer.
    pub maximum_cash_in_atoms: u64,
    /// Residual minimum a seller must receive from the dealer.
    pub minimum_cash_out_atoms: u64,
    /// Exact upstream-quoted, digest-bound external fee amount.
    pub external_fee_atoms: u64,
}

/// Canonical unused quote row.
pub const EMPTY_DEALER_QUOTE_ROW_V2: DealerQuoteRowV2 = DealerQuoteRowV2 {
    order_id: [0; 32],
    maximum_cash_in_atoms: 0,
    minimum_cash_out_atoms: 0,
    external_fee_atoms: 0,
};

/// Submitted dealer-fill coordinates. Quote and per-user cash are absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLegCandidateV2 {
    /// Strictly order-ID-sorted active rows followed by exact padding.
    pub rows: [DealerFillRowV2; MAX_DEALER_ROWS_V2],
    /// Active row prefix in `1..=64`.
    pub row_count: u8,
}

/// Canonical dealer Egg movement derived from RelationV2 imbalance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AggregateDealerTradeV2 {
    /// Custodied Eggs transferred from the dealer to filled user buys.
    pub sell_to_users: [u64; MAX_OUTCOMES],
    /// Filled user sells transferred into dealer custody.
    pub buy_from_users: [u64; MAX_OUTCOMES],
}

/// Upstream quote precondition projected to proof-independent semantics.
///
/// This pure crate recomputes `semantic_quote_digest` but cannot authenticate
/// the upstream proof or facility account. The adapter must do so before
/// passing this precondition, exactly as for [`PricePreconditionV2`]. Proof,
/// certificate, signer, and account bytes are deliberately absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerQuotePreconditionV2 {
    /// Exact RelationV2 candidate identity for which this quote was produced.
    pub upstream_economic_candidate_digest: [u8; 32],
    /// Immutable facility and generation binding.
    pub facility: DealerFacilityBindingV2,
    /// Frozen deterministic cash rule.
    pub cash_policy: DealerCashPolicyV2,
    /// Content identity of the exact upstream fee policy.
    pub fee_policy_semantics_digest: [u8; 32],
    /// Canonical dealer trade quoted by the facility kernel.
    pub trade: AggregateDealerTradeV2,
    /// Exact net cash transition quoted by the facility kernel.
    pub receipt: DealerReceiptV2,
    /// Quote rows aligned one-for-one with the active fill rows.
    pub rows: [DealerQuoteRowV2; MAX_DEALER_ROWS_V2],
    /// Recomputed identity of the exact proof-independent quote semantics.
    pub semantic_quote_digest: [u8; 32],
}

/// One exact, fully derived per-order cash allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerCashAllocationV2 {
    /// Exact RelationV2 order identity.
    pub order_id: [u8; 32],
    /// Exact dealer-filled order units.
    pub dealer_fill_units: u64,
    /// Cash this user pays to the dealer.
    pub user_cash_in_atoms: u64,
    /// Cash the dealer pays to this user.
    pub user_cash_out_atoms: u64,
    /// Upstream-quoted, digest-bound external fee; never dealer cash.
    pub external_fee_atoms: u64,
}

/// Canonical unused allocation row.
pub const EMPTY_DEALER_CASH_ALLOCATION_V2: DealerCashAllocationV2 = DealerCashAllocationV2 {
    order_id: [0; 32],
    dealer_fill_units: 0,
    user_cash_in_atoms: 0,
    user_cash_out_atoms: 0,
    external_fee_atoms: 0,
};

/// Public, forgeable DTO for one dealer-leg relation verdict.
///
/// This value is suitable for persistence and untrusted projection only. Its
/// public fields are not evidence that the relation ran, that the quote proof
/// was authenticated, or that any facility account was checked. In-memory
/// consumers that require proof the full pure relation succeeded must accept
/// [`VerifiedDealerLegV2`] instead.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLegVerdictV2 {
    /// Active native Egg width.
    pub outcome_count: u8,
    /// Unique aggregate dealer trade.
    pub trade: AggregateDealerTradeV2,
    /// Exact active allocation prefix followed by canonical padding.
    pub allocations: [DealerCashAllocationV2; MAX_DEALER_ROWS_V2],
    /// Active allocation count.
    pub allocation_count: u8,
    /// Exact sum of quoted external fees; this is not a custody verdict.
    pub total_external_fee_atoms: u128,
    /// User plus dealer demand flow supplied to ScoreV2.
    pub aggregate_buy_flow: [u64; MAX_OUTCOMES],
    /// User plus dealer supply flow supplied to ScoreV2.
    pub aggregate_sell_flow: [u64; MAX_OUTCOMES],
    /// Independently derived direct flow after virtual conversion.
    pub direct_flow: [u64; MAX_OUTCOMES],
    /// Full proof- and representation-independent economic identity.
    pub dealer_economic_candidate_digest: [u8; 32],
    /// Recomputed proof-independent quote identity consumed by this verdict.
    pub dealer_quote_semantics_digest: [u8; 32],
    /// Independently recomputed ScoreV2-Q key.
    pub score: ScoreV2,
}

impl DealerLegVerdictV2 {
    /// Canonical invalid output target for allocation-owning verifiers.
    pub const ZEROED: Self = Self {
        outcome_count: 0,
        trade: AggregateDealerTradeV2 {
            sell_to_users: [0; MAX_OUTCOMES],
            buy_from_users: [0; MAX_OUTCOMES],
        },
        allocations: [EMPTY_DEALER_CASH_ALLOCATION_V2; MAX_DEALER_ROWS_V2],
        allocation_count: 0,
        total_external_fee_atoms: 0,
        aggregate_buy_flow: [0; MAX_OUTCOMES],
        aggregate_sell_flow: [0; MAX_OUTCOMES],
        direct_flow: [0; MAX_OUTCOMES],
        dealer_economic_candidate_digest: [0; 32],
        dealer_quote_semantics_digest: [0; 32],
        score: ScoreV2 {
            risk: RiskObjectiveV2 {
                certified_risk_flow_atoms: 0,
            },
            cash_equivalent_direct_flow_atoms: 0,
            virtual_churn_atoms: 0,
            digest: [0; 32],
        },
    };
}

/// In-memory capability minted only after full dealer-leg verification.
///
/// The private field prevents safe downstream code from wrapping a fabricated
/// or mutated [`DealerLegVerdictV2`] as verified. This capability proves only
/// that [`verify_economic_candidate_with_dealer_v2`] checked the full pure
/// relation. It does not authenticate the upstream quote proof or any account;
/// those remain adapter obligations.
///
/// Detached DTO copies deliberately lose this capability:
///
/// ```compile_fail
/// use clutch_batch::dealer_leg_v2::{DealerLegVerdictV2, VerifiedDealerLegV2};
///
/// fn forge(verdict: DealerLegVerdictV2) -> VerifiedDealerLegV2 {
///     VerifiedDealerLegV2 { verdict }
/// }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedDealerLegV2 {
    verdict: DealerLegVerdictV2,
}

/// Borrowed verifier capability over caller-owned verdict storage.
///
/// Private construction preserves the same authority as
/// [`VerifiedDealerLegV2`] without returning its allocation array by value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedDealerLegRefV2<'a> {
    verdict: &'a DealerLegVerdictV2,
}

impl VerifiedDealerLegRefV2<'_> {
    /// Read the exact fully verified verdict in caller-owned storage.
    pub const fn verdict(&self) -> &DealerLegVerdictV2 {
        self.verdict
    }
}

impl VerifiedDealerLegV2 {
    /// Read the public unauthenticated verdict projection.
    ///
    /// Copying only the returned DTO does not create another verification
    /// capability. The capability itself is safely copyable and remains the
    /// verification authority.
    pub const fn verdict(&self) -> &DealerLegVerdictV2 {
        &self.verdict
    }

    /// Read the active outcome width.
    pub const fn outcome_count(&self) -> u8 {
        self.verdict.outcome_count
    }

    /// Read the uniquely derived aggregate dealer trade.
    pub const fn trade(&self) -> &AggregateDealerTradeV2 {
        &self.verdict.trade
    }

    /// Read the exact derived allocation array.
    pub const fn allocations(&self) -> &[DealerCashAllocationV2; MAX_DEALER_ROWS_V2] {
        &self.verdict.allocations
    }

    /// Read the active allocation prefix length.
    pub const fn allocation_count(&self) -> u8 {
        self.verdict.allocation_count
    }

    /// Read the exact sum of upstream-quoted external fees.
    pub const fn total_external_fee_atoms(&self) -> u128 {
        self.verdict.total_external_fee_atoms
    }

    /// Read the joined aggregate buy flow.
    pub const fn aggregate_buy_flow(&self) -> &[u64; MAX_OUTCOMES] {
        &self.verdict.aggregate_buy_flow
    }

    /// Read the joined aggregate sell flow.
    pub const fn aggregate_sell_flow(&self) -> &[u64; MAX_OUTCOMES] {
        &self.verdict.aggregate_sell_flow
    }

    /// Read the independently derived direct flow.
    pub const fn direct_flow(&self) -> &[u64; MAX_OUTCOMES] {
        &self.verdict.direct_flow
    }

    /// Read the full joined economic candidate identity.
    pub const fn dealer_economic_candidate_digest(&self) -> &[u8; 32] {
        &self.verdict.dealer_economic_candidate_digest
    }

    /// Read the quote semantic identity consumed by this verification.
    pub const fn dealer_quote_semantics_digest(&self) -> &[u8; 32] {
        &self.verdict.dealer_quote_semantics_digest
    }

    /// Read the independently recomputed ScoreV2-Q key.
    pub const fn score(&self) -> &ScoreV2 {
        &self.verdict.score
    }
}

/// Every deterministic refusal in the pure dealer-leg join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerErrorV2 {
    /// The underlying owner-blind economic input was malformed.
    Economic(EconomicErrorV2),
    /// Facility binding selected an unknown dealer-leg version.
    UnknownDealerVersion,
    /// Facility, dealer-policy, or fee-policy semantic identity was zero.
    ZeroSemanticDigest,
    /// Aggregate receipt claimed simultaneous cash in and cash out.
    NonCanonicalReceipt,
    /// RelationV2 already balanced without a dealer.
    ZeroDealerFlow,
    /// Active row count was zero or exceeded the RelationV2 order capacity.
    InvalidRowCount,
    /// Row identities were zero, duplicated, or unordered.
    NonCanonicalRowOrder { row: u8 },
    /// An inactive row was not exactly empty.
    NonCanonicalRowPadding { row: u8 },
    /// An inactive quote row was not exactly empty.
    NonCanonicalQuotePadding { row: u8 },
    /// A quote row did not align with the immutable fill row at that index.
    QuoteRowMismatch { row: u8 },
    /// The facility quote named a different aggregate dealer trade.
    QuoteTradeMismatch,
    /// The quote was produced for a different RelationV2 candidate identity.
    UpstreamEconomicCandidateDigestMismatch,
    /// The quote's semantic digest did not match exact recomputation.
    DealerQuoteSemanticDigestMismatch,
    /// A row named no active RelationV2 order.
    UnknownOrder { row: u8 },
    /// Dealer-filled units were zero or exceeded the selected order fill.
    InvalidDealerFill { row: u8 },
    /// Buy and sell envelope fields were not canonical for the order side.
    NonCanonicalEnvelope { row: u8 },
    /// Per-row coefficient expansion or aggregate dealer flow overflowed.
    FlowOverflow { row: u8, outcome: u8 },
    /// Row flow did not reproduce the uniquely derived aggregate dealer trade.
    DealerFlowMismatch,
    /// A derived per-user cash allocation exceeded its `u64` atom field.
    CashTotalOverflow,
    /// Positive payer cash had no positive residual buyer capacity.
    ZeroAllocationWeight,
    /// Residual buyer capacity could not finance the canonical payer total.
    BuyerCapacityInsufficient,
    /// Aggregate Hamilton weights exceeded the fixed exact `u128` domain.
    AllocationWeightOverflow,
    /// Derived per-user cash did not close the exact aggregate receipt.
    CashConservationMismatch,
    /// A supplied allocation claim differed from exact recomputation.
    AllocationMismatch { row: u8 },
    /// A checked joined-flow or digest calculation overflowed.
    ArithmeticOverflow,
    /// The joined aggregate unexpectedly failed ScoreV2.
    Score(ScoreErrorV2),
}

impl From<EconomicErrorV2> for DealerErrorV2 {
    fn from(value: EconomicErrorV2) -> Self {
        Self::Economic(value)
    }
}

/// Recompute the RelationV2 identity to which a dealer quote must bind.
///
/// Unlike [`crate::relation_v2::verify_economic_candidate_v2`], this helper
/// does not require owner-blind flow to conserve without the dealer. It still
/// performs every RelationV2 domain, book, price, order, and fill check.
pub fn dealer_upstream_economic_candidate_digest_v2(
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    price: &PricePreconditionV2,
    candidate: &EconomicCandidateV2,
) -> Result<[u8; 32], DealerErrorV2> {
    Ok(derive_unbalanced_economics_v2(domain, book, price, candidate)?.economic_candidate_digest)
}

/// Recompute the proof-independent identity of one canonical dealer quote.
///
/// This function checks canonical shape, but not an upstream proof or account.
/// An adapter must authenticate the returned identity under the named facility,
/// dealer policy, fee policy, and generation before constructing the
/// [`DealerQuotePreconditionV2`] consumed by the joined relation.
pub fn dealer_quote_semantics_digest_v2(
    domain: &EconomicDomainV2,
    dealer: &DealerLegCandidateV2,
    quote: &DealerQuotePreconditionV2,
) -> Result<[u8; 32], DealerErrorV2> {
    validate_quote_content(domain, dealer, quote)?;
    let mut hash = Sha256V2::new();
    hash.update(DEALER_QUOTE_SEMANTICS_DIGEST_DOMAIN_V2)?;
    hash.update(&quote.upstream_economic_candidate_digest)?;
    hash.update(&domain.relation_version.to_le_bytes())?;
    hash.update(&domain.market_semantics_digest)?;
    hash.update(&domain.epoch_semantics_digest)?;
    hash.update(&domain.relation_policy_digest)?;
    hash.update(&domain.epoch_index.to_le_bytes())?;
    hash.update(&[domain.outcome_count])?;
    hash.update(&quote.facility.version.to_le_bytes())?;
    hash.update(&quote.facility.facility_semantics_digest)?;
    hash.update(&quote.facility.policy_semantics_digest)?;
    hash.update(&quote.facility.pre_generation.to_le_bytes())?;
    hash.update(&[cash_policy_byte(quote.cash_policy)])?;
    hash.update(&quote.fee_policy_semantics_digest)?;
    hash.update(&quote.receipt.dealer_net_cash_in_atoms.to_le_bytes())?;
    hash.update(&quote.receipt.dealer_net_cash_out_atoms.to_le_bytes())?;
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        hash.update(&quote.trade.sell_to_users[outcome].to_le_bytes())?;
        hash.update(&quote.trade.buy_from_users[outcome].to_le_bytes())?;
        outcome += 1;
    }
    hash.update(&[dealer.row_count])?;
    let mut row = 0usize;
    while row < MAX_DEALER_ROWS_V2 {
        let fill = dealer.rows[row];
        let envelope = quote.rows[row];
        hash.update(&fill.order_id)?;
        hash.update(&fill.dealer_fill_units.to_le_bytes())?;
        hash.update(&envelope.order_id)?;
        hash.update(&envelope.maximum_cash_in_atoms.to_le_bytes())?;
        hash.update(&envelope.minimum_cash_out_atoms.to_le_bytes())?;
        hash.update(&envelope.external_fee_atoms.to_le_bytes())?;
        row += 1;
    }
    hash.finalize().map_err(DealerErrorV2::Economic)
}

/// Verify one owner-blind candidate closed by one covered dealer.
///
/// The legacy [`crate::relation_v2::verify_economic_candidate_v2`] API is not
/// relaxed: it still refuses any user flow that does not conserve without a
/// dealer. This additive verifier is the only path that admits the derived
/// counterparty flow.
pub fn verify_economic_candidate_with_dealer_v2(
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    price: &PricePreconditionV2,
    candidate: &EconomicCandidateV2,
    dealer: &DealerLegCandidateV2,
    quote: &DealerQuotePreconditionV2,
) -> Result<VerifiedDealerLegV2, DealerErrorV2> {
    let mut verdict = DealerLegVerdictV2::ZEROED;
    verify_economic_candidate_with_dealer_into_v2(
        domain,
        book,
        price,
        candidate,
        dealer,
        quote,
        &mut verdict,
    )?;
    Ok(VerifiedDealerLegV2 { verdict })
}

/// Verify one covered-Dealer leg directly into caller-owned storage.
///
/// The returned borrowed capability cannot outlive the exact verdict buffer
/// filled by this call. This is the frame-bounded authority path for SBF;
/// caller-provided verdict bytes are overwritten rather than trusted.
#[allow(clippy::too_many_arguments)]
pub fn verify_economic_candidate_with_dealer_into_v2<'a>(
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    price: &PricePreconditionV2,
    candidate: &EconomicCandidateV2,
    dealer: &DealerLegCandidateV2,
    quote: &DealerQuotePreconditionV2,
    output: &'a mut DealerLegVerdictV2,
) -> Result<VerifiedDealerLegRefV2<'a>, DealerErrorV2> {
    let unbalanced = derive_unbalanced_economics_v2(domain, book, price, candidate)?;
    let trade = derive_aggregate_dealer_trade(
        domain.outcome_count,
        &unbalanced.aggregate_buy_flow,
        &unbalanced.aggregate_sell_flow,
        candidate.virtual_split,
        candidate.virtual_merge,
    )?;
    validate_quote_precondition(
        domain,
        dealer,
        quote,
        &trade,
        &unbalanced.economic_candidate_digest,
    )?;
    let total_external_fee_atoms = verify_cash_allocations_into(
        domain,
        book,
        candidate,
        dealer,
        quote,
        &trade,
        &mut output.allocations,
    )?;

    let mut aggregate_buy_flow = unbalanced.aggregate_buy_flow;
    let mut aggregate_sell_flow = unbalanced.aggregate_sell_flow;
    let mut direct_flow = [0u64; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < usize::from(domain.outcome_count) {
        aggregate_buy_flow[outcome] = aggregate_buy_flow[outcome]
            .checked_add(trade.buy_from_users[outcome])
            .ok_or(DealerErrorV2::ArithmeticOverflow)?;
        aggregate_sell_flow[outcome] = aggregate_sell_flow[outcome]
            .checked_add(trade.sell_to_users[outcome])
            .ok_or(DealerErrorV2::ArithmeticOverflow)?;
        let from_buy = aggregate_buy_flow[outcome]
            .checked_sub(candidate.virtual_split)
            .ok_or(DealerErrorV2::ArithmeticOverflow)?;
        let from_sell = aggregate_sell_flow[outcome]
            .checked_sub(candidate.virtual_merge)
            .ok_or(DealerErrorV2::ArithmeticOverflow)?;
        let left = aggregate_buy_flow[outcome]
            .checked_add(candidate.virtual_merge)
            .ok_or(DealerErrorV2::ArithmeticOverflow)?;
        let right = aggregate_sell_flow[outcome]
            .checked_add(candidate.virtual_split)
            .ok_or(DealerErrorV2::ArithmeticOverflow)?;
        if left != right || from_buy != from_sell {
            return Err(DealerErrorV2::DealerFlowMismatch);
        }
        direct_flow[outcome] = from_buy;
        outcome += 1;
    }

    let digest = dealer_economic_digest(
        &unbalanced.economic_candidate_digest,
        &quote.semantic_quote_digest,
    )?;
    let delta = CandidateDeltaV2 {
        normalization_policy: NormalizationPolicyV2::OwnerBlindAggregate,
        outcome_count: domain.outcome_count,
        aggregate_buy_flow,
        aggregate_sell_flow,
        claimed_direct_flow: direct_flow,
        virtual_split: candidate.virtual_split,
        virtual_merge: candidate.virtual_merge,
        candidate_digest: digest,
    };
    let score = score_candidate_v2(&delta).map_err(DealerErrorV2::Score)?;
    output.outcome_count = domain.outcome_count;
    output.trade = trade;
    output.allocation_count = dealer.row_count;
    output.total_external_fee_atoms = total_external_fee_atoms;
    output.aggregate_buy_flow = aggregate_buy_flow;
    output.aggregate_sell_flow = aggregate_sell_flow;
    output.direct_flow = direct_flow;
    output.dealer_economic_candidate_digest = digest;
    output.dealer_quote_semantics_digest = quote.semantic_quote_digest;
    output.score = score;
    Ok(VerifiedDealerLegRefV2 { verdict: output })
}

/// Compare an adapter-carried allocation with a fully verified dealer leg.
///
/// Production codecs need not persist these derived bytes. If a settlement
/// checkpoint does carry them for execution convenience, this function makes
/// the capability's exact derived projection authoritative without repeating
/// any order, price, quote, or allocation-policy validation.
pub fn verify_claimed_dealer_allocations_v2(
    verified: &VerifiedDealerLegV2,
    claimed: &[DealerCashAllocationV2; MAX_DEALER_ROWS_V2],
) -> Result<(), DealerErrorV2> {
    let mut row = 0usize;
    while row < MAX_DEALER_ROWS_V2 {
        if claimed[row] != verified.allocations()[row] {
            return Err(DealerErrorV2::AllocationMismatch {
                row: bounded_row(row)?,
            });
        }
        row += 1;
    }
    Ok(())
}

fn validate_facility(quote: &DealerQuotePreconditionV2) -> Result<(), DealerErrorV2> {
    if quote.facility.version != DEALER_LEG_VERSION_V2 {
        return Err(DealerErrorV2::UnknownDealerVersion);
    }
    if is_zero_digest(&quote.facility.facility_semantics_digest)
        || is_zero_digest(&quote.facility.policy_semantics_digest)
        || is_zero_digest(&quote.fee_policy_semantics_digest)
    {
        return Err(DealerErrorV2::ZeroSemanticDigest);
    }
    if quote.receipt.dealer_net_cash_in_atoms != 0 && quote.receipt.dealer_net_cash_out_atoms != 0 {
        return Err(DealerErrorV2::NonCanonicalReceipt);
    }
    Ok(())
}

fn validate_quote_content(
    domain: &EconomicDomainV2,
    dealer: &DealerLegCandidateV2,
    quote: &DealerQuotePreconditionV2,
) -> Result<(), DealerErrorV2> {
    domain.validate()?;
    validate_facility(quote)?;
    let count = usize::from(dealer.row_count);
    if count == 0 || count > MAX_DEALER_ROWS_V2 {
        return Err(DealerErrorV2::InvalidRowCount);
    }

    let mut has_trade = false;
    let mut outcome = 0usize;
    while outcome < usize::from(domain.outcome_count) {
        if quote.trade.sell_to_users[outcome] != 0 && quote.trade.buy_from_users[outcome] != 0 {
            return Err(DealerErrorV2::QuoteTradeMismatch);
        }
        has_trade |=
            quote.trade.sell_to_users[outcome] != 0 || quote.trade.buy_from_users[outcome] != 0;
        outcome += 1;
    }
    while outcome < MAX_OUTCOMES {
        if quote.trade.sell_to_users[outcome] != 0 || quote.trade.buy_from_users[outcome] != 0 {
            return Err(DealerErrorV2::QuoteTradeMismatch);
        }
        outcome += 1;
    }
    if !has_trade {
        return Err(DealerErrorV2::QuoteTradeMismatch);
    }

    let mut previous = [0u8; 32];
    let mut row = 0usize;
    while row < count {
        let fill = dealer.rows[row];
        if is_zero_digest(&fill.order_id) || (row != 0 && previous >= fill.order_id) {
            return Err(DealerErrorV2::NonCanonicalRowOrder {
                row: bounded_row(row)?,
            });
        }
        if fill.dealer_fill_units == 0 {
            return Err(DealerErrorV2::InvalidDealerFill {
                row: bounded_row(row)?,
            });
        }
        previous = fill.order_id;
        if quote.rows[row].order_id != fill.order_id {
            return Err(DealerErrorV2::QuoteRowMismatch {
                row: bounded_row(row)?,
            });
        }
        row += 1;
    }
    while row < MAX_DEALER_ROWS_V2 {
        if dealer.rows[row] != EMPTY_DEALER_FILL_ROW_V2 {
            return Err(DealerErrorV2::NonCanonicalRowPadding {
                row: bounded_row(row)?,
            });
        }
        if quote.rows[row] != EMPTY_DEALER_QUOTE_ROW_V2 {
            return Err(DealerErrorV2::NonCanonicalQuotePadding {
                row: bounded_row(row)?,
            });
        }
        row += 1;
    }
    Ok(())
}

fn validate_quote_precondition(
    domain: &EconomicDomainV2,
    dealer: &DealerLegCandidateV2,
    quote: &DealerQuotePreconditionV2,
    derived_trade: &AggregateDealerTradeV2,
    upstream_economic_candidate_digest: &[u8; 32],
) -> Result<(), DealerErrorV2> {
    validate_quote_content(domain, dealer, quote)?;
    if quote.trade != *derived_trade {
        return Err(DealerErrorV2::QuoteTradeMismatch);
    }
    if quote.upstream_economic_candidate_digest != *upstream_economic_candidate_digest {
        return Err(DealerErrorV2::UpstreamEconomicCandidateDigestMismatch);
    }
    if quote.semantic_quote_digest != dealer_quote_semantics_digest_v2(domain, dealer, quote)? {
        return Err(DealerErrorV2::DealerQuoteSemanticDigestMismatch);
    }
    Ok(())
}

fn derive_aggregate_dealer_trade(
    outcome_count: u8,
    user_buy: &[u64; MAX_OUTCOMES],
    user_sell: &[u64; MAX_OUTCOMES],
    virtual_split: u64,
    virtual_merge: u64,
) -> Result<AggregateDealerTradeV2, DealerErrorV2> {
    let mut trade = AggregateDealerTradeV2 {
        sell_to_users: [0; MAX_OUTCOMES],
        buy_from_users: [0; MAX_OUTCOMES],
    };
    let mut has_flow = false;
    let mut outcome = 0usize;
    while outcome < usize::from(outcome_count) {
        let demand = user_buy[outcome]
            .checked_add(virtual_merge)
            .ok_or(DealerErrorV2::ArithmeticOverflow)?;
        let supply = user_sell[outcome]
            .checked_add(virtual_split)
            .ok_or(DealerErrorV2::ArithmeticOverflow)?;
        if demand > supply {
            trade.sell_to_users[outcome] = demand - supply;
            has_flow = true;
        } else if supply > demand {
            trade.buy_from_users[outcome] = supply - demand;
            has_flow = true;
        }
        outcome += 1;
    }
    if !has_flow {
        return Err(DealerErrorV2::ZeroDealerFlow);
    }
    Ok(trade)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RowEconomicsV2 {
    payer_weights: [u128; MAX_DEALER_ROWS_V2],
    receiver_weights: [u128; MAX_DEALER_ROWS_V2],
    receiver_minima: [u128; MAX_DEALER_ROWS_V2],
    total_payer_capacity: u128,
    total_receiver_minimum: u128,
    total_external_fee_atoms: u128,
}

#[inline(never)]
fn validate_rows(
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    candidate: &EconomicCandidateV2,
    dealer: &DealerLegCandidateV2,
    quote: &DealerQuotePreconditionV2,
    trade: &AggregateDealerTradeV2,
) -> Result<RowEconomicsV2, DealerErrorV2> {
    let count = usize::from(dealer.row_count);
    let mut row_economics = RowEconomicsV2 {
        payer_weights: [0; MAX_DEALER_ROWS_V2],
        receiver_weights: [0; MAX_DEALER_ROWS_V2],
        receiver_minima: [0; MAX_DEALER_ROWS_V2],
        total_payer_capacity: 0,
        total_receiver_minimum: 0,
        total_external_fee_atoms: 0,
    };
    let mut row_sell = [0u64; MAX_OUTCOMES];
    let mut row_buy = [0u64; MAX_OUTCOMES];
    let mut row = 0usize;
    while row < count {
        let supplied = dealer.rows[row];
        let quoted = quote.rows[row];
        let order_index =
            find_order(book, &supplied.order_id).ok_or(DealerErrorV2::UnknownOrder {
                row: bounded_row(row)?,
            })?;
        if supplied.dealer_fill_units == 0
            || supplied.dealer_fill_units > candidate.fills[order_index]
        {
            return Err(DealerErrorV2::InvalidDealerFill {
                row: bounded_row(row)?,
            });
        }
        let order = book.orders[order_index];
        match order.side {
            Side::Buy => {
                if quoted.minimum_cash_out_atoms != 0 {
                    return Err(DealerErrorV2::NonCanonicalEnvelope {
                        row: bounded_row(row)?,
                    });
                }
                row_economics.payer_weights[row] = u128::from(quoted.maximum_cash_in_atoms);
                row_economics.total_payer_capacity = row_economics
                    .total_payer_capacity
                    .checked_add(u128::from(quoted.maximum_cash_in_atoms))
                    .ok_or(DealerErrorV2::CashTotalOverflow)?;
            }
            Side::Sell => {
                if quoted.maximum_cash_in_atoms != 0 {
                    return Err(DealerErrorV2::NonCanonicalEnvelope {
                        row: bounded_row(row)?,
                    });
                }
                row_economics.receiver_minima[row] = u128::from(quoted.minimum_cash_out_atoms);
                row_economics.total_receiver_minimum = row_economics
                    .total_receiver_minimum
                    .checked_add(u128::from(quoted.minimum_cash_out_atoms))
                    .ok_or(DealerErrorV2::CashTotalOverflow)?;
            }
        }
        row_economics.total_external_fee_atoms = row_economics
            .total_external_fee_atoms
            .checked_add(u128::from(quoted.external_fee_atoms))
            .ok_or(DealerErrorV2::CashTotalOverflow)?;

        let mut native_atoms = 0u128;
        let mut outcome = 0usize;
        while outcome < usize::from(domain.outcome_count) {
            let leg = order.coefficients[outcome]
                .checked_mul(supplied.dealer_fill_units)
                .ok_or(DealerErrorV2::FlowOverflow {
                    row: bounded_row(row)?,
                    outcome: bounded_outcome(outcome)?,
                })?;
            if order.side == Side::Sell {
                native_atoms = native_atoms
                    .checked_add(u128::from(leg))
                    .ok_or(DealerErrorV2::AllocationWeightOverflow)?;
            }
            let aggregate = match order.side {
                Side::Buy => &mut row_sell[outcome],
                Side::Sell => &mut row_buy[outcome],
            };
            *aggregate = aggregate
                .checked_add(leg)
                .ok_or(DealerErrorV2::FlowOverflow {
                    row: bounded_row(row)?,
                    outcome: bounded_outcome(outcome)?,
                })?;
            outcome += 1;
        }
        if order.side == Side::Sell {
            row_economics.receiver_weights[row] = native_atoms;
        }
        row += 1;
    }
    if row_sell != trade.sell_to_users || row_buy != trade.buy_from_users {
        return Err(DealerErrorV2::DealerFlowMismatch);
    }
    Ok(row_economics)
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn verify_cash_allocations_into(
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    candidate: &EconomicCandidateV2,
    dealer: &DealerLegCandidateV2,
    quote: &DealerQuotePreconditionV2,
    trade: &AggregateDealerTradeV2,
    allocations: &mut [DealerCashAllocationV2; MAX_DEALER_ROWS_V2],
) -> Result<u128, DealerErrorV2> {
    let economics = validate_rows(domain, book, candidate, dealer, quote, trade)?;
    allocate_cash_into(dealer, quote, &economics, allocations)
}

#[inline(never)]
fn allocate_cash_into(
    dealer: &DealerLegCandidateV2,
    quote: &DealerQuotePreconditionV2,
    economics: &RowEconomicsV2,
    allocations: &mut [DealerCashAllocationV2; MAX_DEALER_ROWS_V2],
) -> Result<u128, DealerErrorV2> {
    let receipt_in = u128::from(quote.receipt.dealer_net_cash_in_atoms);
    let receipt_out = u128::from(quote.receipt.dealer_net_cash_out_atoms);
    let minimum_receivers = economics.total_receiver_minimum;
    let receipt_shortfall = receipt_out.saturating_sub(receipt_in);
    let receiver_total_u128 = if minimum_receivers > receipt_shortfall {
        minimum_receivers
    } else {
        receipt_shortfall
    };
    let payer_total_u128 = receiver_total_u128
        .checked_add(receipt_in)
        .and_then(|value| value.checked_sub(receipt_out))
        .ok_or(DealerErrorV2::CashTotalOverflow)?;
    if payer_total_u128 != 0 && economics.total_payer_capacity == 0 {
        return Err(DealerErrorV2::ZeroAllocationWeight);
    }
    if payer_total_u128 > economics.total_payer_capacity {
        return Err(DealerErrorV2::BuyerCapacityInsufficient);
    }

    let count = usize::from(dealer.row_count);
    let payer_allocations = hamilton_allocate(
        payer_total_u128,
        &economics.payer_weights,
        &dealer.rows,
        count,
    )?;
    let receiver_extra = receiver_total_u128
        .checked_sub(economics.total_receiver_minimum)
        .ok_or(DealerErrorV2::CashTotalOverflow)?;
    let receiver_extras = hamilton_allocate(
        receiver_extra,
        &economics.receiver_weights,
        &dealer.rows,
        count,
    )?;

    let mut clear = 0usize;
    while clear < MAX_DEALER_ROWS_V2 {
        allocations[clear] = EMPTY_DEALER_CASH_ALLOCATION_V2;
        clear += 1;
    }
    let mut aggregate_in = 0u128;
    let mut aggregate_out = 0u128;
    let mut row = 0usize;
    while row < count {
        let supplied = dealer.rows[row];
        let quoted = quote.rows[row];
        let cash_out_u128 = economics.receiver_minima[row]
            .checked_add(receiver_extras[row])
            .ok_or(DealerErrorV2::CashTotalOverflow)?;
        let cash_out =
            u64::try_from(cash_out_u128).map_err(|_| DealerErrorV2::CashTotalOverflow)?;
        let payer_allocation =
            u64::try_from(payer_allocations[row]).map_err(|_| DealerErrorV2::CashTotalOverflow)?;
        if payer_allocation > quoted.maximum_cash_in_atoms
            || cash_out < quoted.minimum_cash_out_atoms
            || (payer_allocation != 0 && cash_out != 0)
        {
            return Err(DealerErrorV2::CashConservationMismatch);
        }
        allocations[row] = DealerCashAllocationV2 {
            order_id: supplied.order_id,
            dealer_fill_units: supplied.dealer_fill_units,
            user_cash_in_atoms: payer_allocation,
            user_cash_out_atoms: cash_out,
            external_fee_atoms: quoted.external_fee_atoms,
        };
        aggregate_in = aggregate_in
            .checked_add(u128::from(payer_allocation))
            .ok_or(DealerErrorV2::CashTotalOverflow)?;
        aggregate_out = aggregate_out
            .checked_add(u128::from(cash_out))
            .ok_or(DealerErrorV2::CashTotalOverflow)?;
        row += 1;
    }
    let left = aggregate_in
        .checked_add(receipt_out)
        .ok_or(DealerErrorV2::CashTotalOverflow)?;
    let right = aggregate_out
        .checked_add(receipt_in)
        .ok_or(DealerErrorV2::CashTotalOverflow)?;
    if left != right {
        return Err(DealerErrorV2::CashConservationMismatch);
    }
    Ok(economics.total_external_fee_atoms)
}

fn hamilton_allocate(
    total: u128,
    weights: &[u128; MAX_DEALER_ROWS_V2],
    rows: &[DealerFillRowV2; MAX_DEALER_ROWS_V2],
    count: usize,
) -> Result<[u128; MAX_DEALER_ROWS_V2], DealerErrorV2> {
    let mut result = [0u128; MAX_DEALER_ROWS_V2];
    if total == 0 {
        return Ok(result);
    }
    let mut weight_total = 0u128;
    let mut row = 0usize;
    while row < count {
        weight_total = weight_total
            .checked_add(weights[row])
            .ok_or(DealerErrorV2::AllocationWeightOverflow)?;
        row += 1;
    }
    if weight_total == 0 {
        return Err(DealerErrorV2::ZeroAllocationWeight);
    }

    let mut remainders = [0u128; MAX_DEALER_ROWS_V2];
    let mut assigned = 0u128;
    row = 0;
    while row < count {
        let (quotient, remainder) = exact_mul_div_rem(total, weights[row], weight_total)?;
        result[row] = quotient;
        remainders[row] = remainder;
        assigned = assigned
            .checked_add(result[row])
            .ok_or(DealerErrorV2::ArithmeticOverflow)?;
        row += 1;
    }
    let mut left = total
        .checked_sub(assigned)
        .ok_or(DealerErrorV2::ArithmeticOverflow)?;
    let mut awarded = [false; MAX_DEALER_ROWS_V2];
    while left != 0 {
        let mut winner = None;
        row = 0;
        while row < count {
            if weights[row] != 0 && !awarded[row] {
                winner = match winner {
                    None => Some(row),
                    Some(current) => {
                        if remainders[row] > remainders[current]
                            || (remainders[row] == remainders[current]
                                && rows[row].order_id < rows[current].order_id)
                        {
                            Some(row)
                        } else {
                            Some(current)
                        }
                    }
                };
            }
            row += 1;
        }
        let selected = winner.ok_or(DealerErrorV2::ArithmeticOverflow)?;
        result[selected] = result[selected]
            .checked_add(1)
            .ok_or(DealerErrorV2::ArithmeticOverflow)?;
        awarded[selected] = true;
        left = left
            .checked_sub(1)
            .ok_or(DealerErrorV2::ArithmeticOverflow)?;
    }
    Ok(result)
}

/// Compute `(multiplicand * multiplier) / denominator` and its remainder
/// without requiring a double-width product.
///
/// Modular doubling keeps intermediate remainders below `denominator`, even
/// for hostile `u128` inputs. The result refuses only if the mathematical
/// quotient itself cannot fit `u128` or the denominator is zero.
pub(crate) fn exact_mul_div_rem(
    multiplicand: u128,
    multiplier: u128,
    denominator: u128,
) -> Result<(u128, u128), DealerErrorV2> {
    if denominator == 0 {
        return Err(DealerErrorV2::ZeroAllocationWeight);
    }
    let mut quotient = 0u128;
    let mut remainder = 0u128;
    let multiplicand_quotient = multiplicand / denominator;
    let multiplicand_remainder = multiplicand % denominator;
    let mut bit = 1u128 << 127;
    loop {
        quotient = quotient
            .checked_mul(2)
            .ok_or(DealerErrorV2::ArithmeticOverflow)?;
        let double_complement = denominator - remainder;
        let (mut carry, mut next_remainder) = if remainder >= double_complement {
            (1u128, remainder - double_complement)
        } else {
            (0u128, remainder + remainder)
        };
        if multiplier & bit != 0 {
            carry = carry
                .checked_add(multiplicand_quotient)
                .ok_or(DealerErrorV2::ArithmeticOverflow)?;
            let add_complement = denominator - next_remainder;
            if multiplicand_remainder >= add_complement {
                carry = carry
                    .checked_add(1)
                    .ok_or(DealerErrorV2::ArithmeticOverflow)?;
                next_remainder = multiplicand_remainder - add_complement;
            } else {
                next_remainder += multiplicand_remainder;
            }
        }
        quotient = quotient
            .checked_add(carry)
            .ok_or(DealerErrorV2::ArithmeticOverflow)?;
        remainder = next_remainder;
        if bit == 1 {
            break;
        }
        bit >>= 1;
    }
    Ok((quotient, remainder))
}

fn dealer_economic_digest(
    base_digest: &[u8; 32],
    quote_digest: &[u8; 32],
) -> Result<[u8; 32], DealerErrorV2> {
    let mut hash = Sha256V2::new();
    hash.update(DEALER_ECONOMIC_DIGEST_DOMAIN_V2)?;
    hash.update(base_digest)?;
    hash.update(quote_digest)?;
    hash.finalize().map_err(DealerErrorV2::Economic)
}

fn find_order(book: &EconomicBookV2, order_id: &[u8; 32]) -> Option<usize> {
    let mut order = 0usize;
    while order < usize::from(book.len) {
        if &book.orders[order].order_id == order_id {
            return Some(order);
        }
        order += 1;
    }
    None
}

const fn cash_policy_byte(policy: DealerCashPolicyV2) -> u8 {
    match policy {
        DealerCashPolicyV2::MinimumGrossHamiltonV1 => 1,
    }
}

fn is_zero_digest(digest: &[u8; 32]) -> bool {
    let mut index = 0usize;
    while index < digest.len() {
        if digest[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

fn bounded_row(row: usize) -> Result<u8, DealerErrorV2> {
    u8::try_from(row).map_err(|_| DealerErrorV2::ArithmeticOverflow)
}

fn bounded_outcome(outcome: usize) -> Result<u8, DealerErrorV2> {
    u8::try_from(outcome).map_err(|_| DealerErrorV2::ArithmeticOverflow)
}
