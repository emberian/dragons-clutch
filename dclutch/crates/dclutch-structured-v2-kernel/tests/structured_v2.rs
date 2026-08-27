//! Behaviour and refusal evidence for the shard-backed Structured V2 kernel.

mod support;

use dclutch_structured_v2_kernel::{
    Error, STRUCTURED_TERMS_HEADER_BYTES_V2, ShardMovementV2, StructuredCoordinateObservationV2,
    StructuredPhaseV2, StructuredProjectionV2, StructuredSettlementRowV2, StructuredTermsInputV2,
    StructuredTermsV2, encode_structured_terms_v2, plan_structured_issue_v2,
    plan_structured_retire_v2, plan_structured_terminal_redeem_v2, plan_structured_unwrap_v2,
    structured_terms_bytes_v2,
};
use support::{
    GRAPH_ID, MARKET, PRODUCT_RECORD, RECEIPT_MINT, RECEIPT_TOKEN_BEHAVIOR, RELEASE_SET,
    RESULT_DOMAIN, SHARD_EXPOSURE, TOKEN_PROGRAM, digest, exact_rows, identity, projection_bytes,
    shard_mints, shard_terms, shard_terms_bytes, structured_admission, structured_terms,
    structured_terms_bytes, structured_terms_bytes_with,
};

const DENOMINATOR: u64 = 4;
const COEFFICIENTS: [u64; 2] = [1, 3];

fn movements() -> Vec<ShardMovementV2> {
    vec![ShardMovementV2::default(); COEFFICIENTS.len()]
}

fn settlement() -> Vec<StructuredSettlementRowV2> {
    vec![StructuredSettlementRowV2::default(); COEFFICIENTS.len()]
}

fn row(index: usize, rows: &[ShardMovementV2]) -> ShardMovementV2 {
    rows.get(index).copied().expect("movement row")
}

fn settled(index: usize, rows: &[StructuredSettlementRowV2]) -> StructuredSettlementRowV2 {
    rows.get(index).copied().expect("settlement row")
}

#[test]
fn terms_round_trip_exposes_exact_coefficients() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR);
    let terms = structured_terms(&terms_bytes, shard);
    assert_eq!(terms.representation_width(), 2);
    assert_eq!(terms.denominator(), DENOMINATOR);
    assert_eq!(terms.coefficient(0), Ok(1));
    assert_eq!(terms.coefficient(1), Ok(3));
    assert_eq!(terms.coefficient(2), Err(Error::InvalidCoordinate));
    assert_eq!(terms.receipt_mint(), identity(RECEIPT_MINT));
    assert_eq!(terms.shard_terms(), digest(&shard_bytes));
    assert_eq!(terms.terms_id(), digest(&terms_bytes));
    // The exact backing invariant K_i = S * c_i.
    assert_eq!(terms.required_shard_custody(0, 10), Ok(10));
    assert_eq!(terms.required_shard_custody(1, 10), Ok(30));
    assert_eq!(terms.required_shard_custody(0, 0), Ok(0));
}

#[test]
fn issue_locks_the_exact_basket_and_mints_receipts() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR);
    let terms = structured_terms(&terms_bytes, shard);
    let rows = exact_rows(&COEFFICIENTS, 10, &[0, 0]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Open, 10, 7, &rows);
    let projection = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    let mut plan_rows = movements();
    let plan = plan_structured_issue_v2(terms, shard, projection, 4, 10, &mut plan_rows)
        .expect("issue plan");
    assert_eq!(plan.receipt.receipt_atoms, 4);
    assert_eq!(plan.receipt.pre_receipt_supply, 10);
    assert_eq!(plan.receipt.post_receipt_supply, 14);
    assert_eq!(plan.receipt.post_actor_receipts, 14);
    assert_eq!(plan.receipt.next_revision, 8);
    assert_eq!(plan.total_shard_atoms, 16);
    assert_eq!(row(0, &plan_rows).shard_atoms, 4);
    assert_eq!(row(1, &plan_rows).shard_atoms, 12);
    // Exact backing after the action: K_i = 14 * c_i.
    assert_eq!(row(0, &plan_rows).post_required_custody, 14);
    assert_eq!(row(1, &plan_rows).post_required_custody, 42);
    assert_eq!(row(0, &plan_rows).surplus_shard_custody, 0);
    assert_eq!(
        row(0, &plan_rows).shard_mint,
        shard_mints(2).first().copied().expect("mint")
    );
}

#[test]
fn unwrap_releases_the_exact_basket() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR);
    let terms = structured_terms(&terms_bytes, shard);
    let rows = exact_rows(&COEFFICIENTS, 10, &[0, 0]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Open, 10, 7, &rows);
    let projection = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    let mut plan_rows = movements();
    let plan = plan_structured_unwrap_v2(terms, shard, projection, 3, 10, &mut plan_rows)
        .expect("unwrap plan");
    assert_eq!(plan.receipt.post_receipt_supply, 7);
    assert_eq!(plan.receipt.post_actor_receipts, 7);
    assert_eq!(plan.total_shard_atoms, 12);
    assert_eq!(row(0, &plan_rows).shard_atoms, 3);
    assert_eq!(row(1, &plan_rows).shard_atoms, 9);
    assert_eq!(row(0, &plan_rows).post_required_custody, 7);
    assert_eq!(row(1, &plan_rows).post_required_custody, 21);
}

#[test]
fn terminal_redeem_settles_exactly_and_losers_pay_zero() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR);
    let terms = structured_terms(&terms_bytes, shard);
    let rows = exact_rows(&COEFFICIENTS, 10, &[0, 5]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Terminal, 10, 7, &rows);
    let projection = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    let mut plan_rows = movements();
    let mut settlement_rows = settlement();
    let plan = plan_structured_terminal_redeem_v2(
        terms,
        shard,
        projection,
        10,
        10,
        &mut plan_rows,
        &mut settlement_rows,
    )
    .expect("terminal plan");
    assert_eq!(plan.release.receipt.post_receipt_supply, 0);
    assert_eq!(plan.total_collateral_atoms, 35);
    let losing = settled(0, &settlement_rows);
    assert_eq!(losing.released_shards, 10);
    assert_eq!(losing.whole_claims, 2);
    assert_eq!(losing.burned_shards, 8);
    assert_eq!(losing.change_shards, 2);
    assert_eq!(losing.payout_per_claim, 0);
    // Terminal-zero honesty: two whole native claims still settle for zero.
    assert_eq!(losing.collateral_atoms, 0);
    let winning = settled(1, &settlement_rows);
    assert_eq!(winning.released_shards, 30);
    assert_eq!(winning.whole_claims, 7);
    assert_eq!(winning.burned_shards, 28);
    assert_eq!(winning.change_shards, 2);
    assert_eq!(winning.collateral_atoms, 35);
    // No hidden rounding: released = burned + explicit change, change < D.
    for settled_row in [losing, winning] {
        assert_eq!(
            settled_row.released_shards,
            settled_row.burned_shards + settled_row.change_shards
        );
        assert!(settled_row.change_shards < DENOMINATOR);
        assert_eq!(
            settled_row.burned_shards,
            settled_row.whole_claims * DENOMINATOR
        );
    }
}

#[test]
fn sub_denominator_release_is_explicit_change_not_rounding() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR);
    let terms = structured_terms(&terms_bytes, shard);
    let rows = exact_rows(&COEFFICIENTS, 10, &[0, 5]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Terminal, 10, 7, &rows);
    let projection = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    let mut plan_rows = movements();
    let mut settlement_rows = settlement();
    let plan = plan_structured_terminal_redeem_v2(
        terms,
        shard,
        projection,
        3,
        10,
        &mut plan_rows,
        &mut settlement_rows,
    )
    .expect("terminal plan");
    let inert = settled(0, &settlement_rows);
    assert_eq!(inert.released_shards, 3);
    assert_eq!(inert.whole_claims, 0);
    assert_eq!(inert.burned_shards, 0);
    assert_eq!(inert.change_shards, 3);
    let winning = settled(1, &settlement_rows);
    assert_eq!(winning.released_shards, 9);
    assert_eq!(winning.whole_claims, 2);
    assert_eq!(winning.change_shards, 1);
    assert_eq!(plan.total_collateral_atoms, 10);
}

#[test]
fn zero_coefficient_row_is_admissible_and_inert() {
    let coefficients = [0_u64, 3];
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&coefficients, DENOMINATOR);
    let terms = structured_terms(&terms_bytes, shard);
    let rows = exact_rows(&coefficients, 10, &[7, 5]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Terminal, 10, 7, &rows);
    let projection = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    let mut plan_rows = movements();
    let mut settlement_rows = settlement();
    let plan = plan_structured_terminal_redeem_v2(
        terms,
        shard,
        projection,
        10,
        10,
        &mut plan_rows,
        &mut settlement_rows,
    )
    .expect("terminal plan");
    assert_eq!(row(0, &plan_rows).shard_atoms, 0);
    let inert = settled(0, &settlement_rows);
    assert_eq!(inert.released_shards, 0);
    assert_eq!(inert.whole_claims, 0);
    assert_eq!(inert.change_shards, 0);
    // A positive payout on an unbacked coordinate still settles for zero.
    assert_eq!(inert.payout_per_claim, 7);
    assert_eq!(inert.collateral_atoms, 0);
    assert_eq!(plan.total_collateral_atoms, 35);
}

#[test]
fn donated_surplus_is_named_and_never_becomes_backing() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR);
    let terms = structured_terms(&terms_bytes, shard);
    let rows = vec![
        StructuredCoordinateObservationV2 {
            observed_shard_custody: 10 + 99,
            payout_per_claim: 0,
        },
        StructuredCoordinateObservationV2 {
            observed_shard_custody: 30,
            payout_per_claim: 0,
        },
    ];
    let bytes = projection_bytes(terms, StructuredPhaseV2::Open, 10, 7, &rows);
    let projection = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    assert_eq!(projection.surplus_shard_custody(terms, 0), Ok(99));
    assert_eq!(projection.surplus_shard_custody(terms, 1), Ok(0));
    let mut plan_rows = movements();
    let plan = plan_structured_unwrap_v2(terms, shard, projection, 10, 10, &mut plan_rows)
        .expect("unwrap plan");
    // The donation neither funds nor is released by the action.
    assert_eq!(plan.total_shard_atoms, 40);
    assert_eq!(row(0, &plan_rows).shard_atoms, 10);
    assert_eq!(row(0, &plan_rows).surplus_shard_custody, 99);
    assert_eq!(row(0, &plan_rows).post_required_custody, 0);
}

#[test]
fn retirement_requires_zero_supply_and_zero_observed_custody() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR);
    let terms = structured_terms(&terms_bytes, shard);

    let empty = exact_rows(&COEFFICIENTS, 0, &[0, 5]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Terminal, 0, 7, &empty);
    let projection = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    let plan = plan_structured_retire_v2(terms, shard, projection).expect("retire plan");
    assert_eq!(plan.next_revision, 8);
    assert_eq!(plan.receipt_mint, identity(RECEIPT_MINT));
    assert_eq!(plan.representation_width, 2);

    let outstanding = exact_rows(&COEFFICIENTS, 1, &[0, 5]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Terminal, 1, 7, &outstanding);
    let projection = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    assert_eq!(
        plan_structured_retire_v2(terms, shard, projection),
        Err(Error::OutstandingReceiptSupply)
    );

    let donated = vec![
        StructuredCoordinateObservationV2 {
            observed_shard_custody: 5,
            payout_per_claim: 0,
        },
        StructuredCoordinateObservationV2 {
            observed_shard_custody: 0,
            payout_per_claim: 5,
        },
    ];
    let bytes = projection_bytes(terms, StructuredPhaseV2::Terminal, 0, 7, &donated);
    let projection = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    assert_eq!(
        plan_structured_retire_v2(terms, shard, projection),
        Err(Error::OutstandingShardCustody)
    );

    let open = exact_rows(&COEFFICIENTS, 0, &[0, 0]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Open, 0, 7, &open);
    let projection = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    assert_eq!(
        plan_structured_retire_v2(terms, shard, projection),
        Err(Error::InvalidPhase)
    );
}

#[test]
fn phase_gates_every_action() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR);
    let terms = structured_terms(&terms_bytes, shard);
    let mut plan_rows = movements();
    let mut settlement_rows = settlement();

    let terminal_rows = exact_rows(&COEFFICIENTS, 10, &[0, 5]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Terminal, 10, 7, &terminal_rows);
    let terminal = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    assert_eq!(
        plan_structured_issue_v2(terms, shard, terminal, 1, 10, &mut plan_rows),
        Err(Error::InvalidPhase)
    );
    assert_eq!(
        plan_structured_unwrap_v2(terms, shard, terminal, 1, 10, &mut plan_rows),
        Err(Error::InvalidPhase)
    );

    let open_rows = exact_rows(&COEFFICIENTS, 10, &[0, 0]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Open, 10, 7, &open_rows);
    let open = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    assert_eq!(
        plan_structured_terminal_redeem_v2(
            terms,
            shard,
            open,
            1,
            10,
            &mut plan_rows,
            &mut settlement_rows
        ),
        Err(Error::InvalidPhase)
    );
}

#[test]
fn balance_and_quantity_refusals_are_explicit() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR);
    let terms = structured_terms(&terms_bytes, shard);
    let rows = exact_rows(&COEFFICIENTS, 10, &[0, 0]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Open, 10, 7, &rows);
    let projection = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    let mut plan_rows = movements();

    assert_eq!(
        plan_structured_issue_v2(terms, shard, projection, 0, 10, &mut plan_rows),
        Err(Error::ZeroQuantity)
    );
    // Redeeming more than the actor holds refuses.
    assert_eq!(
        plan_structured_unwrap_v2(terms, shard, projection, 5, 4, &mut plan_rows),
        Err(Error::InsufficientBalance)
    );
    // A holder balance beyond total supply refuses.
    assert_eq!(
        plan_structured_unwrap_v2(terms, shard, projection, 5, 11, &mut plan_rows),
        Err(Error::InsufficientBalance)
    );
    // Caller storage must be exactly the representation width.
    let mut short = vec![ShardMovementV2::default(); 1];
    assert_eq!(
        plan_structured_issue_v2(terms, shard, projection, 1, 10, &mut short),
        Err(Error::InvalidLength)
    );
}

#[test]
fn overflow_refuses_rather_than_wrapping() {
    let coefficients = [u64::MAX / 2, 3];
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&coefficients, DENOMINATOR);
    let terms = structured_terms(&terms_bytes, shard);
    let rows = exact_rows(&coefficients, 1, &[0, 0]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Open, 1, 7, &rows);
    let projection = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    let mut plan_rows = movements();
    assert_eq!(
        plan_structured_issue_v2(terms, shard, projection, 4, 1, &mut plan_rows),
        Err(Error::ArithmeticOverflow)
    );
}

#[test]
fn substituted_shard_layer_refuses() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR);

    // A same-shaped shard layer at another denominator is not this basis.
    let other_bytes = shard_terms_bytes(2, 8);
    let other = shard_terms(&other_bytes);
    assert_eq!(
        StructuredTermsV2::decode(&terms_bytes, structured_admission(&terms_bytes), other),
        Err(Error::ShardLayerMismatch)
    );

    // A same-denominator shard layer with substituted Mints is not this basis.
    let mut mints = shard_mints(2);
    if let Some(first) = mints.first_mut() {
        *first = identity(0x7e);
    }
    let substituted_bytes = support::shard_terms_bytes_with_mints(&mints, DENOMINATOR);
    let substituted = shard_terms(&substituted_bytes);
    assert_eq!(
        StructuredTermsV2::decode(
            &terms_bytes,
            structured_admission(&terms_bytes),
            substituted
        ),
        Err(Error::ShardLayerMismatch)
    );

    // The correct layer still admits.
    assert!(
        StructuredTermsV2::decode(&terms_bytes, structured_admission(&terms_bytes), shard).is_ok()
    );
}

#[test]
fn receipt_mint_aliasing_a_shard_mint_refuses() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let aliased = shard_mints(2).first().copied().expect("mint");
    let terms_bytes = structured_terms_bytes_with(&COEFFICIENTS, DENOMINATOR, aliased);
    assert_eq!(
        StructuredTermsV2::decode(&terms_bytes, structured_admission(&terms_bytes), shard),
        Err(Error::DuplicateIdentity)
    );
}

#[test]
fn unbacked_and_degenerate_bases_refuse() {
    let size = structured_terms_bytes_v2(2).expect("width");
    let mut scratch = vec![0_u8; size];
    let mut output = vec![0_u8; size];
    let base = StructuredTermsInputV2 {
        market: identity(MARKET),
        product_record: identity(PRODUCT_RECORD),
        result_domain: identity(RESULT_DOMAIN),
        release_set: identity(RELEASE_SET),
        token_program: identity(TOKEN_PROGRAM),
        token_behavior: identity(RECEIPT_TOKEN_BEHAVIOR),
        shard_terms: digest(&shard_terms_bytes(2, DENOMINATOR)),
        shard_exposure: identity(SHARD_EXPOSURE),
        receipt_mint: identity(RECEIPT_MINT),
        graph_id: identity(GRAPH_ID),
        denominator: DENOMINATOR,
        coefficients: &[0, 0],
    };
    assert_eq!(
        encode_structured_terms_v2(base, &mut scratch, &mut output),
        Err(Error::UnbackedBasis)
    );
    assert_eq!(
        encode_structured_terms_v2(
            StructuredTermsInputV2 {
                denominator: 1,
                coefficients: &COEFFICIENTS,
                ..base
            },
            &mut scratch,
            &mut output,
        ),
        Err(Error::NonFractionalDenominator)
    );
    assert_eq!(
        encode_structured_terms_v2(
            StructuredTermsInputV2 {
                market: [0; 32],
                coefficients: &COEFFICIENTS,
                ..base
            },
            &mut scratch,
            &mut output,
        ),
        Err(Error::ZeroIdentity)
    );
}

#[test]
fn hostile_terms_bytes_refuse() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let accepted = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR);
    let admission = structured_admission(&accepted);

    let mut wrong_magic = accepted.clone();
    if let Some(byte) = wrong_magic.first_mut() {
        *byte ^= 0xff;
    }
    assert_eq!(
        StructuredTermsV2::decode(&wrong_magic, structured_admission(&wrong_magic), shard),
        Err(Error::InvalidMagic)
    );

    let mut wrong_version = accepted.clone();
    if let Some(byte) = wrong_version.get_mut(8) {
        *byte = 9;
    }
    assert_eq!(
        StructuredTermsV2::decode(&wrong_version, structured_admission(&wrong_version), shard),
        Err(Error::UnsupportedVersion)
    );

    let mut dirty_reserved = accepted.clone();
    if let Some(byte) = dirty_reserved.get_mut(12) {
        *byte = 1;
    }
    assert_eq!(
        StructuredTermsV2::decode(
            &dirty_reserved,
            structured_admission(&dirty_reserved),
            shard
        ),
        Err(Error::NonCanonical)
    );

    let mut nonzero_decimals = accepted.clone();
    if let Some(byte) = nonzero_decimals.get_mut(10) {
        *byte = 6;
    }
    assert_eq!(
        StructuredTermsV2::decode(
            &nonzero_decimals,
            structured_admission(&nonzero_decimals),
            shard
        ),
        Err(Error::NonCanonical)
    );

    let truncated = accepted
        .get(..accepted.len() - 1)
        .expect("truncate")
        .to_vec();
    assert_eq!(
        StructuredTermsV2::decode(&truncated, structured_admission(&truncated), shard),
        Err(Error::InvalidLength)
    );

    let mut extended = accepted.clone();
    extended.push(0);
    assert_eq!(
        StructuredTermsV2::decode(&extended, structured_admission(&extended), shard),
        Err(Error::InvalidLength)
    );

    let header_only = accepted
        .get(..STRUCTURED_TERMS_HEADER_BYTES_V2)
        .expect("header")
        .to_vec();
    assert_eq!(
        StructuredTermsV2::decode(&header_only, structured_admission(&header_only), shard),
        Err(Error::InvalidLength)
    );

    // A correct byte string with unauthenticated Record evidence refuses.
    assert_eq!(
        StructuredTermsV2::decode(
            &accepted,
            StructuredTermsAdmission::unauthenticated(admission),
            shard
        ),
        Err(Error::UnauthenticatedRecord)
    );
    assert_eq!(
        StructuredTermsV2::decode(
            &accepted,
            StructuredTermsAdmission::wrong_digest(admission),
            shard
        ),
        Err(Error::AdmissionMismatch)
    );
}

struct StructuredTermsAdmission;

impl StructuredTermsAdmission {
    fn unauthenticated(
        admission: dclutch_structured_v2_kernel::StructuredTermsAdmissionV2,
    ) -> dclutch_structured_v2_kernel::StructuredTermsAdmissionV2 {
        dclutch_structured_v2_kernel::StructuredTermsAdmissionV2 {
            record_authenticated: false,
            ..admission
        }
    }

    fn wrong_digest(
        admission: dclutch_structured_v2_kernel::StructuredTermsAdmissionV2,
    ) -> dclutch_structured_v2_kernel::StructuredTermsAdmissionV2 {
        dclutch_structured_v2_kernel::StructuredTermsAdmissionV2 {
            recomputed_terms_digest: identity(0x5c),
            ..admission
        }
    }
}

#[test]
fn hostile_projection_bytes_refuse() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR);
    let terms = structured_terms(&terms_bytes, shard);

    // Backing shortfall refuses: observed custody below the exact requirement.
    let short = vec![
        StructuredCoordinateObservationV2 {
            observed_shard_custody: 9,
            payout_per_claim: 0,
        },
        StructuredCoordinateObservationV2 {
            observed_shard_custody: 30,
            payout_per_claim: 0,
        },
    ];
    let bytes = projection_bytes(terms, StructuredPhaseV2::Open, 10, 7, &short);
    assert_eq!(
        StructuredProjectionV2::decode(&bytes, terms),
        Err(Error::BackingMismatch)
    );

    // A payout before terminal resolution is noncanonical.
    let early = exact_rows(&COEFFICIENTS, 10, &[0, 5]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Open, 10, 7, &early);
    assert_eq!(
        StructuredProjectionV2::decode(&bytes, terms),
        Err(Error::NonCanonical)
    );

    // A retired projection with outstanding supply is noncanonical.
    let retired = exact_rows(&COEFFICIENTS, 0, &[0, 0]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Retired, 3, 7, &retired);
    assert_eq!(
        StructuredProjectionV2::decode(&bytes, terms),
        Err(Error::NonCanonical)
    );

    // An unknown phase tag is noncanonical.
    let ok_rows = exact_rows(&COEFFICIENTS, 10, &[0, 0]);
    let mut bytes = projection_bytes(terms, StructuredPhaseV2::Open, 10, 7, &ok_rows);
    if let Some(byte) = bytes.get_mut(10) {
        *byte = 9;
    }
    assert_eq!(
        StructuredProjectionV2::decode(&bytes, terms),
        Err(Error::NonCanonical)
    );

    // A projection describing another Structured basis refuses.
    let other_terms_bytes = structured_terms_bytes(&[2, 5], DENOMINATOR);
    let other_terms = structured_terms(&other_terms_bytes, shard);
    let other_rows = exact_rows(&[2, 5], 10, &[0, 0]);
    let bytes = projection_bytes(other_terms, StructuredPhaseV2::Open, 10, 7, &other_rows);
    assert_eq!(
        StructuredProjectionV2::decode(&bytes, terms),
        Err(Error::AdmissionMismatch)
    );
}

#[test]
fn projection_round_trip_preserves_every_observation() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR);
    let terms = structured_terms(&terms_bytes, shard);
    let rows = exact_rows(&COEFFICIENTS, 12, &[0, 5]);
    let bytes = projection_bytes(terms, StructuredPhaseV2::Terminal, 12, 41, &rows);
    let projection = StructuredProjectionV2::decode(&bytes, terms).expect("projection");
    assert_eq!(projection.phase(), StructuredPhaseV2::Terminal);
    assert_eq!(projection.receipt_supply(), 12);
    assert_eq!(projection.revision(), 41);
    assert_eq!(projection.denominator(), DENOMINATOR);
    assert_eq!(projection.representation_width(), 2);
    assert_eq!(projection.shard_terms(), digest(&shard_bytes));
    assert_eq!(
        projection.observation(0),
        Ok(StructuredCoordinateObservationV2 {
            observed_shard_custody: 12,
            payout_per_claim: 0,
        })
    );
    assert_eq!(
        projection.observation(1),
        Ok(StructuredCoordinateObservationV2 {
            observed_shard_custody: 36,
            payout_per_claim: 5,
        })
    );
    assert_eq!(projection.observation(2), Err(Error::InvalidCoordinate));
}
