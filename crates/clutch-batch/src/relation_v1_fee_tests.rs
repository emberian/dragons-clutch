//! Falsifiers for the composite fee base `kappa*G(a,p) + kappa'*R(a)`.
//!
//! Three obligations live here.
//!
//! 1. **The laboratory differential.**  `composite_fee_quote` must agree, field
//!    for field, with `research/economics-admission/model.py`'s
//!    `composite_floor_quote` on every row of
//!    `fixtures/composite_fee_lab_differential.txt` — a generated artifact
//!    whose every expected value comes from the unbounded-integer Python model
//!    the fee-base selection report ran, never from this crate.
//! 2. **Conservation with a fee.**  The V7/V8 closes must hold at nonzero
//!    rates: the buyer pays consideration *plus* fee, the fee accumulates in
//!    the summary, and `opening_reserved = consideration + fee + refund +
//!    netting_cancelled` closes with the fee term present.
//! 3. **The zero-rate anchor.**  Every verdict at the zero rate pair must be
//!    bit-identical to `FeeBaseV1::None`'s, so the arithmetic landing here
//!    moved no byte that any frozen artifact depends on.
//!
//! **No rate in this file is a proposal.**  Every `TEST_COMPOSITE_*` pair is a
//! laboratory calibration; the production rates are undecided and ember's
//! alone (`docs/decisions/REPORT_fee-base-selection_2026-08-20.md` §1).

use super::*;
use crate::{DustPolicy, PartialPolicy, Side};

const SCALE: u64 = PRICE_SCALE;

fn base_policy() -> FrozenPolicyV1 {
    FrozenPolicyV1 {
        allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
        self_cross: SelfCrossPolicyV1::AllowGateAtPairing,
        aon: AonPolicyV1::RefuseAdmission,
        rounding: RoundingBoundaryV1::TerminalOwnerFloor,
        residual_settlement: ResidualSettlementV1::UniqueSliceReceipts,
        transfer_phase: TransferPhaseV1::ActiveOrResolved,
        portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
        pairing_witness: PairingWitnessPolicyV1::RecomputedConstructor,
        dust: DustPolicy::AssignCanonical,
        score: ScorePolicyV1::LexicographicDispersionV1,
        fee_base: FeeBaseV1::None,
    }
}

fn fee_domain(fee_base: FeeBaseV1, outcomes: u8, owners: u16) -> RelationDomainV1 {
    RelationDomainV1 {
        relation_version: RELATION_VERSION_V1,
        market_id: 11,
        book_id: 22,
        epoch: 7,
        policy_id: 33,
        order_set_id: 44,
        outcome_count: outcomes,
        owner_count: owners,
        price_scale: SCALE,
        remainder_seed: 7,
        policy: FrozenPolicyV1 {
            fee_base,
            ..base_policy()
        },
    }
}

fn single(id: u64, owner: u16, outcome: u8, side: Side, quantity: u64, limit: u64) -> OrderV1 {
    OrderV1::SingleEgg(SingleEggOrderV1 {
        canonical_order_id: id,
        owner,
        outcome,
        side,
        quantity,
        limit_price: limit,
        minimum_fill: 1,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: u64::MAX,
    })
}

fn book_of(orders: &[OrderV1]) -> BookV1 {
    let mut book = BookV1::empty();
    let mut i = 0usize;
    while i < orders.len() {
        book.orders[i] = orders[i];
        i += 1;
    }
    book.len = orders.len() as u8;
    book
}

fn prices(values: &[u64]) -> [u64; MAX_OUTCOMES] {
    let mut vector = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < values.len() {
        vector[i] = values[i];
        i += 1;
    }
    vector
}

fn payoffs(values: &[u64]) -> [u64; MAX_OUTCOMES] {
    prices(values)
}

// ---------------------------------------------------------------------------
// 1. The laboratory differential.
// ---------------------------------------------------------------------------

const LAB_DIFFERENTIAL: &str = include_str!("../fixtures/composite_fee_lab_differential.txt");

/// One parsed fixture row.  Payoffs are read as `u128` so a vector outside the
/// relation's `u64` payoff domain can still be *stated* by the fixture and
/// asserted out of domain, rather than quietly dropped from the differential.
struct LabRow<'a> {
    name: &'a str,
    payoffs: [u128; MAX_OUTCOMES],
    outcomes: usize,
    prices: [u64; MAX_OUTCOMES],
    price_scale: u64,
    dispersion_bps: u32,
    floor_range_bps: u32,
    prior_carry: u128,
    expect: &'a str,
    quote: FeeQuoteV1,
}

fn field<'a>(line: &'a str, key: &str) -> &'a str {
    let rest = line
        .strip_prefix(key)
        .unwrap_or_else(|| panic!("expected `{key}` but found `{line}`"));
    rest.trim()
}

fn scalar<T: core::str::FromStr>(line: &str, key: &str) -> T {
    match field(line, key).parse::<T>() {
        Ok(value) => value,
        Err(_) => panic!("unparseable `{key}` in `{line}`"),
    }
}

fn lab_rows() -> impl Iterator<Item = LabRow<'static>> {
    let mut lines = LAB_DIFFERENTIAL
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'));
    core::iter::from_fn(move || {
        let name = field(lines.next()?, "row ");
        let mut vector = [0u128; MAX_OUTCOMES];
        let mut outcomes = 0usize;
        for token in field(lines.next().expect("payoffs"), "payoffs").split_whitespace() {
            vector[outcomes] = token.parse().expect("payoff");
            outcomes += 1;
        }
        let mut price_vector = [0u64; MAX_OUTCOMES];
        let mut width = 0usize;
        for token in field(lines.next().expect("prices"), "prices").split_whitespace() {
            price_vector[width] = token.parse().expect("price");
            width += 1;
        }
        assert_eq!(width, outcomes, "{name}: ragged fixture row");
        let price_scale = scalar(lines.next().expect("price_scale"), "price_scale");
        let dispersion_bps = scalar(lines.next().expect("dispersion_bps"), "dispersion_bps");
        let floor_range_bps = scalar(lines.next().expect("floor_range_bps"), "floor_range_bps");
        let prior_carry = scalar(lines.next().expect("prior_carry"), "prior_carry");
        let expect = field(lines.next().expect("expect"), "expect ");
        let quote = if expect == "ok" {
            FeeQuoteV1 {
                base_numerator: scalar(lines.next().expect("base_numerator"), "base_numerator"),
                base_denominator: scalar(
                    lines.next().expect("base_denominator"),
                    "base_denominator",
                ),
                exact_numerator: scalar(lines.next().expect("exact_numerator"), "exact_numerator"),
                exact_denominator: scalar(
                    lines.next().expect("exact_denominator"),
                    "exact_denominator",
                ),
                floor_atoms: scalar(lines.next().expect("floor_atoms"), "floor_atoms"),
                terminal_ceil_atoms: scalar(
                    lines.next().expect("terminal_ceil_atoms"),
                    "terminal_ceil_atoms",
                ),
                carry: scalar(lines.next().expect("carry"), "carry"),
            }
        } else {
            FeeQuoteV1 {
                base_numerator: 0,
                base_denominator: 0,
                exact_numerator: 0,
                exact_denominator: 0,
                floor_atoms: 0,
                terminal_ceil_atoms: 0,
                carry: 0,
            }
        };
        Some(LabRow {
            name,
            payoffs: vector,
            outcomes,
            prices: price_vector,
            price_scale,
            dispersion_bps,
            floor_range_bps,
            prior_carry,
            expect,
            quote,
        })
    })
}

#[test]
fn composite_quote_equals_the_economics_laboratory_on_every_row() {
    let mut ok = 0u32;
    let mut refused = 0u32;
    let mut out_of_domain = 0u32;
    for row in lab_rows() {
        let mut narrow = [0u64; MAX_OUTCOMES];
        let mut representable = true;
        let mut i = 0usize;
        while i < row.outcomes {
            if row.payoffs[i] > u64::MAX as u128 {
                representable = false;
            } else {
                narrow[i] = row.payoffs[i] as u64;
            }
            i += 1;
        }
        if row.expect == "payoff_not_u64" {
            assert!(
                !representable,
                "{}: the fixture claims a payoff outside the relation's domain, \
                 but every component fits u64",
                row.name
            );
            out_of_domain += 1;
            continue;
        }
        assert!(
            representable,
            "{}: a quotable row must have a u64 payoff vector",
            row.name
        );
        let quoted = composite_fee_quote(
            &narrow,
            &row.prices,
            row.outcomes,
            row.price_scale,
            row.dispersion_bps,
            row.floor_range_bps,
            row.prior_carry,
        );
        match row.expect {
            "ok" => {
                assert_eq!(
                    quoted,
                    Ok(row.quote),
                    "{}: the lab and the relation disagree",
                    row.name
                );
                ok += 1;
            }
            "overflow" => {
                assert_eq!(
                    quoted,
                    Err(ErrorV1::ArithmeticOverflow),
                    "{}: an unrepresentable rational must refuse, never wrap",
                    row.name
                );
                refused += 1;
            }
            "price_scale_out_of_domain" => {
                assert_eq!(
                    quoted,
                    Err(ErrorV1::InvalidPriceScale),
                    "{}: a price scale past the composite bound must refuse",
                    row.name
                );
                refused += 1;
            }
            other => panic!("{}: unknown expectation `{other}`", row.name),
        }
    }
    // The corpus is pinned: a row silently vanishing from the fixture would
    // otherwise pass this test vacuously.
    assert_eq!(ok, 38, "the lab differential's agreeing-row count moved");
    assert_eq!(refused, 2, "the lab differential's refusal-row count moved");
    assert_eq!(
        out_of_domain, 1,
        "the lab differential's out-of-domain row moved"
    );
}

#[test]
fn composite_quote_reproduces_the_selection_reports_measured_grid() {
    // Report §3.1, composite column: one 10,000-atom binary claim at price
    // scale S = 100, at the lab calibration (40 bp + 10 bp of range), terminal
    // -ceil atoms charged.  Restated here as literals so the published table is
    // checked against the code directly, not only through the fixture file.
    for (price, expected) in [
        (0u64, 10u128),
        (1, 11),
        (10, 14),
        (50, 20),
        (90, 14),
        (99, 11),
        (100, 10),
    ] {
        let quote = composite_fee_quote(
            &payoffs(&[10_000, 0]),
            &prices(&[price, 100 - price]),
            2,
            100,
            40,
            10,
            0,
        )
        .unwrap();
        assert_eq!(
            quote.terminal_ceil_atoms, expected,
            "the published grid moved at price {price}"
        );
    }
}

#[test]
fn composite_kernel_is_exactly_the_diagonal_at_every_admissible_price() {
    // The property the composite was selected for: a risk-free complete set is
    // free at *every* price vector, boundary included, and nothing else is.
    // Bare dispersion loses this at the boundary (Proposition 9); the floor
    // restores it.
    let (dispersion_bps, floor_range_bps) = rates(TEST_COMPOSITE_LAB);
    let mut boundary = 0u64;
    while boundary <= SCALE {
        let vector = prices(&[boundary, SCALE - boundary]);
        // On the diagonal: exactly zero, floor and ceiling alike.
        for constant in [0u64, 1, 7, 1_000_000] {
            let quote = composite_fee_quote(
                &payoffs(&[constant, constant]),
                &vector,
                2,
                SCALE,
                dispersion_bps,
                floor_range_bps,
                0,
            )
            .unwrap();
            assert_eq!(quote.base_numerator, 0, "a complete set was charged");
            assert_eq!(quote.terminal_ceil_atoms, 0);
            assert_eq!(quote.carry, 0);
        }
        // Off the diagonal: strictly positive, whatever the prices do.
        let quote = composite_fee_quote(
            &payoffs(&[1_000_000, 0]),
            &vector,
            2,
            SCALE,
            dispersion_bps,
            floor_range_bps,
            0,
        )
        .unwrap();
        assert!(
            quote.base_numerator > 0 && quote.terminal_ceil_atoms > 0,
            "the kernel swallowed a real transfer at price {boundary}"
        );
        boundary += SCALE / 8;
    }
}

#[test]
fn only_the_floor_term_charges_the_zero_price_channel() {
    // FEE_GEOMETRY §5's laundering channel, at the relation's own price scale.
    // Prices (0, 0, S) put the whole transfer inside dispersion's boundary
    // kernel; the price-free floor is what charges it.
    let vector = prices(&[0, 0, SCALE]);
    let transfer = payoffs(&[1_000_000_000_000_000_000, 0, 0]);
    let (kappa, kappa_floor) = rates(TEST_COMPOSITE_LAB);

    let bare = composite_fee_quote(&transfer, &vector, 3, SCALE, kappa, 0, 0).unwrap();
    assert_eq!(
        bare.base_numerator, 0,
        "bare dispersion must be feeless on the channel — that is the finding"
    );
    assert_eq!(bare.terminal_ceil_atoms, 0);

    let composite =
        composite_fee_quote(&transfer, &vector, 3, SCALE, kappa, kappa_floor, 0).unwrap();
    // kappa' * R exactly: 10/10_000 of 10^18.
    assert_eq!(composite.floor_atoms, 1_000_000_000_000_000);
    assert_eq!(composite.carry, 0);
    assert_eq!(
        composite.floor_atoms,
        (kappa_floor as u128) * 1_000_000_000_000_000_000 / (FEE_BPS_DENOMINATOR as u128)
    );
}

#[test]
fn composite_carry_makes_fragmentation_and_dust_strictly_costly() {
    // Homogeneity plus a persistent carry: splitting one intent into `n` equal
    // fragments and chaining the carry pays exactly what the whole intent pays.
    let (kappa, kappa_floor) = rates(TEST_COMPOSITE_LAB);
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let whole =
        composite_fee_quote(&payoffs(&[12, 0]), &vector, 2, SCALE, kappa, kappa_floor, 0).unwrap();
    let mut carried = 0u128;
    let mut paid = 0u128;
    for _ in 0..12 {
        let fragment = composite_fee_quote(
            &payoffs(&[1, 0]),
            &vector,
            2,
            SCALE,
            kappa,
            kappa_floor,
            carried,
        )
        .unwrap();
        paid += fragment.floor_atoms;
        carried = fragment.carry;
    }
    assert_eq!(paid, whole.floor_atoms, "fragmentation changed the fee");
    assert_eq!(carried, whole.carry, "fragmentation reset the carry");
    // A per-fragment floor with no carry would have collected nothing at all.
    let no_carry =
        composite_fee_quote(&payoffs(&[1, 0]), &vector, 2, SCALE, kappa, kappa_floor, 0).unwrap();
    assert_eq!(no_carry.floor_atoms, 0);
    assert!(
        no_carry.terminal_ceil_atoms > 0,
        "the terminal ceil still closes a dust intent"
    );
}

#[test]
fn composite_quote_refuses_rather_than_wraps_or_guesses() {
    let (kappa, kappa_floor) = rates(TEST_COMPOSITE_LAB);
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let vec4 = payoffs(&[4, 0]);
    assert_eq!(
        composite_fee_quote(&vec4, &vector, 1, SCALE, kappa, kappa_floor, 0),
        Err(ErrorV1::InvalidOutcome)
    );
    assert_eq!(
        composite_fee_quote(
            &vec4,
            &vector,
            MAX_OUTCOMES + 1,
            SCALE,
            kappa,
            kappa_floor,
            0
        ),
        Err(ErrorV1::InvalidOutcome)
    );
    assert_eq!(
        composite_fee_quote(&vec4, &vector, 2, 0, kappa, kappa_floor, 0),
        Err(ErrorV1::InvalidPriceScale)
    );
    assert_eq!(
        composite_fee_quote(
            &vec4,
            &vector,
            2,
            MAX_COMPOSITE_PRICE_SCALE + 1,
            kappa,
            kappa_floor,
            0
        ),
        Err(ErrorV1::InvalidPriceScale)
    );
    assert_eq!(
        composite_fee_quote(
            &vec4,
            &prices(&[SCALE, SCALE]),
            2,
            SCALE,
            kappa,
            kappa_floor,
            0
        ),
        Err(ErrorV1::SimplexSumMismatch)
    );
    assert_eq!(
        composite_fee_quote(
            &vec4,
            &prices(&[SCALE + 1, 0]),
            2,
            SCALE,
            kappa,
            kappa_floor,
            0
        ),
        Err(ErrorV1::PriceOutOfRange)
    );
    // A carry at or past its own denominator is not a carry any honest ledger
    // can produce.
    let denominator = (FEE_BPS_DENOMINATOR as u128)
        * (SCALE as u128)
        * (SCALE as u128)
        * (FEE_BPS_DENOMINATOR as u128);
    assert_eq!(
        composite_fee_quote(&vec4, &vector, 2, SCALE, kappa, kappa_floor, denominator),
        Err(ErrorV1::FeeMismatch)
    );
    assert!(composite_fee_quote(
        &vec4,
        &vector,
        2,
        SCALE,
        kappa,
        kappa_floor,
        denominator - 1
    )
    .is_ok());
}

#[test]
fn composite_never_overflows_at_the_relations_own_price_scale() {
    // At `PRICE_SCALE` the widest representable payoff vector at the widest
    // admissible rates still fits `u128` — the one width statement this lane
    // can make without the frozen bounds FEE_GEOMETRY §3 still owes.
    let (kappa, kappa_floor) = rates(TEST_COMPOSITE_BOUNDARY);
    let mut widest = [0u64; MAX_OUTCOMES];
    widest[0] = u64::MAX;
    let mut vector = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < MAX_OUTCOMES {
        vector[i] = SCALE / (MAX_OUTCOMES as u64);
        if i % 2 == 1 {
            widest[i] = u64::MAX;
        }
        i += 1;
    }
    assert!(
        composite_fee_quote(&widest, &vector, MAX_OUTCOMES, SCALE, kappa, kappa_floor, 0).is_ok()
    );
}

fn rates(fee_base: FeeBaseV1) -> (u32, u32) {
    match fee_base {
        FeeBaseV1::CompositeDispersionFloor {
            dispersion_bps,
            floor_range_bps,
        } => (dispersion_bps, floor_range_bps),
        other => panic!("{other:?} is not a composite rate pair"),
    }
}

// ---------------------------------------------------------------------------
// 2. Conservation with fees, at nonzero TEST rates.
// ---------------------------------------------------------------------------

/// Every conservation identity the fee term participates in, restated over one
/// summary.  A fee appearing from nowhere, or vanishing, breaks one of these.
fn assert_fee_conservation(summary: &SummaryV1) {
    assert_eq!(
        summary.opening_reserved_cash_price_units,
        summary.buyer_consideration_price_units
            + summary.fee_price_units
            + summary.cash_refund_price_units,
        "the payer's reservation must name consideration, fee, and refund exactly"
    );
    // The seller's credit is untouched by the fee: the payer is the only payer.
    assert_eq!(
        summary.buyer_consideration_price_units + summary.merge_proceeds_price_units,
        summary.seller_credit_price_units + summary.split_cost_price_units,
        "the fee must not be taken out of the counterparty's credit"
    );
}

/// A fill large enough that the composite charges whole atoms at the
/// laboratory calibration rather than living entirely in the carry.  At
/// `PRICE_SCALE` and a midpoint clearing price the lab pair charges `q/500`
/// atoms, so the fee at this size is 80 atoms.
const FUNDED_QUANTITY: u64 = 40_000;

#[test]
fn composite_conservation_closes_at_nonzero_test_rates() {
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    // Reservations are generous enough to fund consideration plus fee at every
    // rate under test; the tight-reservation refusal is its own test below.
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, FUNDED_QUANTITY, SCALE),
        single(2, 1, 0, Side::Sell, FUNDED_QUANTITY, 0),
    ]);
    for fee_base in [
        TEST_COMPOSITE_ZERO,
        TEST_COMPOSITE_SMALL,
        TEST_COMPOSITE_LAB,
        TEST_COMPOSITE_DISPERSION_ONLY,
        TEST_COMPOSITE_FLOOR_ONLY,
    ] {
        let domain = fee_domain(fee_base, 2, 2);
        assert_eq!(domain.validate(), Ok(()), "{fee_base:?} must be admissible");
        let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
        let summary = verify(&domain, &book, &candidate, None).unwrap();
        assert_fee_conservation(&summary);

        // The summary's fee term is exactly the owner-level quote, converted
        // into the ledger's price units by the one exact multiplication, and
        // the summary's carry is exactly the quote's carry.
        let (kappa, kappa_floor) = rates(fee_base);
        let (expected_fee, expected_carry) = if kappa == 0 && kappa_floor == 0 {
            (0, 0)
        } else {
            let quote = composite_fee_quote(
                &payoffs(&[FUNDED_QUANTITY, 0]),
                &vector,
                2,
                SCALE,
                kappa,
                kappa_floor,
                0,
            )
            .unwrap();
            (quote.floor_atoms * (SCALE as u128), quote.carry)
        };
        assert_eq!(
            summary.fee_price_units, expected_fee,
            "{fee_base:?}: the ledger and the quote disagree"
        );
        assert_eq!(
            summary.fee_carry_bps_units, expected_carry,
            "{fee_base:?}: the carry the ledger reports is not the quote's"
        );
        if kappa != 0 || kappa_floor != 0 {
            assert!(
                summary.fee_price_units > 0,
                "{fee_base:?}: a nonzero rate charged nothing on a real transfer"
            );
        }
    }
    // The lab calibration's exact charge, stated as a literal: `q/500` atoms.
    let lab = fee_domain(TEST_COMPOSITE_LAB, 2, 2);
    let candidate = canonical_candidate(&lab, &book, &vector, 0, 0).unwrap();
    let summary = verify(&lab, &book, &candidate, None).unwrap();
    assert_eq!(summary.fee_price_units, 80 * (SCALE as u128));

    // The rate boundary is not fundable and must say so rather than clear: a
    // floor at 100% of the model-free range exceeds any bounded reservation on
    // a single-Egg buy, because the reservation is capped at `q * S` while the
    // floor alone is `q` atoms, i.e. `q * S` price units, on top of
    // consideration.  A refusal, not a silent under-charge.
    let boundary = fee_domain(TEST_COMPOSITE_BOUNDARY, 2, 2);
    assert_eq!(boundary.validate(), Ok(()));
    assert_eq!(
        canonical_candidate(&boundary, &book, &vector, 0, 0),
        Err(ErrorV1::FeePayerUnfunded)
    );
}

#[test]
fn composite_at_zero_rates_is_bit_identical_to_no_fee() {
    // The regression anchor: landing the arithmetic must not have moved a
    // single number on any zero-rate route.  Only the two legacy digest fields
    // may differ, because they fold the policy tag.
    let vector = prices(&[SCALE / 3, SCALE - SCALE / 3]);
    let books = [
        book_of(&[
            single(1, 0, 0, Side::Buy, 4, SCALE),
            single(2, 1, 0, Side::Sell, 4, 0),
        ]),
        book_of(&[
            single(1, 0, 0, Side::Buy, 3, SCALE),
            single(2, 1, 0, Side::Sell, 3, 0),
            single(3, 2, 1, Side::Buy, 2, SCALE),
            single(4, 1, 1, Side::Sell, 2, 0),
        ]),
    ];
    for rounding in [
        RoundingBoundaryV1::TerminalOwnerFloor,
        RoundingBoundaryV1::ReceiptFloor,
    ] {
        for book in &books {
            let none = RelationDomainV1 {
                policy: FrozenPolicyV1 {
                    rounding,
                    ..base_policy()
                },
                ..fee_domain(FeeBaseV1::None, 2, 3)
            };
            let zero = RelationDomainV1 {
                policy: FrozenPolicyV1 {
                    rounding,
                    fee_base: TEST_COMPOSITE_ZERO,
                    ..base_policy()
                },
                ..fee_domain(FeeBaseV1::None, 2, 3)
            };
            let candidate = canonical_candidate(&none, book, &vector, 0, 0).unwrap();
            assert!(candidate.fills.iter().any(|fill| *fill != 0));
            // The zero-rate composite folds a different policy tag, so the
            // candidate's claimed digest belongs to the zero-fee domain; the
            // comparison is of the recomputed ledger, not of the claim.
            let mut under_none =
                verify_ignoring_claimed_aggregates(&none, book, &candidate, None).unwrap();
            let mut under_zero =
                verify_ignoring_claimed_aggregates(&zero, book, &candidate, None).unwrap();
            assert_eq!(under_zero.fee_price_units, 0);
            assert_eq!(under_zero.fee_carry_bps_units, 0);
            under_none.score.digest = 0;
            under_none.candidate_digest = 0;
            under_zero.score.digest = 0;
            under_zero.candidate_digest = 0;
            assert_eq!(
                under_none, under_zero,
                "the zero-rate composite moved a number"
            );
        }
    }
}

#[test]
fn composite_payer_must_fund_consideration_plus_fee() {
    // A payer that reserved exactly its consideration cannot fund any fee — the
    // same refusal `FlatNotional` has, now reachable through the composite.
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let tight = book_of(&[
        single(1, 0, 0, Side::Buy, FUNDED_QUANTITY, SCALE / 2),
        single(2, 1, 0, Side::Sell, FUNDED_QUANTITY, 0),
    ]);
    let domain = fee_domain(TEST_COMPOSITE_LAB, 2, 2);
    assert_eq!(
        canonical_candidate(&domain, &tight, &vector, 0, 0),
        Err(ErrorV1::FeePayerUnfunded)
    );
    // At zero rates the identical book clears: the refusal is the fee, not the
    // shape.
    let zero = fee_domain(TEST_COMPOSITE_ZERO, 2, 2);
    assert!(canonical_candidate(&zero, &tight, &vector, 0, 0).is_ok());
}

#[test]
fn composite_charges_the_zero_price_channel_through_the_whole_relation() {
    // The §10.5 fixture the selection report owes, proved end to end rather
    // than at the quote: a candidate clearing at a ZERO price coordinate is
    // charged by the composite and feeless under bare dispersion.
    let vector = prices(&[0, SCALE]);
    let book = book_of(&[
        single(1, 0, 1, Side::Buy, 100, SCALE),
        single(2, 1, 1, Side::Sell, 100, 0),
        // The laundering leg: a transfer supported entirely on the zero-priced
        // outcome.  Its consideration is zero, so no consideration-proportional
        // base can see it.
        single(3, 2, 0, Side::Buy, 1_000_000, SCALE),
        single(4, 3, 0, Side::Sell, 1_000_000, 0),
    ]);

    let bare = fee_domain(TEST_COMPOSITE_DISPERSION_ONLY, 2, 4);
    let candidate = canonical_candidate(&bare, &book, &vector, 0, 0).unwrap();
    let under_bare = verify(&bare, &book, &candidate, None).unwrap();
    assert_fee_conservation(&under_bare);

    let composite = fee_domain(TEST_COMPOSITE_LAB, 2, 4);
    let composite_candidate = canonical_candidate(&composite, &book, &vector, 0, 0).unwrap();
    let under_composite = verify(&composite, &book, &composite_candidate, None).unwrap();
    assert_fee_conservation(&under_composite);
    assert!(
        under_composite.fee_price_units > under_bare.fee_price_units,
        "the floor must charge what the dispersion kernel swallows"
    );

    // The laundering owner is owner 2, whose entire filled vector sits on the
    // zero-priced outcome.  Bare dispersion charges it exactly nothing; the
    // composite charges it exactly kappa' * R.
    let launderer = payoffs(&[1_000_000, 0]);
    assert_eq!(
        composite_fee_quote(&launderer, &vector, 2, SCALE, 40, 0, 0)
            .unwrap()
            .terminal_ceil_atoms,
        0,
        "the channel is real: bare dispersion charges the launderer nothing"
    );
    assert_eq!(
        composite_fee_quote(&launderer, &vector, 2, SCALE, 40, 10, 0)
            .unwrap()
            .floor_atoms,
        1_000,
        "the composite charges it kappa' * R"
    );
}

#[test]
fn composite_wash_round_trip_is_strictly_costly() {
    // ECONOMICS.md's wash-negativity claim, at the relation plane: two owners
    // taking both sides in opposite directions across two outcomes pay a
    // strictly positive fee on every leg, at every rate pair under test.
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let wash = book_of(&[
        single(1, 0, 0, Side::Buy, FUNDED_QUANTITY, SCALE),
        single(2, 1, 0, Side::Sell, FUNDED_QUANTITY, 0),
        single(3, 1, 1, Side::Buy, FUNDED_QUANTITY, SCALE),
        single(4, 0, 1, Side::Sell, FUNDED_QUANTITY, 0),
    ]);
    let zero = fee_domain(TEST_COMPOSITE_ZERO, 2, 2);
    let free = verify(
        &zero,
        &wash,
        &canonical_candidate(&zero, &wash, &vector, 0, 0).unwrap(),
        None,
    )
    .unwrap();
    assert_eq!(
        free.fee_price_units, 0,
        "the anchor: a wash is free at zero rates"
    );

    for fee_base in [
        TEST_COMPOSITE_SMALL,
        TEST_COMPOSITE_LAB,
        TEST_COMPOSITE_FLOOR_ONLY,
    ] {
        let domain = fee_domain(fee_base, 2, 2);
        let candidate = canonical_candidate(&domain, &wash, &vector, 0, 0).unwrap();
        let summary = verify(&domain, &wash, &candidate, None).unwrap();
        assert_fee_conservation(&summary);
        assert!(
            summary.fee_price_units > 0,
            "{fee_base:?}: a wash round trip must be strictly costly"
        );
    }
}

#[test]
fn composite_is_quoted_owner_level_not_per_order() {
    // `G` is subadditive, so a netted portfolio must be charged on its net
    // shape.  One owner buying both legs of a complete set pays *strictly less*
    // than the two legs would pay apart — under the composite it pays the floor
    // on the net range only, which for a complete set is zero.
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let netted = book_of(&[
        single(1, 0, 0, Side::Buy, FUNDED_QUANTITY, SCALE),
        single(2, 0, 1, Side::Buy, FUNDED_QUANTITY, SCALE),
        single(3, 1, 0, Side::Sell, FUNDED_QUANTITY, 0),
        single(4, 2, 1, Side::Sell, FUNDED_QUANTITY, 0),
    ]);
    let domain = fee_domain(TEST_COMPOSITE_LAB, 2, 3);
    let candidate = canonical_candidate(&domain, &netted, &vector, 0, 0).unwrap();
    let summary = verify(&domain, &netted, &candidate, None).unwrap();
    assert_fee_conservation(&summary);
    assert_eq!(
        summary.fee_price_units, 0,
        "a risk-free complete set must be free — that is the venue's own objective"
    );
    assert_eq!(summary.fee_carry_bps_units, 0);

    // The same two legs bought by two different owners are two real transfers
    // and are charged.
    let split = book_of(&[
        single(1, 0, 0, Side::Buy, FUNDED_QUANTITY, SCALE),
        single(2, 3, 1, Side::Buy, FUNDED_QUANTITY, SCALE),
        single(3, 1, 0, Side::Sell, FUNDED_QUANTITY, 0),
        single(4, 2, 1, Side::Sell, FUNDED_QUANTITY, 0),
    ]);
    let wide = fee_domain(TEST_COMPOSITE_LAB, 2, 4);
    let split_candidate = canonical_candidate(&wide, &split, &vector, 0, 0).unwrap();
    let split_summary = verify(&wide, &split, &split_candidate, None).unwrap();
    assert_fee_conservation(&split_summary);
    assert!(
        split_summary.fee_price_units > 0,
        "two separate owners moved real risk and must be charged"
    );
}

#[test]
fn score_v1_known_defect_sybil_complete_set_wash_is_free_scored_volume() {
    // KNOWN SCOREV1 DEFECT, not a desired invariant.  Two owner tags controlled
    // by one actor can cross a complete set.  Each side's filled payoff vector
    // is constant, their joint signed exposure is zero, and the composite
    // correctly charges zero.  But ScoreV1 only removes same-owner/same-outcome
    // overlap, so the two tags turn the wash into positive primary score.
    // A successor score must quotient constant complete-set directions (and
    // state its identity/Sybil assumption) before rewarding executed risk mass.
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let wash = book_of(&[
        single(1, 0, 0, Side::Buy, FUNDED_QUANTITY, SCALE),
        single(2, 0, 1, Side::Buy, FUNDED_QUANTITY, SCALE),
        single(3, 1, 0, Side::Sell, FUNDED_QUANTITY, 0),
        single(4, 1, 1, Side::Sell, FUNDED_QUANTITY, 0),
    ]);
    let domain = fee_domain(TEST_COMPOSITE_LAB, 2, 2);
    let candidate = canonical_candidate(&domain, &wash, &vector, 0, 0).unwrap();
    let summary = verify(&domain, &wash, &candidate, None).unwrap();

    // Derive the exposure rows from the relation's admitted book and actual
    // fills; these are not hand-written proxy payoff vectors.
    let normalized = normalize(&domain, &wash).unwrap();
    let mut participation = ParticipationV1::zeroed();
    participation_from_fills(&domain, &normalized, &candidate.fills, &mut participation).unwrap();
    assert_eq!(
        &participation.buy[0][..2],
        &[FUNDED_QUANTITY, FUNDED_QUANTITY]
    );
    assert_eq!(
        &participation.sell[1][..2],
        &[FUNDED_QUANTITY, FUNDED_QUANTITY]
    );
    assert_eq!(
        participation.buy[0][0].abs_diff(participation.buy[0][1]),
        0,
        "the buyer's state-dependent payoff range must be zero"
    );
    assert_eq!(
        participation.sell[1][0].abs_diff(participation.sell[1][1]),
        0,
        "the seller's state-dependent liability range must be zero"
    );
    for outcome in 0..2 {
        assert_eq!(
            participation.buy[0][outcome], participation.sell[1][outcome],
            "the Sybil pair's joint signed exposure must net to zero"
        );
    }

    assert_eq!(summary.virtual_split, 0);
    assert_eq!(summary.virtual_merge, 0);
    assert_eq!(summary.self_overlap_volume, 0);
    assert_eq!(
        &summary.direct_flow[..2],
        &[FUNDED_QUANTITY, FUNDED_QUANTITY]
    );
    assert_eq!(
        summary.fee_price_units, 0,
        "constant complete-set exposure is in the composite fee kernel"
    );
    assert_eq!(summary.fee_carry_bps_units, 0);

    let expected_weighted_direct_volume =
        2 * (FUNDED_QUANTITY as i128) * (SCALE as i128 / 2) * (SCALE as i128 / 2);
    assert_eq!(
        summary.score.weighted_direct_volume,
        expected_weighted_direct_volume
    );
    assert!(summary.score.weighted_direct_volume > 0);
    assert!(
        summary.score.is_better_than(&ScoreV1::ZERO),
        "the positive first component dominates the zero score"
    );
}

#[test]
fn composite_rate_admissibility_is_the_basis_point_band() {
    for (dispersion_bps, floor_range_bps, admitted) in [
        (0u32, 0u32, true),
        (1, 0, true),
        (0, 1, true),
        (40, 10, true),
        (10_000, 10_000, true),
        (10_001, 0, false),
        (0, 10_001, false),
        (u32::MAX, u32::MAX, false),
    ] {
        let policy = FrozenPolicyV1 {
            fee_base: FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps,
                floor_range_bps,
            },
            ..base_policy()
        };
        assert_eq!(
            policy.validate().is_ok(),
            admitted,
            "({dispersion_bps}, {floor_range_bps}) admissibility moved"
        );
    }
    // A fee-bearing composite domain carries the tighter price-scale bound; the
    // zero-rate shape does not, because it never forms a denominator.
    let wide = MAX_COMPOSITE_PRICE_SCALE + 1;
    let mut domain = fee_domain(TEST_COMPOSITE_LAB, 2, 2);
    domain.price_scale = wide;
    assert_eq!(domain.validate(), Err(ErrorV1::InvalidPriceScale));
    domain.policy.fee_base = TEST_COMPOSITE_ZERO;
    assert_eq!(domain.validate(), Ok(()));
    domain.policy.fee_base = FeeBaseV1::None;
    assert_eq!(domain.validate(), Ok(()));
}

#[test]
fn no_test_rate_pair_is_a_frozen_production_const() {
    // The discipline this lane must not break: rates are ember's decision.
    // Every nonzero pair in this crate is a laboratory calibration, and the one
    // shape a frozen artifact may carry is the zero pair.
    assert_eq!(
        TEST_COMPOSITE_ZERO,
        FeeBaseV1::CompositeDispersionFloor {
            dispersion_bps: 0,
            floor_range_bps: 0,
        }
    );
    for calibration in [
        TEST_COMPOSITE_SMALL,
        TEST_COMPOSITE_LAB,
        TEST_COMPOSITE_DISPERSION_ONLY,
        TEST_COMPOSITE_FLOOR_ONLY,
        TEST_COMPOSITE_BOUNDARY,
    ] {
        assert_ne!(
            calibration, TEST_COMPOSITE_ZERO,
            "a calibration collapsed onto the anchor"
        );
        let (dispersion_bps, floor_range_bps) = rates(calibration);
        assert!(dispersion_bps != 0 || floor_range_bps != 0);
    }
}
