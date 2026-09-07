//! The first Claims founding executed against a real ELF.
//!
//! Nothing in this tree had ever run one. `found_program_test.rs` drives Core's
//! Found stage to `Founding + Prepaid` and stops there; every Claims
//! program-test genesis-plants the aggregate with
//! `encode_liability_basis_market_v2` instead of founding it; and the only
//! executor of `DCLFDR05` is the local-validator bootstrap, against a live
//! cluster. So the route that CREATES every Claims aggregate — and, since
//! CLAIMS-17, seats the failure escrow — had no fixture-level evidence at all.
//!
//! # What had to exist first
//!
//! One thing, and it is why this was never written: `authenticate_authority`
//! requires the frame's account 0 to be the PDA
//! `CallerAuthoritySeedsV1(release_set, market, Trading,
//! founding_intent_digest, request_digest)` addresses **under the request's own
//! trading program**, and `invoke_signed` signs only for the calling program's
//! own addresses. `programs/dclutch-claims-sbf/test-programs/founding-caller`
//! is that caller. It declares no refusal code of its own on purpose, so a
//! founding's refusal reaches this file as the founding's own discriminant
//! rather than as one wrapper code covering thirty-three account conjuncts.
//!
//! # The prestate is forged, and every digest that binds it is stated here
//!
//! The permit is PLANTED as canonical Core-owned bytes rather than issued by
//! Core's Found stage. That is the fixture line this campaign draws, and it is
//! drawn where the tree already draws it — `narrow_fixture` plants the Core
//! state and the Product graph for the same reason. What is NOT weakened is the
//! join: the permit's `FoundingIntentV5` still has to satisfy every one of
//! `authenticate_permit_body`'s named conjuncts, both projected-custody
//! receipts still have to hash to what the request commits to, and the Custody
//! replay cursor still has to name the realization receipt's own digest. The
//! chain this file builds, in dependency order, is:
//!
//! ```text
//!   ticket_context ─hashv(PROJECTED_HOARD_CONTEXT_DOMAIN_V1)→ projected_context
//!   lock receipt ─hash→ request.custody_receipt_digest
//!   core state   ─hash→ projected_receipt.market_state_digest
//!   projected receipt ─hash→ replay.last_poststate_commitment
//!   intent ─hash→ request.founding_intent_digest = permit.claims_intent_digest
//!   request ─hash→ permit.claims_request_digest, and the authority PDA's last seed
//! ```
//!
//! Nothing in that chain is asserted; each link is computed from the previous
//! one, so a fixture that drifts refuses rather than passing.
//!
//! # CU per stage, and a correction to the headline number
//!
//! Measured at `a514cace` on an ELF built `--features claims-cu-profile`, which
//! is a DIFFERENT ARTIFACT from the one the campaigns above run: its totals are
//! 234,825 and 200,941 against the shipped 240,040 and 209,160. Only the shape
//! below is evidence; the two totals to quote are the shipped ones.
//!
//! | stage | refunding | categorical | difference |
//! |---|---|---|---|
//! | `found-frame` (decode, parse, privileges) | 7,388 | 7,388 | 0 |
//! | `found-authority` | 3,563 | 2,063 | +1,500 |
//! | `found-releases` (the four-role loop) | 47,165 | 47,165 | 0 |
//! | `found-permit` (permit, both receipts) | 5,350 | 5,350 | 0 |
//! | `found-custody` (poststate, replay) | 2,044 | 2,044 | 0 |
//! | `found-product-core` (record walk) | 47,159 | 35,154 | +12,005 |
//! | `found-rent-vacancy` (+ escrow seating) | 26,073 | 26,070 | +3 |
//! | `found-candidates` | 17,101 | 13,850 | +3,251 |
//! | `found-allocate` (System CPIs) | 25,850 | 14,328 | +11,522 |
//! | `found-commit` | 10,469 | 6,366 | +4,103 |
//! | enter to commit | 192,162 | 159,778 | +32,384 |
//!
//! **THE 30,880 IS NOT ALL ESCROW, and the table is what says so.** The escrow's
//! own work is `found-candidates` + `found-allocate` + `found-commit` — the
//! second Position and admission built, allocated over four more System CPIs,
//! and copied — and that is about **18,900**. `found-authority`'s +1,500 and
//! `found-product-core`'s +12,005 are `find_program_address` iteration variance:
//! the two campaigns carry different request and record digests, so their bump
//! searches take different numbers of turns, and a turn costs about 1,500. A
//! lane that quoted the whole difference as the escrow's price would be pricing
//! the fixture's digests.
//!
//! `found-releases` is the other number worth having: **47,165 CU, a quarter of
//! this route's own consumption, and identical under both shapes.** The route's
//! own comment asks for exactly this quantity — "`found-releases` minus
//! `found-authority` is that question's answer for this route" — because a
//! Claims child is one `consumed` line in its caller's log, and one number
//! cannot say whether it was spent on work only Claims can do or on
//! re-establishing a release set its caller had already established. It is the
//! latter, and it is the single largest stage.
//!
//! # Both shapes, over one fixture
//!
//! The categorical founding and the refunding one differ in exactly one input —
//! the basis record's payout scale, `1` against `basis_width - 1` — because
//! `categorical_refunds_on_failure_v3` reads the shape off the RECORD and no
//! caller states it. Everything else, including all thirty-three accounts, is
//! identical. That is decision 0025 item 2's claim made checkable: the escrow
//! accounts ride on both frames, and only the refunding founding allocates them.

use dclutch_claims::{
    founding_v5::{
        CLAIMS_FOUNDING_RECEIPT_BYTES_V5, CLAIMS_FOUNDING_REQUEST_BYTES_V5, ClaimsFoundingReceiptV5,
    },
    liability_basis_state_v2::{LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2},
    protocol_position_v2::ProtocolPositionAdmissionV2,
};
use dclutch_claims_sbf::founding_v5::ClaimsFoundingSbfErrorV5;
use dclutch_fractional_atomic_program_test::founding_world::{
    CLAIM_COUNT, CLAIMS_PROGRAM_ID, FoundingShapeV1, HostileV1, QUANTITY, found, world,
};
use solana_sdk_ids::system_program;

// ---------------------------------------------------------------------------
/// A REFUNDING founding, executed on the real Claims ELF.
///
/// The aggregate, the founder's Position and its admission come into existence,
/// and so do the escrow's Position and admission, because the record says the
/// Market refunds. The founder holds every ordinary coordinate and no failure
/// claim; the escrow holds the failure column and nothing else; the two sum to
/// one complete set at every coordinate.
#[tokio::test]
async fn a_refunding_founding_seats_the_escrow_on_the_real_elf() {
    let (world, mut context, outcome) = found(
        FoundingShapeV1::Refunding,
        HostileV1::None,
        "claims founding: refunding, escrow seated",
    )
    .await;
    assert!(
        outcome.accepted,
        "the refunding founding must be accepted; refusal {:?}, logs {:#?}",
        outcome.refusal, outcome.logs,
    );
    println!(
        "claims founding (refunding, width {CLAIM_COUNT}): accepted, {} CU consumed",
        outcome.units,
    );

    let aggregate = context
        .banks_client
        .get_account(world.aggregate)
        .await
        .expect("aggregate query")
        .expect("the founding created the aggregate");
    assert_eq!(aggregate.owner, CLAIMS_PROGRAM_ID);
    let market = LiabilityBasisMarketViewV2::decode(&aggregate.data).expect("aggregate decodes");
    assert_eq!(market.claim_count, CLAIM_COUNT);
    assert_eq!(market.revision, 1);
    for coordinate in 0..CLAIM_COUNT {
        assert_eq!(
            market.supply(&aggregate.data, coordinate).expect("supply"),
            QUANTITY,
            "a founding issues one complete set: supply is uniform at coordinate {coordinate}",
        );
    }

    let founder_position = context
        .banks_client
        .get_account(world.position)
        .await
        .expect("position query")
        .expect("the founding created the founder Position");
    let founder = LiabilityBasisPositionViewV2::decode(&founder_position.data).expect("decodes");
    let escrow_account = context
        .banks_client
        .get_account(world.escrow_position)
        .await
        .expect("escrow query")
        .expect("a refunding founding creates the escrow Position");
    assert_eq!(escrow_account.owner, CLAIMS_PROGRAM_ID);
    let escrow = LiabilityBasisPositionViewV2::decode(&escrow_account.data).expect("decodes");

    let failure = CLAIM_COUNT - 1;
    for coordinate in 0..CLAIM_COUNT {
        let held = founder
            .balance(&founder_position.data, coordinate)
            .expect("founder balance")
            + escrow
                .balance(&escrow_account.data, coordinate)
                .expect("escrow balance");
        assert_eq!(
            held, QUANTITY,
            "the two Positions sum to one complete set at coordinate {coordinate}",
        );
    }
    assert_eq!(
        founder
            .balance(&founder_position.data, failure)
            .expect("founder failure balance"),
        0,
        "the founder is issued NO failure claim on a refunding Market",
    );
    assert_eq!(
        escrow
            .balance(&escrow_account.data, failure)
            .expect("escrow failure balance"),
        QUANTITY,
        "the escrow holds the whole failure column",
    );
    assert_eq!(
        escrow_account.lamports,
        world.position_rent + world.bond,
        "the escrow Position holds its rent AND the founder bond; the founding \
         neither tops the bond up nor spends it",
    );

    let admission_account = context
        .banks_client
        .get_account(world.escrow_admission)
        .await
        .expect("escrow admission query")
        .expect("a refunding founding writes the escrow's ClaimsCapability admission");
    assert_eq!(admission_account.owner, CLAIMS_PROGRAM_ID);
    // Nothing is written FOR the bond: the admission the seating already wrote
    // is its record, and these two fields are what every later reader -- the
    // payout draw, the closure, the census -- subtracts to recover it
    // (decision 0030).
    let admitted = ProtocolPositionAdmissionV2::decode(&admission_account.data)
        .expect("the escrow admission decodes")
        .request();
    assert_eq!(
        admitted.position_rent_principal, world.position_rent,
        "the admission records the rent the escrow was funded at",
    );
    assert_eq!(
        admitted.observed_position_lamports,
        world.position_rent + world.bond,
        "and the balance observed beside it, so the bond is exactly the \
         difference of two fields of one account",
    );
}

/// A CATEGORICAL founding over the same thirty-three accounts.
///
/// Only the basis record's payout scale differs. The escrow accounts are in the
/// frame and stay vacant, the founder holds the whole complete set including
/// the last coordinate, and the receipt's post-resource transcript is
/// byte-identical to what three accounts produced before the escrow existed --
/// which is what the V5-to-V6 frame change promised.
#[tokio::test]
async fn a_categorical_founding_leaves_the_escrow_accounts_vacant() {
    let (world, mut context, outcome) = found(
        FoundingShapeV1::Categorical,
        HostileV1::None,
        "claims founding: categorical, no escrow",
    )
    .await;
    assert!(
        outcome.accepted,
        "the categorical founding must be accepted; refusal {:?}, logs {:#?}",
        outcome.refusal, outcome.logs,
    );
    println!(
        "claims founding (categorical, width {CLAIM_COUNT}): accepted, {} CU consumed",
        outcome.units,
    );
    let founder_position = context
        .banks_client
        .get_account(world.position)
        .await
        .expect("position query")
        .expect("the founding created the founder Position");
    let founder = LiabilityBasisPositionViewV2::decode(&founder_position.data).expect("decodes");
    for coordinate in 0..CLAIM_COUNT {
        assert_eq!(
            founder
                .balance(&founder_position.data, coordinate)
                .expect("founder balance"),
            QUANTITY,
            "a categorical founder holds the whole complete set, coordinate {coordinate}",
        );
    }
    let escrow = context
        .banks_client
        .get_account(world.escrow_position)
        .await
        .expect("escrow query");
    assert!(
        escrow.is_none_or(|account| account.owner == system_program::ID && account.data.is_empty()),
        "a categorical founding allocates neither escrow account",
    );
}

/// The fixture's own arithmetic, without a bank.
///
/// Every number the campaigns above submit is derived rather than typed, and
/// this is where that is checkable: the collateral is the exact product, the
/// rents are the widths' own minimums, and the two shapes differ in the payout
/// scale and in nothing else.
#[test]
fn the_two_shapes_differ_in_the_payout_scale_and_nothing_else() {
    let (_, refunding) = world(FoundingShapeV1::Refunding, HostileV1::None);
    let (_, categorical) = world(FoundingShapeV1::Categorical, HostileV1::None);
    assert_eq!(refunding.shared.payout_scale, u64::from(CLAIM_COUNT - 1));
    assert_eq!(categorical.shared.payout_scale, 1);
    assert_eq!(
        refunding.request.claim_count(),
        categorical.request.claim_count(),
    );
    assert_eq!(refunding.request.quantity(), categorical.request.quantity());
    assert_eq!(
        refunding.aggregate_rent, categorical.aggregate_rent,
        "the aggregate's width does not depend on the shape",
    );
    assert_eq!(refunding.position_rent, categorical.position_rent);
    assert_eq!(refunding.admission_rent, categorical.admission_rent);
    assert_eq!(
        refunding.core_state.len(),
        categorical.core_state.len(),
        "and neither does the Core state's",
    );
    assert_eq!(
        refunding.instruction_data.len(),
        categorical.instruction_data.len(),
        "the wire does not move between the two shapes",
    );
    assert_eq!(
        refunding.instruction_data.len(),
        CLAIMS_FOUNDING_REQUEST_BYTES_V5 + 640,
        "request, lock receipt, realization receipt",
    );
    assert_ne!(
        refunding.shared.linked_basis.digest, categorical.shared.linked_basis.digest,
        "the RECORD is what says which shape this Market is",
    );
    assert_eq!(CLAIMS_FOUNDING_RECEIPT_BYTES_V5, 1008);
    assert!(
        ClaimsFoundingReceiptV5::decode(&[0; CLAIMS_FOUNDING_RECEIPT_BYTES_V5]).is_err(),
        "a receipt of the right WIDTH and the wrong bytes is refused by the magic, not accepted",
    );
    assert_ne!(
        refunding.bond, 0,
        "the founder bond's size rule has a nonzero floor at this width, which \
         is what MANDATORY means; a zero here would make every bond assertion \
         in this file pass over nothing",
    );
    assert_eq!(
        refunding.bond, categorical.bond,
        "the bond is priced off the width and the rent rate, neither of which \
         the payout scale moves",
    );
}

// ---------------------------------------------------------------------------
// The two conjuncts CLAIMS-17 added, which had never run anywhere
// ---------------------------------------------------------------------------

/// A refunding founding whose escrow is not the MARKET's own refuses `0x5010`.
///
/// `FailureEscrowIdentityV1::derive` became the sole author of the escrow's
/// identity for both the founding that seats it and the complete-set gate that
/// requires it to stay seated, and the point of one author is that the same
/// mistake gets the same code wherever it is made. The account this campaign
/// substitutes is a well-formed, vacant Position of the SAME aggregate under a
/// different owner, so every seed helper succeeds and only the derivation
/// disagrees -- which is the only way to reach the conjunct rather than a
/// vacancy check in front of it.
#[tokio::test]
async fn a_refunding_founding_whose_escrow_is_not_the_markets_own_refuses() {
    let (_, _, outcome) = found(
        FoundingShapeV1::Refunding,
        HostileV1::EscrowIsNotTheMarketsOwn,
        "claims founding: refunding, escrow substituted",
    )
    .await;
    assert!(!outcome.accepted);
    assert_eq!(
        outcome.refusal,
        Some(dclutch_claims_sbf::ClaimsSbfError::FailureEscrow as u32),
        "a founding whose escrow account is not the derived one is the routed \
         split's mistake made one stage earlier, and carries the same code; \
         logs {:#?}",
        outcome.logs,
    );
}

/// A refunding founding whose escrow rent is not prepaid refuses `0x5186 Rent`.
///
/// The escrow's two accounts ride on EVERY founding and are prepaid on exactly
/// the refunding ones, which is what stops a caller signalling a Market's shape
/// by what it funds. This campaign leaves them at zero lamports -- the state a
/// categorical founding is entitled to -- over a record that says the Market
/// refunds, and the founding refuses rather than seating an escrow the founder
/// did not pay for.
#[tokio::test]
async fn a_refunding_founding_whose_escrow_rent_is_not_prepaid_refuses() {
    let (_, _, outcome) = found(
        FoundingShapeV1::Refunding,
        HostileV1::EscrowRentNotPrepaid,
        "claims founding: refunding, escrow rent absent",
    )
    .await;
    assert!(!outcome.accepted);
    assert_eq!(
        outcome.refusal,
        Some(FOUNDING_RENT_REFUSAL_V5),
        "the escrow rent conjunct is the one that must fire; logs {:#?}",
        outcome.logs,
    );
    assert_ne!(
        FOUNDING_RENT_REFUSAL_V5,
        dclutch_claims_sbf::ClaimsSbfError::FailureEscrow as u32,
        "the two escrow-seating refusals must be distinguishable, or this pair \
         and the one above prove one thing between them",
    );
}

/// `ClaimsFoundingSbfErrorV5::Rent`, derived from the registered band rather
/// than typed.
///
/// The founding enum is private to the Claims program, so the discriminant
/// cannot be read off it from here. It is derived instead from the two things
/// that fix it -- the program's registered refusal base and the founding
/// route's own sub-band offset -- so a band move breaks this line rather than
/// silently re-pointing it at another route's code. `0x5186` is the seventh
/// variant of the run that starts at `CLAIMS_REFUSAL_BASE + 0x180`.
const FOUNDING_RENT_REFUSAL_V5: u32 = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x180 + 6;

// ---------------------------------------------------------------------------
// The founder bond, decision 0033
// ---------------------------------------------------------------------------

/// A refunding founding one lamport short of the bond refuses by name.
///
/// The escrow holds strictly MORE than its own rent, so the `Rent` conjunct
/// standing in front of the bond is satisfied and cannot be what fires; the
/// only quantity that moved is the last lamport of the size rule. The rule is a
/// FLOOR -- `founded_v1` admits `recorded_rent + bond <= lamports`, so an
/// over-funded escrow founds and the surplus is bond like any other lamport
/// above the recorded rent -- and what MANDATORY means is that the floor is not
/// zero. This breaks the moment the founding prices the bond at a rate other
/// than the one it funds the escrow at, or reads the size rule at a width other
/// than this Market's.
#[tokio::test]
async fn a_founding_one_lamport_short_of_the_bond_refuses_by_name() {
    let (_, _, outcome) = found(
        FoundingShapeV1::Refunding,
        HostileV1::FounderBondOneLamportShort,
        "claims founding: refunding, founder bond one lamport short",
    )
    .await;
    assert!(!outcome.accepted);
    assert_eq!(
        outcome.refusal,
        Some(ClaimsFoundingSbfErrorV5::FounderBondUnderfunded as u32),
        "the bond conjunct is the one that must fire; logs {:#?}",
        outcome.logs,
    );
    assert!(
        outcome
            .logs
            .iter()
            .any(|line| line.contains("claims founding v5: refused, founder bond underfunded")),
        "and the refusal names itself in the validator log, because the wire \
         carries one u32 and a reader of that log has nothing else; logs {:#?}",
        outcome.logs,
    );
}

/// An escrow at exactly its rent refuses the BOND, not the rent.
///
/// Two mistakes the same host makes -- transferring nothing, and transferring
/// only `Rent::minimum_balance` -- and they must not arrive as one code,
/// because the remedies differ: one is a transfer that was forgotten, the other
/// is a host that has never read decision 0033. This runs both worlds and holds
/// the two discriminants apart, so a refactor that folds the bond conjunct back
/// into `Rent` fails here rather than in a cohort.
#[tokio::test]
async fn an_escrow_holding_its_rent_but_no_bond_refuses_the_bond_not_the_rent() {
    let (_, _, no_bond) = found(
        FoundingShapeV1::Refunding,
        HostileV1::FounderBondAbsent,
        "claims founding: refunding, escrow rent without the bond",
    )
    .await;
    assert!(!no_bond.accepted);
    assert_eq!(
        no_bond.refusal,
        Some(ClaimsFoundingSbfErrorV5::FounderBondUnderfunded as u32),
        "an escrow at exactly its rent is past the rent conjunct and short at \
         the bond's; logs {:#?}",
        no_bond.logs,
    );

    let (_, _, no_rent) = found(
        FoundingShapeV1::Refunding,
        HostileV1::EscrowRentNotPrepaid,
        "claims founding: refunding, escrow below its own rent",
    )
    .await;
    assert!(!no_rent.accepted);
    assert_eq!(
        no_rent.refusal,
        Some(FOUNDING_RENT_REFUSAL_V5),
        "and an escrow below its own rent still refuses the rent; logs {:#?}",
        no_rent.logs,
    );
    assert_ne!(
        ClaimsFoundingSbfErrorV5::FounderBondUnderfunded as u32,
        FOUNDING_RENT_REFUSAL_V5,
        "or the split above proves one thing between the two campaigns",
    );
    assert_eq!(
        FOUNDING_RENT_REFUSAL_V5,
        ClaimsFoundingSbfErrorV5::Rent as u32,
        "the band-derived code and the enum's own must be the same number",
    );
}
