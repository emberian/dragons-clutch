//! Runtime-width candidate verification for the successor General vertical.
//!
//! The compact page row is only a transport projection. Generic Trading must
//! authenticate the immutable order identified by the row before constructing
//! [`AuthenticatedOrderTermsV2`]. This evaluator then derives quote debit and
//! credit itself, enforces the signed limits, aggregates claim movement, and
//! emits one complete runtime-width verified-candidate record. It owns no
//! accounts, performs no CPI, and copies caller-owned candidate outputs only
//! after the complete row transition accepts.
//!
//! # The joint arm (cohort-18)
//!
//! `MECHANISM_JOINT_CLEARING_2026_09_04.md` §1.2 states the clearing as a
//! CERTIFICATE: eight integer KKT conjuncts, which is what this streamed
//! verifier now checks, one row per transaction and the rest at the terminal
//! row. Row by row: bounded (`ExcessLots`), at-or-better (`QuoteLimit`,
//! `CreditLimit`), and the dual half the shipping verifier could not state
//! because it never saw an unfilled order -- an order rationed strictly inside
//! its limit (`RationedInsideLimit`), which is why a row may now fill ZERO
//! lots. At the terminal row: every live order enumerated (`OrderOmitted`,
//! held to the candidate's `live_order_count` that `SubmitCandidate`
//! authenticated against the batch), the cover and complementary-slackness
//! pair that replaced the uniform complete-set delta (`net_i ≤ M` everywhere,
//! `net_i = M` wherever `p_i > 0`, else `PricedResidual`), and decision 0032
//! §2b's tie-break as a conjunct rather than a preference
//! (`NonMinimalPriceVector`): the cursor accumulates the box every row places
//! on its outcome's price and the terminal row computes the box's
//! lexicographic minimum, which is the ONE vector any certified candidate for
//! this book may carry (`JointClearingV1.lexMinFrom`, and the LP-duality
//! argument in that module's header). The simplex conjunct is the Candidate
//! decoder's (`InvalidSimplex`); distinctness is `NonCanonicalOrder`.
//!
//! What the terminal row leaves behind is the clearing itself: the verified
//! certificate carries the price vector as its first tail, so settlement can
//! re-derive the sets and the residual it must strand, and the close can
//! publish the prices (`ClearingPriceV1`).

use crate::general_codec::{SelectionCriterion, SelectionPolicyV1};

use crate::general::runtime_manifest::{
    RuntimeManifestErrorV2, SettlementManifestHeaderV2, SettlementManifestV2,
    SettlementOrderHeaderV2, initialize_manifest_v2, settlement_manifest_len_v2,
    write_scaled_order_v2,
};
use crate::general::runtime_width::{
    CANDIDATE_HEADER_BYTES_V2, CandidateV2, PageV2, RuntimeWidthErrorV2, VerifiedCandidateHeaderV2,
    VerifiedCandidateV2, verified_candidate_len,
};

/// Exact fixed bytes before seven runtime-width `u64` tails in the verifier.
pub const RUNTIME_VERIFIER_HEADER_BYTES_V2: usize = 288;
/// Runtime-width tails after the fixed header: prices, the current order's
/// receive and deliver vectors, aggregate claim inputs and outputs, and the
/// price box's floor and ceiling.
pub const RUNTIME_VERIFIER_TAIL_COUNT_V2: usize = 7;

const VERIFIER_MAGIC: [u8; 8] = *b"DCGVFY02";
const VERSION: u16 = 2;
const PRICES_TAIL: usize = 0;
const CURRENT_RECEIVE_TAIL: usize = 1;
const CURRENT_DELIVER_TAIL: usize = 2;
const CLAIM_INPUTS_TAIL: usize = 3;
const CLAIM_OUTPUTS_TAIL: usize = 4;
/// The least price each outcome may carry given the rows so far.
const PRICE_FLOOR_TAIL: usize = 5;
/// The greatest price each outcome may carry given the rows so far.
const PRICE_CEILING_TAIL: usize = 6;

/// Typed canonical verifier-cursor coordinates for generic Effect writes.
///
/// The runtime verifier remains the single wire-layout owner; data-defined
/// artifacts consume these accessors instead of repeating offsets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeVerifierLayoutV2;

impl RuntimeVerifierLayoutV2 {
    /// Verifier magic byte offset.
    pub const fn magic() -> u32 {
        0
    }

    /// Verifier version byte offset.
    pub const fn version() -> u32 {
        8
    }

    /// Current-order presence byte offset.
    pub const fn has_current_order() -> u32 {
        10
    }

    /// Product-derived outcome count offset.
    pub const fn outcome_count() -> u32 {
        12
    }

    /// Declared page count offset.
    pub const fn page_count() -> u32 {
        16
    }

    /// Next page index offset.
    pub const fn next_page_index() -> u32 {
        20
    }

    /// Next row index offset.
    pub const fn next_row_index() -> u32 {
        24
    }

    /// Distinct completed-order count offset: every order the certificate
    /// enumerated, filled or not.
    pub const fn order_count() -> u32 {
        28
    }

    /// Optimistic verifier revision offset.
    pub const fn revision() -> u32 {
        32
    }

    /// Candidate coordinate offset.
    pub const fn candidate_coordinate() -> u32 {
        40
    }

    /// Filled-order count offset: the orders that emitted a settlement
    /// manifest row. The four bytes at 44 were reserved zero until the joint
    /// clearing; an unfilled order is a row of the certificate and not a row
    /// of the settlement, so the two counts parted here.
    pub const fn filled_order_count() -> u32 {
        44
    }

    /// Candidate identity offset.
    pub const fn candidate_id() -> u32 {
        48
    }

    /// Product identity offset.
    pub const fn product_id() -> u32 {
        80
    }

    /// Batch identity offset.
    pub const fn batch_id() -> u32 {
        112
    }

    /// Price-scale offset.
    pub const fn price_scale() -> u32 {
        144
    }

    /// Filled-lots aggregate offset.
    pub const fn filled_lots() -> u32 {
        152
    }

    /// Quote-debit aggregate offset.
    pub const fn quote_debit() -> u32 {
        160
    }

    /// Quote-credit aggregate offset.
    pub const fn quote_credit() -> u32 {
        168
    }

    /// Current-order identity offset.
    pub const fn current_order_id() -> u32 {
        176
    }

    /// Current-order owner offset.
    pub const fn current_owner_id() -> u32 {
        208
    }

    /// Current-order nonce offset.
    pub const fn current_nonce() -> u32 {
        240
    }

    /// Current-order maximum-lots offset.
    pub const fn current_max_lots() -> u32 {
        248
    }

    /// Current-order debit cap offset.
    pub const fn current_max_quote_debit_per_lot() -> u32 {
        256
    }

    /// Current-order credit floor offset.
    ///
    /// The LAST eight bytes of the fixed header, which were reserved zero until
    /// 2026-09-04 and are now the seller's half of the limit. Placed here and
    /// not beside the debit cap because the alternative was to move
    /// `current_lots` and the two source coordinates, which every persisted
    /// cursor and every fixed-offset EffectProgram write already names.
    pub const fn current_min_quote_credit_per_lot() -> u32 {
        280
    }

    /// Current-order accumulated lots offset.
    pub const fn current_lots() -> u32 {
        264
    }

    /// Source page index of the current order's first fragment.
    pub const fn current_source_page_index() -> u32 {
        272
    }

    /// Source execution index of the current order's first fragment.
    pub const fn current_source_execution_index() -> u32 {
        276
    }

    /// First runtime `u64` tail offset.
    pub const fn tails_base() -> u32 {
        288
    }

    /// Bytes per Product outcome in one verifier tail.
    pub const fn tail_item_stride() -> u32 {
        8
    }

    /// Runtime-width tails after the fixed header.
    pub const fn tail_count() -> u32 {
        RUNTIME_VERIFIER_TAIL_COUNT_V2 as u32
    }
}

/// Stable refusal from runtime-width candidate verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeVerifyErrorV2 {
    /// A Candidate, Page, Execution, or verified-candidate record refused.
    Codec,
    /// A caller-owned cursor or certificate bank had another exact width.
    InvalidLength,
    /// A checked scalar, tail, or byte calculation overflowed.
    ArithmeticOverflow,
    /// Candidate, Page, row, or optimistic revision coordinates differed.
    CoordinateMismatch,
    /// The authenticated immutable order did not match its compact row.
    AuthenticatedOrderMismatch,
    /// Candidate rows were not globally grouped by increasing order identity.
    NonCanonicalOrder,
    /// Two fragments of one order carried different immutable terms.
    OrderSubstitution,
    /// The immutable maximum number of orders was exceeded.
    TooManyOrders,
    /// Candidate-wide lots exceeded the signed order maximum.
    ExcessLots,
    /// Derived quote debit exceeded the authenticated signed-order limit.
    QuoteLimit,
    /// Derived quote credit fell below the authenticated seller's floor.
    ///
    /// `QuoteLimit`'s twin, and it is its own word rather than a second cause
    /// for that one: they accuse opposite parties. A `QuoteLimit` says the
    /// candidate charged a buyer more than the buyer signed for; a
    /// `CreditLimit` says it paid a seller less than the seller signed for, and
    /// a reader who cannot tell those apart has to re-derive the fill to know
    /// which side of the book refused.
    CreditLimit,
    /// An order left short of its maximum while the price was strictly inside
    /// its limit: the marginal conjunct, `f_o < q_o ⇒ ℓ_o ≤ a_o·p`.
    RationedInsideLimit,
    /// The certificate enumerated fewer distinct orders than the batch holds
    /// live: the completeness conjunct, at the terminal row.
    OrderOmitted,
    /// Sets were claimed on a priced outcome nobody funded: the batch would
    /// hold a residual at `p_i > 0`. Complementary slackness, and the exact
    /// form of "the minted sets are funded".
    PricedResidual,
    /// The price vector is not the lexicographic minimum of the box the rows
    /// induce: decision 0032 §2b's tie-break, refused rather than preferred.
    NonMinimalPriceVector,
    /// A row's claim vectors are not the single-outcome shape its authenticated
    /// order names, or the shape is not an interval inside the width.
    ShapeNotInterval,
    /// Aggregate claim inputs and outputs had no uniform complete-set delta.
    ///
    /// Retired by the joint arm -- cover and slackness replaced uniformity --
    /// and kept as a word so a reader of an older log still finds it. Nothing
    /// raises it.
    ClaimImbalance,
    /// Derived quote inventory could not fund the complete-set move and credits.
    QuoteImbalance,
    /// A persisted verifier cursor was hostile or noncanonical.
    InvalidCursor,
    /// Candidate comparison used different Product, Batch, width, or scale.
    ComparisonDomain,
}

impl RuntimeVerifyErrorV2 {
    /// The exact line a program writes to the validator log for this refusal.
    ///
    /// The accelerator publishes ONE canonical refused acknowledgement for every
    /// semantic refusal it can raise, on purpose, so that Trading can tell a
    /// transport fault from a failure-atomic refusal. The consequence is that
    /// the wire cannot carry which conjunct of row verification refused, and
    /// until 2026-09-04 nothing else did either. This line is what does.
    ///
    /// A `&'static str` per variant rather than a `{:?}`: the caller is a
    /// `no_std` program whose peak heap already binds at runtime width 258, and
    /// `sol_log` takes a `&str` with no allocation at all. The match is
    /// exhaustive, so a new variant does not compile until its author says
    /// what a reader should see.
    #[must_use]
    pub const fn log_line(self) -> &'static str {
        match self {
            Self::Codec => "general-verify: refused, a record did not decode",
            Self::InvalidLength => "general-verify: refused, a bank had another exact width",
            Self::ArithmeticOverflow => "general-verify: refused, checked arithmetic overflowed",
            Self::CoordinateMismatch => "general-verify: refused, page/row/revision coordinates",
            Self::AuthenticatedOrderMismatch => {
                "general-verify: refused, the row is not the order it names"
            }
            Self::NonCanonicalOrder => "general-verify: refused, rows are not in identity order",
            Self::OrderSubstitution => "general-verify: refused, two fragments, two terms",
            Self::TooManyOrders => "general-verify: refused, more orders than the batch admits",
            Self::ExcessLots => "general-verify: refused, fill exceeds the signed maximum lots",
            Self::QuoteLimit => "general-verify: refused, buyer charged above the signed cap",
            Self::CreditLimit => "general-verify: refused, seller paid below the signed floor",
            Self::RationedInsideLimit => {
                "general-verify: refused, an order rationed strictly inside its limit"
            }
            Self::OrderOmitted => "general-verify: refused, the certificate omits a live order",
            Self::PricedResidual => {
                "general-verify: refused, sets claimed on a priced outcome nobody funded"
            }
            Self::NonMinimalPriceVector => {
                "general-verify: refused, the price vector is not the book's minimum"
            }
            Self::ShapeNotInterval => {
                "general-verify: refused, a row is not the single-outcome shape it names"
            }
            Self::ClaimImbalance => "general-verify: refused, no uniform complete-set delta",
            Self::QuoteImbalance => "general-verify: refused, quote inventory does not fund it",
            Self::InvalidCursor => "general-verify: refused, the persisted cursor is hostile",
            Self::ComparisonDomain => "general-verify: refused, candidates from another domain",
        }
    }
}

/// Result alias for runtime-width candidate verification.
pub type RuntimeVerifyResultV2<T> = core::result::Result<T, RuntimeVerifyErrorV2>;

/// Which way one order's claims flow.
///
/// `GeneralOrderV2Abi.Side`: a buy receives claims on its interval, a sell
/// delivers them. The tags are the wire's (`ORDER_SIDE_BUY_V2`, `_SELL_V2`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OrderSideV2 {
    /// The maker receives claims and pays quote, bounded above by the cap.
    Buy = crate::general::generated_order_v2::ORDER_SIDE_BUY_V2,
    /// The maker delivers claims and is paid quote, bounded below by the floor.
    Sell = crate::general::generated_order_v2::ORDER_SIDE_SELL_V2,
}

impl OrderSideV2 {
    /// Decode one side tag; zero is not a side.
    #[must_use]
    pub const fn decode(tag: u8) -> Option<Self> {
        match tag {
            crate::general::generated_order_v2::ORDER_SIDE_BUY_V2 => Some(Self::Buy),
            crate::general::generated_order_v2::ORDER_SIDE_SELL_V2 => Some(Self::Sell),
            _ => None,
        }
    }

    /// The canonical one-byte tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }
}

/// Already-authenticated immutable order terms omitted from the compact row.
///
/// Generic Trading constructs this value only after its selected Account and
/// Request Profiles authenticate the finalized order record and bind
/// `order_id` to its exact contents. This value is not itself an authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedOrderTermsV2 {
    /// Immutable order content identity.
    pub order_id: [u8; 32],
    /// Immutable order owner identity.
    pub owner_id: [u8; 32],
    /// Signed-order nonce.
    pub nonce: u64,
    /// Candidate-wide maximum fill.
    pub max_lots: u64,
    /// Candidate-wide maximum derived quote debit per filled lot.
    pub max_quote_debit_per_lot: u64,
    /// Candidate-wide minimum derived quote credit per filled lot.
    ///
    /// The SELLER'S HALF of the limit, and until 2026-09-04 there was none: a
    /// net seller signed a portfolio and a maximum fill and accepted whatever
    /// price the winning candidate chose, down to zero. `max_quote_debit_per_lot`
    /// bounded only the direction in which the maker PAYS.
    ///
    /// Zero is "no floor", which is exactly the behaviour every order signed
    /// before this field existed had.
    pub min_quote_credit_per_lot: u64,
    /// Which way the claims flow.
    pub side: OrderSideV2,
    /// First outcome of the inclusive interval.
    pub outcome_lo: u32,
    /// Last outcome of the inclusive interval.
    pub outcome_hi: u32,
    /// Claims one lot moves at every coordinate of the interval.
    pub claims_per_lot: u64,
}

impl AuthenticatedOrderTermsV2 {
    /// The `(receive, deliver)` row the shape derives at one outcome.
    #[must_use]
    pub const fn derived_row(self, outcome: u32) -> (u64, u64) {
        if outcome < self.outcome_lo || outcome > self.outcome_hi {
            return (0, 0);
        }
        match self.side {
            OrderSideV2::Buy => (self.claims_per_lot, 0),
            OrderSideV2::Sell => (0, self.claims_per_lot),
        }
    }
}

/// Fixed fields decoded from one persisted runtime-width verifier cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeVerifierHeaderV2 {
    /// Runtime outcome width.
    pub outcome_count: u32,
    /// Total immutable candidate pages.
    pub page_count: u32,
    /// Zero-based next page index, equal to `page_count` only when complete.
    pub next_page_index: u32,
    /// Zero-based next row index inside the next page.
    pub next_row_index: u32,
    /// Number of distinct globally grouped orders consumed, filled or not.
    pub order_count: u32,
    /// Number of consumed orders that filled at least one lot: the settlement
    /// manifest's row count.
    pub filled_order_count: u32,
    /// Optimistic revision, advanced exactly once per row.
    pub revision: u64,
    /// Immutable candidate coordinate in its Batch.
    pub candidate_coordinate: u32,
    /// Candidate content identity.
    pub candidate_id: [u8; 32],
    /// Product content identity.
    pub product_id: [u8; 32],
    /// Batch content identity.
    pub batch_id: [u8; 32],
    /// Exact price denominator.
    pub price_scale: u64,
    /// Candidate-wide filled lots.
    pub filled_lots: u64,
    /// Candidate-wide derived quote debit.
    pub quote_debit: u64,
    /// Candidate-wide derived quote credit.
    pub quote_credit: u64,
    /// Whether an unfinished globally grouped order is present.
    pub has_current_order: bool,
}

/// Fixed fields of the optional globally grouped order carried by a verifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCurrentOrderV2 {
    /// Immutable signed-order identity.
    pub order_id: [u8; 32],
    /// Immutable order owner.
    pub owner_id: [u8; 32],
    /// Signed order nonce.
    pub nonce: u64,
    /// Candidate-wide maximum fill.
    pub max_lots: u64,
    /// Signed maximum quote debit per lot.
    pub max_quote_debit_per_lot: u64,
    /// Signed minimum quote credit per lot.
    pub min_quote_credit_per_lot: u64,
    /// Lots accumulated for this grouped order so far.
    pub lots: u64,
    /// Page containing the first execution fragment for this order.
    pub source_page_index: u32,
    /// Row containing the first execution fragment for this order.
    pub source_execution_index: u32,
}

/// Borrowed hostile-decoded runtime-width verifier cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCandidateVerifierV2<'a> {
    bytes: &'a [u8],
    header: RuntimeVerifierHeaderV2,
}

impl<'a> RuntimeCandidateVerifierV2<'a> {
    /// Hostile-decode one exact `288 + 56N` verifier cursor.
    pub fn decode(bytes: &'a [u8]) -> RuntimeVerifyResultV2<Self> {
        if bytes.len() < RUNTIME_VERIFIER_HEADER_BYTES_V2
            || bytes.get(..8) != Some(VERIFIER_MAGIC.as_slice())
            || read_u16(bytes, 8)? != VERSION
            || !zero_range(bytes, 11, 1)?
        {
            return Err(RuntimeVerifyErrorV2::InvalidCursor);
        }
        let has_current_order = match read_byte(bytes, 10)? {
            0 => false,
            1 => true,
            _ => return Err(RuntimeVerifyErrorV2::InvalidCursor),
        };
        let header = RuntimeVerifierHeaderV2 {
            outcome_count: read_u32(bytes, 12)?,
            page_count: read_u32(bytes, 16)?,
            next_page_index: read_u32(bytes, 20)?,
            next_row_index: read_u32(bytes, 24)?,
            order_count: read_u32(bytes, 28)?,
            revision: read_u64(bytes, 32)?,
            candidate_coordinate: read_u32(bytes, 40)?,
            filled_order_count: read_u32(bytes, 44)?,
            candidate_id: read_array32(bytes, 48)?,
            product_id: read_array32(bytes, 80)?,
            batch_id: read_array32(bytes, 112)?,
            price_scale: read_u64(bytes, 144)?,
            filled_lots: read_u64(bytes, 152)?,
            quote_debit: read_u64(bytes, 160)?,
            quote_credit: read_u64(bytes, 168)?,
            has_current_order,
        };
        if bytes.len() != runtime_verifier_len_v2(header.outcome_count)? {
            return Err(RuntimeVerifyErrorV2::InvalidLength);
        }
        validate_cursor(bytes, header)?;
        Ok(Self { bytes, header })
    }

    /// Return the fixed verifier coordinates and aggregates.
    pub const fn header(self) -> RuntimeVerifierHeaderV2 {
        self.header
    }

    /// Return whether all declared pages have been consumed.
    pub const fn is_complete(self) -> bool {
        self.header.next_page_index == self.header.page_count
    }

    /// Return one checked exact simplex price.
    pub fn price(self, index: u32) -> RuntimeVerifyResultV2<u64> {
        read_tail_u64(self.bytes, self.header.outcome_count, PRICES_TAIL, index)
    }

    /// Return one checked aggregate claim input.
    pub fn claim_input(self, index: u32) -> RuntimeVerifyResultV2<u64> {
        read_tail_u64(
            self.bytes,
            self.header.outcome_count,
            CLAIM_INPUTS_TAIL,
            index,
        )
    }

    /// Return one checked aggregate claim output.
    pub fn claim_output(self, index: u32) -> RuntimeVerifyResultV2<u64> {
        read_tail_u64(
            self.bytes,
            self.header.outcome_count,
            CLAIM_OUTPUTS_TAIL,
            index,
        )
    }

    /// Return the least price the rows so far admit at one outcome.
    pub fn price_floor(self, index: u32) -> RuntimeVerifyResultV2<u64> {
        read_tail_u64(
            self.bytes,
            self.header.outcome_count,
            PRICE_FLOOR_TAIL,
            index,
        )
    }

    /// Return the greatest price the rows so far admit at one outcome.
    pub fn price_ceiling(self, index: u32) -> RuntimeVerifyResultV2<u64> {
        read_tail_u64(
            self.bytes,
            self.header.outcome_count,
            PRICE_CEILING_TAIL,
            index,
        )
    }

    /// Return the optional current grouped-order fields.
    pub fn current_order(self) -> RuntimeVerifyResultV2<Option<RuntimeCurrentOrderV2>> {
        if !self.header.has_current_order {
            return Ok(None);
        }
        Ok(Some(RuntimeCurrentOrderV2 {
            order_id: read_array32(self.bytes, 176)?,
            owner_id: read_array32(self.bytes, 208)?,
            nonce: read_u64(self.bytes, 240)?,
            max_lots: read_u64(self.bytes, 248)?,
            max_quote_debit_per_lot: read_u64(self.bytes, 256)?,
            min_quote_credit_per_lot: read_u64(self.bytes, 280)?,
            lots: read_u64(self.bytes, 264)?,
            source_page_index: read_u32(self.bytes, 272)?,
            source_execution_index: read_u32(self.bytes, 276)?,
        }))
    }

    /// Return one current-order receive coefficient, or canonical zero when no
    /// grouped order is open.
    pub fn current_receive_per_lot(self, index: u32) -> RuntimeVerifyResultV2<u64> {
        read_tail_u64(
            self.bytes,
            self.header.outcome_count,
            CURRENT_RECEIVE_TAIL,
            index,
        )
    }

    /// Return one current-order deliver coefficient, or canonical zero when no
    /// grouped order is open.
    pub fn current_deliver_per_lot(self, index: u32) -> RuntimeVerifyResultV2<u64> {
        read_tail_u64(
            self.bytes,
            self.header.outcome_count,
            CURRENT_DELIVER_TAIL,
            index,
        )
    }

    /// Return the exact hostile-decoded cursor bytes.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Readonly inputs for one exact runtime-width candidate row.
pub struct RuntimeConsiderRowViewV2<'a> {
    /// Immutable runtime-width Candidate record.
    pub candidate: &'a [u8],
    /// Immutable runtime-width Page containing the next row.
    pub page: &'a [u8],
    /// All-zero initial or canonical persisted verifier cursor.
    pub cursor_before: &'a [u8],
    /// All-zero candidate certificate destination.
    pub verified_before: &'a [u8],
    /// Authenticated order terms bound by the compact row's `order_id`.
    pub authenticated_order: AuthenticatedOrderTermsV2,
    /// Zero-based optimistic page index.
    pub expected_page_index: u32,
    /// Zero-based optimistic row index.
    pub expected_row_index: u32,
    /// Exact immutable Page revision selected by authenticated state.
    pub expected_page_revision: u64,
    /// Exact optimistic verifier revision.
    pub expected_revision: u64,
    /// Immutable positive order-count envelope.
    pub max_orders: u32,
}

/// Scratch and candidate banks for one failure-atomic runtime row.
pub struct RuntimeConsiderRowBuffersV2<'a> {
    /// Non-authoritative verifier scratch; may change on refusal.
    pub cursor_scratch: &'a mut [u8],
    /// Complete verifier candidate; unchanged on refusal.
    pub cursor_output: &'a mut [u8],
    /// Non-authoritative verified-candidate scratch; may change on refusal.
    pub verified_scratch: &'a mut [u8],
    /// Complete verified-candidate candidate; unchanged on refusal.
    pub verified_output: &'a mut [u8],
}

/// Exact per-step manifest candidate banks for authoritative verification.
pub struct RuntimeManifestBuffersV2<'a> {
    /// Non-authoritative manifest scratch; may change on refusal.
    pub manifest_scratch: &'a mut [u8],
    /// Complete verifier-derived manifest chunk; unchanged on refusal.
    pub manifest_output: &'a mut [u8],
}

struct RuntimeConsiderRowInnerBuffersV2<'a> {
    cursor_scratch: &'a mut [u8],
    cursor_output: Option<&'a mut [u8]>,
    verified_scratch: &'a mut [u8],
    verified_output: Option<&'a mut [u8]>,
}

struct RuntimeManifestInnerBuffersV2<'a> {
    manifest_scratch: &'a mut [u8],
    manifest_output: Option<&'a mut [u8]>,
}

/// Accepted summary for one runtime-width candidate row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConsiderRowSummaryV2 {
    /// Whether the row completed every declared candidate page.
    pub complete: bool,
    /// Exact distinct globally grouped order count, filled or not.
    pub order_count: u32,
    /// Exact successor verifier revision.
    pub revision: u64,
}

/// Complete-set direction derived from a verified candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCompleteSetMoveV2 {
    /// The greatest net claim flow is zero: no set moves.
    None,
    /// Net claims leave the batch: the batch mints that many sets.
    Mint,
    /// Net claims enter the batch: the batch merges that many sets.
    Merge,
}

/// Exact materialization and terminal quote consequence of a certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCandidateBalanceV2 {
    /// Sole complete-set direction.
    pub complete_set_move: RuntimeCompleteSetMoveV2,
    /// Uniform quantity minted or merged: `|M|`, the greatest net flow.
    pub complete_set_quantity: u64,
    /// Exact quote remainder after materialization and credits.
    pub quote_surplus: u64,
}

/// Complete persisted key for comparing best valid submitted candidates.
///
/// These are exactly the three facts interpreted by [`SelectionPolicyV1`].
/// Persisting this key lets later submissions be compared without requiring an
/// optional incumbent-certificate account in the authenticated runtime frame.
/// The joint arm adds no price criterion on purpose: every certified candidate
/// for one book carries the same, minimal, vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCandidateComparisonKeyV2 {
    /// Candidate-wide filled-lots objective.
    pub filled_lots: u64,
    /// Exact quote remainder after materialization and credits.
    pub quote_surplus: u64,
    /// Immutable Candidate content identity.
    pub candidate_id: [u8; 32],
}

/// Return the exact `288 + 56N` runtime verifier cursor width.
pub fn runtime_verifier_len_v2(outcome_count: u32) -> RuntimeVerifyResultV2<usize> {
    if outcome_count == 0 {
        return Err(RuntimeVerifyErrorV2::InvalidLength);
    }
    let count =
        usize::try_from(outcome_count).map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    RUNTIME_VERIFIER_HEADER_BYTES_V2
        .checked_add(
            count
                .checked_mul(8 * RUNTIME_VERIFIER_TAIL_COUNT_V2)
                .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)
}

/// Evaluate one exact row and copy complete candidate banks only on success.
///
/// A zero cursor starts at page and row index zero. Nonzero cursors must match
/// every optimistic coordinate. The final row also derives the exact complete
/// verified-candidate record; earlier rows leave the all-zero certificate
/// destination unchanged.
#[inline(never)]
pub fn evaluate_runtime_consider_row_v2(
    view: RuntimeConsiderRowViewV2<'_>,
    buffers: RuntimeConsiderRowBuffersV2<'_>,
) -> RuntimeVerifyResultV2<RuntimeConsiderRowSummaryV2> {
    let RuntimeConsiderRowBuffersV2 {
        cursor_scratch,
        cursor_output,
        verified_scratch,
        verified_output,
    } = buffers;
    evaluate_runtime_consider_row_inner_v2(
        view,
        RuntimeConsiderRowInnerBuffersV2 {
            cursor_scratch,
            cursor_output: Some(cursor_output),
            verified_scratch,
            verified_output: Some(verified_output),
        },
        None,
    )
}

/// Evaluate one exact row and also emit every newly completed order manifest.
///
/// The manifest capacity must equal the exact zero-, one-, or two-row semantic
/// result derived from the authenticated cursor and selected execution
/// ([`runtime_manifest_orders_for_row_v2`]). It can never truncate the plan.
/// Generic Trading persists these rows into its own authenticated scratch
/// pages before later settlement actions consume them.
#[inline(never)]
pub fn evaluate_runtime_consider_row_with_manifest_v2(
    view: RuntimeConsiderRowViewV2<'_>,
    buffers: RuntimeConsiderRowBuffersV2<'_>,
    manifest: RuntimeManifestBuffersV2<'_>,
) -> RuntimeVerifyResultV2<RuntimeConsiderRowSummaryV2> {
    let RuntimeConsiderRowBuffersV2 {
        cursor_scratch,
        cursor_output,
        verified_scratch,
        verified_output,
    } = buffers;
    let RuntimeManifestBuffersV2 {
        manifest_scratch,
        manifest_output,
    } = manifest;
    evaluate_runtime_consider_row_inner_v2(
        view,
        RuntimeConsiderRowInnerBuffersV2 {
            cursor_scratch,
            cursor_output: Some(cursor_output),
            verified_scratch,
            verified_output: Some(verified_output),
        },
        Some(RuntimeManifestInnerBuffersV2 {
            manifest_scratch,
            manifest_output: Some(manifest_output),
        }),
    )
}

/// Evaluate one row directly in non-authoritative verifier, certificate, and
/// manifest workspaces.
///
/// This preserves the same decoder and transition semantics as
/// [`evaluate_runtime_consider_row_with_manifest_v2`] but omits the three
/// state-last copies. It is only suitable where the caller discards every
/// workspace on refusal and publishes no effect before success.
#[inline(never)]
pub fn evaluate_runtime_consider_row_with_manifest_workspace_v2(
    view: RuntimeConsiderRowViewV2<'_>,
    cursor_workspace: &mut [u8],
    verified_workspace: &mut [u8],
    manifest_workspace: &mut [u8],
) -> RuntimeVerifyResultV2<RuntimeConsiderRowSummaryV2> {
    evaluate_runtime_consider_row_inner_v2(
        view,
        RuntimeConsiderRowInnerBuffersV2 {
            cursor_scratch: cursor_workspace,
            cursor_output: None,
            verified_scratch: verified_workspace,
            verified_output: None,
        },
        Some(RuntimeManifestInnerBuffersV2 {
            manifest_scratch: manifest_workspace,
            manifest_output: None,
        }),
    )
}

#[inline(never)]
fn evaluate_runtime_consider_row_inner_v2(
    view: RuntimeConsiderRowViewV2<'_>,
    mut buffers: RuntimeConsiderRowInnerBuffersV2<'_>,
    mut manifest_buffers: Option<RuntimeManifestInnerBuffersV2<'_>>,
) -> RuntimeVerifyResultV2<RuntimeConsiderRowSummaryV2> {
    let candidate = CandidateV2::decode(view.candidate).map_err(map_codec)?;
    let page = PageV2::decode(view.page).map_err(map_codec)?;
    let candidate_header = candidate.header();
    let cursor_len = runtime_verifier_len_v2(candidate_header.outcome_count)?;
    let verified_len = verified_candidate_len(candidate_header.outcome_count).map_err(map_codec)?;
    if (!view.cursor_before.is_empty() && view.cursor_before.len() != cursor_len)
        || buffers.cursor_scratch.len() != cursor_len
        || buffers
            .cursor_output
            .as_ref()
            .is_some_and(|output| output.len() != cursor_len)
        || (!view.verified_before.is_empty() && view.verified_before.len() != verified_len)
        || buffers.verified_scratch.len() != verified_len
        || buffers
            .verified_output
            .as_ref()
            .is_some_and(|output| output.len() != verified_len)
        || view.max_orders == 0
        || view.verified_before.iter().any(|byte| *byte != 0)
    {
        return Err(RuntimeVerifyErrorV2::InvalidLength);
    }
    let page_header = page.header();
    let expected_page_coordinate = view
        .expected_page_index
        .checked_add(1)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    if page.row_count() == 0
        || page_header.outcome_count != candidate_header.outcome_count
        || page_header.page_count != candidate_header.page_count
        || page_header.page_coordinate != expected_page_coordinate
        || page_header.revision != view.expected_page_revision
        || page_header.candidate_id != candidate_header.candidate_id
        || view.expected_row_index >= page.row_count()
    {
        return Err(RuntimeVerifyErrorV2::CoordinateMismatch);
    }
    let execution = page.execution(view.expected_row_index).map_err(map_codec)?;
    require_authenticated_order(execution.header(), view.authenticated_order)?;

    if view.cursor_before.is_empty() || view.cursor_before.iter().all(|byte| *byte == 0) {
        if view.expected_page_index != 0
            || view.expected_row_index != 0
            || view.expected_revision != 0
        {
            return Err(RuntimeVerifyErrorV2::CoordinateMismatch);
        }
        initialize_cursor(candidate, buffers.cursor_scratch)?;
    } else {
        let before = RuntimeCandidateVerifierV2::decode(view.cursor_before)?;
        require_candidate(before, candidate)?;
        let before_header = before.header();
        if before_header.next_page_index != view.expected_page_index
            || before_header.next_row_index != view.expected_row_index
            || before_header.revision != view.expected_revision
            || before_header.order_count > view.max_orders
            || before.is_complete()
        {
            return Err(RuntimeVerifyErrorV2::CoordinateMismatch);
        }
        buffers.cursor_scratch.copy_from_slice(view.cursor_before);
    }
    let terminal_step = view.expected_page_index.checked_add(1)
        == Some(candidate_header.page_count)
        && view.expected_row_index.checked_add(1) == Some(page.row_count());
    let manifest_order_count = measure_manifest_orders_v2(
        buffers.cursor_scratch,
        execution.header().order_id,
        execution.header().lots,
        terminal_step,
    )?;
    let successor_revision = view
        .expected_revision
        .checked_add(1)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    let mut manifest_writer = match manifest_buffers.as_mut() {
        Some(manifest) => {
            let required =
                settlement_manifest_len_v2(candidate_header.outcome_count, manifest_order_count)
                    .map_err(map_manifest)?;
            if manifest.manifest_scratch.len() != required
                || manifest
                    .manifest_output
                    .as_ref()
                    .is_some_and(|output| output.len() != required)
            {
                return Err(RuntimeVerifyErrorV2::InvalidLength);
            }
            initialize_manifest_v2(
                SettlementManifestHeaderV2 {
                    outcome_count: candidate_header.outcome_count,
                    order_count: manifest_order_count,
                    candidate_coordinate: candidate_header.candidate_coordinate,
                    revision: successor_revision,
                    candidate_id: candidate_header.candidate_id,
                },
                manifest.manifest_scratch,
            )
            .map_err(map_manifest)?;
            Some(ManifestWriterV2 {
                bytes: &mut *manifest.manifest_scratch,
                next: 0,
            })
        }
        None => None,
    };
    buffers.verified_scratch.fill(0);
    ingest_execution(
        buffers.cursor_scratch,
        execution,
        view.authenticated_order,
        view.expected_page_index,
        view.expected_row_index,
        view.max_orders,
        &mut manifest_writer,
    )?;

    let next_row = view
        .expected_row_index
        .checked_add(1)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    if next_row == page.row_count() {
        put_u32(
            buffers.cursor_scratch,
            20,
            view.expected_page_index
                .checked_add(1)
                .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
        )?;
        put_u32(buffers.cursor_scratch, 24, 0)?;
    } else {
        put_u32(buffers.cursor_scratch, 24, next_row)?;
    }
    put_u64(buffers.cursor_scratch, 32, successor_revision)?;

    let mut complete = false;
    let reached_terminal_page =
        read_u32(buffers.cursor_scratch, 20)? == candidate_header.page_count;
    if reached_terminal_page {
        finalize_current_order(buffers.cursor_scratch, &mut manifest_writer)?;
        let completed = RuntimeCandidateVerifierV2::decode(buffers.cursor_scratch)?;
        let header = completed.header();
        // THE COMPLETENESS CONJUNCT. The candidate's live order count was
        // authenticated against the closed batch at `SubmitCandidate`; a
        // certificate that enumerated fewer distinct orders has omitted one,
        // and omission is how a solver would evade the marginal conjunct.
        if header.order_count != candidate_header.live_order_count {
            return Err(RuntimeVerifyErrorV2::OrderOmitted);
        }
        let balance = balance_from_cursor(completed)?;
        require_minimal_price_vector(completed, balance)?;
        let prices = tail_bytes(buffers.cursor_scratch, header.outcome_count, PRICES_TAIL)?;
        let inputs = tail_bytes(
            buffers.cursor_scratch,
            header.outcome_count,
            CLAIM_INPUTS_TAIL,
        )?;
        let outputs = tail_bytes(
            buffers.cursor_scratch,
            header.outcome_count,
            CLAIM_OUTPUTS_TAIL,
        )?;
        VerifiedCandidateV2::encode_le_tails_into(
            VerifiedCandidateHeaderV2 {
                outcome_count: header.outcome_count,
                page_count: header.page_count,
                candidate_coordinate: header.candidate_coordinate,
                revision: header.revision,
                candidate_id: header.candidate_id,
                product_id: header.product_id,
                batch_id: header.batch_id,
                filled_lots: header.filled_lots,
                quote_debit: header.quote_debit,
                quote_credit: header.quote_credit,
                price_scale: header.price_scale,
            },
            prices,
            inputs,
            outputs,
            buffers.verified_scratch,
        )
        .map_err(map_codec)?;
        if runtime_verified_balance_v2(buffers.verified_scratch)? != balance {
            return Err(RuntimeVerifyErrorV2::InvalidCursor);
        }
        complete = true;
    } else {
        RuntimeCandidateVerifierV2::decode(buffers.cursor_scratch)?;
    }

    if manifest_writer
        .as_ref()
        .is_some_and(|writer| writer.next != manifest_order_count)
    {
        return Err(RuntimeVerifyErrorV2::InvalidCursor);
    }
    let _ = manifest_writer.take();
    if let Some(manifest) = manifest_buffers.as_mut() {
        SettlementManifestV2::decode(manifest.manifest_scratch).map_err(map_manifest)?;
        if let Some(output) = manifest.manifest_output.as_deref_mut() {
            output.copy_from_slice(manifest.manifest_scratch);
        }
    }

    let accepted = RuntimeCandidateVerifierV2::decode(buffers.cursor_scratch)?;
    if let Some(output) = buffers.cursor_output.as_deref_mut() {
        output.copy_from_slice(buffers.cursor_scratch);
    }
    if complete && let Some(output) = buffers.verified_output.as_deref_mut() {
        output.copy_from_slice(buffers.verified_scratch);
    }
    Ok(RuntimeConsiderRowSummaryV2 {
        complete,
        order_count: accepted.header().order_count,
        revision: accepted.header().revision,
    })
}

/// Derive the unique complete-set movement and exact quote surplus from a
/// verified certificate, re-proving cover and complementary slackness from
/// the prices it carries.
pub fn runtime_verified_balance_v2(
    verified_bytes: &[u8],
) -> RuntimeVerifyResultV2<RuntimeCandidateBalanceV2> {
    let verified = VerifiedCandidateV2::decode(verified_bytes).map_err(map_codec)?;
    let header = verified.header();
    derive_balance(
        header.outcome_count,
        |index| verified.price(index).map_err(map_codec),
        |index| verified.claim_input(index).map_err(map_codec),
        |index| verified.claim_output(index).map_err(map_codec),
        header.quote_debit,
        header.quote_credit,
    )
}

/// The residual the clearing leaves at one outcome: `M − net_i`, the claims
/// the close STRANDS (decision 0032 §2a). Zero wherever the price is positive
/// for any certificate that passed [`runtime_verified_balance_v2`].
pub fn runtime_verified_residual_v2(
    verified_bytes: &[u8],
    outcome: u32,
) -> RuntimeVerifyResultV2<u64> {
    let verified = VerifiedCandidateV2::decode(verified_bytes).map_err(map_codec)?;
    let header = verified.header();
    let sets = signed_sets(
        header.outcome_count,
        |index| verified.claim_input(index).map_err(map_codec),
        |index| verified.claim_output(index).map_err(map_codec),
    )?;
    let net = net_flow(
        verified.claim_input(outcome).map_err(map_codec)?,
        verified.claim_output(outcome).map_err(map_codec)?,
    );
    u64::try_from(sets - net).map_err(|_| RuntimeVerifyErrorV2::InvalidCursor)
}

/// Compare two valid submitted candidates under immutable interpreted policy.
///
/// This does not claim global optimality on its own; `JointClearingV1.certificate_is_optimal`
/// does, for every candidate that passed the terminal row. It implements only
/// the exact lexicographic comparison used to maintain the best valid
/// submitted candidate among candidates the protocol has actually verified.
pub fn runtime_candidate_better_v2(
    policy: &SelectionPolicyV1,
    left_bytes: &[u8],
    right_bytes: &[u8],
) -> RuntimeVerifyResultV2<bool> {
    let left = VerifiedCandidateV2::decode(left_bytes).map_err(map_codec)?;
    let right = VerifiedCandidateV2::decode(right_bytes).map_err(map_codec)?;
    let left_header = left.header();
    let right_header = right.header();
    if left_header.product_id != right_header.product_id
        || left_header.batch_id != right_header.batch_id
        || left_header.outcome_count != right_header.outcome_count
        || left_header.price_scale != right_header.price_scale
    {
        return Err(RuntimeVerifyErrorV2::ComparisonDomain);
    }
    let left_balance = runtime_verified_balance_v2(left_bytes)?;
    let right_balance = runtime_verified_balance_v2(right_bytes)?;
    runtime_candidate_key_better_v2(
        policy,
        RuntimeCandidateComparisonKeyV2 {
            filled_lots: left_header.filled_lots,
            quote_surplus: left_balance.quote_surplus,
            candidate_id: left_header.candidate_id,
        },
        RuntimeCandidateComparisonKeyV2 {
            filled_lots: right_header.filled_lots,
            quote_surplus: right_balance.quote_surplus,
            candidate_id: right_header.candidate_id,
        },
    )
}

/// Compare two complete candidate keys under one immutable interpreted policy.
///
/// The caller must already have established that both candidates inhabit the
/// same Product, Batch, outcome-width, and price-scale comparison domain. This
/// helper is the sole interpreter for the persisted comparison key.
pub fn runtime_candidate_key_better_v2(
    policy: &SelectionPolicyV1,
    left: RuntimeCandidateComparisonKeyV2,
    right: RuntimeCandidateComparisonKeyV2,
) -> RuntimeVerifyResultV2<bool> {
    for criterion in policy
        .criteria
        .iter()
        .take(usize::from(policy.criterion_count))
    {
        match criterion {
            SelectionCriterion::MaximizeFilledLots if left.filled_lots != right.filled_lots => {
                return Ok(left.filled_lots > right.filled_lots);
            }
            SelectionCriterion::MinimizeQuoteSurplus
                if left.quote_surplus != right.quote_surplus =>
            {
                return Ok(left.quote_surplus < right.quote_surplus);
            }
            SelectionCriterion::MinimizeCandidateId if left.candidate_id != right.candidate_id => {
                return Ok(le_numeric_id(&left.candidate_id, &right.candidate_id));
            }
            _ => {}
        }
    }
    Ok(false)
}

fn initialize_cursor(candidate: CandidateV2<'_>, output: &mut [u8]) -> RuntimeVerifyResultV2<()> {
    let header = candidate.header();
    if output.len() != runtime_verifier_len_v2(header.outcome_count)? {
        return Err(RuntimeVerifyErrorV2::InvalidLength);
    }
    output.fill(0);
    put(output, 0, &VERIFIER_MAGIC)?;
    put_u16(output, 8, VERSION)?;
    put_u32(output, 12, header.outcome_count)?;
    put_u32(output, 16, header.page_count)?;
    put_u32(output, 40, header.candidate_coordinate)?;
    put(output, 48, &header.candidate_id)?;
    put(output, 80, &header.product_id)?;
    put(output, 112, &header.batch_id)?;
    put_u64(output, 144, header.price_scale)?;
    let prices = candidate
        .as_bytes()
        .get(CANDIDATE_HEADER_BYTES_V2..)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?;
    put(
        output,
        tail_offset(header.outcome_count, PRICES_TAIL)?,
        prices,
    )?;
    // The box opens as wide as the simplex: no row has constrained anything.
    for outcome in 0..header.outcome_count {
        write_tail_u64(
            output,
            header.outcome_count,
            PRICE_CEILING_TAIL,
            outcome,
            header.price_scale,
        )?;
    }
    Ok(())
}

fn require_candidate(
    cursor: RuntimeCandidateVerifierV2<'_>,
    candidate: CandidateV2<'_>,
) -> RuntimeVerifyResultV2<()> {
    let observed = cursor.header();
    let expected = candidate.header();
    if observed.outcome_count != expected.outcome_count
        || observed.page_count != expected.page_count
        || observed.candidate_coordinate != expected.candidate_coordinate
        || observed.price_scale != expected.price_scale
        || observed.candidate_id != expected.candidate_id
        || observed.product_id != expected.product_id
        || observed.batch_id != expected.batch_id
    {
        return Err(RuntimeVerifyErrorV2::CoordinateMismatch);
    }
    let candidate_prices = candidate
        .as_bytes()
        .get(CANDIDATE_HEADER_BYTES_V2..)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?;
    if tail_bytes(cursor.as_bytes(), observed.outcome_count, PRICES_TAIL)? != candidate_prices {
        return Err(RuntimeVerifyErrorV2::CoordinateMismatch);
    }
    Ok(())
}

fn require_authenticated_order(
    execution: crate::general::runtime_width::ExecutionHeaderV2,
    order: AuthenticatedOrderTermsV2,
) -> RuntimeVerifyResultV2<()> {
    if zero_identity(&order.order_id)
        || zero_identity(&order.owner_id)
        || order.max_lots == 0
        || execution.order_id != order.order_id
        || execution.owner_id != order.owner_id
        || execution.nonce != order.nonce
        || execution.max_lots != order.max_lots
    {
        Err(RuntimeVerifyErrorV2::AuthenticatedOrderMismatch)
    } else {
        Ok(())
    }
}

struct ManifestWriterV2<'a> {
    bytes: &'a mut [u8],
    next: u32,
}

impl ManifestWriterV2<'_> {
    fn emit(
        &mut self,
        cursor: &[u8],
        header: RuntimeVerifierHeaderV2,
        lots: u64,
        quote_debit: u64,
        quote_credit: u64,
    ) -> RuntimeVerifyResultV2<()> {
        let claim_inputs_per_lot = tail_bytes(cursor, header.outcome_count, CURRENT_DELIVER_TAIL)?;
        let claim_outputs_per_lot = tail_bytes(cursor, header.outcome_count, CURRENT_RECEIVE_TAIL)?;
        write_scaled_order_v2(
            self.bytes,
            self.next,
            SettlementOrderHeaderV2 {
                outcome_count: header.outcome_count,
                // The settlement row's coordinate counts FILLED orders: an
                // unfilled order has nothing to collect or distribute and
                // emits no row, so it must not occupy a coordinate either.
                order_coordinate: header.filled_order_count,
                source_page_index: read_u32(cursor, 272)?,
                nonce: read_u64(cursor, 240)?,
                candidate_id: header.candidate_id,
                order_id: read_array32(cursor, 176)?,
                owner_id: read_array32(cursor, 208)?,
                lots,
                quote_debit,
                quote_credit,
                source_execution_index: read_u32(cursor, 276)?,
            },
            claim_inputs_per_lot,
            claim_outputs_per_lot,
        )
        .map_err(map_manifest)?;
        self.next = self
            .next
            .checked_add(1)
            .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
        Ok(())
    }
}

/// How many manifest rows one row step emits: one for the preceding order it
/// closes, if that order filled anything, and one for the terminal order, if
/// it fills anything once this row is in.
fn measure_manifest_orders_v2(
    cursor: &[u8],
    execution_order: [u8; 32],
    execution_lots: u64,
    terminal_step: bool,
) -> RuntimeVerifyResultV2<u32> {
    let decoded = RuntimeCandidateVerifierV2::decode(cursor)?;
    let header = decoded.header();
    let current_id = read_array32(cursor, 176)?;
    let current_lots = read_u64(cursor, 264)?;
    let same_order = header.has_current_order && current_id == execution_order;
    let closes_preceding = header.has_current_order && !same_order && current_lots != 0;
    let terminal_lots = if same_order {
        add(current_lots, execution_lots)?
    } else {
        execution_lots
    };
    Ok(u32::from(closes_preceding) + u32::from(terminal_step && terminal_lots != 0))
}

fn ingest_execution(
    cursor: &mut [u8],
    execution: crate::general::runtime_width::ExecutionV2<'_>,
    order: AuthenticatedOrderTermsV2,
    source_page_index: u32,
    source_execution_index: u32,
    max_orders: u32,
    manifest: &mut Option<ManifestWriterV2<'_>>,
) -> RuntimeVerifyResultV2<()> {
    let before = RuntimeCandidateVerifierV2::decode(cursor)?;
    let header = before.header();
    let execution_header = execution.header();
    if execution_header.outcome_count != header.outcome_count {
        return Err(RuntimeVerifyErrorV2::CoordinateMismatch);
    }
    if header.has_current_order {
        let current_id = read_array32(cursor, 176)?;
        if current_id == execution_header.order_id {
            require_same_order(cursor, execution, order)?;
        } else {
            if !le_numeric_id(&current_id, &execution_header.order_id) {
                return Err(RuntimeVerifyErrorV2::NonCanonicalOrder);
            }
            finalize_current_order(cursor, manifest)?;
            start_current_order(
                cursor,
                execution,
                order,
                source_page_index,
                source_execution_index,
                max_orders,
            )?;
        }
    } else {
        start_current_order(
            cursor,
            execution,
            order,
            source_page_index,
            source_execution_index,
            max_orders,
        )?;
    }

    let lots = execution_header.lots;
    let current_lots = add(read_u64(cursor, 264)?, lots)?;
    if current_lots > order.max_lots {
        return Err(RuntimeVerifyErrorV2::ExcessLots);
    }
    put_u64(cursor, 264, current_lots)?;
    put_u64(cursor, 152, add(read_u64(cursor, 152)?, lots)?)?;
    for outcome in 0..header.outcome_count {
        let receive = execution.receive_per_lot(outcome).map_err(map_codec)?;
        let deliver = execution.deliver_per_lot(outcome).map_err(map_codec)?;
        add_tail_u64(
            cursor,
            header.outcome_count,
            CLAIM_INPUTS_TAIL,
            outcome,
            multiply(deliver, lots)?,
        )?;
        add_tail_u64(
            cursor,
            header.outcome_count,
            CLAIM_OUTPUTS_TAIL,
            outcome,
            multiply(receive, lots)?,
        )?;
    }
    Ok(())
}

fn start_current_order(
    cursor: &mut [u8],
    execution: crate::general::runtime_width::ExecutionV2<'_>,
    order: AuthenticatedOrderTermsV2,
    source_page_index: u32,
    source_execution_index: u32,
    max_orders: u32,
) -> RuntimeVerifyResultV2<()> {
    let header = RuntimeCandidateVerifierV2::decode(cursor)?.header();
    // THE SHAPE, re-derived by the second opinion: a single-outcome interval
    // inside the width, and a row whose vectors are exactly what the shape
    // says. The collection half checked the row against the record; the
    // accelerator cannot reach the record and checks it against the terms.
    let count = header.outcome_count;
    if order.outcome_lo != order.outcome_hi
        || order.outcome_hi >= count
        || order.claims_per_lot == 0
    {
        return Err(RuntimeVerifyErrorV2::ShapeNotInterval);
    }
    for outcome in 0..count {
        let expected = order.derived_row(outcome);
        if execution.receive_per_lot(outcome).map_err(map_codec)? != expected.0
            || execution.deliver_per_lot(outcome).map_err(map_codec)? != expected.1
        {
            return Err(RuntimeVerifyErrorV2::ShapeNotInterval);
        }
    }
    let next_count = header
        .order_count
        .checked_add(1)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    if next_count > max_orders {
        return Err(RuntimeVerifyErrorV2::TooManyOrders);
    }
    put_byte(cursor, 10, 1)?;
    put_u32(cursor, 28, next_count)?;
    put(cursor, 176, &order.order_id)?;
    put(cursor, 208, &order.owner_id)?;
    put_u64(cursor, 240, order.nonce)?;
    put_u64(cursor, 248, order.max_lots)?;
    put_u64(cursor, 256, order.max_quote_debit_per_lot)?;
    put_u64(cursor, 280, order.min_quote_credit_per_lot)?;
    put_u64(cursor, 264, 0)?;
    put_u32(cursor, 272, source_page_index)?;
    put_u32(cursor, 276, source_execution_index)?;
    for outcome in 0..count {
        write_tail_u64(
            cursor,
            count,
            CURRENT_RECEIVE_TAIL,
            outcome,
            execution.receive_per_lot(outcome).map_err(map_codec)?,
        )?;
        write_tail_u64(
            cursor,
            count,
            CURRENT_DELIVER_TAIL,
            outcome,
            execution.deliver_per_lot(outcome).map_err(map_codec)?,
        )?;
    }
    Ok(())
}

fn require_same_order(
    cursor: &[u8],
    execution: crate::general::runtime_width::ExecutionV2<'_>,
    order: AuthenticatedOrderTermsV2,
) -> RuntimeVerifyResultV2<()> {
    let header = RuntimeCandidateVerifierV2::decode(cursor)?.header();
    if read_array32(cursor, 176)? != order.order_id
        || read_array32(cursor, 208)? != order.owner_id
        || read_u64(cursor, 240)? != order.nonce
        || read_u64(cursor, 248)? != order.max_lots
        || read_u64(cursor, 256)? != order.max_quote_debit_per_lot
        || read_u64(cursor, 280)? != order.min_quote_credit_per_lot
    {
        return Err(RuntimeVerifyErrorV2::OrderSubstitution);
    }
    for outcome in 0..header.outcome_count {
        if read_tail_u64(cursor, header.outcome_count, CURRENT_RECEIVE_TAIL, outcome)?
            != execution.receive_per_lot(outcome).map_err(map_codec)?
            || read_tail_u64(cursor, header.outcome_count, CURRENT_DELIVER_TAIL, outcome)?
                != execution.deliver_per_lot(outcome).map_err(map_codec)?
        {
            return Err(RuntimeVerifyErrorV2::OrderSubstitution);
        }
    }
    Ok(())
}

/// The one outcome the current order moves, its magnitude, and its side --
/// recovered from the current tails, which `start_current_order` proved are
/// the shape.
fn current_shape(cursor: &[u8], count: u32) -> RuntimeVerifyResultV2<(u32, u64, OrderSideV2)> {
    let mut found: Option<(u32, u64, OrderSideV2)> = None;
    for outcome in 0..count {
        let receive = read_tail_u64(cursor, count, CURRENT_RECEIVE_TAIL, outcome)?;
        let deliver = read_tail_u64(cursor, count, CURRENT_DELIVER_TAIL, outcome)?;
        let here = match (receive, deliver) {
            (0, 0) => continue,
            (magnitude, 0) => (outcome, magnitude, OrderSideV2::Buy),
            (0, magnitude) => (outcome, magnitude, OrderSideV2::Sell),
            _ => return Err(RuntimeVerifyErrorV2::ShapeNotInterval),
        };
        if found.is_some() {
            return Err(RuntimeVerifyErrorV2::ShapeNotInterval);
        }
        found = Some(here);
    }
    found.ok_or(RuntimeVerifyErrorV2::ShapeNotInterval)
}

/// `limit × scale / magnitude`, rounded down or up, clamped to the scale: the
/// price bound a limit in atoms per lot induces in scale units per claim.
fn price_bound(
    limit: u64,
    scale: u64,
    magnitude: u64,
    round_up: bool,
) -> RuntimeVerifyResultV2<u64> {
    let product = u128::from(limit) * u128::from(scale);
    let magnitude = u128::from(magnitude);
    if magnitude == 0 {
        return Err(RuntimeVerifyErrorV2::ShapeNotInterval);
    }
    let quotient = if round_up {
        (product + magnitude - 1) / magnitude
    } else {
        product / magnitude
    };
    Ok(u64::try_from(quotient.min(u128::from(scale))).unwrap_or(scale))
}

fn finalize_current_order(
    cursor: &mut [u8],
    manifest: &mut Option<ManifestWriterV2<'_>>,
) -> RuntimeVerifyResultV2<()> {
    let decoded = RuntimeCandidateVerifierV2::decode(cursor)?;
    let header = decoded.header();
    if !header.has_current_order {
        return Ok(());
    }
    let lots = read_u64(cursor, 264)?;
    let max_lots = read_u64(cursor, 248)?;
    if lots > max_lots {
        return Err(RuntimeVerifyErrorV2::ExcessLots);
    }
    let count = header.outcome_count;
    let (outcome, magnitude, side) = current_shape(cursor, count)?;
    let price = decoded.price(outcome)?;
    // Exactly one side of the pair is priced: `price × magnitude` per lot,
    // in the debit direction for a buy and the credit direction for a sell.
    let per_lot = u128::from(price) * u128::from(magnitude);
    let total = per_lot * u128::from(lots);
    let scale = u128::from(header.price_scale);
    let (debit, credit) = match side {
        OrderSideV2::Buy => (
            u64::try_from((total + scale - 1) / scale)
                .map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?,
            0,
        ),
        OrderSideV2::Sell => (
            0,
            u64::try_from(total / scale).map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?,
        ),
    };
    let cap = read_u64(cursor, 256)?;
    let floor = read_u64(cursor, 280)?;
    // AT OR BETTER, on the rounded quantities exactly as before the joint arm:
    // `ceil(x/s) ≤ n ⇔ x ≤ n·s`, so the rounded and exact conjuncts agree.
    if debit > multiply(cap, lots)? {
        return Err(RuntimeVerifyErrorV2::QuoteLimit);
    }
    if credit < multiply(floor, lots)? {
        return Err(RuntimeVerifyErrorV2::CreditLimit);
    }
    // THE MARGINAL CONJUNCT: an order left short of its maximum may not sit
    // strictly inside its limit. `f_o < q_o ⇒ ℓ_o ≤ a_o·p`, exact in scale
    // units: a buyer rationed only at or above their cap, a seller only at or
    // below their floor. A floorless seller left short is refused at every
    // positive price, which is the LP's answer and not a defect.
    if lots < max_lots {
        let inside = match side {
            OrderSideV2::Buy => u128::from(cap) * scale > per_lot,
            OrderSideV2::Sell => per_lot > u128::from(floor) * scale,
        };
        if inside {
            return Err(RuntimeVerifyErrorV2::RationedInsideLimit);
        }
    }
    // THE BOX. Each row narrows its outcome's admissible price interval from
    // the side its fill status decides; the terminal row asks the box for its
    // lexicographic minimum (`JointClearingV1.Fill.priceBounds`).
    let scale64 = header.price_scale;
    match side {
        OrderSideV2::Buy => {
            if lots > 0 {
                let ceiling = price_bound(cap, scale64, magnitude, false)?;
                lower_tail_u64(cursor, count, PRICE_CEILING_TAIL, outcome, ceiling)?;
            }
            if lots < max_lots {
                let floor_bound = price_bound(cap, scale64, magnitude, true)?;
                raise_tail_u64(cursor, count, PRICE_FLOOR_TAIL, outcome, floor_bound)?;
            }
        }
        OrderSideV2::Sell => {
            if lots > 0 {
                let floor_bound = price_bound(floor, scale64, magnitude, true)?;
                raise_tail_u64(cursor, count, PRICE_FLOOR_TAIL, outcome, floor_bound)?;
            }
            if lots < max_lots {
                let ceiling = price_bound(floor, scale64, magnitude, false)?;
                lower_tail_u64(cursor, count, PRICE_CEILING_TAIL, outcome, ceiling)?;
            }
        }
    }
    if lots != 0 {
        let filled = header
            .filled_order_count
            .checked_add(1)
            .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
        put_u32(cursor, 44, filled)?;
        if let Some(writer) = manifest.as_mut() {
            let mut counted = header;
            counted.filled_order_count = filled;
            writer.emit(cursor, counted, lots, debit, credit)?;
        }
    }
    put_u64(cursor, 160, add(header.quote_debit, debit)?)?;
    put_u64(cursor, 168, add(header.quote_credit, credit)?)?;
    put_byte(cursor, 10, 0)?;
    zero_mut(cursor, 176, 112)?;
    zero_tail(cursor, header.outcome_count, CURRENT_RECEIVE_TAIL)?;
    zero_tail(cursor, header.outcome_count, CURRENT_DELIVER_TAIL)
}

fn balance_from_cursor(
    cursor: RuntimeCandidateVerifierV2<'_>,
) -> RuntimeVerifyResultV2<RuntimeCandidateBalanceV2> {
    let header = cursor.header();
    derive_balance(
        header.outcome_count,
        |index| cursor.price(index),
        |index| cursor.claim_input(index),
        |index| cursor.claim_output(index),
        header.quote_debit,
        header.quote_credit,
    )
}

fn net_flow(input: u64, output: u64) -> i128 {
    i128::from(output) - i128::from(input)
}

/// `M`, the signed complete-set count: the greatest net claim flow over the
/// outcomes. Cover (`net_i ≤ M`) holds by construction; slackness is checked
/// against it.
fn signed_sets(
    count: u32,
    mut input: impl FnMut(u32) -> RuntimeVerifyResultV2<u64>,
    mut output: impl FnMut(u32) -> RuntimeVerifyResultV2<u64>,
) -> RuntimeVerifyResultV2<i128> {
    if count == 0 {
        return Err(RuntimeVerifyErrorV2::InvalidCursor);
    }
    let mut sets = i128::MIN;
    for outcome in 0..count {
        sets = sets.max(net_flow(input(outcome)?, output(outcome)?));
    }
    Ok(sets)
}

/// The cover-and-slackness pair, and the quote consequence.
fn derive_balance(
    count: u32,
    mut price: impl FnMut(u32) -> RuntimeVerifyResultV2<u64>,
    mut input: impl FnMut(u32) -> RuntimeVerifyResultV2<u64>,
    mut output: impl FnMut(u32) -> RuntimeVerifyResultV2<u64>,
    quote_debit: u64,
    quote_credit: u64,
) -> RuntimeVerifyResultV2<RuntimeCandidateBalanceV2> {
    let sets = signed_sets(count, &mut input, &mut output)?;
    for outcome in 0..count {
        // COMPLEMENTARY SLACKNESS: a residual (`net_i < M`) is admitted only
        // where the batch priced the outcome at zero.
        if price(outcome)? != 0 && net_flow(input(outcome)?, output(outcome)?) != sets {
            return Err(RuntimeVerifyErrorV2::PricedResidual);
        }
    }
    let (complete_set_move, quantity) = if sets == 0 {
        (RuntimeCompleteSetMoveV2::None, 0)
    } else if sets > 0 {
        (
            RuntimeCompleteSetMoveV2::Mint,
            u64::try_from(sets).map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?,
        )
    } else {
        (
            RuntimeCompleteSetMoveV2::Merge,
            u64::try_from(-sets).map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?,
        )
    };
    let available = match complete_set_move {
        RuntimeCompleteSetMoveV2::None => quote_debit,
        RuntimeCompleteSetMoveV2::Mint => quote_debit
            .checked_sub(quantity)
            .ok_or(RuntimeVerifyErrorV2::QuoteImbalance)?,
        RuntimeCompleteSetMoveV2::Merge => add(quote_debit, quantity)?,
    };
    let quote_surplus = available
        .checked_sub(quote_credit)
        .ok_or(RuntimeVerifyErrorV2::QuoteImbalance)?;
    Ok(RuntimeCandidateBalanceV2 {
        complete_set_move,
        complete_set_quantity: quantity,
        quote_surplus,
    })
}

/// THE TIE-BREAK AS A CONJUNCT. The box the rows induced, with every
/// residual outcome pinned to zero, has one lexicographic minimum on the
/// simplex; the certificate's vector must be it.
fn require_minimal_price_vector(
    cursor: RuntimeCandidateVerifierV2<'_>,
    balance: RuntimeCandidateBalanceV2,
) -> RuntimeVerifyResultV2<()> {
    let header = cursor.header();
    let count = header.outcome_count;
    let scale = header.price_scale;
    let sets = match balance.complete_set_move {
        RuntimeCompleteSetMoveV2::None => 0_i128,
        RuntimeCompleteSetMoveV2::Mint => i128::from(balance.complete_set_quantity),
        RuntimeCompleteSetMoveV2::Merge => -i128::from(balance.complete_set_quantity),
    };
    // Suffix sums of the ceilings, then the greedy front to back:
    // `p_i = min(hi_i, max(lo_i, remaining − Σ_{j>i} hi_j))`.
    let ceiling_at = |outcome: u32| -> RuntimeVerifyResultV2<u128> {
        let residual = net_flow(cursor.claim_input(outcome)?, cursor.claim_output(outcome)?) < sets;
        Ok(if residual {
            0
        } else {
            u128::from(cursor.price_ceiling(outcome)?)
        })
    };
    let mut suffix = 0_u128;
    for outcome in 0..count {
        suffix += ceiling_at(outcome)?;
    }
    let mut remaining = u128::from(scale);
    for outcome in 0..count {
        let ceiling = ceiling_at(outcome)?;
        suffix -= ceiling;
        let floor = u128::from(cursor.price_floor(outcome)?);
        let forced = remaining.saturating_sub(suffix);
        let minimum = ceiling.min(floor.max(forced));
        if u128::from(cursor.price(outcome)?) != minimum {
            return Err(RuntimeVerifyErrorV2::NonMinimalPriceVector);
        }
        remaining -= minimum;
    }
    Ok(())
}

fn validate_cursor(bytes: &[u8], header: RuntimeVerifierHeaderV2) -> RuntimeVerifyResultV2<()> {
    let initial = header.revision == 0
        && header.next_page_index == 0
        && header.next_row_index == 0
        && header.order_count == 0
        && header.filled_order_count == 0
        && header.filled_lots == 0
        && header.quote_debit == 0
        && header.quote_credit == 0
        && !header.has_current_order
        && tail_is_zero(bytes, header.outcome_count, CLAIM_INPUTS_TAIL)?
        && tail_is_zero(bytes, header.outcome_count, CLAIM_OUTPUTS_TAIL)?
        && tail_is_zero(bytes, header.outcome_count, PRICE_FLOOR_TAIL)?;
    if header.outcome_count == 0
        || header.page_count == 0
        || header.candidate_coordinate == 0
        || header.price_scale == 0
        || zero_identity(&header.candidate_id)
        || zero_identity(&header.product_id)
        || zero_identity(&header.batch_id)
        || header.next_page_index > header.page_count
        || (header.next_page_index == header.page_count && header.next_row_index != 0)
        || (header.revision == 0) != initial
        || (!initial && header.order_count == 0)
        || header.filled_order_count > header.order_count
    {
        return Err(RuntimeVerifyErrorV2::InvalidCursor);
    }
    let mut price_total = 0_u64;
    for outcome in 0..header.outcome_count {
        let price = read_tail_u64(bytes, header.outcome_count, PRICES_TAIL, outcome)?;
        price_total = add(price_total, price)?;
        // The box always contains the certificate's own vector: every row that
        // narrowed it also proved the price at or inside its limit.
        let floor = read_tail_u64(bytes, header.outcome_count, PRICE_FLOOR_TAIL, outcome)?;
        let ceiling = read_tail_u64(bytes, header.outcome_count, PRICE_CEILING_TAIL, outcome)?;
        if floor > price || price > ceiling || ceiling > header.price_scale {
            return Err(RuntimeVerifyErrorV2::InvalidCursor);
        }
        if initial && ceiling != header.price_scale {
            return Err(RuntimeVerifyErrorV2::InvalidCursor);
        }
    }
    if price_total != header.price_scale {
        return Err(RuntimeVerifyErrorV2::InvalidCursor);
    }
    if header.has_current_order {
        let current_lots = read_u64(bytes, 264)?;
        let max_lots = read_u64(bytes, 248)?;
        let source_page_index = read_u32(bytes, 272)?;
        let source_execution_index = read_u32(bytes, 276)?;
        let source_precedes_cursor = source_page_index < header.next_page_index
            || source_page_index == header.next_page_index
                && source_execution_index < header.next_row_index;
        if zero_identity(&read_array32(bytes, 176)?)
            || zero_identity(&read_array32(bytes, 208)?)
            || max_lots == 0
            || current_lots > max_lots
            || !source_precedes_cursor
        {
            return Err(RuntimeVerifyErrorV2::InvalidCursor);
        }
    } else if !zero_range(bytes, 176, 112)?
        || !tail_is_zero(bytes, header.outcome_count, CURRENT_RECEIVE_TAIL)?
        || !tail_is_zero(bytes, header.outcome_count, CURRENT_DELIVER_TAIL)?
    {
        return Err(RuntimeVerifyErrorV2::InvalidCursor);
    }
    Ok(())
}

fn tail_offset(count: u32, tail: usize) -> RuntimeVerifyResultV2<usize> {
    let count = usize::try_from(count).map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    RUNTIME_VERIFIER_HEADER_BYTES_V2
        .checked_add(
            count
                .checked_mul(8)
                .and_then(|width| width.checked_mul(tail))
                .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)
}

fn tail_bytes(bytes: &[u8], count: u32, tail: usize) -> RuntimeVerifyResultV2<&[u8]> {
    let start = tail_offset(count, tail)?;
    let width = usize::try_from(count)
        .map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    bytes
        .get(
            start
                ..start
                    .checked_add(width)
                    .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)
}

fn read_tail_u64(bytes: &[u8], count: u32, tail: usize, index: u32) -> RuntimeVerifyResultV2<u64> {
    if index >= count {
        return Err(RuntimeVerifyErrorV2::InvalidLength);
    }
    let index = usize::try_from(index).map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    read_u64(
        bytes,
        tail_offset(count, tail)?
            .checked_add(
                index
                    .checked_mul(8)
                    .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
            )
            .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
    )
}

fn write_tail_u64(
    bytes: &mut [u8],
    count: u32,
    tail: usize,
    index: u32,
    value: u64,
) -> RuntimeVerifyResultV2<()> {
    if index >= count {
        return Err(RuntimeVerifyErrorV2::InvalidLength);
    }
    let index = usize::try_from(index).map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    put_u64(
        bytes,
        tail_offset(count, tail)?
            .checked_add(
                index
                    .checked_mul(8)
                    .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
            )
            .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
        value,
    )
}

fn add_tail_u64(
    bytes: &mut [u8],
    count: u32,
    tail: usize,
    index: u32,
    value: u64,
) -> RuntimeVerifyResultV2<()> {
    let successor = add(read_tail_u64(bytes, count, tail, index)?, value)?;
    write_tail_u64(bytes, count, tail, index, successor)
}

fn raise_tail_u64(
    bytes: &mut [u8],
    count: u32,
    tail: usize,
    index: u32,
    value: u64,
) -> RuntimeVerifyResultV2<()> {
    let current = read_tail_u64(bytes, count, tail, index)?;
    write_tail_u64(bytes, count, tail, index, current.max(value))
}

fn lower_tail_u64(
    bytes: &mut [u8],
    count: u32,
    tail: usize,
    index: u32,
    value: u64,
) -> RuntimeVerifyResultV2<()> {
    let current = read_tail_u64(bytes, count, tail, index)?;
    write_tail_u64(bytes, count, tail, index, current.min(value))
}

fn zero_tail(bytes: &mut [u8], count: u32, tail: usize) -> RuntimeVerifyResultV2<()> {
    let start = tail_offset(count, tail)?;
    let width = usize::try_from(count)
        .map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    zero_mut(bytes, start, width)
}

fn tail_is_zero(bytes: &[u8], count: u32, tail: usize) -> RuntimeVerifyResultV2<bool> {
    Ok(tail_bytes(bytes, count, tail)?
        .iter()
        .all(|byte| *byte == 0))
}

fn map_codec(_: RuntimeWidthErrorV2) -> RuntimeVerifyErrorV2 {
    RuntimeVerifyErrorV2::Codec
}

fn map_manifest(_: RuntimeManifestErrorV2) -> RuntimeVerifyErrorV2 {
    RuntimeVerifyErrorV2::Codec
}

fn zero_identity(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

/// Whether `left` strictly precedes `right` in the protocol's identity order.
///
/// A 32-byte identity is ordered as a LITTLE-ENDIAN 256-bit integer, which is
/// NOT `[u8; 32]`'s lexicographic `Ord`. Every candidate row group and every
/// `MinimizeCandidateId` comparison uses this order, so a builder that sorts
/// with the derived `Ord` produces candidates the verifier refuses with
/// `NonCanonicalOrder`. That defect is invisible against fabricated identities
/// of the shape `[low, 0, 0, ...]`, where the two orders agree, and appears the
/// moment the identities are real digests -- which is exactly how it was found.
///
/// Exported so a candidate builder can sort by the order the protocol reads
/// rather than rediscovering it from a refusal.
#[must_use]
pub fn runtime_identity_precedes_v2(left: &[u8; 32], right: &[u8; 32]) -> bool {
    le_numeric_id(left, right)
}

/// Return the exact number of manifest order rows one row step will emit.
///
/// A caller must size the manifest bank before evaluating, and the count is a
/// function of the cursor's open order, this row's lots, and whether this is
/// the terminal step: an order that fills nothing emits no settlement row.
pub fn runtime_manifest_orders_for_row_v2(
    cursor_before: &[u8],
    execution_order_id: [u8; 32],
    execution_lots: u64,
    terminal_step: bool,
) -> RuntimeVerifyResultV2<u32> {
    if cursor_before.iter().all(|byte| *byte == 0) {
        return Ok(u32::from(terminal_step && execution_lots != 0));
    }
    measure_manifest_orders_v2(
        cursor_before,
        execution_order_id,
        execution_lots,
        terminal_step,
    )
}

fn le_numeric_id(left: &[u8; 32], right: &[u8; 32]) -> bool {
    for index in (0..32).rev() {
        if left[index] != right[index] {
            return left[index] < right[index];
        }
    }
    false
}

fn add(left: u64, right: u64) -> RuntimeVerifyResultV2<u64> {
    left.checked_add(right)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)
}

fn multiply(left: u64, right: u64) -> RuntimeVerifyResultV2<u64> {
    left.checked_mul(right)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)
}

fn read_byte(bytes: &[u8], offset: usize) -> RuntimeVerifyResultV2<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> RuntimeVerifyResultV2<u16> {
    let end = offset
        .checked_add(2)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    let value = bytes
        .get(offset..end)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?;
    let array = <[u8; 2]>::try_from(value).map_err(|_| RuntimeVerifyErrorV2::InvalidLength)?;
    Ok(u16::from_le_bytes(array))
}

fn read_u32(bytes: &[u8], offset: usize) -> RuntimeVerifyResultV2<u32> {
    let end = offset
        .checked_add(4)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    let value = bytes
        .get(offset..end)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?;
    let array = <[u8; 4]>::try_from(value).map_err(|_| RuntimeVerifyErrorV2::InvalidLength)?;
    Ok(u32::from_le_bytes(array))
}

fn read_u64(bytes: &[u8], offset: usize) -> RuntimeVerifyResultV2<u64> {
    let end = offset
        .checked_add(8)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    let value = bytes
        .get(offset..end)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?;
    let array = <[u8; 8]>::try_from(value).map_err(|_| RuntimeVerifyErrorV2::InvalidLength)?;
    Ok(u64::from_le_bytes(array))
}

fn read_array32(bytes: &[u8], offset: usize) -> RuntimeVerifyResultV2<[u8; 32]> {
    let end = offset
        .checked_add(32)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    let value = bytes
        .get(offset..end)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?;
    <[u8; 32]>::try_from(value).map_err(|_| RuntimeVerifyErrorV2::InvalidLength)
}

fn zero_range(bytes: &[u8], offset: usize, length: usize) -> RuntimeVerifyResultV2<bool> {
    let end = offset
        .checked_add(length)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    Ok(bytes
        .get(offset..end)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0))
}

fn zero_mut(bytes: &mut [u8], offset: usize, length: usize) -> RuntimeVerifyResultV2<()> {
    let end = offset
        .checked_add(length)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    bytes
        .get_mut(offset..end)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?
        .fill(0);
    Ok(())
}

fn put(bytes: &mut [u8], offset: usize, value: &[u8]) -> RuntimeVerifyResultV2<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    bytes
        .get_mut(offset..end)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(bytes: &mut [u8], offset: usize, value: u8) -> RuntimeVerifyResultV2<()> {
    *bytes
        .get_mut(offset)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)? = value;
    Ok(())
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) -> RuntimeVerifyResultV2<()> {
    put(bytes, offset, &value.to_le_bytes())
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) -> RuntimeVerifyResultV2<()> {
    put(bytes, offset, &value.to_le_bytes())
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) -> RuntimeVerifyResultV2<()> {
    put(bytes, offset, &value.to_le_bytes())
}

#[cfg(test)]
mod tests;
